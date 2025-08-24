// Native exFAT formatter implementation
// Formats drives as exFAT without using external tools

use moses_core::{Device, MosesError, FormatOptions, FilesystemFormatter, SimulationReport, Platform};
use async_trait::async_trait;
use std::io::{Write, Seek, SeekFrom};
use log::info;
use crate::fat_common::generate_volume_serial;
use super::structures::*;
use super::bitmap::ExFatBitmap;
use super::upcase::generate_upcase_table;

pub struct ExFatNativeFormatter;

impl ExFatNativeFormatter {
    /// Calculate exFAT parameters based on volume size
    fn calculate_params(total_bytes: u64) -> ExFatParams {
        // Determine optimal cluster size based on volume size
        let sectors_per_cluster = match total_bytes {
            0..=256_000_000 => 8,           // <= 256MB: 4KB clusters
            256_000_001..=32_000_000_000 => 64,   // <= 32GB: 32KB clusters
            32_000_000_001..=256_000_000_000 => 256, // <= 256GB: 128KB clusters
            _ => 512,                        // > 256GB: 256KB clusters
        };
        
        let bytes_per_sector = 512;
        let bytes_per_cluster = sectors_per_cluster * bytes_per_sector;
        let total_sectors = total_bytes / bytes_per_sector as u64;
        
        // exFAT layout:
        // - Boot region (min 24 sectors)
        // - FAT region
        // - Data region
        
        // Use 128 sectors for boot region like Windows does (for alignment)
        // This is larger than the minimum 24 sectors but provides better alignment
        let boot_sectors = 128;
        let total_clusters = ((total_sectors - boot_sectors) * bytes_per_sector as u64) / bytes_per_cluster as u64;
        
        // FAT size: 4 bytes per cluster
        let fat_bytes = total_clusters * 4;
        let fat_sectors = (fat_bytes + bytes_per_sector as u64 - 1) / bytes_per_sector as u64;
        
        // Heap (bitmap + upcase table) goes in first clusters
        let bitmap_size = (total_clusters + 7) / 8;  // 1 bit per cluster
        let bitmap_clusters = (bitmap_size + bytes_per_cluster as u64 - 1) / bytes_per_cluster as u64;
        
        let upcase_size = 128 * 1024;  // 128KB for Unicode upcase table
        let upcase_clusters = (upcase_size + bytes_per_cluster as u64 - 1) / bytes_per_cluster as u64;
        
        let heap_clusters = bitmap_clusters + upcase_clusters;
        let usable_clusters = total_clusters - heap_clusters - 1;  // -1 for root directory
        
        ExFatParams {
            bytes_per_sector: bytes_per_sector as u32,
            sectors_per_cluster: sectors_per_cluster as u32,
            total_sectors,
            total_clusters: total_clusters as u32,
            fat_offset: boot_sectors,
            fat_length: fat_sectors as u32,
            cluster_heap_offset: boot_sectors + fat_sectors,
            cluster_count: usable_clusters as u32,
            first_cluster_of_root: (heap_clusters + 2) as u32,  // After bitmap and upcase
            _bitmap_start_cluster: 2,  // First data cluster
            bitmap_length: bitmap_clusters as u32,
            upcase_start_cluster: (2 + bitmap_clusters) as u32,
            _upcase_length: upcase_clusters as u32,
        }
    }
    
    /// Create exFAT boot sector
    fn create_boot_sector(params: &ExFatParams, volume_serial: u32, _label: Option<&str>) -> [u8; 512] {
        let mut boot = [0u8; 512];
        
        // Jump boot (3 bytes)
        boot[0] = 0xEB;
        boot[1] = 0x76;
        boot[2] = 0x90;
        
        // File system name (8 bytes)
        boot[3..11].copy_from_slice(b"EXFAT   ");
        
        // Must be zero (53 bytes)
        // Already zero-initialized
        
        // Partition offset (8 bytes) - offset 64
        boot[64..72].copy_from_slice(&0u64.to_le_bytes());
        
        // Volume length in SECTORS (8 bytes) - offset 72
        // Note: Despite some docs, this field is in sectors, not bytes!
        boot[72..80].copy_from_slice(&params.total_sectors.to_le_bytes());
        
        // FAT offset (4 bytes) - offset 80
        boot[80..84].copy_from_slice(&(params.fat_offset as u32).to_le_bytes());
        
        // FAT length (4 bytes) - offset 84
        boot[84..88].copy_from_slice(&params.fat_length.to_le_bytes());
        
        // Cluster heap offset (4 bytes) - offset 88
        boot[88..92].copy_from_slice(&(params.cluster_heap_offset as u32).to_le_bytes());
        
        // Cluster count (4 bytes) - offset 92
        boot[92..96].copy_from_slice(&params.cluster_count.to_le_bytes());
        
        // First cluster of root directory (4 bytes) - offset 96
        boot[96..100].copy_from_slice(&params.first_cluster_of_root.to_le_bytes());
        
        // Volume serial number (4 bytes) - offset 100
        boot[100..104].copy_from_slice(&volume_serial.to_le_bytes());
        
        // File system revision (2 bytes) - offset 104
        boot[104..106].copy_from_slice(&0x0100u16.to_le_bytes());  // Version 1.00
        
        // Volume flags (2 bytes) - offset 106
        // Bit 0: ActiveFAT (0 = first FAT, 1 = second FAT)
        // Bit 1: VolumeDirty (0 = clean, 1 = dirty)
        // Bit 2: MediaFailure (0 = no failures, 1 = failures reported)
        let volume_flags: u16 = 0x0000;  // Clean volume, first FAT active, no failures
        boot[106..108].copy_from_slice(&volume_flags.to_le_bytes());
        
        // Bytes per sector shift (1 byte) - offset 108
        boot[108] = params.bytes_per_sector.trailing_zeros() as u8;
        
        // Sectors per cluster shift (1 byte) - offset 109
        boot[109] = params.sectors_per_cluster.trailing_zeros() as u8;
        
        // Number of FATs (1 byte) - offset 110
        boot[110] = 1;  // exFAT typically uses 1 FAT
        
        // Drive select (1 byte) - offset 111
        boot[111] = 0x80;  // Hard disk
        
        // Percent in use (1 byte) - offset 112
        boot[112] = 0;  // 0% used initially
        
        // Reserved (7 bytes) - offset 113
        // Already zero
        
        // Boot code (390 bytes) - offset 120
        // We'll leave this empty
        
        // Boot signature - offset 510
        boot[510] = 0x55;
        boot[511] = 0xAA;
        
        boot
    }
    
    /// Calculate boot region checksum according to exFAT specification
    fn calculate_boot_checksum(boot_sector: &[u8], oem_params: &[u8]) -> u32 {
        let mut checksum = 0u32;
        
        // Process boot sector (sector 0)
        for i in 0..512 {
            // Skip VolumeFlags (106-107) and PercentInUse (112)
            if i == 106 || i == 107 || i == 112 {
                continue;
            }
            // Official algorithm: if LSB is 1, add 0x80000000 after shift
            checksum = if (checksum & 1) != 0 { 0x80000000 } else { 0 }
                + (checksum >> 1) 
                + (boot_sector[i] as u32);
        }
        
        // Process Extended Boot Sectors + OEM parameters (sectors 1-9)
        for &byte in oem_params {
            checksum = if (checksum & 1) != 0 { 0x80000000 } else { 0 }
                + (checksum >> 1)
                + (byte as u32);
        }
        
        // Process reserved sector (sector 10) - all zeros
        for _ in 0..512 {
            checksum = if (checksum & 1) != 0 { 0x80000000 } else { 0 }
                + (checksum >> 1);
            // Adding 0, so just the rotate operation
        }
        
        checksum
    }
    
    /// Create root directory with volume label
    fn create_root_directory(label: Option<&str>, params: &ExFatParams, upcase_checksum: u32) -> Vec<u8> {
        let mut entries = Vec::new();
        
        // Volume label entry (if provided)
        if let Some(label) = label {
            let mut label_entry = ExFatDirectoryEntry::default();
            unsafe {
                label_entry.generic.entry_type = EXFAT_ENTRY_VOLUME_LABEL;
                
                // Convert label to UTF-16LE
                let label_utf16: Vec<u16> = label.chars().take(11).map(|c| c as u16).collect();
                label_entry.label.character_count = label_utf16.len() as u8;
                
                for (i, &ch) in label_utf16.iter().enumerate() {
                    if i < 11 {  // Max 11 chars in volume label
                        label_entry.label.volume_label[i] = ch;
                    }
                }
            }
            
            entries.extend_from_slice(&label_entry.to_bytes());
        }
        
        // Volume GUID entry (required by Windows)
        let mut guid_entry = ExFatDirectoryEntry::default();
        unsafe {
            guid_entry.volume_guid.entry_type = EXFAT_ENTRY_VOLUME_GUID;
            guid_entry.volume_guid.secondary_count = 0;
            guid_entry.volume_guid.set_checksum = 0;  // Not used for GUID entry
            guid_entry.volume_guid.flags = 0;
            
            // Generate a random GUID using the volume serial as seed
            use crate::fat_common::generate_volume_serial;
            let serial = generate_volume_serial();
            
            // Create a simple GUID based on the serial number
            // Format: XXXXXXXX-XXXX-4XXX-8XXX-XXXXXXXXXXXX (version 4 random UUID)
            guid_entry.volume_guid.volume_guid[0..4].copy_from_slice(&serial.to_le_bytes());
            guid_entry.volume_guid.volume_guid[4..6].copy_from_slice(&[0x12, 0x34]);
            guid_entry.volume_guid.volume_guid[6] = 0x40 | (serial as u8 & 0x0F);  // Version 4
            guid_entry.volume_guid.volume_guid[7] = serial.wrapping_shr(8) as u8;
            guid_entry.volume_guid.volume_guid[8] = 0x80 | (serial.wrapping_shr(16) as u8 & 0x3F);  // Variant
            guid_entry.volume_guid.volume_guid[9] = serial.wrapping_shr(24) as u8;
            // Fill remaining bytes
            for i in 10..16 {
                guid_entry.volume_guid.volume_guid[i] = ((serial.wrapping_mul(i as u32 + 1)) & 0xFF) as u8;
            }
        }
        
        entries.extend_from_slice(&guid_entry.to_bytes());
        
        // Bitmap allocation entry
        let mut bitmap_entry = ExFatDirectoryEntry::default();
        bitmap_entry.bitmap.entry_type = EXFAT_ENTRY_BITMAP;
        bitmap_entry.bitmap.flags = 0;
        bitmap_entry.bitmap.first_cluster = 2;  // Bitmap starts at cluster 2
        bitmap_entry.bitmap.data_length = ((params.cluster_count + 7) / 8) as u64;
        entries.extend_from_slice(&bitmap_entry.to_bytes());
        
        // Upcase table entry
        let mut upcase_entry = ExFatDirectoryEntry::default();
        upcase_entry.upcase.entry_type = EXFAT_ENTRY_UPCASE;
        upcase_entry.upcase.table_checksum = upcase_checksum;
        upcase_entry.upcase.first_cluster = params.upcase_start_cluster;
        upcase_entry.upcase.data_length = 128 * 1024;  // 128KB
        entries.extend_from_slice(&upcase_entry.to_bytes());
        
        // Add a test file to demonstrate directory entry sets (optional)
        // This helps verify the filesystem is working
        if cfg!(debug_assertions) {
            use super::directory_entries::DirectoryEntrySetBuilder;
            
            let test_file = DirectoryEntrySetBuilder::new_file("README.TXT")
                .size(46)  // Small test message
                .first_cluster(0)  // No cluster allocated (empty file)
                .build();
            
            for entry in test_file {
                entries.extend_from_slice(&entry.to_bytes());
            }
            
            info!("Added test file README.TXT to root directory");
        }
        
        // Pad to cluster size
        let cluster_size = params.sectors_per_cluster * params.bytes_per_sector;
        while entries.len() < cluster_size as usize {
            entries.extend_from_slice(&[0u8; 32]);  // Empty entries
        }
        
        entries
    }
    
    async fn write_exfat_to_file(
        file: &mut std::fs::File,
        volume_label: Option<&str>,
        write_offset: u64,
        partition_size: u64,
    ) -> Result<(), MosesError> {
        let params = Self::calculate_params(partition_size);
        let volume_serial = generate_volume_serial();
        
        info!("exFAT parameters: {} total sectors, {} sectors/cluster, {} total clusters",
              params.total_sectors, params.sectors_per_cluster, params.total_clusters);
        
        // 1. Write main boot sector
        let boot_sector = Self::create_boot_sector(&params, volume_serial, volume_label);
        file.seek(SeekFrom::Start(write_offset))?;
        file.write_all(&boot_sector)?;
        info!("Wrote exFAT boot sector");
        
        // 2. Write Extended Boot Sectors (sectors 1-8)
        let extended_boot = vec![0u8; 8 * 512];
        file.write_all(&extended_boot)?;
        info!("Wrote Extended Boot Sectors");
        
        // 3. Write OEM Parameters (sector 9)
        let oem_params = vec![0u8; 512];
        file.write_all(&oem_params)?;
        info!("Wrote OEM parameters");
        
        // 4. Write Reserved sector (sector 10)
        file.write_all(&vec![0u8; 512])?;
        info!("Wrote reserved sector");
        
        // 5. Calculate and write boot checksum (sector 11)
        // Checksum covers sectors 0-10 (boot + extended + OEM + reserved)
        let mut all_boot_data = Vec::new();
        all_boot_data.extend_from_slice(&extended_boot);
        all_boot_data.extend_from_slice(&oem_params);
        let checksum = Self::calculate_boot_checksum(&boot_sector, &all_boot_data);
        let mut checksum_sector = vec![0u8; 512];
        for i in 0..128 {
            let offset = i * 4;
            checksum_sector[offset..offset + 4].copy_from_slice(&checksum.to_le_bytes());
        }
        file.write_all(&checksum_sector)?;
        info!("Wrote boot checksum: 0x{:08X}", checksum);
        
        // 6. Write backup boot region (sectors 12-23)
        file.seek(SeekFrom::Start(write_offset + 12 * 512))?;
        file.write_all(&boot_sector)?;     // Backup boot sector
        file.write_all(&extended_boot)?;   // Backup extended boot sectors
        file.write_all(&oem_params)?;      // Backup OEM params
        file.write_all(&vec![0u8; 512])?;  // Backup reserved
        file.write_all(&checksum_sector)?; // Backup checksum
        info!("Wrote backup boot region");
        
        // 7. Initialize FAT
        let fat_offset = write_offset + (params.fat_offset * params.bytes_per_sector as u64);
        file.seek(SeekFrom::Start(fat_offset))?;
        
        // Build FAT in memory first to write in sector-aligned chunks
        let mut fat_buffer = Vec::new();
        
        // First two FAT entries (each is 32-bit in exFAT)
        fat_buffer.extend_from_slice(&0xFFFFFFF8u32.to_le_bytes());  // Entry 0: Media descriptor
        fat_buffer.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());  // Entry 1: End of chain
        
        // Mark system clusters as allocated
        // Cluster 2: Bitmap (contiguous allocation)
        for _i in 0..params.bitmap_length {
            fat_buffer.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());  // End of chain
        }
        
        // Upcase table clusters (contiguous allocation)
        let upcase_clusters = (128 * 1024) / (params.sectors_per_cluster * params.bytes_per_sector);
        for _i in 0..upcase_clusters {
            fat_buffer.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());  // End of chain
        }
        
        // Root directory cluster
        fat_buffer.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());  // End of chain
        
        // Pad to sector boundary for Windows raw device access
        while fat_buffer.len() % 512 != 0 {
            fat_buffer.push(0);
        }
        
        file.write_all(&fat_buffer)?;
        info!("Initialized FAT");
        
        // 7. Write allocation bitmap
        let bitmap_offset = write_offset + (params.cluster_heap_offset * params.bytes_per_sector as u64);
        let mut bitmap = ExFatBitmap::new(params.cluster_count);
        
        // Mark system clusters as allocated (clusters start at 2 in exFAT)
        // Bitmap clusters
        for i in 0..params.bitmap_length {
            bitmap.set_allocated(i);
        }
        // Upcase table clusters  
        let upcase_clusters = (128 * 1024) / (params.sectors_per_cluster * params.bytes_per_sector);
        for i in 0..upcase_clusters {
            bitmap.set_allocated(params.bitmap_length + i);
        }
        // Root directory cluster
        bitmap.set_allocated(params.first_cluster_of_root - 2);  // Convert to 0-based index
        
        // Get bitmap data and pad to cluster size for proper alignment
        let mut bitmap_data = bitmap.to_bytes();
        let cluster_size = (params.sectors_per_cluster * params.bytes_per_sector) as usize;
        while bitmap_data.len() % cluster_size != 0 {
            bitmap_data.push(0);
        }
        
        file.seek(SeekFrom::Start(bitmap_offset))?;
        file.write_all(&bitmap_data)?;
        info!("Wrote allocation bitmap");
        
        // 8. Write upcase table
        let upcase_offset = bitmap_offset + 
            (params.bitmap_length as u64 * params.sectors_per_cluster as u64 * params.bytes_per_sector as u64);
        let upcase_table = generate_upcase_table();
        let upcase_checksum = super::upcase::calculate_upcase_checksum(&upcase_table);
        
        // Upcase table is already 128KB which should be cluster-aligned
        // But ensure it's padded if needed
        let mut upcase_data = upcase_table;
        while upcase_data.len() % cluster_size != 0 {
            upcase_data.push(0);
        }
        
        file.seek(SeekFrom::Start(upcase_offset))?;
        file.write_all(&upcase_data)?;
        info!("Wrote Unicode upcase table (checksum: 0x{:08X})", upcase_checksum);
        
        // 9. Write root directory
        let root_offset = bitmap_offset + 
            ((params.first_cluster_of_root - 2) as u64 * params.sectors_per_cluster as u64 * params.bytes_per_sector as u64);
        let root_dir = Self::create_root_directory(volume_label, &params, upcase_checksum);
        
        // Root directory is already padded to cluster size in create_root_directory
        file.seek(SeekFrom::Start(root_offset))?;
        file.write_all(&root_dir)?;
        info!("Wrote root directory");
        
        file.flush()?;
        Ok(())
    }
}

#[async_trait]
impl FilesystemFormatter for ExFatNativeFormatter {
    fn name(&self) -> &'static str {
        "exFAT (Native)"
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        vec![Platform::Windows, Platform::Linux, Platform::MacOS]
    }
    
    fn can_format(&self, _device: &Device) -> bool {
        true  // Can format any device
    }
    
    fn requires_external_tools(&self) -> bool {
        false  // Native implementation, no external tools needed
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        vec![]  // No external tools
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        if let Some(label) = &options.label {
            if label.len() > 15 {
                return Err(MosesError::InvalidInput(
                    "exFAT volume label must be 15 characters or less".to_string()
                ));
            }
        }
        Ok(())
    }
    
    async fn dry_run(&self, device: &Device, options: &FormatOptions) -> Result<SimulationReport, MosesError> {
        let report = SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: std::time::Duration::from_secs(5),
            warnings: vec!["All data on the device will be lost".to_string()],
            required_tools: vec![],
            will_erase_data: true,
            space_after_format: device.size,
        };
        
        Ok(report)
    }
    
    async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        use crate::utils::open_device_write;
        
        info!("Starting native exFAT format of device: {}", device.name);
        
        // For now, we'll format the whole device without partitioning
        // TODO: Add partition table support in FormatOptions
        let write_offset = 0u64;
        let partition_size = device.size;
        
        // Open device for writing (uses physical drive path, not drive letter)
        let mut file = open_device_write(device)?;
        
        // Format the partition/device as exFAT
        Self::write_exfat_to_file(&mut file, options.label.as_deref(), write_offset, partition_size).await?;
        
        info!("Successfully formatted device as exFAT");
        Ok(())
    }
}

/// exFAT filesystem parameters
struct ExFatParams {
    bytes_per_sector: u32,
    sectors_per_cluster: u32,
    total_sectors: u64,
    total_clusters: u32,
    fat_offset: u64,
    fat_length: u32,
    cluster_heap_offset: u64,
    cluster_count: u32,
    first_cluster_of_root: u32,
    _bitmap_start_cluster: u32,
    bitmap_length: u32,
    upcase_start_cluster: u32,
    _upcase_length: u32,
}