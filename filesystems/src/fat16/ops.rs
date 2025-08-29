// FAT16 FilesystemOps implementation for mounting
use crate::ops::{FilesystemOps, FileAttributes, DirectoryEntry, FilesystemInfo as OpsFilesystemInfo};
use crate::device_reader::FilesystemReader;
use crate::ops_helpers::convert_filesystem_info;
use super::reader::Fat16Reader;
use super::writer::Fat16Writer;
use super::file_ops::Fat16FileOps;
use moses_core::{Device, MosesError};
use std::path::Path;
use std::sync::Mutex;

/// FAT16 filesystem operations wrapper
pub struct Fat16Ops {
    reader: Mutex<Option<Fat16Reader>>,
    writer: Mutex<Option<Fat16Writer>>,
    file_ops: Mutex<Option<Fat16FileOps>>,
    device: Option<Device>,
}

impl Fat16Ops {
    pub fn new() -> Self {
        Fat16Ops {
            reader: Mutex::new(None),
            writer: Mutex::new(None),
            file_ops: Mutex::new(None),
            device: None,
        }
    }
}

impl FilesystemOps for Fat16Ops {
    fn filesystem_type(&self) -> &str {
        "fat16"
    }
    
    fn init(&mut self, device: &Device) -> Result<(), MosesError> {
        // Initialize reader and writer
        let reader = Fat16Reader::new(device.clone())?;
        let writer = Fat16Writer::new(device.clone())?;
        
        // Store them temporarily
        *self.reader.lock().unwrap() = Some(reader);
        *self.writer.lock().unwrap() = Some(writer);
        
        // Now create file_ops with both reader and writer
        // This requires taking them out and putting them back
        // For now, we'll keep them separate and create file_ops on demand
        
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
        
        // Extract parent directory and filename
        let (parent_path, file_name) = if let Some(pos) = path_str.rfind('/') {
            if pos == 0 {
                ("/", &path_str[1..])
            } else {
                (&path_str[..pos], &path_str[pos + 1..])
            }
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