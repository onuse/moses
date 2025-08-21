// Modular FAT validation framework
// Shared validation logic for FAT16 and FAT32 filesystems

// File I/O imports removed - not used in this module
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use super::constants::*;

/// Result of a validation check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationResult {
    Pass(String),
    Warning(String),
    Fail(String),
}

/// Overall validation status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationStatus {
    Perfect,            // 100% compliant
    Compliant,         // Spec compliant with minor issues
    PartiallyCompliant, // Some violations but likely works
    NonCompliant,      // Major violations
    Corrupted,         // Filesystem corrupted
}

/// Common BPB fields that both FAT16 and FAT32 share
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommonBpbValidation {
    pub bytes_per_sector: ValidationResult,
    pub sectors_per_cluster: ValidationResult,
    pub reserved_sectors: ValidationResult,
    pub num_fats: ValidationResult,
    pub media_descriptor: ValidationResult,
    pub sectors_per_track: ValidationResult,
    pub num_heads: ValidationResult,
    pub hidden_sectors: ValidationResult,
    pub total_sectors: ValidationResult,
}

/// Boot sector validation common to both FAT types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommonBootSectorValidation {
    pub jump_instruction: ValidationResult,
    pub oem_name: ValidationResult,
    pub boot_signature: ValidationResult,
    pub common_bpb: CommonBpbValidation,
}

/// FAT-specific validation trait
pub trait FatValidator {
    /// Get the FAT type name
    fn fat_type(&self) -> &'static str;
    
    /// Validate FAT-specific boot sector fields
    fn validate_specific_fields(&self, boot_sector: &[u8]) -> HashMap<String, ValidationResult>;
    
    /// Validate FAT table entries
    fn validate_fat_entries(&self, fat_table: &[u8]) -> ValidationResult;
    
    /// Check cluster count validity
    fn validate_cluster_count(&self, cluster_count: u32) -> ValidationResult;
}

/// Common boot sector validator
pub struct BootSectorValidator;

impl BootSectorValidator {
    /// Validate common boot sector fields
    pub fn validate_common(boot_sector: &[u8]) -> CommonBootSectorValidation {
        let validation = CommonBootSectorValidation {
            jump_instruction: Self::validate_jump(boot_sector),
            oem_name: Self::validate_oem_name(boot_sector),
            boot_signature: Self::validate_boot_signature(boot_sector),
            common_bpb: Self::validate_common_bpb(boot_sector),
        };
        
        validation
    }
    
    fn validate_jump(boot_sector: &[u8]) -> ValidationResult {
        if boot_sector.len() < 3 {
            return ValidationResult::Fail("Boot sector too small".to_string());
        }
        
        match boot_sector[0] {
            0xEB => {
                if boot_sector[2] == 0x90 {
                    ValidationResult::Pass("Valid short jump with NOP".to_string())
                } else {
                    ValidationResult::Warning(format!("Short jump but third byte is 0x{:02X}, not 0x90", boot_sector[2]))
                }
            }
            0xE9 => ValidationResult::Pass("Valid near jump".to_string()),
            _ => ValidationResult::Fail(format!("Invalid jump instruction: 0x{:02X}", boot_sector[0]))
        }
    }
    
    fn validate_oem_name(boot_sector: &[u8]) -> ValidationResult {
        if boot_sector.len() < 11 {
            return ValidationResult::Fail("Boot sector too small for OEM name".to_string());
        }
        
        let oem_name = &boot_sector[BS_OEM_NAME..BS_OEM_NAME + 8];
        let name_str = String::from_utf8_lossy(oem_name);
        
        // Common valid OEM names
        let valid_names = ["MSWIN4.1", "MSDOS5.0", "FRDOS7.1", "IBM  3.3", "MOSES1.0"];
        
        if valid_names.iter().any(|&n| n.as_bytes() == oem_name) {
            ValidationResult::Pass(format!("Standard OEM name: {}", name_str))
        } else if oem_name.iter().all(|&b| b >= 0x20 && b <= 0x7E) {
            ValidationResult::Warning(format!("Non-standard but valid OEM name: {}", name_str))
        } else {
            ValidationResult::Fail(format!("Invalid OEM name: {:?}", oem_name))
        }
    }
    
    fn validate_boot_signature(boot_sector: &[u8]) -> ValidationResult {
        if boot_sector.len() < 512 {
            return ValidationResult::Fail("Boot sector smaller than 512 bytes".to_string());
        }
        
        if boot_sector[510] == 0x55 && boot_sector[511] == 0xAA {
            ValidationResult::Pass("Valid boot signature 0x55AA".to_string())
        } else {
            ValidationResult::Fail(format!("Invalid boot signature: 0x{:02X}{:02X}", 
                boot_sector[510], boot_sector[511]))
        }
    }
    
    fn validate_common_bpb(boot_sector: &[u8]) -> CommonBpbValidation {
        CommonBpbValidation {
            bytes_per_sector: Self::validate_bytes_per_sector(boot_sector),
            sectors_per_cluster: Self::validate_sectors_per_cluster(boot_sector),
            reserved_sectors: Self::validate_reserved_sectors(boot_sector),
            num_fats: Self::validate_num_fats(boot_sector),
            media_descriptor: Self::validate_media_descriptor(boot_sector),
            sectors_per_track: Self::validate_sectors_per_track(boot_sector),
            num_heads: Self::validate_num_heads(boot_sector),
            hidden_sectors: Self::validate_hidden_sectors(boot_sector),
            total_sectors: Self::validate_total_sectors(boot_sector),
        }
    }
    
    fn validate_bytes_per_sector(boot_sector: &[u8]) -> ValidationResult {
        let bytes_per_sector = u16::from_le_bytes([boot_sector[BPB_BYTES_PER_SEC], boot_sector[BPB_BYTES_PER_SEC + 1]]);
        
        match bytes_per_sector {
            512 => ValidationResult::Pass("Standard 512 bytes per sector".to_string()),
            1024 | 2048 | 4096 => ValidationResult::Warning(format!("{} bytes per sector (non-standard but valid)", bytes_per_sector)),
            _ => ValidationResult::Fail(format!("Invalid bytes per sector: {}", bytes_per_sector))
        }
    }
    
    fn validate_sectors_per_cluster(boot_sector: &[u8]) -> ValidationResult {
        let sectors_per_cluster = boot_sector[BPB_SEC_PER_CLUS];
        
        // Must be power of 2
        if sectors_per_cluster == 0 {
            return ValidationResult::Fail("Sectors per cluster is zero".to_string());
        }
        
        if (sectors_per_cluster & (sectors_per_cluster - 1)) != 0 {
            return ValidationResult::Fail(format!("Sectors per cluster ({}) is not a power of 2", sectors_per_cluster));
        }
        
        // Check valid range (1, 2, 4, 8, 16, 32, 64, 128)
        if sectors_per_cluster <= 128 {
            ValidationResult::Pass(format!("{} sectors per cluster", sectors_per_cluster))
        } else {
            ValidationResult::Fail(format!("Sectors per cluster ({}) exceeds maximum (128)", sectors_per_cluster))
        }
    }
    
    fn validate_reserved_sectors(boot_sector: &[u8]) -> ValidationResult {
        let reserved = u16::from_le_bytes([boot_sector[BPB_RSVD_SEC_CNT], boot_sector[BPB_RSVD_SEC_CNT + 1]]);
        
        if reserved == 0 {
            ValidationResult::Fail("Reserved sectors is zero".to_string())
        } else if reserved == 1 {
            ValidationResult::Pass("Standard 1 reserved sector (FAT16)".to_string())
        } else if reserved == 32 {
            ValidationResult::Pass("Standard 32 reserved sectors (FAT32)".to_string())
        } else {
            ValidationResult::Warning(format!("{} reserved sectors (non-standard)", reserved))
        }
    }
    
    fn validate_num_fats(boot_sector: &[u8]) -> ValidationResult {
        let num_fats = boot_sector[BPB_NUM_FATS];
        
        match num_fats {
            2 => ValidationResult::Pass("Standard 2 FAT tables".to_string()),
            1 => ValidationResult::Warning("Only 1 FAT table (risky)".to_string()),
            0 => ValidationResult::Fail("No FAT tables".to_string()),
            n => ValidationResult::Warning(format!("{} FAT tables (unusual)", n))
        }
    }
    
    fn validate_media_descriptor(boot_sector: &[u8]) -> ValidationResult {
        let media = boot_sector[BPB_MEDIA];
        
        match media {
            0xF8 => ValidationResult::Pass("Fixed disk media descriptor".to_string()),
            0xF0 => ValidationResult::Pass("Removable media descriptor".to_string()),
            0xF9..=0xFF => ValidationResult::Warning(format!("Valid but uncommon media descriptor: 0x{:02X}", media)),
            _ => ValidationResult::Fail(format!("Invalid media descriptor: 0x{:02X}", media))
        }
    }
    
    fn validate_sectors_per_track(boot_sector: &[u8]) -> ValidationResult {
        let spt = u16::from_le_bytes([boot_sector[BPB_SEC_PER_TRK], boot_sector[BPB_SEC_PER_TRK + 1]]);
        
        if spt == 0 {
            ValidationResult::Warning("Sectors per track is 0 (LBA mode)".to_string())
        } else if spt == 63 {
            ValidationResult::Pass("Standard 63 sectors per track".to_string())
        } else if spt <= 255 {
            ValidationResult::Warning(format!("{} sectors per track (non-standard)", spt))
        } else {
            ValidationResult::Fail(format!("Invalid sectors per track: {}", spt))
        }
    }
    
    fn validate_num_heads(boot_sector: &[u8]) -> ValidationResult {
        let heads = u16::from_le_bytes([boot_sector[BPB_NUM_HEADS], boot_sector[BPB_NUM_HEADS + 1]]);
        
        if heads == 0 {
            ValidationResult::Warning("Number of heads is 0 (LBA mode)".to_string())
        } else if heads == 255 {
            ValidationResult::Pass("Standard 255 heads".to_string())
        } else if heads <= 255 {
            ValidationResult::Warning(format!("{} heads (non-standard)", heads))
        } else {
            ValidationResult::Fail(format!("Invalid number of heads: {}", heads))
        }
    }
    
    fn validate_hidden_sectors(boot_sector: &[u8]) -> ValidationResult {
        let hidden = u32::from_le_bytes([
            boot_sector[BPB_HIDD_SEC],
            boot_sector[BPB_HIDD_SEC + 1],
            boot_sector[BPB_HIDD_SEC + 2],
            boot_sector[BPB_HIDD_SEC + 3],
        ]);
        
        if hidden == 0 {
            ValidationResult::Pass("No hidden sectors (unpartitioned or first partition)".to_string())
        } else if hidden == 63 {
            ValidationResult::Pass("Legacy 63 hidden sectors".to_string())
        } else if hidden == 2048 {
            ValidationResult::Pass("Modern 1MB aligned partition (2048 sectors)".to_string())
        } else {
            ValidationResult::Warning(format!("{} hidden sectors", hidden))
        }
    }
    
    fn validate_total_sectors(boot_sector: &[u8]) -> ValidationResult {
        let total16 = u16::from_le_bytes([boot_sector[BPB_TOT_SEC16], boot_sector[BPB_TOT_SEC16 + 1]]);
        let total32 = u32::from_le_bytes([
            boot_sector[BPB_TOT_SEC32],
            boot_sector[BPB_TOT_SEC32 + 1],
            boot_sector[BPB_TOT_SEC32 + 2],
            boot_sector[BPB_TOT_SEC32 + 3],
        ]);
        
        if total16 == 0 && total32 == 0 {
            ValidationResult::Fail("No total sectors specified".to_string())
        } else if total16 != 0 && total32 != 0 {
            ValidationResult::Fail("Both 16-bit and 32-bit total sectors specified".to_string())
        } else if total16 != 0 {
            ValidationResult::Pass(format!("{} total sectors (16-bit field)", total16))
        } else {
            ValidationResult::Pass(format!("{} total sectors (32-bit field)", total32))
        }
    }
}

/// Calculate cluster count from boot sector
pub fn calculate_cluster_count(boot_sector: &[u8]) -> Option<u32> {
    if boot_sector.len() < 512 {
        return None;
    }
    
    let bytes_per_sector = u16::from_le_bytes([boot_sector[BPB_BYTES_PER_SEC], boot_sector[BPB_BYTES_PER_SEC + 1]]);
    let sectors_per_cluster = boot_sector[BPB_SEC_PER_CLUS];
    let reserved_sectors = u16::from_le_bytes([boot_sector[BPB_RSVD_SEC_CNT], boot_sector[BPB_RSVD_SEC_CNT + 1]]);
    let num_fats = boot_sector[BPB_NUM_FATS];
    let root_entries = u16::from_le_bytes([boot_sector[BPB_ROOT_ENT_CNT], boot_sector[BPB_ROOT_ENT_CNT + 1]]);
    let total16 = u16::from_le_bytes([boot_sector[BPB_TOT_SEC16], boot_sector[BPB_TOT_SEC16 + 1]]);
    let sectors_per_fat16 = u16::from_le_bytes([boot_sector[BPB_FAT_SZ16], boot_sector[BPB_FAT_SZ16 + 1]]);
    let total32 = u32::from_le_bytes([
        boot_sector[BPB_TOT_SEC32],
        boot_sector[BPB_TOT_SEC32 + 1],
        boot_sector[BPB_TOT_SEC32 + 2],
        boot_sector[BPB_TOT_SEC32 + 3],
    ]);
    
    // For FAT32
    let sectors_per_fat32 = if boot_sector.len() >= 40 {
        u32::from_le_bytes([
            boot_sector[BPB_FAT_SZ32],
            boot_sector[BPB_FAT_SZ32 + 1],
            boot_sector[BPB_FAT_SZ32 + 2],
            boot_sector[BPB_FAT_SZ32 + 3],
        ])
    } else {
        0
    };
    
    let total_sectors = if total16 != 0 { total16 as u32 } else { total32 };
    let sectors_per_fat = if sectors_per_fat16 != 0 { sectors_per_fat16 as u32 } else { sectors_per_fat32 };
    
    if total_sectors == 0 || sectors_per_cluster == 0 {
        return None;
    }
    
    let root_dir_sectors = ((root_entries * 32) + (bytes_per_sector - 1)) / bytes_per_sector;
    let data_start = reserved_sectors as u32 + (num_fats as u32 * sectors_per_fat) + root_dir_sectors as u32;
    
    if data_start >= total_sectors {
        return None;
    }
    
    let data_sectors = total_sectors - data_start;
    Some(data_sectors / sectors_per_cluster as u32)
}