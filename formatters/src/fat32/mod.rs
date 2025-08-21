// FAT32 module - formatter and reader

pub mod formatter;
pub mod formatter_native;
pub mod reader;
pub mod reader_improved;
pub mod validator;

// Use the native formatter as default (like FAT16)
pub use formatter_native::Fat32NativeFormatter as Fat32Formatter;
// Keep the system formatter available for testing
pub use formatter::Fat32Formatter as Fat32SystemFormatter;
// Use the improved reader that uses aligned device reading
pub use reader_improved::Fat32ReaderImproved as Fat32Reader;

use crate::detection::FilesystemDetector;

pub struct Fat32Detector;

impl FilesystemDetector for Fat32Detector {
    fn detect(boot_sector: &[u8], _ext_superblock: Option<&[u8]>) -> Option<String> {
        // FAT32 signature is at offset 82: "FAT32"
        if boot_sector.len() >= 87 && &boot_sector[82..87] == b"FAT32" {
            Some("fat32".to_string())
        } else if boot_sector.len() >= 57 && &boot_sector[54..57] == b"FAT" {
            // FAT16/12 - check for "FAT" at offset 54
            Some("fat16".to_string())
        } else {
            None
        }
    }
}