// Shared timestamp handling for FAT family filesystems
// FAT uses MS-DOS date/time format, exFAT uses a similar but extended format

use std::time::{SystemTime, UNIX_EPOCH, Duration};
use chrono::{DateTime, Utc, Datelike, Timelike};

/// Convert FAT date/time to Unix timestamp
/// FAT date: bits 15-9: year (0=1980), bits 8-5: month, bits 4-0: day
/// FAT time: bits 15-11: hours, bits 10-5: minutes, bits 4-0: seconds/2
pub fn fat_datetime_to_unix(date: u16, time: u16) -> u64 {
    let year = ((date >> 9) & 0x7F) as i32 + 1980;
    let month = ((date >> 5) & 0x0F) as u32;
    let day = (date & 0x1F) as u32;
    
    let hour = ((time >> 11) & 0x1F) as u32;
    let minute = ((time >> 5) & 0x3F) as u32;
    let second = ((time & 0x1F) * 2) as u32;  // FAT stores seconds/2
    
    // Create NaiveDateTime using NaiveDate
    use chrono::NaiveDate;
    if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
        if let Some(dt) = date.and_hms_opt(hour, minute, second) {
            dt.and_utc().timestamp() as u64
        } else {
            0  // Invalid time
        }
    } else {
        0  // Invalid date
    }
}

/// Convert Unix timestamp to FAT date/time
pub fn unix_to_fat_datetime(timestamp: u64) -> (u16, u16) {
    let datetime = DateTime::<Utc>::from(UNIX_EPOCH + Duration::from_secs(timestamp));
    
    let year = datetime.year();
    let month = datetime.month();
    let day = datetime.day();
    let hour = datetime.hour();
    let minute = datetime.minute();
    let second = datetime.second();
    
    // Clamp year to FAT range (1980-2107)
    let fat_year = if year < 1980 {
        0
    } else if year > 2107 {
        127
    } else {
        (year - 1980) as u16
    };
    
    let fat_date = (fat_year << 9) | ((month as u16) << 5) | (day as u16);
    let fat_time = ((hour as u16) << 11) | ((minute as u16) << 5) | ((second / 2) as u16);
    
    (fat_date, fat_time)
}

/// Get current FAT date/time
pub fn get_current_fat_datetime() -> (u16, u16) {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => unix_to_fat_datetime(duration.as_secs()),
        Err(_) => (0, 0),  // Fallback to epoch
    }
}

/// exFAT timestamp format (100 nanosecond intervals since 1601-01-01)
/// Also includes timezone offset
#[derive(Debug, Clone, Copy)]
pub struct ExFatTimestamp {
    pub timestamp: u64,      // 100ns intervals since 1601-01-01
    pub timezone_offset: i8, // 15-minute intervals from UTC
    pub centiseconds: u8,    // Additional precision (0-199)
}

impl ExFatTimestamp {
    /// Create from current system time
    pub fn now() -> Self {
        let now = SystemTime::now();
        let unix_secs = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        
        // Convert Unix epoch (1970) to Windows epoch (1601)
        // Difference is 11644473600 seconds
        const EPOCH_DIFF: u64 = 11644473600;
        let windows_secs = unix_secs + EPOCH_DIFF;
        let timestamp = windows_secs * 10_000_000;  // Convert to 100ns intervals
        
        Self {
            timestamp,
            timezone_offset: 0,  // UTC
            centiseconds: 0,
        }
    }
    
    /// Convert to FAT-style date/time for compatibility
    pub fn to_fat_datetime(&self) -> (u16, u16) {
        const EPOCH_DIFF: u64 = 11644473600;
        let unix_secs = (self.timestamp / 10_000_000).saturating_sub(EPOCH_DIFF);
        unix_to_fat_datetime(unix_secs)
    }
    
    /// Create from FAT date/time
    pub fn from_fat_datetime(date: u16, time: u16) -> Self {
        let unix_secs = fat_datetime_to_unix(date, time);
        const EPOCH_DIFF: u64 = 11644473600;
        let windows_secs = unix_secs + EPOCH_DIFF;
        
        Self {
            timestamp: windows_secs * 10_000_000,
            timezone_offset: 0,
            centiseconds: 0,
        }
    }
}

/// Convert exFAT date/time fields to timestamp
/// Used for parsing directory entries
pub fn exfat_fields_to_timestamp(
    date: u16,
    time: u16,
    centiseconds: u8,
    timezone: u8,
) -> ExFatTimestamp {
    // exFAT uses similar format to FAT but with extra precision
    let base_timestamp = ExFatTimestamp::from_fat_datetime(date, time);
    
    ExFatTimestamp {
        timestamp: base_timestamp.timestamp + (centiseconds as u64 * 100_000),
        timezone_offset: timezone as i8,
        centiseconds,
    }
}

/// Encode exFAT timestamp to directory entry fields
pub fn exfat_timestamp_to_fields(ts: &ExFatTimestamp) -> (u16, u16, u8, u8) {
    let (date, time) = ts.to_fat_datetime();
    (date, time, ts.centiseconds, ts.timezone_offset as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fat_datetime_conversion() {
        // Test known date: 2024-01-15 14:30:00
        let (date, time) = unix_to_fat_datetime(1705330200);
        
        // Verify round-trip
        let unix = fat_datetime_to_unix(date, time);
        assert_eq!(unix, 1705330200);
    }
    
    #[test]
    fn test_exfat_timestamp() {
        let ts = ExFatTimestamp::now();
        assert!(ts.timestamp > 0);
        
        // Test conversion to FAT format
        let (date, time) = ts.to_fat_datetime();
        assert!(date > 0);
        assert!(time > 0);
    }
}