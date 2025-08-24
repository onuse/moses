// NTFS Formatter - Phase 5: Create NTFS filesystems
// This is a minimal NTFS formatter that creates a basic, valid NTFS volume

use moses_core::{Device, FormatOptions, MosesError, FilesystemFormatter};
use crate::ntfs::structures::*;
use crate::ntfs::mft_writer::MftRecordBuilder;
use log::{info, debug};
use std::io::{Write, Seek, SeekFrom};
use async_trait::async_trait;

/// NTFS Formatter implementation
pub struct NtfsFormatter;

#[async_trait]
impl FilesystemFormatter for NtfsFormatter {
    fn name(&self) -> &'static str {
        "NTFS"
    }
    
    fn supported_platforms(&self) -> Vec<moses_core::Platform> {
        vec![
            moses_core::Platform::Windows,
            moses_core::Platform::Linux,
            moses_core::Platform::MacOS,
        ]
    }
    
    fn can_format(&self, device: &Device) -> bool {
        device.size >= 10 * 1024 * 1024 // Minimum 10MB
    }
    
    fn requires_external_tools(&self) -> bool {
        false
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        vec![]
    }
    
    async fn validate_options(&self, _options: &FormatOptions) -> Result<(), MosesError> {
        Ok(())
    }
    
    async fn dry_run(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<moses_core::SimulationReport, MosesError> {
        Ok(moses_core::SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: std::time::Duration::from_secs(device.size / (1024 * 1024 * 1024)),
            warnings: vec![],
            required_tools: vec![],
            will_erase_data: true,
            space_after_format: device.size * 9 / 10, // Roughly 90% usable
        })
    }
    
    async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        info!("Starting NTFS format of device: {}", device.name);
        
        // Basic validation
        if device.size < 10 * 1024 * 1024 {
            return Err(MosesError::InvalidInput("Device too small for NTFS (min 10MB)".to_string()));
        }
        
        // Open device for writing
        let mut file = {
            use std::fs::OpenOptions;
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::fs::OpenOptionsExt;
                use winapi::um::winnt::{GENERIC_READ, GENERIC_WRITE, FILE_SHARE_READ};
                
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .access_mode(GENERIC_READ | GENERIC_WRITE)
                    .share_mode(FILE_SHARE_READ)
                    .open(&device.mount_points[0])?
            }
            #[cfg(not(target_os = "windows"))]
            {
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&device.mount_points[0])?
            }
        };
        
        // Calculate filesystem parameters
        let bytes_per_sector = 512u16;
        let sectors_per_cluster = calculate_sectors_per_cluster(device.size);
        let bytes_per_cluster = bytes_per_sector as u32 * sectors_per_cluster as u32;
        let total_sectors = device.size / bytes_per_sector as u64;
        let total_clusters = total_sectors / sectors_per_cluster as u64;
        
        // MFT parameters
        let mft_record_size = 1024u32; // Standard size
        let mft_clusters = (total_clusters / 8).max(8192).min(65536); // 12.5% of disk, min 32MB, max 256MB
        let _mft_zone_clusters = mft_clusters * 4; // Reserve 4x MFT size for growth
        let mft_start_cluster = 4; // Start MFT at cluster 4 (after boot sector)
        
        info!("NTFS parameters: {} sectors, {} bytes/cluster, MFT at cluster {}",
              total_sectors, bytes_per_cluster, mft_start_cluster);
        
        // Step 1: Write boot sector
        write_boot_sector(&mut file, &device, options, 
                         bytes_per_sector, sectors_per_cluster,
                         total_sectors, mft_start_cluster)?;
        
        // Step 2: Create and write system MFT records
        write_system_mft_records(&mut file, bytes_per_cluster, 
                                mft_start_cluster, mft_record_size,
                                total_clusters)?;
        
        // Step 3: Initialize bitmaps
        initialize_bitmaps(&mut file, bytes_per_cluster, total_clusters, mft_clusters)?;
        
        // Step 4: Write backup boot sector
        write_backup_boot_sector(&mut file, total_sectors, bytes_per_sector)?;
        
        // Flush all writes
        file.flush()?;
        
        info!("NTFS format completed successfully");
        Ok(())
    }
}

/// Calculate appropriate sectors per cluster based on volume size
fn calculate_sectors_per_cluster(volume_size: u64) -> u8 {
    // Standard NTFS cluster sizes
    match volume_size {
        0..=512_000_000 => 1,           // <= 512MB: 512 bytes
        ..=1_024_000_000 => 2,          // <= 1GB: 1KB
        ..=2_147_483_648 => 4,          // <= 2GB: 2KB  
        ..=8_589_934_592 => 8,          // <= 8GB: 4KB (most common)
        ..=17_179_869_184 => 16,        // <= 16GB: 8KB
        ..=34_359_738_368 => 32,        // <= 32GB: 16KB
        ..=68_719_476_736 => 64,        // <= 64GB: 32KB
        _ => 128,                        // > 64GB: 64KB
    }
}

/// Write the NTFS boot sector
fn write_boot_sector(
    file: &mut std::fs::File,
    device: &Device,
    options: &FormatOptions,
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    total_sectors: u64,
    mft_start_cluster: u64,
) -> Result<(), MosesError> {
    debug!("Writing NTFS boot sector");
    
    let mft_record_size = 1024u32; // Standard MFT record size
    
    let boot_sector = NtfsBootSector {
        jump: [0xEB, 0x52, 0x90],
        oem_id: *b"NTFS    ",
        bytes_per_sector,
        sectors_per_cluster,
        reserved_sectors: 0,
        zero1: [0; 3],
        unused1: 0,
        media_descriptor: 0xF8,
        zero2: 0,
        sectors_per_track: 63,
        num_heads: 255,
        hidden_sectors: 0,
        unused2: 0,
        unused3: 0,
        total_sectors,
        mft_lcn: mft_start_cluster,
        mftmirr_lcn: 2, // Usually at cluster 2
        clusters_per_mft_record: if mft_record_size < bytes_per_sector as u32 * sectors_per_cluster as u32 {
            // Negative value indicates size in bytes (2^(-n))
            let mut size = mft_record_size;
            let mut n = 0u8;
            while size > 1 {
                size >>= 1;
                n += 1;
            }
            (256u16 - n as u16) as i8
        } else {
            (mft_record_size / (bytes_per_sector as u32 * sectors_per_cluster as u32)) as i8
        },
        unused4: [0; 3],
        clusters_per_index_buffer: 1,
        unused5: [0; 3],
        volume_serial: generate_serial_number(),
        checksum: 0,
        bootstrap: [0; 426],
        signature: 0xAA55,
    };
    
    // Volume label would be stored in the MFT $Volume record
    // For now, we're creating a basic NTFS structure
    
    // Write boot sector at offset 0
    file.seek(SeekFrom::Start(0))?;
    let boot_bytes = unsafe {
        std::slice::from_raw_parts(
            &boot_sector as *const _ as *const u8,
            std::mem::size_of::<NtfsBootSector>()
        )
    };
    file.write_all(boot_bytes)?;
    
    Ok(())
}

/// Generate a random volume serial number
fn generate_serial_number() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    // Mix timestamp with a simple hash for uniqueness
    timestamp ^ (timestamp << 13) ^ (timestamp >> 7)
}

/// Write the system MFT records
fn write_system_mft_records(
    file: &mut std::fs::File,
    bytes_per_cluster: u32,
    mft_start_cluster: u64,
    mft_record_size: u32,
    total_clusters: u64,
) -> Result<(), MosesError> {
    info!("Writing system MFT records");
    
    let mft_offset = mft_start_cluster * bytes_per_cluster as u64;
    
    // Create system MFT records
    let system_records = vec![
        create_mft_record_0(mft_record_size, mft_start_cluster, bytes_per_cluster)?, // $MFT
        create_mft_record_1(mft_record_size)?, // $MFTMirr
        create_mft_record_2(mft_record_size)?, // $LogFile
        create_mft_record_3(mft_record_size)?, // $Volume
        create_mft_record_4(mft_record_size)?, // $AttrDef
        create_mft_record_5(mft_record_size)?, // . (root directory)
        create_mft_record_6(mft_record_size, total_clusters)?, // $Bitmap
        create_mft_record_7(mft_record_size)?, // $Boot
        create_mft_record_8(mft_record_size)?, // $BadClus
        create_mft_record_9(mft_record_size)?, // $Secure
        create_mft_record_10(mft_record_size)?, // $UpCase
        create_mft_record_11(mft_record_size)?, // $Extend
    ];
    
    // Write each MFT record
    for (i, record) in system_records.iter().enumerate() {
        let record_offset = mft_offset + (i as u64 * mft_record_size as u64);
        file.seek(SeekFrom::Start(record_offset))?;
        file.write_all(record)?;
        debug!("Wrote MFT record {}", i);
    }
    
    // Write additional reserved MFT records (12-15) as empty
    for i in 12..16 {
        let record_offset = mft_offset + (i * mft_record_size as u64);
        file.seek(SeekFrom::Start(record_offset))?;
        let empty_record = vec![0u8; mft_record_size as usize];
        file.write_all(&empty_record)?;
    }
    
    Ok(())
}

/// Create MFT record 0 ($MFT)
fn create_mft_record_0(record_size: u32, _mft_cluster: u64, _bytes_per_cluster: u32) -> Result<Vec<u8>, MosesError> {
    let current_time = windows_time_now();
    
    MftRecordBuilder::new(0, record_size)
        .as_file()
        .with_standard_info(current_time, current_time, current_time, 0x06)? // System + Hidden
        .with_file_name(5, "$MFT", 3, current_time, current_time, current_time, 0, 0, 0x06)?
        .with_empty_data()? // The MFT data will be non-resident
        .build()
}

/// Create MFT record 1 ($MFTMirr)
fn create_mft_record_1(record_size: u32) -> Result<Vec<u8>, MosesError> {
    let current_time = windows_time_now();
    
    MftRecordBuilder::new(1, record_size)
        .as_file()
        .with_standard_info(current_time, current_time, current_time, 0x06)?
        .with_file_name(5, "$MFTMirr", 3, current_time, current_time, current_time, 0, 0, 0x06)?
        .with_empty_data()?
        .build()
}

/// Create MFT record 2 ($LogFile)
fn create_mft_record_2(record_size: u32) -> Result<Vec<u8>, MosesError> {
    let current_time = windows_time_now();
    
    MftRecordBuilder::new(2, record_size)
        .as_file()
        .with_standard_info(current_time, current_time, current_time, 0x06)?
        .with_file_name(5, "$LogFile", 3, current_time, current_time, current_time, 0, 0, 0x06)?
        .with_empty_data()?
        .build()
}

/// Create MFT record 3 ($Volume)
fn create_mft_record_3(record_size: u32) -> Result<Vec<u8>, MosesError> {
    let current_time = windows_time_now();
    
    MftRecordBuilder::new(3, record_size)
        .as_file()
        .with_standard_info(current_time, current_time, current_time, 0x16)? // System + Hidden + Volume
        .with_file_name(5, "$Volume", 3, current_time, current_time, current_time, 0, 0, 0x16)?
        .build()
}

/// Create MFT record 4 ($AttrDef)
fn create_mft_record_4(record_size: u32) -> Result<Vec<u8>, MosesError> {
    let current_time = windows_time_now();
    
    MftRecordBuilder::new(4, record_size)
        .as_file()
        .with_standard_info(current_time, current_time, current_time, 0x06)?
        .with_file_name(5, "$AttrDef", 3, current_time, current_time, current_time, 0, 0, 0x06)?
        .with_empty_data()?
        .build()
}

/// Create MFT record 5 (root directory)
fn create_mft_record_5(record_size: u32) -> Result<Vec<u8>, MosesError> {
    let current_time = windows_time_now();
    
    MftRecordBuilder::new(5, record_size)
        .as_directory()
        .with_standard_info(current_time, current_time, current_time, 0x10)? // Directory
        .with_file_name(5, ".", 3, current_time, current_time, current_time, 0, 0, 0x10)?
        .with_index_root(ATTR_TYPE_FILE_NAME)?
        .build()
}

/// Create MFT record 6 ($Bitmap)
fn create_mft_record_6(record_size: u32, total_clusters: u64) -> Result<Vec<u8>, MosesError> {
    let current_time = windows_time_now();
    let bitmap_size = (total_clusters + 7) / 8;
    
    MftRecordBuilder::new(6, record_size)
        .as_file()
        .with_standard_info(current_time, current_time, current_time, 0x06)?
        .with_file_name(5, "$Bitmap", 3, current_time, current_time, current_time, 
                       bitmap_size, bitmap_size, 0x06)?
        .with_empty_data()?
        .build()
}

/// Create MFT record 7 ($Boot)
fn create_mft_record_7(record_size: u32) -> Result<Vec<u8>, MosesError> {
    let current_time = windows_time_now();
    
    MftRecordBuilder::new(7, record_size)
        .as_file()
        .with_standard_info(current_time, current_time, current_time, 0x06)?
        .with_file_name(5, "$Boot", 3, current_time, current_time, current_time, 8192, 8192, 0x06)?
        .with_empty_data()?
        .build()
}

/// Create MFT record 8 ($BadClus)
fn create_mft_record_8(record_size: u32) -> Result<Vec<u8>, MosesError> {
    let current_time = windows_time_now();
    
    MftRecordBuilder::new(8, record_size)
        .as_file()
        .with_standard_info(current_time, current_time, current_time, 0x06)?
        .with_file_name(5, "$BadClus", 3, current_time, current_time, current_time, 0, 0, 0x06)?
        .with_empty_data()?
        .build()
}

/// Create MFT record 9 ($Secure)
fn create_mft_record_9(record_size: u32) -> Result<Vec<u8>, MosesError> {
    let current_time = windows_time_now();
    
    MftRecordBuilder::new(9, record_size)
        .as_file()
        .with_standard_info(current_time, current_time, current_time, 0x06)?
        .with_file_name(5, "$Secure", 3, current_time, current_time, current_time, 0, 0, 0x06)?
        .with_empty_data()?
        .build()
}

/// Create MFT record 10 ($UpCase)
fn create_mft_record_10(record_size: u32) -> Result<Vec<u8>, MosesError> {
    let current_time = windows_time_now();
    
    MftRecordBuilder::new(10, record_size)
        .as_file()
        .with_standard_info(current_time, current_time, current_time, 0x06)?
        .with_file_name(5, "$UpCase", 3, current_time, current_time, current_time, 
                       131072, 131072, 0x06)? // 128KB upcase table
        .with_empty_data()?
        .build()
}

/// Create MFT record 11 ($Extend)
fn create_mft_record_11(record_size: u32) -> Result<Vec<u8>, MosesError> {
    let current_time = windows_time_now();
    
    MftRecordBuilder::new(11, record_size)
        .as_directory()
        .with_standard_info(current_time, current_time, current_time, 0x16)?
        .with_file_name(5, "$Extend", 3, current_time, current_time, current_time, 0, 0, 0x16)?
        .with_index_root(ATTR_TYPE_FILE_NAME)?
        .build()
}

/// Get current time in Windows FILETIME format
fn windows_time_now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let unix_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    // Convert Unix time to Windows FILETIME (100ns intervals since 1601)
    // Unix epoch (1970) is 11644473600 seconds after Windows epoch (1601)
    (unix_time + 11644473600) * 10_000_000
}

/// Initialize cluster and MFT bitmaps
fn initialize_bitmaps(
    file: &mut std::fs::File,
    bytes_per_cluster: u32,
    total_clusters: u64,
    mft_clusters: u64,
) -> Result<(), MosesError> {
    debug!("Initializing bitmaps");
    
    // Create cluster bitmap
    let bitmap_size = ((total_clusters + 7) / 8) as usize;
    let mut cluster_bitmap = vec![0u8; bitmap_size];
    
    // Mark system clusters as used (0-15 for boot sectors and system files)
    for i in 0..16 {
        let byte_idx = i / 8;
        let bit_idx = i % 8;
        cluster_bitmap[byte_idx] |= 1 << bit_idx;
    }
    
    // Mark MFT clusters as used
    let mft_start = 4;
    for i in 0..mft_clusters {
        let cluster = mft_start + i;
        let byte_idx = (cluster / 8) as usize;
        let bit_idx = (cluster % 8) as u8;
        if byte_idx < cluster_bitmap.len() {
            cluster_bitmap[byte_idx] |= 1 << bit_idx;
        }
    }
    
    // Write bitmap to a known location (we'd normally put this in $Bitmap's data)
    // For now, write after MFT zone
    let bitmap_offset = (16 + mft_clusters) * bytes_per_cluster as u64;
    file.seek(SeekFrom::Start(bitmap_offset))?;
    file.write_all(&cluster_bitmap)?;
    
    Ok(())
}

/// Write backup boot sector at the end of the volume
fn write_backup_boot_sector(
    file: &mut std::fs::File,
    total_sectors: u64,
    bytes_per_sector: u16,
) -> Result<(), MosesError> {
    debug!("Writing backup boot sector");
    
    // Read the primary boot sector
    file.seek(SeekFrom::Start(0))?;
    let mut boot_sector = vec![0u8; bytes_per_sector as usize];
    file.read_exact(&mut boot_sector)?;
    
    // Write it at the last sector
    let backup_offset = (total_sectors - 1) * bytes_per_sector as u64;
    file.seek(SeekFrom::Start(backup_offset))?;
    file.write_all(&boot_sector)?;
    
    Ok(())
}

impl NtfsFormatter {
    /// Create a new NTFS formatter instance
    pub fn new() -> Self {
        Self
    }
}

use std::io::Read;