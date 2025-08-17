// Improved exFAT reader that keeps the device file handle open

use moses_core::{Device, MosesError};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::collections::HashMap;
use log::{info, debug};

use super::reader::{
    ExFatBootSector, FileDirectoryEntry, StreamExtensionEntry, 
    FileNameEntry, ExFatFile, EXFAT_SIGNATURE
};

/// Improved exFAT filesystem reader with persistent file handle
pub struct ExFatReaderImproved {
    device: Device,
    boot_sector: ExFatBootSector,
    bytes_per_cluster: u32,
    cluster_heap_offset: u64,
    fat_offset: u64,
    
    // Keep file handle open
    file_handle: File,
    
    // Cache
    fat_cache: HashMap<u32, u32>,
    dir_cache: HashMap<String, Vec<ExFatFile>>,
}

impl ExFatReaderImproved {
    /// Open an exFAT filesystem for reading
    pub fn new(mut device: Device) -> Result<Self, MosesError> {
        use crate::utils::{open_device_with_fallback, get_device_path};
        
        info!("Opening exFAT filesystem on device: {}", device.name);
        
        // For exFAT, we might need to use the physical disk path instead of the volume path
        // because we're reading at specific offsets that are relative to the disk, not the volume
        let original_id = device.id.clone();
        
        // Try with the device as-is first
        let path = get_device_path(&device);
        info!("Opening device at path: {}", path);
        
        let mut file_handle = match open_device_with_fallback(&device) {
            Ok(handle) => handle,
            Err(e) => {
                // If that fails and we were using a volume path, try the physical disk
                info!("Failed with volume path, trying physical disk: {}", original_id);
                device.mount_points.clear(); // Force use of physical disk path
                device.id = original_id; // Ensure we use the physical disk ID
                open_device_with_fallback(&device)?
            }
        };
        
        // Read boot sector
        let boot_sector = Self::read_boot_sector_from_handle(&mut file_handle)?;
        
        // Validate signature
        if boot_sector.fs_name != EXFAT_SIGNATURE {
            return Err(MosesError::Other("Not an exFAT filesystem".to_string()));
        }
        
        // Calculate parameters
        let bytes_per_sector = 1u32 << boot_sector.bytes_per_sector_shift;
        let sectors_per_cluster = 1u32 << boot_sector.sectors_per_cluster_shift;
        let bytes_per_cluster = bytes_per_sector * sectors_per_cluster;
        
        // In exFAT, offsets are relative to the partition start
        // When using a volume handle (\\.\E:), we're already at the partition start
        // When using a physical disk handle, we need to add the partition offset
        let partition_offset = boot_sector.partition_offset;
        let partition_offset_bytes = partition_offset * bytes_per_sector as u64;
        info!("Partition offset: {} sectors = {} bytes", partition_offset, partition_offset_bytes);
        
        // Check if we're using a volume handle (drive letter) or physical disk
        // Volume handles start at the partition, physical disks start at sector 0
        let using_volume_handle = device.mount_points.iter()
            .any(|p| {
                let s = p.to_string_lossy();
                s.len() >= 2 && s.chars().nth(1) == Some(':')
            });
        
        info!("Using {} handle", if using_volume_handle { "volume" } else { "physical disk" });
        
        // Copy values to avoid unaligned access
        let fat_offset_sectors = boot_sector.fat_offset;
        let cluster_heap_offset_sectors = boot_sector.cluster_heap_offset;
        let fat_length = boot_sector.fat_length;
        
        info!("Raw boot sector values:");
        info!("  Partition offset: {} sectors ({:#x} bytes)", partition_offset, partition_offset_bytes);
        info!("  FAT offset: {} sectors ({:#x} bytes)", fat_offset_sectors, fat_offset_sectors as u64 * bytes_per_sector as u64);
        info!("  FAT length: {} sectors", fat_length);
        info!("  Cluster heap offset: {} sectors ({:#x} bytes)", cluster_heap_offset_sectors, cluster_heap_offset_sectors as u64 * bytes_per_sector as u64);
        
        // According to Microsoft exFAT specification, FatOffset and ClusterHeapOffset
        // are "volume-relative sector offsets" - meaning relative to the partition start
        
        // When using a volume handle (\\.\E:), we're at the partition start, so use offsets directly
        // When using a physical disk handle, we need to add the partition offset
        
        let (fat_offset, cluster_heap_offset) = if using_volume_handle {
            // Volume handle: we're at partition start, use offsets directly
            info!("Using volume-relative offsets directly for volume handle");
            let fat = fat_offset_sectors as u64 * bytes_per_sector as u64;
            let cluster_heap = cluster_heap_offset_sectors as u64 * bytes_per_sector as u64;
            
            // Sanity check: FAT should be at least 24 sectors from partition start (after boot sectors)
            if fat_offset_sectors < 24 {
                info!("WARNING: FAT offset {} sectors seems too small, minimum expected is 24", fat_offset_sectors);
            }
            
            (fat, cluster_heap)
        } else {
            // Physical disk: need to add partition offset to get absolute disk position
            info!("Adding partition offset for physical disk handle");
            let fat = partition_offset_bytes + (fat_offset_sectors as u64 * bytes_per_sector as u64);
            let cluster_heap = partition_offset_bytes + (cluster_heap_offset_sectors as u64 * bytes_per_sector as u64);
            (fat, cluster_heap)
        };
        
        info!("Final adjusted offsets:");
        info!("  FAT offset: {:#x}", fat_offset);
        info!("  Cluster heap offset: {:#x}", cluster_heap_offset);
        
        // Copy values to avoid unaligned access
        let cluster_count = boot_sector.cluster_count;
        let first_cluster_of_root = boot_sector.first_cluster_of_root;
        
        info!("exFAT filesystem details:");
        info!("  Bytes per sector: {}", bytes_per_sector);
        info!("  Sectors per cluster: {}", sectors_per_cluster);
        info!("  Cluster count: {}", cluster_count);
        info!("  Root directory cluster: {}", first_cluster_of_root);
        
        Ok(ExFatReaderImproved {
            device,
            boot_sector,
            bytes_per_cluster,
            cluster_heap_offset,
            fat_offset,
            file_handle,
            fat_cache: HashMap::new(),
            dir_cache: HashMap::new(),
        })
    }
    
    /// Read boot sector from an open file handle
    fn read_boot_sector_from_handle(file: &mut File) -> Result<ExFatBootSector, MosesError> {
        use crate::utils::read_sector;
        
        // Ensure we're at the beginning
        file.seek(SeekFrom::Start(0))
            .map_err(|e| MosesError::Other(format!("Failed to seek to boot sector: {}", e)))?;
        
        let buffer = read_sector(file, 0)?;
        
        let boot_sector = unsafe {
            std::ptr::read_unaligned(buffer.as_ptr() as *const ExFatBootSector)
        };
        
        Ok(boot_sector)
    }
    
    /// Read a cluster by number (using the persistent file handle)
    fn read_cluster(&mut self, cluster_num: u32) -> Result<Vec<u8>, MosesError> {
        if cluster_num < 2 || cluster_num >= self.boot_sector.cluster_count + 2 {
            return Err(MosesError::Other(format!("Invalid cluster number: {}", cluster_num)));
        }
        
        let offset = self.cluster_heap_offset + 
                    ((cluster_num - 2) as u64 * self.bytes_per_cluster as u64);
        
        debug!("Reading cluster {} at offset {:#x}, size: {} bytes", 
               cluster_num, offset, self.bytes_per_cluster);
        
        // Seek to the cluster
        debug!("Seeking to offset {:#x} for cluster {}", offset, cluster_num);
        if let Err(e) = self.file_handle.seek(SeekFrom::Start(offset)) {
            let os_error = e.raw_os_error().unwrap_or(-1);
            let error_msg = format!(
                "Failed to seek to cluster {} at offset {:#x}: {} (OS error: {})", 
                cluster_num, offset, e, os_error
            );
            info!("ERROR: {}", error_msg);
            return Err(MosesError::Other(error_msg));
        }
        debug!("Successfully seeked to offset {:#x}", offset);
        
        // Read the cluster data
        // For large clusters, we might need to read in chunks
        let cluster_size = self.bytes_per_cluster as usize;
        let mut buffer = vec![0u8; cluster_size];
        
        // Windows may have limits on device reads, so read in chunks if needed
        const MAX_READ_SIZE: usize = 65536; // 64KB chunks
        
        if cluster_size <= MAX_READ_SIZE {
            // Read in one go
            if let Err(e) = self.file_handle.read_exact(&mut buffer) {
                let os_error = e.raw_os_error().unwrap_or(-1);
                return Err(MosesError::Other(format!(
                    "Failed to read cluster {} ({} bytes): {} (OS error: {})", 
                    cluster_num, cluster_size, e, os_error
                )));
            }
        } else {
            // Read in chunks
            let num_chunks = (cluster_size + MAX_READ_SIZE - 1) / MAX_READ_SIZE;
            debug!("Reading large cluster in {} chunks", num_chunks);
            let mut bytes_read = 0;
            for chunk_idx in 0..num_chunks {
                let chunk_size = (cluster_size - bytes_read).min(MAX_READ_SIZE);
                debug!("Reading chunk {}/{}: {} bytes at buffer offset {}", 
                       chunk_idx + 1, num_chunks, chunk_size, bytes_read);
                
                match self.file_handle.read_exact(&mut buffer[bytes_read..bytes_read + chunk_size]) {
                    Ok(_) => {
                        debug!("Successfully read chunk {}/{}", chunk_idx + 1, num_chunks);
                    }
                    Err(e) => {
                        let os_error = e.raw_os_error().unwrap_or(-1);
                        let error_msg = format!(
                            "Failed to read cluster {} chunk {}/{} at buffer offset {} ({} bytes): {} (OS error: {})", 
                            cluster_num, chunk_idx + 1, num_chunks, bytes_read, chunk_size, e, os_error
                        );
                        info!("ERROR: {}", error_msg);
                        return Err(MosesError::Other(error_msg));
                    }
                }
                bytes_read += chunk_size;
            }
        }
        
        Ok(buffer)
    }
    
    /// Get next cluster from FAT (using the persistent file handle)
    fn get_next_cluster(&mut self, cluster: u32) -> Result<Option<u32>, MosesError> {
        debug!("Getting next cluster for cluster {}", cluster);
        
        // Check cache first
        if let Some(&next) = self.fat_cache.get(&cluster) {
            debug!("Found cluster {} -> {} in cache", cluster, next);
            return Ok(if next >= 0xFFFFFFF8 { None } else { Some(next) });
        }
        
        // Read FAT in larger chunks to avoid small offset issues on Windows
        // Read 1 sector (512 bytes) at a time, which contains 128 FAT entries
        const SECTOR_SIZE: u64 = 512;
        const ENTRIES_PER_SECTOR: u32 = 128; // 512 / 4
        
        // Calculate which sector contains this cluster's FAT entry
        let fat_sector = cluster / ENTRIES_PER_SECTOR;
        let entry_in_sector = cluster % ENTRIES_PER_SECTOR;
        let sector_offset = self.fat_offset + (fat_sector as u64 * SECTOR_SIZE);
        
        debug!("Reading FAT sector {} at offset {:#x} for cluster {} (entry {} in sector)", 
               fat_sector, sector_offset, cluster, entry_in_sector);
        
        // Seek to the FAT sector
        if let Err(e) = self.file_handle.seek(SeekFrom::Start(sector_offset)) {
            let os_error = e.raw_os_error().unwrap_or(-1);
            let error_msg = format!(
                "Failed to seek to FAT sector at offset {:#x}: {} (OS error: {})", 
                sector_offset, e, os_error
            );
            info!("ERROR in get_next_cluster: {}", error_msg);
            return Err(MosesError::Other(error_msg));
        }
        
        // Read the entire sector
        let mut sector_buffer = vec![0u8; SECTOR_SIZE as usize];
        if let Err(e) = self.file_handle.read_exact(&mut sector_buffer) {
            let os_error = e.raw_os_error().unwrap_or(-1);
            let error_msg = format!(
                "Failed to read FAT sector at offset {:#x}: {} (OS error: {})", 
                sector_offset, e, os_error
            );
            info!("ERROR in get_next_cluster: {}", error_msg);
            return Err(MosesError::Other(error_msg));
        }
        
        // Cache all entries in this sector for efficiency
        for i in 0..ENTRIES_PER_SECTOR {
            let offset = (i * 4) as usize;
            let entry_cluster = fat_sector * ENTRIES_PER_SECTOR + i;
            let value = u32::from_le_bytes([
                sector_buffer[offset],
                sector_buffer[offset + 1],
                sector_buffer[offset + 2],
                sector_buffer[offset + 3],
            ]);
            self.fat_cache.insert(entry_cluster, value);
        }
        
        // Now get the requested entry from cache
        let next_cluster = self.fat_cache[&cluster];
        debug!("FAT entry: cluster {} -> {:#x}", cluster, next_cluster);
        
        // Check for end of chain (0xFFFFFFF8 - 0xFFFFFFFF)
        Ok(if next_cluster >= 0xFFFFFFF8 { 
            debug!("Cluster {} is end of chain (value: {:#x})", cluster, next_cluster);
            None 
        } else { 
            debug!("Next cluster after {} is {}", cluster, next_cluster);
            Some(next_cluster) 
        })
    }
    
    /// Read cluster chain
    fn read_cluster_chain(&mut self, first_cluster: u32, max_clusters: Option<usize>) -> Result<Vec<u8>, MosesError> {
        let mut data = Vec::new();
        let mut current_cluster = first_cluster;
        let mut clusters_read = 0;
        
        debug!("Starting cluster chain read from cluster {}", first_cluster);
        
        loop {
            debug!("Reading cluster {} (cluster {} in chain)", current_cluster, clusters_read + 1);
            let cluster_data = self.read_cluster(current_cluster)?;
            debug!("Successfully read cluster {}, got {} bytes", current_cluster, cluster_data.len());
            
            data.extend_from_slice(&cluster_data);
            clusters_read += 1;
            
            if let Some(max) = max_clusters {
                if clusters_read >= max {
                    debug!("Reached max clusters limit ({})", max);
                    break;
                }
            }
            
            debug!("Getting next cluster in chain after cluster {}", current_cluster);
            match self.get_next_cluster(current_cluster)? {
                Some(next) => {
                    debug!("Next cluster in chain: {}", next);
                    current_cluster = next;
                },
                None => {
                    debug!("End of cluster chain reached");
                    break;
                }
            }
        }
        
        debug!("Cluster chain read complete: {} clusters, {} bytes total", clusters_read, data.len());
        Ok(data)
    }
    
    /// Parse directory entries from cluster data
    fn parse_directory_entries(&self, data: &[u8]) -> Vec<ExFatFile> {
        info!("Parsing directory entries from {} bytes of data", data.len());
        let mut files = Vec::new();
        let mut i = 0;
        let mut entries_examined = 0;
        
        while i + 32 <= data.len() {
            entries_examined += 1;
            let entry_type = data[i];
            
            // Check if this is an in-use entry
            if entry_type & 0x80 == 0 {
                i += 32;
                continue;
            }
            
            // File directory entry (0x85)
            if entry_type == 0x85 {
                let file_entry = unsafe {
                    std::ptr::read_unaligned((data.as_ptr().add(i)) as *const FileDirectoryEntry)
                };
                
                // Read the stream extension (should be next)
                if i + 32 < data.len() && data[i + 32] == 0xC0 {
                    let stream_entry = unsafe {
                        std::ptr::read_unaligned((data.as_ptr().add(i + 32)) as *const StreamExtensionEntry)
                    };
                    
                    // Read file name entries
                    let name_length = stream_entry.name_length as usize;
                    let name_entries = (name_length + 14) / 15; // Each entry holds 15 chars
                    
                    let mut name = String::new();
                    for j in 0..name_entries {
                        if i + 64 + j * 32 >= data.len() {
                            break;
                        }
                        
                        if data[i + 64 + j * 32] == 0xC1 {
                            let name_entry = unsafe {
                                std::ptr::read_unaligned((data.as_ptr().add(i + 64 + j * 32)) as *const FileNameEntry)
                            };
                            
                            // Convert UTF-16LE to String
                            // Copy the array to avoid unaligned access
                            let file_name = name_entry.file_name;
                            for &ch in &file_name {
                                if ch == 0 {
                                    break;
                                }
                                if let Some(c) = char::from_u32(ch as u32) {
                                    name.push(c);
                                }
                            }
                        }
                    }
                    
                    if name.len() > name_length {
                        name.truncate(name_length);
                    }
                    
                    files.push(ExFatFile {
                        name,
                        is_directory: file_entry.file_attributes & 0x10 != 0,
                        size: stream_entry.data_length,
                        first_cluster: stream_entry.first_cluster,
                        attributes: file_entry.file_attributes,
                    });
                    
                    // Skip all the entries we just read
                    i += 32 * (2 + name_entries);
                    continue;
                }
            }
            
            i += 32;
        }
        
        info!("Examined {} entries, found {} files/directories", entries_examined, files.len());
        for file in &files {
            info!("  - {} (dir: {}, size: {}, cluster: {})", 
                  file.name, file.is_directory, file.size, file.first_cluster);
        }
        
        files
    }
    
    /// Read root directory
    pub fn read_root(&mut self) -> Result<Vec<ExFatFile>, MosesError> {
        info!("Reading exFAT root directory");
        
        // Check cache first
        if let Some(cached) = self.dir_cache.get("/") {
            debug!("Using cached root directory");
            return Ok(cached.clone());
        }
        
        let root_cluster = self.boot_sector.first_cluster_of_root;
        info!("Root cluster is {}", root_cluster);
        
        // Read the cluster chain (limit to reasonable number for root)
        info!("Reading cluster chain starting from cluster {}", root_cluster);
        let data = self.read_cluster_chain(root_cluster, Some(32))?;
        info!("Successfully read {} bytes of directory data", data.len());
        
        info!("Parsing directory entries from {} bytes", data.len());
        let files = self.parse_directory_entries(&data);
        
        info!("Found {} entries in root directory", files.len());
        for file in &files {
            debug!("  Entry: {} (dir: {}, size: {})", file.name, file.is_directory, file.size);
        }
        
        // Cache the result
        self.dir_cache.insert("/".to_string(), files.clone());
        
        Ok(files)
    }
    
    /// Read a specific directory
    pub fn read_directory(&mut self, path: &str) -> Result<Vec<ExFatFile>, MosesError> {
        info!("Reading exFAT directory: {}", path);
        
        // Check cache first
        if let Some(cached) = self.dir_cache.get(path) {
            debug!("Using cached directory: {}", path);
            return Ok(cached.clone());
        }
        
        // Navigate to the directory
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        
        // Start from root
        let mut current_entries = self.read_root()?;
        
        for part in parts {
            // Find the directory entry
            let dir_entry = current_entries.iter()
                .find(|e| e.name.eq_ignore_ascii_case(part) && e.is_directory)
                .ok_or_else(|| MosesError::Other(format!("Directory not found: {}", part)))?;
            
            let current_cluster = dir_entry.first_cluster;
            
            // Read the directory contents
            let data = self.read_cluster_chain(current_cluster, Some(32))?;
            current_entries = self.parse_directory_entries(&data);
        }
        
        info!("Found {} entries in directory {}", current_entries.len(), path);
        
        // Cache the result
        self.dir_cache.insert(path.to_string(), current_entries.clone());
        
        Ok(current_entries)
    }
}