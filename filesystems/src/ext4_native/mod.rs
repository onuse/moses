// EXT4 Native Windows Implementation
// Phase 0: Foundation and Infrastructure

pub mod core;
pub mod reader;
pub mod validation;
pub mod ops;
pub mod writer;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(test)]
mod tests;

// Re-export main formatter
pub use self::core::formatter::Ext4NativeFormatter;
// Re-export reader for filesystem browsing
pub use self::reader::ExtReader;
// Re-export filesystem operations
pub use self::ops::{Ext4Ops, ExtDetector as ExtOpsDetector};

use crate::detection::FilesystemDetector;

pub struct ExtDetector;

impl FilesystemDetector for ExtDetector {
    fn detect(_boot_sector: &[u8], ext_superblock: Option<&[u8]>) -> Option<String> {
        // ext2/3/4 filesystems have their superblock at offset 1024
        // We need the extended superblock data for detection
        if let Some(sb) = ext_superblock {
            // ext2/3/4 magic number at offset 56 (0xEF53)
            if sb.len() >= 58 && sb[56] == 0x53 && sb[57] == 0xEF {
                // Check features to determine ext version
                if sb.len() >= 100 {
                    let incompat = u32::from_le_bytes([sb[96], sb[97], sb[98], sb[99]]);
                    
                    // Check for ext4 features
                    if (incompat & 0x0040) != 0 || // INCOMPAT_64BIT
                       (incompat & 0x0200) != 0 || // INCOMPAT_FLEX_BG
                       (incompat & 0x1000) != 0 {  // INCOMPAT_EXTENTS
                        return Some("ext4".to_string());
                    }
                    
                    // Check for ext3 journal feature
                    if (incompat & 0x0004) != 0 { // INCOMPAT_RECOVER
                        return Some("ext3".to_string());
                    }
                }
                return Some("ext2".to_string());
            }
        }
        None
    }
}