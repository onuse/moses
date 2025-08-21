// FAT16 Filesystem Verifier - Validates FAT16 compliance
use std::io::{Read, Seek, SeekFrom};
use std::fs::File;
use log::{info, warn, error};

/// FAT16 Boot Sector structure according to Microsoft specification
#[repr(C, packed(1))]
#[derive(Debug, Clone, Copy)]
pub struct Fat16BootSector {
    pub jump_boot: [u8; 3],         // 0x00: Jump instruction (EB xx 90 or E9 xx xx)
    pub oem_name: [u8; 8],          // 0x03: OEM name (padded with spaces)
    pub bytes_per_sector: u16,      // 0x0B: Bytes per sector (must be 512, 1024, 2048, or 4096)
    pub sectors_per_cluster: u8,    // 0x0D: Sectors per cluster (must be power of 2)
    pub reserved_sectors: u16,      // 0x0E: Reserved sectors (usually 1)
    pub num_fats: u8,              // 0x10: Number of FATs (usually 2)
    pub root_entries: u16,         // 0x11: Root directory entries (typically 512)
    pub total_sectors_16: u16,     // 0x13: Total sectors if < 65536
    pub media_descriptor: u8,      // 0x15: Media descriptor (F8 for hard disk)
    pub sectors_per_fat: u16,      // 0x16: Sectors per FAT
    pub sectors_per_track: u16,    // 0x18: Sectors per track (geometry)
    pub num_heads: u16,            // 0x1A: Number of heads (geometry)
    pub hidden_sectors: u32,       // 0x1C: Hidden sectors before partition
    pub total_sectors_32: u32,     // 0x20: Total sectors if >= 65536
    pub drive_number: u8,          // 0x24: BIOS drive number (0x80 for hard disk)
    pub reserved: u8,              // 0x25: Reserved (should be 0)
    pub boot_signature: u8,        // 0x26: Extended boot signature (0x29)
    pub volume_id: u32,            // 0x27: Volume serial number
    pub volume_label: [u8; 11],    // 0x2B: Volume label
    pub fs_type: [u8; 8],          // 0x36: File system type "FAT16   "
}

#[derive(Debug)]
pub struct VerificationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub info: Vec<String>,
}

impl VerificationResult {
    fn new() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            info: Vec::new(),
        }
    }

    fn add_error(&mut self, msg: String) {
        self.is_valid = false;
        self.errors.push(msg);
        error!("FAT16 Verification Error: {}", self.errors.last().unwrap());
    }

    fn add_warning(&mut self, msg: String) {
        self.warnings.push(msg);
        warn!("FAT16 Verification Warning: {}", self.warnings.last().unwrap());
    }

    fn add_info(&mut self, msg: String) {
        self.info.push(msg);
        info!("FAT16 Verification Info: {}", self.info.last().unwrap());
    }
}

pub struct Fat16Verifier;

impl Fat16Verifier {
    /// Verify a FAT16 filesystem on a device or file
    pub fn verify_filesystem(path: &str) -> Result<VerificationResult, std::io::Error> {
        let mut file = File::open(path)?;
        let mut result = VerificationResult::new();
        
        // Read boot sector
        let boot_sector = Self::read_boot_sector(&mut file)?;
        
        // Verify boot sector
        Self::verify_boot_sector(&boot_sector, &mut result);
        
        // Verify FAT tables
        Self::verify_fat_tables(&mut file, &boot_sector, &mut result)?;
        
        // Verify root directory
        Self::verify_root_directory(&mut file, &boot_sector, &mut result)?;
        
        // Calculate and verify cluster count
        Self::verify_cluster_count(&boot_sector, &mut result);
        
        Ok(result)
    }
    
    fn read_boot_sector(file: &mut File) -> Result<Fat16BootSector, std::io::Error> {
        file.seek(SeekFrom::Start(0))?;
        
        let mut buffer = [0u8; 512];
        file.read_exact(&mut buffer)?;
        
        // Check boot signature
        if buffer[510] != 0x55 || buffer[511] != 0xAA {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid boot signature (should be 0x55AA)"
            ));
        }
        
        // Copy to struct
        unsafe {
            Ok(std::ptr::read(buffer.as_ptr() as *const Fat16BootSector))
        }
    }
    
    fn verify_boot_sector(bs: &Fat16BootSector, result: &mut VerificationResult) {
        result.add_info("Verifying FAT16 Boot Sector...".to_string());
        
        // Check jump instruction
        if !(bs.jump_boot[0] == 0xEB || bs.jump_boot[0] == 0xE9) {
            result.add_error(format!(
                "Invalid jump instruction: {:02X} {:02X} {:02X} (should start with EB or E9)",
                bs.jump_boot[0], bs.jump_boot[1], bs.jump_boot[2]
            ));
        } else if bs.jump_boot[0] == 0xEB && bs.jump_boot[2] != 0x90 {
            result.add_warning(format!(
                "Non-standard NOP in jump: {:02X} (usually 0x90)",
                bs.jump_boot[2]
            ));
        }
        
        // Check OEM name
        let oem = String::from_utf8_lossy(&bs.oem_name);
        result.add_info(format!("OEM Name: '{}'", oem.trim()));
        
        // Verify bytes per sector
        if ![512, 1024, 2048, 4096].contains(&{ bs.bytes_per_sector }) {
            result.add_error(format!(
                "Invalid bytes per sector: {} (must be 512, 1024, 2048, or 4096)",
                { bs.bytes_per_sector }
            ));
        } else if { bs.bytes_per_sector } != 512 {
            result.add_warning(format!(
                "Non-standard sector size: {} (512 is most compatible)",
                { bs.bytes_per_sector }
            ));
        }
        
        // Verify sectors per cluster
        if !{ bs.sectors_per_cluster }.is_power_of_two() || { bs.sectors_per_cluster } == 0 {
            result.add_error(format!(
                "Invalid sectors per cluster: {} (must be power of 2)",
                { bs.sectors_per_cluster }
            ));
        }
        
        // Check cluster size doesn't exceed 32KB (FAT16 limit)
        let cluster_size = { bs.bytes_per_sector } as u32 * { bs.sectors_per_cluster } as u32;
        if cluster_size > 32768 {
            result.add_error(format!(
                "Cluster size {} exceeds 32KB limit for FAT16",
                cluster_size
            ));
        }
        
        // Verify reserved sectors
        if { bs.reserved_sectors } == 0 {
            result.add_error("Reserved sectors cannot be 0".to_string());
        } else if { bs.reserved_sectors } != 1 {
            result.add_warning(format!(
                "Non-standard reserved sectors: {} (usually 1)",
                { bs.reserved_sectors }
            ));
        }
        
        // Verify number of FATs
        if { bs.num_fats } == 0 {
            result.add_error("Number of FATs cannot be 0".to_string());
        } else if { bs.num_fats } != 2 {
            result.add_warning(format!(
                "Non-standard FAT count: {} (usually 2)",
                { bs.num_fats }
            ));
        }
        
        // Verify root entries
        if { bs.root_entries } == 0 {
            result.add_error("Root entries cannot be 0 for FAT16".to_string());
        } else if { bs.root_entries } != 512 {
            result.add_warning(format!(
                "Non-standard root entries: {} (usually 512)",
                { bs.root_entries }
            ));
        }
        
        // Check that root entries * 32 is divisible by bytes_per_sector
        if ({ bs.root_entries } as u32 * 32) % { bs.bytes_per_sector } as u32 != 0 {
            result.add_error(format!(
                "Root directory size {} not aligned to sector size {}",
                { bs.root_entries } * 32, { bs.bytes_per_sector }
            ));
        }
        
        // Verify total sectors
        if { bs.total_sectors_16 } == 0 && { bs.total_sectors_32 } == 0 {
            result.add_error("Total sectors not specified".to_string());
        } else if { bs.total_sectors_16 } != 0 && { bs.total_sectors_32 } != 0 {
            result.add_error("Both 16-bit and 32-bit sector counts specified".to_string());
        }
        
        let total_sectors = if { bs.total_sectors_16 } != 0 {
            ({ bs.total_sectors_16 }) as u64
        } else {
            ({ bs.total_sectors_32 }) as u64
        };
        
        result.add_info(format!("Total sectors: {}", total_sectors));
        
        // Verify media descriptor
        if ![0xF0, 0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF].contains(&{ bs.media_descriptor }) {
            result.add_warning(format!(
                "Unusual media descriptor: 0x{:02X} (0xF8 is standard for hard disk)",
                { bs.media_descriptor }
            ));
        }
        
        // Verify extended boot signature
        if { bs.boot_signature } == 0x29 {
            result.add_info(format!("Volume ID: {:08X}", { bs.volume_id }));
            let label = String::from_utf8_lossy(&bs.volume_label);
            result.add_info(format!("Volume Label: '{}'", label.trim()));
            let fs_type = String::from_utf8_lossy(&bs.fs_type);
            
            // Check filesystem type string
            if !fs_type.starts_with("FAT16") && !fs_type.starts_with("FAT12") && !fs_type.starts_with("FAT") {
                result.add_warning(format!(
                    "Unexpected filesystem type string: '{}' (expected 'FAT16   ')",
                    fs_type.trim()
                ));
            }
        } else if { bs.boot_signature } != 0 {
            result.add_warning(format!(
                "Invalid extended boot signature: 0x{:02X} (should be 0x29 or 0x00)",
                { bs.boot_signature }
            ));
        }
        
        // Verify geometry (for compatibility)
        if { bs.sectors_per_track } == 0 || { bs.num_heads } == 0 {
            result.add_warning("Disk geometry not specified (may cause compatibility issues)".to_string());
        }
    }
    
    fn verify_fat_tables(
        file: &mut File,
        bs: &Fat16BootSector,
        result: &mut VerificationResult
    ) -> Result<(), std::io::Error> {
        result.add_info("Verifying FAT tables...".to_string());
        
        let fat_start = { bs.reserved_sectors } as u64 * { bs.bytes_per_sector } as u64;
        let fat_size = { bs.sectors_per_fat } as u64 * { bs.bytes_per_sector } as u64;
        
        // Read first FAT
        file.seek(SeekFrom::Start(fat_start))?;
        let mut fat1 = vec![0u8; fat_size as usize];
        file.read_exact(&mut fat1)?;
        
        // Check FAT[0] - should contain media descriptor in low byte
        if fat1[0] != { bs.media_descriptor } {
            result.add_error(format!(
                "FAT[0] low byte 0x{:02X} doesn't match media descriptor 0x{:02X}",
                fat1[0], { bs.media_descriptor }
            ));
        }
        
        // Check FAT[0] high byte - should be 0xFF
        if fat1[1] != 0xFF {
            result.add_warning(format!(
                "FAT[0] high byte is 0x{:02X} (should be 0xFF)",
                fat1[1]
            ));
        }
        
        // Check FAT[1] - should be 0xFFFF (end of chain marker)
        if fat1[2] != 0xFF || fat1[3] != 0xFF {
            result.add_warning(format!(
                "FAT[1] is 0x{:02X}{:02X} (should be 0xFFFF)",
                fat1[3], fat1[2]
            ));
        }
        
        // If there are multiple FATs, verify they're identical
        let num_fats = { bs.num_fats };
        if num_fats > 1 {
            for i in 1..num_fats {
                file.seek(SeekFrom::Start(fat_start + (i as u64 * fat_size)))?;
                let mut fat_n = vec![0u8; fat_size as usize];
                file.read_exact(&mut fat_n)?;
                
                if fat_n != fat1 {
                    result.add_error(format!("FAT {} doesn't match FAT 0", i));
                    break;
                }
            }
            
            if num_fats == 2 && !result.errors.iter().any(|e| e.contains("doesn't match")) {
                result.add_info("Both FAT tables are identical (good)".to_string());
            }
        }
        
        Ok(())
    }
    
    fn verify_root_directory(
        file: &mut File,
        bs: &Fat16BootSector,
        result: &mut VerificationResult
    ) -> Result<(), std::io::Error> {
        result.add_info("Verifying root directory...".to_string());
        
        let root_start = ({ bs.reserved_sectors } as u64 + 
                         ({ bs.num_fats } as u64 * { bs.sectors_per_fat } as u64)) * 
                         { bs.bytes_per_sector } as u64;
        let root_size = { bs.root_entries } as u64 * 32;
        
        file.seek(SeekFrom::Start(root_start))?;
        let mut root_dir = vec![0u8; root_size as usize];
        file.read_exact(&mut root_dir)?;
        
        // Check if root directory is empty (all zeros is valid)
        let is_empty = root_dir.iter().all(|&b| b == 0);
        if is_empty {
            result.add_info("Root directory is empty (valid)".to_string());
        } else {
            // Parse directory entries
            let mut valid_entries = 0;
            let mut deleted_entries = 0;
            
            let root_entries = { bs.root_entries } as usize;
            for i in 0..root_entries {
                let entry = &root_dir[i * 32..(i + 1) * 32];
                
                if entry[0] == 0x00 {
                    // End of directory
                    break;
                } else if entry[0] == 0xE5 {
                    // Deleted entry
                    deleted_entries += 1;
                } else if entry[11] == 0x0F {
                    // Long filename entry
                    valid_entries += 1;
                } else {
                    // Regular entry
                    valid_entries += 1;
                    
                    // Check for volume label
                    if entry[11] & 0x08 != 0 {
                        let label = String::from_utf8_lossy(&entry[0..11]);
                        result.add_info(format!("Found volume label in root: '{}'", label.trim()));
                    }
                }
            }
            
            result.add_info(format!(
                "Root directory: {} valid entries, {} deleted entries",
                valid_entries, deleted_entries
            ));
        }
        
        Ok(())
    }
    
    fn verify_cluster_count(bs: &Fat16BootSector, result: &mut VerificationResult) {
        result.add_info("Verifying cluster count for FAT16...".to_string());
        
        let total_sectors = if { bs.total_sectors_16 } != 0 {
            ({ bs.total_sectors_16 }) as u64
        } else {
            ({ bs.total_sectors_32 }) as u64
        };
        
        let root_dir_sectors = (({ bs.root_entries } as u64 * 32) + 
                               ({ bs.bytes_per_sector } as u64 - 1)) / 
                               { bs.bytes_per_sector } as u64;
        
        let data_sectors = total_sectors - 
                          ({ bs.reserved_sectors } as u64 + 
                           ({ bs.num_fats } as u64 * { bs.sectors_per_fat } as u64) + 
                           root_dir_sectors);
        
        let total_clusters = data_sectors / { bs.sectors_per_cluster } as u64;
        
        result.add_info(format!("Total clusters: {}", total_clusters));
        
        // FAT16 must have between 4085 and 65524 clusters
        if total_clusters < 4085 {
            result.add_error(format!(
                "Too few clusters for FAT16: {} (minimum 4085)",
                total_clusters
            ));
            result.add_info("This filesystem should be FAT12".to_string());
        } else if total_clusters > 65524 {
            result.add_error(format!(
                "Too many clusters for FAT16: {} (maximum 65524)",
                total_clusters
            ));
            result.add_info("This filesystem should be FAT32".to_string());
        } else {
            result.add_info(format!(
                "Cluster count {} is valid for FAT16 (4085-65524)",
                total_clusters
            ));
        }
        
        // Check FAT can address all clusters
        let fat_entries = ({ bs.sectors_per_fat } as u64 * { bs.bytes_per_sector } as u64) / 2;
        if fat_entries < total_clusters + 2 {
            result.add_error(format!(
                "FAT too small: {} entries but {} clusters",
                fat_entries, total_clusters
            ));
        }
    }
    
    /// Generate a detailed report of the verification
    pub fn generate_report(result: &VerificationResult) -> String {
        let mut report = String::new();
        
        report.push_str("FAT16 Filesystem Verification Report\n");
        report.push_str("=====================================\n\n");
        
        report.push_str(&format!("Overall Status: {}\n\n", 
            if result.is_valid { "VALID" } else { "INVALID" }
        ));
        
        if !result.errors.is_empty() {
            report.push_str("ERRORS:\n");
            for error in &result.errors {
                report.push_str(&format!("  ✗ {}\n", error));
            }
            report.push_str("\n");
        }
        
        if !result.warnings.is_empty() {
            report.push_str("WARNINGS:\n");
            for warning in &result.warnings {
                report.push_str(&format!("  ⚠ {}\n", warning));
            }
            report.push_str("\n");
        }
        
        if !result.info.is_empty() {
            report.push_str("INFORMATION:\n");
            for info in &result.info {
                report.push_str(&format!("  ℹ {}\n", info));
            }
        }
        
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_power_of_two() {
        assert!(1u8.is_power_of_two());
        assert!(2u8.is_power_of_two());
        assert!(4u8.is_power_of_two());
        assert!(8u8.is_power_of_two());
        assert!(16u8.is_power_of_two());
        assert!(32u8.is_power_of_two());
        assert!(64u8.is_power_of_two());
        assert!(128u8.is_power_of_two());
        
        assert!(!3u8.is_power_of_two());
        assert!(!5u8.is_power_of_two());
        assert!(!6u8.is_power_of_two());
        assert!(!7u8.is_power_of_two());
    }
}