// NTFS filesystem support module
// Phase 1: Read-only implementation
// Fully portable - no OS-specific dependencies

pub mod structures;
pub mod detector;
pub mod boot_sector;
pub mod mft;
pub mod mft_writer;
pub mod attributes;
pub mod data_runs;
pub mod index;
pub mod compression;
pub mod sparse;
pub mod attribute_list;
pub mod reparse;
pub mod reader;
pub mod writer;
pub mod formatter;

// Re-export main types
pub use detector::NtfsDetector;
pub use reader::NtfsReader;
pub use formatter::NtfsFormatter;
pub use structures::*;