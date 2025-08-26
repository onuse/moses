// WinFsp filesystem implementation for Windows
// This bridges Moses FilesystemOps to WinFsp API

use super::{MountOptions, MountProvider};
use crate::ops::{FilesystemOps, FileAttributes};
use moses_core::{Device, MosesError};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStrExt;
use winfsp::filesystem::{
    FileSystem, FileSystemContext, FileInfo, DirInfo,
    OpenFileInfo, VolumeInfo, FileAttributes as WinFspAttributes,
    CreateOptions, PFileInfo,
};
use winfsp::error::FspError;
use std::time::SystemTime;

/// Moses filesystem implementation for WinFsp
struct MosesFileSystem {
    ops: Arc<Mutex<Box<dyn FilesystemOps>>>,
    device: Device,
    readonly: bool,
}

impl MosesFileSystem {
    fn new(ops: Box<dyn FilesystemOps>, device: Device, readonly: bool) -> Self {
        Self {
            ops: Arc::new(Mutex::new(ops)),
            device,
            readonly,
        }
    }
    
    /// Convert Moses FileAttributes to WinFsp FileInfo
    fn convert_attributes(&self, path: &Path, attrs: &FileAttributes) -> FileInfo {
        let mut info = FileInfo::default();
        
        // Set basic attributes
        info.file_attributes = if attrs.is_directory {
            WinFspAttributes::DIRECTORY
        } else {
            WinFspAttributes::NORMAL
        };
        
        if self.readonly {
            info.file_attributes |= WinFspAttributes::READONLY;
        }
        
        // Set size
        info.file_size = attrs.size;
        info.allocation_size = (attrs.size + 4095) & !4095; // Round up to 4K
        
        // Set timestamps
        if let Some(created) = attrs.created {
            info.creation_time = created as i64;
        }
        if let Some(modified) = attrs.modified {
            info.last_write_time = modified as i64;
            info.change_time = modified as i64;
        }
        if let Some(accessed) = attrs.accessed {
            info.last_access_time = accessed as i64;
        }
        
        // Set index (use a hash of the path as a pseudo-inode)
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        info.index_number = hasher.finish();
        
        info
    }
}

impl FileSystemContext for MosesFileSystem {
    type FileContext = PathBuf;
    
    fn get_volume_info(&self) -> Result<VolumeInfo, FspError> {
        let mut ops = self.ops.lock().unwrap();
        
        match ops.statfs() {
            Ok(info) => {
                let mut volume = VolumeInfo::default();
                volume.total_size = info.total_space;
                volume.free_size = info.free_space;
                
                // Set volume label
                if let Some(label) = info.volume_label {
                    volume.set_volume_label(&label);
                } else {
                    volume.set_volume_label(&format!("Moses {}", info.filesystem_type));
                }
                
                // Set filesystem name
                volume.set_filesystem_name(&format!("Moses-{}", info.filesystem_type));
                
                // Set capabilities
                volume.case_sensitive_search = info.filesystem_type == "ext4" 
                    || info.filesystem_type == "ext3" 
                    || info.filesystem_type == "ext2";
                volume.case_preserved_names = true;
                volume.unicode_on_disk = true;
                volume.persistent_acls = false;
                volume.supports_reparse_points = false;
                volume.supports_sparse_files = false;
                volume.read_only_volume = self.readonly;
                
                Ok(volume)
            }
            Err(e) => {
                log::error!("Failed to get volume info: {}", e);
                Err(FspError::from_win32_error(0x1F)) // ERROR_GEN_FAILURE
            }
        }
    }
    
    fn open(
        &self,
        path: &Path,
        create_options: CreateOptions,
        granted_access: u32,
        _file_info: &mut PFileInfo,
        _normalized_name: &mut OsString,
    ) -> Result<Self::FileContext, FspError> {
        // For read-only filesystem, reject any write attempts
        if self.readonly && (create_options != CreateOptions::FILE_OPEN) {
            return Err(FspError::from_win32_error(0x13)); // ERROR_WRITE_PROTECT
        }
        
        let mut ops = self.ops.lock().unwrap();
        
        // Check if file exists
        match ops.stat(path) {
            Ok(_attrs) => {
                // File exists
                if create_options == CreateOptions::FILE_CREATE {
                    return Err(FspError::from_win32_error(0x50)); // ERROR_FILE_EXISTS
                }
                Ok(path.to_path_buf())
            }
            Err(_) => {
                // File doesn't exist
                if create_options == CreateOptions::FILE_OPEN 
                    || create_options == CreateOptions::FILE_OPEN_IF {
                    return Err(FspError::from_win32_error(0x2)); // ERROR_FILE_NOT_FOUND
                }
                // Would create file here, but we're read-only
                Err(FspError::from_win32_error(0x13)) // ERROR_WRITE_PROTECT
            }
        }
    }
    
    fn get_file_info(
        &self,
        context: &Self::FileContext,
    ) -> Result<FileInfo, FspError> {
        let mut ops = self.ops.lock().unwrap();
        
        match ops.stat(context) {
            Ok(attrs) => Ok(self.convert_attributes(context, &attrs)),
            Err(e) => {
                log::error!("Failed to stat {}: {}", context.display(), e);
                Err(FspError::from_win32_error(0x2)) // ERROR_FILE_NOT_FOUND
            }
        }
    }
    
    fn read_directory(
        &self,
        context: &Self::FileContext,
        _pattern: Option<&OsStr>,
        _marker: Option<&OsStr>,
    ) -> Result<Vec<DirInfo>, FspError> {
        let mut ops = self.ops.lock().unwrap();
        
        match ops.readdir(context) {
            Ok(entries) => {
                let mut results = Vec::new();
                
                for entry in entries {
                    let mut dir_info = DirInfo::default();
                    
                    // Set file name
                    dir_info.set_file_name(&entry.name);
                    
                    // Convert attributes
                    let file_info = self.convert_attributes(
                        &context.join(&entry.name),
                        &entry.attributes
                    );
                    dir_info.file_info = file_info;
                    
                    results.push(dir_info);
                }
                
                Ok(results)
            }
            Err(e) => {
                log::error!("Failed to read directory {}: {}", context.display(), e);
                Err(FspError::from_win32_error(0x3)) // ERROR_PATH_NOT_FOUND
            }
        }
    }
    
    fn read(
        &self,
        context: &Self::FileContext,
        buffer: &mut [u8],
        offset: u64,
    ) -> Result<u32, FspError> {
        let mut ops = self.ops.lock().unwrap();
        
        match ops.read(context, offset, buffer.len() as u32) {
            Ok(data) => {
                let bytes_read = std::cmp::min(data.len(), buffer.len());
                buffer[..bytes_read].copy_from_slice(&data[..bytes_read]);
                Ok(bytes_read as u32)
            }
            Err(e) => {
                log::error!("Failed to read {}: {}", context.display(), e);
                Err(FspError::from_win32_error(0x1E)) // ERROR_READ_FAULT
            }
        }
    }
    
    // Write operations - all return error for read-only filesystem
    fn write(
        &self,
        _context: &Self::FileContext,
        _buffer: &[u8],
        _offset: u64,
        _write_to_eof: bool,
        _constrained_io: bool,
        _file_info: &mut PFileInfo,
    ) -> Result<u32, FspError> {
        Err(FspError::from_win32_error(0x13)) // ERROR_WRITE_PROTECT
    }
    
    fn cleanup(
        &self,
        _context: &Self::FileContext,
        _flags: u32,
    ) {
        // Nothing to clean up for read-only operations
    }
    
    fn close(&self, _context: Self::FileContext) {
        // Nothing to close for read-only operations
    }
}

/// WinFsp mount provider
pub struct WinFspMount {
    filesystems: Vec<(PathBuf, FileSystem<MosesFileSystem>)>,
}

impl WinFspMount {
    pub fn new() -> Result<Self, MosesError> {
        Ok(Self {
            filesystems: Vec::new(),
        })
    }
}

impl MountProvider for WinFspMount {
    fn mount(
        &mut self,
        device: &Device,
        mut ops: Box<dyn FilesystemOps>,
        options: &MountOptions,
    ) -> Result<(), MosesError> {
        // Initialize the filesystem ops
        ops.init(device)?;
        
        // Create Moses filesystem
        let moses_fs = MosesFileSystem::new(ops, device.clone(), options.readonly);
        
        // Create WinFsp filesystem
        let mut fs_params = winfsp::filesystem::FileSystemParams::default();
        
        // Set filesystem name
        fs_params.set_filesystem_name(&format!(
            "Moses-{}", 
            moses_fs.ops.lock().unwrap().filesystem_type()
        ));
        
        // Set volume parameters
        fs_params.volume_creation_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        fs_params.volume_serial_number = 0x12345678; // TODO: Generate from device
        fs_params.file_info_timeout = 1000; // 1 second cache
        
        // Create the filesystem
        let filesystem = FileSystem::new(moses_fs, &options.mount_point, Some(fs_params))
            .map_err(|e| MosesError::Other(format!("Failed to create WinFsp filesystem: {:?}", e)))?;
        
        // Start the filesystem
        filesystem.start()
            .map_err(|e| MosesError::Other(format!("Failed to start filesystem: {:?}", e)))?;
        
        // Store the filesystem
        let mount_path = PathBuf::from(&options.mount_point);
        self.filesystems.push((mount_path, filesystem));
        
        log::info!("Successfully mounted {} at {}", device.name, options.mount_point);
        Ok(())
    }
    
    fn unmount(&mut self, mount_point: &Path) -> Result<(), MosesError> {
        // Find and remove the filesystem
        if let Some(index) = self.filesystems.iter().position(|(path, _)| path == mount_point) {
            let (_path, filesystem) = self.filesystems.remove(index);
            
            // Stop the filesystem
            filesystem.stop();
            
            log::info!("Successfully unmounted {}", mount_point.display());
            Ok(())
        } else {
            Err(MosesError::Other(format!("No filesystem mounted at {}", mount_point.display())))
        }
    }
    
    fn is_mounted(&self, mount_point: &Path) -> bool {
        self.filesystems.iter().any(|(path, _)| path == mount_point)
    }
}