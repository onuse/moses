// NTFS Read-Write FilesystemOps implementation
// This version includes write support using NtfsWriter with high-level operations

use crate::ops::{FilesystemOps, FileAttributes, DirectoryEntry, FilesystemInfo as OpsFilesystemInfo};
use crate::device_reader::FilesystemReader;
use crate::ops_helpers::convert_filesystem_info;
use super::reader::NtfsReader;
use super::writer::{NtfsWriter, NtfsWriteConfig};
use super::path_resolver::PathResolver;
use super::structures::MFT_RECORD_ROOT;
use moses_core::{Device, MosesError};
use std::path::Path;
use std::sync::Mutex;
use std::collections::HashMap;
use log::{info, debug};

/// NTFS filesystem operations with read-write support
pub struct NtfsRwOps {
    reader: Mutex<Option<NtfsReader>>,
    writer: Mutex<Option<NtfsWriter>>,
    device: Option<Device>,
    write_enabled: bool,
    // Cache mapping file paths to MFT record numbers
    path_to_mft: Mutex<HashMap<String, u64>>,
    // Path resolver for subdirectory navigation
    #[allow(dead_code)]
    path_resolver: Mutex<PathResolver>,
}

impl NtfsRwOps {
    pub fn new() -> Self {
        NtfsRwOps {
            reader: Mutex::new(None),
            writer: Mutex::new(None),
            device: None,
            write_enabled: false,
            path_to_mft: Mutex::new(HashMap::new()),
            path_resolver: Mutex::new(PathResolver::new()),
        }
    }
    
    /// Enable write support (disabled by default for safety)
    pub fn enable_writes(&mut self, enable: bool) {
        self.write_enabled = enable;
        info!("NTFS write support: {}", if enable { "ENABLED" } else { "DISABLED" });
    }
    
    /// Find MFT record number for a file path
    fn find_mft_record(&mut self, path: &str) -> Result<u64, MosesError> {
        // Check cache first
        if let Some(&mft_num) = self.path_to_mft.lock().unwrap().get(path) {
            return Ok(mft_num);
        }
        
        // For now, only support files in root directory
        if !path.starts_with('/') || path.contains('/') && path != "/" {
            return Err(MosesError::NotSupported("Subdirectory navigation not yet implemented".to_string()));
        }
        
        let file_name = path.trim_start_matches('/');
        if file_name.is_empty() {
            return Ok(MFT_RECORD_ROOT);  // Root directory
        }
        
        // List root directory and find the file
        let mut reader = self.reader.lock().unwrap();
        let reader = reader.as_mut()
            .ok_or_else(|| MosesError::Other("Reader not initialized".to_string()))?;
        
        let entries = reader.list_directory("/")?;
        
        for entry in entries {
            if entry.name == file_name {
                // The cluster field contains the MFT record number
                if let Some(mft_num) = entry.cluster {
                    let mft_record = mft_num as u64;
                    self.path_to_mft.lock().unwrap().insert(path.to_string(), mft_record);
                    return Ok(mft_record);
                }
            }
        }
        
        Err(MosesError::Other(format!("File not found: {}", path)))
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
    fn write(&mut self, path: &Path, offset: u64, data: &[u8]) -> Result<u32, MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("NTFS write support not enabled".to_string()));
        }
        
        let path_str = path.to_str()
            .ok_or_else(|| MosesError::Other("Invalid path".to_string()))?;
        
        debug!("Writing {} bytes to {} at offset {}", data.len(), path_str, offset);
        
        // Find the MFT record for this file
        let mft_record = self.find_mft_record(path_str)?;
        
        let mut writer = self.writer.lock().unwrap();
        let writer = writer.as_mut()
            .ok_or_else(|| MosesError::Other("Writer not initialized".to_string()))?;
        
        // Use the new high-level write operation
        let bytes_written = writer.write_file_data(mft_record, offset, data)?;
        
        Ok(bytes_written as u32)
    }
    
    fn create(&mut self, path: &Path, _mode: u32) -> Result<(), MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("NTFS write support not enabled".to_string()));
        }
        
        let path_str = path.to_str()
            .ok_or_else(|| MosesError::Other("Invalid path".to_string()))?;
        
        // Extract filename
        let file_name = if path_str.starts_with('/') {
            path_str.trim_start_matches('/')
        } else {
            path_str
        };
        
        if file_name.is_empty() {
            return Err(MosesError::Other("Cannot create file with empty name".to_string()));
        }
        
        debug!("Creating file: {}", file_name);
        
        let mut writer = self.writer.lock().unwrap();
        let writer = writer.as_mut()
            .ok_or_else(|| MosesError::Other("Writer not initialized".to_string()))?;
        
        // Create the file with initial size 0
        let mft_record = writer.create_file(file_name, 0)?;
        
        // Cache the path to MFT mapping
        self.path_to_mft.lock().unwrap().insert(path_str.to_string(), mft_record);
        
        info!("Created file '{}' with MFT record {}", file_name, mft_record);
        
        Ok(())
    }
    
    fn mkdir(&mut self, path: &Path, _mode: u32) -> Result<(), MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("NTFS write support not enabled".to_string()));
        }
        
        let path_str = path.to_str()
            .ok_or_else(|| MosesError::Other("Invalid path".to_string()))?;
        
        debug!("Creating directory: {}", path_str);
        
        // Directory creation is similar to file creation but with directory flag
        // For now, not implemented
        Err(MosesError::NotSupported("NTFS directory creation not yet implemented".to_string()))
    }
    
    fn unlink(&mut self, path: &Path) -> Result<(), MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("NTFS write support not enabled".to_string()));
        }
        
        let path_str = path.to_str()
            .ok_or_else(|| MosesError::Other("Invalid path".to_string()))?;
        
        debug!("Deleting file: {}", path_str);
        
        // Find the MFT record for this file
        let mft_record = self.find_mft_record(path_str)?;
        
        let mut writer = self.writer.lock().unwrap();
        let writer = writer.as_mut()
            .ok_or_else(|| MosesError::Other("Writer not initialized".to_string()))?;
        
        // Delete the file
        writer.delete_file(mft_record)?;
        
        // Remove from cache
        self.path_to_mft.lock().unwrap().remove(path_str);
        
        info!("Deleted file '{}'", path_str);
        
        Ok(())
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