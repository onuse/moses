// Legacy formatters - kept for reference but not actively used
// These implementations have been replaced by better alternatives

#[cfg(target_os = "windows")]
pub mod ext4_windows;  // Replaced by ext4_native - WSL dependency removed

// To access legacy formatters (not recommended for production):
// use moses_filesystems::legacy::ext4_windows::Ext4WindowsFormatter;