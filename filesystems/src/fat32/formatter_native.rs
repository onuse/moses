// Native FAT32 formatter implementation
// Uses shared FAT components for maximum code reuse

use moses_core::{Device, MosesError, FormatOptions, FilesystemFormatter, SimulationReport, Platform};
use async_trait::async_trait;
use std::io::{Write, Seek, SeekFrom};
use log::info;
#[cfg(target_os = "windows")]
use log::warn;
use crate::fat_common::{
    Fat32BootSector, generate_volume_serial, format_volume_label,
    init_fat32_table, get_media_descriptor,
    calculate_fat32_params,
    FAT32_ROOT_CLUSTER, FAT32_FS_INFO_SECTOR, FAT32_BACKUP_BOOT_SECTOR
};

pub struct Fat32NativeFormatter;

impl Fat32NativeFormatter {
    /// Create FSInfo sector
    fn create_fsinfo_sector(free_clusters: u32, next_free: u32) -> [u8; 512] {
        let mut fsinfo = [0u8; 512];
        
        // Lead signature "RRaA" at offset 0
        fsinfo[0..4].copy_from_slice(&0x41615252u32.to_le_bytes());
        
        // Reserved area (480 bytes of zeros)
        // Already zero-initialized
        
        // Struct signature "rrAa" at offset 484
        fsinfo[484..488].copy_from_slice(&0x61417272u32.to_le_bytes());
        
        // Free cluster count at offset 488
        // 0xFFFFFFFF means unknown - we'll use actual count
        fsinfo[488..492].copy_from_slice(&free_clusters.to_le_bytes());
        
        // Next free cluster hint at offset 492
        // Start searching from cluster 3 (after root directory)
        fsinfo[492..496].copy_from_slice(&next_free.to_le_bytes());
        
        // Reserved (12 bytes)
        // Already zero
        
        // Trail signature at offset 508
        fsinfo[508..512].copy_from_slice(&[0x00, 0x00, 0x55, 0xAA]);
        
        fsinfo
    }
    
    async fn write_fat32_to_file(
        file: &mut std::fs::File,
        volume_label: Option<&str>,
        write_offset: u64,
        partition_size: u64,
    ) -> Result<(), MosesError> {
        // Calculate FAT32 parameters
        let total_sectors = partition_size / 512;
        let fat_params = calculate_fat32_params(total_sectors)?;
        
        info!("FAT32 parameters: {} sectors, {} sectors/cluster, {} sectors/FAT, {} total clusters",
              total_sectors, fat_params.sectors_per_cluster, 
              fat_params.sectors_per_fat, fat_params.total_clusters);
        
        // Create boot sector using the common structure
        let mut boot_sector = Fat32BootSector::new();
        
        // Set common BPB fields
        boot_sector.common_bpb.oem_name = *b"MSWIN4.1";
        boot_sector.common_bpb.bytes_per_sector = 512;
        boot_sector.common_bpb.sectors_per_cluster = fat_params.sectors_per_cluster;
        boot_sector.common_bpb.reserved_sectors = 32;  // FAT32 typically uses 32
        boot_sector.common_bpb.num_fats = 2;
        boot_sector.common_bpb.root_entries = 0;  // FAT32 has no fixed root
        boot_sector.common_bpb.total_sectors_16 = 0;
        boot_sector.common_bpb.media_descriptor = get_media_descriptor(false);  // Fixed disk
        boot_sector.common_bpb.sectors_per_fat_16 = 0;  // Not used in FAT32
        boot_sector.common_bpb.sectors_per_track = 63;
        boot_sector.common_bpb.num_heads = 255;
        boot_sector.common_bpb.hidden_sectors = (write_offset / 512) as u32;
        boot_sector.common_bpb.total_sectors_32 = total_sectors as u32;
        
        // Set FAT32-specific fields
        boot_sector.extended_bpb.sectors_per_fat_32 = fat_params.sectors_per_fat;
        boot_sector.extended_bpb.ext_flags = 0;
        boot_sector.extended_bpb.fs_version = 0;
        boot_sector.extended_bpb.root_cluster = FAT32_ROOT_CLUSTER;
        boot_sector.extended_bpb.fs_info = FAT32_FS_INFO_SECTOR;
        boot_sector.extended_bpb.backup_boot_sector = FAT32_BACKUP_BOOT_SECTOR;
        boot_sector.extended_bpb.drive_number = 0x80;  // Fixed disk
        boot_sector.extended_bpb.boot_signature = 0x29;
        boot_sector.extended_bpb.volume_id = generate_volume_serial();
        boot_sector.extended_bpb.volume_label = format_volume_label(volume_label);
        boot_sector.extended_bpb.fs_type = *b"FAT32   ";
        
        // Convert to bytes
        let boot_sector_bytes = unsafe {
            std::slice::from_raw_parts(
                &boot_sector as *const Fat32BootSector as *const u8,
                std::mem::size_of::<Fat32BootSector>()
            )
        };
        
        // Write boot sector
        file.seek(SeekFrom::Start(write_offset))?;
        file.write_all(boot_sector_bytes)?;
        info!("Wrote FAT32 boot sector at offset {}", write_offset);
        
        // Create and write FSInfo sector
        let free_clusters = fat_params.total_clusters - 1;  // -1 for root directory
        let fsinfo = Self::create_fsinfo_sector(free_clusters, 3);
        file.seek(SeekFrom::Start(write_offset + 512))?;  // Sector 1
        file.write_all(&fsinfo)?;
        info!("Wrote FSInfo sector");
        
        // Write backup boot sector at sector 6
        file.seek(SeekFrom::Start(write_offset + (6 * 512)))?;
        file.write_all(boot_sector_bytes)?;
        info!("Wrote backup boot sector");
        
        // Write backup FSInfo at sector 7
        file.seek(SeekFrom::Start(write_offset + (7 * 512)))?;
        file.write_all(&fsinfo)?;
        info!("Wrote backup FSInfo sector");
        
        // Write FAT tables with proper sector alignment for Windows physical drives
        let fat_offset = write_offset + (boot_sector.common_bpb.reserved_sectors as u64 * 512);
        
        // Write each FAT table
        for fat_num in 0..boot_sector.common_bpb.num_fats {
            let this_fat_offset = fat_offset + (fat_num as u64 * fat_params.sectors_per_fat as u64 * 512);
            
            // Seek to FAT start
            file.seek(SeekFrom::Start(this_fat_offset))?;
            
            // Initialize FAT with zeros in sector-sized chunks
            let fat_size = fat_params.sectors_per_fat as usize * 512;
            let zeros = vec![0u8; fat_size.min(1024 * 1024)];  // Write in 1MB chunks
            let mut remaining = fat_size;
            while remaining > 0 {
                let chunk_size = remaining.min(zeros.len());
                file.write_all(&zeros[..chunk_size])?;
                remaining -= chunk_size;
            }
            
            // Initialize FAT with proper reserved entries using common helper
            if fat_num == 0 {
                // Only initialize the first FAT, then copy it
                file.seek(SeekFrom::Start(this_fat_offset))?;
                let mut first_sector = vec![0u8; 512];
                init_fat32_table(&mut first_sector, get_media_descriptor(false), FAT32_ROOT_CLUSTER);
                file.write_all(&first_sector)?;
            }
        }
        
        info!("Wrote {} FAT32 tables", boot_sector.common_bpb.num_fats);
        
        // Initialize root directory cluster (cluster 2)
        // Root directory starts after FATs
        let data_offset = fat_offset + (boot_sector.common_bpb.num_fats as u64 * fat_params.sectors_per_fat as u64 * 512);
        let root_dir_offset = data_offset;  // Cluster 2 is the first data cluster
        
        // Clear root directory cluster
        file.seek(SeekFrom::Start(root_dir_offset))?;
        let empty_cluster = vec![0u8; boot_sector.common_bpb.sectors_per_cluster as usize * 512];
        file.write_all(&empty_cluster)?;
        info!("Initialized root directory cluster");
        
        // Sync to disk
        file.sync_all()?;
        info!("FAT32 format completed successfully");
        
        Ok(())
    }
}

#[async_trait]
impl FilesystemFormatter for Fat32NativeFormatter {
    fn name(&self) -> &'static str {
        "fat32"
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        vec![Platform::Windows, Platform::Linux, Platform::MacOS]
    }
    
    fn can_format(&self, device: &Device) -> bool {
        if device.is_system {
            return false;
        }
        
        // FAT32 max size is technically 2TB (with 512-byte sectors)
        // Some implementations support up to 8TB with 4096-byte sectors
        device.size <= 2 * 1024_u64.pow(4)
    }
    
    fn requires_external_tools(&self) -> bool {
        false  // Native implementation
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        vec![]
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        if options.filesystem_type != "fat32" {
            return Err(MosesError::Other("Invalid filesystem type for FAT32 formatter".to_string()));
        }
        
        if let Some(ref label) = options.label {
            if label.len() > 11 {
                return Err(MosesError::InvalidInput("FAT32 label maximum is 11 characters".to_string()));
            }
        }
        
        Ok(())
    }
    
    async fn dry_run(&self, device: &Device, options: &FormatOptions) -> Result<SimulationReport, MosesError> {
        let fat_params = calculate_fat32_params(device.size / 512)?;
        
        let fat_size = fat_params.sectors_per_fat as u64 * 512 * 2;  // 2 FATs
        let reserved_size = 32 * 512;  // 32 reserved sectors
        let overhead = reserved_size + fat_size;
        
        let mut warnings = vec![];
        
        // Windows 32GB limitation warning
        #[cfg(target_os = "windows")]
        {
            if device.size > 32 * 1024_u64.pow(3) {
                warnings.push("Note: Windows typically limits FAT32 formatting to 32GB".to_string());
                warnings.push("Moses can format larger drives as FAT32".to_string());
            }
        }
        
        if device.size < 260 * 1024 * 1024 {
            warnings.push("Volume may be too small for FAT32 (minimum ~260MB)".to_string());
        }
        
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: std::time::Duration::from_secs(5),
            warnings,
            required_tools: vec![],
            will_erase_data: true,
            space_after_format: device.size - overhead,
        })
    }
    
    async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        self.validate_options(options).await?;
        
        if !self.can_format(device) {
            return Err(MosesError::UnsafeDevice(
                "Device cannot be formatted (system device or too large)".to_string()
            ));
        }
        
        info!("Starting native FAT32 format for device: {}", device.name);
        
        // On Windows, cleanup the disk first (dismount volumes)
        #[cfg(target_os = "windows")]
        {
            if let Some(drive_number) = crate::ext4_native::windows::get_drive_number_from_path(&device.id) {
                info!("Cleaning up disk {} before FAT32 format", drive_number);
                if let Err(e) = crate::ext4_native::windows::cleanup_disk_for_format(drive_number) {
                    warn!("Disk cleanup warning: {}", e);
                    // Continue anyway - the open might still work
                }
            }
        }
        
        // Check if we should create a partition table
        let create_partition_table = options.additional_options
            .get("create_partition_table")
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(false);
        
        // Open device for writing using the utility function (physical drive, not volume)
        let mut file = crate::utils::open_device_write(device)?;
        
        if create_partition_table {
            info!("Creating MBR partition table for FAT32");
            
            // Create MBR with FAT32 partition
            use crate::partitioner::{create_single_partition_table, PartitionTableType, write_partition_table};
            
            let partition_table = create_single_partition_table(
                device,
                PartitionTableType::MBR,
                "fat32"
            )?;
            
            // Write the partition table
            write_partition_table(&mut file, &partition_table)?;
            file.sync_all().map_err(|e| MosesError::IoError(e))?;
            
            // Write FAT32 at partition offset (typically 1MB)
            let partition_offset = 1024 * 1024;  // 1MB aligned
            let partition_size = device.size - partition_offset;
            
            // Use the same file handle to write FAT32
            Self::write_fat32_to_file(
                &mut file,
                options.label.as_deref(),
                partition_offset,
                partition_size,
            ).await?;
        } else {
            // Write FAT32 directly to device (no partition table)
            info!("Formatting device directly as FAT32 (no partition table)");
            
            Self::write_fat32_to_file(
                &mut file,
                options.label.as_deref(),
                0,
                device.size,
            ).await?;
        }
        
        // Final sync
        file.sync_all().map_err(|e| MosesError::IoError(e))?;
        
        Ok(())
    }
}