// exFAT file operations - helpers for creating and writing files
// This module provides high-level operations for working with exFAT filesystems

use std::io::{Write, Seek};
use moses_core::MosesError;
use super::directory_entries::{DirectoryEntrySetBuilder};
use super::bitmap::BitmapAllocator;
use super::structures::*;
use crate::families::fat::common::cluster_io::{write_cluster, clusters_needed};

/// High-level file writer for exFAT
pub struct ExFatFileWriter<'a, W: Write + Seek> {
    writer: &'a mut W,
    allocator: &'a mut BitmapAllocator,
    sectors_per_cluster: u32,
    bytes_per_sector: u32,
    data_start_offset: u64,
}

impl<'a, W: Write + Seek> ExFatFileWriter<'a, W> {
    pub fn new(
        writer: &'a mut W,
        allocator: &'a mut BitmapAllocator,
        sectors_per_cluster: u32,
        bytes_per_sector: u32,
        data_start_offset: u64,
    ) -> Self {
        Self {
            writer,
            allocator,
            sectors_per_cluster,
            bytes_per_sector,
            data_start_offset,
        }
    }
    
    /// Write a file with data and return its directory entries
    pub fn write_file(
        &mut self,
        name: &str,
        data: &[u8],
        attributes: u16,
    ) -> Result<Vec<ExFatDirectoryEntry>, MosesError> {
        let bytes_per_cluster = self.sectors_per_cluster * self.bytes_per_sector;
        
        // Calculate clusters needed
        let clusters = clusters_needed(data.len() as u64, bytes_per_cluster);
        
        // Allocate clusters
        let first_cluster = if clusters > 0 {
            if clusters == 1 {
                self.allocator.allocate_cluster()?
            } else {
                // Try to allocate contiguous clusters for better performance
                self.allocator.allocate_contiguous(clusters)
                    .or_else(|_| {
                        // Fall back to non-contiguous allocation
                        let mut first = 0;
                        for i in 0..clusters {
                            let cluster = self.allocator.allocate_cluster()?;
                            if i == 0 {
                                first = cluster;
                            }
                            // In a real implementation, we'd update FAT chains here
                        }
                        Ok::<u32, MosesError>(first)
                    })?
            }
        } else {
            0  // Empty file
        };
        
        // Write data to clusters
        if first_cluster > 0 && !data.is_empty() {
            let mut offset = 0;
            let mut current_cluster = first_cluster;
            
            while offset < data.len() {
                let chunk_size = std::cmp::min(bytes_per_cluster as usize, data.len() - offset);
                let chunk = &data[offset..offset + chunk_size];
                
                write_cluster(
                    self.writer,
                    current_cluster,
                    chunk,
                    self.sectors_per_cluster,
                    self.bytes_per_sector,
                    self.data_start_offset,
                )?;
                
                offset += chunk_size;
                if offset < data.len() {
                    // In a real implementation, follow FAT chain or allocate next cluster
                    current_cluster += 1;
                }
            }
        }
        
        // Create directory entries
        let entries = DirectoryEntrySetBuilder::new_file(name)
            .size(data.len() as u64)
            .first_cluster(first_cluster)
            .attributes(attributes)
            .build();
        
        Ok(entries)
    }
    
    /// Create an empty directory
    pub fn create_directory(
        &mut self,
        name: &str,
        parent_cluster: u32,
    ) -> Result<Vec<ExFatDirectoryEntry>, MosesError> {
        // Allocate one cluster for the directory
        let dir_cluster = self.allocator.allocate_cluster()?;
        
        // Create dot entries for the directory
        use super::directory_entries::create_dot_entries;
        let dot_entries = create_dot_entries(dir_cluster, parent_cluster);
        
        // Write dot entries to the allocated cluster
        let mut entry_bytes = Vec::new();
        for entry in &dot_entries {
            entry_bytes.extend_from_slice(&entry.to_bytes());
        }
        
        // Pad to cluster size
        let bytes_per_cluster = self.sectors_per_cluster * self.bytes_per_sector;
        while entry_bytes.len() < bytes_per_cluster as usize {
            entry_bytes.extend_from_slice(&[0u8; 32]);
        }
        
        write_cluster(
            self.writer,
            dir_cluster,
            &entry_bytes,
            self.sectors_per_cluster,
            self.bytes_per_sector,
            self.data_start_offset,
        )?;
        
        // Create directory entry for parent
        let entries = DirectoryEntrySetBuilder::new_directory(name)
            .first_cluster(dir_cluster)
            .build();
        
        Ok(entries)
    }
}

/// Helper to write a directory table with multiple entries
pub fn write_directory_table<W: Write + Seek>(
    writer: &mut W,
    entries: &[ExFatDirectoryEntry],
    cluster: u32,
    sectors_per_cluster: u32,
    bytes_per_sector: u32,
    data_start_offset: u64,
) -> Result<(), MosesError> {
    let mut entry_bytes = Vec::new();
    
    for entry in entries {
        entry_bytes.extend_from_slice(&entry.to_bytes());
    }
    
    // Pad to cluster size
    let bytes_per_cluster = sectors_per_cluster * bytes_per_sector;
    while entry_bytes.len() < bytes_per_cluster as usize {
        entry_bytes.extend_from_slice(&[0u8; 32]);
    }
    
    write_cluster(
        writer,
        cluster,
        &entry_bytes,
        sectors_per_cluster,
        bytes_per_sector,
        data_start_offset,
    )
}

/// Create a simple filesystem structure with some test files
pub fn create_test_filesystem<W: Write + Seek>(
    writer: &mut W,
    allocator: &mut BitmapAllocator,
    root_cluster: u32,
    sectors_per_cluster: u32,
    bytes_per_sector: u32,
    data_start_offset: u64,
) -> Result<(), MosesError> {
    let mut file_writer = ExFatFileWriter::new(
        writer,
        allocator,
        sectors_per_cluster,
        bytes_per_sector,
        data_start_offset,
    );
    
    let mut root_entries = Vec::new();
    
    // Create a test text file
    let readme_data = b"This is a test exFAT filesystem created by Moses.\n";
    let readme_entries = file_writer.write_file(
        "README.TXT",
        readme_data,
        EXFAT_ATTR_ARCHIVE,
    )?;
    root_entries.extend(readme_entries);
    
    // Create a test directory
    let docs_entries = file_writer.create_directory("DOCS", root_cluster)?;
    root_entries.extend(docs_entries);
    
    // Write root directory entries
    write_directory_table(
        writer,
        &root_entries,
        root_cluster,
        sectors_per_cluster,
        bytes_per_sector,
        data_start_offset,
    )?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::fat::exfat::bitmap::ExFatBitmap;
    use std::io::Cursor;
    
    #[test]
    fn test_file_writer() {
        let mut buffer = Cursor::new(vec![0u8; 1024 * 1024]); // 1MB buffer
        let bitmap = ExFatBitmap::new(256);
        let mut allocator = BitmapAllocator::from_bitmap(bitmap);
        
        let mut writer = ExFatFileWriter::new(
            &mut buffer,
            &mut allocator,
            8,   // 8 sectors per cluster
            512, // 512 bytes per sector
            0,   // Data starts at offset 0 for test
        );
        
        let test_data = b"Hello, exFAT!";
        let entries = writer.write_file("test.txt", test_data, 0).unwrap();
        
        // Should have File + Stream + FileName entries
        assert!(entries.len() >= 3);
        assert_eq!(entries[0].entry_type(), EXFAT_ENTRY_FILE);
        
        // Check that cluster was allocated
        assert_eq!(allocator.free_clusters(), 255); // One cluster used
    }
}