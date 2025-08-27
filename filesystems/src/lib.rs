pub mod ntfs;
pub mod fat16;
pub mod fat32;
pub mod exfat;
pub mod registration;
pub mod utils;
pub mod detection;
pub mod device_reader;
pub mod device_writer;
pub mod diagnostics;
pub mod diagnostics_improved;
pub mod partitioner;
pub mod disk_manager;
pub mod fat_common;
pub mod ops;
pub mod ops_helpers;
pub mod ops_registry;

#[cfg(test)]
pub mod test_helpers;

#[cfg(feature = "mount")]
pub mod mount;

// Legacy implementations (kept for reference, not exposed by default)
#[cfg(feature = "legacy")]
pub mod legacy;

// Native ext4 implementation - used for all platforms
pub mod ext4_native;
pub use ext4_native::{Ext4NativeFormatter, ExtReader, Ext4Ops};

// Extended ext family support (ext2/ext3) using ext4_native base
pub mod ext_family;
pub use ext_family::{Ext2Formatter, Ext3Formatter};

// Re-export formatters and readers
// NTFS implementation - read and format support
pub use ntfs::{NtfsDetector, NtfsReader, NtfsFormatter, NtfsOps, NtfsRwOps};
pub use fat16::{Fat16Formatter, Fat16Reader, Fat16Ops};
pub use fat32::{Fat32Formatter, Fat32Reader};
pub use exfat::{ExFatFormatter, ExFatReader, ExFatOps};

// Deprecated - using Ext4NativeFormatter instead
// #[cfg(target_os = "windows")]
// pub use ext4_windows::Ext4WindowsFormatter;

// Deprecated - using native NTFS implementation instead
// #[cfg(target_os = "windows")]
// pub use ntfs_windows::NtfsWindowsFormatter;

// Re-export registration functions
pub use registration::{register_builtin_formatters, list_available_formatters, get_formatter_info};

// Re-export filesystem operations
pub use ops::{
    FilesystemOps, FilesystemOpsRegistry, FilesystemDetector, 
    FileAttributes, DirectoryEntry, FilesystemInfo, register_builtin_ops,
    MountSource, SubfolderOps, HostFolderOps
};
pub use ops_registry::register_all_filesystems;