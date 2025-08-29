// exFAT module - native formatter and reader implementation

pub mod formatter;
pub mod formatter_native;
pub mod reader;
pub mod reader_improved;
pub mod reader_aligned;
pub mod writer;
pub mod structures;
pub mod bitmap;
pub mod upcase;
pub mod validator;
pub mod directory_entries;
pub mod file_operations;
pub mod ops;

// Use the native formatter as default
pub use formatter_native::ExFatNativeFormatter as ExFatFormatter;
// Keep the system formatter available for compatibility
pub use formatter::ExFatFormatter as ExFatSystemFormatter;
// Use the aligned reader that leverages our common abstraction
pub use reader_aligned::ExFatReaderAligned as ExFatReader;
pub use writer::ExFatWriter;
pub use ops::ExFatOps;

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