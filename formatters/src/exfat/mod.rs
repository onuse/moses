// exFAT module - formatter and reader

pub mod formatter;
pub mod reader;
pub mod reader_improved;

pub use formatter::ExFatFormatter;
// Use the improved reader that keeps file handle open
pub use reader_improved::ExFatReaderImproved as ExFatReader;

use crate::detection::FilesystemDetector;

pub struct ExFatDetector;

impl FilesystemDetector for ExFatDetector {
    fn detect(boot_sector: &[u8], _ext_superblock: Option<&[u8]>) -> Option<String> {
        // exFAT signature is at offset 3: "EXFAT   "
        if boot_sector.len() >= 11 && &boot_sector[3..11] == b"EXFAT   " {
            Some("exfat".to_string())
        } else {
            None
        }
    }
}