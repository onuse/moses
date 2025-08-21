// MBR Verifier - Validates Master Boot Record compliance
use log::info;

#[derive(Debug)]
pub struct MbrVerificationResult {
    pub is_valid: bool,
    pub has_disk_signature: bool,
    pub partitions: Vec<PartitionInfo>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub struct PartitionInfo {
    pub number: u8,
    pub bootable: bool,
    pub partition_type: u8,
    pub type_name: String,
    pub start_lba: u32,
    pub size_sectors: u32,
    pub start_chs: (u32, u32, u32),  // Cylinder, Head, Sector
    pub end_chs: (u32, u32, u32),
}

pub struct MbrVerifier;

impl MbrVerifier {
    /// Verify an MBR sector (512 bytes)
    pub fn verify_mbr(mbr_data: &[u8]) -> MbrVerificationResult {
        let mut result = MbrVerificationResult {
            is_valid: true,
            has_disk_signature: false,
            partitions: Vec::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
        };
        
        // Check size
        if mbr_data.len() != 512 {
            result.errors.push(format!("Invalid MBR size: {} bytes (should be 512)", mbr_data.len()));
            result.is_valid = false;
            return result;
        }
        
        // Check MBR signature
        if mbr_data[510] != 0x55 || mbr_data[511] != 0xAA {
            result.errors.push(format!(
                "Invalid MBR signature: {:02X}{:02X} (should be 55AA)",
                mbr_data[510], mbr_data[511]
            ));
            result.is_valid = false;
        }
        
        // Check disk signature (offset 440-443)
        let disk_sig = u32::from_le_bytes([
            mbr_data[440], mbr_data[441], mbr_data[442], mbr_data[443]
        ]);
        
        if disk_sig != 0 {
            result.has_disk_signature = true;
            info!("Disk signature: 0x{:08X}", disk_sig);
        } else {
            result.warnings.push("No disk signature found (Windows may not recognize this MBR)".to_string());
        }
        
        // Parse partition entries (4 entries at offset 446)
        for i in 0..4 {
            let offset = 446 + (i * 16);
            let entry = &mbr_data[offset..offset + 16];
            
            // Check if partition exists (type != 0)
            if entry[4] != 0 {
                let partition = Self::parse_partition_entry(i as u8 + 1, entry);
                
                // Validate partition
                Self::validate_partition(&partition, &mut result);
                
                result.partitions.push(partition);
            }
        }
        
        // Check for overlapping partitions
        for i in 0..result.partitions.len() {
            for j in i + 1..result.partitions.len() {
                let p1 = &result.partitions[i];
                let p2 = &result.partitions[j];
                
                let p1_end = p1.start_lba + p1.size_sectors;
                let p2_end = p2.start_lba + p2.size_sectors;
                
                if (p1.start_lba >= p2.start_lba && p1.start_lba < p2_end) ||
                   (p2.start_lba >= p1.start_lba && p2.start_lba < p1_end) {
                    result.errors.push(format!(
                        "Partitions {} and {} overlap!",
                        p1.number, p2.number
                    ));
                    result.is_valid = false;
                }
            }
        }
        
        if result.partitions.is_empty() {
            result.warnings.push("No partitions found in MBR".to_string());
        }
        
        result
    }
    
    fn parse_partition_entry(number: u8, entry: &[u8]) -> PartitionInfo {
        // Parse CHS values
        let start_head = entry[1] as u32;
        let start_sector = (entry[2] & 0x3F) as u32;
        let start_cyl_high = ((entry[2] & 0xC0) as u32) << 2;
        let start_cyl_low = entry[3] as u32;
        let start_cylinder = start_cyl_high | start_cyl_low;
        
        let end_head = entry[5] as u32;
        let end_sector = (entry[6] & 0x3F) as u32;
        let end_cyl_high = ((entry[6] & 0xC0) as u32) << 2;
        let end_cyl_low = entry[7] as u32;
        let end_cylinder = end_cyl_high | end_cyl_low;
        
        // Parse LBA values
        let start_lba = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]);
        let size_sectors = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]);
        
        // Determine partition type name
        let type_name = match entry[4] {
            0x00 => "Empty",
            0x01 => "FAT12",
            0x04 => "FAT16 (<32MB)",
            0x05 => "Extended",
            0x06 => "FAT16",
            0x07 => "NTFS/exFAT",
            0x0B => "FAT32 (CHS)",
            0x0C => "FAT32 (LBA)",
            0x0E => "FAT16 (LBA)",
            0x0F => "Extended (LBA)",
            0x82 => "Linux swap",
            0x83 => "Linux",
            0xEE => "GPT Protective",
            _ => "Unknown",
        }.to_string();
        
        PartitionInfo {
            number,
            bootable: entry[0] == 0x80,
            partition_type: entry[4],
            type_name,
            start_lba,
            size_sectors,
            start_chs: (start_cylinder, start_head, start_sector),
            end_chs: (end_cylinder, end_head, end_sector),
        }
    }
    
    fn validate_partition(partition: &PartitionInfo, result: &mut MbrVerificationResult) {
        // Check bootable flag
        let boot_flag = if partition.bootable { 0x80 } else { 0x00 };
        if boot_flag != 0x80 && boot_flag != 0x00 {
            result.warnings.push(format!(
                "Partition {} has invalid boot flag: 0x{:02X}",
                partition.number, boot_flag
            ));
        }
        
        // Check if CHS and LBA agree (for small disks)
        // Standard geometry: 255 heads, 63 sectors per track
        let (cyl, head, sector) = partition.start_chs;
        if cyl < 1024 && sector > 0 {  // CHS is valid only for first 1024 cylinders
            let calculated_lba = (cyl * 255 * 63) + (head * 63) + (sector.saturating_sub(1));
            if calculated_lba != partition.start_lba {
                result.warnings.push(format!(
                    "Partition {} CHS/LBA mismatch: CHS gives LBA {} but actual is {}",
                    partition.number, calculated_lba, partition.start_lba
                ));
            }
        }
        
        // Check partition alignment
        if partition.start_lba % 2048 != 0 && partition.start_lba != 63 {
            result.warnings.push(format!(
                "Partition {} not aligned: starts at LBA {} (modern systems prefer 2048 alignment)",
                partition.number, partition.start_lba
            ));
        }
        
        // Log partition info
        info!("Partition {}:", partition.number);
        info!("  Type: 0x{:02X} ({})", partition.partition_type, partition.type_name);
        info!("  Bootable: {}", partition.bootable);
        info!("  Start LBA: {}", partition.start_lba);
        info!("  Size: {} sectors ({} MB)", 
              partition.size_sectors, 
              partition.size_sectors * 512 / 1024 / 1024);
        info!("  CHS Start: C:{} H:{} S:{}", cyl, head, sector);
    }
    
    /// Generate a human-readable report
    pub fn generate_report(result: &MbrVerificationResult) -> String {
        let mut report = String::new();
        
        report.push_str("MBR Verification Report\n");
        report.push_str("=======================\n\n");
        
        report.push_str(&format!("Status: {}\n", 
            if result.is_valid { "VALID" } else { "INVALID" }));
        report.push_str(&format!("Disk Signature: {}\n", 
            if result.has_disk_signature { "Present" } else { "Missing (Windows compatibility issue)" }));
        report.push_str(&format!("Partitions: {}\n\n", result.partitions.len()));
        
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
        
        if !result.partitions.is_empty() {
            report.push_str("PARTITIONS:\n");
            for p in &result.partitions {
                report.push_str(&format!("  Partition {}:\n", p.number));
                report.push_str(&format!("    Type: 0x{:02X} ({})\n", p.partition_type, p.type_name));
                report.push_str(&format!("    Bootable: {}\n", p.bootable));
                report.push_str(&format!("    Start: LBA {} ({}MB from start)\n", 
                    p.start_lba, p.start_lba * 512 / 1024 / 1024));
                report.push_str(&format!("    Size: {} sectors ({}MB)\n", 
                    p.size_sectors, p.size_sectors * 512 / 1024 / 1024));
            }
        }
        
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_valid_mbr() {
        let mut mbr = vec![0u8; 512];
        
        // Add MBR signature
        mbr[510] = 0x55;
        mbr[511] = 0xAA;
        
        // Add disk signature
        mbr[440] = 0x12;
        mbr[441] = 0x34;
        mbr[442] = 0x56;
        mbr[443] = 0x78;
        
        // Add a FAT16 partition
        let offset = 446;
        mbr[offset] = 0x80;  // Bootable
        mbr[offset + 4] = 0x06;  // FAT16
        mbr[offset + 8] = 0x00;  // Start LBA 2048
        mbr[offset + 9] = 0x08;
        mbr[offset + 12] = 0x00;  // Size
        mbr[offset + 13] = 0x00;
        mbr[offset + 14] = 0x10;
        mbr[offset + 15] = 0x00;
        
        let result = MbrVerifier::verify_mbr(&mbr);
        
        assert!(result.is_valid);
        assert!(result.has_disk_signature);
        assert_eq!(result.partitions.len(), 1);
        assert_eq!(result.partitions[0].partition_type, 0x06);
    }
    
    #[test]
    fn test_missing_signature() {
        let mbr = vec![0u8; 512];  // No 55AA signature
        
        let result = MbrVerifier::verify_mbr(&mbr);
        
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("Invalid MBR signature")));
    }
}