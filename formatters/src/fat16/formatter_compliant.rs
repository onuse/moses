// FAT16 formatter with full specification compliance
// Fixes all known issues with Windows recognition

use moses_core::{Device, MosesError, FormatOptions, FilesystemFormatter, SimulationReport, Platform};
use async_trait::async_trait;
use std::io::{Write, Seek, SeekFrom};
use log::{info, warn};
use std::time::SystemTime;

pub struct Fat16CompliantFormatter;

impl Fat16CompliantFormatter {
    fn calculate_fat16_params(size_bytes: u64, requested_cluster_size: Option<u32>) -> Result<(u8, u16, u16), MosesError> {
        // Use the validated parameter calculation
        use super::validation::validate_and_fix_fat16_params;
        
        // If user specified a cluster size, validate it works with the device size
        if let Some(cluster_bytes) = requested_cluster_size {
            let sectors_per_cluster = (cluster_bytes / 512) as u8;
            let total_sectors = size_bytes / 512;
            
            // Calculate with the requested cluster size
            let reserved_sectors = 1u16;
            let _num_fats = 2u8;
            let root_entries = 512u16;
            let root_dir_sectors = (root_entries * 32 + 511) / 512;
            
            // Estimate data area
            let data_start_estimate = reserved_sectors + root_dir_sectors;
            let usable_sectors = total_sectors.saturating_sub(data_start_estimate as u64);
            let total_clusters = usable_sectors / sectors_per_cluster as u64;
            
            // Validate cluster count for FAT16
            if total_clusters < 4085 {
                return Err(MosesError::Other(format!(
                    "Cluster size {} bytes produces too few clusters ({}) for FAT16. Minimum is 4085.",
                    cluster_bytes, total_clusters
                )));
            }
            if total_clusters > 65524 {
                return Err(MosesError::Other(format!(
                    "Cluster size {} bytes produces too many clusters ({}) for FAT16. Maximum is 65524.",
                    cluster_bytes, total_clusters
                )));
            }
            
            // Calculate FAT size
            let fat_entries = total_clusters + 2;
            let fat_bytes = fat_entries * 2;
            let sectors_per_fat = ((fat_bytes + 511) / 512) as u16;
            
            info!("Using user-specified cluster size: {} bytes ({} sectors)", 
                  cluster_bytes, sectors_per_cluster);
            
            return Ok((sectors_per_cluster, sectors_per_fat, root_entries));
        }
        
        // Otherwise use automatic calculation
        let (sectors_per_cluster, sectors_per_fat, root_entries, notes) = 
            validate_and_fix_fat16_params(size_bytes)?;
        
        if notes.contains("WARNING") {
            warn!("FAT16 validation: {}", notes);
        } else {
            info!("FAT16 validation: {}", notes);
        }
        
        Ok((sectors_per_cluster, sectors_per_fat, root_entries))
    }
    
    fn create_boot_sector_bytes(
        device: &Device,
        total_sectors: u64,
        sectors_per_cluster: u8,
        sectors_per_fat: u16,
        root_entries: u16,
        hidden_sectors: u32,
        volume_label: Option<&str>,
    ) -> Vec<u8> {
        let mut boot_sector = vec![0u8; 512];
        
        // Jump instruction (0x00)
        boot_sector[0] = 0xEB;
        boot_sector[1] = 0x3C;
        boot_sector[2] = 0x90;
        
        // OEM Name (0x03) - 8 bytes
        let oem_name = b"MSWIN4.1";
        boot_sector[3..11].copy_from_slice(oem_name);
        
        // Bytes per sector (0x0B) - 2 bytes
        boot_sector[0x0B..0x0D].copy_from_slice(&512u16.to_le_bytes());
        
        // Sectors per cluster (0x0D) - 1 byte
        boot_sector[0x0D] = sectors_per_cluster;
        
        // Reserved sectors (0x0E) - 2 bytes
        boot_sector[0x0E..0x10].copy_from_slice(&1u16.to_le_bytes());
        
        // Number of FATs (0x10) - 1 byte
        boot_sector[0x10] = 2;
        
        // Root entries (0x11) - 2 bytes
        boot_sector[0x11..0x13].copy_from_slice(&root_entries.to_le_bytes());
        
        // Total sectors 16 (0x13) - 2 bytes
        if total_sectors < 65536 {
            boot_sector[0x13..0x15].copy_from_slice(&(total_sectors as u16).to_le_bytes());
        }
        
        // Media descriptor (0x15) - 1 byte
        // 0xF0 for removable media, 0xF8 for fixed disk
        let media_descriptor = if device.is_removable { 0xF0 } else { 0xF8 };
        boot_sector[0x15] = media_descriptor;
        
        // Sectors per FAT (0x16) - 2 bytes
        boot_sector[0x16..0x18].copy_from_slice(&sectors_per_fat.to_le_bytes());
        
        // Sectors per track (0x18) - 2 bytes (geometry)
        boot_sector[0x18..0x1A].copy_from_slice(&63u16.to_le_bytes());
        
        // Number of heads (0x1A) - 2 bytes (geometry)
        boot_sector[0x1A..0x1C].copy_from_slice(&255u16.to_le_bytes());
        
        // Hidden sectors (0x1C) - 4 bytes
        boot_sector[0x1C..0x20].copy_from_slice(&hidden_sectors.to_le_bytes());
        
        // Total sectors 32 (0x20) - 4 bytes
        if total_sectors >= 65536 {
            boot_sector[0x20..0x24].copy_from_slice(&(total_sectors as u32).to_le_bytes());
        }
        
        // Drive number (0x24) - 1 byte
        // 0x00 for removable media, 0x80 for fixed disk
        let drive_number = if device.is_removable { 0x00 } else { 0x80 };
        boot_sector[0x24] = drive_number;
        
        // Reserved (0x25) - 1 byte
        boot_sector[0x25] = 0;
        
        // Extended boot signature (0x26) - 1 byte
        boot_sector[0x26] = 0x29;
        
        // Volume ID (0x27) - 4 bytes (should be unique)
        let volume_id = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;
        boot_sector[0x27..0x2B].copy_from_slice(&volume_id.to_le_bytes());
        
        // Volume label (0x2B) - 11 bytes
        let mut label_bytes = [b' '; 11];
        if let Some(label) = volume_label {
            let label_str = label.as_bytes();
            let len = label_str.len().min(11);
            label_bytes[..len].copy_from_slice(&label_str[..len]);
        } else {
            label_bytes.copy_from_slice(b"NO NAME    ");
        }
        boot_sector[0x2B..0x36].copy_from_slice(&label_bytes);
        
        // File system type (0x36) - 8 bytes
        boot_sector[0x36..0x3E].copy_from_slice(b"FAT16   ");
        
        // Boot code (can be zeros)
        // ...
        
        // Boot sector signature (0x1FE) - 2 bytes
        boot_sector[0x1FE] = 0x55;
        boot_sector[0x1FF] = 0xAA;
        
        boot_sector
    }
}

#[async_trait]
impl FilesystemFormatter for Fat16CompliantFormatter {
    fn name(&self) -> &'static str {
        "FAT16 (Compliant)"
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
        
        // Validate cluster size if specified
        if let Some(cluster_size) = options.cluster_size {
            // FAT16 supports cluster sizes from 512 bytes to 32KB (64 sectors)
            let valid_sizes = [512, 1024, 2048, 4096, 8192, 16384, 32768];
            
            if !valid_sizes.contains(&cluster_size) {
                return Err(MosesError::Other(format!(
                    "Invalid cluster size {} for FAT16. Valid sizes are: 512, 1024, 2048, 4096, 8192, 16384, 32768 bytes",
                    cluster_size
                )));
            }
            
            // Warn about larger cluster sizes
            if cluster_size > 16384 {
                warn!("Cluster size {} bytes may have compatibility issues with older systems", cluster_size);
            }
        }
        
        Ok(())
    }
    
    fn can_format(&self, device: &Device) -> bool {
        !device.is_system && device.size <= 4 * 1024 * 1024 * 1024 // Max 4GB for FAT16
    }
    
    async fn dry_run(&self, device: &Device, options: &FormatOptions) -> Result<SimulationReport, MosesError> {
        let (_sectors_per_cluster, sectors_per_fat, root_entries) = 
            Self::calculate_fat16_params(device.size, options.cluster_size)?;
        
        let fat_size = sectors_per_fat as u64 * 512 * 2; // 2 FATs
        let root_dir_size = root_entries as u64 * 32;
        let overhead = 512 + fat_size + root_dir_size; // Boot sector + FATs + Root
        
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: std::time::Duration::from_secs(2),
            warnings: if device.size > 2 * 1024 * 1024 * 1024 {
                vec!["Volume larger than 2GB may have compatibility issues with FAT16".to_string()]
            } else {
                vec![]
            },
            required_tools: vec![],
            will_erase_data: true,
            space_after_format: device.size - overhead,
        })
    }
    
    async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        info!("Starting FAT16 compliant format for device: {}", device.name);
        
        // Check if we should create a partition table
        let create_partition = options.additional_options
            .get("create_partition_table")
            .map(|v| v == "true")
            .unwrap_or(false);
        
        info!("Partition table creation: {}", if create_partition { "enabled" } else { "disabled (direct format)" });
        
        // Calculate parameters based on partition size
        let (partition_size, partition_offset, hidden_sectors) = if create_partition {
            // Partition starts at sector 2048 (1MB offset) for alignment
            let offset = 2048 * 512;
            let size = device.size - offset;
            (size, offset, 2048u32)
        } else {
            (device.size, 0u64, 0u32)
        };
        
        let total_sectors = partition_size / 512;
        let (sectors_per_cluster, sectors_per_fat, root_entries) = 
            Self::calculate_fat16_params(partition_size, options.cluster_size)?;
        
        info!("FAT16 parameters: {} sectors, {} sectors/cluster, {} sectors/FAT, {} root entries",
              total_sectors, sectors_per_cluster, sectors_per_fat, root_entries);
        
        // Create boot sector bytes
        let boot_sector_bytes = Self::create_boot_sector_bytes(
            device,
            total_sectors,
            sectors_per_cluster,
            sectors_per_fat,
            root_entries,
            hidden_sectors,
            options.label.as_deref(),
        );
        
        // Verify boot sector has correct data
        info!("Boot sector verification:");
        info!("  Jump: {:02X} {:02X} {:02X}", boot_sector_bytes[0], boot_sector_bytes[1], boot_sector_bytes[2]);
        info!("  Bytes per sector at 0x0B: {:04X}", u16::from_le_bytes([boot_sector_bytes[0x0B], boot_sector_bytes[0x0C]]));
        info!("  Sectors per cluster at 0x0D: {:02X}", boot_sector_bytes[0x0D]);
        info!("  Boot signature at 0x1FE: {:02X} {:02X}", boot_sector_bytes[0x1FE], boot_sector_bytes[0x1FF]);
        
        // Open device for writing using proper physical drive access
        use crate::utils::open_device_write;
        
        info!("Opening device for writing: {}", device.name);
        
        let mut file = open_device_write(device)?;
        
        // Write partition table if requested
        if create_partition {
            info!("Creating MBR partition table");
            
            use crate::partitioner::{create_single_partition_table, PartitionTableType, write_partition_table};
            
            let partition_table = create_single_partition_table(
                device,
                PartitionTableType::MBR,
                "fat16"
            )?;
            
            // Write the partition table
            write_partition_table(&mut file, &partition_table)?;
            
            // Use sync_all like FAT32 does - this is crucial!
            file.sync_all()
                .map_err(|e| MosesError::Other(format!("Failed to sync after partition write: {}", e)))?;
            
            info!("Partition table written and synced");
        }
        
        // Seek to partition start
        if partition_offset > 0 {
            info!("Writing FAT16 at offset {} (partition)", partition_offset);
            file.seek(SeekFrom::Start(partition_offset))
                .map_err(|e| MosesError::Other(format!("Failed to seek to partition start: {}", e)))?;
        } else {
            info!("Writing FAT16 at offset 0 (direct format, no partition table)");
        }
        
        // Write boot sector
        info!("Writing boot sector (512 bytes)");
        file.write_all(&boot_sector_bytes)
            .map_err(|e| MosesError::Other(format!("Failed to write boot sector: {}", e)))?;
        info!("Boot sector written successfully");
        
        // Create and write FAT tables
        let fat_size = sectors_per_fat as usize * 512;
        let mut fat = vec![0u8; fat_size];
        
        // FAT16 uses 16-bit entries. First two entries are reserved:
        // FAT[0] = Media descriptor in low byte, 0xFF in high byte
        // FAT[1] = End of chain marker (0xFFFF)
        let media_descriptor = if device.is_removable { 0xF0 } else { 0xF8 };
        
        // FAT[0]: 16-bit value with media descriptor
        let fat0_value: u16 = 0xFF00 | (media_descriptor as u16);
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
        
        // Initialize root directory with volume label
        use crate::fat16::root_directory::create_root_directory_with_label;
        let root_dir = create_root_directory_with_label(root_entries, options.label.as_deref());
        file.write_all(&root_dir)
            .map_err(|e| MosesError::Other(format!("Failed to write root directory: {}", e)))?;
        
        // Flush to ensure all data is written
        // Use sync_all for final sync, like FAT32 does
        file.sync_all()
            .map_err(|e| MosesError::Other(format!("Failed to sync: {}", e)))?;
        
        info!("FAT16 compliant format completed successfully");
        Ok(())
    }
}