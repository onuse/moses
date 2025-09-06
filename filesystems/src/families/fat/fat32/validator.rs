// FAT32-specific validator implementation
// Uses the common validation framework for shared checks

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::collections::HashMap;
use crate::families::fat::common::{
    validator::{
        ValidationResult, ValidationStatus, FatValidator,
        BootSectorValidator, CommonBootSectorValidation,
        calculate_cluster_count
    },
    constants::*,
};

pub struct Fat32Validator;

impl FatValidator for Fat32Validator {
    fn fat_type(&self) -> &'static str {
        "FAT32"
    }
    
    fn validate_specific_fields(&self, boot_sector: &[u8]) -> HashMap<String, ValidationResult> {
        let mut results = HashMap::new();
        
        // FAT32-specific BPB fields
        results.insert("root_entries".to_string(), self.validate_root_entries(boot_sector));
        results.insert("sectors_per_fat16".to_string(), self.validate_fat16_size(boot_sector));
        results.insert("sectors_per_fat32".to_string(), self.validate_fat32_size(boot_sector));
        results.insert("root_cluster".to_string(), self.validate_root_cluster(boot_sector));
        results.insert("fs_info_sector".to_string(), self.validate_fs_info_sector(boot_sector));
        results.insert("backup_boot_sector".to_string(), self.validate_backup_boot_sector(boot_sector));
        results.insert("fs_type_label".to_string(), self.validate_fs_type_label(boot_sector));
        
        results
    }
    
    fn validate_fat_entries(&self, fat_table: &[u8]) -> ValidationResult {
        if fat_table.len() < 12 {
            return ValidationResult::Fail("FAT table too small".to_string());
        }
        
        // Check first three entries (reserved)
        let entry0 = u32::from_le_bytes([fat_table[0], fat_table[1], fat_table[2], fat_table[3]]) & 0x0FFFFFFF;
        let entry1 = u32::from_le_bytes([fat_table[4], fat_table[5], fat_table[6], fat_table[7]]) & 0x0FFFFFFF;
        let entry2 = u32::from_le_bytes([fat_table[8], fat_table[9], fat_table[10], fat_table[11]]) & 0x0FFFFFFF;
        
        let mut issues = Vec::new();
        
        // Entry 0 should contain media descriptor in low byte
        if (entry0 & 0xFF) != 0xF8 && (entry0 & 0xFF) != 0xF0 {
            issues.push(format!("Invalid media descriptor in FAT[0]: 0x{:02X}", entry0 & 0xFF));
        }
        
        // Entry 1 should be end-of-chain
        if entry1 != 0x0FFFFFFF && entry1 != 0x0FFFFFF8 {
            issues.push(format!("FAT[1] should be EOC, got: 0x{:08X}", entry1));
        }
        
        // Entry 2 is root directory, should be EOC
        if entry2 < 0x0FFFFFF8 && entry2 != 0 {
            issues.push(format!("FAT[2] (root dir) unexpected value: 0x{:08X}", entry2));
        }
        
        if issues.is_empty() {
            ValidationResult::Pass("FAT32 reserved entries valid".to_string())
        } else {
            ValidationResult::Fail(issues.join(", "))
        }
    }
    
    fn validate_cluster_count(&self, cluster_count: u32) -> ValidationResult {
        if cluster_count < FAT32_MIN_CLUSTERS {
            ValidationResult::Fail(format!(
                "Too few clusters for FAT32: {} (minimum {})",
                cluster_count, FAT32_MIN_CLUSTERS
            ))
        } else if cluster_count > 0x0FFFFFFF {
            ValidationResult::Fail(format!(
                "Too many clusters for FAT32: {} (maximum 268435455)",
                cluster_count
            ))
        } else {
            ValidationResult::Pass(format!("{} clusters (valid FAT32 range)", cluster_count))
        }
    }
}

impl Fat32Validator {
    fn validate_root_entries(&self, boot_sector: &[u8]) -> ValidationResult {
        let root_entries = u16::from_le_bytes([
            boot_sector[BPB_ROOT_ENT_CNT],
            boot_sector[BPB_ROOT_ENT_CNT + 1]
        ]);
        
        if root_entries == 0 {
            ValidationResult::Pass("Root entries = 0 (correct for FAT32)".to_string())
        } else {
            ValidationResult::Fail(format!("Root entries = {} (must be 0 for FAT32)", root_entries))
        }
    }
    
    fn validate_fat16_size(&self, boot_sector: &[u8]) -> ValidationResult {
        let fat16_size = u16::from_le_bytes([
            boot_sector[BPB_FAT_SZ16],
            boot_sector[BPB_FAT_SZ16 + 1]
        ]);
        
        if fat16_size == 0 {
            ValidationResult::Pass("FAT16 size = 0 (correct for FAT32)".to_string())
        } else {
            ValidationResult::Fail(format!("FAT16 size = {} (must be 0 for FAT32)", fat16_size))
        }
    }
    
    fn validate_fat32_size(&self, boot_sector: &[u8]) -> ValidationResult {
        if boot_sector.len() < BPB_FAT_SZ32 + 4 {
            return ValidationResult::Fail("Boot sector too small for FAT32 fields".to_string());
        }
        
        let fat32_size = u32::from_le_bytes([
            boot_sector[BPB_FAT_SZ32],
            boot_sector[BPB_FAT_SZ32 + 1],
            boot_sector[BPB_FAT_SZ32 + 2],
            boot_sector[BPB_FAT_SZ32 + 3]
        ]);
        
        if fat32_size == 0 {
            ValidationResult::Fail("FAT32 size is 0".to_string())
        } else {
            ValidationResult::Pass(format!("{} sectors per FAT", fat32_size))
        }
    }
    
    fn validate_root_cluster(&self, boot_sector: &[u8]) -> ValidationResult {
        if boot_sector.len() < BPB_ROOT_CLUS + 4 {
            return ValidationResult::Fail("Boot sector too small for root cluster field".to_string());
        }
        
        let root_cluster = u32::from_le_bytes([
            boot_sector[BPB_ROOT_CLUS],
            boot_sector[BPB_ROOT_CLUS + 1],
            boot_sector[BPB_ROOT_CLUS + 2],
            boot_sector[BPB_ROOT_CLUS + 3]
        ]);
        
        if root_cluster == 2 {
            ValidationResult::Pass("Root directory at cluster 2 (standard)".to_string())
        } else if root_cluster >= 2 && root_cluster < 0x0FFFFFF7 {
            ValidationResult::Warning(format!("Root directory at cluster {} (non-standard but valid)", root_cluster))
        } else {
            ValidationResult::Fail(format!("Invalid root cluster: {}", root_cluster))
        }
    }
    
    fn validate_fs_info_sector(&self, boot_sector: &[u8]) -> ValidationResult {
        if boot_sector.len() < BPB_FS_INFO + 2 {
            return ValidationResult::Fail("Boot sector too small for FSInfo sector field".to_string());
        }
        
        let fs_info = u16::from_le_bytes([
            boot_sector[BPB_FS_INFO],
            boot_sector[BPB_FS_INFO + 1]
        ]);
        
        if fs_info == 1 {
            ValidationResult::Pass("FSInfo at sector 1 (standard)".to_string())
        } else if fs_info == 0xFFFF || fs_info == 0 {
            ValidationResult::Warning("No FSInfo sector".to_string())
        } else {
            ValidationResult::Warning(format!("FSInfo at sector {} (non-standard)", fs_info))
        }
    }
    
    fn validate_backup_boot_sector(&self, boot_sector: &[u8]) -> ValidationResult {
        if boot_sector.len() < BPB_BK_BOOT_SEC + 2 {
            return ValidationResult::Fail("Boot sector too small for backup boot sector field".to_string());
        }
        
        let backup = u16::from_le_bytes([
            boot_sector[BPB_BK_BOOT_SEC],
            boot_sector[BPB_BK_BOOT_SEC + 1]
        ]);
        
        if backup == 6 {
            ValidationResult::Pass("Backup boot sector at sector 6 (standard)".to_string())
        } else if backup == 0 || backup == 0xFFFF {
            ValidationResult::Warning("No backup boot sector".to_string())
        } else {
            ValidationResult::Warning(format!("Backup boot sector at sector {} (non-standard)", backup))
        }
    }
    
    fn validate_fs_type_label(&self, boot_sector: &[u8]) -> ValidationResult {
        if boot_sector.len() < BS32_FIL_SYS_TYPE + 8 {
            return ValidationResult::Fail("Boot sector too small for filesystem type field".to_string());
        }
        
        let fs_type = &boot_sector[BS32_FIL_SYS_TYPE..BS32_FIL_SYS_TYPE + 8];
        
        if fs_type == b"FAT32   " {
            ValidationResult::Pass("Filesystem type label: 'FAT32   '".to_string())
        } else {
            let label = String::from_utf8_lossy(fs_type);
            ValidationResult::Warning(format!("Non-standard filesystem type label: '{}'", label))
        }
    }
}

/// Validate FSInfo sector
pub fn validate_fsinfo_sector(fsinfo: &[u8]) -> ValidationResult {
    if fsinfo.len() < 512 {
        return ValidationResult::Fail("FSInfo sector smaller than 512 bytes".to_string());
    }
    
    let mut issues = Vec::new();
    
    // Check lead signature (offset 0)
    let lead_sig = u32::from_le_bytes([fsinfo[0], fsinfo[1], fsinfo[2], fsinfo[3]]);
    if lead_sig != 0x41615252 {
        issues.push(format!("Invalid lead signature: 0x{:08X} (expected 0x41615252 'RRaA')", lead_sig));
    }
    
    // Check struct signature (offset 484)
    let struct_sig = u32::from_le_bytes([fsinfo[484], fsinfo[485], fsinfo[486], fsinfo[487]]);
    if struct_sig != 0x61417272 {
        issues.push(format!("Invalid struct signature: 0x{:08X} (expected 0x61417272 'rrAa')", struct_sig));
    }
    
    // Check trail signature (offset 508)
    let trail_sig = u32::from_le_bytes([fsinfo[508], fsinfo[509], fsinfo[510], fsinfo[511]]);
    if trail_sig != 0xAA550000 {
        issues.push(format!("Invalid trail signature: 0x{:08X} (expected 0xAA550000)", trail_sig));
    }
    
    // Check free cluster count (offset 488)
    let free_count = u32::from_le_bytes([fsinfo[488], fsinfo[489], fsinfo[490], fsinfo[491]]);
    let next_free = u32::from_le_bytes([fsinfo[492], fsinfo[493], fsinfo[494], fsinfo[495]]);
    
    if free_count == 0xFFFFFFFF {
        issues.push("Free cluster count unknown (0xFFFFFFFF)".to_string());
    }
    
    if next_free == 0xFFFFFFFF {
        issues.push("Next free cluster unknown (0xFFFFFFFF)".to_string());
    } else if next_free < 2 {
        issues.push(format!("Invalid next free cluster: {} (must be >= 2)", next_free));
    }
    
    if issues.is_empty() {
        ValidationResult::Pass(format!(
            "Valid FSInfo: {} free clusters, next free at {}",
            if free_count == 0xFFFFFFFF { "unknown".to_string() } else { free_count.to_string() },
            if next_free == 0xFFFFFFFF { "unknown".to_string() } else { next_free.to_string() }
        ))
    } else {
        ValidationResult::Fail(issues.join(", "))
    }
}

/// Comprehensive FAT32 validation
pub struct Fat32ComprehensiveValidator;

impl Fat32ComprehensiveValidator {
    pub fn validate_filesystem(device_path: &str) -> Result<Fat32ValidationReport, std::io::Error> {
        let mut file = File::open(device_path)?;
        let mut boot_sector = vec![0u8; 512];
        file.read_exact(&mut boot_sector)?;
        
        // Validate common fields
        let common_validation = BootSectorValidator::validate_common(&boot_sector);
        
        // Validate FAT32-specific fields
        let validator = Fat32Validator;
        let specific_fields = validator.validate_specific_fields(&boot_sector);
        
        // Check cluster count
        let cluster_count = calculate_cluster_count(&boot_sector);
        let cluster_validation = cluster_count
            .map(|c| validator.validate_cluster_count(c))
            .unwrap_or(ValidationResult::Fail("Could not calculate cluster count".to_string()));
        
        // Read and validate FSInfo sector
        let mut fsinfo = vec![0u8; 512];
        file.seek(SeekFrom::Start(512))?;
        file.read_exact(&mut fsinfo)?;
        let fsinfo_validation = validate_fsinfo_sector(&fsinfo);
        
        // Read and validate FAT table
        let reserved_sectors = u16::from_le_bytes([
            boot_sector[BPB_RSVD_SEC_CNT],
            boot_sector[BPB_RSVD_SEC_CNT + 1]
        ]) as u64;
        
        let mut fat_sample = vec![0u8; 512];
        file.seek(SeekFrom::Start(reserved_sectors * 512))?;
        file.read_exact(&mut fat_sample)?;
        let fat_validation = validator.validate_fat_entries(&fat_sample);
        
        // Determine overall status
        let mut has_errors = false;
        let mut has_warnings = false;
        
        // Check all validations
        let mut check_result = |result: &ValidationResult| {
            match result {
                ValidationResult::Fail(_) => has_errors = true,
                ValidationResult::Warning(_) => has_warnings = true,
                _ => {}
            }
        };
        
        check_result(&common_validation.jump_instruction);
        check_result(&common_validation.oem_name);
        check_result(&common_validation.boot_signature);
        check_result(&cluster_validation);
        check_result(&fsinfo_validation);
        check_result(&fat_validation);
        
        for result in specific_fields.values() {
            check_result(result);
        }
        
        let overall_status = if has_errors {
            ValidationStatus::NonCompliant
        } else if has_warnings {
            ValidationStatus::Compliant
        } else {
            ValidationStatus::Perfect
        };
        
        Ok(Fat32ValidationReport {
            overall_status,
            common_validation,
            specific_fields,
            cluster_validation,
            fsinfo_validation,
            fat_validation,
            cluster_count,
        })
    }
}

#[derive(Debug)]
pub struct Fat32ValidationReport {
    pub overall_status: ValidationStatus,
    pub common_validation: CommonBootSectorValidation,
    pub specific_fields: HashMap<String, ValidationResult>,
    pub cluster_validation: ValidationResult,
    pub fsinfo_validation: ValidationResult,
    pub fat_validation: ValidationResult,
    pub cluster_count: Option<u32>,
}