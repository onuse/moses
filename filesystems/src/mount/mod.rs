// Filesystem mounting support for Moses Bridge
// Provides native filesystem access through OS-specific APIs

#[cfg(all(target_os = "windows", feature = "mount-windows"))]
pub mod winfsp;

#[cfg(all(unix, feature = "mount-unix"))]
pub mod fuse;

use crate::ops::FilesystemOps;
use moses_core::{Device, MosesError};
use std::path::Path;

/// Mount options for filesystem mounting
#[derive(Debug, Clone)]
pub struct MountOptions {
    pub readonly: bool,
    pub mount_point: String,
    pub filesystem_type: Option<String>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub allow_other: bool,
    pub direct_io: bool,
    pub max_read: Option<u32>,
}

impl Default for MountOptions {
    fn default() -> Self {
        Self {
            readonly: true,  // Default to read-only for safety
            mount_point: String::new(),
            filesystem_type: None,
            uid: None,
            gid: None,
            allow_other: false,
            direct_io: false,
            max_read: Some(128 * 1024), // 128KB default
        }
    }
}

/// Common mount interface
pub trait MountProvider {
    /// Mount a filesystem
    fn mount(
        &mut self,
        device: &Device,
        ops: Box<dyn FilesystemOps>,
        options: &MountOptions,
    ) -> Result<(), MosesError>;
    
    /// Unmount a filesystem
    fn unmount(&mut self, mount_point: &Path) -> Result<(), MosesError>;
    
    /// Check if a mount point is active
    fn is_mounted(&self, mount_point: &Path) -> bool;
}

/// Get the appropriate mount provider for the current platform
pub fn get_mount_provider() -> Result<Box<dyn MountProvider>, MosesError> {
    #[cfg(all(target_os = "windows", feature = "mount-windows"))]
    {
        Ok(Box::new(winfsp::WinFspMount::new()?))
    }
    
    #[cfg(all(unix, feature = "mount-unix"))]
    {
        Ok(Box::new(fuse::FuseMount::new()?))
    }
    
    #[cfg(not(any(
        all(target_os = "windows", feature = "mount-windows"),
        all(unix, feature = "mount-unix")
    )))]
    {
        Err(MosesError::NotSupported(
            "Mounting not supported on this platform or feature not enabled".to_string()
        ))
    }
}