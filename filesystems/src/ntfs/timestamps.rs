// NTFS Windows Timestamp Handling
// Converts between Unix and Windows FILETIME formats

use std::time::{SystemTime, Duration, UNIX_EPOCH};
use moses_core::MosesError;

/// Windows FILETIME epoch (January 1, 1601 00:00:00 UTC)
/// Number of seconds between 1601 and Unix epoch (1970)
const WINDOWS_EPOCH_DIFF: u64 = 11644473600;

/// FILETIME is in 100-nanosecond intervals
const FILETIME_TICKS_PER_SECOND: u64 = 10_000_000;

/// Converts Windows FILETIME to Unix timestamp
pub fn filetime_to_unix(filetime: u64) -> Option<SystemTime> {
    // Convert from 100ns intervals to seconds
    let seconds = filetime / FILETIME_TICKS_PER_SECOND;
    
    // Subtract the epoch difference
    if seconds >= WINDOWS_EPOCH_DIFF {
        let unix_seconds = seconds - WINDOWS_EPOCH_DIFF;
        let nanos = ((filetime % FILETIME_TICKS_PER_SECOND) * 100) as u32;
        
        UNIX_EPOCH.checked_add(Duration::new(unix_seconds, nanos))
    } else {
        // Time is before Unix epoch
        None
    }
}

/// Converts Unix timestamp to Windows FILETIME
pub fn unix_to_filetime(time: SystemTime) -> u64 {
    if let Ok(duration) = time.duration_since(UNIX_EPOCH) {
        // Get Unix timestamp in seconds and nanoseconds
        let unix_seconds = duration.as_secs();
        let unix_nanos = duration.subsec_nanos() as u64;
        
        // Add epoch difference and convert to FILETIME
        let windows_seconds = unix_seconds + WINDOWS_EPOCH_DIFF;
        let filetime = windows_seconds * FILETIME_TICKS_PER_SECOND + (unix_nanos / 100);
        
        filetime
    } else {
        // Time is before Unix epoch, use Windows epoch
        0
    }
}

/// Get current time as Windows FILETIME
pub fn current_filetime() -> u64 {
    unix_to_filetime(SystemTime::now())
}

/// NTFS timestamps structure (all times in FILETIME format)
#[derive(Debug, Clone, Copy)]
pub struct NtfsTimestamps {
    pub creation_time: u64,
    pub modification_time: u64,
    pub mft_modification_time: u64,
    pub access_time: u64,
}

impl NtfsTimestamps {
    /// Create new timestamps with current time
    pub fn now() -> Self {
        let now = current_filetime();
        Self {
            creation_time: now,
            modification_time: now,
            mft_modification_time: now,
            access_time: now,
        }
    }
    
    /// Create from Unix timestamps
    pub fn from_unix(
        created: Option<SystemTime>,
        modified: Option<SystemTime>,
        accessed: Option<SystemTime>,
    ) -> Self {
        let now = current_filetime();
        
        Self {
            creation_time: created.map(unix_to_filetime).unwrap_or(now),
            modification_time: modified.map(unix_to_filetime).unwrap_or(now),
            mft_modification_time: now, // Always current time for MFT changes
            access_time: accessed.map(unix_to_filetime).unwrap_or(now),
        }
    }
    
    /// Convert to Unix timestamps
    pub fn to_unix(&self) -> (Option<SystemTime>, Option<SystemTime>, Option<SystemTime>) {
        (
            filetime_to_unix(self.creation_time),
            filetime_to_unix(self.modification_time),
            filetime_to_unix(self.access_time),
        )
    }
    
    /// Update modification time
    pub fn touch_modified(&mut self) {
        let now = current_filetime();
        self.modification_time = now;
        self.mft_modification_time = now;
    }
    
    /// Update access time
    pub fn touch_accessed(&mut self) {
        self.access_time = current_filetime();
    }
    
    /// Parse from raw bytes (little-endian)
    pub fn from_bytes(data: &[u8]) -> Result<Self, MosesError> {
        if data.len() < 32 {
            return Err(MosesError::Other("Insufficient data for timestamps".to_string()));
        }
        
        Ok(Self {
            creation_time: u64::from_le_bytes(data[0..8].try_into().unwrap()),
            modification_time: u64::from_le_bytes(data[8..16].try_into().unwrap()),
            mft_modification_time: u64::from_le_bytes(data[16..24].try_into().unwrap()),
            access_time: u64::from_le_bytes(data[24..32].try_into().unwrap()),
        })
    }
    
    /// Serialize to bytes (little-endian)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&self.creation_time.to_le_bytes());
        bytes.extend_from_slice(&self.modification_time.to_le_bytes());
        bytes.extend_from_slice(&self.mft_modification_time.to_le_bytes());
        bytes.extend_from_slice(&self.access_time.to_le_bytes());
        bytes
    }
}

/// Format FILETIME for display
pub fn format_filetime(filetime: u64) -> String {
    if let Some(time) = filetime_to_unix(filetime) {
        if let Ok(duration) = time.duration_since(UNIX_EPOCH) {
            let secs = duration.as_secs();
            let datetime = chrono::DateTime::<chrono::Utc>::from_timestamp(secs as i64, 0);
            if let Some(dt) = datetime {
                return dt.format("%Y-%m-%d %H:%M:%S UTC").to_string();
            }
        }
    }
    format!("Invalid FILETIME: {}", filetime)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_epoch_conversion() {
        // Windows epoch in FILETIME (Jan 1, 1601)
        let windows_epoch = 0u64;
        assert!(filetime_to_unix(windows_epoch).is_none());
        
        // Unix epoch in FILETIME (Jan 1, 1970)
        let unix_epoch_filetime = WINDOWS_EPOCH_DIFF * FILETIME_TICKS_PER_SECOND;
        let unix_time = filetime_to_unix(unix_epoch_filetime).unwrap();
        assert_eq!(unix_time, UNIX_EPOCH);
        
        // Round trip conversion
        let now = SystemTime::now();
        let filetime = unix_to_filetime(now);
        let converted_back = filetime_to_unix(filetime).unwrap();
        
        // Should be very close (within microseconds due to precision)
        let diff = now.duration_since(converted_back).unwrap_or_else(|e| e.duration());
        assert!(diff.as_micros() < 10);
    }
    
    #[test]
    fn test_ntfs_timestamps() {
        let timestamps = NtfsTimestamps::now();
        assert!(timestamps.creation_time > 0);
        assert!(timestamps.modification_time > 0);
        assert!(timestamps.mft_modification_time > 0);
        assert!(timestamps.access_time > 0);
        
        // Test serialization
        let bytes = timestamps.to_bytes();
        assert_eq!(bytes.len(), 32);
        
        let parsed = NtfsTimestamps::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.creation_time, timestamps.creation_time);
        assert_eq!(parsed.modification_time, timestamps.modification_time);
        assert_eq!(parsed.mft_modification_time, timestamps.mft_modification_time);
        assert_eq!(parsed.access_time, timestamps.access_time);
    }
    
    #[test]
    fn test_known_timestamp() {
        // January 1, 2000 00:00:00 UTC in FILETIME
        // This is 30 years and 5 leap days after Unix epoch
        let year_2000_unix = 946684800u64; // Seconds since Unix epoch
        let year_2000_filetime = (year_2000_unix + WINDOWS_EPOCH_DIFF) * FILETIME_TICKS_PER_SECOND;
        
        let converted = filetime_to_unix(year_2000_filetime).unwrap();
        let duration = converted.duration_since(UNIX_EPOCH).unwrap();
        assert_eq!(duration.as_secs(), year_2000_unix);
    }
}