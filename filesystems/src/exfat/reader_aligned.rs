// exFAT filesystem reader using common device abstraction
// Simplified version that leverages AlignedDeviceReader

use moses_core::{Device, MosesError};
use crate::device_reader::{AlignedDeviceReader, FilesystemReader, FileEntry, FilesystemInfo, FileMetadata};
use log::{info, debug};
use std::collections::HashMap;

// Re-use structures from the original reader
use super::reader::{
    ExFatBootSector, FileDirectoryEntry, StreamExtensionEntry, 
    FileNameEntry, EXFAT_SIGNATURE
};

/// exFAT filesystem reader with aligned device reading
pub struct ExFatReaderAligned {
    _device: Device,
    reader: AlignedDeviceReader,
    _boot_sector: ExFatBootSector,
    
    // Filesystem parameters
    _bytes_per_sector: u32,
    _sectors_per_cluster: u32,
    bytes_per_cluster: u32,
    fat_offset: u64,
    _fat_length: u64,
    cluster_heap_offset: u64,
    root_cluster: u32,
    total_clusters: u32,
    
    // Cache
    fat_cache: HashMap<u32, u32>,  // cluster -> next cluster
    dir_cache: HashMap<String, Vec<FileEntry>>,
}

impl ExFatReaderAligned {
    /// Create a new exFAT reader
    pub fn new(device: Device) -> Result<Self, MosesError> {
        use crate::utils::open_device_with_fallback;
        
        info!("Opening exFAT filesystem on device: {}", device.name);
        
        // Open device with our aligned reader
        let file = open_device_with_fallback(&device)?;
        let mut reader = AlignedDeviceReader::new(file);
        
        // Read boot sector
        let boot_data = reader.read_at(0, 512)?;
        let boot_sector = unsafe {
            std::ptr::read_unaligned(boot_data.as_ptr() as *const ExFatBootSector)
        };
        
        // Validate exFAT
        if boot_sector.fs_name != EXFAT_SIGNATURE {
            return Err(MosesError::Other("Not an exFAT filesystem".to_string()));
        }
        
        // Copy values to avoid unaligned access
        let bytes_per_sector = 1u32 << boot_sector.bytes_per_sector_shift;
        let sectors_per_cluster = 1u32 << boot_sector.sectors_per_cluster_shift;
        let bytes_per_cluster = bytes_per_sector * sectors_per_cluster;
        let partition_offset = boot_sector.partition_offset;
        let fat_offset_sectors = boot_sector.fat_offset;
        let fat_length_sectors = boot_sector.fat_length;
        let cluster_heap_offset_sectors = boot_sector.cluster_heap_offset;
        let cluster_count = boot_sector.cluster_count;
        let root_cluster = boot_sector.first_cluster_of_root;
        
        // Determine if we're using a volume handle
        let using_volume_handle = device.mount_points.iter()
            .any(|p| {
                let s = p.to_string_lossy();
                s.len() >= 2 && s.chars().nth(1) == Some(':')
            });
        
        // Calculate offsets based on handle type
        let (fat_offset, cluster_heap_offset) = if using_volume_handle {
            // Volume handle: offsets are already relative to partition
            info!("Using volume handle, offsets are partition-relative");
            (
                fat_offset_sectors as u64 * bytes_per_sector as u64,
                cluster_heap_offset_sectors as u64 * bytes_per_sector as u64,
            )
        } else {
            // Physical disk: add partition offset
            info!("Using physical disk handle, adding partition offset");
            let partition_bytes = partition_offset * bytes_per_sector as u64;
            (
                partition_bytes + (fat_offset_sectors as u64 * bytes_per_sector as u64),
                partition_bytes + (cluster_heap_offset_sectors as u64 * bytes_per_sector as u64),
            )
        };
        
        let fat_length = fat_length_sectors as u64 * bytes_per_sector as u64;
        
        info!("exFAT filesystem details:");
        info!("  Bytes per sector: {}", bytes_per_sector);
        info!("  Sectors per cluster: {}", sectors_per_cluster);
        info!("  Bytes per cluster: {}", bytes_per_cluster);
        info!("  FAT offset: {:#x}", fat_offset);
        info!("  FAT length: {:#x}", fat_length);
        info!("  Cluster heap offset: {:#x}", cluster_heap_offset);
        info!("  Root cluster: {}", root_cluster);
        info!("  Total clusters: {}", cluster_count);
        
        Ok(Self {
            _device: device,
            reader,
            _boot_sector: boot_sector,
            _bytes_per_sector: bytes_per_sector,
            _sectors_per_cluster: sectors_per_cluster,
            bytes_per_cluster,
            fat_offset,
            _fat_length: fat_length,
            cluster_heap_offset,
            root_cluster,
            total_clusters: cluster_count,
            fat_cache: HashMap::new(),
            dir_cache: HashMap::new(),
        })
    }
    
    /// Read a cluster by number
    fn read_cluster(&mut self, cluster_num: u32) -> Result<Vec<u8>, MosesError> {
        if cluster_num < 2 || cluster_num >= self.total_clusters + 2 {
            return Err(MosesError::Other(format!("Invalid cluster number: {}", cluster_num)));
        }
        
        let offset = self.cluster_heap_offset + 
                    ((cluster_num - 2) as u64 * self.bytes_per_cluster as u64);
        
        debug!("Reading cluster {} at offset {:#x}, size: {} bytes", 
               cluster_num, offset, self.bytes_per_cluster);
        
        // AlignedDeviceReader handles all the sector alignment for us!
        self.reader.read_at(offset, self.bytes_per_cluster as usize)
    }
    
    /// Get next cluster from FAT
    fn get_next_cluster(&mut self, cluster: u32) -> Result<Option<u32>, MosesError> {
        // Check cache first
        if let Some(&next) = self.fat_cache.get(&cluster) {
            return Ok(if next >= 0xFFFFFFF8 { None } else { Some(next) });
        }
        
        // Read FAT entry (4 bytes per entry in exFAT)
        let fat_entry_offset = self.fat_offset + (cluster * 4) as u64;
        debug!("Reading FAT entry for cluster {} at offset {:#x}", cluster, fat_entry_offset);
        
        // AlignedDeviceReader handles the alignment!
        let entry_data = self.reader.read_at(fat_entry_offset, 4)?;
        let next_cluster = u32::from_le_bytes([
            entry_data[0], entry_data[1], entry_data[2], entry_data[3]
        ]);
        
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
        
        debug!("Reading cluster chain starting from cluster {}", first_cluster);
        
        loop {
            let cluster_data = self.read_cluster(current_cluster)?;
            data.extend_from_slice(&cluster_data);
            
            clusters_read += 1;
            if let Some(max) = max_clusters {
                if clusters_read >= max {
                    debug!("Reached max clusters limit ({})", max);
                    break;
                }
            }
            
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
        
        debug!("Read {} clusters, {} bytes total", clusters_read, data.len());
        Ok(data)
    }
    
    /// Parse directory entries from cluster data
    fn parse_directory_entries(&self, data: &[u8]) -> Vec<FileEntry> {
        let mut entries = Vec::new();
        let mut i = 0;
        
        while i + 32 <= data.len() {
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
                            
                            // Convert UTF-16LE to String (copy array to avoid unaligned access)
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
                    
                    entries.push(FileEntry {
                        name,
                        is_directory: file_entry.file_attributes & 0x10 != 0,
                        size: stream_entry.data_length,
                        cluster: Some(stream_entry.first_cluster),
                        metadata: FileMetadata::default(),
                    });
                    
                    // Skip all the entries we just read
                    i += 32 * (2 + name_entries);
                    continue;
                }
            }
            
            i += 32;
        }
        
        entries
    }
    
    /// Read root directory
    pub fn read_root(&mut self) -> Result<Vec<FileEntry>, MosesError> {
        debug!("Reading exFAT root directory");
        let data = self.read_cluster_chain(self.root_cluster, Some(32))?;
        Ok(self.parse_directory_entries(&data))
    }
    
    /// Read a specific directory by path
    pub fn read_directory(&mut self, path: &str) -> Result<Vec<FileEntry>, MosesError> {
        if path == "/" || path.is_empty() {
            return self.read_root();
        }
        
        // Navigate to the directory
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut current_cluster = self.root_cluster;
        
        for part in parts {
            let data = self.read_cluster_chain(current_cluster, Some(32))?;
            let entries = self.parse_directory_entries(&data);
            
            let dir = entries.iter()
                .find(|e| e.name.eq_ignore_ascii_case(part) && e.is_directory)
                .ok_or_else(|| MosesError::Other(format!("Directory not found: {}", part)))?;
            
            current_cluster = dir.cluster.unwrap();
        }
        
        let data = self.read_cluster_chain(current_cluster, Some(32))?;
        Ok(self.parse_directory_entries(&data))
    }
}

impl FilesystemReader for ExFatReaderAligned {
    fn read_metadata(&mut self) -> Result<(), MosesError> {
        // Already read in new()
        Ok(())
    }
    
    fn list_directory(&mut self, path: &str) -> Result<Vec<FileEntry>, MosesError> {
        // Check cache first
        if let Some(cached) = self.dir_cache.get(path) {
            return Ok(cached.clone());
        }
        
        let entries = self.read_directory(path)?;
        
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
        
        // Read directory
        let entries = self.list_directory(dir_path)?;
        
        // Find file
        let file = entries.iter()
            .find(|e| e.name.eq_ignore_ascii_case(file_name) && !e.is_directory)
            .ok_or_else(|| MosesError::Other(format!("File not found: {}", file_name)))?;
        
        // Read file data
        if let Some(cluster) = file.cluster {
            if cluster == 0 {
                // Empty file
                Ok(Vec::new())
            } else {
                let mut data = self.read_cluster_chain(cluster, None)?;
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
            fs_type: "exFAT".to_string(),
            label: None, // Would need to read volume label from root directory
            total_bytes,
            used_bytes: 0, // Would need to scan allocation bitmap
            cluster_size: Some(self.bytes_per_cluster),
        }
    }
}

// Re-export the structures that other modules might need
pub use super::reader::ExFatFile;