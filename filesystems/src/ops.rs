// Universal filesystem operations trait
// This trait provides a common interface for all filesystem operations,
// enabling Moses to read, write, and mount any filesystem on any platform

use moses_core::{Device, MosesError};
use std::path::{Path, PathBuf};

/// File attributes returned by stat operations
#[derive(Debug, Clone)]
pub struct FileAttributes {
    pub size: u64,
    pub is_directory: bool,
    pub is_file: bool,
    pub is_symlink: bool,
    pub created: Option<u64>,     // Unix timestamp
    pub modified: Option<u64>,     // Unix timestamp
    pub accessed: Option<u64>,     // Unix timestamp
    pub permissions: u32,          // Unix-style permissions
    pub owner: Option<u32>,        // UID
    pub group: Option<u32>,        // GID
}

/// Directory entry returned by readdir operations
#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    pub name: String,
    pub attributes: FileAttributes,
}

/// Core filesystem operations trait
/// All operations are synchronous to match WinFsp/FUSE requirements
pub trait FilesystemOps: Send + Sync {
    /// Initialize the filesystem for a device
    fn init(&mut self, device: &Device) -> Result<(), MosesError>;
    
    /// Get filesystem information
    fn statfs(&self) -> Result<FilesystemInfo, MosesError>;
    
    /// Get file/directory attributes
    fn stat(&mut self, path: &Path) -> Result<FileAttributes, MosesError>;
    
    /// List directory contents
    fn readdir(&mut self, path: &Path) -> Result<Vec<DirectoryEntry>, MosesError>;
    
    /// Read file contents
    fn read(&mut self, path: &Path, offset: u64, size: u32) -> Result<Vec<u8>, MosesError>;
    
    /// Write file contents (optional - not all filesystems support write)
    fn write(&mut self, _path: &Path, _offset: u64, _data: &[u8]) -> Result<u32, MosesError> {
        Err(MosesError::NotSupported("Filesystem is read-only".to_string()))
    }
    
    /// Create a file (optional)
    fn create(&mut self, _path: &Path, _mode: u32) -> Result<(), MosesError> {
        Err(MosesError::NotSupported("Filesystem is read-only".to_string()))
    }
    
    /// Create a directory (optional)
    fn mkdir(&mut self, _path: &Path, _mode: u32) -> Result<(), MosesError> {
        Err(MosesError::NotSupported("Filesystem is read-only".to_string()))
    }
    
    /// Remove a file (optional)
    fn unlink(&mut self, _path: &Path) -> Result<(), MosesError> {
        Err(MosesError::NotSupported("Filesystem is read-only".to_string()))
    }
    
    /// Remove a directory (optional)
    fn rmdir(&mut self, _path: &Path) -> Result<(), MosesError> {
        Err(MosesError::NotSupported("Filesystem is read-only".to_string()))
    }
    
    /// Rename a file or directory (optional)
    fn rename(&mut self, _from: &Path, _to: &Path) -> Result<(), MosesError> {
        Err(MosesError::NotSupported("Filesystem is read-only".to_string()))
    }
    
    /// Truncate a file (optional)
    fn truncate(&mut self, _path: &Path, _size: u64) -> Result<(), MosesError> {
        Err(MosesError::NotSupported("Filesystem is read-only".to_string()))
    }
    
    /// Flush any pending writes
    fn sync(&mut self) -> Result<(), MosesError> {
        Ok(()) // No-op for read-only filesystems
    }
    
    /// Check if filesystem supports writes
    fn is_readonly(&self) -> bool {
        true // Default to read-only
    }
    
    /// Get filesystem type name (e.g., "ext4", "ntfs", "fat32")
    fn filesystem_type(&self) -> &str;
}

/// Filesystem information
#[derive(Debug, Clone)]
pub struct FilesystemInfo {
    pub total_space: u64,
    pub free_space: u64,
    pub available_space: u64,
    pub total_inodes: u64,
    pub free_inodes: u64,
    pub block_size: u32,
    pub fragment_size: u32,
    pub max_filename_length: u32,
    pub filesystem_type: String,
    pub volume_label: Option<String>,
    pub volume_uuid: Option<String>,
    pub is_readonly: bool,
}

/// Filesystem detector trait - identifies filesystem type from device
pub trait FilesystemDetector: Send + Sync {
    /// Detect filesystem type on a device
    /// Returns the filesystem type name if detected, None otherwise
    fn detect(&self, device: &Device) -> Result<Option<String>, MosesError>;
    
    /// Get priority for this detector (higher = checked first)
    fn priority(&self) -> i32 {
        0
    }
}

/// Registry for filesystem operations
pub struct FilesystemOpsRegistry {
    ops: std::collections::HashMap<String, Box<dyn Fn(&Device) -> Result<Box<dyn FilesystemOps>, MosesError>>>,
    detectors: Vec<Box<dyn FilesystemDetector>>,
}

impl FilesystemOpsRegistry {
    pub fn new() -> Self {
        Self {
            ops: std::collections::HashMap::new(),
            detectors: Vec::new(),
        }
    }
    
    /// Register a filesystem operations factory
    pub fn register_ops<F>(&mut self, filesystem_type: &str, factory: F)
    where
        F: Fn(&Device) -> Result<Box<dyn FilesystemOps>, MosesError> + 'static,
    {
        self.ops.insert(filesystem_type.to_string(), Box::new(factory));
    }
    
    /// Register a filesystem detector
    pub fn register_detector(&mut self, detector: Box<dyn FilesystemDetector>) {
        self.detectors.push(detector);
        // Sort by priority (highest first)
        self.detectors.sort_by_key(|d| -d.priority());
    }
    
    /// Create filesystem operations for a device
    pub fn create_ops(&self, device: &Device, filesystem_type: Option<&str>) -> Result<Box<dyn FilesystemOps>, MosesError> {
        // If filesystem type is specified, use it directly
        if let Some(fs_type) = filesystem_type {
            if let Some(factory) = self.ops.get(fs_type) {
                return factory(device);
            }
            return Err(MosesError::NotSupported(format!("Filesystem type '{}' not supported", fs_type)));
        }
        
        // Otherwise, detect the filesystem type
        for detector in &self.detectors {
            if let Some(detected_type) = detector.detect(device)? {
                if let Some(factory) = self.ops.get(&detected_type) {
                    return factory(device);
                }
            }
        }
        
        Err(MosesError::NotSupported("Could not detect filesystem type".to_string()))
    }
    
    /// List supported filesystem types
    pub fn supported_types(&self) -> Vec<String> {
        self.ops.keys().cloned().collect()
    }
}

impl Default for FilesystemOpsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ===== Extended Operations for Subfolder and Host Mounting =====

/// Extended mount source options beyond just devices
#[derive(Debug, Clone)]
pub enum MountSource {
    /// Mount entire device from root
    Device(Device),
    /// Mount a specific path within a device
    DevicePath {
        device: Device,
        base_path: PathBuf,
    },
    /// Mount a folder from the host filesystem directly
    HostPath(PathBuf),
}

/// Wrapper that adds base path support to any FilesystemOps
/// This allows mounting any subfolder as if it were the root
pub struct SubfolderOps {
    inner: Box<dyn FilesystemOps>,
    base_path: PathBuf,
}

impl SubfolderOps {
    pub fn new(mut inner: Box<dyn FilesystemOps>, device: &Device, base_path: PathBuf) -> Result<Self, MosesError> {
        // Initialize the inner ops with the device
        inner.init(device)?;
        
        // Verify the base path exists and is a directory
        let attrs = inner.stat(&base_path)?;
        if !attrs.is_directory {
            return Err(MosesError::InvalidInput(format!(
                "{} is not a directory", 
                base_path.display()
            )));
        }
        
        Ok(Self {
            inner,
            base_path,
        })
    }
    
    /// Convert an external path to internal path
    fn to_internal_path(&self, path: &Path) -> PathBuf {
        if path == Path::new("/") {
            self.base_path.clone()
        } else {
            // Strip leading slash and join with base path
            let stripped = path.strip_prefix("/").unwrap_or(path);
            self.base_path.join(stripped)
        }
    }
}

impl FilesystemOps for SubfolderOps {
    fn init(&mut self, _device: &Device) -> Result<(), MosesError> {
        Ok(()) // Already initialized in new()
    }
    
    fn statfs(&self) -> Result<FilesystemInfo, MosesError> {
        let mut info = self.inner.statfs()?;
        info.filesystem_type = format!("{} (subfolder)", info.filesystem_type);
        Ok(info)
    }
    
    fn stat(&mut self, path: &Path) -> Result<FileAttributes, MosesError> {
        let internal_path = self.to_internal_path(path);
        self.inner.stat(&internal_path)
    }
    
    fn readdir(&mut self, path: &Path) -> Result<Vec<DirectoryEntry>, MosesError> {
        let internal_path = self.to_internal_path(path);
        self.inner.readdir(&internal_path)
    }
    
    fn read(&mut self, path: &Path, offset: u64, size: u32) -> Result<Vec<u8>, MosesError> {
        let internal_path = self.to_internal_path(path);
        self.inner.read(&internal_path, offset, size)
    }
    
    fn filesystem_type(&self) -> &str {
        self.inner.filesystem_type()
    }
    
    fn is_readonly(&self) -> bool {
        self.inner.is_readonly()
    }
}

/// Host filesystem operations - mount any folder from the host OS as a drive
pub struct HostFolderOps {
    base_path: PathBuf,
    fs_type: String,
}

impl HostFolderOps {
    pub fn new(path: PathBuf) -> Result<Self, MosesError> {
        if !path.exists() {
            return Err(MosesError::InvalidInput(format!(
                "Path {} does not exist",
                path.display()
            )));
        }
        
        if !path.is_dir() {
            return Err(MosesError::InvalidInput(format!(
                "Path {} is not a directory",
                path.display()
            )));
        }
        
        // Detect host filesystem type
        let fs_type = if cfg!(windows) {
            "NTFS"
        } else if cfg!(target_os = "macos") {
            "APFS"
        } else {
            "ext4"
        }.to_string();
        
        Ok(Self {
            base_path: path,
            fs_type,
        })
    }
}

impl FilesystemOps for HostFolderOps {
    fn init(&mut self, _device: &Device) -> Result<(), MosesError> {
        Ok(())
    }
    
    fn statfs(&self) -> Result<FilesystemInfo, MosesError> {
        Ok(FilesystemInfo {
            total_space: 0, // Would need platform-specific code for real values
            free_space: 0,
            available_space: 0,
            total_inodes: 0,
            free_inodes: 0,
            block_size: 4096,
            fragment_size: 4096,
            max_filename_length: 255,
            filesystem_type: format!("host:{}", self.fs_type),
            volume_label: Some(self.base_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("folder")
                .to_string()),
            volume_uuid: None,
            is_readonly: false,
        })
    }
    
    fn stat(&mut self, path: &Path) -> Result<FileAttributes, MosesError> {
        use std::fs;
        
        let full_path = if path == Path::new("/") {
            self.base_path.clone()
        } else {
            self.base_path.join(path.strip_prefix("/").unwrap_or(path))
        };
        
        let metadata = fs::metadata(&full_path)
            .map_err(|e| MosesError::IoError(e))?;
        
        let modified = metadata.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
        
        let accessed = metadata.accessed()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
        
        let created = metadata.created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
        
        Ok(FileAttributes {
            size: metadata.len(),
            is_directory: metadata.is_dir(),
            is_file: metadata.is_file(),
            is_symlink: metadata.file_type().is_symlink(),
            created,
            modified,
            accessed,
            permissions: 0o755,
            owner: None,
            group: None,
        })
    }
    
    fn readdir(&mut self, path: &Path) -> Result<Vec<DirectoryEntry>, MosesError> {
        use std::fs;
        
        let full_path = if path == Path::new("/") {
            self.base_path.clone()
        } else {
            self.base_path.join(path.strip_prefix("/").unwrap_or(path))
        };
        
        let mut entries = Vec::new();
        
        for entry in fs::read_dir(&full_path).map_err(|e| MosesError::IoError(e))? {
            let entry = entry.map_err(|e| MosesError::IoError(e))?;
            let name = entry.file_name().to_string_lossy().to_string();
            
            if let Ok(metadata) = entry.metadata() {
                let attrs = FileAttributes {
                    size: metadata.len(),
                    is_directory: metadata.is_dir(),
                    is_file: metadata.is_file(),
                    is_symlink: metadata.file_type().is_symlink(),
                    created: None,
                    modified: None,
                    accessed: None,
                    permissions: 0o755,
                    owner: None,
                    group: None,
                };
                
                entries.push(DirectoryEntry {
                    name,
                    attributes: attrs,
                });
            }
        }
        
        Ok(entries)
    }
    
    fn read(&mut self, path: &Path, offset: u64, size: u32) -> Result<Vec<u8>, MosesError> {
        use std::fs::File;
        use std::io::{Read, Seek, SeekFrom};
        
        let full_path = if path == Path::new("/") {
            self.base_path.clone()
        } else {
            self.base_path.join(path.strip_prefix("/").unwrap_or(path))
        };
        
        let mut file = File::open(&full_path).map_err(|e| MosesError::IoError(e))?;
        file.seek(SeekFrom::Start(offset)).map_err(|e| MosesError::IoError(e))?;
        
        let mut buffer = vec![0u8; size as usize];
        let bytes_read = file.read(&mut buffer).map_err(|e| MosesError::IoError(e))?;
        buffer.truncate(bytes_read);
        
        Ok(buffer)
    }
    
    fn filesystem_type(&self) -> &str {
        &self.fs_type
    }
}

/// Register all built-in filesystem operations
pub fn register_builtin_ops(registry: &mut FilesystemOpsRegistry) {
    use crate::ext4_native::{Ext4Ops, ExtOpsDetector};
    
    // Register ext4 operations (supports ext2/ext3/ext4)
    registry.register_ops("ext4", |device| {
        let mut ops = Box::new(Ext4Ops::new(device.clone())?);
        ops.init(device)?;
        Ok(ops)
    });
    
    registry.register_ops("ext3", |device| {
        let mut ops = Box::new(Ext4Ops::new(device.clone())?);
        ops.init(device)?;
        Ok(ops)
    });
    
    registry.register_ops("ext2", |device| {
        let mut ops = Box::new(Ext4Ops::new(device.clone())?);
        ops.init(device)?;
        Ok(ops)
    });
    
    // Register ext detector
    registry.register_detector(Box::new(ExtOpsDetector));
    
    // TODO: Add NTFS, FAT32, exFAT once their readers support the necessary operations
}