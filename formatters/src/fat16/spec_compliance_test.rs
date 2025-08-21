// FAT16 Specification Compliance Test
// Based on Microsoft FAT specification

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

#[derive(Debug)]
pub struct Fat16ComplianceResult {
    pub is_compliant: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub info: Vec<String>,
}

pub fn check_fat16_compliance(device_path: &str) -> Result<Fat16ComplianceResult, std::io::Error> {
    let mut file = File::open(device_path)?;
    let mut result = Fat16ComplianceResult {
        is_compliant: true,
        errors: Vec::new(),
        warnings: Vec::new(),
        info: Vec::new(),
    };
    
    // Read boot sector
    let mut boot_sector = vec![0u8; 512];
    file.read_exact(&mut boot_sector)?;
    
    // 1. Check jump instruction (offset 0x00)
    if !(boot_sector[0] == 0xEB && boot_sector[2] == 0x90) && boot_sector[0] != 0xE9 {
        result.errors.push(format!(
            "Invalid jump instruction: {:02X} {:02X} {:02X} (should be EB xx 90 or E9 xx xx)",
            boot_sector[0], boot_sector[1], boot_sector[2]
        ));
        result.is_compliant = false;
    }
    
    // 2. Check OEM name (offset 0x03, 8 bytes)
    let oem_name = String::from_utf8_lossy(&boot_sector[3..11]);
    result.info.push(format!("OEM Name: '{}'", oem_name));
    
    // 3. Check bytes per sector (offset 0x0B, must be 512, 1024, 2048, or 4096)
    let bytes_per_sector = u16::from_le_bytes([boot_sector[0x0B], boot_sector[0x0C]]);
    if ![512, 1024, 2048, 4096].contains(&bytes_per_sector) {
        result.errors.push(format!(
            "Invalid bytes per sector: {} (must be 512, 1024, 2048, or 4096)",
            bytes_per_sector
        ));
        result.is_compliant = false;
    }
    result.info.push(format!("Bytes per sector: {}", bytes_per_sector));
    
    // 4. Check sectors per cluster (offset 0x0D, must be power of 2)
    let sectors_per_cluster = boot_sector[0x0D];
    if sectors_per_cluster == 0 || (sectors_per_cluster & (sectors_per_cluster - 1)) != 0 {
        result.errors.push(format!(
            "Invalid sectors per cluster: {} (must be power of 2)",
            sectors_per_cluster
        ));
        result.is_compliant = false;
    }
    result.info.push(format!("Sectors per cluster: {}", sectors_per_cluster));
    
    // 5. Check reserved sectors (offset 0x0E, usually 1 for FAT16)
    let reserved_sectors = u16::from_le_bytes([boot_sector[0x0E], boot_sector[0x0F]]);
    if reserved_sectors == 0 {
        result.errors.push("Reserved sectors cannot be 0".to_string());
        result.is_compliant = false;
    }
    if reserved_sectors != 1 {
        result.warnings.push(format!(
            "Unusual reserved sectors count: {} (typically 1 for FAT16)",
            reserved_sectors
        ));
    }
    result.info.push(format!("Reserved sectors: {}", reserved_sectors));
    
    // 6. Check number of FATs (offset 0x10, usually 2)
    let num_fats = boot_sector[0x10];
    if num_fats == 0 {
        result.errors.push("Number of FATs cannot be 0".to_string());
        result.is_compliant = false;
    }
    if num_fats != 2 {
        result.warnings.push(format!("Unusual number of FATs: {} (typically 2)", num_fats));
    }
    result.info.push(format!("Number of FATs: {}", num_fats));
    
    // 7. Check root entries (offset 0x11, typically 512 for FAT16)
    let root_entries = u16::from_le_bytes([boot_sector[0x11], boot_sector[0x12]]);
    if root_entries == 0 {
        result.errors.push("Root entries cannot be 0 for FAT16".to_string());
        result.is_compliant = false;
    }
    if root_entries != 512 {
        result.warnings.push(format!(
            "Unusual root entries: {} (typically 512 for FAT16)",
            root_entries
        ));
    }
    result.info.push(format!("Root entries: {}", root_entries));
    
    // 8. Check total sectors (offset 0x13 or 0x20)
    let total_sectors_16 = u16::from_le_bytes([boot_sector[0x13], boot_sector[0x14]]);
    let total_sectors_32 = u32::from_le_bytes([
        boot_sector[0x20], boot_sector[0x21], boot_sector[0x22], boot_sector[0x23]
    ]);
    
    let total_sectors = if total_sectors_16 != 0 {
        total_sectors_16 as u32
    } else {
        total_sectors_32
    };
    
    if total_sectors == 0 {
        result.errors.push("Total sectors cannot be 0".to_string());
        result.is_compliant = false;
    }
    result.info.push(format!("Total sectors: {}", total_sectors));
    
    // 9. Check media descriptor (offset 0x15)
    let media_descriptor = boot_sector[0x15];
    if ![0xF0, 0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF].contains(&media_descriptor) {
        result.warnings.push(format!(
            "Unusual media descriptor: 0x{:02X} (typically 0xF8 for fixed disk)",
            media_descriptor
        ));
    }
    result.info.push(format!("Media descriptor: 0x{:02X}", media_descriptor));
    
    // 10. Check sectors per FAT (offset 0x16)
    let sectors_per_fat = u16::from_le_bytes([boot_sector[0x16], boot_sector[0x17]]);
    if sectors_per_fat == 0 {
        result.errors.push("Sectors per FAT cannot be 0".to_string());
        result.is_compliant = false;
    }
    result.info.push(format!("Sectors per FAT: {}", sectors_per_fat));
    
    // 11. Check extended boot signature (offset 0x26)
    let boot_signature = boot_sector[0x26];
    if boot_signature == 0x29 {
        // Extended BPB is present
        let volume_id = u32::from_le_bytes([
            boot_sector[0x27], boot_sector[0x28], boot_sector[0x29], boot_sector[0x2A]
        ]);
        let volume_label = String::from_utf8_lossy(&boot_sector[0x2B..0x36]);
        let fs_type = String::from_utf8_lossy(&boot_sector[0x36..0x3E]);
        
        result.info.push(format!("Extended BPB present"));
        result.info.push(format!("Volume ID: 0x{:08X}", volume_id));
        result.info.push(format!("Volume Label: '{}'", volume_label));
        result.info.push(format!("FS Type: '{}'", fs_type));
        
        // Check if FS type field contains "FAT16"
        if !fs_type.contains("FAT16") && !fs_type.contains("FAT") {
            result.warnings.push(format!(
                "FS type field doesn't contain 'FAT16': '{}'",
                fs_type.trim()
            ));
        }
    } else if boot_signature != 0x28 {
        result.warnings.push(format!(
            "No extended BPB (boot signature: 0x{:02X}, expected 0x29 or 0x28)",
            boot_signature
        ));
    }
    
    // 12. Check boot sector signature (offset 0x1FE)
    if boot_sector[0x1FE] != 0x55 || boot_sector[0x1FF] != 0xAA {
        result.errors.push(format!(
            "Invalid boot sector signature: {:02X}{:02X} (should be 55AA)",
            boot_sector[0x1FE], boot_sector[0x1FF]
        ));
        result.is_compliant = false;
    }
    
    // 13. Calculate and verify cluster count for FAT16
    let root_dir_sectors = ((root_entries * 32) + (bytes_per_sector - 1)) / bytes_per_sector;
    let data_sectors = total_sectors - (reserved_sectors as u32 + (num_fats as u32 * sectors_per_fat as u32) + root_dir_sectors as u32);
    let total_clusters = data_sectors / sectors_per_cluster as u32;
    
    result.info.push(format!("Total clusters: {}", total_clusters));
    
    // FAT16 must have between 4085 and 65524 clusters
    if total_clusters < 4085 {
        result.errors.push(format!(
            "Too few clusters for FAT16: {} (minimum 4085)",
            total_clusters
        ));
        result.is_compliant = false;
    } else if total_clusters > 65524 {
        result.errors.push(format!(
            "Too many clusters for FAT16: {} (maximum 65524)",
            total_clusters
        ));
        result.is_compliant = false;
    }
    
    // 14. Check first FAT entries
    file.seek(SeekFrom::Start((reserved_sectors * bytes_per_sector) as u64))?;
    let mut fat_entries = vec![0u8; 4];
    file.read_exact(&mut fat_entries)?;
    
    // First FAT entry should contain media descriptor in low byte
    if fat_entries[0] != media_descriptor {
        result.warnings.push(format!(
            "FAT[0] low byte (0x{:02X}) doesn't match media descriptor (0x{:02X})",
            fat_entries[0], media_descriptor
        ));
    }
    if fat_entries[1] != 0xFF {
        result.warnings.push(format!(
            "FAT[0] high byte is 0x{:02X} (expected 0xFF)",
            fat_entries[1]
        ));
    }
    
    // Second FAT entry should be end-of-chain (0xFFFF)
    if fat_entries[2] != 0xFF || fat_entries[3] != 0xFF {
        result.warnings.push(format!(
            "FAT[1] is 0x{:02X}{:02X} (expected 0xFFFF for end-of-chain)",
            fat_entries[3], fat_entries[2]
        ));
    }
    
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_windows_formatted_fat16() {
        // Create a test file and format it with Windows' format.com
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();
        
        // Create a 100MB test file
        Command::new("fsutil")
            .args(&["file", "createnew", path, "104857600"])
            .output()
            .expect("Failed to create test file");
        
        // Format as FAT16 using Windows
        Command::new("format")
            .args(&[path, "/FS:FAT", "/Q", "/Y"])
            .output()
            .expect("Failed to format with Windows");
        
        // Check compliance
        let result = check_fat16_compliance(path).unwrap();
        
        println!("Windows FAT16 compliance test:");
        for info in &result.info {
            println!("  INFO: {}", info);
        }
        for warning in &result.warnings {
            println!("  WARN: {}", warning);
        }
        for error in &result.errors {
            println!("  ERROR: {}", error);
        }
        
        assert!(result.is_compliant, "Windows-formatted FAT16 should be compliant");
    }
}