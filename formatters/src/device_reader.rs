// Common device reading abstraction for Windows
// Handles sector alignment and caching automatically

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::collections::HashMap;
use moses_core::MosesError;
use log::{debug, trace};

const SECTOR_SIZE: usize = 512;

/// A device reader that handles Windows sector alignment requirements automatically
/// 
/// Windows requires all reads from raw devices to be:
/// - Aligned to sector boundaries (512 bytes)
/// - In multiples of sector size
/// 
/// This abstraction handles these requirements transparently
pub struct AlignedDeviceReader {
    file: File,
    /// Cache of sectors we've already read
    sector_cache: HashMap<u64, Vec<u8>>,
    /// Optional limit on cache size (in sectors)
    max_cache_sectors: usize,
}

impl AlignedDeviceReader {
    /// Create a new aligned device reader
    pub fn new(file: File) -> Self {
        Self {
            file,
            sector_cache: HashMap::new(),
            max_cache_sectors: 1000, // Default: cache up to 500KB
        }
    }
    
    /// Create with a specific cache size limit
    pub fn with_cache_limit(file: File, max_sectors: usize) -> Self {
        Self {
            file,
            sector_cache: HashMap::new(),
            max_cache_sectors: max_sectors,
        }
    }
    
    /// Read bytes from any offset, handling alignment automatically
    pub fn read_at(&mut self, offset: u64, size: usize) -> Result<Vec<u8>, MosesError> {
        if size == 0 {
            return Ok(Vec::new());
        }
        
        // Calculate sector-aligned boundaries
        let start_sector = offset / SECTOR_SIZE as u64;
        let end_byte = offset + size as u64;
        let end_sector = (end_byte + SECTOR_SIZE as u64 - 1) / SECTOR_SIZE as u64;
        let sectors_needed = (end_sector - start_sector) as usize;
        
        trace!("Reading {} bytes at offset {:#x}", size, offset);
        trace!("  Requires sectors {} to {} ({} sectors)", start_sector, end_sector - 1, sectors_needed);
        
        // Collect all needed sectors
        let mut all_data = Vec::with_capacity(sectors_needed * SECTOR_SIZE);
        
        for sector_num in start_sector..end_sector {
            let sector_data = self.read_sector(sector_num)?;
            all_data.extend_from_slice(&sector_data);
        }
        
        // Extract the requested bytes
        let offset_in_first_sector = (offset % SECTOR_SIZE as u64) as usize;
        let result = all_data[offset_in_first_sector..offset_in_first_sector + size].to_vec();
        
        Ok(result)
    }
    
    /// Read a single sector, using cache if available
    fn read_sector(&mut self, sector_num: u64) -> Result<Vec<u8>, MosesError> {
        // Check cache first
        if let Some(cached) = self.sector_cache.get(&sector_num) {
            trace!("Sector {} found in cache", sector_num);
            return Ok(cached.clone());
        }
        
        // Read from disk
        let offset = sector_num * SECTOR_SIZE as u64;
        trace!("Reading sector {} from disk at offset {:#x}", sector_num, offset);
        
        self.file.seek(SeekFrom::Start(offset))
            .map_err(|e| MosesError::IoError(e))?;
        
        let mut buffer = vec![0u8; SECTOR_SIZE];
        self.file.read_exact(&mut buffer)
            .map_err(|e| MosesError::IoError(e))?;
        
        // Cache it (with size limit)
        if self.sector_cache.len() < self.max_cache_sectors {
            self.sector_cache.insert(sector_num, buffer.clone());
        }
        
        Ok(buffer)
    }
    
    /// Read multiple sectors efficiently
    pub fn read_sectors(&mut self, start_sector: u64, count: usize) -> Result<Vec<u8>, MosesError> {
        debug!("Reading {} sectors starting at sector {}", count, start_sector);
        
        let mut result = Vec::with_capacity(count * SECTOR_SIZE);
        
        // Try to read contiguous uncached sectors in one go
        let mut current_sector = start_sector;
        let end_sector = start_sector + count as u64;
        
        while current_sector < end_sector {
            // Find how many contiguous sectors we need to read from disk
            let mut contiguous_count = 0;
            let mut check_sector = current_sector;
            
            while check_sector < end_sector && !self.sector_cache.contains_key(&check_sector) {
                contiguous_count += 1;
                check_sector += 1;
            }
            
            if contiguous_count > 0 {
                // Read multiple sectors at once
                let offset = current_sector * SECTOR_SIZE as u64;
                let read_size = contiguous_count * SECTOR_SIZE;
                
                debug!("Reading {} contiguous sectors from disk at offset {:#x}", 
                       contiguous_count, offset);
                
                self.file.seek(SeekFrom::Start(offset))
                    .map_err(|e| MosesError::IoError(e))?;
                
                let mut buffer = vec![0u8; read_size];
                self.file.read_exact(&mut buffer)
                    .map_err(|e| MosesError::IoError(e))?;
                
                // Add to result and cache
                for i in 0..contiguous_count {
                    let sector_data = &buffer[i * SECTOR_SIZE..(i + 1) * SECTOR_SIZE];
                    result.extend_from_slice(sector_data);
                    
                    if self.sector_cache.len() < self.max_cache_sectors {
                        self.sector_cache.insert(current_sector + i as u64, sector_data.to_vec());
                    }
                }
                
                current_sector += contiguous_count as u64;
            } else {
                // Use cached sector
                let cached = self.sector_cache.get(&current_sector)
                    .ok_or_else(|| MosesError::Other("Sector should be cached but isn't".to_string()))?;
                result.extend_from_slice(cached);
                current_sector += 1;
            }
        }
        
        Ok(result)
    }
    
    /// Clear the cache
    pub fn clear_cache(&mut self) {
        self.sector_cache.clear();
    }
    
    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        (self.sector_cache.len(), self.max_cache_sectors)
    }
}

// Implement standard Read trait for AlignedDeviceReader
impl Read for AlignedDeviceReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Get current position
        let pos = self.file.stream_position()?;
        
        // Read using our aligned method
        let data = self.read_at(pos, buf.len())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        
        // Copy to buffer
        let bytes_read = data.len().min(buf.len());
        buf[..bytes_read].copy_from_slice(&data[..bytes_read]);
        
        // Update file position
        self.file.seek(SeekFrom::Start(pos + bytes_read as u64))?;
        
        Ok(bytes_read)
    }
}

// Implement Seek trait for AlignedDeviceReader
impl Seek for AlignedDeviceReader {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(pos)
    }
}

/// Trait for common filesystem operations
/// All filesystem readers should implement this
pub trait FilesystemReader {
    /// Read the boot sector or superblock
    fn read_metadata(&mut self) -> Result<(), MosesError>;
    
    /// List files in a directory
    fn list_directory(&mut self, path: &str) -> Result<Vec<FileEntry>, MosesError>;
    
    /// Read file contents
    fn read_file(&mut self, path: &str) -> Result<Vec<u8>, MosesError>;
    
    /// Get filesystem information
    fn get_info(&self) -> FilesystemInfo;
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub is_directory: bool,
    pub size: u64,
    pub cluster: Option<u32>, // For FAT-based filesystems
}

#[derive(Debug, Clone)]
pub struct FilesystemInfo {
    pub fs_type: String,
    pub label: Option<String>,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub cluster_size: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_offset_calculation() {
        // Test that we correctly calculate sector boundaries
        
        // Reading 10 bytes at offset 0 should read sector 0
        let start_sector = 0u64 / SECTOR_SIZE as u64;
        let end_sector = (0u64 + 10) / SECTOR_SIZE as u64 + 1;
        assert_eq!(start_sector, 0);
        assert_eq!(end_sector, 1);
        
        // Reading 10 bytes at offset 510 should read sectors 0 and 1
        let start_sector = 510u64 / SECTOR_SIZE as u64;
        let end_sector = (510u64 + 10 + SECTOR_SIZE as u64 - 1) / SECTOR_SIZE as u64;
        assert_eq!(start_sector, 0);
        assert_eq!(end_sector, 2);
    }
}