pub mod ext4;
pub mod ntfs;
pub mod fat32;
pub mod exfat;
pub mod registration;
pub mod safe_ext4;

#[cfg(target_os = "linux")]
pub mod ext4_linux;

#[cfg(target_os = "windows")]
pub mod ext4_windows;

// New native ext4 implementation
pub mod ext4_native;
pub use ext4_native::Ext4NativeFormatter;

#[cfg(target_os = "windows")]
pub mod ntfs_windows;

// Re-export formatters
pub use ext4::Ext4Formatter;
pub use ntfs::NtfsFormatter;
pub use fat32::Fat32Formatter;
pub use exfat::ExFatFormatter;

#[cfg(target_os = "linux")]
pub use ext4_linux::Ext4LinuxFormatter;

#[cfg(target_os = "windows")]
pub use ext4_windows::Ext4WindowsFormatter;

#[cfg(target_os = "windows")]
pub use ntfs_windows::NtfsWindowsFormatter;

// Re-export registration functions
pub use registration::{register_builtin_formatters, list_available_formatters, get_formatter_info};