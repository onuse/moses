// Fixed FAT16 formatter implementation with proper structure alignment

use moses_core::{Device, MosesError, FormatOptions, FilesystemFormatter, SimulationReport, Platform};
use async_trait::async_trait;
use std::fs::OpenOptions;
use std::io::{Write, Seek, SeekFrom};
use log::info;

// Properly aligned FAT16 boot sector
#[repr(C, packed(1))]
#[derive(Debug, Clone, Copy)]
struct Fat16BootSector {
    jump_boot: [u8; 3],         // 0x00: Jump instruction
    oem_name: [u8; 8],          // 0x03: OEM name
    bytes_per_sector: u16,      // 0x0B: Bytes per sector (512)
    sectors_per_cluster: u8,    // 0x0D: Sectors per cluster
    reserved_sectors: u16,      // 0x0E: Reserved sectors (usually 1)
    num_fats: u8,              // 0x10: Number of FATs (usually 2)
    root_entries: u16,         // 0x11: Root directory entries (512 for FAT16)
    total_sectors_16: u16,     // 0x13: Total sectors (if < 65536)
    media_descriptor: u8,      // 0x15: Media descriptor (0xF8 for hard disk)
    sectors_per_fat: u16,      // 0x16: Sectors per FAT
    sectors_per_track: u16,    // 0x18: Sectors per track
    num_heads: u16,            // 0x1A: Number of heads
    hidden_sectors: u32,       // 0x1C: Hidden sectors
    total_sectors_32: u32,     // 0x20: Total sectors (if >= 65536)
    drive_number: u8,          // 0x24: Drive number (0x80 for hard disk)
    reserved: u8,              // 0x25: Reserved
    boot_signature: u8,        // 0x26: Boot signature (0x29)
    volume_id: u32,            // 0x27: Volume ID
    volume_label: [u8; 11],    // 0x2B: Volume label
    fs_type: [u8; 8],          // 0x36: File system type "FAT16   "
}

pub struct Fat16FormatterFixed;

impl Fat16FormatterFixed {
    fn calculate_fat16_params(size_bytes: u64) -> Result<(u8, u16, u16), MosesError> {
        let total_sectors = size_bytes / 512;
        
        // Microsoft's recommended cluster sizes for FAT16
        let sectors_per_cluster = if total_sectors <= 32_680 {
            2   // 1KB clusters for <= 16MB
        } else if total_sectors <= 262_144 {
            4   // 2KB clusters for <= 128MB
        } else if total_sectors <= 524_288 {
            8   // 4KB clusters for <= 256MB
        } else if total_sectors <= 1_048_576 {
            16  // 8KB clusters for <= 512MB
        } else if total_sectors <= 2_097_152 {
            32  // 16KB clusters for <= 1GB
        } else if total_sectors <= 4_194_304 {
            64  // 32KB clusters for <= 2GB
        } else if total_sectors <= 8_388_608 {
            128 // 64KB clusters for <= 4GB (maximum for FAT16)
        } else {
            return Err(MosesError::Other("Volume too large for FAT16 (max 4GB with 64KB clusters)".to_string()));
        };
        
        // Calculate FAT size
        // We need to account for: boot sector, FATs, root directory, data area
        let root_entries = 512u16; // Standard for FAT16
        let root_dir_sectors = (root_entries * 32 + 511) / 512;
        let reserved_sectors = 1u16; // Boot sector
        
        // Estimate data sectors (total - boot - root)
        let data_sectors = total_sectors - reserved_sectors as u64 - root_dir_sectors as u64;
        
        // Calculate number of clusters
        let total_clusters = data_sectors / sectors_per_cluster as u64;
        
        // FAT16 must have between 4085 and 65524 clusters
        if total_clusters < 4085 {
            // Too few clusters for FAT16, might be FAT12
            return Err(MosesError::Other("Volume too small for FAT16".to_string()));
        }
        if total_clusters > 65524 {
            // Too many clusters for FAT16
            return Err(MosesError::Other("Too many clusters for FAT16".to_string()));
        }
        
        // Calculate FAT size (each FAT entry is 2 bytes)
        let fat_entries = total_clusters + 2; // +2 for reserved entries
        let fat_bytes = fat_entries * 2;
        let sectors_per_fat = ((fat_bytes + 511) / 512) as u16;
        
        Ok((sectors_per_cluster, sectors_per_fat, root_entries))
    }
    
    fn create_boot_sector(
        total_sectors: u64,
        sectors_per_cluster: u8,
        sectors_per_fat: u16,
        root_entries: u16,
        hidden_sectors: u32,
        volume_label: Option<&str>,
    ) -> Fat16BootSector {
        let mut boot_sector = Fat16BootSector {
            jump_boot: [0xEB, 0x3C, 0x90],  // Standard jump instruction
            oem_name: *b"MSWIN4.1",          // Windows compatible OEM name
            bytes_per_sector: 512,
            sectors_per_cluster,
            reserved_sectors: 1,
            num_fats: 2,
            root_entries,
            total_sectors_16: if total_sectors < 65536 { total_sectors as u16 } else { 0 },
            media_descriptor: 0xF8,  // Fixed disk
            sectors_per_fat,
            sectors_per_track: 63,   // Standard geometry
            num_heads: 255,          // Standard geometry
            hidden_sectors,
            total_sectors_32: if total_sectors >= 65536 { total_sectors as u32 } else { 0 },
            drive_number: 0x80,      // Hard disk
            reserved: 0,
            boot_signature: 0x29,    // Extended boot signature
            volume_id: 0x12345678,   // Could be random
            volume_label: *b"NO NAME    ",
            fs_type: *b"FAT16   ",
        };
        
        // Set volume label if provided
        if let Some(label) = volume_label {
            let label_bytes = label.as_bytes();
            let len = label_bytes.len().min(11);
            boot_sector.volume_label[..len].copy_from_slice(&label_bytes[..len]);
            // Pad with spaces
            for i in len..11 {
                boot_sector.volume_label[i] = b' ';
            }
        }
        
        boot_sector
    }
}

#[async_trait]
impl FilesystemFormatter for Fat16FormatterFixed {
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
        Ok(())
    }
    
    fn can_format(&self, device: &Device) -> bool {
        if device.is_system {
            return false;
        }
        
        // FAT16 max size is 4GB (with 64KB clusters)
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
        
        let mut warnings = vec![];
        if device.size > 2 * 1024 * 1024 * 1024 {
            warnings.push("Volume larger than 2GB uses 64KB clusters. Some older systems may have compatibility issues.".to_string());
        }
        
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: std::time::Duration::from_secs(1),
            warnings,
            required_tools: vec![],
            will_erase_data: true,
            space_after_format: device.size - overhead,
        })
    }
    
    async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        info!("Formatting {} as FAT16 (native implementation)", device.name);
        
        // Check if we should create a partition table
        // For removable devices, default to NO partition table for better Windows compatibility
        // For fixed disks, partition tables are more common
        let create_partition = options.additional_options
            .get("create_partition_table")
            .map(|v| v == "true")
            .unwrap_or_else(|| {
                // Only default to partition table for non-removable devices
                !device.is_removable && device.size > 512 * 1024 * 1024 // > 512MB
            });
        
        info!("Partition table creation: {}", if create_partition { "enabled" } else { "disabled (direct format)" });
        
        // Calculate parameters based on partition size
        let (partition_size, partition_offset, hidden_sectors) = if create_partition {
            // Partition starts at sector 2048 (1MB offset)
            let offset = 2048 * 512;
            let size = device.size - offset;
            (size, offset, 2048u32)
        } else {
            (device.size, 0u64, 0u32)
        };
        
        let total_sectors = partition_size / 512;
        let (sectors_per_cluster, sectors_per_fat, root_entries) = 
            Self::calculate_fat16_params(partition_size)?;
        
        info!("FAT16 parameters: {} sectors, {} sectors/cluster, {} sectors/FAT, {} root entries",
              total_sectors, sectors_per_cluster, sectors_per_fat, root_entries);
        
        // Create boot sector
        let boot_sector = Self::create_boot_sector(
            total_sectors,
            sectors_per_cluster,
            sectors_per_fat,
            root_entries,
            hidden_sectors,
            options.label.as_deref(),
        );
        
        // Open device for writing
        let device_path = if !device.mount_points.is_empty() {
            let mount = &device.mount_points[0];
            let mount_str = mount.to_string_lossy();
            if mount_str.len() >= 2 && mount_str.chars().nth(1) == Some(':') {
                format!("\\\\.\\{}", mount_str.trim_end_matches('\\'))
            } else {
                device.id.clone()
            }
        } else {
            device.id.clone()
        };
        
        info!("Opening device at: {}", device_path);
        
        let mut file = OpenOptions::new()
            .write(true)
            .open(&device_path)
            .map_err(|e| MosesError::Other(format!("Failed to open device: {}", e)))?;
        
        // Write partition table if requested
        if create_partition {
            info!("Creating MBR partition table");
            
            use crate::partitioner::{create_single_partition_table, PartitionTableType, write_partition_table};
            
            let partition_table = create_single_partition_table(
                device,
                PartitionTableType::MBR,
                "fat16"
            )?;
            
            write_partition_table(&mut file, &partition_table)?;
        }
        
        // Seek to partition start (or stay at 0 for direct format)
        if partition_offset > 0 {
            info!("Writing FAT16 at offset {} (partition)", partition_offset);
            file.seek(SeekFrom::Start(partition_offset))
                .map_err(|e| MosesError::Other(format!("Failed to seek to partition start: {}", e)))?;
        } else {
            info!("Writing FAT16 at offset 0 (direct format, no partition table)");
        }
        
        // Write boot sector
        let boot_sector_bytes = unsafe {
            std::slice::from_raw_parts(
                &boot_sector as *const _ as *const u8,
                std::mem::size_of::<Fat16BootSector>()
            )
        };
        
        // Ensure boot sector is exactly 512 bytes
        let mut full_boot_sector = vec![0u8; 512];
        full_boot_sector[..boot_sector_bytes.len().min(512)].copy_from_slice(boot_sector_bytes);
        
        // Add boot signature at the end
        full_boot_sector[510] = 0x55;
        full_boot_sector[511] = 0xAA;
        
        file.write_all(&full_boot_sector)
            .map_err(|e| MosesError::Other(format!("Failed to write boot sector: {}", e)))?;
        
        // Create and write FAT tables
        let fat_size = sectors_per_fat as usize * 512;
        let mut fat = vec![0u8; fat_size];
        
        // FAT16 uses 16-bit entries. First two entries are reserved:
        // FAT[0] = Media descriptor in low byte, 0xFF in high byte
        // FAT[1] = End of chain marker (0xFFFF)
        
        // FAT[0]: 16-bit value with media descriptor (0xF8 for fixed disk)
        let fat0_value: u16 = 0xFFF8;  // 0xFF in high byte, 0xF8 in low byte
        fat[0..2].copy_from_slice(&fat0_value.to_le_bytes());
        
        // FAT[1]: End-of-chain marker (0xFFFF)
        let fat1_value: u16 = 0xFFFF;
        fat[2..4].copy_from_slice(&fat1_value.to_le_bytes());
        
        // Write first FAT
        file.write_all(&fat)
            .map_err(|e| MosesError::Other(format!("Failed to write FAT1: {}", e)))?;
        
        // Write second FAT
        file.write_all(&fat)
            .map_err(|e| MosesError::Other(format!("Failed to write FAT2: {}", e)))?;
        
        // Initialize root directory (all zeros)
        let root_dir_sectors = (root_entries * 32 + 511) / 512;
        let root_dir = vec![0u8; root_dir_sectors as usize * 512];
        file.write_all(&root_dir)
            .map_err(|e| MosesError::Other(format!("Failed to write root directory: {}", e)))?;
        
        // Flush to ensure all data is written
        file.flush()
            .map_err(|e| MosesError::Other(format!("Failed to flush: {}", e)))?;
        
        info!("FAT16 format completed successfully");
        Ok(())
    }
}