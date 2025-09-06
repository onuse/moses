// FAT32 File Operations Module  
// High-level file operations using writer and path resolver

use moses_core::MosesError;
use crate::families::fat::fat32::writer::Fat32Writer;
use crate::families::fat::fat32::path_resolver::Fat32PathResolver;
use crate::families::fat::fat32::reader::{Fat32Reader, Fat32DirEntry, LongNameEntry};
use std::path::PathBuf;
use log::{info, debug};

type MosesResult<T> = Result<T, MosesError>;

// Directory entry constants
const ATTR_READ_ONLY: u8 = 0x01;
const ATTR_HIDDEN: u8 = 0x02;
const ATTR_SYSTEM: u8 = 0x04;
const ATTR_VOLUME_ID: u8 = 0x08;
const ATTR_DIRECTORY: u8 = 0x10;
const ATTR_ARCHIVE: u8 = 0x20;
const ATTR_LONG_NAME: u8 = ATTR_READ_ONLY | ATTR_HIDDEN | ATTR_SYSTEM | ATTR_VOLUME_ID;

/// High-level FAT32 file operations
pub struct Fat32FileOps {
    writer: Fat32Writer,
    reader: Fat32Reader,
}

impl Fat32FileOps {
    /// Create new file operations handler
    pub fn new(device: moses_core::Device) -> MosesResult<Self> {
        let writer = Fat32Writer::new(device.clone())?;
        let reader = Fat32Reader::new(device)?;
        
        Ok(Self {
            writer,
            reader,
        })
    }
    
    /// Write a file
    pub fn write_file(&mut self, path: &str, data: &[u8]) -> MosesResult<()> {
        info!("Writing file: {} ({} bytes)", path, data.len());
        
        // Parse path into directory and filename
        let path = PathBuf::from(path);
        let parent_path = path.parent()
            .map(|p| p.to_str().unwrap_or("/"))
            .unwrap_or("/");
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| MosesError::Other("Invalid filename".into()))?;
        
        // Resolve parent directory
        let mut resolver = Fat32PathResolver::new(&mut self.reader);
        let parent = resolver.resolve_path(parent_path)?;
        
        if !parent.is_directory {
            return Err(MosesError::Other(format!("{} is not a directory", parent_path)));
        }
        
        // Check if file already exists
        let entries = resolver.read_directory_entries(parent.cluster)?;
        let existing = entries.iter()
            .find(|e| e.name.eq_ignore_ascii_case(filename));
        
        let (start_cluster, mut dir_entry) = if let Some(existing_entry) = existing {
            if existing_entry.is_directory {
                return Err(MosesError::Other(format!("{} is a directory", filename)));
            }
            
            debug!("Overwriting existing file");
            // Reuse existing cluster chain
            (existing_entry.cluster, self.read_dir_entry(parent.cluster, filename)?)
        } else {
            debug!("Creating new file");
            // Allocate new cluster for file
            let cluster = self.writer.allocate_cluster()?;
            
            // Create directory entry
            let short_names: Vec<String> = entries.iter()
                .map(|e| e.short_name.clone())
                .collect();
            let short_name = Fat32Writer::create_short_name(filename, &short_names);
            
            let mut entry = Fat32Writer::create_directory_entry(
                &short_name,
                ATTR_ARCHIVE,
                cluster,
                data.len() as u32,
            );
            
            // Fill in the 8.3 name
            Self::fill_short_name(&mut entry, &short_name);
            
            (cluster, entry)
        };
        
        // Write file data
        self.writer.write_file_data(start_cluster, data)?;
        
        // Update directory entry with new size
        dir_entry.file_size = data.len() as u32;
        self.update_directory_entry(parent.cluster, filename, &dir_entry)?;
        
        // Flush changes
        self.writer.flush()?;
        
        info!("File written successfully");
        Ok(())
    }
    
    /// Create a directory
    pub fn create_directory(&mut self, path: &str) -> MosesResult<()> {
        info!("Creating directory: {}", path);
        
        // Parse path
        let path = PathBuf::from(path);
        let parent_path = path.parent()
            .map(|p| p.to_str().unwrap_or("/"))
            .unwrap_or("/");
        let dirname = path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| MosesError::Other("Invalid directory name".into()))?;
        
        // Resolve parent directory
        let mut resolver = Fat32PathResolver::new(&mut self.reader);
        let parent = resolver.resolve_path(parent_path)?;
        
        if !parent.is_directory {
            return Err(MosesError::Other(format!("{} is not a directory", parent_path)));
        }
        
        // Check if already exists
        let entries = resolver.read_directory_entries(parent.cluster)?;
        if entries.iter().any(|e| e.name.eq_ignore_ascii_case(dirname)) {
            return Err(MosesError::Other(format!("{} already exists", dirname)));
        }
        
        // Allocate cluster for new directory
        let dir_cluster = self.writer.allocate_cluster()?;
        
        // Create directory entries for . and ..
        self.create_dot_entries(dir_cluster, parent.cluster)?;
        
        // Create directory entry in parent
        let short_names: Vec<String> = entries.iter()
            .map(|e| e.short_name.clone())
            .collect();
        let short_name = Fat32Writer::create_short_name(dirname, &short_names);
        
        let mut dir_entry = Fat32Writer::create_directory_entry(
            &short_name,
            ATTR_DIRECTORY,
            dir_cluster,
            0,
        );
        
        Self::fill_short_name(&mut dir_entry, &short_name);
        
        // Add entry to parent directory
        self.add_directory_entry(parent.cluster, &dir_entry, Some(dirname))?;
        
        // Flush changes
        self.writer.flush()?;
        
        info!("Directory created successfully");
        Ok(())
    }
    
    /// Delete a file
    pub fn delete_file(&mut self, path: &str) -> MosesResult<()> {
        info!("Deleting file: {}", path);
        
        // Resolve path
        let mut resolver = Fat32PathResolver::new(&mut self.reader);
        let resolved = resolver.resolve_path(path)?;
        
        if resolved.is_directory {
            return Err(MosesError::Other(format!("{} is a directory", path)));
        }
        
        // Parse path for parent
        let path = PathBuf::from(path);
        let parent_path = path.parent()
            .map(|p| p.to_str().unwrap_or("/"))
            .unwrap_or("/");
        let parent = resolver.resolve_path(parent_path)?;
        
        // Free the cluster chain
        self.writer.free_cluster_chain(resolved.cluster)?;
        
        // Mark directory entry as deleted
        self.delete_directory_entry(parent.cluster, &resolved.name)?;
        
        // Flush changes
        self.writer.flush()?;
        
        info!("File deleted successfully");
        Ok(())
    }
    
    /// Delete a directory (must be empty)
    pub fn delete_directory(&mut self, path: &str) -> MosesResult<()> {
        info!("Deleting directory: {}", path);
        
        // Resolve path
        let mut resolver = Fat32PathResolver::new(&mut self.reader);
        let resolved = resolver.resolve_path(path)?;
        
        if !resolved.is_directory {
            return Err(MosesError::Other(format!("{} is not a directory", path)));
        }
        
        // Check if directory is empty (except for . and ..)
        let entries = resolver.read_directory_entries(resolved.cluster)?;
        if entries.len() > 0 {  // Our parser skips . and ..
            return Err(MosesError::Other("Directory is not empty".into()));
        }
        
        // Parse path for parent
        let path = PathBuf::from(path);
        let parent_path = path.parent()
            .map(|p| p.to_str().unwrap_or("/"))
            .unwrap_or("/");
        let parent = resolver.resolve_path(parent_path)?;
        
        // Free the cluster chain
        self.writer.free_cluster_chain(resolved.cluster)?;
        
        // Mark directory entry as deleted
        self.delete_directory_entry(parent.cluster, &resolved.name)?;
        
        // Flush changes
        self.writer.flush()?;
        
        info!("Directory deleted successfully");
        Ok(())
    }
    
    /// Rename/move a file or directory
    pub fn rename(&mut self, old_path: &str, new_path: &str) -> MosesResult<()> {
        info!("Renaming {} to {}", old_path, new_path);
        
        // Resolve old path
        let mut resolver = Fat32PathResolver::new(&mut self.reader);
        let old_resolved = resolver.resolve_path(old_path)?;
        
        // Parse paths
        let old_path_buf = PathBuf::from(old_path);
        let new_path_buf = PathBuf::from(new_path);
        
        let old_parent_path = old_path_buf.parent()
            .map(|p| p.to_str().unwrap_or("/"))
            .unwrap_or("/");
        let new_parent_path = new_path_buf.parent()
            .map(|p| p.to_str().unwrap_or("/"))
            .unwrap_or("/");
        
        let new_name = new_path_buf.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| MosesError::Other("Invalid new name".into()))?;
        
        let old_parent = resolver.resolve_path(old_parent_path)?;
        let new_parent = resolver.resolve_path(new_parent_path)?;
        
        if !new_parent.is_directory {
            return Err(MosesError::Other(format!("{} is not a directory", new_parent_path)));
        }
        
        // Check if new name already exists
        let new_entries = resolver.read_directory_entries(new_parent.cluster)?;
        if new_entries.iter().any(|e| e.name.eq_ignore_ascii_case(new_name)) {
            return Err(MosesError::Other(format!("{} already exists", new_name)));
        }
        
        // Read the old directory entry
        let old_entry = self.read_dir_entry(old_parent.cluster, &old_resolved.name)?;
        
        // Create new entry with updated name
        let short_names: Vec<String> = new_entries.iter()
            .map(|e| e.short_name.clone())
            .collect();
        let short_name = Fat32Writer::create_short_name(new_name, &short_names);
        
        let mut new_entry = old_entry;
        Self::fill_short_name(&mut new_entry, &short_name);
        
        // If moving directories, update .. entry
        if old_resolved.is_directory && old_parent.cluster != new_parent.cluster {
            self.update_dot_dot_entry(old_resolved.cluster, new_parent.cluster)?;
        }
        
        // Delete old entry
        self.delete_directory_entry(old_parent.cluster, &old_resolved.name)?;
        
        // Add new entry
        self.add_directory_entry(new_parent.cluster, &new_entry, Some(new_name))?;
        
        // Flush changes
        self.writer.flush()?;
        
        info!("Rename completed successfully");
        Ok(())
    }
    
    // Helper methods
    
    /// Fill short name into directory entry
    fn fill_short_name(entry: &mut Fat32DirEntry, short_name: &str) {
        // Clear name field
        entry.name = [0x20; 11];
        
        // Parse short name
        let parts: Vec<&str> = short_name.split('.').collect();
        let base = parts[0];
        let ext = if parts.len() > 1 { parts[1] } else { "" };
        
        // Fill base name (8 chars)
        for (i, ch) in base.chars().take(8).enumerate() {
            entry.name[i] = ch as u8;
        }
        
        // Fill extension (3 chars)
        for (i, ch) in ext.chars().take(3).enumerate() {
            entry.name[8 + i] = ch as u8;
        }
    }
    
    /// Create . and .. entries for a new directory
    fn create_dot_entries(&mut self, dir_cluster: u32, parent_cluster: u32) -> MosesResult<()> {
        let mut data = vec![0u8; self.writer.get_bytes_per_cluster() as usize];
        
        // Create . entry
        let mut dot_entry = Fat32Writer::create_directory_entry(".", ATTR_DIRECTORY, dir_cluster, 0);
        dot_entry.name = [b'.', 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20];
        
        // Create .. entry  
        let mut dotdot_entry = Fat32Writer::create_directory_entry("..", ATTR_DIRECTORY, parent_cluster, 0);
        dotdot_entry.name = [b'.', b'.', 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20];
        
        // Write entries to cluster
        unsafe {
            std::ptr::copy_nonoverlapping(
                &dot_entry as *const _ as *const u8,
                data.as_mut_ptr(),
                32,
            );
            std::ptr::copy_nonoverlapping(
                &dotdot_entry as *const _ as *const u8,
                data.as_mut_ptr().add(32),
                32,
            );
        }
        
        self.writer.write_cluster(dir_cluster, &data)?;
        Ok(())
    }
    
    /// Update .. entry in a directory
    fn update_dot_dot_entry(&mut self, dir_cluster: u32, new_parent: u32) -> MosesResult<()> {
        let mut data = self.writer.read_cluster(dir_cluster)?;
        
        // .. entry is at offset 32
        let entry = unsafe {
            &mut *(data.as_mut_ptr().add(32) as *mut Fat32DirEntry)
        };
        
        entry.first_cluster_hi = (new_parent >> 16) as u16;
        entry.first_cluster_lo = (new_parent & 0xFFFF) as u16;
        
        self.writer.write_cluster(dir_cluster, &data)?;
        Ok(())
    }
    
    /// Read a directory entry by name
    fn read_dir_entry(&mut self, dir_cluster: u32, name: &str) -> MosesResult<Fat32DirEntry> {
        let clusters = self.writer.get_cluster_chain(dir_cluster)?;
        
        for cluster in clusters {
            let data = self.writer.read_cluster(cluster)?;
            
            for chunk in data.chunks_exact(32) {
                if chunk[0] == 0x00 {
                    break; // End of directory
                }
                if chunk[0] == 0xE5 {
                    continue; // Deleted entry
                }
                
                let entry = unsafe {
                    std::ptr::read(chunk.as_ptr() as *const Fat32DirEntry)
                };
                
                if entry.attributes & ATTR_VOLUME_ID != 0 {
                    continue;
                }
                
                // Compare name (simplified - should handle LFN properly)
                let entry_name = Self::parse_short_name(&entry);
                if entry_name.eq_ignore_ascii_case(name) {
                    return Ok(entry);
                }
            }
        }
        
        Err(MosesError::Other(format!("Entry {} not found", name)))
    }
    
    /// Parse short name from directory entry
    fn parse_short_name(entry: &Fat32DirEntry) -> String {
        let base = String::from_utf8_lossy(&entry.name[0..8])
            .trim_end()
            .to_string();
        let ext = String::from_utf8_lossy(&entry.name[8..11])
            .trim_end()
            .to_string();
        
        if ext.is_empty() {
            base
        } else {
            format!("{}.{}", base, ext)
        }
    }
    
    /// Add a directory entry
    fn add_directory_entry(
        &mut self,
        dir_cluster: u32,
        entry: &Fat32DirEntry,
        long_name: Option<&str>,
    ) -> MosesResult<()> {
        let clusters = self.writer.get_cluster_chain(dir_cluster)?;
        
        // Calculate entries needed (1 for short name + LFN entries if needed)
        let lfn_entries = if let Some(name) = long_name {
            Self::calculate_lfn_entries(name)
        } else {
            0
        };
        let total_entries = 1 + lfn_entries;
        
        // Save last cluster before iteration
        let last_cluster = *clusters.last().ok_or_else(|| MosesError::Other("Empty cluster chain".into()))?;

        // Find free space in directory
        for cluster in clusters {
            let mut data = self.writer.read_cluster(cluster)?;
            let mut free_start = None;
            let mut free_count = 0;
            
            for (i, chunk) in data.chunks_exact(32).enumerate() {
                if chunk[0] == 0x00 || chunk[0] == 0xE5 {
                    if free_start.is_none() {
                        free_start = Some(i);
                    }
                    free_count += 1;
                    
                    if free_count >= total_entries {
                        // Found enough space - write entries
                        let offset = free_start.unwrap() * 32;
                        
                        // Write LFN entries if needed
                        if let Some(name) = long_name {
                            self.write_lfn_entries(&mut data[offset..], name, entry)?;
                        }
                        
                        // Write short name entry
                        let entry_offset = offset + (lfn_entries * 32);
                        unsafe {
                            std::ptr::copy_nonoverlapping(
                                entry as *const _ as *const u8,
                                data.as_mut_ptr().add(entry_offset),
                                32,
                            );
                        }
                        
                        self.writer.write_cluster(cluster, &data)?;
                        return Ok(());
                    }
                } else {
                    free_start = None;
                    free_count = 0;
                }
            }
        }
        
        // No space found - extend directory
        let new_cluster = self.writer.allocate_cluster()?;
        self.writer.write_fat_entry(last_cluster, new_cluster)?;
        
        // Write to new cluster
        let mut data = vec![0u8; self.writer.get_bytes_per_cluster() as usize];
        
        // Write LFN entries if needed
        if let Some(name) = long_name {
            self.write_lfn_entries(&mut data, name, entry)?;
        }
        
        // Write short name entry
        let entry_offset = lfn_entries * 32;
        unsafe {
            std::ptr::copy_nonoverlapping(
                entry as *const _ as *const u8,
                data.as_mut_ptr().add(entry_offset),
                32,
            );
        }
        
        self.writer.write_cluster(new_cluster, &data)?;
        Ok(())
    }
    
    /// Calculate number of LFN entries needed
    fn calculate_lfn_entries(name: &str) -> usize {
        (name.len() + 12) / 13
    }
    
    /// Write LFN entries
    fn write_lfn_entries(&self, data: &mut [u8], name: &str, short_entry: &Fat32DirEntry) -> MosesResult<()> {
        let checksum = Self::calculate_checksum(&short_entry.name);
        let entries_needed = Self::calculate_lfn_entries(name);
        let chars: Vec<char> = name.chars().collect();
        
        for i in 0..entries_needed {
            let is_last = i == entries_needed - 1;
            let sequence = (entries_needed - i) as u8;
            let order = if is_last { sequence | 0x40 } else { sequence };
            
            let mut lfn = LongNameEntry {
                order,
                name1: [0xFFFF; 5],
                attributes: ATTR_LONG_NAME,
                entry_type: 0,
                checksum,
                name2: [0xFFFF; 6],
                first_cluster: 0,
                name3: [0xFFFF; 2],
            };
            
            // Fill in characters
            let char_offset = i * 13;
            for j in 0..5 {
                if char_offset + j < chars.len() {
                    lfn.name1[j] = chars[char_offset + j] as u16;
                }
            }
            for j in 0..6 {
                if char_offset + 5 + j < chars.len() {
                    lfn.name2[j] = chars[char_offset + 5 + j] as u16;
                }
            }
            for j in 0..2 {
                if char_offset + 11 + j < chars.len() {
                    lfn.name3[j] = chars[char_offset + 11 + j] as u16;
                }
            }
            
            // Write LFN entry
            let entry_offset = (entries_needed - 1 - i) * 32;
            unsafe {
                std::ptr::copy_nonoverlapping(
                    &lfn as *const _ as *const u8,
                    data.as_mut_ptr().add(entry_offset),
                    32,
                );
            }
        }
        
        Ok(())
    }
    
    /// Calculate checksum for short name
    fn calculate_checksum(name: &[u8; 11]) -> u8 {
        let mut sum = 0u8;
        for &byte in name {
            sum = sum.rotate_right(1).wrapping_add(byte);
        }
        sum
    }
    
    /// Update a directory entry
    fn update_directory_entry(
        &mut self,
        dir_cluster: u32,
        name: &str,
        new_entry: &Fat32DirEntry,
    ) -> MosesResult<()> {
        let clusters = self.writer.get_cluster_chain(dir_cluster)?;
        
        for cluster in clusters {
            let mut data = self.writer.read_cluster(cluster)?;
            
            for chunk in data.chunks_exact_mut(32) {
                if chunk[0] == 0x00 {
                    break;
                }
                if chunk[0] == 0xE5 {
                    continue;
                }
                
                let entry = unsafe {
                    &mut *(chunk.as_mut_ptr() as *mut Fat32DirEntry)
                };
                
                if entry.attributes & ATTR_VOLUME_ID != 0 {
                    continue;
                }
                
                let entry_name = Self::parse_short_name(entry);
                if entry_name.eq_ignore_ascii_case(name) {
                    *entry = *new_entry;
                    self.writer.write_cluster(cluster, &data)?;
                    return Ok(());
                }
            }
        }
        
        Err(MosesError::Other(format!("Entry {} not found", name)))
    }
    
    /// Delete a directory entry
    fn delete_directory_entry(&mut self, dir_cluster: u32, name: &str) -> MosesResult<()> {
        let clusters = self.writer.get_cluster_chain(dir_cluster)?;
        
        for cluster in clusters {
            let mut data = self.writer.read_cluster(cluster)?;
            let mut deleted_short = false;
            
            for chunk in data.chunks_exact_mut(32) {
                if chunk[0] == 0x00 {
                    break;
                }
                if chunk[0] == 0xE5 {
                    continue;
                }
                
                let entry = unsafe {
                    std::ptr::read(chunk.as_ptr() as *const Fat32DirEntry)
                };
                
                // Check if this is an LFN entry
                if entry.attributes == ATTR_LONG_NAME {
                    if deleted_short {
                        // Mark LFN entry as deleted
                        chunk[0] = 0xE5;
                    }
                    continue;
                }
                
                if entry.attributes & ATTR_VOLUME_ID != 0 {
                    continue;
                }
                
                let entry_name = Self::parse_short_name(&entry);
                if entry_name.eq_ignore_ascii_case(name) {
                    // Mark as deleted
                    chunk[0] = 0xE5;
                    deleted_short = true;
                }
            }
            
            if deleted_short {
                self.writer.write_cluster(cluster, &data)?;
                return Ok(());
            }
        }
        
        Err(MosesError::Other(format!("Entry {} not found", name)))
    }
}