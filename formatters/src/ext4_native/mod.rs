// EXT4 Native Windows Implementation
// Phase 0: Foundation and Infrastructure

pub mod core;
pub mod validation;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(test)]
mod tests;

// Re-export main formatter
pub use self::core::formatter::Ext4NativeFormatter;