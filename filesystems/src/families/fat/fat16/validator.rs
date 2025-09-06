// Consolidated FAT16 Validator - Combines the best features from all validators
// Provides both format-time validation and post-format verification

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::collections::HashMap;
use moses_core::MosesError;
use crate::families::fat::common::structures::Fat16BootSector;

// ============================================================================
// Format-time Parameter Validation (from validation.rs)
// ============================================================================

/// Validates FAT16 parameters before formatting and returns corrected values if needed
pub fn validate_format_params(
    size_bytes: u64,
    requested_cluster_size: Option<u32>,
) -> Result<(u8, u16, u16, String), MosesError> {
    let total_sectors = size_bytes / 512;
    
    // FAT16 absolute limits
    const MIN_FAT16_SECTORS: u64 = 4085 * 2;  // Minimum ~4MB
    const MAX_FAT16_SECTORS: u64 = 8_388_608; // Maximum 4GB
    
    if total_sectors < MIN_FAT16_SECTORS {
        return Err(MosesError::Other(format!(
            "Device too small for FAT16. Minimum size is {} MB, device has {} MB",
            MIN_FAT16_SECTORS * 512 / 1024 / 1024,
            size_bytes / 1024 / 1024
        )));
    }
    
    if total_sectors > MAX_FAT16_SECTORS {
        return Err(MosesError::Other(format!(
            "Device too large for FAT16. Maximum size is 4 GB, device has {} GB",
            size_bytes as f64 / 1024.0 / 1024.0 / 1024.0
        )));
    }
    
    // If user specified cluster size, validate it
    if let Some(cluster_bytes) = requested_cluster_size {
        let sectors_per_cluster = (cluster_bytes / 512) as u8;
        let (spf, re, msg) = calculate_fat_params(total_sectors, sectors_per_cluster)?;
        return Ok((sectors_per_cluster, spf, re, msg));
    }
    
    // Otherwise, find optimal cluster size
    let cluster_sizes = [2, 4, 8, 16, 32, 64, 128];
    for &sectors_per_cluster in &cluster_sizes {
        if let Ok((spf, re, msg)) = calculate_fat_params(total_sectors, sectors_per_cluster) {
            return Ok((sectors_per_cluster, spf, re, msg));
        }
    }
    
    Err(MosesError::Other("Could not find valid FAT16 configuration".to_string()))
}

fn calculate_fat_params(total_sectors: u64, sectors_per_cluster: u8) -> Result<(u16, u16, String), MosesError> {
    let reserved_sectors = 1u16;
    let num_fats = 2u8;
    let root_entries = 512u16;
    let root_dir_sectors = (root_entries * 32 + 511) / 512;
    
    // Iteratively calculate FAT size
    let mut sectors_per_fat = 1u16;
    loop {
        let data_start = reserved_sectors + (num_fats as u16 * sectors_per_fat) + root_dir_sectors;
        if data_start as u64 >= total_sectors {
            return Err(MosesError::Other("No space for data area".to_string()));
        }
        
        let data_sectors = total_sectors - data_start as u64;
        let total_clusters = data_sectors / sectors_per_cluster as u64;
        
        // Check if this is valid FAT16
        if total_clusters < 4085 || total_clusters > 65524 {
            return Err(MosesError::Other(format!(
                "Invalid cluster count {} for FAT16 (must be 4085-65524)",
                total_clusters
            )));
        }
        
        // Calculate required FAT size
        let required_fat_entries = total_clusters + 2;
        let required_fat_bytes = required_fat_entries * 2;
        let required_sectors_per_fat = ((required_fat_bytes + 511) / 512) as u16;
        
        if required_sectors_per_fat <= sectors_per_fat {
            let msg = format!(
                "Valid FAT16: {} clusters with {}KB cluster size",
                total_clusters,
                (sectors_per_cluster as u32) * 512 / 1024
            );
            return Ok((sectors_per_fat, root_entries, msg));
        }
        
        sectors_per_fat = required_sectors_per_fat;
        if sectors_per_fat > 256 {
            return Err(MosesError::Other("FAT table too large".to_string()));
        }
    }
}

// ============================================================================
// Post-format Filesystem Verification (best of verifier.rs & comprehensive)
// ============================================================================

#[derive(Debug)]
pub struct ValidationReport {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub info: HashMap<String, String>,
    pub cluster_info: ClusterInfo,
    pub windows_compatibility: WindowsCompatibility,
}

#[derive(Debug)]
pub struct ClusterInfo {
    pub total_clusters: u64,
    pub free_clusters: u64,
    pub bad_clusters: Vec<u16>,
    pub cluster_size_bytes: u32,
}

#[derive(Debug)]
pub struct WindowsCompatibility {
    pub drive_number_correct: bool,
    pub media_descriptor_correct: bool,
    pub volume_id_present: bool,
    pub oem_name_valid: bool,
}

// Use the common Fat16BootSector from fat_common::structures
// No need to redefine it here

pub struct Fat16Validator;

impl Fat16Validator {
    /// Comprehensive validation of a FAT16 filesystem
    pub fn validate(device_path: &str, partition_offset_sectors: Option<u64>) -> Result<ValidationReport, std::io::Error> {
        let mut file = File::open(device_path)?;
        let offset_bytes = partition_offset_sectors.unwrap_or(0) * 512;
        
        if offset_bytes > 0 {
            file.seek(SeekFrom::Start(offset_bytes))?;
        }
        
        // Read boot sector
        let mut boot_sector_bytes = [0u8; 512];
        file.read_exact(&mut boot_sector_bytes)?;
        
        // Parse boot sector
        let boot_sector = unsafe {
            std::ptr::read_unaligned(boot_sector_bytes.as_ptr() as *const Fat16BootSector)
        };
        
        let mut report = ValidationReport {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            info: HashMap::new(),
            cluster_info: ClusterInfo {
                total_clusters: 0,
                free_clusters: 0,
                bad_clusters: Vec::new(),
                cluster_size_bytes: 0,
            },
            windows_compatibility: WindowsCompatibility {
                drive_number_correct: true,
                media_descriptor_correct: true,
                volume_id_present: true,
                oem_name_valid: true,
            },
        };
        
        // Validate jump instruction
        Self::validate_jump_instruction(&boot_sector, &mut report);
        
        // Validate BPB fields
        Self::validate_bpb_fields(&boot_sector, &mut report);
        
        // Validate Windows compatibility
        Self::validate_windows_compatibility(&boot_sector, &mut report);
        
        // Check boot signature
        if boot_sector_bytes[510] != 0x55 || boot_sector_bytes[511] != 0xAA {
            report.errors.push("Invalid boot signature (should be 55 AA)".to_string());
            report.is_valid = false;
        }
        
        // Calculate cluster info
        Self::calculate_cluster_info(&boot_sector, &mut report);
        
        // Validate FAT tables
        Self::validate_fat_tables(&mut file, &boot_sector, &mut report)?;
        
        Ok(report)
    }
    
    fn validate_jump_instruction(boot: &Fat16BootSector, report: &mut ValidationReport) {
        match boot.common_bpb.jump_boot[0] {
            0xEB => {
                if boot.common_bpb.jump_boot[2] != 0x90 {
                    report.errors.push(format!(
                        "Invalid jump instruction: EB {:02X} {:02X} (third byte should be 90)",
                        boot.common_bpb.jump_boot[1], boot.common_bpb.jump_boot[2]
                    ));
                    report.is_valid = false;
                }
            },
            0xE9 => {
                // Near jump is valid
            },
            _ => {
                report.errors.push(format!(
                    "Invalid jump instruction: {:02X} (must be EB or E9)",
                    boot.common_bpb.jump_boot[0]
                ));
                report.is_valid = false;
            }
        }
    }
    
    fn validate_bpb_fields(boot: &Fat16BootSector, report: &mut ValidationReport) {
        // Copy values from packed struct to avoid alignment issues
        let bytes_per_sector = boot.common_bpb.bytes_per_sector;
        let sectors_per_cluster = boot.common_bpb.sectors_per_cluster;
        let reserved_sectors = boot.common_bpb.reserved_sectors;
        let num_fats = boot.common_bpb.num_fats;
        let root_entries = boot.common_bpb.root_entries;
        let sectors_per_fat = boot.common_bpb.sectors_per_fat_16;
        
        // Bytes per sector
        if ![512, 1024, 2048, 4096].contains(&bytes_per_sector) {
            report.errors.push(format!(
                "Invalid bytes per sector: {} (must be 512, 1024, 2048, or 4096)",
                bytes_per_sector
            ));
            report.is_valid = false;
        }
        
        // Sectors per cluster must be power of 2
        if !sectors_per_cluster.is_power_of_two() {
            report.errors.push(format!(
                "Invalid sectors per cluster: {} (must be power of 2)",
                sectors_per_cluster
            ));
            report.is_valid = false;
        }
        
        // Number of FATs
        if num_fats == 0 {
            report.errors.push("Number of FATs cannot be 0".to_string());
            report.is_valid = false;
        } else if num_fats != 2 {
            report.warnings.push(format!(
                "Non-standard number of FATs: {} (usually 2)",
                num_fats
            ));
        }
        
        // Root entries
        if root_entries == 0 {
            report.errors.push("Root entries cannot be 0 for FAT16".to_string());
            report.is_valid = false;
        } else if root_entries != 512 {
            report.warnings.push(format!(
                "Non-standard root entries: {} (usually 512)",
                root_entries
            ));
        }
        
        // File system type
        let fs_type = String::from_utf8_lossy(&boot.extended_bpb.fs_type);
        if !fs_type.starts_with("FAT16") && !fs_type.starts_with("FAT12") && !fs_type.starts_with("FAT") {
            report.warnings.push(format!(
                "Unexpected filesystem type: '{}' (expected 'FAT16   ')",
                fs_type
            ));
        }
        
        report.info.insert("bytes_per_sector".to_string(), bytes_per_sector.to_string());
        report.info.insert("sectors_per_cluster".to_string(), sectors_per_cluster.to_string());
        report.info.insert("reserved_sectors".to_string(), reserved_sectors.to_string());
        report.info.insert("num_fats".to_string(), num_fats.to_string());
        report.info.insert("root_entries".to_string(), root_entries.to_string());
        report.info.insert("sectors_per_fat".to_string(), sectors_per_fat.to_string());
    }
    
    fn validate_windows_compatibility(boot: &Fat16BootSector, report: &mut ValidationReport) {
        // Copy values from packed struct to avoid alignment issues
        let drive_number = boot.extended_bpb.drive_number;
        let media_descriptor = boot.common_bpb.media_descriptor;
        let boot_signature = boot.extended_bpb.boot_signature;
        let volume_id = boot.extended_bpb.volume_id;
        
        // OEM Name
        let oem_name = String::from_utf8_lossy(&boot.common_bpb.oem_name);
        let known_oem = ["MSWIN4.1", "MSDOS5.0", "FRDOS4.1", "IBM  3.3", "MOSES   "];
        if !known_oem.iter().any(|&s| oem_name.trim() == s) {
            report.warnings.push(format!(
                "Non-standard OEM name: '{}' (Windows uses 'MSWIN4.1')",
                oem_name
            ));
            report.windows_compatibility.oem_name_valid = false;
        }
        
        // Drive number (0x00 for removable, 0x80 for fixed)
        if drive_number != 0x00 && drive_number != 0x80 {
            report.warnings.push(format!(
                "Non-standard drive number: 0x{:02X} (should be 0x00 for removable or 0x80 for fixed)",
                drive_number
            ));
            report.windows_compatibility.drive_number_correct = false;
        }
        
        // Media descriptor
        if media_descriptor != 0xF0 && media_descriptor != 0xF8 {
            report.warnings.push(format!(
                "Non-standard media descriptor: 0x{:02X} (should be 0xF0 for removable or 0xF8 for fixed)",
                media_descriptor
            ));
            report.windows_compatibility.media_descriptor_correct = false;
        }
        
        // Boot signature
        if boot_signature != 0x29 {
            report.warnings.push(format!(
                "Missing extended boot signature: 0x{:02X} (should be 0x29)",
                boot_signature
            ));
        }
        
        // Volume ID
        if volume_id == 0 {
            report.warnings.push("Volume ID is 0 (should be unique)".to_string());
            report.windows_compatibility.volume_id_present = false;
        }
        
        report.info.insert("oem_name".to_string(), oem_name.to_string());
        report.info.insert("drive_number".to_string(), format!("0x{:02X}", drive_number));
        report.info.insert("media_descriptor".to_string(), format!("0x{:02X}", media_descriptor));
        report.info.insert("volume_id".to_string(), format!("0x{:08X}", volume_id));
        
        let volume_label = String::from_utf8_lossy(&boot.extended_bpb.volume_label);
        report.info.insert("volume_label".to_string(), format!("'{}'", volume_label.trim()));
    }
    
    fn calculate_cluster_info(boot: &Fat16BootSector, report: &mut ValidationReport) {
        // Copy values from packed struct
        let total_sectors_16 = boot.common_bpb.total_sectors_16;
        let total_sectors_32 = boot.common_bpb.total_sectors_32;
        let root_entries = boot.common_bpb.root_entries;
        let reserved_sectors = boot.common_bpb.reserved_sectors;
        let num_fats = boot.common_bpb.num_fats;
        let sectors_per_fat = boot.common_bpb.sectors_per_fat_16;
        let sectors_per_cluster = boot.common_bpb.sectors_per_cluster;
        let bytes_per_sector = boot.common_bpb.bytes_per_sector;
        
        let total_sectors = if total_sectors_16 != 0 {
            total_sectors_16 as u64
        } else {
            total_sectors_32 as u64
        };
        
        let root_dir_sectors = (root_entries as u64 * 32 + 511) / 512;
        let data_start = reserved_sectors as u64 
            + (num_fats as u64 * sectors_per_fat as u64) 
            + root_dir_sectors;
        
        let data_sectors = total_sectors.saturating_sub(data_start);
        let total_clusters = data_sectors / sectors_per_cluster as u64;
        
        report.cluster_info.total_clusters = total_clusters;
        report.cluster_info.cluster_size_bytes = sectors_per_cluster as u32 * bytes_per_sector as u32;
        
        // Validate cluster count for FAT16
        if total_clusters < 4085 {
            report.errors.push(format!(
                "Too few clusters for FAT16: {} (minimum is 4085)",
                total_clusters
            ));
            report.is_valid = false;
        } else if total_clusters > 65524 {
            report.errors.push(format!(
                "Too many clusters for FAT16: {} (maximum is 65524)",
                total_clusters
            ));
            report.is_valid = false;
        }
        
        report.info.insert("total_sectors".to_string(), total_sectors.to_string());
        report.info.insert("total_clusters".to_string(), total_clusters.to_string());
        report.info.insert("cluster_size".to_string(), 
            format!("{} bytes", report.cluster_info.cluster_size_bytes));
    }
    
    fn validate_fat_tables(
        file: &mut File,
        boot: &Fat16BootSector,
        report: &mut ValidationReport
    ) -> Result<(), std::io::Error> {
        // Copy values from packed struct
        let reserved_sectors = boot.common_bpb.reserved_sectors;
        let media_descriptor = boot.common_bpb.media_descriptor;
        let sectors_per_fat = boot.common_bpb.sectors_per_fat_16;
        
        // Seek to first FAT
        file.seek(SeekFrom::Start(reserved_sectors as u64 * 512))?;
        
        // Read first 4 bytes of FAT
        let mut fat_start = [0u8; 4];
        file.read_exact(&mut fat_start)?;
        
        // First FAT entry should be F8 FF FF FF or F0 FF FF FF
        let expected_first = if media_descriptor == 0xF8 { 0xF8 } else { 0xF0 };
        if fat_start[0] != expected_first {
            report.errors.push(format!(
                "FAT[0] low byte 0x{:02X} doesn't match media descriptor 0x{:02X}",
                fat_start[0], media_descriptor
            ));
            report.is_valid = false;
        }
        
        if fat_start[1] != 0xFF || fat_start[2] != 0xFF || fat_start[3] != 0xFF {
            report.errors.push(format!(
                "Invalid FAT header: {:02X} {:02X} {:02X} {:02X} (expected {:02X} FF FF FF)",
                fat_start[0], fat_start[1], fat_start[2], fat_start[3], expected_first
            ));
            report.is_valid = false;
        }
        
        // Count free clusters (simplified - just check for 0x0000 entries)
        let fat_size_bytes = sectors_per_fat as usize * 512;
        let mut fat_data = vec![0u8; fat_size_bytes];
        file.seek(SeekFrom::Start(reserved_sectors as u64 * 512))?;
        file.read_exact(&mut fat_data)?;
        
        let mut free_count = 0u64;
        for i in (4..fat_data.len()).step_by(2) {
            if i + 1 < fat_data.len() {
                let entry = u16::from_le_bytes([fat_data[i], fat_data[i + 1]]);
                if entry == 0 {
                    free_count += 1;
                }
            }
        }
        
        report.cluster_info.free_clusters = free_count;
        report.info.insert("free_clusters".to_string(), free_count.to_string());
        
        Ok(())
    }
    
    /// Quick check if a device contains a valid FAT16 filesystem
    pub fn is_fat16(device_path: &str) -> bool {
        Self::validate(device_path, None)
            .map(|r| r.is_valid)
            .unwrap_or(false)
    }
}

// ============================================================================
// Backwards compatibility exports
// ============================================================================

pub use self::validate_format_params as validate_and_fix_fat16_params;

// For the verifier.rs compatibility
pub type Fat16Verifier = Fat16Validator;
pub type VerificationResult = ValidationReport;