// NTFS Read-Write FilesystemOps implementation
// This version includes write support using NtfsWriter

use crate::ops::{FilesystemOps, FileAttributes, DirectoryEntry, FilesystemInfo as OpsFilesystemInfo};
use crate::device_reader::FilesystemReader;
use crate::ops_helpers::convert_filesystem_info;
use super::reader::NtfsReader;
use super::writer::{NtfsWriter, NtfsWriteConfig};
use moses_core::{Device, MosesError};
use std::path::Path;
use std::sync::Mutex;
use log::{info, warn, debug};

/// NTFS filesystem operations with read-write support
pub struct NtfsRwOps {
    reader: Mutex<Option<NtfsReader>>,
    writer: Mutex<Option<NtfsWriter>>,
    device: Option<Device>,
    write_enabled: bool,
}

impl NtfsRwOps {
    pub fn new() -> Self {
        NtfsRwOps {
            reader: Mutex::new(None),
            writer: Mutex::new(None),
            device: None,
            write_enabled: false,
        }
    }
    
    /// Enable write support (disabled by default for safety)
    pub fn enable_writes(&mut self, enable: bool) {
        self.write_enabled = enable;
        info!("NTFS write support: {}", if enable { "ENABLED" } else { "DISABLED" });
    }
}

impl FilesystemOps for NtfsRwOps {
    fn filesystem_type(&self) -> &str {
        "ntfs"
    }
    
    fn init(&mut self, device: &Device) -> Result<(), MosesError> {
        // Initialize reader
        let reader = NtfsReader::new(device.clone())?;
        *self.reader.lock().unwrap() = Some(reader);
        
        // Initialize writer if writes are enabled
        if self.write_enabled {
            let mut config = NtfsWriteConfig::default();
            config.enable_writes = true;  // Actually enable writes
            config.verify_writes = true;  // Always verify for safety
            
            info!("Initializing NTFS writer with write verification");
            let writer = NtfsWriter::new(device.clone(), config)?;
            *self.writer.lock().unwrap() = Some(writer);
        }
        
        self.device = Some(device.clone());
        Ok(())
    }
    
    fn statfs(&self) -> Result<OpsFilesystemInfo, MosesError> {
        let reader = self.reader.lock().unwrap();
        let reader = reader.as_ref()
            .ok_or_else(|| MosesError::Other("Filesystem not initialized".to_string()))?;
        Ok(convert_filesystem_info(reader.get_info()))
    }
    
    fn stat(&mut self, path: &Path) -> Result<FileAttributes, MosesError> {
        let path_str = path.to_str()
            .ok_or_else(|| MosesError::Other("Invalid path".to_string()))?;
        
        // Handle root directory specially
        if path_str == "/" || path_str.is_empty() {
            return Ok(FileAttributes {
                size: 0,
                is_directory: true,
                is_file: false,
                is_symlink: false,
                created: None,
                modified: None,
                accessed: None,
                permissions: 0o755,
                owner: None,
                group: None,
            });
        }
        
        // For NTFS, we need to handle paths differently since subdirectory 
        // navigation isn't fully implemented yet
        let (parent_path, file_name) = if path_str.starts_with('/') {
            // For now, assume everything is in root
            ("/", path_str.trim_start_matches('/'))
        } else {
            ("/", path_str)
        };
        
        // List parent directory and find the entry
        let mut reader = self.reader.lock().unwrap();
        let reader = reader.as_mut()
            .ok_or_else(|| MosesError::Other("Filesystem not initialized".to_string()))?;
        
        let entries = reader.list_directory(parent_path)?;
        
        let entry = entries.iter()
            .find(|e| e.name == file_name)
            .ok_or_else(|| MosesError::Other(format!("Path not found: {}", path_str)))?;
        
        Ok(FileAttributes {
            size: entry.size,
            is_directory: entry.is_directory,
            is_file: !entry.is_directory,
            is_symlink: false,
            created: entry.metadata.created,
            modified: entry.metadata.modified,
            accessed: entry.metadata.accessed,
            permissions: if entry.is_directory { 0o755 } else { 0o644 },
            owner: None,
            group: None,
        })
    }
    
    fn readdir(&mut self, path: &Path) -> Result<Vec<DirectoryEntry>, MosesError> {
        let path_str = path.to_str()
            .ok_or_else(|| MosesError::Other("Invalid path".to_string()))?;
        
        let mut reader = self.reader.lock().unwrap();
        let reader = reader.as_mut()
            .ok_or_else(|| MosesError::Other("Filesystem not initialized".to_string()))?;
        
        let entries = reader.list_directory(path_str)?;
        
        Ok(entries.into_iter().map(|e| DirectoryEntry {
            name: e.name.clone(),
            attributes: FileAttributes {
                size: e.size,
                is_directory: e.is_directory,
                is_file: !e.is_directory,
                is_symlink: false,
                created: e.metadata.created,
                modified: e.metadata.modified,
                accessed: e.metadata.accessed,
                permissions: if e.is_directory { 0o755 } else { 0o644 },
                owner: None,
                group: None,
            },
        }).collect())
    }
    
    fn read(&mut self, path: &Path, offset: u64, size: u32) -> Result<Vec<u8>, MosesError> {
        let path_str = path.to_str()
            .ok_or_else(|| MosesError::Other("Invalid path".to_string()))?;
        
        let mut reader = self.reader.lock().unwrap();
        let reader = reader.as_mut()
            .ok_or_else(|| MosesError::Other("Filesystem not initialized".to_string()))?;
        
        // Read the entire file (FilesystemReader doesn't support partial reads)
        let data = reader.read_file(path_str)?;
        
        // Apply offset and size
        let start = offset as usize;
        if start >= data.len() {
            return Ok(Vec::new());
        }
        
        let end = std::cmp::min(start + size as usize, data.len());
        Ok(data[start..end].to_vec())
    }
    
    // Write operations
    fn write(&mut self, _path: &Path, _offset: u64, _data: &[u8]) -> Result<u32, MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("NTFS write support not enabled".to_string()));
        }
        
        let mut writer = self.writer.lock().unwrap();
        let writer = writer.as_mut()
            .ok_or_else(|| MosesError::Other("Writer not initialized".to_string()))?;
        
        // Start a transaction for safety
        writer.begin_transaction()?;
        
        // For now, we'll return not implemented since NtfsWriter doesn't have
        // high-level file write methods yet. This is where we'd implement:
        // 1. Find the MFT record for the file
        // 2. Update the DATA attribute
        // 3. Allocate new clusters if needed
        // 4. Write the actual data
        // 5. Update file size in MFT
        
        warn!("NTFS file write not yet implemented at high level");
        
        // Rollback since we didn't actually do anything
        writer.rollback_transaction()?;
        
        Err(MosesError::NotSupported("NTFS file write not yet implemented".to_string()))
    }
    
    fn create(&mut self, path: &Path, mode: u32) -> Result<(), MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("NTFS write support not enabled".to_string()));
        }
        
        let mut writer = self.writer.lock().unwrap();
        let writer = writer.as_mut()
            .ok_or_else(|| MosesError::Other("Writer not initialized".to_string()))?;
        
        debug!("Creating file: {:?} with mode {:o}", path, mode);
        
        // Start transaction
        writer.begin_transaction()?;
        
        // Implementation would:
        // 1. Allocate a new MFT record
        // 2. Set up standard attributes (STANDARD_INFORMATION, FILE_NAME)
        // 3. Create empty DATA attribute
        // 4. Add to parent directory index
        
        // For now, return not implemented
        writer.rollback_transaction()?;
        
        Err(MosesError::NotSupported("NTFS file creation not yet implemented".to_string()))
    }
    
    fn mkdir(&mut self, path: &Path, mode: u32) -> Result<(), MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("NTFS write support not enabled".to_string()));
        }
        
        debug!("Creating directory: {:?} with mode {:o}", path, mode);
        
        // Similar to create() but with directory flag
        Err(MosesError::NotSupported("NTFS directory creation not yet implemented".to_string()))
    }
    
    fn unlink(&mut self, path: &Path) -> Result<(), MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("NTFS write support not enabled".to_string()));
        }
        
        debug!("Deleting file: {:?}", path);
        
        // Would free MFT record and remove from parent directory
        Err(MosesError::NotSupported("NTFS file deletion not yet implemented".to_string()))
    }
    
    fn sync(&mut self) -> Result<(), MosesError> {
        if let Some(writer) = self.writer.lock().unwrap().as_mut() {
            // If we had pending changes, we'd flush them here
            if writer.is_dry_run() {
                debug!("Sync called in dry-run mode - no actual flush");
            } else {
                debug!("Syncing NTFS writes");
                // Would flush any cached writes
            }
        }
        Ok(())
    }
    
    fn is_readonly(&self) -> bool {
        !self.write_enabled
    }
}