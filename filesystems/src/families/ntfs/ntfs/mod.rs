// NTFS filesystem support module
// Phase 1: Read-only implementation
// Fully portable - no OS-specific dependencies

pub mod structures;
pub mod detector;
pub mod boot_sector;
pub mod mft;
pub mod mft_writer;
pub mod mft_updater;
pub mod resident_data_writer;
pub mod attributes;
pub mod data_runs;
pub mod index;
pub mod index_writer;
pub mod index_updater;
pub mod path_resolver;
pub mod resident_converter;
// pub mod directory_creator;  // TODO: Fix lifetime issue
pub mod file_mover;
pub mod compression;
pub mod sparse;
pub mod attribute_list;
pub mod reparse;
pub mod reader;
pub mod writer;
pub mod writer_ops;
pub mod writer_ops_ext;
pub mod formatter;
pub mod ops;
pub mod ops_rw;
pub mod ops_rw_v2;
pub mod logfile;
pub mod journaled_writer;

// Re-export main types
pub use detector::NtfsDetector;
pub use reader::NtfsReader;
pub use writer::{NtfsWriter, NtfsWriteConfig};
pub use formatter::NtfsFormatter;
pub use ops::NtfsOps;
pub use ops_rw_v2::NtfsRwOps;
pub use structures::*;
pub use journaled_writer::{JournaledNtfsWriter, JournalingConfig};
pub use logfile::{LogFileConfig, LogFileWriter, LogFileReader, LogFileRecovery};