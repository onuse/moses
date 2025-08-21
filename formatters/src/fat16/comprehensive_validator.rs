// Comprehensive FAT16 Specification Validator
// Checks EVERY field against Microsoft FAT specification

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::collections::HashMap;

#[derive(Debug)]
pub struct ValidationReport {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub info: HashMap<String, String>,
    pub hex_dump: Vec<u8>,
}

pub struct Fat16ComprehensiveValidator;

impl Fat16ComprehensiveValidator {
    /// Validate FAT16 filesystem, optionally at a partition offset
    pub fn validate(device_path: &str, partition_offset_sectors: Option<u64>) -> Result<ValidationReport, std::io::Error> {
        let mut file = File::open(device_path)?;
        let mut report = ValidationReport {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            info: HashMap::new(),
            hex_dump: Vec::new(),
        };
        
        // If partition offset provided, seek to partition start
        let offset_bytes = partition_offset_sectors.unwrap_or(0) * 512;
        if offset_bytes > 0 {
            file.seek(SeekFrom::Start(offset_bytes))?;
            report.info.insert("partition_offset".to_string(), format!("{} bytes (sector {})", offset_bytes, partition_offset_sectors.unwrap()));
        }
        
        // Read boot sector
        let mut boot_sector = vec![0u8; 512];
        file.read_exact(&mut boot_sector)?;
        
        // Store first 256 bytes for hex dump
        report.hex_dump = boot_sector[..256.min(boot_sector.len())].to_vec();
        
        // ========== REQUIRED FIELDS ==========
        
        // 1. Jump Instruction (0x00-0x02)
        let jump_valid = match boot_sector[0] {
            0xEB => {
                // Short jump: EB xx 90
                if boot_sector[2] != 0x90 {
                    report.errors.push(format!("Jump instruction: EB {:02X} {:02X} - third byte should be 90 (NOP)", 
                        boot_sector[1], boot_sector[2]));
                    false
                } else {
                    report.info.insert("jump_instruction".to_string(), 
                        format!("EB {:02X} 90 (short jump)", boot_sector[1]));
                    true
                }
            },
            0xE9 => {
                // Near jump: E9 xx xx
                report.info.insert("jump_instruction".to_string(), 
                    format!("E9 {:02X} {:02X} (near jump)", boot_sector[1], boot_sector[2]));
                true
            },
            _ => {
                report.errors.push(format!("Invalid jump instruction: {:02X} (must be EB or E9)", boot_sector[0]));
                false
            }
        };
        if !jump_valid {
            report.is_valid = false;
        }
        
        // 2. OEM Name (0x03-0x0A) - 8 bytes
        let oem_name = String::from_utf8_lossy(&boot_sector[3..11]);
        report.info.insert("oem_name".to_string(), format!("'{}'", oem_name));
        // Check if it's a known Windows OEM string
        if !["MSWIN4.1", "MSDOS5.0", "FRDOS4.1", "IBM  3.3"].contains(&oem_name.trim()) {
            report.warnings.push(format!("Non-standard OEM name: '{}' (common: MSWIN4.1, MSDOS5.0)", oem_name));
        }
        
        // 3. Bytes per Sector (0x0B-0x0C)
        let bytes_per_sector = u16::from_le_bytes([boot_sector[0x0B], boot_sector[0x0C]]);
        if ![512, 1024, 2048, 4096].contains(&bytes_per_sector) {
            report.errors.push(format!("Invalid bytes per sector: {} (must be 512, 1024, 2048, or 4096)", bytes_per_sector));
            report.is_valid = false;
        }
        report.info.insert("bytes_per_sector".to_string(), bytes_per_sector.to_string());
        
        // 4. Sectors per Cluster (0x0D)
        let sectors_per_cluster = boot_sector[0x0D];
        if sectors_per_cluster == 0 || (sectors_per_cluster & (sectors_per_cluster - 1)) != 0 {
            report.errors.push(format!("Invalid sectors per cluster: {} (must be power of 2)", sectors_per_cluster));
            report.is_valid = false;
        }
        // Check cluster size doesn't exceed 32KB (FAT16 limit)
        let cluster_size = bytes_per_sector as u32 * sectors_per_cluster as u32;
        if cluster_size > 32768 {
            report.errors.push(format!("Cluster size {} exceeds 32KB limit for FAT16", cluster_size));
            report.is_valid = false;
        }
        report.info.insert("sectors_per_cluster".to_string(), sectors_per_cluster.to_string());
        report.info.insert("cluster_size".to_string(), format!("{} bytes", cluster_size));
        
        // 5. Reserved Sectors (0x0E-0x0F)
        let reserved_sectors = u16::from_le_bytes([boot_sector[0x0E], boot_sector[0x0F]]);
        if reserved_sectors == 0 {
            report.errors.push("Reserved sectors cannot be 0".to_string());
            report.is_valid = false;
        } else if reserved_sectors != 1 {
            report.warnings.push(format!("Non-standard reserved sectors: {} (typically 1 for FAT16)", reserved_sectors));
        }
        report.info.insert("reserved_sectors".to_string(), reserved_sectors.to_string());
        
        // 6. Number of FATs (0x10)
        let num_fats = boot_sector[0x10];
        if num_fats == 0 {
            report.errors.push("Number of FATs cannot be 0".to_string());
            report.is_valid = false;
        } else if num_fats != 2 {
            report.warnings.push(format!("Non-standard number of FATs: {} (typically 2)", num_fats));
        }
        report.info.insert("num_fats".to_string(), num_fats.to_string());
        
        // 7. Root Entries (0x11-0x12)
        let root_entries = u16::from_le_bytes([boot_sector[0x11], boot_sector[0x12]]);
        if root_entries == 0 {
            report.errors.push("Root entries cannot be 0 for FAT16".to_string());
            report.is_valid = false;
        } else if root_entries != 512 {
            report.warnings.push(format!("Non-standard root entries: {} (typically 512 for FAT16)", root_entries));
        }
        report.info.insert("root_entries".to_string(), root_entries.to_string());
        
        // 8. Total Sectors (0x13-0x14 or 0x20-0x23)
        let total_sectors_16 = u16::from_le_bytes([boot_sector[0x13], boot_sector[0x14]]);
        let total_sectors_32 = u32::from_le_bytes([
            boot_sector[0x20], boot_sector[0x21], boot_sector[0x22], boot_sector[0x23]
        ]);
        
        let total_sectors = if total_sectors_16 != 0 {
            if total_sectors_32 != 0 {
                report.errors.push("Both 16-bit and 32-bit total sectors are set (only one should be non-zero)".to_string());
                report.is_valid = false;
            }
            total_sectors_16 as u32
        } else {
            total_sectors_32
        };
        
        if total_sectors == 0 {
            report.errors.push("Total sectors cannot be 0".to_string());
            report.is_valid = false;
        }
        report.info.insert("total_sectors".to_string(), total_sectors.to_string());
        report.info.insert("volume_size".to_string(), format!("{:.2} MB", (total_sectors as f64 * 512.0) / 1048576.0));
        
        // 9. Media Descriptor (0x15)
        let media_descriptor = boot_sector[0x15];
        let media_type = match media_descriptor {
            0xF0 => "Removable media (3.5\" floppy or similar)",
            0xF8 => "Fixed disk",
            0xF9 => "3.5\" double-sided, 80 tracks, 9 or 18 sectors",
            0xFA => "3.5\" single-sided, 80 tracks, 8 sectors",
            0xFB => "3.5\" double-sided, 80 tracks, 8 sectors",
            0xFC => "5.25\" single-sided, 40 tracks, 9 sectors",
            0xFD => "5.25\" double-sided, 40 tracks, 9 sectors",
            0xFE => "5.25\" single-sided, 40 tracks, 8 sectors",
            0xFF => "5.25\" double-sided, 40 tracks, 8 sectors",
            _ => {
                report.warnings.push(format!("Non-standard media descriptor: 0x{:02X}", media_descriptor));
                "Unknown"
            }
        };
        report.info.insert("media_descriptor".to_string(), format!("0x{:02X} ({})", media_descriptor, media_type));
        
        // 10. Sectors per FAT (0x16-0x17)
        let sectors_per_fat = u16::from_le_bytes([boot_sector[0x16], boot_sector[0x17]]);
        if sectors_per_fat == 0 {
            report.errors.push("Sectors per FAT cannot be 0".to_string());
            report.is_valid = false;
        }
        report.info.insert("sectors_per_fat".to_string(), sectors_per_fat.to_string());
        
        // 11. Sectors per Track (0x18-0x19) - CHS Geometry
        let sectors_per_track = u16::from_le_bytes([boot_sector[0x18], boot_sector[0x19]]);
        if sectors_per_track == 0 {
            report.warnings.push("Sectors per track is 0 (CHS geometry not set)".to_string());
        } else if sectors_per_track != 63 {
            report.warnings.push(format!("Non-standard sectors per track: {} (typically 63)", sectors_per_track));
        }
        report.info.insert("sectors_per_track".to_string(), sectors_per_track.to_string());
        
        // 12. Number of Heads (0x1A-0x1B) - CHS Geometry
        let num_heads = u16::from_le_bytes([boot_sector[0x1A], boot_sector[0x1B]]);
        if num_heads == 0 {
            report.warnings.push("Number of heads is 0 (CHS geometry not set)".to_string());
        } else if num_heads != 255 {
            report.warnings.push(format!("Non-standard number of heads: {} (typically 255)", num_heads));
        }
        report.info.insert("num_heads".to_string(), num_heads.to_string());
        
        // 13. Hidden Sectors (0x1C-0x1F)
        let hidden_sectors = u32::from_le_bytes([
            boot_sector[0x1C], boot_sector[0x1D], boot_sector[0x1E], boot_sector[0x1F]
        ]);
        report.info.insert("hidden_sectors".to_string(), hidden_sectors.to_string());
        if partition_offset_sectors.is_some() && hidden_sectors != partition_offset_sectors.unwrap() as u32 {
            report.errors.push(format!("Hidden sectors {} doesn't match partition offset {}", 
                hidden_sectors, partition_offset_sectors.unwrap()));
            report.is_valid = false;
        }
        
        // ========== EXTENDED BPB (optional but common) ==========
        
        // 14. Drive Number (0x24)
        let drive_number = boot_sector[0x24];
        let drive_type = if drive_number == 0x00 {
            "Removable media"
        } else if drive_number == 0x80 {
            "Fixed disk"
        } else {
            report.warnings.push(format!("Non-standard drive number: 0x{:02X} (should be 0x00 or 0x80)", drive_number));
            "Unknown"
        };
        report.info.insert("drive_number".to_string(), format!("0x{:02X} ({})", drive_number, drive_type));
        
        // 15. Reserved byte (0x25)
        if boot_sector[0x25] != 0 {
            report.warnings.push(format!("Reserved byte at 0x25 is not 0: 0x{:02X}", boot_sector[0x25]));
        }
        
        // 16. Extended Boot Signature (0x26)
        let ext_boot_sig = boot_sector[0x26];
        if ext_boot_sig == 0x29 {
            report.info.insert("extended_bpb".to_string(), "Present (0x29)".to_string());
            
            // 17. Volume ID (0x27-0x2A)
            let volume_id = u32::from_le_bytes([
                boot_sector[0x27], boot_sector[0x28], boot_sector[0x29], boot_sector[0x2A]
            ]);
            report.info.insert("volume_id".to_string(), format!("0x{:08X}", volume_id));
            
            // 18. Volume Label (0x2B-0x35)
            let volume_label = String::from_utf8_lossy(&boot_sector[0x2B..0x36]);
            report.info.insert("volume_label".to_string(), format!("'{}'", volume_label.trim()));
            
            // 19. Filesystem Type (0x36-0x3D)
            let fs_type = String::from_utf8_lossy(&boot_sector[0x36..0x3E]);
            report.info.insert("fs_type_string".to_string(), format!("'{}'", fs_type.trim()));
            if !fs_type.contains("FAT16") && !fs_type.contains("FAT") {
                report.warnings.push(format!("FS type string doesn't contain 'FAT16': '{}'", fs_type.trim()));
            }
        } else if ext_boot_sig == 0x28 {
            report.info.insert("extended_bpb".to_string(), "Old format (0x28)".to_string());
        } else {
            report.info.insert("extended_bpb".to_string(), format!("Not present (0x{:02X})", ext_boot_sig));
        }
        
        // 20. Boot Sector Signature (0x1FE-0x1FF)
        if boot_sector[0x1FE] != 0x55 || boot_sector[0x1FF] != 0xAA {
            report.errors.push(format!("Invalid boot sector signature: {:02X}{:02X} (must be 55AA)", 
                boot_sector[0x1FE], boot_sector[0x1FF]));
            report.is_valid = false;
        } else {
            report.info.insert("boot_signature".to_string(), "55AA (valid)".to_string());
        }
        
        // ========== CLUSTER COUNT VALIDATION ==========
        
        let root_dir_sectors = ((root_entries * 32) + (bytes_per_sector - 1)) / bytes_per_sector;
        let data_start = reserved_sectors as u32 + (num_fats as u32 * sectors_per_fat as u32) + root_dir_sectors as u32;
        
        if data_start >= total_sectors {
            report.errors.push(format!("Data start sector {} exceeds total sectors {}", data_start, total_sectors));
            report.is_valid = false;
        } else {
            let data_sectors = total_sectors - data_start;
            let total_clusters = data_sectors / sectors_per_cluster as u32;
            
            report.info.insert("data_start_sector".to_string(), data_start.to_string());
            report.info.insert("data_sectors".to_string(), data_sectors.to_string());
            report.info.insert("total_clusters".to_string(), total_clusters.to_string());
            
            // FAT16 must have 4085-65524 clusters
            if total_clusters < 4085 {
                report.errors.push(format!("Too few clusters for FAT16: {} (minimum 4085, this is FAT12)", total_clusters));
                report.is_valid = false;
            } else if total_clusters > 65524 {
                report.errors.push(format!("Too many clusters for FAT16: {} (maximum 65524, this would be FAT32)", total_clusters));
                report.is_valid = false;
            } else {
                report.info.insert("fat_type_by_clusters".to_string(), "FAT16 (valid range)".to_string());
            }
        }
        
        // ========== FAT TABLE VALIDATION ==========
        
        // Read first FAT entries
        let fat_offset = (reserved_sectors as u64 * bytes_per_sector as u64) + offset_bytes;
        file.seek(SeekFrom::Start(fat_offset))?;
        let mut fat_entries = vec![0u8; 4];
        file.read_exact(&mut fat_entries)?;
        
        // Check FAT[0]
        let fat0 = u16::from_le_bytes([fat_entries[0], fat_entries[1]]);
        if (fat0 & 0xFF) != media_descriptor as u16 {
            report.errors.push(format!("FAT[0] low byte (0x{:02X}) doesn't match media descriptor (0x{:02X})", 
                fat0 & 0xFF, media_descriptor));
            report.is_valid = false;
        }
        if (fat0 >> 8) != 0xFF {
            report.warnings.push(format!("FAT[0] high byte is 0x{:02X} (expected 0xFF)", fat0 >> 8));
        }
        report.info.insert("fat0_entry".to_string(), format!("0x{:04X}", fat0));
        
        // Check FAT[1]
        let fat1 = u16::from_le_bytes([fat_entries[2], fat_entries[3]]);
        if fat1 != 0xFFFF {
            report.warnings.push(format!("FAT[1] is 0x{:04X} (expected 0xFFFF for end-of-chain)", fat1));
        }
        report.info.insert("fat1_entry".to_string(), format!("0x{:04X}", fat1));
        
        Ok(report)
    }
    
    /// Format the validation report as a string
    pub fn format_report(report: &ValidationReport) -> String {
        let mut output = String::new();
        
        output.push_str(&format!("=== FAT16 COMPREHENSIVE VALIDATION REPORT ===\n"));
        output.push_str(&format!("Overall Status: {}\n\n", if report.is_valid { "✅ VALID" } else { "❌ INVALID" }));
        
        output.push_str("=== FILESYSTEM INFORMATION ===\n");
        for (key, value) in &report.info {
            output.push_str(&format!("{:20} : {}\n", key, value));
        }
        
        if !report.warnings.is_empty() {
            output.push_str("\n=== WARNINGS ===\n");
            for warning in &report.warnings {
                output.push_str(&format!("⚠️  {}\n", warning));
            }
        }
        
        if !report.errors.is_empty() {
            output.push_str("\n=== ERRORS ===\n");
            for error in &report.errors {
                output.push_str(&format!("❌ {}\n", error));
            }
        }
        
        output.push_str("\n=== HEX DUMP (First 256 bytes) ===\n");
        for (i, chunk) in report.hex_dump.chunks(16).enumerate() {
            output.push_str(&format!("{:04X}: ", i * 16));
            for byte in chunk {
                output.push_str(&format!("{:02X} ", byte));
            }
            output.push_str(" |");
            for byte in chunk {
                if *byte >= 0x20 && *byte < 0x7F {
                    output.push(*byte as char);
                } else {
                    output.push('.');
                }
            }
            output.push_str("|\n");
        }
        
        output
    }
}