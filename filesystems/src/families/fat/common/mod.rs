// Common FAT filesystem components shared between FAT16, FAT32, and exFAT
// This module provides reusable building blocks to avoid code duplication

pub mod constants;
pub mod boot_sector;
pub mod structures;
pub mod fat_init;
pub mod cluster_calc;
pub mod fat_table;
pub mod validator;
pub mod directory;
pub mod cluster_io;
pub mod timestamps;
pub mod long_names;

pub use constants::*;
pub use boot_sector::*;
pub use structures::*;
pub use fat_init::*;
pub use cluster_calc::*;
pub use fat_table::*;
pub use directory::*;
pub use cluster_io::*;
pub use timestamps::*;

use std::time::SystemTime;

/// Generate a volume serial number based on current time
/// Used by both FAT16 and FAT32
pub fn generate_volume_serial() -> u32 {
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => {
            let secs = duration.as_secs() as u32;
            let nanos = duration.subsec_nanos();
            // Combine seconds and nanoseconds for uniqueness
            secs.wrapping_add(nanos)
        }
        Err(_) => {
            // Fallback to a pseudo-random value
            0x12345678
        }
    }
}

/// Convert a string to FAT volume label format (11 bytes, space-padded)
pub fn format_volume_label(label: Option<&str>) -> [u8; 11] {
    let mut result = [0x20u8; 11]; // Space-padded
    
    if let Some(label) = label {
        let label = label.to_uppercase();
        let bytes = label.as_bytes();
        let len = bytes.len().min(11);
        result[..len].copy_from_slice(&bytes[..len]);
    }
    
    result
}

/// Calculate CHS geometry for a given LBA
/// Used for partition table entries
pub fn lba_to_chs(lba: u32, heads: u16, sectors: u16) -> (u8, u8, u8) {
    let total_sectors = heads as u32 * sectors as u32;
    let cylinder = lba / total_sectors;
    let temp = lba % total_sectors;
    let head = (temp / sectors as u32) as u8;
    let sector = ((temp % sectors as u32) + 1) as u8;
    
    // CHS has limits: 1023 cylinders, 254 heads, 63 sectors
    if cylinder > 1023 {
        // Use maximum CHS values for large disks
        (0xFE, 0xFF, 0xFF)
    } else {
        let cyl_high = ((cylinder >> 2) & 0xC0) as u8;
        let cyl_low = (cylinder & 0xFF) as u8;
        (head, sector | cyl_high, cyl_low)
    }
}