pub mod ext4;
pub mod ntfs;
pub mod fat16;
pub mod fat32;
pub mod exfat;
pub mod registration;
pub mod safe_ext4;
pub mod utils;
pub mod detection;
pub mod device_reader;
pub mod diagnostics;

#[cfg(target_os = "linux")]
pub mod ext4_linux;

// Legacy implementations (kept for reference, not exposed by default)
#[cfg(feature = "legacy")]
pub mod legacy;

// New native ext4 implementation
pub mod ext4_native;
pub use ext4_native::Ext4NativeFormatter;

// Extended ext family support (ext2/ext3) using ext4_native base
// This doesn't modify ext4_native, just adds new formatters
pub mod ext_family;
pub use ext_family::{Ext2Formatter, Ext3Formatter};

#[cfg(target_os = "windows")]
pub mod ntfs_windows;

// Re-export formatters and readers
pub use ext4::Ext4Formatter;
pub use ntfs::{NtfsFormatter, NtfsReader};
pub use fat16::{Fat16Formatter, Fat16Reader};
pub use fat32::{Fat32Formatter, Fat32Reader};
pub use exfat::{ExFatFormatter, ExFatReader};

#[cfg(target_os = "linux")]
pub use ext4_linux::Ext4LinuxFormatter;

// Deprecated - using Ext4NativeFormatter instead
// #[cfg(target_os = "windows")]
// pub use ext4_windows::Ext4WindowsFormatter;

#[cfg(target_os = "windows")]
pub use ntfs_windows::NtfsWindowsFormatter;

// Re-export registration functions
pub use registration::{register_builtin_formatters, list_available_formatters, get_formatter_info};