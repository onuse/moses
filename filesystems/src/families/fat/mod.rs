// FAT Filesystem Family
// Includes FAT12, FAT16, FAT32, and exFAT

pub mod common;
pub mod fat16;
pub mod fat32;
pub mod exfat;

use super::{FilesystemFamily, FamilySignature, FamilyMetadata};

/// The FAT filesystem family
pub struct FatFamily;

impl FilesystemFamily for FatFamily {
    fn family_name(&self) -> &str {
        "FAT"
    }
    
    fn variants(&self) -> Vec<String> {
        vec![
            "FAT12".to_string(),
            "FAT16".to_string(), 
            "FAT32".to_string(),
            "exFAT".to_string(),
        ]
    }
    
    fn family_signatures(&self) -> Vec<FamilySignature> {
        vec![
            FamilySignature {
                offset: 0x52,
                signature: b"FAT".to_vec(),
                variant_hint: Some("FAT16".to_string()),
                confidence: 0.7,
            },
            FamilySignature {
                offset: 0x52,
                signature: b"FAT32".to_vec(),
                variant_hint: Some("FAT32".to_string()),
                confidence: 0.9,
            },
            FamilySignature {
                offset: 0x03,
                signature: b"EXFAT".to_vec(),
                variant_hint: Some("exFAT".to_string()),
                confidence: 0.95,
            },
            FamilySignature {
                offset: 0x1FE,
                signature: vec![0x55, 0xAA], // Boot signature
                variant_hint: None,
                confidence: 0.3, // Low confidence, many filesystems use this
            },
        ]
    }
}

impl FatFamily {
    /// Get metadata about the FAT family
    pub fn metadata() -> FamilyMetadata {
        FamilyMetadata {
            era_start: 1977, // FAT12 with Microsoft Standalone Disk BASIC-80
            era_end: None,   // Still in use
            common_block_sizes: vec![512, 1024, 2048, 4096],
            max_volume_size: 128 * 1024 * 1024 * 1024 * 1024, // 128TB for exFAT
            supports_journaling: false,
            supports_compression: false,
        }
    }
}

/// Trait for FAT filesystem variants
pub trait FatVariant {
    /// Get the FAT type (12, 16, 32, or 64 for exFAT)
    fn fat_bits(&self) -> u8;
    
    /// Maximum cluster number for this variant
    fn max_cluster(&self) -> u32;
    
    /// Read a FAT entry
    fn read_fat_entry(&self, cluster: u32, fat_data: &[u8]) -> u32;
    
    /// Write a FAT entry
    fn write_fat_entry(&mut self, cluster: u32, value: u32, fat_data: &mut [u8]);
    
    /// Check if cluster number is valid
    fn is_valid_cluster(&self, cluster: u32) -> bool {
        cluster >= 2 && cluster <= self.max_cluster()
    }
    
    /// Check if cluster is end of chain
    fn is_end_of_chain(&self, cluster: u32) -> bool {
        cluster >= 0x0FFFFFF8  // Works for FAT16/32, override for FAT12
    }
}

/// Shared FAT filesystem detection
pub fn detect_fat_variant(boot_sector: &[u8]) -> Option<String> {
    if boot_sector.len() < 512 {
        return None;
    }
    
    // Check for boot signature
    if boot_sector[0x1FE] != 0x55 || boot_sector[0x1FF] != 0xAA {
        return None;
    }
    
    // Check for exFAT
    if &boot_sector[0x03..0x08] == b"EXFAT" {
        return Some("exFAT".to_string());
    }
    
    // Check for FAT32
    if &boot_sector[0x52..0x57] == b"FAT32" {
        return Some("FAT32".to_string());
    }
    
    // Check for FAT16
    if &boot_sector[0x36..0x3B] == b"FAT16" {
        return Some("FAT16".to_string());
    }
    
    // Try to determine by cluster count
    let bytes_per_sector = u16::from_le_bytes([boot_sector[0x0B], boot_sector[0x0C]]);
    let sectors_per_cluster = boot_sector[0x0D];
    let total_sectors = if boot_sector[0x13] != 0 || boot_sector[0x14] != 0 {
        u16::from_le_bytes([boot_sector[0x13], boot_sector[0x14]]) as u32
    } else {
        u32::from_le_bytes([boot_sector[0x20], boot_sector[0x21], boot_sector[0x22], boot_sector[0x23]])
    };
    
    if bytes_per_sector == 0 || sectors_per_cluster == 0 {
        return None;
    }
    
    let total_clusters = total_sectors / sectors_per_cluster as u32;
    
    match total_clusters {
        0..=4084 => Some("FAT12".to_string()),
        4085..=65524 => Some("FAT16".to_string()),
        _ => Some("FAT32".to_string()),
    }
}