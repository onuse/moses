// FilesystemOps implementation for ext2/ext3/ext4 filesystems
use crate::ops::{FilesystemOps, FileAttributes, DirectoryEntry, FilesystemInfo};
use super::reader::{ExtReader, FileType};
use super::writer::Ext4Writer;
use super::journaled_writer::{JournaledExt4Writer, Ext4JournalingConfig};
use moses_core::{Device, MosesError};
use std::path::Path;
use std::sync::Mutex;

pub struct Ext4Ops {
    reader: Option<ExtReader>,
    writer: Option<Mutex<Ext4Writer>>,
    journaled_writer: Option<Mutex<JournaledExt4Writer>>,
    device: Device,
    write_enabled: bool,
    journaling_enabled: bool,
}

impl Ext4Ops {
    pub fn new(device: Device) -> Result<Self, MosesError> {
        Ok(Self {
            reader: None,
            writer: None,
            journaled_writer: None,
            device,
            write_enabled: false,
            journaling_enabled: true,  // Enable journaling by default for safety
        })
    }
    
    /// Enable write support (must be called explicitly for safety)
    pub fn enable_write_support(&mut self) -> Result<(), MosesError> {
        if !self.write_enabled {
            if self.journaling_enabled {
                // Use journaled writer
                let config = Ext4JournalingConfig::default();
                let journaled = JournaledExt4Writer::new(self.device.clone(), config)?;
                self.journaled_writer = Some(Mutex::new(journaled));
            } else {
                // Use regular writer
                let writer = Ext4Writer::new(self.device.clone())?;
                self.writer = Some(Mutex::new(writer));
            }
            self.write_enabled = true;
        }
        Ok(())
    }
    
    /// Enable or disable journaling (must be set before enabling write support)
    pub fn set_journaling(&mut self, enabled: bool) {
        if !self.write_enabled {
            self.journaling_enabled = enabled;
        }
    }
}

impl FilesystemOps for Ext4Ops {
    fn init(&mut self, device: &Device) -> Result<(), MosesError> {
        self.device = device.clone();
        self.reader = Some(ExtReader::new(device.clone())?);
        Ok(())
    }
    
    fn statfs(&self) -> Result<FilesystemInfo, MosesError> {
        let reader = self.reader.as_ref()
            .ok_or_else(|| MosesError::Other("Filesystem not initialized".to_string()))?;
        
        // Get complete filesystem information from the reader
        let info = reader.get_info();
        
        // Calculate available space (free blocks minus reserved blocks)
        let available_blocks = info.free_blocks.saturating_sub(info.reserved_blocks);
        let available_space = available_blocks * info.block_size as u64;
        let free_space = info.free_blocks * info.block_size as u64;
        let total_space = info.block_count * info.block_size as u64;
        
        Ok(FilesystemInfo {
            total_space,
            free_space,
            available_space,
            total_inodes: info.total_inodes as u64,
            free_inodes: info.free_inodes as u64,
            block_size: info.block_size,
            fragment_size: info.block_size, // ext4 doesn't use fragments
            max_filename_length: 255,
            filesystem_type: info.filesystem_type,
            volume_label: info.label,
            volume_uuid: info.uuid,
            is_readonly: !self.write_enabled,
        })
    }
    
    fn stat(&mut self, path: &Path) -> Result<FileAttributes, MosesError> {
        let reader = self.reader.as_mut()
            .ok_or_else(|| MosesError::Other("Filesystem not initialized".to_string()))?;
        
        let path_str = path.to_str()
            .ok_or_else(|| MosesError::InvalidInput("Invalid path".to_string()))?;
        
        let metadata = reader.stat(path_str)?;
        
        Ok(FileAttributes {
            size: metadata.size,
            is_directory: metadata.file_type == FileType::Directory,
            is_file: metadata.file_type == FileType::Regular,
            is_symlink: metadata.file_type == FileType::Symlink,
            created: Some(metadata.ctime as u64),
            modified: Some(metadata.mtime as u64),
            accessed: Some(metadata.atime as u64),
            permissions: metadata.mode as u32,
            owner: Some(metadata.uid),
            group: Some(metadata.gid),
        })
    }
    
    fn readdir(&mut self, path: &Path) -> Result<Vec<DirectoryEntry>, MosesError> {
        let reader = self.reader.as_mut()
            .ok_or_else(|| MosesError::Other("Filesystem not initialized".to_string()))?;
        
        let path_str = path.to_str()
            .ok_or_else(|| MosesError::InvalidInput("Invalid path".to_string()))?;
        
        let entries = reader.read_directory(path_str)?;
        
        let mut result = Vec::new();
        for entry in entries {
            // Skip . and .. entries
            if entry.name == "." || entry.name == ".." {
                continue;
            }
            
            // Build full path for stat
            let full_path = if path_str == "/" {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", path_str, entry.name)
            };
            
            // Get attributes for each entry
            let attrs = if let Ok(metadata) = reader.stat(&full_path) {
                FileAttributes {
                    size: metadata.size,
                    is_directory: metadata.file_type == FileType::Directory,
                    is_file: metadata.file_type == FileType::Regular,
                    is_symlink: metadata.file_type == FileType::Symlink,
                    created: Some(metadata.ctime as u64),
                    modified: Some(metadata.mtime as u64),
                    accessed: Some(metadata.atime as u64),
                    permissions: metadata.mode as u32,
                    owner: Some(metadata.uid),
                    group: Some(metadata.gid),
                }
            } else {
                // Fallback if stat fails
                FileAttributes {
                    size: 0,
                    is_directory: entry.entry_type == FileType::Directory,
                    is_file: entry.entry_type == FileType::Regular,
                    is_symlink: entry.entry_type == FileType::Symlink,
                    created: None,
                    modified: None,
                    accessed: None,
                    permissions: 0,
                    owner: None,
                    group: None,
                }
            };
            
            result.push(DirectoryEntry {
                name: entry.name,
                attributes: attrs,
            });
        }
        
        Ok(result)
    }
    
    fn read(&mut self, path: &Path, offset: u64, size: u32) -> Result<Vec<u8>, MosesError> {
        let reader = self.reader.as_mut()
            .ok_or_else(|| MosesError::Other("Filesystem not initialized".to_string()))?;
        
        let path_str = path.to_str()
            .ok_or_else(|| MosesError::InvalidInput("Invalid path".to_string()))?;
        
        // Read entire file (ExtReader doesn't support partial reads yet)
        let data = reader.read_file(path_str)?;
        
        // Apply offset and size
        let start = offset as usize;
        let end = std::cmp::min(start + size as usize, data.len());
        
        if start >= data.len() {
            return Ok(Vec::new());
        }
        
        Ok(data[start..end].to_vec())
    }
    
    // Write operations
    
    fn write(&mut self, path: &Path, offset: u64, data: &[u8]) -> Result<u32, MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("Write support not enabled".to_string()));
        }
        
        let writer = self.writer.as_ref()
            .ok_or_else(|| MosesError::Other("Writer not initialized".to_string()))?;
        
        let mut writer_guard = writer.lock()
            .map_err(|_| MosesError::Other("Failed to lock writer".to_string()))?;
        
        let written = writer_guard.write_file(path, offset, data)?;
        Ok(written as u32)
    }
    
    fn create(&mut self, path: &Path, mode: u32) -> Result<(), MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("Write support not enabled".to_string()));
        }
        
        let writer = self.writer.as_ref()
            .ok_or_else(|| MosesError::Other("Writer not initialized".to_string()))?;
        
        let mut writer_guard = writer.lock()
            .map_err(|_| MosesError::Other("Failed to lock writer".to_string()))?;
        
        // Default uid/gid to 0 (root) - could be made configurable
        writer_guard.create_file(path, mode as u16, 0, 0)?;
        Ok(())
    }
    
    fn mkdir(&mut self, path: &Path, mode: u32) -> Result<(), MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("Write support not enabled".to_string()));
        }
        
        let writer = self.writer.as_ref()
            .ok_or_else(|| MosesError::Other("Writer not initialized".to_string()))?;
        
        let mut writer_guard = writer.lock()
            .map_err(|_| MosesError::Other("Failed to lock writer".to_string()))?;
        
        writer_guard.create_directory(path, mode as u16, 0, 0)?;
        Ok(())
    }
    
    fn unlink(&mut self, path: &Path) -> Result<(), MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("Write support not enabled".to_string()));
        }
        
        let writer = self.writer.as_ref()
            .ok_or_else(|| MosesError::Other("Writer not initialized".to_string()))?;
        
        let mut writer_guard = writer.lock()
            .map_err(|_| MosesError::Other("Failed to lock writer".to_string()))?;
        
        writer_guard.unlink_file(path)?;
        Ok(())
    }
    
    fn rmdir(&mut self, path: &Path) -> Result<(), MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("Write support not enabled".to_string()));
        }
        
        let writer = self.writer.as_ref()
            .ok_or_else(|| MosesError::Other("Writer not initialized".to_string()))?;
        
        let mut writer_guard = writer.lock()
            .map_err(|_| MosesError::Other("Failed to lock writer".to_string()))?;
        
        writer_guard.remove_directory(path)?;
        Ok(())
    }
    
    fn rename(&mut self, from: &Path, to: &Path) -> Result<(), MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("Write support not enabled".to_string()));
        }
        
        let writer = self.writer.as_ref()
            .ok_or_else(|| MosesError::Other("Writer not initialized".to_string()))?;
        
        writer.lock()
            .map_err(|_| MosesError::Other("Failed to lock writer".to_string()))?
            .rename(from, to)
    }
    
    fn truncate(&mut self, path: &Path, size: u64) -> Result<(), MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("Write support not enabled".to_string()));
        }
        
        let writer = self.writer.as_ref()
            .ok_or_else(|| MosesError::Other("Writer not initialized".to_string()))?;
        
        writer.lock()
            .map_err(|_| MosesError::Other("Failed to lock writer".to_string()))?
            .truncate(path, size)
    }
    
    fn sync(&mut self) -> Result<(), MosesError> {
        if self.write_enabled {
            if let Some(ref writer) = self.writer {
                let mut writer = writer.lock()
                    .map_err(|_| MosesError::Other("Failed to lock writer".to_string()))?;
                
                // Flush all pending writes to disk
                writer.flush_all_writes()?;
                
                // Checkpoint the journal to ensure all transactions are persisted
                writer.checkpoint_journal()?;
                
                // Update superblock write time
                writer.update_superblock_write_time()?;
            }
        }
        Ok(())
    }
    
    fn is_readonly(&self) -> bool {
        !self.write_enabled
    }
    
    fn filesystem_type(&self) -> &str {
        if let Some(ref reader) = self.reader {
            match reader.version {
                super::core::ext_config::ExtVersion::Ext2 => "ext2",
                super::core::ext_config::ExtVersion::Ext3 => "ext3",
                super::core::ext_config::ExtVersion::Ext4 => "ext4",
            }
        } else {
            "ext4"
        }
    }
}

/// Ext filesystem detector
pub struct ExtDetector;

impl crate::ops::FilesystemDetector for ExtDetector {
    fn detect(&self, device: &Device) -> Result<Option<String>, MosesError> {
        // Try to read the superblock magic
        use crate::utils::{open_device_read, read_block};
        
        let mut file = open_device_read(device)?;
        
        // Read magic at offset 1024 + 56 (0x438)
        let buffer = read_block(&mut file, 1024 + 56, 2)?;
        let magic = u16::from_le_bytes([buffer[0], buffer[1]]);
        
        if magic == 0xEF53 {
            // It's an ext filesystem, try to determine version
            // For now, just return ext4 as it's backwards compatible
            Ok(Some("ext4".to_string()))
        } else {
            Ok(None)
        }
    }
    
    fn priority(&self) -> i32 {
        10 // Higher priority for common Linux filesystem
    }
}