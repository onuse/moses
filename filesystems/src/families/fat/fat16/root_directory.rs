// FAT16 Root Directory Entry structures and creation

use std::time::SystemTime;

/// FAT16 Directory Entry (32 bytes)
#[repr(C, packed(1))]
#[derive(Debug, Clone, Copy)]
pub struct Fat16DirEntry {
    pub name: [u8; 8],        // 0x00: File name (8 bytes, space padded)
    pub ext: [u8; 3],         // 0x08: Extension (3 bytes, space padded)
    pub attributes: u8,       // 0x0B: File attributes
    pub reserved: u8,         // 0x0C: Reserved (usually 0)
    pub creation_time_tenth: u8, // 0x0D: Creation time in tenths of second
    pub creation_time: u16,   // 0x0E: Creation time (DOS format)
    pub creation_date: u16,   // 0x10: Creation date (DOS format)
    pub last_access_date: u16, // 0x12: Last access date
    pub first_cluster_hi: u16, // 0x14: High 16 bits of first cluster (0 for FAT16)
    pub write_time: u16,      // 0x16: Last write time
    pub write_date: u16,      // 0x18: Last write date
    pub first_cluster_lo: u16, // 0x1A: Low 16 bits of first cluster
    pub file_size: u32,       // 0x1C: File size in bytes
}

/// File attribute flags
pub mod attributes {
    pub const READ_ONLY: u8 = 0x01;
    pub const HIDDEN: u8 = 0x02;
    pub const SYSTEM: u8 = 0x04;
    pub const VOLUME_ID: u8 = 0x08;  // Volume label entry
    pub const DIRECTORY: u8 = 0x10;
    pub const ARCHIVE: u8 = 0x20;
    pub const LFN: u8 = READ_ONLY | HIDDEN | SYSTEM | VOLUME_ID; // Long filename
}

impl Fat16DirEntry {
    /// Create an empty (deleted) directory entry
    pub fn empty() -> Self {
        let mut entry = Self {
            name: [0; 8],
            ext: [0; 3],
            attributes: 0,
            reserved: 0,
            creation_time_tenth: 0,
            creation_time: 0,
            creation_date: 0,
            last_access_date: 0,
            first_cluster_hi: 0,
            write_time: 0,
            write_date: 0,
            first_cluster_lo: 0,
            file_size: 0,
        };
        entry.name[0] = 0xE5; // Mark as deleted/empty
        entry
    }
    
    /// Create a volume label entry
    pub fn volume_label(label: Option<&str>) -> Self {
        let mut entry = Self {
            name: [b' '; 8],
            ext: [b' '; 3],
            attributes: attributes::VOLUME_ID,
            reserved: 0,
            creation_time_tenth: 0,
            creation_time: 0,
            creation_date: 0,
            last_access_date: 0,
            first_cluster_hi: 0,
            write_time: 0,
            write_date: 0,
            first_cluster_lo: 0,
            file_size: 0,
        };
        
        // Set the volume label (11 characters total: 8 name + 3 ext)
        if let Some(label) = label {
            let label_upper = label.to_uppercase();
            let label_bytes = label_upper.as_bytes();
            
            // Copy up to 11 characters into name and ext fields
            let len = label_bytes.len().min(11);
            if len <= 8 {
                entry.name[..len].copy_from_slice(&label_bytes[..len]);
            } else {
                entry.name.copy_from_slice(&label_bytes[..8]);
                let ext_len = len - 8;
                entry.ext[..ext_len].copy_from_slice(&label_bytes[8..len]);
            }
        } else {
            // Default label "NO NAME"
            entry.name[..7].copy_from_slice(b"NO NAME");
        }
        
        // Set creation date/time to current time
        let (date, time) = get_dos_datetime();
        entry.creation_date = date;
        entry.creation_time = time;
        entry.write_date = date;
        entry.write_time = time;
        entry.last_access_date = date;
        
        entry
    }
}

/// Convert current system time to DOS date/time format
fn get_dos_datetime() -> (u16, u16) {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    
    // Simple conversion (not accounting for timezone)
    // DOS epoch is January 1, 1980
    let seconds_since_1980 = now.as_secs().saturating_sub(315532800); // Unix time for 1980-01-01
    let days_since_1980 = (seconds_since_1980 / 86400) as u32;
    
    // Approximate date calculation (simplified, doesn't handle leap years perfectly)
    let years = days_since_1980 / 365;
    let remaining_days = days_since_1980 % 365;
    let months = remaining_days / 30;
    let days = (remaining_days % 30) + 1;
    
    // DOS date format: bits 15-9: year (0-127, from 1980), bits 8-5: month (1-12), bits 4-0: day (1-31)
    let dos_date = ((years.min(127) as u16) << 9) | 
                   (((months + 1).min(12) as u16) << 5) | 
                   (days.min(31) as u16);
    
    // Time within the day
    let seconds_today = seconds_since_1980 % 86400;
    let hours = seconds_today / 3600;
    let minutes = (seconds_today % 3600) / 60;
    let seconds = (seconds_today % 60) / 2; // DOS stores seconds/2
    
    // DOS time format: bits 15-11: hours (0-23), bits 10-5: minutes (0-59), bits 4-0: seconds/2 (0-29)
    let dos_time = ((hours.min(23) as u16) << 11) | 
                   ((minutes.min(59) as u16) << 5) | 
                   (seconds.min(29) as u16);
    
    (dos_date, dos_time)
}

/// Create a root directory with volume label
pub fn create_root_directory_with_label(root_entries: u16, volume_label: Option<&str>) -> Vec<u8> {
    let root_dir_size = root_entries as usize * 32;
    let mut root_dir = vec![0u8; root_dir_size];
    
    // First entry is the volume label
    if volume_label.is_some() || true { // Always create a volume label entry
        let label_entry = Fat16DirEntry::volume_label(volume_label);
        
        // Write the volume label entry at the beginning
        let entry_bytes = unsafe {
            std::slice::from_raw_parts(
                &label_entry as *const _ as *const u8,
                std::mem::size_of::<Fat16DirEntry>()
            )
        };
        root_dir[..32].copy_from_slice(entry_bytes);
        
        // Mark second entry as end of directory (optional but good practice)
        // All remaining entries are already zeroed
    }
    
    root_dir
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_volume_label_creation() {
        let label_entry = Fat16DirEntry::volume_label(Some("TEST DISK"));
        
        assert_eq!(&label_entry.name, b"TEST DIS");
        assert_eq!(&label_entry.ext, b"K  ");
        assert_eq!(label_entry.attributes, attributes::VOLUME_ID);
        // Copy fields to avoid unaligned access
        let first_cluster = label_entry.first_cluster_lo;
        let file_size = label_entry.file_size;
        assert_eq!(first_cluster, 0);
        assert_eq!(file_size, 0);
    }
    
    #[test]
    fn test_short_label() {
        let label_entry = Fat16DirEntry::volume_label(Some("MYDISK"));
        
        assert_eq!(&label_entry.name, b"MYDISK  ");
        assert_eq!(&label_entry.ext, b"   ");
    }
    
    #[test]
    fn test_root_directory_creation() {
        let root_dir = create_root_directory_with_label(512, Some("TEST VOL"));
        
        assert_eq!(root_dir.len(), 512 * 32);
        
        // Check first entry is volume label
        assert_eq!(&root_dir[0..8], b"TEST VOL");
        assert_eq!(root_dir[11], attributes::VOLUME_ID);
    }
}