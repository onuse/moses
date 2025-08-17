// NTFS module - formatter and reader

pub mod formatter;
pub mod reader;

pub use formatter::NtfsFormatter;
pub use reader::NtfsReader;

use crate::detection::FilesystemDetector;

pub struct NtfsDetector;

impl FilesystemDetector for NtfsDetector {
    fn detect(boot_sector: &[u8], _ext_superblock: Option<&[u8]>) -> Option<String> {
        // NTFS signature is at offset 3: "NTFS    "
        if boot_sector.len() >= 8 && &boot_sector[3..8] == b"NTFS " {
            Some("ntfs".to_string())
        } else {
            None
        }
    }
}