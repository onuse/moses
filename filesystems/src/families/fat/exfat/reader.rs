// exFAT filesystem reader - read exFAT volumes on any platform!
// Implements the exFAT specification for cross-platform access

use moses_core::{Device, MosesError};
use log::info;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};

// exFAT constants
pub const EXFAT_SIGNATURE: [u8; 8] = [0x45, 0x58, 0x46, 0x41, 0x54, 0x20, 0x20, 0x20]; // "EXFAT   "
const _SECTOR_SIZE: u32 = 512; // Minimum sector size

// Entry types
const _ENTRY_TYPE_ALLOCATION_BITMAP: u8 = 0x81;
const _ENTRY_TYPE_UPCASE_TABLE: u8 = 0x82;
const ENTRY_TYPE_VOLUME_LABEL: u8 = 0x83;
const ENTRY_TYPE_FILE: u8 = 0x85;
const ENTRY_TYPE_STREAM_EXTENSION: u8 = 0xC0;
const ENTRY_TYPE_FILE_NAME: u8 = 0xC1;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatBootSector {
    pub jump_boot: [u8; 3],
    pub fs_name: [u8; 8],              // "EXFAT   "
    pub must_be_zero: [u8; 53],
    pub partition_offset: u64,
    pub volume_length: u64,
    pub fat_offset: u32,
    pub fat_length: u32,
    pub cluster_heap_offset: u32,
    pub cluster_count: u32,
    pub first_cluster_of_root: u32,
    pub volume_serial_number: u32,
    pub fs_revision: u16,
    pub volume_flags: u16,
    pub bytes_per_sector_shift: u8,
    pub sectors_per_cluster_shift: u8,
    pub number_of_fats: u8,
    pub drive_select: u8,
    pub percent_in_use: u8,
    pub reserved: [u8; 7],
    pub boot_code: [u8; 390],
    pub boot_signature: u16,           // 0xAA55
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatDirectoryEntry {
    pub entry_type: u8,
    pub data: [u8; 31],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct FileDirectoryEntry {
    pub entry_type: u8,                // 0x85
    pub secondary_count: u8,
    pub set_checksum: u16,
    pub file_attributes: u16,
    pub reserved1: u16,
    pub create_timestamp: u32,
    pub last_modified_timestamp: u32,
    pub last_accessed_timestamp: u32,
    pub create_10ms_increment: u8,
    pub last_modified_10ms_increment: u8,
    pub create_tz_offset: u8,
    pub last_modified_tz_offset: u8,
    pub last_accessed_tz_offset: u8,
    pub reserved2: [u8; 7],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct StreamExtensionEntry {
    pub entry_type: u8,                // 0xC0
    pub general_secondary_flags: u8,
    pub reserved1: u8,
    pub name_length: u8,
    pub name_hash: u16,
    pub reserved2: u16,
    pub valid_data_length: u64,
    pub reserved3: u32,
    pub first_cluster: u32,
    pub data_length: u64,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct FileNameEntry {
    pub entry_type: u8,                // 0xC1
    pub general_secondary_flags: u8,
    pub file_name: [u16; 15],          // UTF-16LE
}

#[derive(Debug, Clone)]
pub struct ExFatFile {
    pub name: String,
    pub is_directory: bool,
    pub size: u64,
    pub first_cluster: u32,
    pub attributes: u16,
}

/// exFAT filesystem reader
pub struct ExFatReader {
    device: Device,
    boot_sector: ExFatBootSector,
    _bytes_per_sector: u32,
    _sectors_per_cluster: u32,
    bytes_per_cluster: u32,
    cluster_heap_offset: u64,
    fat_offset: u64,
    
    // Cache
    fat_cache: HashMap<u32, u32>,
    dir_cache: HashMap<String, Vec<ExFatFile>>,
}

impl ExFatReader {
    /// Open an exFAT filesystem for reading
    pub fn new(device: Device) -> Result<Self, MosesError> {
        info!("Opening exFAT filesystem on device: {}", device.name);
        
        // Read boot sector
        let boot_sector = Self::read_boot_sector(&device)?;
        
        // Validate signature
        if boot_sector.fs_name != EXFAT_SIGNATURE {
            return Err(MosesError::Other("Not an exFAT filesystem".to_string()));
        }
        
        // Calculate parameters
        let bytes_per_sector = 1u32 << boot_sector.bytes_per_sector_shift;
        let sectors_per_cluster = 1u32 << boot_sector.sectors_per_cluster_shift;
        let bytes_per_cluster = bytes_per_sector * sectors_per_cluster;
        
        let cluster_heap_offset = boot_sector.cluster_heap_offset as u64 * bytes_per_sector as u64;
        let fat_offset = boot_sector.fat_offset as u64 * bytes_per_sector as u64;
        
        // Copy values to avoid unaligned access
        let cluster_count = boot_sector.cluster_count;
        let first_cluster_of_root = boot_sector.first_cluster_of_root;
        
        info!("exFAT filesystem details:");
        info!("  Bytes per sector: {}", bytes_per_sector);
        info!("  Sectors per cluster: {}", sectors_per_cluster);
        info!("  Cluster count: {}", cluster_count);
        info!("  Root directory cluster: {}", first_cluster_of_root);
        
        Ok(ExFatReader {
            device,
            boot_sector,
            _bytes_per_sector: bytes_per_sector,
            _sectors_per_cluster: sectors_per_cluster,
            bytes_per_cluster,
            cluster_heap_offset,
            fat_offset,
            fat_cache: HashMap::new(),
            dir_cache: HashMap::new(),
        })
    }
    
    /// Read boot sector from device
    fn read_boot_sector(device: &Device) -> Result<ExFatBootSector, MosesError> {
        use crate::utils::{open_device_with_fallback, read_sector, get_device_path};
        
        let path = get_device_path(device);
        info!("Reading exFAT boot sector from path: {}", path);
        info!("Device mount points: {:?}", device.mount_points);
        
        // Use the fallback method which tries multiple paths
        let mut file = open_device_with_fallback(device)?;
        let buffer = read_sector(&mut file, 0)?;
        
        let boot_sector = unsafe {
            std::ptr::read_unaligned(buffer.as_ptr() as *const ExFatBootSector)
        };
        
        Ok(boot_sector)
    }
    
    /// Read a cluster by number
    fn read_cluster(&mut self, cluster_num: u32) -> Result<Vec<u8>, MosesError> {
        use crate::utils::{open_device_with_fallback, read_block};
        
        if cluster_num < 2 || cluster_num >= self.boot_sector.cluster_count + 2 {
            return Err(MosesError::Other(format!("Invalid cluster number: {}", cluster_num)));
        }
        
        let offset = self.cluster_heap_offset + 
                    ((cluster_num - 2) as u64 * self.bytes_per_cluster as u64);
        
        let mut file = open_device_with_fallback(&self.device)?;
        read_block(&mut file, offset, self.bytes_per_cluster as usize)
    }
    
    /// Get next cluster from FAT
    fn get_next_cluster(&mut self, cluster: u32) -> Result<Option<u32>, MosesError> {
        use crate::utils::open_device_with_fallback;
        
        // Check cache first
        if let Some(&next) = self.fat_cache.get(&cluster) {
            return Ok(if next >= 0xFFFFFFF8 { None } else { Some(next) });
        }
        
        // Read from FAT
        let fat_entry_offset = self.fat_offset + (cluster * 4) as u64;
        
        let mut file = open_device_with_fallback(&self.device)?;
        file.seek(SeekFrom::Start(fat_entry_offset))?;
        
        let mut buffer = [0u8; 4];
        file.read_exact(&mut buffer)?;
        
        let next_cluster = u32::from_le_bytes(buffer);
        
        // Cache it
        self.fat_cache.insert(cluster, next_cluster);
        
        // Check for end of chain (0xFFFFFFF8 - 0xFFFFFFFF)
        Ok(if next_cluster >= 0xFFFFFFF8 { None } else { Some(next_cluster) })
    }
    
    /// Read cluster chain
    fn read_cluster_chain(&mut self, first_cluster: u32, max_clusters: Option<usize>) -> Result<Vec<u8>, MosesError> {
        let mut data = Vec::new();
        let mut current_cluster = first_cluster;
        let mut clusters_read = 0;
        
        loop {
            let cluster_data = self.read_cluster(current_cluster)?;
            data.extend_from_slice(&cluster_data);
            
            clusters_read += 1;
            if let Some(max) = max_clusters {
                if clusters_read >= max {
                    break;
                }
            }
            
            match self.get_next_cluster(current_cluster)? {
                Some(next) => current_cluster = next,
                None => break,
            }
        }
        
        Ok(data)
    }
    
    /// Parse directory entries from cluster data
    fn parse_directory_entries(&self, data: &[u8]) -> Vec<ExFatFile> {
        let mut files = Vec::new();
        let mut offset = 0;
        
        while offset + 32 <= data.len() {
            let entry = unsafe {
                &*(data.as_ptr().add(offset) as *const ExFatDirectoryEntry)
            };
            
            // Check if entry is in use (bit 7 set)
            if entry.entry_type & 0x80 == 0 {
                offset += 32;
                continue;
            }
            
            // Handle file entry
            if entry.entry_type == ENTRY_TYPE_FILE {
                let file_entry = unsafe {
                    &*(data.as_ptr().add(offset) as *const FileDirectoryEntry)
                };
                
                // Read stream extension (should be next entry)
                if offset + 32 < data.len() {
                    let stream_entry = unsafe {
                        &*(data.as_ptr().add(offset + 32) as *const StreamExtensionEntry)
                    };
                    
                    if stream_entry.entry_type == ENTRY_TYPE_STREAM_EXTENSION {
                        // Read file name entries
                        let mut name = String::new();
                        let name_entries = (stream_entry.name_length + 14) / 15; // 15 chars per entry
                        
                        for i in 0..name_entries {
                            let name_offset = offset + 64 + (i as usize * 32);
                            if name_offset + 32 <= data.len() {
                                let name_entry = unsafe {
                                    &*(data.as_ptr().add(name_offset) as *const FileNameEntry)
                                };
                                
                                if name_entry.entry_type == ENTRY_TYPE_FILE_NAME {
                                    // Convert UTF-16LE to String
                                    // Copy the file_name array to avoid unaligned access
                                    let file_name = name_entry.file_name;
                                    for ch in file_name {
                                        if ch == 0 { break; }
                                        if let Some(c) = char::from_u32(ch as u32) {
                                            name.push(c);
                                        }
                                    }
                                }
                            }
                        }
                        
                        // Trim to actual name length
                        name.truncate(stream_entry.name_length as usize);
                        
                        files.push(ExFatFile {
                            name,
                            is_directory: file_entry.file_attributes & 0x10 != 0,
                            size: stream_entry.data_length,
                            first_cluster: stream_entry.first_cluster,
                            attributes: file_entry.file_attributes,
                        });
                        
                        // Skip all entries in this set
                        offset += 32 * (2 + name_entries as usize);
                        continue;
                    }
                }
            }
            
            offset += 32;
        }
        
        files
    }
    
    /// Read root directory
    pub fn read_root(&mut self) -> Result<Vec<ExFatFile>, MosesError> {
        self.read_directory_cluster(self.boot_sector.first_cluster_of_root)
    }
    
    /// Read directory by cluster
    fn read_directory_cluster(&mut self, cluster: u32) -> Result<Vec<ExFatFile>, MosesError> {
        // Check cache
        let cache_key = format!("cluster_{}", cluster);
        if let Some(cached) = self.dir_cache.get(&cache_key) {
            return Ok(cached.clone());
        }
        
        // Read directory data
        let data = self.read_cluster_chain(cluster, Some(100))?; // Limit to 100 clusters
        let files = self.parse_directory_entries(&data);
        
        // Cache it
        self.dir_cache.insert(cache_key, files.clone());
        
        Ok(files)
    }
    
    /// Read directory by path
    pub fn read_directory(&mut self, path: &str) -> Result<Vec<ExFatFile>, MosesError> {
        if path == "/" || path.is_empty() {
            return self.read_root();
        }
        
        // Navigate to the directory
        let mut current_cluster = self.boot_sector.first_cluster_of_root;
        let components: Vec<&str> = path.split('/')
            .filter(|s| !s.is_empty())
            .collect();
        
        for component in components {
            let entries = self.read_directory_cluster(current_cluster)?;
            
            let dir = entries.iter()
                .find(|e| e.name.eq_ignore_ascii_case(component) && e.is_directory)
                .ok_or_else(|| MosesError::Other(format!("Directory not found: {}", component)))?;
            
            current_cluster = dir.first_cluster;
        }
        
        self.read_directory_cluster(current_cluster)
    }
    
    /// Read file contents
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>, MosesError> {
        // Parse path
        let (dir_path, file_name) = if let Some(pos) = path.rfind('/') {
            (&path[..pos], &path[pos + 1..])
        } else {
            ("", path)
        };
        
        // Read directory
        let entries = self.read_directory(dir_path)?;
        
        // Find file
        let file = entries.iter()
            .find(|e| e.name.eq_ignore_ascii_case(file_name) && !e.is_directory)
            .ok_or_else(|| MosesError::Other(format!("File not found: {}", file_name)))?;
        
        // Read file data
        if file.first_cluster == 0 {
            // Empty file
            return Ok(Vec::new());
        }
        
        let mut data = self.read_cluster_chain(file.first_cluster, None)?;
        data.truncate(file.size as usize);
        
        Ok(data)
    }
    
    /// Get filesystem information
    pub fn get_info(&self) -> ExFatInfo {
        ExFatInfo {
            filesystem_type: "exFAT".to_string(),
            label: self.get_volume_label(),
            total_clusters: self.boot_sector.cluster_count,
            bytes_per_cluster: self.bytes_per_cluster,
            serial_number: self.boot_sector.volume_serial_number,
        }
    }
    
    /// Get volume label
    fn get_volume_label(&self) -> Option<String> {
        // Read root directory to find volume label entry
        let root_cluster = self.boot_sector.first_cluster_of_root;
        
        // Read root directory data directly (avoid mutable self)
        self.read_volume_label_from_cluster(root_cluster)
    }
    
    /// Helper to read volume label from a cluster (avoids mutable self)
    fn read_volume_label_from_cluster(&self, cluster: u32) -> Option<String> {
        use crate::utils::{open_device_read, read_block};
        
        // We need to read the cluster data without mutating self
        // So we'll do a direct read here
        let offset = self.cluster_heap_offset + 
                    ((cluster - 2) as u64 * self.bytes_per_cluster as u64);
        
        let mut file = open_device_read(&self.device).ok()?;
        let buffer = read_block(&mut file, offset, self.bytes_per_cluster as usize).ok()?;
        
        // Parse directory entries looking for volume label
        let mut offset = 0;
        while offset + 32 <= buffer.len() {
            let entry = unsafe {
                &*(buffer.as_ptr().add(offset) as *const ExFatDirectoryEntry)
            };
            
            // Check if it's a volume label entry (0x83)
            if entry.entry_type == ENTRY_TYPE_VOLUME_LABEL {
                // Volume label entry structure:
                // Byte 0: Type (0x83)
                // Byte 1: Character count (max 11)
                // Bytes 2-23: Volume label in UTF-16LE (11 characters max)
                let char_count = entry.data[0] as usize;
                if char_count > 11 {
                    offset += 32;
                    continue;
                }
                
                let mut label = String::new();
                for i in 0..char_count {
                    let byte_offset = 1 + i * 2; // Skip char_count byte, then 2 bytes per UTF-16 char
                    if byte_offset + 1 < 31 {
                        let ch = u16::from_le_bytes([
                            entry.data[byte_offset],
                            entry.data[byte_offset + 1],
                        ]);
                        if let Some(c) = char::from_u32(ch as u32) {
                            label.push(c);
                        }
                    }
                }
                
                return Some(label);
            }
            
            // If entry type is 0 or doesn't have the in-use bit set, we're done
            if entry.entry_type == 0 || entry.entry_type & 0x80 == 0 {
                break;
            }
            
            offset += 32;
        }
        
        None
    }
}

#[derive(Debug)]
pub struct ExFatInfo {
    pub filesystem_type: String,
    pub label: Option<String>,
    pub total_clusters: u32,
    pub bytes_per_cluster: u32,
    pub serial_number: u32,
}

/// Entry in an exFAT directory (public API)
#[derive(Debug, Clone)]
pub struct ExFatEntry {
    pub name: String,
    pub is_directory: bool,
    pub size: u64,
}

impl From<ExFatFile> for ExFatEntry {
    fn from(file: ExFatFile) -> Self {
        ExFatEntry {
            name: file.name,
            is_directory: file.is_directory,
            size: file.size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_exfat_constants() {
        assert_eq!(ENTRY_TYPE_FILE, 0x85);
        assert_eq!(ENTRY_TYPE_STREAM_EXTENSION, 0xC0);
        assert_eq!(ENTRY_TYPE_FILE_NAME, 0xC1);
    }
}