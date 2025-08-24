// FAT16 formatter with full specification compliance
// Fixes all known issues with Windows recognition

use moses_core::{Device, MosesError, FormatOptions, FilesystemFormatter, SimulationReport, Platform};
use async_trait::async_trait;
use std::io::{Write, Seek, SeekFrom};
use log::{info, warn};
use crate::fat_common::{
    Fat16BootSector, generate_volume_serial, format_volume_label,
    init_fat16_table, write_fat_tables, get_media_descriptor
};

pub struct Fat16CompliantFormatter;

impl Fat16CompliantFormatter {
    fn calculate_fat16_params(size_bytes: u64, requested_cluster_size: Option<u32>) -> Result<(u8, u16, u16), MosesError> {
        // Use the validated parameter calculation
        use super::validator::validate_format_params;
        
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
            validate_format_params(size_bytes, requested_cluster_size)?;
        
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
        // Create a proper FAT16 boot sector using the common structure
        let mut boot_sector = Fat16BootSector::new();
        
        // Set OEM name to Windows 4.1 for compatibility
        boot_sector.common_bpb.oem_name = *b"MSWIN4.1";
        
        // Set BPB fields
        boot_sector.common_bpb.bytes_per_sector = 512;
        boot_sector.common_bpb.sectors_per_cluster = sectors_per_cluster;
        boot_sector.common_bpb.reserved_sectors = 1;
        boot_sector.common_bpb.num_fats = 2;
        boot_sector.common_bpb.root_entries = root_entries;
        
        // Total sectors
        if total_sectors < 65536 {
            boot_sector.common_bpb.total_sectors_16 = total_sectors as u16;
            boot_sector.common_bpb.total_sectors_32 = 0;
        } else {
            boot_sector.common_bpb.total_sectors_16 = 0;
            boot_sector.common_bpb.total_sectors_32 = total_sectors as u32;
        }
        
        // Media descriptor: 0xF0 for removable, 0xF8 for fixed
        boot_sector.common_bpb.media_descriptor = get_media_descriptor(device.is_removable);
        
        // FAT16-specific fields
        boot_sector.common_bpb.sectors_per_fat_16 = sectors_per_fat;
        boot_sector.common_bpb.sectors_per_track = 63;
        boot_sector.common_bpb.num_heads = 255;
        boot_sector.common_bpb.hidden_sectors = hidden_sectors;
        
        // Extended BPB fields
        boot_sector.extended_bpb.drive_number = if device.is_removable { 0x00 } else { 0x80 };
        boot_sector.extended_bpb.reserved = 0;
        boot_sector.extended_bpb.boot_signature = 0x29;
        boot_sector.extended_bpb.volume_id = generate_volume_serial();
        
        // Volume label
        boot_sector.extended_bpb.volume_label = format_volume_label(volume_label);
        
        // File system type
        boot_sector.extended_bpb.fs_type = *b"FAT16   ";
        
        // Boot signature is already set by Fat16BootSector::new()
        
        // Convert the structure to bytes
        let mut bytes = vec![0u8; 512];
        unsafe {
            let boot_sector_ptr = &boot_sector as *const Fat16BootSector as *const u8;
            let boot_sector_bytes = std::slice::from_raw_parts(boot_sector_ptr, std::mem::size_of::<Fat16BootSector>());
            bytes[..boot_sector_bytes.len()].copy_from_slice(boot_sector_bytes);
        }
        
        bytes
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
        
        // Create and initialize FAT tables using common helper
        let fat_size = sectors_per_fat as usize * 512;
        let mut fat = vec![0u8; fat_size];
        
        // Use common FAT initialization
        let media_descriptor = get_media_descriptor(device.is_removable);
        init_fat16_table(&mut fat, media_descriptor);
        
        // Write FAT tables using common helper
        write_fat_tables(
            &mut file,
            &fat,
            1,  // FAT starts at sector 1 (after boot sector)
            sectors_per_fat as u32,
            2,  // Number of FATs
            512 // Bytes per sector
        ).map_err(|e| MosesError::Other(format!("Failed to write FAT tables: {}", e)))?;
        
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