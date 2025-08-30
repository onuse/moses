// FAT16 Writer Module
// Handles file writing, directory creation, and cluster management for FAT16

use moses_core::{Device, MosesError};
use crate::fat_common::{Fat16BootSector, FatDirEntry as Fat16DirEntry};
use std::collections::HashMap;
use std::io::{Read, Write, Seek, SeekFrom};
use std::fs::{File, OpenOptions};
use log::{info, debug};
use chrono::{Datelike, Timelike};

// FAT16 constants
const FAT16_EOC: u16 = 0xFFF8;     // End of cluster chain marker
const FAT16_BAD: u16 = 0xFFF7;     // Bad cluster marker  
const FAT16_FREE: u16 = 0x0000;    // Free cluster marker
const FAT16_MASK: u16 = 0xFFFF;    // Mask for valid FAT16 entries

// Directory entry attributes
const ATTR_READ_ONLY: u8 = 0x01;
const ATTR_HIDDEN: u8 = 0x02;
const ATTR_SYSTEM: u8 = 0x04;
const ATTR_VOLUME_ID: u8 = 0x08;
const ATTR_DIRECTORY: u8 = 0x10;
const ATTR_ARCHIVE: u8 = 0x20;
const ATTR_LONG_NAME: u8 = ATTR_READ_ONLY | ATTR_HIDDEN | ATTR_SYSTEM | ATTR_VOLUME_ID;

type MosesResult<T> = Result<T, MosesError>;

/// FAT16 Writer with cluster allocation and write capabilities
pub struct Fat16Writer {
    device: Device,
    file: File,
    boot_sector: Fat16BootSector,
    
    // Filesystem parameters
    bytes_per_sector: u32,
    sectors_per_cluster: u32,
    bytes_per_cluster: u32,
    fat_start_byte: u64,
    fat_size_bytes: u64,
    root_dir_start_byte: u64,
    root_dir_sectors: u32,
    data_start_byte: u64,
    total_clusters: u32,
    
    // FAT cache for performance
    fat_cache: HashMap<u16, u16>,
    dirty_fat_entries: HashMap<u16, u16>,
    
    // Cluster allocation state
    last_allocated_cluster: u16,
    free_cluster_hint: u16,
}

impl Fat16Writer {
    /// Create a new FAT16 writer
    pub fn new(device: Device) -> MosesResult<Self> {
        info!("Opening FAT16 filesystem for writing on device: {}", device.name);
        
        // Open device for read/write
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&device.mount_points[0])
            .map_err(|e| MosesError::IoError(e))?;
        
        // Read boot sector
        let mut boot_bytes = vec![0u8; 512];
        file.seek(SeekFrom::Start(0))
            .map_err(|e| MosesError::IoError(e))?;
        file.read_exact(&mut boot_bytes)
            .map_err(|e| MosesError::IoError(e))?;
        
        // Verify boot signature
        if boot_bytes[510] != 0x55 || boot_bytes[511] != 0xAA {
            return Err(MosesError::Other("Invalid FAT16 boot signature".into()));
        }
        
        // Parse boot sector
        let boot_sector = unsafe {
            std::ptr::read(boot_bytes.as_ptr() as *const Fat16BootSector)
        };
        
        // Calculate filesystem parameters
        let bytes_per_sector = boot_sector.common_bpb.bytes_per_sector as u32;
        let sectors_per_cluster = boot_sector.common_bpb.sectors_per_cluster as u32;
        let bytes_per_cluster = bytes_per_sector * sectors_per_cluster;
        
        let fat_size_sectors = boot_sector.common_bpb.sectors_per_fat_16 as u32;
        let reserved_sectors = boot_sector.common_bpb.reserved_sectors as u32;
        let num_fats = boot_sector.common_bpb.num_fats as u32;
        let root_entries = boot_sector.common_bpb.root_entries as u32;
        
        let fat_start_byte = reserved_sectors as u64 * bytes_per_sector as u64;
        let fat_size_bytes = fat_size_sectors as u64 * bytes_per_sector as u64;
        
        // Root directory location and size
        let root_dir_sectors = ((root_entries * 32) + (bytes_per_sector - 1)) / bytes_per_sector;
        let root_dir_start_byte = (reserved_sectors + (num_fats * fat_size_sectors)) as u64 * bytes_per_sector as u64;
        
        // Data area starts after root directory
        let data_start_sector = reserved_sectors + (num_fats * fat_size_sectors) + root_dir_sectors;
        let data_start_byte = data_start_sector as u64 * bytes_per_sector as u64;
        
        let total_sectors = if boot_sector.common_bpb.total_sectors_16 != 0 {
            boot_sector.common_bpb.total_sectors_16 as u32
        } else {
            boot_sector.common_bpb.total_sectors_32
        };
        
        let data_sectors = total_sectors - data_start_sector;
        let total_clusters = data_sectors / sectors_per_cluster;
        
        // Verify it's FAT16 (4085 < clusters < 65525)
        if total_clusters <= 4085 || total_clusters >= 65525 {
            return Err(MosesError::Other(format!("Not a FAT16 filesystem (clusters: {})", total_clusters)));
        }
        
        Ok(Self {
            device,
            file,
            boot_sector,
            bytes_per_sector,
            sectors_per_cluster,
            bytes_per_cluster,
            fat_start_byte,
            fat_size_bytes,
            root_dir_start_byte,
            root_dir_sectors,
            data_start_byte,
            total_clusters,
            fat_cache: HashMap::new(),
            dirty_fat_entries: HashMap::new(),
            last_allocated_cluster: 2,
            free_cluster_hint: 2,
        })
    }
    
    /// Get the bytes per cluster value
    pub fn get_bytes_per_cluster(&self) -> u32 {
        self.bytes_per_cluster
    }
    
    /// Get root directory parameters
    pub fn get_root_dir_info(&self) -> (u64, u32) {
        (self.root_dir_start_byte, self.root_dir_sectors)
    }
    
    /// Read a FAT entry
    pub fn read_fat_entry(&mut self, cluster: u16) -> MosesResult<u16> {
        // Check cache first
        if let Some(&entry) = self.dirty_fat_entries.get(&cluster) {
            return Ok(entry);
        }
        if let Some(&entry) = self.fat_cache.get(&cluster) {
            return Ok(entry);
        }
        
        // Read from disk (FAT16 entries are 2 bytes)
        let fat_offset = self.fat_start_byte + (cluster as u64 * 2);
        self.file.seek(SeekFrom::Start(fat_offset))
            .map_err(|e| MosesError::IoError(e))?;
        
        let mut entry_bytes = [0u8; 2];
        self.file.read_exact(&mut entry_bytes)
            .map_err(|e| MosesError::IoError(e))?;
        
        let entry = u16::from_le_bytes(entry_bytes);
        self.fat_cache.insert(cluster, entry);
        
        Ok(entry)
    }
    
    /// Write a FAT entry
    pub fn write_fat_entry(&mut self, cluster: u16, value: u16) -> MosesResult<()> {
        if cluster < 2 || cluster as u32 >= self.total_clusters + 2 {
            return Err(MosesError::Other(format!("Invalid cluster number: {}", cluster)));
        }
        
        // Update cache
        self.dirty_fat_entries.insert(cluster, value);
        self.fat_cache.insert(cluster, value);
        
        Ok(())
    }
    
    /// Flush dirty FAT entries to disk
    pub fn flush_fat(&mut self) -> MosesResult<()> {
        for (&cluster, &value) in &self.dirty_fat_entries {
            // Write to all FAT copies
            for fat_num in 0..self.boot_sector.common_bpb.num_fats {
                let fat_offset = self.fat_start_byte + 
                    (fat_num as u64 * self.fat_size_bytes) + 
                    (cluster as u64 * 2);
                
                self.file.seek(SeekFrom::Start(fat_offset))
                    .map_err(|e| MosesError::IoError(e))?;
                
                let entry_bytes = value.to_le_bytes();
                self.file.write_all(&entry_bytes)
                    .map_err(|e| MosesError::IoError(e))?;
            }
        }
        
        self.file.flush()
            .map_err(|e| MosesError::IoError(e))?;
        
        self.dirty_fat_entries.clear();
        Ok(())
    }
    
    /// Find a free cluster
    pub fn find_free_cluster(&mut self) -> MosesResult<u16> {
        let mut cluster = self.free_cluster_hint;
        let start_cluster = cluster;
        
        loop {
            if cluster >= 2 && (cluster as u32) < self.total_clusters + 2 {
                let entry = self.read_fat_entry(cluster)?;
                if entry == FAT16_FREE {
                    self.free_cluster_hint = cluster + 1;
                    return Ok(cluster);
                }
            }
            
            cluster += 1;
            if cluster as u32 >= self.total_clusters + 2 {
                cluster = 2; // Wrap around
            }
            
            if cluster == start_cluster {
                return Err(MosesError::Other("No free clusters available".into()));
            }
        }
    }
    
    /// Allocate a new cluster
    pub fn allocate_cluster(&mut self) -> MosesResult<u16> {
        let cluster = self.find_free_cluster()?;
        self.write_fat_entry(cluster, FAT16_EOC)?;
        
        // Zero out the cluster data
        self.clear_cluster(cluster)?;
        
        debug!("Allocated cluster {}", cluster);
        Ok(cluster)
    }
    
    /// Allocate a chain of clusters
    pub fn allocate_cluster_chain(&mut self, count: u32) -> MosesResult<Vec<u16>> {
        let mut clusters = Vec::new();
        let mut prev_cluster = 0u16;
        
        for _ in 0..count {
            let cluster = self.allocate_cluster()?;
            clusters.push(cluster);
            
            if prev_cluster != 0 {
                self.write_fat_entry(prev_cluster, cluster)?;
            }
            prev_cluster = cluster;
        }
        
        // Mark the last cluster as end of chain
        if prev_cluster != 0 {
            self.write_fat_entry(prev_cluster, FAT16_EOC)?;
        }
        
        Ok(clusters)
    }
    
    /// Extend a cluster chain
    pub fn extend_cluster_chain(&mut self, last_cluster: u16, additional: u32) -> MosesResult<Vec<u16>> {
        let mut new_clusters = Vec::new();
        let mut current = last_cluster;
        
        for _ in 0..additional {
            let new_cluster = self.allocate_cluster()?;
            new_clusters.push(new_cluster);
            
            // Link the previous cluster to the new one
            self.write_fat_entry(current, new_cluster)?;
            current = new_cluster;
        }
        
        // Mark the last cluster as end of chain
        self.write_fat_entry(current, FAT16_EOC)?;
        
        Ok(new_clusters)
    }
    
    /// Free a cluster chain
    pub fn free_cluster_chain(&mut self, start_cluster: u16) -> MosesResult<()> {
        let mut current = start_cluster;
        
        while current >= 2 && current < 0xFFF6 {
            let next = self.read_fat_entry(current)?;
            self.write_fat_entry(current, FAT16_FREE)?;
            current = next;
        }
        
        Ok(())
    }
    
    /// Clear cluster data (zero it out)
    pub fn clear_cluster(&mut self, cluster: u16) -> MosesResult<()> {
        let offset = self.data_start_byte + 
            ((cluster - 2) as u64 * self.bytes_per_cluster as u64);
        
        self.file.seek(SeekFrom::Start(offset))
            .map_err(|e| MosesError::IoError(e))?;
        
        let zeros = vec![0u8; self.bytes_per_cluster as usize];
        self.file.write_all(&zeros)
            .map_err(|e| MosesError::IoError(e))?;
        
        Ok(())
    }
    
    /// Write data to a cluster
    pub fn write_cluster(&mut self, cluster: u16, data: &[u8]) -> MosesResult<()> {
        if cluster < 2 || cluster as u32 >= self.total_clusters + 2 {
            return Err(MosesError::Other(format!("Invalid cluster number: {}", cluster)));
        }
        
        if data.len() > self.bytes_per_cluster as usize {
            return Err(MosesError::Other("Data exceeds cluster size".into()));
        }
        
        let offset = self.data_start_byte + 
            ((cluster - 2) as u64 * self.bytes_per_cluster as u64);
        
        self.file.seek(SeekFrom::Start(offset))
            .map_err(|e| MosesError::IoError(e))?;
        
        self.file.write_all(data)
            .map_err(|e| MosesError::IoError(e))?;
        
        // Pad with zeros if data is smaller than cluster
        if data.len() < self.bytes_per_cluster as usize {
            let padding = vec![0u8; self.bytes_per_cluster as usize - data.len()];
            self.file.write_all(&padding)
                .map_err(|e| MosesError::IoError(e))?;
        }
        
        Ok(())
    }
    
    /// Read a cluster
    pub fn read_cluster(&mut self, cluster: u16) -> MosesResult<Vec<u8>> {
        if cluster < 2 || cluster as u32 >= self.total_clusters + 2 {
            return Err(MosesError::Other(format!("Invalid cluster number: {}", cluster)));
        }
        
        let offset = self.data_start_byte + 
            ((cluster - 2) as u64 * self.bytes_per_cluster as u64);
        
        self.file.seek(SeekFrom::Start(offset))
            .map_err(|e| MosesError::IoError(e))?;
        
        let mut buffer = vec![0u8; self.bytes_per_cluster as usize];
        self.file.read_exact(&mut buffer)
            .map_err(|e| MosesError::IoError(e))?;
        
        Ok(buffer)
    }
    
    /// Write file data to clusters
    pub fn write_file_data(&mut self, start_cluster: u16, data: &[u8]) -> MosesResult<()> {
        let clusters_needed = (data.len() + self.bytes_per_cluster as usize - 1) / 
                            self.bytes_per_cluster as usize;
        
        // Get existing cluster chain
        let mut clusters = self.get_cluster_chain(start_cluster)?;
        
        // Extend chain if needed
        if clusters.len() < clusters_needed {
            let additional = clusters_needed - clusters.len();
            let last_cluster = *clusters.last().unwrap();
            let new_clusters = self.extend_cluster_chain(last_cluster, additional as u32)?;
            clusters.extend(new_clusters);
        }
        
        // Write data to clusters
        for (i, cluster) in clusters.iter().take(clusters_needed).enumerate() {
            let start = i * self.bytes_per_cluster as usize;
            let end = std::cmp::min(start + self.bytes_per_cluster as usize, data.len());
            self.write_cluster(*cluster, &data[start..end])?;
        }
        
        // Free any extra clusters if the file shrank
        if clusters.len() > clusters_needed {
            // Terminate the chain at the last needed cluster
            self.write_fat_entry(clusters[clusters_needed - 1], FAT16_EOC)?;
            
            // Free the rest
            for &cluster in &clusters[clusters_needed..] {
                self.write_fat_entry(cluster, FAT16_FREE)?;
            }
        }
        
        Ok(())
    }
    
    /// Get cluster chain
    pub fn get_cluster_chain(&mut self, start_cluster: u16) -> MosesResult<Vec<u16>> {
        let mut chain = Vec::new();
        let mut current = start_cluster;
        let mut iterations = 0;
        const MAX_ITERATIONS: u32 = 100000;
        
        while current >= 2 && current < 0xFFF6 {
            if iterations >= MAX_ITERATIONS {
                return Err(MosesError::Other("Cluster chain too long or circular".into()));
            }
            
            chain.push(current);
            current = self.read_fat_entry(current)?;
            iterations += 1;
        }
        
        Ok(chain)
    }
    
    /// Write to root directory (FAT16 has fixed root directory)
    pub fn write_root_dir_entry(&mut self, index: usize, entry: &Fat16DirEntry) -> MosesResult<()> {
        let entry_size = std::mem::size_of::<Fat16DirEntry>();
        let max_entries = self.boot_sector.common_bpb.root_entries as usize;
        
        if index >= max_entries {
            return Err(MosesError::Other("Root directory index out of bounds".into()));
        }
        
        let offset = self.root_dir_start_byte + (index * entry_size) as u64;
        
        self.file.seek(SeekFrom::Start(offset))
            .map_err(|e| MosesError::IoError(e))?;
        
        // Convert entry to bytes
        let entry_bytes = unsafe {
            std::slice::from_raw_parts(
                entry as *const Fat16DirEntry as *const u8,
                entry_size
            )
        };
        
        self.file.write_all(entry_bytes)
            .map_err(|e| MosesError::IoError(e))?;
        
        Ok(())
    }
    
    /// Find a free entry in root directory
    pub fn find_free_root_entry(&mut self) -> MosesResult<usize> {
        let entry_size = std::mem::size_of::<Fat16DirEntry>();
        let max_entries = self.boot_sector.common_bpb.root_entries as usize;
        
        for i in 0..max_entries {
            let offset = self.root_dir_start_byte + (i * entry_size) as u64;
            
            self.file.seek(SeekFrom::Start(offset))
                .map_err(|e| MosesError::IoError(e))?;
            
            let mut first_byte = [0u8; 1];
            self.file.read_exact(&mut first_byte)
                .map_err(|e| MosesError::IoError(e))?;
            
            // Free entry if first byte is 0x00 or 0xE5 (deleted)
            if first_byte[0] == 0x00 || first_byte[0] == 0xE5 {
                return Ok(i);
            }
        }
        
        Err(MosesError::Other("Root directory is full".into()))
    }
    
    /// Create a short (8.3) filename from a long name
    pub fn create_short_name(long_name: &str, existing_names: &[String]) -> String {
        let name = long_name.to_uppercase();
        let (base, ext) = if let Some(dot_pos) = name.rfind('.') {
            (&name[..dot_pos], &name[dot_pos + 1..])
        } else {
            (name.as_str(), "")
        };
        
        // Remove invalid characters and truncate
        let base_clean: String = base.chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            .take(8)
            .collect();
        let ext_clean: String = ext.chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            .take(3)
            .collect();
        
        // Try the simple name first
        let mut short_name = if ext_clean.is_empty() {
            format!("{:8}", base_clean)
        } else {
            format!("{:8}.{:3}", base_clean, ext_clean)
        };
        
        // If it exists, add ~1, ~2, etc.
        if existing_names.iter().any(|n| n.eq_ignore_ascii_case(&short_name)) {
            for i in 1..9999 {
                let base_with_num = format!("{}~{}", 
                    &base_clean[..base_clean.len().min(8 - 2 - i.to_string().len())],
                    i
                );
                short_name = if ext_clean.is_empty() {
                    format!("{:8}", base_with_num)
                } else {
                    format!("{:8}.{:3}", base_with_num, ext_clean)
                };
                
                if !existing_names.iter().any(|n| n.eq_ignore_ascii_case(&short_name)) {
                    break;
                }
            }
        }
        
        short_name
    }
    
    /// Create directory entry
    pub fn create_directory_entry(
        name: &str,
        attributes: u8,
        cluster: u16,
        size: u32,
    ) -> Fat16DirEntry {
        let mut entry = Fat16DirEntry {
            name: [0x20; 11], // Space-padded
            attributes,
            nt_reserved: 0,
            creation_time_tenth: 0,
            creation_time: 0,
            creation_date: 0,
            last_access_date: 0,
            first_cluster_high: 0, // Always 0 for FAT16
            write_time: 0,
            write_date: 0,
            first_cluster_low: cluster,
            file_size: if attributes & ATTR_DIRECTORY != 0 { 0 } else { size },
        };
        
        // Populate the 8.3 filename from the name parameter
        let name_upper = name.to_uppercase();
        let name_bytes = name_upper.as_bytes();
        
        // Parse the name into base and extension
        if let Some(dot_pos) = name_upper.find('.') {
            // Has extension
            let base = &name_bytes[..dot_pos.min(8)];
            let ext = if dot_pos + 1 < name_bytes.len() {
                &name_bytes[dot_pos + 1..name_bytes.len().min(dot_pos + 4)]
            } else {
                &[]
            };
            
            // Copy base name (up to 8 chars)
            for (i, &b) in base.iter().enumerate().take(8) {
                entry.name[i] = b;
            }
            
            // Copy extension (up to 3 chars)
            for (i, &b) in ext.iter().enumerate().take(3) {
                entry.name[8 + i] = b;
            }
        } else {
            // No extension - just copy the name
            for (i, &b) in name_bytes.iter().enumerate().take(11) {
                entry.name[i] = b;
            }
        }
        
        // Set current time
        let now = chrono::Local::now();
        let (date, time) = Self::encode_datetime(&now);
        entry.creation_date = date;
        entry.creation_time = time;
        entry.write_date = date;
        entry.write_time = time;
        entry.last_access_date = date;
        
        entry
    }
    
    /// Encode datetime to FAT format
    fn encode_datetime(dt: &chrono::DateTime<chrono::Local>) -> (u16, u16) {
        let date = ((dt.year() - 1980) as u16) << 9 |
                  (dt.month() as u16) << 5 |
                  dt.day() as u16;
        
        let time = (dt.hour() as u16) << 11 |
                  (dt.minute() as u16) << 5 |
                  (dt.second() as u16 / 2);
        
        (date, time)
    }
    
    /// Flush all pending writes
    pub fn flush(&mut self) -> MosesResult<()> {
        self.flush_fat()?;
        self.file.flush()
            .map_err(|e| MosesError::IoError(e))?;
        Ok(())
    }
}

impl Drop for Fat16Writer {
    fn drop(&mut self) {
        // Best effort to flush on drop
        let _ = self.flush();
    }
}