// exFAT Writer Module
// Handles file writing, directory creation, and cluster management for exFAT

use moses_core::{Device, MosesError};
use crate::exfat::structures::*;
use crate::exfat::bitmap::ExFatBitmap;
use std::collections::HashMap;
use std::io::{Read, Write, Seek, SeekFrom};
use std::fs::{File, OpenOptions};
use log::{info, debug, warn};
use chrono::{DateTime, Utc, Datelike, Timelike};

// exFAT constants
const EXFAT_EOC: u32 = 0xFFFFFFF8;  // End of cluster chain
const EXFAT_BAD: u32 = 0xFFFFFFF7;  // Bad cluster
const EXFAT_FREE: u32 = 0x00000000; // Free cluster

// File attribute flags
const ATTR_READ_ONLY: u16 = 0x0001;
const ATTR_HIDDEN: u16 = 0x0002;
const ATTR_SYSTEM: u16 = 0x0004;
const ATTR_DIRECTORY: u16 = 0x0010;
const ATTR_ARCHIVE: u16 = 0x0020;

type MosesResult<T> = Result<T, MosesError>;

/// exFAT Writer with cluster allocation and write capabilities
pub struct ExFatWriter {
    device: Device,
    file: File,
    boot_sector: ExFatBootSector,
    
    // Filesystem parameters
    bytes_per_sector: u32,
    sectors_per_cluster: u32,
    bytes_per_cluster: u32,
    fat_offset: u64,
    fat_length: u64,
    cluster_heap_offset: u64,
    root_cluster: u32,
    total_clusters: u32,
    
    // FAT cache for performance
    fat_cache: HashMap<u32, u32>,
    dirty_fat_entries: HashMap<u32, u32>,
    
    // Bitmap for cluster allocation
    allocation_bitmap: ExFatBitmap,
    bitmap_modified: bool,
    
    // Cluster allocation state
    last_allocated_cluster: u32,
    free_cluster_hint: u32,
}

impl ExFatWriter {
    /// Create a new exFAT writer
    pub fn new(device: Device) -> MosesResult<Self> {
        info!("Opening exFAT filesystem for writing on device: {}", device.name);
        
        // Open device for read/write
        let mount_point = device.mount_points.get(0)
            .ok_or_else(|| MosesError::Other("No mount point available".into()))?;
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(mount_point)
            .map_err(|e| MosesError::IoError(e))?;
        
        // Read boot sector
        let mut boot_bytes = vec![0u8; 512];
        file.seek(SeekFrom::Start(0))
            .map_err(|e| MosesError::IoError(e))?;
        file.read_exact(&mut boot_bytes)
            .map_err(|e| MosesError::IoError(e))?;
        
        // Verify boot signature
        if boot_bytes[510] != 0x55 || boot_bytes[511] != 0xAA {
            return Err(MosesError::Other("Invalid exFAT boot signature".into()));
        }
        
        // Parse boot sector
        let boot_sector = unsafe {
            std::ptr::read(boot_bytes.as_ptr() as *const ExFatBootSector)
        };
        
        // Verify exFAT signature
        if &boot_sector.file_system_name != b"EXFAT   " {
            return Err(MosesError::Other("Not an exFAT filesystem".into()));
        }
        
        // Calculate filesystem parameters
        let bytes_per_sector = 1u32 << boot_sector.bytes_per_sector_shift;
        let sectors_per_cluster = 1u32 << boot_sector.sectors_per_cluster_shift;
        let bytes_per_cluster = bytes_per_sector * sectors_per_cluster;
        
        let fat_offset = boot_sector.fat_offset as u64 * bytes_per_sector as u64;
        let fat_length = boot_sector.fat_length as u64 * bytes_per_sector as u64;
        let cluster_heap_offset = boot_sector.cluster_heap_offset as u64 * bytes_per_sector as u64;
        let root_cluster = boot_sector.first_cluster_of_root;
        let total_clusters = boot_sector.cluster_count;
        
        // Load allocation bitmap
        // For now, create an empty bitmap - in production, would read from disk
        let allocation_bitmap = ExFatBitmap::new(total_clusters);
        
        Ok(Self {
            device,
            file,
            boot_sector,
            bytes_per_sector,
            sectors_per_cluster,
            bytes_per_cluster,
            fat_offset,
            fat_length,
            cluster_heap_offset,
            root_cluster,
            total_clusters,
            fat_cache: HashMap::new(),
            dirty_fat_entries: HashMap::new(),
            allocation_bitmap,
            bitmap_modified: false,
            last_allocated_cluster: 2,
            free_cluster_hint: 2,
        })
    }
    
    /// Get the bytes per cluster value
    pub fn get_bytes_per_cluster(&self) -> u32 {
        self.bytes_per_cluster
    }
    
    /// Get the root cluster
    pub fn get_root_cluster(&self) -> u32 {
        self.root_cluster
    }
    
    /// Read a FAT entry
    pub fn read_fat_entry(&mut self, cluster: u32) -> MosesResult<u32> {
        // Check cache first
        if let Some(&entry) = self.dirty_fat_entries.get(&cluster) {
            return Ok(entry);
        }
        if let Some(&entry) = self.fat_cache.get(&cluster) {
            return Ok(entry);
        }
        
        // Read from disk (4 bytes per FAT entry in exFAT)
        let fat_entry_offset = self.fat_offset + (cluster as u64 * 4);
        self.file.seek(SeekFrom::Start(fat_entry_offset))
            .map_err(|e| MosesError::IoError(e))?;
        
        let mut entry_bytes = [0u8; 4];
        self.file.read_exact(&mut entry_bytes)
            .map_err(|e| MosesError::IoError(e))?;
        
        let entry = u32::from_le_bytes(entry_bytes);
        self.fat_cache.insert(cluster, entry);
        
        Ok(entry)
    }
    
    /// Write a FAT entry
    pub fn write_fat_entry(&mut self, cluster: u32, value: u32) -> MosesResult<()> {
        if cluster < 2 || cluster >= self.total_clusters + 2 {
            return Err(MosesError::Other(format!("Invalid cluster number: {}", cluster)));
        }
        
        // Update cache
        self.dirty_fat_entries.insert(cluster, value);
        self.fat_cache.insert(cluster, value);
        
        // Update bitmap if allocating/freeing
        if value == EXFAT_FREE {
            self.allocation_bitmap.set_free(cluster - 2);
            self.bitmap_modified = true;
        } else if value != EXFAT_FREE && value != EXFAT_BAD {
            self.allocation_bitmap.set_allocated(cluster - 2);
            self.bitmap_modified = true;
        }
        
        Ok(())
    }
    
    /// Flush dirty FAT entries to disk
    pub fn flush_fat(&mut self) -> MosesResult<()> {
        for (&cluster, &value) in &self.dirty_fat_entries {
            // Write to all FAT copies (exFAT can have 1 or 2 FATs)
            for fat_num in 0..self.boot_sector.number_of_fats {
                let fat_entry_offset = self.fat_offset + 
                    (fat_num as u64 * self.fat_length) + 
                    (cluster as u64 * 4);
                
                self.file.seek(SeekFrom::Start(fat_entry_offset))
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
    
    /// Find a free cluster using the bitmap
    pub fn find_free_cluster(&mut self) -> MosesResult<u32> {
        // Start searching from the hint
        for i in self.free_cluster_hint..self.total_clusters + 2 {
            if i >= 2 && !self.allocation_bitmap.is_allocated(i - 2) {
                self.free_cluster_hint = i + 1;
                return Ok(i);
            }
        }
        
        // Wrap around and search from beginning
        for i in 2..self.free_cluster_hint {
            if !self.allocation_bitmap.is_allocated(i - 2) {
                self.free_cluster_hint = i + 1;
                return Ok(i);
            }
        }
        
        Err(MosesError::Other("No free clusters available".into()))
    }
    
    /// Allocate a new cluster
    pub fn allocate_cluster(&mut self) -> MosesResult<u32> {
        let cluster = self.find_free_cluster()?;
        self.write_fat_entry(cluster, EXFAT_EOC)?;
        
        // Zero out the cluster data
        self.clear_cluster(cluster)?;
        
        debug!("Allocated cluster {}", cluster);
        Ok(cluster)
    }
    
    /// Allocate a chain of clusters
    pub fn allocate_cluster_chain(&mut self, count: u32) -> MosesResult<Vec<u32>> {
        let mut clusters = Vec::new();
        let mut prev_cluster = 0u32;
        
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
            self.write_fat_entry(prev_cluster, EXFAT_EOC)?;
        }
        
        Ok(clusters)
    }
    
    /// Extend a cluster chain
    pub fn extend_cluster_chain(&mut self, last_cluster: u32, additional: u32) -> MosesResult<Vec<u32>> {
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
        self.write_fat_entry(current, EXFAT_EOC)?;
        
        Ok(new_clusters)
    }
    
    /// Free a cluster chain
    pub fn free_cluster_chain(&mut self, start_cluster: u32) -> MosesResult<()> {
        let mut current = start_cluster;
        
        while current >= 2 && current < 0xFFFFFFF6 {
            let next = self.read_fat_entry(current)?;
            self.write_fat_entry(current, EXFAT_FREE)?;
            current = next;
        }
        
        Ok(())
    }
    
    /// Clear cluster data (zero it out)
    pub fn clear_cluster(&mut self, cluster: u32) -> MosesResult<()> {
        let offset = self.cluster_heap_offset + 
            ((cluster - 2) as u64 * self.bytes_per_cluster as u64);
        
        self.file.seek(SeekFrom::Start(offset))
            .map_err(|e| MosesError::IoError(e))?;
        
        let zeros = vec![0u8; self.bytes_per_cluster as usize];
        self.file.write_all(&zeros)
            .map_err(|e| MosesError::IoError(e))?;
        
        Ok(())
    }
    
    /// Write data to a cluster
    pub fn write_cluster(&mut self, cluster: u32, data: &[u8]) -> MosesResult<()> {
        if cluster < 2 || cluster >= self.total_clusters + 2 {
            return Err(MosesError::Other(format!("Invalid cluster number: {}", cluster)));
        }
        
        if data.len() > self.bytes_per_cluster as usize {
            return Err(MosesError::Other("Data exceeds cluster size".into()));
        }
        
        let offset = self.cluster_heap_offset + 
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
    pub fn read_cluster(&mut self, cluster: u32) -> MosesResult<Vec<u8>> {
        if cluster < 2 || cluster >= self.total_clusters + 2 {
            return Err(MosesError::Other(format!("Invalid cluster number: {}", cluster)));
        }
        
        let offset = self.cluster_heap_offset + 
            ((cluster - 2) as u64 * self.bytes_per_cluster as u64);
        
        self.file.seek(SeekFrom::Start(offset))
            .map_err(|e| MosesError::IoError(e))?;
        
        let mut buffer = vec![0u8; self.bytes_per_cluster as usize];
        self.file.read_exact(&mut buffer)
            .map_err(|e| MosesError::IoError(e))?;
        
        Ok(buffer)
    }
    
    /// Write file data to clusters
    pub fn write_file_data(&mut self, start_cluster: u32, data: &[u8]) -> MosesResult<()> {
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
            self.write_fat_entry(clusters[clusters_needed - 1], EXFAT_EOC)?;
            
            // Free the rest
            for &cluster in &clusters[clusters_needed..] {
                self.write_fat_entry(cluster, EXFAT_FREE)?;
            }
        }
        
        Ok(())
    }
    
    /// Get cluster chain
    pub fn get_cluster_chain(&mut self, start_cluster: u32) -> MosesResult<Vec<u32>> {
        let mut chain = Vec::new();
        let mut current = start_cluster;
        let mut iterations = 0;
        const MAX_ITERATIONS: u32 = 1000000;
        
        while current >= 2 && current < 0xFFFFFFF6 {
            if iterations >= MAX_ITERATIONS {
                return Err(MosesError::Other("Cluster chain too long or circular".into()));
            }
            
            chain.push(current);
            current = self.read_fat_entry(current)?;
            iterations += 1;
        }
        
        Ok(chain)
    }
    
    /// Create a file directory entry set
    pub fn create_file_entry_set(
        name: &str,
        attributes: u16,
        first_cluster: u32,
        data_length: u64,
    ) -> Vec<ExFatDirectoryEntry> {
        let mut entries = Vec::new();
        
        // Calculate number of name entries needed
        let name_chars: Vec<u16> = name.encode_utf16().collect();
        let name_entries_needed = (name_chars.len() + 14) / 15; // 15 chars per entry
        
        // Create file entry
        let mut file_entry = ExFatDirectoryEntry::default();
        unsafe {
            file_entry.file = ExFatFileEntry {
                entry_type: EXFAT_ENTRY_FILE,
                secondary_count: (1 + name_entries_needed) as u8,
                set_checksum: 0, // Will calculate later
                file_attributes: attributes,
                reserved1: 0,
                create_timestamp: Self::encode_timestamp(&Utc::now()),
                last_modified_timestamp: Self::encode_timestamp(&Utc::now()),
                last_accessed_timestamp: Self::encode_timestamp(&Utc::now()),
                create_10ms_increment: 0,
                last_modified_10ms_increment: 0,
                create_tz_offset: 0,
                last_modified_tz_offset: 0,
                last_accessed_tz_offset: 0,
                reserved2: [0; 7],
            };
        }
        entries.push(file_entry);
        
        // Create stream extension entry
        let mut stream_entry = ExFatDirectoryEntry::default();
        unsafe {
            stream_entry.stream = ExFatStreamEntry {
                entry_type: EXFAT_ENTRY_STREAM,
                flags: 0x01, // Allocation possible
                reserved1: 0,
                name_length: name_chars.len() as u8,
                name_hash: Self::calculate_name_hash(&name_chars),
                reserved2: 0,
                valid_data_length: data_length,
                reserved3: 0,
                first_cluster,
                data_length,
            };
        }
        entries.push(stream_entry);
        
        // Create file name entries
        for i in 0..name_entries_needed {
            let mut name_entry = ExFatDirectoryEntry::default();
            unsafe {
                name_entry.filename.entry_type = EXFAT_ENTRY_FILE_NAME;
                name_entry.filename.flags = 0;
                
                // Copy name characters (15 per entry)
                let start = i * 15;
                let end = std::cmp::min(start + 15, name_chars.len());
                for j in 0..(end - start) {
                    name_entry.filename.file_name[j] = name_chars[start + j];
                }
                
                // Pad with 0xFFFF if necessary
                for j in (end - start)..15 {
                    name_entry.filename.file_name[j] = 0xFFFF;
                }
            }
            entries.push(name_entry);
        }
        
        // Calculate and set checksum
        let checksum = Self::calculate_entry_set_checksum(&entries);
        unsafe {
            entries[0].file.set_checksum = checksum;
        }
        
        entries
    }
    
    /// Calculate name hash for exFAT
    fn calculate_name_hash(name_chars: &[u16]) -> u16 {
        let mut hash = 0u16;
        for &ch in name_chars {
            hash = ((hash << 15) | (hash >> 1)) + (ch & 0xFF) as u16;
            hash = ((hash << 15) | (hash >> 1)) + ((ch >> 8) & 0xFF) as u16;
        }
        hash
    }
    
    /// Calculate entry set checksum
    fn calculate_entry_set_checksum(entries: &[ExFatDirectoryEntry]) -> u16 {
        let mut checksum = 0u16;
        
        for entry in entries {
            let bytes = entry.to_bytes();
            for (i, &byte) in bytes.iter().enumerate() {
                // Skip checksum bytes in primary entry (2-3)
                if entry.entry_type() == EXFAT_ENTRY_FILE && (i == 2 || i == 3) {
                    continue;
                }
                checksum = ((checksum << 15) | (checksum >> 1)) + byte as u16;
            }
        }
        
        checksum
    }
    
    /// Encode datetime to exFAT timestamp format
    fn encode_timestamp(dt: &DateTime<Utc>) -> u32 {
        let year = dt.year() - 1980;
        let month = dt.month();
        let day = dt.day();
        let hour = dt.hour();
        let minute = dt.minute();
        let second = dt.second() / 2; // exFAT uses 2-second resolution
        
        ((year as u32) << 25) |
        ((month as u32) << 21) |
        ((day as u32) << 16) |
        ((hour as u32) << 11) |
        ((minute as u32) << 5) |
        (second as u32)
    }
    
    /// Write directory entries to a cluster
    pub fn write_directory_entries(
        &mut self,
        dir_cluster: u32,
        entries: &[ExFatDirectoryEntry],
    ) -> MosesResult<()> {
        // Read the directory cluster
        let mut dir_data = self.read_cluster(dir_cluster)?;
        
        // Find free space (entries starting with 0x00 or 0x05)
        let entry_size = 32;
        let entries_per_cluster = self.bytes_per_cluster as usize / entry_size;
        let entries_needed = entries.len();
        
        // Find contiguous free entries
        let mut free_start = None;
        let mut free_count = 0;
        
        for i in 0..entries_per_cluster {
            let offset = i * entry_size;
            if dir_data[offset] == 0x00 || dir_data[offset] == 0x05 {
                if free_start.is_none() {
                    free_start = Some(i);
                }
                free_count += 1;
                
                if free_count >= entries_needed {
                    break;
                }
            } else {
                free_start = None;
                free_count = 0;
            }
        }
        
        if free_count < entries_needed {
            return Err(MosesError::Other("Not enough space in directory".into()));
        }
        
        // Write entries
        let start_index = free_start.unwrap();
        for (i, entry) in entries.iter().enumerate() {
            let offset = (start_index + i) * entry_size;
            let entry_bytes = entry.to_bytes();
            dir_data[offset..offset + entry_size].copy_from_slice(&entry_bytes);
        }
        
        // Write back the cluster
        self.write_cluster(dir_cluster, &dir_data)?;
        
        Ok(())
    }
    
    /// Flush all pending writes
    pub fn flush(&mut self) -> MosesResult<()> {
        self.flush_fat()?;
        
        // TODO: Write bitmap back to disk if modified
        if self.bitmap_modified {
            warn!("Bitmap write-back not yet implemented");
        }
        
        self.file.flush()
            .map_err(|e| MosesError::IoError(e))?;
        Ok(())
    }
}

impl Drop for ExFatWriter {
    fn drop(&mut self) {
        // Best effort to flush on drop
        let _ = self.flush();
    }
}