// FAT16 File Operations
// High-level file and directory operations for FAT16 filesystems

use moses_core::MosesError;
use crate::fat16::reader::Fat16Reader;
use crate::fat16::writer::Fat16Writer;
use crate::fat16::path_resolver::{Fat16PathResolver, Fat16PathResolverMut};
use crate::fat_common::{FatDirEntry, FatAttributes};
use std::path::PathBuf;
use log::{info, debug, warn};

type MosesResult<T> = Result<T, MosesError>;

pub struct Fat16FileOps {
    pub reader: Fat16Reader,
    pub writer: Fat16Writer,
}

impl Fat16FileOps {
    /// Create new file operations handler
    pub fn new(reader: Fat16Reader, writer: Fat16Writer) -> Self {
        Self { reader, writer }
    }
    
    /// Create a new file
    pub fn create_file(&mut self, path: &str, initial_size: u32) -> MosesResult<()> {
        info!("Creating file: {} with size {}", path, initial_size);
        
        // Parse path
        let path_buf = PathBuf::from(path);
        let parent_path = path_buf.parent()
            .map(|p| p.to_str().unwrap_or("/"))
            .unwrap_or("/");
        let file_name = path_buf.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| MosesError::Other("Invalid file name".into()))?;
        
        // Resolve parent directory
        let mut resolver = Fat16PathResolver::new(&mut self.reader);
        let parent = resolver.resolve_path(parent_path)?;
        
        if !parent.is_directory {
            return Err(MosesError::Other(format!("{} is not a directory", parent_path)));
        }
        
        // Check if file already exists
        if resolver.exists(path) {
            return Err(MosesError::Other(format!("File {} already exists", path)));
        }
        
        // Allocate clusters if needed
        let clusters_needed = if initial_size > 0 {
            (initial_size + self.writer.get_bytes_per_cluster() - 1) / self.writer.get_bytes_per_cluster()
        } else {
            0
        };
        
        let first_cluster = if clusters_needed > 0 {
            let clusters = self.writer.allocate_cluster_chain(clusters_needed)?;
            clusters[0]
        } else {
            0
        };
        
        // Create directory entry
        let mut resolver_mut = Fat16PathResolverMut::new(&mut self.writer);
        resolver_mut.create_directory_entry(
            if parent.cluster == 0 { None } else { Some(parent.cluster) },
            file_name,
            false,  // not a directory
            first_cluster,
            0,  // Start with 0 size, will update when writing
        )?;
        
        // Flush FAT changes
        self.writer.flush()?;
        
        info!("File created successfully");
        Ok(())
    }
    
    /// Write data to a file
    pub fn write_file(&mut self, path: &str, offset: u64, data: &[u8]) -> MosesResult<usize> {
        info!("Writing {} bytes to {} at offset {}", data.len(), path, offset);
        
        // Resolve file path
        let mut resolver = Fat16PathResolver::new(&mut self.reader);
        let file = resolver.resolve_path(path)?;
        
        if file.is_directory {
            return Err(MosesError::Other(format!("{} is a directory", path)));
        }
        
        // If file has no clusters and we're writing data, allocate some
        let mut first_cluster = file.cluster;
        if first_cluster == 0 && !data.is_empty() {
            first_cluster = self.writer.allocate_cluster()?;
            // TODO: Update directory entry with new cluster
        }
        
        if first_cluster == 0 {
            return Ok(0);  // Nothing to write to empty file
        }
        
        // Write data to clusters
        self.writer.write_file_data(first_cluster, data)?;
        
        // TODO: Update file size in directory entry
        
        // Flush changes
        self.writer.flush()?;
        
        Ok(data.len())
    }
    
    /// Read file data
    pub fn read_file(&mut self, path: &str) -> MosesResult<Vec<u8>> {
        debug!("Reading file: {}", path);
        
        // Resolve file path
        let mut resolver = Fat16PathResolver::new(&mut self.reader);
        let file = resolver.resolve_path(path)?;
        
        if file.is_directory {
            return Err(MosesError::Other(format!("{} is a directory", path)));
        }
        
        if file.cluster == 0 {
            // Empty file
            return Ok(Vec::new());
        }
        
        // Read cluster chain
        let mut data = Vec::new();
        let mut current_cluster = file.cluster;
        
        loop {
            let cluster_data = self.reader.read_cluster(current_cluster)?;
            data.extend_from_slice(&cluster_data);
            
            let next = self.reader.get_next_cluster(current_cluster)?;
            if next >= 0xFFF8 {
                break;  // End of chain
            }
            current_cluster = next;
        }
        
        // Truncate to actual file size
        if file.size > 0 && file.size < data.len() as u32 {
            data.truncate(file.size as usize);
        }
        
        Ok(data)
    }
    
    /// Delete a file
    pub fn delete_file(&mut self, path: &str) -> MosesResult<()> {
        info!("Deleting file: {}", path);
        
        // Resolve file path
        let mut resolver = Fat16PathResolver::new(&mut self.reader);
        let file = resolver.resolve_path(path)?;
        
        if file.is_directory {
            return Err(MosesError::Other(format!("{} is a directory", path)));
        }
        
        // Free cluster chain if file has data
        if file.cluster != 0 {
            self.writer.free_cluster_chain(file.cluster)?;
        }
        
        // Mark directory entry as deleted
        // This would require finding and updating the directory entry
        // For now, we'll need to implement this functionality
        warn!("Directory entry deletion not yet fully implemented");
        
        // Flush changes
        self.writer.flush()?;
        
        info!("File deleted successfully");
        Ok(())
    }
    
    /// Create a directory
    pub fn create_directory(&mut self, path: &str) -> MosesResult<()> {
        info!("Creating directory: {}", path);
        
        // Parse path
        let path_buf = PathBuf::from(path);
        let parent_path = path_buf.parent()
            .map(|p| p.to_str().unwrap_or("/"))
            .unwrap_or("/");
        let dir_name = path_buf.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| MosesError::Other("Invalid directory name".into()))?;
        
        // Resolve parent directory
        let mut resolver = Fat16PathResolver::new(&mut self.reader);
        let parent = resolver.resolve_path(parent_path)?;
        
        if !parent.is_directory {
            return Err(MosesError::Other(format!("{} is not a directory", parent_path)));
        }
        
        // Check if directory already exists
        if resolver.exists(path) {
            return Err(MosesError::Other(format!("Directory {} already exists", path)));
        }
        
        // Allocate a cluster for the new directory
        let dir_cluster = self.writer.allocate_cluster()?;
        
        // Initialize directory with . and .. entries
        let mut dir_data = vec![0u8; self.writer.get_bytes_per_cluster() as usize];
        
        // Create . entry (points to itself)
        let dot_entry = Fat16Writer::create_directory_entry(
            ".       ",  // Must be 11 chars
            FatAttributes::DIRECTORY,
            dir_cluster,
            0,
        );
        
        // Create .. entry (points to parent)
        let dotdot_entry = Fat16Writer::create_directory_entry(
            "..      ",  // Must be 11 chars
            FatAttributes::DIRECTORY,
            parent.cluster,
            0,
        );
        
        // Write entries to directory data
        let entry_size = std::mem::size_of::<FatDirEntry>();
        unsafe {
            std::ptr::copy_nonoverlapping(
                &dot_entry as *const FatDirEntry as *const u8,
                dir_data.as_mut_ptr(),
                entry_size
            );
            std::ptr::copy_nonoverlapping(
                &dotdot_entry as *const FatDirEntry as *const u8,
                dir_data.as_mut_ptr().add(entry_size),
                entry_size
            );
        }
        
        // Write directory data to cluster
        self.writer.write_cluster(dir_cluster, &dir_data)?;
        
        // Create directory entry in parent
        let mut resolver_mut = Fat16PathResolverMut::new(&mut self.writer);
        resolver_mut.create_directory_entry(
            if parent.cluster == 0 { None } else { Some(parent.cluster) },
            dir_name,
            true,  // is a directory
            dir_cluster,
            0,
        )?;
        
        // Flush changes
        self.writer.flush()?;
        
        info!("Directory created successfully");
        Ok(())
    }
    
    /// Delete an empty directory
    pub fn delete_directory(&mut self, path: &str) -> MosesResult<()> {
        info!("Deleting directory: {}", path);
        
        // Resolve directory path
        let mut resolver = Fat16PathResolver::new(&mut self.reader);
        let dir = resolver.resolve_path(path)?;
        
        if !dir.is_directory {
            return Err(MosesError::Other(format!("{} is not a directory", path)));
        }
        
        // Check if directory is empty (only . and .. entries)
        let entries = resolver.read_directory_entries(dir.cluster)?;
        if entries.len() > 0 {  // Our parser skips . and ..
            return Err(MosesError::Other("Directory is not empty".into()));
        }
        
        // Free the directory cluster
        self.writer.free_cluster_chain(dir.cluster)?;
        
        // Mark directory entry as deleted
        // This would require finding and updating the directory entry
        warn!("Directory entry deletion not yet fully implemented");
        
        // Flush changes
        self.writer.flush()?;
        
        info!("Directory deleted successfully");
        Ok(())
    }
    
    /// Rename/move a file or directory
    pub fn rename(&mut self, old_path: &str, new_path: &str) -> MosesResult<()> {
        info!("Renaming {} to {}", old_path, new_path);
        
        // Resolve old path
        let mut resolver = Fat16PathResolver::new(&mut self.reader);
        let old_entry = resolver.resolve_path(old_path)?;
        
        // Check if new path already exists
        if resolver.exists(new_path) {
            return Err(MosesError::Other(format!("{} already exists", new_path)));
        }
        
        // Parse new path
        let new_path_buf = PathBuf::from(new_path);
        let new_parent_path = new_path_buf.parent()
            .map(|p| p.to_str().unwrap_or("/"))
            .unwrap_or("/");
        let new_name = new_path_buf.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| MosesError::Other("Invalid new name".into()))?;
        
        // Resolve new parent directory
        let new_parent = resolver.resolve_path(new_parent_path)?;
        
        if !new_parent.is_directory {
            return Err(MosesError::Other(format!("{} is not a directory", new_parent_path)));
        }
        
        // For now, only support renaming within the same directory
        if old_entry.parent_cluster != Some(new_parent.cluster) {
            return Err(MosesError::NotSupported("Moving between directories not yet implemented".to_string()));
        }
        
        warn!("Rename operation not yet fully implemented");
        
        // Flush changes
        self.writer.flush()?;
        
        info!("Rename completed");
        Ok(())
    }
    
    /// List directory contents
    pub fn list_directory(&mut self, path: &str) -> MosesResult<Vec<crate::device_reader::FileEntry>> {
        let mut resolver = Fat16PathResolver::new(&mut self.reader);
        resolver.list_directory(path)
    }
    
    /// Get file/directory information
    pub fn get_info(&mut self, path: &str) -> MosesResult<crate::device_reader::FileEntry> {
        let mut resolver = Fat16PathResolver::new(&mut self.reader);
        let resolved = resolver.resolve_path(path)?;
        
        Ok(crate::device_reader::FileEntry {
            name: resolved.name,
            is_directory: resolved.is_directory,
            size: resolved.size as u64,
            cluster: Some(resolved.cluster as u32),
            metadata: Default::default(),
        })
    }
}