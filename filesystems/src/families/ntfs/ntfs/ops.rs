// NTFS FilesystemOps implementation for mounting
use crate::ops::{FilesystemOps, FileAttributes, DirectoryEntry, FilesystemInfo as OpsFilesystemInfo};
use crate::device_reader::FilesystemReader;
use crate::ops_helpers::convert_filesystem_info;
use super::reader::NtfsReader;
use moses_core::{Device, MosesError};
use std::path::Path;
use std::sync::Mutex;

/// NTFS filesystem operations wrapper
pub struct NtfsOps {
    reader: Mutex<Option<NtfsReader>>,
    device: Option<Device>,
}

impl NtfsOps {
    pub fn new() -> Self {
        NtfsOps {
            reader: Mutex::new(None),
            device: None,
        }
    }
}

impl FilesystemOps for NtfsOps {
    fn filesystem_type(&self) -> &str {
        "ntfs"
    }
    
    fn init(&mut self, device: &Device) -> Result<(), MosesError> {
        let reader = NtfsReader::new(device.clone())?;
        *self.reader.lock().unwrap() = Some(reader);
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
}