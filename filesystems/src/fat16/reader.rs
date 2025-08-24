// FAT16 filesystem reader

use moses_core::{Device, MosesError};
use crate::device_reader::{AlignedDeviceReader, FilesystemReader, FileEntry, FilesystemInfo, FileMetadata};
use crate::fat_common::{Fat16BootSector, FatDirEntry, FatAttributes};
use log::{info, debug};
use std::collections::HashMap;

// Helper constants for FAT16 reader
const ATTR_LONG_NAME: u8 = FatAttributes::READ_ONLY | FatAttributes::HIDDEN | 
                           FatAttributes::SYSTEM | FatAttributes::VOLUME_ID;

pub struct Fat16Reader {
    _device: Device,
    reader: AlignedDeviceReader,
    _boot_sector: Fat16BootSector,
    
    // Filesystem parameters
    bytes_per_sector: u32,
    sectors_per_cluster: u32,
    bytes_per_cluster: u32,
    reserved_sectors: u32,
    fat_sectors: u32,
    root_dir_sectors: u32,
    first_data_sector: u32,
    total_clusters: u32,
    
    // Cache
    fat_cache: HashMap<u16, u16>,
    dir_cache: HashMap<String, Vec<FileEntry>>,
}

impl Fat16Reader {
    pub fn new(device: Device) -> Result<Self, MosesError> {
        use crate::utils::open_device_with_fallback;
        
        info!("Opening FAT16 filesystem on device: {}", device.name);
        
        // Open device
        let file = open_device_with_fallback(&device)?;
        let mut reader = AlignedDeviceReader::new(file);
        
        // Read boot sector
        let boot_data = reader.read_at(0, 512)?;
        let boot_sector = unsafe {
            std::ptr::read_unaligned(boot_data.as_ptr() as *const Fat16BootSector)
        };
        
        // Validate FAT16
        let fs_type = String::from_utf8_lossy(&boot_sector.extended_bpb.fs_type);
        if !fs_type.starts_with("FAT16") && !fs_type.starts_with("FAT") {
            return Err(MosesError::Other("Not a FAT16 filesystem".to_string()));
        }
        
        // Extract parameters (copy to avoid unaligned access)
        let bytes_per_sector = boot_sector.common_bpb.bytes_per_sector as u32;
        let sectors_per_cluster = boot_sector.common_bpb.sectors_per_cluster as u32;
        let bytes_per_cluster = bytes_per_sector * sectors_per_cluster;
        let reserved_sectors = boot_sector.common_bpb.reserved_sectors as u32;
        let num_fats = boot_sector.common_bpb.num_fats as u32;
        let root_entries = boot_sector.common_bpb.root_entries as u32;
        let sectors_per_fat = boot_sector.common_bpb.sectors_per_fat_16 as u32;
        
        // Calculate layout
        let root_dir_sectors = ((root_entries * 32) + (bytes_per_sector - 1)) / bytes_per_sector;
        let fat_sectors = num_fats * sectors_per_fat;
        let first_data_sector = reserved_sectors + fat_sectors + root_dir_sectors;
        
        // Calculate total sectors
        let total_sectors = if boot_sector.common_bpb.total_sectors_16 != 0 {
            boot_sector.common_bpb.total_sectors_16 as u32
        } else {
            boot_sector.common_bpb.total_sectors_32
        };
        
        let data_sectors = total_sectors - first_data_sector;
        let total_clusters = data_sectors / sectors_per_cluster;
        
        info!("FAT16 filesystem details:");
        info!("  Bytes per sector: {}", bytes_per_sector);
        info!("  Sectors per cluster: {}", sectors_per_cluster);
        info!("  Root entries: {}", root_entries);
        info!("  First data sector: {}", first_data_sector);
        info!("  Total clusters: {}", total_clusters);
        
        Ok(Self {
            _device: device,
            reader,
            _boot_sector: boot_sector,
            bytes_per_sector,
            sectors_per_cluster,
            bytes_per_cluster,
            reserved_sectors,
            fat_sectors,
            root_dir_sectors,
            first_data_sector,
            total_clusters,
            fat_cache: HashMap::new(),
            dir_cache: HashMap::new(),
        })
    }
    
    /// Get next cluster from FAT
    fn get_next_cluster(&mut self, cluster: u16) -> Result<Option<u16>, MosesError> {
        // Check cache
        if let Some(&next) = self.fat_cache.get(&cluster) {
            return Ok(if next >= 0xFFF8 { None } else { Some(next) });
        }
        
        // Read FAT entry (2 bytes per entry in FAT16)
        let fat_offset = self.reserved_sectors * self.bytes_per_sector + (cluster as u32 * 2);
        let entry_data = self.reader.read_at(fat_offset as u64, 2)?;
        let next_cluster = u16::from_le_bytes([entry_data[0], entry_data[1]]);
        
        // Cache it
        self.fat_cache.insert(cluster, next_cluster);
        
        // Check for end of chain (0xFFF8 - 0xFFFF)
        Ok(if next_cluster >= 0xFFF8 { None } else { Some(next_cluster) })
    }
    
    /// Read a cluster
    fn read_cluster(&mut self, cluster: u16) -> Result<Vec<u8>, MosesError> {
        if cluster < 2 || cluster as u32 >= self.total_clusters + 2 {
            return Err(MosesError::Other(format!("Invalid cluster: {}", cluster)));
        }
        
        let sector = self.first_data_sector + ((cluster - 2) as u32 * self.sectors_per_cluster);
        let offset = sector as u64 * self.bytes_per_sector as u64;
        
        self.reader.read_at(offset, self.bytes_per_cluster as usize)
    }
    
    /// Read cluster chain
    fn read_cluster_chain(&mut self, first_cluster: u16) -> Result<Vec<u8>, MosesError> {
        if first_cluster == 0 {
            return Ok(Vec::new());
        }
        
        let mut data = Vec::new();
        let mut current = first_cluster;
        
        loop {
            let cluster_data = self.read_cluster(current)?;
            data.extend_from_slice(&cluster_data);
            
            match self.get_next_cluster(current)? {
                Some(next) => current = next,
                None => break,
            }
        }
        
        Ok(data)
    }
    
    /// Parse short filename from FAT directory entry
    fn parse_short_name(entry: &FatDirEntry) -> String {
        let mut name = String::new();
        
        // The name field in FatDirEntry is 11 bytes (8.3 format)
        // Parse main name (first 8 chars)
        for i in 0..8 {
            let b = entry.name[i];
            if b == 0x20 || b == 0x00 {
                break;
            }
            if b == 0x05 {
                name.push(0xE5 as char); // Special case
            } else {
                name.push(b as char);
            }
        }
        
        // Parse extension (last 3 chars)
        let mut has_ext = false;
        for i in 8..11 {
            let b = entry.name[i];
            if b != 0x20 && b != 0x00 {
                if !has_ext {
                    name.push('.');
                    has_ext = true;
                }
                name.push(b as char);
            }
        }
        
        name
    }
    
    /// Parse directory entries
    fn parse_directory(&self, data: &[u8]) -> Vec<FileEntry> {
        let mut entries = Vec::new();
        let mut i = 0;
        
        while i + 32 <= data.len() {
            let entry_bytes = &data[i..i + 32];
            
            // Check for end of directory
            if entry_bytes[0] == 0x00 {
                break;
            }
            
            // Skip deleted entries
            if entry_bytes[0] == 0xE5 {
                i += 32;
                continue;
            }
            
            let entry = unsafe {
                std::ptr::read_unaligned(entry_bytes.as_ptr() as *const FatDirEntry)
            };
            
            // Skip long name entries and volume labels
            if entry.attributes & ATTR_LONG_NAME == ATTR_LONG_NAME {
                i += 32;
                continue;
            }
            if entry.attributes & FatAttributes::VOLUME_ID != 0 {
                i += 32;
                continue;
            }
            
            let name = Self::parse_short_name(&entry);
            
            // Skip . and .. entries
            if name == "." || name == ".." {
                i += 32;
                continue;
            }
            
            entries.push(FileEntry {
                name,
                is_directory: entry.attributes & FatAttributes::DIRECTORY != 0,
                size: if entry.attributes & FatAttributes::DIRECTORY != 0 { 0 } else { entry.file_size as u64 },
                cluster: Some(entry.first_cluster() as u32),
                metadata: FileMetadata::default(),
            });
            
            i += 32;
        }
        
        entries
    }
    
    /// Read root directory
    fn read_root_directory(&mut self) -> Result<Vec<FileEntry>, MosesError> {
        // Root directory is at a fixed location in FAT16
        let root_offset = (self.reserved_sectors + self.fat_sectors) * self.bytes_per_sector;
        let root_size = self.root_dir_sectors * self.bytes_per_sector;
        
        debug!("Reading root directory at offset {:#x}, size: {}", root_offset, root_size);
        
        let data = self.reader.read_at(root_offset as u64, root_size as usize)?;
        Ok(self.parse_directory(&data))
    }
    
    /// Read a subdirectory
    fn read_subdirectory(&mut self, cluster: u16) -> Result<Vec<FileEntry>, MosesError> {
        let data = self.read_cluster_chain(cluster)?;
        Ok(self.parse_directory(&data))
    }
}

impl FilesystemReader for Fat16Reader {
    fn read_metadata(&mut self) -> Result<(), MosesError> {
        // Already read in new()
        Ok(())
    }
    
    fn list_directory(&mut self, path: &str) -> Result<Vec<FileEntry>, MosesError> {
        // Check cache
        if let Some(cached) = self.dir_cache.get(path) {
            return Ok(cached.clone());
        }
        
        let entries = if path == "/" || path.is_empty() {
            self.read_root_directory()?
        } else {
            // Navigate to directory
            let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            let mut current_entries = self.read_root_directory()?;
            
            for part in parts {
                let dir = current_entries.iter()
                    .find(|e| e.name.eq_ignore_ascii_case(part) && e.is_directory)
                    .ok_or_else(|| MosesError::Other(format!("Directory not found: {}", part)))?;
                
                let current_cluster = dir.cluster.unwrap_or(0) as u16;
                if current_cluster == 0 {
                    return Err(MosesError::Other("Invalid directory cluster".to_string()));
                }
                
                current_entries = self.read_subdirectory(current_cluster)?;
            }
            
            current_entries
        };
        
        // Cache result
        self.dir_cache.insert(path.to_string(), entries.clone());
        Ok(entries)
    }
    
    fn read_file(&mut self, path: &str) -> Result<Vec<u8>, MosesError> {
        // Parse path
        let (dir_path, file_name) = if let Some(pos) = path.rfind('/') {
            (&path[..pos], &path[pos + 1..])
        } else {
            ("", path)
        };
        
        // Read directory
        let entries = self.list_directory(dir_path)?;
        
        // Find file
        let file = entries.iter()
            .find(|e| e.name.eq_ignore_ascii_case(file_name) && !e.is_directory)
            .ok_or_else(|| MosesError::Other(format!("File not found: {}", file_name)))?;
        
        // Read file data
        if let Some(cluster) = file.cluster {
            if cluster == 0 {
                Ok(Vec::new())
            } else {
                let mut data = self.read_cluster_chain(cluster as u16)?;
                data.truncate(file.size as usize);
                Ok(data)
            }
        } else {
            Ok(Vec::new())
        }
    }
    
    fn get_info(&self) -> FilesystemInfo {
        let total_bytes = self.total_clusters as u64 * self.bytes_per_cluster as u64;
        
        FilesystemInfo {
            fs_type: "FAT16".to_string(),
            label: None, // Would need to read volume label
            total_bytes,
            used_bytes: 0, // Would need to scan FAT
            cluster_size: Some(self.bytes_per_cluster),
        }
    }
}