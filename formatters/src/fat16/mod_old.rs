// FAT16 module - formatter and reader

pub mod formatter;
pub mod formatter_fixed;
pub mod formatter_compliant;
pub mod system_formatter;
pub mod reader;
pub mod verifier;
pub mod spec_compliance_test;
pub mod root_directory;

#[cfg(test)]
mod tests;

// Use the compliant formatter as the default
pub use formatter_compliant::Fat16CompliantFormatter as Fat16Formatter;
// Keep the old ones available for testing
pub use formatter_fixed::Fat16FormatterFixed;
pub use formatter::Fat16Formatter as Fat16FormatterOriginal;
pub use reader::Fat16Reader;
pub use verifier::{Fat16Verifier, VerificationResult};

use crate::detection::FilesystemDetector;

pub struct Fat16Detector;

impl FilesystemDetector for Fat16Detector {
    fn detect(boot_sector: &[u8], _ext_superblock: Option<&[u8]>) -> Option<String> {
        // FAT16 detection:
        // - Check for "FAT16" at offset 54
        // - Or "FAT" at offset 54 with specific sector counts
        if boot_sector.len() >= 62 {
            if &boot_sector[54..59] == b"FAT16" {
                return Some("fat16".to_string());
            } else if &boot_sector[54..57] == b"FAT" {
                // Additional checks for FAT16 vs FAT12
                // FAT16 typically has more than 4085 clusters
                // This is a simplified check
                return Some("fat16".to_string());
            }
        }
        None
    }
}pub mod detection;
// Use proper cluster-count-based detector
pub use detection::Fat16ProperDetector as Fat16Detector;
