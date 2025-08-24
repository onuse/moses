// FAT16 formatter implementation

use moses_core::{Device, MosesError, FormatOptions, FilesystemFormatter, SimulationReport, Platform};
use async_trait::async_trait;
use std::io::{Write, Seek, SeekFrom};
use log::info;

#[repr(C, packed)]
struct Fat16BootSector {
    jump_boot: [u8; 3],
    oem_name: [u8; 8],
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    num_fats: u8,
    root_entries: u16,
    total_sectors_16: u16,
    media_descriptor: u8,
    sectors_per_fat: u16,
    sectors_per_track: u16,
    num_heads: u16,
    hidden_sectors: u32,
    total_sectors_32: u32,
    drive_number: u8,
    reserved: u8,
    boot_signature: u8,
    volume_id: u32,
    volume_label: [u8; 11],
    fs_type: [u8; 8],
    boot_code: [u8; 448],
    signature: u16,
}

pub struct Fat16Formatter;

impl Fat16Formatter {
    fn calculate_fat16_params(device_size: u64) -> Result<(u8, u16, u16), MosesError> {
        // Calculate appropriate cluster size and FAT size
        let total_sectors = device_size / 512;
        
        let sectors_per_cluster = if total_sectors < 32680 {
            2  // 1KB clusters for < 16MB
        } else if total_sectors < 262144 {
            4  // 2KB clusters for < 128MB
        } else if total_sectors < 524288 {
            8  // 4KB clusters for < 256MB
        } else if total_sectors < 1048576 {
            16 // 8KB clusters for < 512MB
        } else if total_sectors < 2097152 {
            32 // 16KB clusters for < 1GB
        } else if total_sectors < 4194304 {
            64 // 32KB clusters for < 2GB
        } else if total_sectors < 8388608 {
            128 // 64KB clusters for < 4GB (max for FAT16)
        } else {
            return Err(MosesError::Other("Device too large for FAT16 (max 4GB)".to_string()));
        };
        
        // Calculate number of clusters
        let data_sectors = total_sectors - 1 - 512; // Rough estimate
        let total_clusters = data_sectors / sectors_per_cluster as u64;
        
        if total_clusters > 65524 {
            return Err(MosesError::Other("Too many clusters for FAT16".to_string()));
        }
        
        // Calculate FAT size (each entry is 2 bytes)
        let fat_entries = total_clusters + 2; // Add 2 for reserved entries
        let bytes_per_fat = fat_entries * 2;
        let sectors_per_fat = ((bytes_per_fat + 511) / 512) as u16;
        
        // Root directory entries (typically 512 for FAT16)
        let root_entries = 512u16;
        
        Ok((sectors_per_cluster, sectors_per_fat, root_entries))
    }
}

#[async_trait]
impl FilesystemFormatter for Fat16Formatter {
    fn name(&self) -> &'static str {
        "FAT16"
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        vec![Platform::Windows, Platform::Linux, Platform::MacOS]
    }
    
    fn requires_external_tools(&self) -> bool {
        false
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        vec![]
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        if options.filesystem_type != "fat16" {
            return Err(MosesError::Other("Invalid filesystem type for FAT16 formatter".to_string()));
        }
        
        // FAT16 doesn't support custom cluster sizes in our implementation
        if options.cluster_size.is_some() {
            info!("FAT16 will use automatic cluster size selection");
        }
        
        Ok(())
    }
    
    fn can_format(&self, device: &Device) -> bool {
        // Don't format system drives
        if device.is_system {
            return false;
        }
        
        // Check size limits (max 4GB for FAT16)
        if device.size > 4 * 1024 * 1024 * 1024 {
            return false;
        }
        
        true
    }
    
    async fn dry_run(&self, device: &Device, options: &FormatOptions) -> Result<SimulationReport, MosesError> {
        let (_sectors_per_cluster, sectors_per_fat, root_entries) = 
            Self::calculate_fat16_params(device.size)?;
        
        let fat_size = sectors_per_fat as u64 * 512 * 2; // 2 FATs
        let root_dir_size = root_entries as u64 * 32;
        let overhead = 512 + fat_size + root_dir_size; // Boot sector + FATs + Root
        
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: std::time::Duration::from_secs(1),
            warnings: if device.size > 2 * 1024 * 1024 * 1024 {
                vec!["Volume larger than 2GB may have compatibility issues".to_string()]
            } else {
                vec![]
            },
            required_tools: vec![],
            will_erase_data: true,
            space_after_format: device.size - overhead,
        })
    }
    
    async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        info!("Formatting {} as FAT16", device.name);
        
        // Check if we should create a partition table
        let create_partition = options.additional_options
            .get("create_partition_table")
            .map(|v| v == "true")
            .unwrap_or(false);
        
        // Calculate parameters based on partition size if creating partition table
        let partition_size = if create_partition {
            // When creating partition table, partition starts at sector 2048
            // So available size is reduced by 1MB
            device.size - (2048 * 512)
        } else {
            device.size
        };
        
        let (sectors_per_cluster, sectors_per_fat, root_entries) = 
            Self::calculate_fat16_params(partition_size)?;
        
        let total_sectors = partition_size / 512;
        let hidden_sectors = if create_partition { 2048u32 } else { 0u32 };
        
        // Create boot sector
        let mut boot_sector = Fat16BootSector {
            jump_boot: [0xEB, 0x3C, 0x90],
            oem_name: *b"MOSES   ",
            bytes_per_sector: 512,
            sectors_per_cluster,
            reserved_sectors: 1,
            num_fats: 2,
            root_entries,
            total_sectors_16: if total_sectors < 65536 { total_sectors as u16 } else { 0 },
            media_descriptor: 0xF8, // Fixed disk
            sectors_per_fat,
            sectors_per_track: 63,
            num_heads: 255,
            hidden_sectors,
            total_sectors_32: if total_sectors >= 65536 { total_sectors as u32 } else { 0 },
            drive_number: 0x80,
            reserved: 0,
            boot_signature: 0x29,
            volume_id: 0x12345678,
            volume_label: *b"MOSES FAT16",
            fs_type: *b"FAT16   ",
            boot_code: [0; 448],
            signature: 0xAA55,
        };
        
        // Set volume label if provided
        if let Some(ref label) = options.label {
            let label_bytes = label.as_bytes();
            let len = label_bytes.len().min(11);
            boot_sector.volume_label[..len].copy_from_slice(&label_bytes[..len]);
            // Pad with spaces
            for i in len..11 {
                boot_sector.volume_label[i] = b' ';
            }
        }
        
        // Open device for writing using proper physical drive access
        use crate::utils::open_device_write;
        
        info!("Opening device for writing: {}", device.name);
        
        let mut file = open_device_write(device)?;
        
        // If requested, write partition table first
        let partition_offset = if create_partition {
            info!("Creating MBR partition table");
            
            use crate::partitioner::{create_single_partition_table, PartitionTableType, write_partition_table};
            
            let partition_table = create_single_partition_table(
                device,
                PartitionTableType::MBR,
                "fat16"
            )?;
            
            write_partition_table(&mut file, &partition_table)?;
            
            // FAT16 filesystem will start at sector 2048 (1MB offset)
            2048 * 512
        } else {
            // No partition table, filesystem starts at sector 0
            0
        };
        
        // Seek to partition start
        if partition_offset > 0 {
            file.seek(SeekFrom::Start(partition_offset))
                .map_err(|e| MosesError::Other(format!("Failed to seek to partition start: {}", e)))?;
        }
        
        // Write boot sector
        let boot_sector_bytes = unsafe {
            std::slice::from_raw_parts(
                &boot_sector as *const _ as *const u8,
                std::mem::size_of::<Fat16BootSector>()
            )
        };
        
        file.write_all(boot_sector_bytes)
            .map_err(|e| MosesError::Other(format!("Failed to write boot sector: {}", e)))?;
        
        // Write FAT tables
        let fat_size = sectors_per_fat as usize * 512;
        let mut fat = vec![0u8; fat_size];
        
        // First two FAT entries are reserved
        fat[0] = 0xF8; // Media descriptor
        fat[1] = 0xFF;
        fat[2] = 0xFF; // End of chain marker
        fat[3] = 0xFF;
        
        // Write first FAT (after boot sector, which is at partition_offset)
        file.seek(SeekFrom::Start(partition_offset + 512))
            .map_err(|e| MosesError::Other(format!("Failed to seek to FAT1: {}", e)))?;
        file.write_all(&fat)
            .map_err(|e| MosesError::Other(format!("Failed to write FAT1: {}", e)))?;
        
        // Write second FAT (immediately after first FAT)
        file.write_all(&fat)
            .map_err(|e| MosesError::Other(format!("Failed to write FAT2: {}", e)))?;
        
        // Clear root directory
        let root_dir_sectors = (root_entries * 32 + 511) / 512;
        let root_dir = vec![0u8; root_dir_sectors as usize * 512];
        file.write_all(&root_dir)
            .map_err(|e| MosesError::Other(format!("Failed to write root directory: {}", e)))?;
        
        file.flush()
            .map_err(|e| MosesError::Other(format!("Failed to flush: {}", e)))?;
        
        info!("FAT16 format completed successfully");
        Ok(())
    }
}