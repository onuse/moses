// FAT32 Reader with Enhanced MosesError - Practical Migration Example
// Shows how to use the pragmatic error_v2 approach

use crate::device_reader::{FileEntry, FilesystemReader, FilesystemInfo, FileMetadata};
use moses_core::error::{MosesError, CorruptionLevel, MosesResult, ErrorContext};
use moses_core::Device;
use std::io::{Read, Seek, SeekFrom};
use std::fs::File;
use std::path::PathBuf;

/// FAT32 Reader with pragmatic error handling
pub struct Fat32ReaderEnhanced {
    device: Device,
    file: File,
    boot_sector: Fat32BootSector,
    fat_start: u64,
    data_start: u64,
    root_cluster: u32,
    bytes_per_cluster: u32,
    max_clusters: u32,
}

#[derive(Debug)]
struct Fat32BootSector {
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    num_fats: u8,
    sectors_per_fat: u32,
    root_cluster: u32,
    total_sectors: u32,
}

impl Fat32ReaderEnhanced {
    /// Create a new FAT32 reader with enhanced error handling
    pub fn new(device: Device) -> MosesResult<Self> {
        let device_path = device.mount_points[0].clone();
        
        // Open device with proper error context
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .open(&device_path)
            .map_err(|e| MosesError::DeviceNotFound {
                path: device_path.clone(),
                source: Some(e),
            })?;
        
        // Read boot sector with rich error info
        let boot_sector = Self::read_boot_sector(&mut file, &device_path)
            .fs_context("FAT32", "reading boot sector")?;
        
        // Validate boot sector
        Self::validate_boot_sector(&boot_sector)?;
        
        // Calculate FAT and data regions
        let fat_start = boot_sector.reserved_sectors as u64 * boot_sector.bytes_per_sector as u64;
        let fat_size = boot_sector.num_fats as u64 * boot_sector.sectors_per_fat as u64 * 
                      boot_sector.bytes_per_sector as u64;
        let data_start = fat_start + fat_size;
        let bytes_per_cluster = boot_sector.sectors_per_cluster as u32 * 
                               boot_sector.bytes_per_sector as u32;
        
        // Calculate maximum valid cluster number
        let data_sectors = boot_sector.total_sectors - 
                          (boot_sector.reserved_sectors as u32 + 
                           (boot_sector.num_fats as u32 * boot_sector.sectors_per_fat));
        let max_clusters = data_sectors / boot_sector.sectors_per_cluster as u32;
        
        Ok(Self {
            device,
            file,
            boot_sector,
            fat_start,
            data_start,
            root_cluster: boot_sector.root_cluster,
            bytes_per_cluster,
            max_clusters,
        })
    }
    
    /// Read boot sector with detailed error reporting
    fn read_boot_sector(file: &mut File, device_path: &str) -> MosesResult<Fat32BootSector> {
        let mut boot_data = [0u8; 512];
        
        file.seek(SeekFrom::Start(0))
            .map_err(|e| MosesError::io(e, 0))?;
            
        file.read_exact(&mut boot_data)
            .map_err(|e| MosesError::io(e, 0))
            .context("Reading FAT32 boot sector")?;
        
        // Parse with validation
        if &boot_data[510..512] != &[0x55, 0xAA] {
            return Err(MosesError::ValidationFailed {
                field: "boot_signature".into(),
                expected: "55 AA".into(),
                actual: format!("{:02X} {:02X}", boot_data[510], boot_data[511]),
            });
        }
        
        // Check FAT32 signature
        if &boot_data[82..87] != b"FAT32" {
            return Err(MosesError::ValidationFailed {
                field: "filesystem_type".into(),
                expected: "FAT32".into(),
                actual: String::from_utf8_lossy(&boot_data[82..87]).to_string(),
            });
        }
        
        Ok(Fat32BootSector {
            bytes_per_sector: u16::from_le_bytes([boot_data[11], boot_data[12]]),
            sectors_per_cluster: boot_data[13],
            reserved_sectors: u16::from_le_bytes([boot_data[14], boot_data[15]]),
            num_fats: boot_data[16],
            sectors_per_fat: u32::from_le_bytes([
                boot_data[36], boot_data[37], boot_data[38], boot_data[39]
            ]),
            root_cluster: u32::from_le_bytes([
                boot_data[44], boot_data[45], boot_data[46], boot_data[47]
            ]),
            total_sectors: u32::from_le_bytes([
                boot_data[32], boot_data[33], boot_data[34], boot_data[35]
            ]),
        })
    }
    
    /// Validate boot sector values
    fn validate_boot_sector(boot: &Fat32BootSector) -> MosesResult<()> {
        // Validate bytes per sector
        if ![512, 1024, 2048, 4096].contains(&boot.bytes_per_sector) {
            return Err(MosesError::ValidationFailed {
                field: "bytes_per_sector".into(),
                expected: "512, 1024, 2048, or 4096".into(),
                actual: boot.bytes_per_sector.to_string(),
            });
        }
        
        // Validate sectors per cluster
        if ![1, 2, 4, 8, 16, 32, 64, 128].contains(&boot.sectors_per_cluster) {
            return Err(MosesError::ValidationFailed {
                field: "sectors_per_cluster".into(),
                expected: "power of 2 from 1 to 128".into(),
                actual: boot.sectors_per_cluster.to_string(),
            });
        }
        
        // Validate root cluster
        if boot.root_cluster < 2 {
            return Err(MosesError::corruption(
                "Invalid root cluster number in boot sector",
                CorruptionLevel::Severe,
            ).at_offset(44));
        }
        
        Ok(())
    }
    
    /// Read FAT entry with error context
    pub fn read_fat_entry(&mut self, cluster: u32) -> MosesResult<u32> {
        // Validate cluster number
        if cluster >= self.max_clusters {
            return Err(MosesError::InvalidArgument {
                message: format!("Cluster {} exceeds maximum {}", cluster, self.max_clusters),
            });
        }
        
        let fat_offset = self.fat_start + (cluster * 4) as u64;
        
        // Seek with error context
        self.file.seek(SeekFrom::Start(fat_offset))
            .map_err(|e| MosesError::io(e, fat_offset))?;
        
        let mut entry_bytes = [0u8; 4];
        self.file.read_exact(&mut entry_bytes)
            .map_err(|e| MosesError::io(e, fat_offset))
            .context("Reading FAT entry")?;
        
        let entry = u32::from_le_bytes(entry_bytes) & 0x0FFFFFFF;
        
        // Check for bad cluster marker
        if entry == 0x0FFFFFF7 {
            return Err(MosesError::corruption(
                format!("Bad cluster marker found at cluster {}", cluster),
                CorruptionLevel::Moderate,
            ).at_offset(fat_offset));
        }
        
        Ok(entry)
    }
    
    /// Read cluster chain with error tracking
    pub fn read_cluster_chain(&mut self, start_cluster: u32) -> MosesResult<Vec<u8>> {
        let mut data = Vec::new();
        let mut current = start_cluster;
        let mut clusters_read = 0;
        const MAX_CHAIN_LENGTH: u32 = 100000; // Prevent infinite loops
        
        while current >= 2 && current < 0x0FFFFFF6 {
            if clusters_read >= MAX_CHAIN_LENGTH {
                return Err(MosesError::corruption(
                    format!("Cluster chain too long starting at {}", start_cluster),
                    CorruptionLevel::Severe,
                ));
            }
            
            // Read cluster data
            let cluster_data = self.read_cluster(current)?;
            data.extend_from_slice(&cluster_data);
            
            // Get next cluster
            current = self.read_fat_entry(current)?;
            clusters_read += 1;
        }
        
        Ok(data)
    }
    
    /// Read single cluster with proper error handling
    fn read_cluster(&mut self, cluster: u32) -> MosesResult<Vec<u8>> {
        if cluster < 2 || cluster >= self.max_clusters {
            return Err(MosesError::InvalidArgument {
                message: format!("Invalid cluster number: {}", cluster),
            });
        }
        
        let offset = self.data_start + ((cluster - 2) * self.bytes_per_cluster) as u64;
        let mut buffer = vec![0u8; self.bytes_per_cluster as usize];
        
        self.file.seek(SeekFrom::Start(offset))
            .map_err(|e| MosesError::io(e, offset))?;
        
        self.file.read_exact(&mut buffer)
            .map_err(|e| MosesError::io(e, offset))
            .context("Reading cluster data")?;
        
        Ok(buffer)
    }
    
    /// Read directory entries with error handling
    pub fn read_directory(&mut self, cluster: u32) -> MosesResult<Vec<DirectoryEntry>> {
        let data = self.read_cluster_chain(cluster)?;
        let mut entries = Vec::new();
        
        for chunk in data.chunks(32) {
            if chunk.len() != 32 {
                continue; // Ignore incomplete entries
            }
            
            // Check for end of directory
            if chunk[0] == 0x00 {
                break;
            }
            
            // Skip deleted entries
            if chunk[0] == 0xE5 {
                continue;
            }
            
            // Parse entry (simplified)
            entries.push(DirectoryEntry {
                name: String::from_utf8_lossy(&chunk[0..11]).trim().to_string(),
                attributes: chunk[11],
                cluster: u32::from_le_bytes([chunk[26], chunk[27], chunk[20], chunk[21]]),
                size: u32::from_le_bytes([chunk[28], chunk[29], chunk[30], chunk[31]]),
            });
        }
        
        Ok(entries)
    }
}

#[derive(Debug)]
pub struct DirectoryEntry {
    pub name: String,
    pub attributes: u8,
    pub cluster: u32,
    pub size: u32,
}

impl DirectoryEntry {
    pub fn is_directory(&self) -> bool {
        self.attributes & 0x10 != 0
    }
    
    pub fn is_file(&self) -> bool {
        !self.is_directory() && self.attributes & 0x08 == 0
    }
}

// Implement FilesystemReader trait
impl FilesystemReader for Fat32ReaderEnhanced {
    fn get_info(&mut self) -> Result<FilesystemInfo, moses_core::MosesError> {
        Ok(FilesystemInfo {
            filesystem_type: "FAT32".to_string(),
            total_space: self.boot_sector.total_sectors as u64 * 
                        self.boot_sector.bytes_per_sector as u64,
            used_space: 0, // Would need to scan FAT to calculate
            volume_label: String::new(), // Would need to read from root directory
        })
    }
    
    fn list_directory(&mut self, path: &str) -> Result<Vec<FileEntry>, moses_core::MosesError> {
        // Start from root if path is empty or "/"
        let cluster = if path.is_empty() || path == "/" {
            self.root_cluster
        } else {
            // Would need to implement path resolution
            return Err(moses_core::MosesError::NotSupported(
                "Path resolution not yet implemented".to_string()
            ));
        };
        
        let entries = self.read_directory(cluster)
            .map_err(|e| match e {
                MosesError::IoError { source, .. } => moses_core::MosesError::IoError(source),
                MosesError::Corruption { message, .. } => moses_core::MosesError::Other(message),
                _ => moses_core::MosesError::Other(e.to_string()),
            })?;
        
        Ok(entries.into_iter().map(|e| FileEntry {
            name: e.name,
            is_directory: e.is_directory(),
            size: e.size as u64,
            metadata: FileMetadata {
                created: None,
                modified: None,
                accessed: None,
                permissions: None,
            },
        }).collect())
    }
    
    fn read_file(&mut self, path: &str) -> Result<Vec<u8>, moses_core::MosesError> {
        // Would need to implement path resolution and file reading
        Err(moses_core::MosesError::NotSupported(
            "File reading not yet implemented".to_string()
        ))
    }
}

// This example shows:
// 1. Using the pragmatic MosesError enhancements
// 2. Proper error context with ErrorContext trait
// 3. Specific error variants (ValidationFailed, Corruption, IoError)
// 4. Offset tracking for I/O errors
// 5. Corruption severity levels
// 6. Clean migration path from old to new error system