pub mod ext4;
pub mod ntfs;

#[cfg(target_os = "linux")]
pub mod ext4_linux;

#[cfg(target_os = "windows")]
pub mod ext4_windows;

// Re-export formatters
pub use ext4::Ext4Formatter;
pub use ntfs::NtfsFormatter;

#[cfg(target_os = "linux")]
pub use ext4_linux::Ext4LinuxFormatter;

#[cfg(target_os = "windows")]
pub use ext4_windows::Ext4WindowsFormatter;