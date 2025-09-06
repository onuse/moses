// NTFS Read-Write FilesystemOps implementation
// This version includes write support using NtfsWriter

use crate::ops::{FilesystemOps, FileAttributes, DirectoryEntry, FilesystemInfo as OpsFilesystemInfo};
use crate::device_reader::FilesystemReader;
use crate::ops_helpers::convert_filesystem_info;
use super::reader::NtfsReader;
use super::writer::{NtfsWriter, NtfsWriteConfig};
use super::path_resolver::PathResolver;
use moses_core::{Device, MosesError};
use std::path::Path;
use std::sync::Mutex;
use log::{info, debug};

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
    fn write(&mut self, path: &Path, offset: u64, data: &[u8]) -> Result<u32, MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("NTFS write support not enabled".to_string()));
        }
        
        let path_str = path.to_str()
            .ok_or_else(|| MosesError::Other("Invalid path".to_string()))?;
        
        // Get the MFT record number for the file
        let mut reader = self.reader.lock().unwrap();
        let reader = reader.as_mut()
            .ok_or_else(|| MosesError::Other("Reader not initialized".to_string()))?;
        
        let mut path_resolver = PathResolver::new();
        let mft_record_num = path_resolver.resolve_path(reader, path_str)?;
        
        // Use the writer to write data
        let mut writer = self.writer.lock().unwrap();
        let writer = writer.as_mut()
            .ok_or_else(|| MosesError::Other("Writer not initialized".to_string()))?;
        
        let bytes_written = writer.write_file_data(mft_record_num, offset, data)?;
        
        Ok(bytes_written as u32)
    }
    
    fn create(&mut self, path: &Path, mode: u32) -> Result<(), MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("NTFS write support not enabled".to_string()));
        }
        
        let path_str = path.to_str()
            .ok_or_else(|| MosesError::Other("Invalid path".to_string()))?;
        
        
        // For now, we'll create files in the root directory
        // Full implementation would parse the parent path and add to appropriate directory
        
        let mut writer = self.writer.lock().unwrap();
        let writer = writer.as_mut()
            .ok_or_else(|| MosesError::Other("Writer not initialized".to_string()))?;
        
        debug!("Creating file: {:?} with mode {:o}", path, mode);
        
        // Create the file
        let _mft_record_num = writer.create_file(path_str, 0)?;
        
        Ok(())
    }
    
    fn mkdir(&mut self, path: &Path, mode: u32) -> Result<(), MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("NTFS write support not enabled".to_string()));
        }
        
        let path_str = path.to_str()
            .ok_or_else(|| MosesError::Other("Invalid path".to_string()))?;

        
        debug!("Creating directory: {:?} with mode {:o}", path, mode);
        
        let mut writer = self.writer.lock().unwrap();
        let writer = writer.as_mut()
            .ok_or_else(|| MosesError::Other("Writer not initialized".to_string()))?;
        
        // Create the directory
        let _mft_record_num = writer.create_directory(path_str)?;
        
        Ok(())
    }
    
    fn unlink(&mut self, path: &Path) -> Result<(), MosesError> {
        if !self.write_enabled {
            return Err(MosesError::NotSupported("NTFS write support not enabled".to_string()));
        }
        
        let path_str = path.to_str()
            .ok_or_else(|| MosesError::Other("Invalid path".to_string()))?;
        
        debug!("Deleting file: {:?}", path);
        
        // Get the MFT record number for the file
        let mut reader = self.reader.lock().unwrap();
        let reader = reader.as_mut()
            .ok_or_else(|| MosesError::Other("Reader not initialized".to_string()))?;
        
        let mut path_resolver = PathResolver::new();
        let mft_record_num = path_resolver.resolve_path(reader, path_str)?;
        
        // Delete the file
        let mut writer = self.writer.lock().unwrap();
        let writer = writer.as_mut()
            .ok_or_else(|| MosesError::Other("Writer not initialized".to_string()))?;
        
        writer.delete_file(mft_record_num)?;
        
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