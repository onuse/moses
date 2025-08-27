// FAT32 filesystem reader using common device abstraction
// Handles Windows sector alignment automatically

use moses_core::{Device, MosesError};
use crate::device_reader::{AlignedDeviceReader, FilesystemReader, FileEntry, FilesystemInfo, FileMetadata};
use crate::utils::open_device_read;
use log::{info, debug};
use std::collections::HashMap;

// FAT32 structures
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

/// FAT32 filesystem reader with persistent file handle and aligned reads
pub struct Fat32Reader {
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

impl Fat32Reader {
    /// Open a FAT32 filesystem for reading
    pub fn new(device: Device) -> Result<Self, MosesError> {
        info!("Opening FAT32 filesystem on device: {}", device.name);
        
        // Open device for reading
        let file = open_device_read(&device)?;
        // Create aligned reader
        let mut reader = AlignedDeviceReader::new(file);
        
        // Read boot sector
        let boot_bytes = reader.read_at(0, 512)?;
        
        // Verify boot signature
        if boot_bytes.len() < 512 || boot_bytes[510] != 0x55 || boot_bytes[511] != 0xAA {
            return Err(MosesError::Other("Invalid FAT32 boot signature".into()));
        }
        
        // Parse boot sector
        let boot_sector = unsafe {
            std::ptr::read(boot_bytes.as_ptr() as *const Fat32BootSector)
        };
        
        // Basic validation
        let bytes_per_sector = boot_sector.bytes_per_sector;
        let sectors_per_cluster = boot_sector.sectors_per_cluster;
        
        if bytes_per_sector == 0 || sectors_per_cluster == 0 {
            return Err(MosesError::Other("Invalid FAT32 parameters".into()));
        }
        
        if boot_sector.root_entry_count != 0 || boot_sector.total_sectors_16 != 0 {
            return Err(MosesError::Other("Not a FAT32 filesystem (might be FAT16)".into()));
        }
        
        // Calculate filesystem parameters
        let bytes_per_sector = bytes_per_sector as u32;
        let sectors_per_cluster = sectors_per_cluster as u32;
        let bytes_per_cluster = bytes_per_sector * sectors_per_cluster;
        
        let fat_size_sectors = if boot_sector.fat_size_16 != 0 {
            boot_sector.fat_size_16 as u32
        } else {
            boot_sector.fat_size_32
        };
        
        let reserved_sectors = boot_sector.reserved_sectors as u32;
        let num_fats = boot_sector.num_fats as u32;
        
        let fat_start_sector = reserved_sectors;
        let fat_start_byte = fat_start_sector as u64 * bytes_per_sector as u64;
        let fat_size_bytes = fat_size_sectors as u64 * bytes_per_sector as u64;
        
        let data_start_sector = reserved_sectors + (num_fats * fat_size_sectors);
        let data_start_byte = data_start_sector as u64 * bytes_per_sector as u64;
        
        let total_sectors = if boot_sector.total_sectors_16 != 0 {
            boot_sector.total_sectors_16 as u32
        } else {
            boot_sector.total_sectors_32
        };
        
        let data_sectors = total_sectors - data_start_sector;
        let total_clusters = data_sectors / sectors_per_cluster;
        
        info!("FAT32 filesystem info:");
        info!("  Bytes per sector: {}", bytes_per_sector);
        info!("  Sectors per cluster: {}", sectors_per_cluster);
        info!("  FAT start: sector {}", fat_start_sector);
        info!("  Data start: sector {}", data_start_sector);
        let root_cluster_copy = boot_sector.root_cluster;
        info!("  Root cluster: {}", root_cluster_copy);
        info!("  Total clusters: {}", total_clusters);
        
        let volume_label = String::from_utf8_lossy(&boot_sector.volume_label)
            .trim()
            .to_string();
        info!("  Volume label: '{}'", volume_label);
        
        Ok(Fat32Reader {
            _device: device,
            reader,
            boot_sector,
            _bytes_per_sector: bytes_per_sector,
            _sectors_per_cluster: sectors_per_cluster,
            bytes_per_cluster,
            fat_start_byte,
            _fat_size_bytes: fat_size_bytes,
            data_start_byte,
            root_cluster: boot_sector.root_cluster,
            total_clusters,
            fat_cache: HashMap::new(),
            dir_cache: HashMap::new(),
        })
    }
    
    /// Get FAT entry for a cluster
    pub fn get_fat_entry(&mut self, cluster: u32) -> Result<u32, MosesError> {
        // Check cache first
        if let Some(&next) = self.fat_cache.get(&cluster) {
            return Ok(next);
        }
        
        // FAT32 uses 32-bit entries (but only 28 bits are used)
        let fat_offset = cluster * 4;
        let fat_byte_offset = self.fat_start_byte + fat_offset as u64;
        
        // Read 4 bytes for the FAT entry
        let entry_bytes = self.reader.read_at(fat_byte_offset, 4)?;
        if entry_bytes.len() < 4 {
            return Err(MosesError::IoError(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Failed to read FAT entry"
            )));
        }
        
        let entry = u32::from_le_bytes([
            entry_bytes[0], entry_bytes[1], 
            entry_bytes[2], entry_bytes[3]
        ]) & 0x0FFFFFFF;  // Mask to 28 bits
        
        // Cache the result
        self.fat_cache.insert(cluster, entry);
        
        Ok(entry)
    }
    
    /// Follow cluster chain to get all clusters for a file/directory
    pub fn get_cluster_chain(&mut self, start_cluster: u32) -> Result<Vec<u32>, MosesError> {
        let mut chain = Vec::new();
        let mut current = start_cluster;
        let mut iterations = 0;
        const MAX_ITERATIONS: u32 = 100000;
        
        while current >= 2 && current < 0x0FFFFFF8 {
            if iterations >= MAX_ITERATIONS {
                return Err(MosesError::Other("Cluster chain too long or circular".into()));
            }
            
            chain.push(current);
            current = self.get_fat_entry(current)?;
            iterations += 1;
        }
        
        Ok(chain)
    }
    
    /// Read data from a cluster
    pub fn read_cluster(&mut self, cluster: u32) -> Result<Vec<u8>, MosesError> {
        if cluster < 2 || cluster >= self.total_clusters + 2 {
            return Err(MosesError::Other(format!("Invalid cluster number: {}", cluster)));
        }
        
        // Calculate byte offset for this cluster
        // Cluster 2 is the first data cluster
        let cluster_offset = (cluster - 2) as u64 * self.bytes_per_cluster as u64;
        let byte_offset = self.data_start_byte + cluster_offset;
        
        self.reader.read_at(byte_offset, self.bytes_per_cluster as usize)
    }
    
    /// Parse directory entries from raw data
    fn parse_directory(&self, data: &[u8]) -> Vec<FileEntry> {
        let mut entries = Vec::new();
        let mut long_name_parts: Vec<LongNameEntry> = Vec::new();
        
        for chunk in data.chunks_exact(32) {
            // Check for end of directory
            if chunk[0] == 0x00 {
                break;
            }
            
            // Skip deleted entries
            if chunk[0] == 0xE5 {
                long_name_parts.clear();
                continue;
            }
            
            let dir_entry = unsafe {
                std::ptr::read(chunk.as_ptr() as *const Fat32DirEntry)
            };
            
            // Check if this is a long name entry
            if dir_entry.attributes == ATTR_LONG_NAME {
                let lfn = unsafe {
                    std::ptr::read(chunk.as_ptr() as *const LongNameEntry)
                };
                long_name_parts.push(lfn);
                continue;
            }
            
            // Skip volume label entries
            if dir_entry.attributes & ATTR_VOLUME_ID != 0 {
                long_name_parts.clear();
                continue;
            }
            
            // Build the name
            let name = if !long_name_parts.is_empty() {
                // Reconstruct long filename
                let mut full_name = String::new();
                
                // LFN entries are stored in reverse order
                for lfn in long_name_parts.iter().rev() {
                    // Extract characters from each part - copy arrays to avoid alignment issues
                    let name1 = lfn.name1;
                    let name2 = lfn.name2;
                    let name3 = lfn.name3;
                    
                    for &ch in &name1 {
                        if ch == 0 || ch == 0xFFFF { break; }
                        if let Some(c) = char::from_u32(ch as u32) {
                            full_name.push(c);
                        }
                    }
                    for &ch in &name2 {
                        if ch == 0 || ch == 0xFFFF { break; }
                        if let Some(c) = char::from_u32(ch as u32) {
                            full_name.push(c);
                        }
                    }
                    for &ch in &name3 {
                        if ch == 0 || ch == 0xFFFF { break; }
                        if let Some(c) = char::from_u32(ch as u32) {
                            full_name.push(c);
                        }
                    }
                }
                
                long_name_parts.clear();
                full_name
            } else {
                // Parse 8.3 name
                let name_part = String::from_utf8_lossy(&dir_entry.name[0..8])
                    .trim_end()
                    .to_string();
                let ext_part = String::from_utf8_lossy(&dir_entry.name[8..11])
                    .trim_end()
                    .to_string();
                
                if ext_part.is_empty() {
                    name_part
                } else {
                    format!("{}.{}", name_part, ext_part)
                }
            };
            
            // Skip special entries
            if name == "." || name == ".." {
                continue;
            }
            
            let cluster = ((dir_entry.first_cluster_hi as u32) << 16) | (dir_entry.first_cluster_lo as u32);
            let is_directory = (dir_entry.attributes & ATTR_DIRECTORY) != 0;
            
            entries.push(FileEntry {
                name: name.clone(),
                is_directory,
                size: if is_directory { 0 } else { dir_entry.file_size as u64 },
                cluster: Some(cluster),
                metadata: FileMetadata::default(),
            });
        }
        
        entries
    }
    
    /// Read a directory by cluster
    fn read_directory_cluster(&mut self, cluster: u32) -> Result<Vec<FileEntry>, MosesError> {
        // Get all clusters for this directory
        let clusters = self.get_cluster_chain(cluster)?;
        
        let mut all_data = Vec::new();
        for cluster in clusters {
            let data = self.read_cluster(cluster)?;
            all_data.extend_from_slice(&data);
        }
        
        Ok(self.parse_directory(&all_data))
    }
    
    /// Read root directory
    pub fn read_root(&mut self) -> Result<Vec<FileEntry>, MosesError> {
        self.read_directory_cluster(self.root_cluster)
    }
}

impl FilesystemReader for Fat32Reader {
    fn read_metadata(&mut self) -> Result<(), MosesError> {
        // Metadata is already read in new()
        Ok(())
    }
    
    fn get_info(&self) -> FilesystemInfo {
        let volume_label = String::from_utf8_lossy(&self.boot_sector.volume_label)
            .trim()
            .to_string();
        
        let bytes_per_sector = self.boot_sector.bytes_per_sector as u64;
        let sectors_per_cluster = self.boot_sector.sectors_per_cluster as u64;
        let total_sectors = if self.boot_sector.total_sectors_16 != 0 {
            self.boot_sector.total_sectors_16 as u64
        } else {
            self.boot_sector.total_sectors_32 as u64
        };
        
        let total_size = total_sectors * bytes_per_sector;
        let cluster_size = bytes_per_sector * sectors_per_cluster;
        
        // Estimate used space (very rough for FAT32)
        let used_size = total_size / 2;  // Just a placeholder
        
        FilesystemInfo {
            fs_type: "FAT32".to_string(),
            label: if volume_label.is_empty() { 
                None 
            } else { 
                Some(volume_label) 
            },
            total_bytes: total_size,
            used_bytes: used_size,
            cluster_size: Some(cluster_size as u32),
        }
    }
    
    fn list_directory(&mut self, path: &str) -> Result<Vec<FileEntry>, MosesError> {
        // Check cache first
        if let Some(entries) = self.dir_cache.get(path) {
            return Ok(entries.clone());
        }
        
        // Parse path and navigate to directory
        let path = path.trim_start_matches('/');
        
        let mut current_cluster = self.root_cluster;
        let mut current_path = String::new();
        
        if !path.is_empty() {
            let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            
            for component in components {
                let entries = self.read_directory_cluster(current_cluster)?;
                
                let entry = entries.iter()
                    .find(|e| e.name.eq_ignore_ascii_case(component))
                    .ok_or_else(|| MosesError::Other(format!("Directory '{}' not found", component)))?;
                
                if !entry.is_directory {
                    return Err(MosesError::Other(format!("'{}' is not a directory", component)));
                }
                
                // For FAT32, we need to extract cluster from the original dir entry
                // This is a limitation of our current structure - we'd need to store cluster in FileEntry
                // For now, re-read to get cluster
                let dir_data = self.read_cluster(current_cluster)?;
                for chunk in dir_data.chunks_exact(32) {
                    if chunk[0] == 0x00 || chunk[0] == 0xE5 {
                        continue;
                    }
                    
                    let dir_entry = unsafe {
                        std::ptr::read(chunk.as_ptr() as *const Fat32DirEntry)
                    };
                    
                    if dir_entry.attributes & ATTR_VOLUME_ID != 0 {
                        continue;
                    }
                    
                    let name_part = String::from_utf8_lossy(&dir_entry.name[0..8])
                        .trim_end()
                        .to_string();
                    
                    if name_part.eq_ignore_ascii_case(component) {
                        current_cluster = ((dir_entry.first_cluster_hi as u32) << 16) | 
                                        (dir_entry.first_cluster_lo as u32);
                        break;
                    }
                }
                
                if !current_path.is_empty() {
                    current_path.push('/');
                }
                current_path.push_str(component);
            }
        }
        
        // Read the directory
        let entries = self.read_directory_cluster(current_cluster)?;
        
        // Entries are ready - no path field to update
        
        // Cache the result
        self.dir_cache.insert(path.to_string(), entries.clone());
        
        Ok(entries)
    }
    
    fn read_file(&mut self, path: &str) -> Result<Vec<u8>, MosesError> {
        let path = path.trim_start_matches('/');
        
        // Navigate to parent directory and find file
        let (parent_path, file_name) = if let Some(pos) = path.rfind('/') {
            (&path[..pos], &path[pos + 1..])
        } else {
            ("", path)
        };
        
        let entries = self.list_directory(parent_path)?;
        
        let file_entry = entries.iter()
            .find(|e| e.name.eq_ignore_ascii_case(file_name))
            .ok_or_else(|| MosesError::Other(format!("File '{}' not found", file_name)))?;
        
        if file_entry.is_directory {
            return Err(MosesError::Other(format!("'{}' is a directory", file_name)));
        }
        
        // Get the file's starting cluster (we need to re-read directory to get it)
        let parent_cluster = if parent_path.is_empty() {
            self.root_cluster
        } else {
            // Navigate to parent directory to get its cluster
            // This is a limitation - we'd need to track clusters in our navigation
            self.root_cluster  // Simplified for now
        };
        
        let dir_data = self.read_cluster(parent_cluster)?;
        let mut file_cluster = 0u32;
        let mut file_size = 0u32;
        
        for chunk in dir_data.chunks_exact(32) {
            if chunk[0] == 0x00 || chunk[0] == 0xE5 {
                continue;
            }
            
            let dir_entry = unsafe {
                std::ptr::read(chunk.as_ptr() as *const Fat32DirEntry)
            };
            
            if dir_entry.attributes & ATTR_VOLUME_ID != 0 {
                continue;
            }
            
            let name_part = String::from_utf8_lossy(&dir_entry.name[0..8])
                .trim_end()
                .to_string();
            let ext_part = String::from_utf8_lossy(&dir_entry.name[8..11])
                .trim_end()
                .to_string();
            
            let full_name = if ext_part.is_empty() {
                name_part
            } else {
                format!("{}.{}", name_part, ext_part)
            };
            
            if full_name.eq_ignore_ascii_case(file_name) {
                file_cluster = ((dir_entry.first_cluster_hi as u32) << 16) | 
                              (dir_entry.first_cluster_lo as u32);
                file_size = dir_entry.file_size;
                break;
            }
        }
        
        if file_cluster == 0 {
            return Err(MosesError::Other(format!("File '{}' cluster not found", file_name)));
        }
        
        // Read file data
        let clusters = self.get_cluster_chain(file_cluster)?;
        let mut file_data = Vec::new();
        
        for cluster in clusters {
            let data = self.read_cluster(cluster)?;
            file_data.extend_from_slice(&data);
        }
        
        // Trim to actual file size
        file_data.truncate(file_size as usize);
        
        debug!("Read file '{}': {} bytes", path, file_data.len());
        Ok(file_data)
    }
}