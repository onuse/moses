// FAT32 filesystem reader - read FAT32 volumes on any platform!
// This is simpler than ext4 as FAT32 has a more straightforward structure

use moses_core::{Device, MosesError};
use log::info;
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

#[derive(Debug, Clone)]
pub struct FatDirEntry {
    pub name: String,
    pub is_directory: bool,
    pub size: u32,
    pub cluster: u32,
    pub attributes: u8,
}

/// FAT32 filesystem reader
pub struct Fat32Reader {
    device: Device,
    boot_sector: Fat32BootSector,
    bytes_per_sector: u32,
    sectors_per_cluster: u32,
    bytes_per_cluster: u32,
    fat_start_sector: u32,
    _fat_size_sectors: u32,
    data_start_sector: u32,
    root_cluster: u32,
    
    // Cache
    fat_cache: HashMap<u32, Vec<u32>>,  // Cluster chain cache
    dir_cache: HashMap<u32, Vec<FatDirEntry>>,  // Directory cache
}

impl Fat32Reader {
    /// Open a FAT32 filesystem for reading
    pub fn new(device: Device) -> Result<Self, MosesError> {
        info!("Opening FAT32 filesystem on device: {}", device.name);
        
        // Read boot sector
        let boot_sector = Self::read_boot_sector(&device)?;
        
        // Validate it's FAT32
        if boot_sector.root_entry_count != 0 || boot_sector.total_sectors_16 != 0 {
            return Err(MosesError::Other("Not a FAT32 filesystem".to_string()));
        }
        
        let bytes_per_sector = boot_sector.bytes_per_sector as u32;
        let sectors_per_cluster = boot_sector.sectors_per_cluster as u32;
        let bytes_per_cluster = bytes_per_sector * sectors_per_cluster;
        let fat_start_sector = boot_sector.reserved_sectors as u32;
        let fat_size_sectors = boot_sector.fat_size_32;
        let data_start_sector = fat_start_sector + (boot_sector.num_fats as u32 * fat_size_sectors);
        let root_cluster = boot_sector.root_cluster;
        
        info!("FAT32 filesystem details:");
        info!("  Bytes per sector: {}", bytes_per_sector);
        info!("  Sectors per cluster: {}", sectors_per_cluster);
        info!("  Root cluster: {}", root_cluster);
        
        Ok(Fat32Reader {
            device,
            boot_sector,
            bytes_per_sector,
            sectors_per_cluster,
            bytes_per_cluster,
            fat_start_sector,
            _fat_size_sectors: fat_size_sectors,
            data_start_sector,
            root_cluster,
            fat_cache: HashMap::new(),
            dir_cache: HashMap::new(),
        })
    }
    
    /// Read boot sector from device
    fn read_boot_sector(device: &Device) -> Result<Fat32BootSector, MosesError> {
        use crate::utils::{open_device_read, read_sector};
        
        let mut file = open_device_read(device)?;
        let buffer = read_sector(&mut file, 0)?;
        
        // Parse boot sector
        let boot_sector = unsafe {
            std::ptr::read_unaligned(buffer.as_ptr() as *const Fat32BootSector)
        };
        
        // Basic validation
        if boot_sector.boot_signature != 0x29 {
            return Err(MosesError::Other("Invalid FAT32 boot signature".to_string()));
        }
        
        Ok(boot_sector)
    }
    
    /// Read a cluster from the device
    fn read_cluster(&mut self, cluster: u32) -> Result<Vec<u8>, MosesError> {
        use crate::utils::{open_device_read, read_block};
        
        if cluster < 2 {
            return Err(MosesError::Other("Invalid cluster number".to_string()));
        }
        
        let sector = self.data_start_sector + ((cluster - 2) * self.sectors_per_cluster);
        let offset = sector as u64 * self.bytes_per_sector as u64;
        
        let mut file = open_device_read(&self.device)?;
        read_block(&mut file, offset, self.bytes_per_cluster as usize)
    }
    
    /// Get the next cluster in the chain from FAT
    fn get_next_cluster(&mut self, cluster: u32) -> Result<Option<u32>, MosesError> {
        use crate::utils::{open_device_read, read_sector};
        
        // Calculate FAT entry position
        let fat_offset = cluster * 4;  // Each FAT32 entry is 4 bytes
        let fat_sector = self.fat_start_sector + (fat_offset / self.bytes_per_sector);
        let fat_entry_offset = (fat_offset % self.bytes_per_sector) as usize;
        
        let mut file = open_device_read(&self.device)?;
        let buffer = read_sector(&mut file, fat_sector as u64)?;
        
        // Read the 4-byte FAT entry
        let fat_entry = u32::from_le_bytes([
            buffer[fat_entry_offset],
            buffer[fat_entry_offset + 1],
            buffer[fat_entry_offset + 2],
            buffer[fat_entry_offset + 3],
        ]) & 0x0FFFFFFF;  // Mask off upper 4 bits
        
        // Check for end of chain
        if fat_entry >= 0x0FFFFFF8 {
            Ok(None)  // End of chain
        } else if fat_entry == 0 || fat_entry == 1 {
            Err(MosesError::Other("Invalid FAT entry".to_string()))
        } else {
            Ok(Some(fat_entry))
        }
    }
    
    /// Get all clusters in a chain
    fn get_cluster_chain(&mut self, start_cluster: u32) -> Result<Vec<u32>, MosesError> {
        // Check cache first
        if let Some(cached) = self.fat_cache.get(&start_cluster) {
            return Ok(cached.clone());
        }
        
        let mut chain = vec![start_cluster];
        let mut current = start_cluster;
        
        // Follow the chain
        while let Some(next) = self.get_next_cluster(current)? {
            if chain.len() > 100000 {
                return Err(MosesError::Other("Cluster chain too long".to_string()));
            }
            chain.push(next);
            current = next;
        }
        
        // Cache it
        self.fat_cache.insert(start_cluster, chain.clone());
        
        Ok(chain)
    }
    
    /// Read a directory
    pub fn read_directory(&mut self, cluster: u32) -> Result<Vec<FatDirEntry>, MosesError> {
        // Check cache first
        if let Some(cached) = self.dir_cache.get(&cluster) {
            return Ok(cached.clone());
        }
        
        let mut entries = Vec::new();
        let chain = self.get_cluster_chain(cluster)?;
        
        for cluster_num in chain {
            let data = self.read_cluster(cluster_num)?;
            let mut offset = 0;
            
            while offset < data.len() {
                let entry = unsafe {
                    &*(data.as_ptr().add(offset) as *const Fat32DirEntry)
                };
                
                // Check for end of directory
                if entry.name[0] == 0x00 {
                    break;
                }
                
                // Skip deleted entries
                if entry.name[0] == 0xE5 {
                    offset += 32;
                    continue;
                }
                
                // Skip long name entries for now (TODO: implement LFN support)
                if entry.attributes == ATTR_LONG_NAME {
                    offset += 32;
                    continue;
                }
                
                // Skip volume label
                if entry.attributes & ATTR_VOLUME_ID != 0 {
                    offset += 32;
                    continue;
                }
                
                // Parse 8.3 name
                let name = Self::parse_83_name(&entry.name);
                let is_directory = entry.attributes & ATTR_DIRECTORY != 0;
                let cluster = (entry.first_cluster_hi as u32) << 16 | entry.first_cluster_lo as u32;
                
                // Skip . and .. entries
                if name != "." && name != ".." {
                    entries.push(FatDirEntry {
                        name,
                        is_directory,
                        size: if is_directory { 0 } else { entry.file_size },
                        cluster,
                        attributes: entry.attributes,
                    });
                }
                
                offset += 32;
            }
        }
        
        // Cache it
        self.dir_cache.insert(cluster, entries.clone());
        
        Ok(entries)
    }
    
    /// Parse 8.3 filename
    fn parse_83_name(name: &[u8; 11]) -> String {
        // Extract base name (first 8 chars)
        let base = std::str::from_utf8(&name[0..8])
            .unwrap_or("")
            .trim_end();
        
        // Extract extension (last 3 chars)
        let ext = std::str::from_utf8(&name[8..11])
            .unwrap_or("")
            .trim_end();
        
        if ext.is_empty() {
            base.to_string()
        } else {
            format!("{}.{}", base, ext)
        }
    }
    
    /// Read root directory
    pub fn read_root(&mut self) -> Result<Vec<FatDirEntry>, MosesError> {
        self.read_directory(self.root_cluster)
    }
    
    /// Navigate to a path and get directory listing
    pub fn list_directory(&mut self, path: &str) -> Result<Vec<FatDirEntry>, MosesError> {
        if path == "/" || path.is_empty() {
            return self.read_root();
        }
        
        let mut current_cluster = self.root_cluster;
        let components: Vec<&str> = path.split('/')
            .filter(|s| !s.is_empty())
            .collect();
        
        for component in components {
            let entries = self.read_directory(current_cluster)?;
            
            let entry = entries.iter()
                .find(|e| e.name.eq_ignore_ascii_case(component))
                .ok_or_else(|| MosesError::Other(
                    format!("Path component '{}' not found", component)
                ))?;
            
            if !entry.is_directory {
                return Err(MosesError::Other(
                    format!("'{}' is not a directory", component)
                ));
            }
            
            current_cluster = entry.cluster;
        }
        
        self.read_directory(current_cluster)
    }
    
    /// Read a file's contents
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>, MosesError> {
        // Navigate to parent directory
        let (parent_path, file_name) = if let Some(pos) = path.rfind('/') {
            (&path[..pos], &path[pos + 1..])
        } else {
            ("/", path)
        };
        
        let entries = self.list_directory(parent_path)?;
        
        let entry = entries.iter()
            .find(|e| e.name.eq_ignore_ascii_case(file_name))
            .ok_or_else(|| MosesError::Other(format!("File '{}' not found", file_name)))?;
        
        if entry.is_directory {
            return Err(MosesError::Other(format!("'{}' is a directory", file_name)));
        }
        
        // Read the file clusters
        let chain = self.get_cluster_chain(entry.cluster)?;
        let mut file_data = Vec::with_capacity(entry.size as usize);
        let mut bytes_remaining = entry.size as usize;
        
        for cluster_num in chain {
            let cluster_data = self.read_cluster(cluster_num)?;
            
            let bytes_to_copy = std::cmp::min(bytes_remaining, cluster_data.len());
            file_data.extend_from_slice(&cluster_data[..bytes_to_copy]);
            bytes_remaining -= bytes_to_copy;
            
            if bytes_remaining == 0 {
                break;
            }
        }
        
        Ok(file_data)
    }
    
    /// Get filesystem info
    pub fn get_info(&self) -> FsInfo {
        let label = String::from_utf8_lossy(&self.boot_sector.volume_label)
            .trim()
            .to_string();
        
        FsInfo {
            filesystem_type: "FAT32".to_string(),
            label: if label.is_empty() { None } else { Some(label) },
            total_bytes: self.boot_sector.total_sectors_32 as u64 * self.bytes_per_sector as u64,
            cluster_size: self.bytes_per_cluster,
            volume_id: self.boot_sector.volume_id,
        }
    }
}

#[derive(Debug)]
pub struct FsInfo {
    pub filesystem_type: String,
    pub label: Option<String>,
    pub total_bytes: u64,
    pub cluster_size: u32,
    pub volume_id: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_83_name() {
        let name1 = *b"README  TXT";
        assert_eq!(Fat32Reader::parse_83_name(&name1), "README.TXT");
        
        let name2 = *b"FOLDER     ";
        assert_eq!(Fat32Reader::parse_83_name(&name2), "FOLDER");
    }
}