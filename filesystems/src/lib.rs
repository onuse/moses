// Filesystem families organization
pub mod families;

// Filesystem modules are now organized in families
pub mod registration;
pub mod utils;
pub mod detection;
pub mod device_reader;
pub mod device_writer;
pub mod diagnostics_improved;
pub mod partitioner;
pub mod disk_manager;
// FAT common module now in families/fat/common
pub mod ops;
pub mod ops_helpers;
pub mod ops_registry;

pub mod error_recovery;
#[cfg(test)]
pub mod test_helpers;

#[cfg(feature = "mount")]
pub mod mount;


// Native ext4 implementation - used for all platforms
pub use families::ext::ext4_native::{Ext4NativeFormatter, ExtReader, Ext4Ops};

// Extended ext family support (ext2/ext3) using ext4_native base
pub use families::ext::{Ext2Formatter, Ext3Formatter};

// Re-export formatters and readers
// NTFS implementation - read and format support
pub use families::ntfs::ntfs::{NtfsDetector, NtfsReader, NtfsFormatter, NtfsOps, NtfsRwOps};
pub use families::fat::fat16::{Fat16Formatter, Fat16Reader, Fat16Ops};
pub use families::fat::fat32::{Fat32Formatter, Fat32Reader, Fat32Ops};
pub use families::fat::exfat::{ExFatFormatter, ExFatReader, ExFatOps};


// Re-export registration functions
pub use registration::{register_builtin_formatters, list_available_formatters, get_formatter_info};

// Re-export filesystem operations
pub use ops::{
    FilesystemOps, FilesystemOpsRegistry, FilesystemDetector, 
    FileAttributes, DirectoryEntry, FilesystemInfo, register_builtin_ops,
    MountSource, SubfolderOps, HostFolderOps
};
pub use ops_registry::register_all_filesystems;