// FAT16 Subdirectory Operations
// Handles subdirectory management for FAT16 filesystems

use moses_core::MosesError;
use crate::families::fat::fat16::writer::Fat16Writer;
use crate::families::fat::fat16::reader::Fat16Reader;
use crate::families::fat::common::{FatDirEntry, FatAttributes};
use log::{debug, trace};

/// Subdirectory operations support for FAT16
pub struct SubdirectoryOps;

impl SubdirectoryOps {
    /// Find a free directory entry in a subdirectory cluster chain
    pub fn find_free_entry_in_subdirectory(
        reader: &mut Fat16Reader,
        writer: &mut Fat16Writer,
        cluster: u16,
    ) -> Result<(u16, usize), MosesError> {
        debug!("Finding free entry in subdirectory cluster {}", cluster);
        
        let mut current_cluster = cluster;
        let entry_size = std::mem::size_of::<FatDirEntry>();
        
        loop {
            // Read cluster data
            let cluster_data = reader.read_cluster(current_cluster)?;
            let entries_per_cluster = cluster_data.len() / entry_size;
            
            // Look for a free entry
            for i in 0..entries_per_cluster {
                let offset = i * entry_size;
                if offset + entry_size > cluster_data.len() {
                    break;
                }
                
                let entry_bytes = &cluster_data[offset..offset + entry_size];
                let entry = unsafe {
                    std::ptr::read(entry_bytes.as_ptr() as *const FatDirEntry)
                };
                
                // Found a free entry (deleted or end of directory)
                if entry.name[0] == 0xE5 || entry.name[0] == 0x00 {
                    trace!("Found free entry at cluster {} index {}", current_cluster, i);
                    return Ok((current_cluster, i));
                }
            }
            
            // Check if we need to follow the chain
            let next_cluster = reader.get_next_cluster(current_cluster)?;
            
            if next_cluster >= 0xFFF8 {
                // End of chain - need to allocate a new cluster
                debug!("End of directory chain, allocating new cluster");
                let new_cluster = writer.allocate_cluster()?;
                
                // Link the new cluster to the chain
                writer.write_fat_entry(current_cluster, new_cluster)?;
                writer.write_fat_entry(new_cluster, 0xFFFF)?; // Mark as end of chain
                
                // Initialize the new cluster with zeros
                let empty_cluster = vec![0u8; writer.get_bytes_per_cluster() as usize];
                writer.write_cluster(new_cluster, &empty_cluster)?;
                
                return Ok((new_cluster, 0));
            }
            
            current_cluster = next_cluster;
        }
    }
    
    /// Write a directory entry to a specific location in a subdirectory
    pub fn write_entry_to_subdirectory(
        reader: &mut Fat16Reader,
        writer: &mut Fat16Writer,
        cluster: u16,
        index: usize,
        entry: &FatDirEntry,
    ) -> Result<(), MosesError> {
        debug!("Writing entry to subdirectory cluster {} index {}", cluster, index);
        
        // Read the cluster
        let mut cluster_data = reader.read_cluster(cluster)?;
        let entry_size = std::mem::size_of::<FatDirEntry>();
        let offset = index * entry_size;
        
        if offset + entry_size > cluster_data.len() {
            return Err(MosesError::Other("Invalid directory entry index".to_string()));
        }
        
        // Write the entry
        unsafe {
            std::ptr::copy_nonoverlapping(
                entry as *const FatDirEntry as *const u8,
                cluster_data.as_mut_ptr().add(offset),
                entry_size
            );
        }
        
        // Write back the cluster
        writer.write_cluster(cluster, &cluster_data)?;
        
        Ok(())
    }
    
    /// Create a new subdirectory with . and .. entries
    pub fn create_subdirectory(
        writer: &mut Fat16Writer,
        parent_cluster: u16,
    ) -> Result<u16, MosesError> {
        debug!("Creating new subdirectory with parent cluster {}", parent_cluster);
        
        // Allocate a cluster for the new directory
        let dir_cluster = writer.allocate_cluster()?;
        
        // Initialize directory data
        let mut dir_data = vec![0u8; writer.get_bytes_per_cluster() as usize];
        
        // Create . entry (points to itself)
        let dot_entry = Fat16Writer::create_directory_entry(
            ".       ",  // 8.3 format padded with spaces
            FatAttributes::DIRECTORY,
            dir_cluster,
            0,
        );
        
        // Create .. entry (points to parent)
        let dotdot_entry = Fat16Writer::create_directory_entry(
            "..      ",  // 8.3 format padded with spaces
            FatAttributes::DIRECTORY,
            parent_cluster,
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
        writer.write_cluster(dir_cluster, &dir_data)?;
        
        Ok(dir_cluster)
    }
    
    /// Update the size field of a directory entry
    pub fn update_entry_size(
        reader: &mut Fat16Reader,
        writer: &mut Fat16Writer,
        parent_cluster: Option<u16>,
        entry_name: &str,
        new_size: u32,
    ) -> Result<(), MosesError> {
        debug!("Updating size for entry '{}' to {}", entry_name, new_size);
        
        if parent_cluster.is_none() {
            // Root directory - would need special handling
            return Err(MosesError::NotSupported("Root directory size update not implemented".to_string()));
        }
        
        let cluster = parent_cluster.unwrap();
        let mut current_cluster = cluster;
        let entry_size = std::mem::size_of::<FatDirEntry>();
        
        loop {
            // Read cluster data
            let mut cluster_data = reader.read_cluster(current_cluster)?;
            let entries_per_cluster = cluster_data.len() / entry_size;
            
            // Look for the entry
            for i in 0..entries_per_cluster {
                let offset = i * entry_size;
                if offset + entry_size > cluster_data.len() {
                    break;
                }
                
                let entry_bytes = &cluster_data[offset..offset + entry_size];
                let mut entry = unsafe {
                    std::ptr::read(entry_bytes.as_ptr() as *const FatDirEntry)
                };
                
                // Skip deleted and empty entries
                if entry.name[0] == 0xE5 || entry.name[0] == 0x00 {
                    continue;
                }
                
                // Check if this is our entry (simplified name comparison)
                let entry_name_bytes = entry_name.as_bytes();
                let matches = if entry_name_bytes.len() <= 8 {
                    entry.name[..entry_name_bytes.len()] == *entry_name_bytes
                } else {
                    false
                };
                
                if matches {
                    // Update the size
                    entry.file_size = new_size;
                    
                    // Write back the entry
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            &entry as *const FatDirEntry as *const u8,
                            cluster_data.as_mut_ptr().add(offset),
                            entry_size
                        );
                    }
                    
                    // Write back the cluster
                    writer.write_cluster(current_cluster, &cluster_data)?;
                    return Ok(());
                }
            }
            
            // Get next cluster in chain
            let next_cluster = reader.get_next_cluster(current_cluster)?;
            if next_cluster >= 0xFFF8 {
                break;  // End of chain
            }
            current_cluster = next_cluster;
        }
        
        Err(MosesError::Other(format!("Entry '{}' not found", entry_name)))
    }
}