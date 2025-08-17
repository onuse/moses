// Improved FAT32 filesystem reader using common device abstraction
// Handles Windows sector alignment automatically

use moses_core::{Device, MosesError};
use crate::device_reader::{AlignedDeviceReader, FilesystemReader, FileEntry, FilesystemInfo};
use log::{info, debug};
use std::collections::HashMap;

// FAT32 structures (same as original)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Fat32BootSector {
    pub jmp_boot: [u8; 3],
    pub oem_name: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub num_fats: u8,
    pub root_entry_count: u16,  // 0 for FAT32
    pub total_sectors_16: u16,  // 0 for FAT32
    pub media: u8,
    pub fat_size_16: u16,        // 0 for FAT32
    pub sectors_per_track: u16,
    pub num_heads: u16,
    pub hidden_sectors: u32,
    pub total_sectors_32: u32,
    // FAT32 specific
    pub fat_size_32: u32,
    pub ext_flags: u16,
    pub fs_version: u16,
    pub root_cluster: u32,
    pub fs_info: u16,
    pub backup_boot_sector: u16,
    pub reserved: [u8; 12],
    pub drive_number: u8,
    pub reserved1: u8,
    pub boot_signature: u8,
    pub volume_id: u32,
    pub volume_label: [u8; 11],
    pub fs_type: [u8; 8],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Fat32DirEntry {
    pub name: [u8; 11],          // 8.3 format
    pub attributes: u8,
    pub nt_reserved: u8,
    pub creation_time_tenth: u8,
    pub creation_time: u16,
    pub creation_date: u16,
    pub last_access_date: u16,
    pub first_cluster_hi: u16,
    pub write_time: u16,
    pub write_date: u16,
    pub first_cluster_lo: u16,
    pub file_size: u32,
}

// Directory entry attributes
const ATTR_READ_ONLY: u8 = 0x01;
const ATTR_HIDDEN: u8 = 0x02;
const ATTR_SYSTEM: u8 = 0x04;
const ATTR_VOLUME_ID: u8 = 0x08;
const ATTR_DIRECTORY: u8 = 0x10;
const _ATTR_ARCHIVE: u8 = 0x20;
const ATTR_LONG_NAME: u8 = ATTR_READ_ONLY | ATTR_HIDDEN | ATTR_SYSTEM | ATTR_VOLUME_ID;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct LongNameEntry {
    pub order: u8,
    pub name1: [u16; 5],
    pub attributes: u8,  // Always 0x0F
    pub entry_type: u8,  // Always 0x00
    pub checksum: u8,
    pub name2: [u16; 6],
    pub first_cluster: u16,  // Always 0x0000
    pub name3: [u16; 2],
}

/// Improved FAT32 filesystem reader with persistent file handle and aligned reads
pub struct Fat32ReaderImproved {
    _device: Device,
    reader: AlignedDeviceReader,
    boot_sector: Fat32BootSector,
    
    // Filesystem parameters
    _bytes_per_sector: u32,
    _sectors_per_cluster: u32,
    bytes_per_cluster: u32,
    fat_start_byte: u64,
    _fat_size_bytes: u64,
    data_start_byte: u64,
    root_cluster: u32,
    total_clusters: u32,
    
    // Cache
    fat_cache: HashMap<u32, u32>,  // cluster -> next cluster
    dir_cache: HashMap<String, Vec<FileEntry>>,
}

impl Fat32ReaderImproved {
    /// Create a new FAT32 reader
    pub fn new(device: Device) -> Result<Self, MosesError> {
        use crate::utils::open_device_with_fallback;
        
        info!("Opening FAT32 filesystem on device: {}", device.name);
        
        // Open device with our aligned reader
        let file = open_device_with_fallback(&device)?;
        let mut reader = AlignedDeviceReader::new(file);
        
        // Read boot sector
        let boot_data = reader.read_at(0, 512)?;
        let boot_sector = unsafe {
            std::ptr::read_unaligned(boot_data.as_ptr() as *const Fat32BootSector)
        };
        
        // Validate FAT32
        if boot_sector.boot_signature != 0x29 {
            return Err(MosesError::Other("Invalid FAT32 boot signature".to_string()));
        }
        
        // Copy values to avoid unaligned access
        let bytes_per_sector = boot_sector.bytes_per_sector as u32;
        let sectors_per_cluster = boot_sector.sectors_per_cluster as u32;
        let reserved_sectors = boot_sector.reserved_sectors as u32;
        let fat_size_sectors = boot_sector.fat_size_32;
        let num_fats = boot_sector.num_fats as u32;
        let root_cluster = boot_sector.root_cluster;
        let total_sectors = boot_sector.total_sectors_32;
        
        // Calculate filesystem layout
        let bytes_per_cluster = bytes_per_sector * sectors_per_cluster;
        let fat_start_sector = reserved_sectors;
        let fat_start_byte = fat_start_sector as u64 * bytes_per_sector as u64;
        let fat_size_bytes = fat_size_sectors as u64 * bytes_per_sector as u64;
        let data_start_sector = reserved_sectors + (num_fats * fat_size_sectors);
        let data_start_byte = data_start_sector as u64 * bytes_per_sector as u64;
        
        // Calculate total clusters
        let data_sectors = total_sectors - data_start_sector;
        let total_clusters = data_sectors / sectors_per_cluster;
        
        info!("FAT32 filesystem details:");
        info!("  Bytes per sector: {}", bytes_per_sector);
        info!("  Sectors per cluster: {}", sectors_per_cluster);
        info!("  FAT starts at: {:#x}", fat_start_byte);
        info!("  Data starts at: {:#x}", data_start_byte);
        info!("  Root cluster: {}", root_cluster);
        info!("  Total clusters: {}", total_clusters);
        
        Ok(Self {
            _device: device,
            reader,
            boot_sector,
            _bytes_per_sector: bytes_per_sector,
            _sectors_per_cluster: sectors_per_cluster,
            bytes_per_cluster,
            fat_start_byte,
            _fat_size_bytes: fat_size_bytes,
            data_start_byte,
            root_cluster,
            total_clusters,
            fat_cache: HashMap::new(),
            dir_cache: HashMap::new(),
        })
    }
    
    /// Read a cluster by number
    fn read_cluster(&mut self, cluster: u32) -> Result<Vec<u8>, MosesError> {
        if cluster < 2 || cluster >= self.total_clusters + 2 {
            return Err(MosesError::Other(format!("Invalid cluster number: {}", cluster)));
        }
        
        let offset = self.data_start_byte + ((cluster - 2) as u64 * self.bytes_per_cluster as u64);
        debug!("Reading cluster {} at offset {:#x}", cluster, offset);
        
        self.reader.read_at(offset, self.bytes_per_cluster as usize)
    }
    
    /// Get next cluster from FAT
    fn get_next_cluster(&mut self, cluster: u32) -> Result<Option<u32>, MosesError> {
        // Check cache first
        if let Some(&next) = self.fat_cache.get(&cluster) {
            return Ok(if next >= 0x0FFFFFF8 { None } else { Some(next) });
        }
        
        // Calculate FAT entry position
        let fat_offset = self.fat_start_byte + (cluster * 4) as u64;
        debug!("Reading FAT entry for cluster {} at offset {:#x}", cluster, fat_offset);
        
        // Read the 4-byte FAT entry (AlignedDeviceReader handles alignment)
        let entry_data = self.reader.read_at(fat_offset, 4)?;
        let fat_entry = u32::from_le_bytes([
            entry_data[0], entry_data[1], entry_data[2], entry_data[3]
        ]) & 0x0FFFFFFF;  // Mask off upper 4 bits
        
        // Cache it
        self.fat_cache.insert(cluster, fat_entry);
        
        // Check for end of chain
        if fat_entry >= 0x0FFFFFF8 {
            Ok(None)  // End of chain
        } else if fat_entry == 0 || fat_entry == 1 {
            Err(MosesError::Other("Invalid FAT entry".to_string()))
        } else {
            Ok(Some(fat_entry))
        }
    }
    
    /// Read a cluster chain
    fn read_cluster_chain(&mut self, start_cluster: u32) -> Result<Vec<u8>, MosesError> {
        let mut data = Vec::new();
        let mut current = start_cluster;
        let mut count = 0;
        
        loop {
            let cluster_data = self.read_cluster(current)?;
            data.extend_from_slice(&cluster_data);
            
            count += 1;
            if count > 10000 {
                return Err(MosesError::Other("Cluster chain too long".to_string()));
            }
            
            match self.get_next_cluster(current)? {
                Some(next) => current = next,
                None => break,
            }
        }
        
        Ok(data)
    }
    
    /// Parse a short (8.3) filename
    fn parse_short_name(name: &[u8; 11]) -> String {
        let mut result = String::new();
        
        // Parse base name (first 8 bytes)
        for &byte in &name[0..8] {
            if byte == 0x20 || byte == 0x00 {
                break;
            }
            result.push(byte as char);
        }
        
        // Parse extension (last 3 bytes)
        let ext_start = 8;
        let mut has_ext = false;
        for &byte in &name[ext_start..11] {
            if byte != 0x20 && byte != 0x00 {
                if !has_ext {
                    result.push('.');
                    has_ext = true;
                }
                result.push(byte as char);
            }
        }
        
        result
    }
    
    /// Parse directory entries from raw data
    fn parse_directory_entries(&self, data: &[u8]) -> Vec<FileEntry> {
        let mut entries = Vec::new();
        let mut offset = 0;
        let mut long_name_parts: Vec<LongNameEntry> = Vec::new();
        
        while offset + 32 <= data.len() {
            let entry_bytes = &data[offset..offset + 32];
            
            // Check for end of directory
            if entry_bytes[0] == 0x00 {
                break;
            }
            
            // Skip deleted entries
            if entry_bytes[0] == 0xE5 {
                offset += 32;
                continue;
            }
            
            // Check if it's a long name entry
            if entry_bytes[11] == ATTR_LONG_NAME {
                let long_entry = unsafe {
                    std::ptr::read_unaligned(entry_bytes.as_ptr() as *const LongNameEntry)
                };
                long_name_parts.push(long_entry);
            } else {
                // Regular directory entry
                let dir_entry = unsafe {
                    std::ptr::read_unaligned(entry_bytes.as_ptr() as *const Fat32DirEntry)
                };
                
                // Skip volume labels and special entries
                if dir_entry.attributes & ATTR_VOLUME_ID != 0 {
                    long_name_parts.clear();
                    offset += 32;
                    continue;
                }
                
                // Build the filename
                let name = if !long_name_parts.is_empty() {
                    // Reconstruct long filename
                    let mut long_name = String::new();
                    
                    // Sort by order (they're stored in reverse)
                    long_name_parts.sort_by(|a, b| (a.order & 0x3F).cmp(&(b.order & 0x3F)));
                    
                    for part in &long_name_parts {
                        // Extract characters from each part (copy arrays to avoid unaligned access)
                        let name1 = part.name1;
                        for &ch in &name1 {
                            if ch == 0 || ch == 0xFFFF { break; }
                            if let Some(c) = char::from_u32(ch as u32) {
                                long_name.push(c);
                            }
                        }
                        let name2 = part.name2;
                        for &ch in &name2 {
                            if ch == 0 || ch == 0xFFFF { break; }
                            if let Some(c) = char::from_u32(ch as u32) {
                                long_name.push(c);
                            }
                        }
                        let name3 = part.name3;
                        for &ch in &name3 {
                            if ch == 0 || ch == 0xFFFF { break; }
                            if let Some(c) = char::from_u32(ch as u32) {
                                long_name.push(c);
                            }
                        }
                    }
                    
                    long_name
                } else {
                    // Use short name
                    Self::parse_short_name(&dir_entry.name)
                };
                
                // Skip . and .. entries
                if name == "." || name == ".." {
                    long_name_parts.clear();
                    offset += 32;
                    continue;
                }
                
                // Get cluster number
                let cluster = ((dir_entry.first_cluster_hi as u32) << 16) | 
                             (dir_entry.first_cluster_lo as u32);
                
                entries.push(FileEntry {
                    name,
                    is_directory: dir_entry.attributes & ATTR_DIRECTORY != 0,
                    size: if dir_entry.attributes & ATTR_DIRECTORY != 0 { 0 } else { dir_entry.file_size as u64 },
                    cluster: Some(cluster),
                });
                
                long_name_parts.clear();
            }
            
            offset += 32;
        }
        
        entries
    }
    
    /// Read the root directory
    pub fn read_root(&mut self) -> Result<Vec<FileEntry>, MosesError> {
        self.read_directory_cluster(self.root_cluster)
    }
    
    /// Read a directory by cluster
    fn read_directory_cluster(&mut self, cluster: u32) -> Result<Vec<FileEntry>, MosesError> {
        debug!("Reading directory at cluster {}", cluster);
        
        let data = self.read_cluster_chain(cluster)?;
        Ok(self.parse_directory_entries(&data))
    }
}

impl FilesystemReader for Fat32ReaderImproved {
    fn read_metadata(&mut self) -> Result<(), MosesError> {
        // Already read in new()
        Ok(())
    }
    
    fn list_directory(&mut self, path: &str) -> Result<Vec<FileEntry>, MosesError> {
        // Check cache first
        if let Some(cached) = self.dir_cache.get(path) {
            return Ok(cached.clone());
        }
        
        let entries = if path == "/" || path.is_empty() {
            self.read_root()?
        } else {
            // Navigate to the directory
            let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            let mut current_cluster = self.root_cluster;
            
            for part in parts {
                let entries = self.read_directory_cluster(current_cluster)?;
                let dir = entries.iter()
                    .find(|e| e.name.eq_ignore_ascii_case(part) && e.is_directory)
                    .ok_or_else(|| MosesError::Other(format!("Directory not found: {}", part)))?;
                    
                current_cluster = dir.cluster.unwrap();
            }
            
            self.read_directory_cluster(current_cluster)?
        };
        
        // Cache the result
        self.dir_cache.insert(path.to_string(), entries.clone());
        
        Ok(entries)
    }
    
    fn read_file(&mut self, path: &str) -> Result<Vec<u8>, MosesError> {
        // Parse path to get directory and filename
        let (dir_path, file_name) = if let Some(pos) = path.rfind('/') {
            (&path[..pos], &path[pos + 1..])
        } else {
            ("", path)
        };
        
        // Get directory listing
        let entries = self.list_directory(dir_path)?;
        
        // Find the file
        let file = entries.iter()
            .find(|e| e.name.eq_ignore_ascii_case(file_name) && !e.is_directory)
            .ok_or_else(|| MosesError::Other(format!("File not found: {}", file_name)))?;
        
        // Read file data
        if let Some(cluster) = file.cluster {
            if cluster == 0 {
                // Empty file
                Ok(Vec::new())
            } else {
                let mut data = self.read_cluster_chain(cluster)?;
                data.truncate(file.size as usize);
                Ok(data)
            }
        } else {
            Ok(Vec::new())
        }
    }
    
    fn get_info(&self) -> FilesystemInfo {
        // Extract volume label
        let mut label = String::new();
        for &byte in &self.boot_sector.volume_label {
            if byte == 0x20 || byte == 0x00 {
                break;
            }
            label.push(byte as char);
        }
        
        let total_bytes = self.total_clusters as u64 * self.bytes_per_cluster as u64;
        
        FilesystemInfo {
            fs_type: "FAT32".to_string(),
            label: if label.is_empty() { None } else { Some(label) },
            total_bytes,
            used_bytes: 0, // Would need to scan FAT to calculate
            cluster_size: Some(self.bytes_per_cluster),
        }
    }
}