// Improved filesystem diagnostics that properly handles partitions
// This version follows MBR/GPT partitions to analyze the actual filesystems

use std::io::{Read, Seek, SeekFrom};
use moses_core::{Device, MosesError};

/// Comprehensive filesystem analysis that handles partitions properly
pub fn analyze_filesystem_comprehensive<R: Read + Seek>(
    file: &mut R, 
    device: &Device
) -> Result<String, MosesError> {
    let mut report = String::new();
    
    // Read first sector
    let mut sector0 = vec![0u8; 512];
    file.read_exact(&mut sector0)
        .map_err(|e| MosesError::Other(format!("Failed to read sector 0: {}", e)))?;
    
    report.push_str(&format!("Device: {} ({})\n", device.name, device.id));
    report.push_str(&format!("Size: {:.2} GB\n\n", device.size as f64 / (1024.0 * 1024.0 * 1024.0)));
    
    // Check if this is an MBR
    let has_boot_signature = sector0[510] == 0x55 && sector0[511] == 0xAA;
    let mut mbr_partitions = Vec::new();
    
    if has_boot_signature {
        // Check for MBR partitions
        for i in 0..4 {
            let offset = 446 + i * 16;
            let partition_type = sector0[offset + 4];
            if partition_type != 0 {
                let start_lba = u32::from_le_bytes([
                    sector0[offset + 8], sector0[offset + 9], 
                    sector0[offset + 10], sector0[offset + 11]
                ]);
                let size_sectors = u32::from_le_bytes([
                    sector0[offset + 12], sector0[offset + 13],
                    sector0[offset + 14], sector0[offset + 15]
                ]);
                
                mbr_partitions.push((i + 1, partition_type, start_lba, size_sectors));
            }
        }
    }
    
    // Check if it's GPT
    let is_gpt = mbr_partitions.iter().any(|(_, ptype, _, _)| *ptype == 0xEE);
    
    if is_gpt {
        report.push_str("=== GPT Disk Detected ===\n");
        analyze_gpt_partitions(file, &mut report)?;
    } else if !mbr_partitions.is_empty() {
        report.push_str("=== MBR Disk Detected ===\n");
        report.push_str(&format!("Found {} partition(s)\n\n", mbr_partitions.len()));
        
        // Analyze each MBR partition
        for (num, ptype, start_lba, size_sectors) in mbr_partitions {
            let type_name = match ptype {
                0x01 => "FAT12",
                0x04 | 0x06 => "FAT16",
                0x0B | 0x0C => "FAT32",
                0x07 => "NTFS/exFAT",
                0x0E => "FAT16 LBA",
                0x0F => "Extended LBA",
                0x83 => "Linux",
                0x82 => "Linux swap",
                0x8E => "Linux LVM",
                _ => "Unknown",
            };
            
            report.push_str(&format!("=== Partition {} ===\n", num));
            report.push_str(&format!("Type: 0x{:02X} ({})\n", ptype, type_name));
            report.push_str(&format!("Start: LBA {} (offset 0x{:X})\n", start_lba, start_lba as u64 * 512));
            report.push_str(&format!("Size: {} sectors ({:.2} MB)\n", 
                size_sectors, (size_sectors as f64 * 512.0) / 1048576.0));
            
            // Analyze the filesystem in this partition
            analyze_partition_filesystem(file, start_lba as u64, &mut report)?;
            report.push_str("\n");
        }
    } else {
        // No partition table - analyze as direct filesystem
        report.push_str("=== Direct Filesystem (No Partition Table) ===\n");
        analyze_boot_sector(&sector0, &mut report);
        
        // Show hex dump
        report.push_str("\n=== First 256 bytes (hex) ===\n");
        for (i, chunk) in sector0[..256.min(sector0.len())].chunks(16).enumerate() {
            report.push_str(&format!("{:04X}: ", i * 16));
            for byte in chunk {
                report.push_str(&format!("{:02X} ", byte));
            }
            report.push_str(" |");
            for byte in chunk {
                if *byte >= 0x20 && *byte < 0x7F {
                    report.push(*byte as char);
                } else {
                    report.push('.');
                }
            }
            report.push_str("|\n");
        }
    }
    
    // Extract ASCII strings for additional clues
    report.push_str("\n=== ASCII strings found ===\n");
    extract_ascii_strings(&sector0, &mut report);
    
    Ok(report)
}

/// Analyze a partition's filesystem
fn analyze_partition_filesystem<R: Read + Seek>(
    file: &mut R,
    start_lba: u64,
    report: &mut String
) -> Result<(), MosesError> {
    let offset = start_lba * 512;
    
    // Seek to partition start
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| MosesError::Other(format!("Failed to seek to partition at LBA {}: {}", start_lba, e)))?;
    
    // Read partition boot sector
    let mut boot_sector = vec![0u8; 512];
    file.read_exact(&mut boot_sector)
        .map_err(|e| MosesError::Other(format!("Failed to read partition boot sector: {}", e)))?;
    
    report.push_str("\nFilesystem Analysis:\n");
    analyze_boot_sector(&boot_sector, report);
    
    // Show first 128 bytes of the partition
    report.push_str("\nFirst 128 bytes of partition (hex):\n");
    for (i, chunk) in boot_sector[..128.min(boot_sector.len())].chunks(16).enumerate() {
        report.push_str(&format!("{:04X}: ", i * 16));
        for byte in chunk {
            report.push_str(&format!("{:02X} ", byte));
        }
        report.push_str(" |");
        for byte in chunk {
            if *byte >= 0x20 && *byte < 0x7F {
                report.push(*byte as char);
            } else {
                report.push('.');
            }
        }
        report.push_str("|\n");
    }
    
    Ok(())
}

/// Analyze GPT partitions
fn analyze_gpt_partitions<R: Read + Seek>(
    file: &mut R,
    report: &mut String
) -> Result<(), MosesError> {
    // Read GPT header at LBA 1
    file.seek(SeekFrom::Start(512))
        .map_err(|e| MosesError::Other(format!("Failed to seek to GPT header: {}", e)))?;
    
    let mut gpt_header = vec![0u8; 512];
    file.read_exact(&mut gpt_header)
        .map_err(|e| MosesError::Other(format!("Failed to read GPT header: {}", e)))?;
    
    // Verify GPT signature
    if &gpt_header[0..8] != b"EFI PART" {
        report.push_str("ERROR: Invalid GPT signature\n");
        return Ok(());
    }
    
    // Parse GPT header
    let partition_entries_lba = u64::from_le_bytes([
        gpt_header[72], gpt_header[73], gpt_header[74], gpt_header[75],
        gpt_header[76], gpt_header[77], gpt_header[78], gpt_header[79]
    ]);
    let num_partition_entries = u32::from_le_bytes([
        gpt_header[80], gpt_header[81], gpt_header[82], gpt_header[83]
    ]);
    
    report.push_str(&format!("Number of partition entries: {}\n", num_partition_entries));
    
    // Read partition entries
    file.seek(SeekFrom::Start(partition_entries_lba * 512))
        .map_err(|e| MosesError::Other(format!("Failed to seek to partition entries: {}", e)))?;
    
    let mut found_partitions = 0;
    for i in 0..num_partition_entries.min(128) {
        // Read partition entry (128 bytes)
        let mut entry = vec![0u8; 128];
        file.read_exact(&mut entry)
            .map_err(|e| MosesError::Other(format!("Failed to read partition entry {}: {}", i, e)))?;
        
        // Check if partition exists (type GUID != all zeros)
        if entry[0..16].iter().any(|&b| b != 0) {
            found_partitions += 1;
            
            let first_lba = u64::from_le_bytes([
                entry[32], entry[33], entry[34], entry[35],
                entry[36], entry[37], entry[38], entry[39]
            ]);
            let last_lba = u64::from_le_bytes([
                entry[40], entry[41], entry[42], entry[43],
                entry[44], entry[45], entry[46], entry[47]
            ]);
            
            report.push_str(&format!("\n=== GPT Partition {} ===\n", found_partitions));
            report.push_str(&format!("Start: LBA {} (offset 0x{:X})\n", first_lba, first_lba * 512));
            report.push_str(&format!("Size: {:.2} MB\n", 
                ((last_lba - first_lba + 1) * 512) as f64 / 1048576.0));
            
            // Analyze filesystem in this partition
            let current_pos = file.stream_position()
                .map_err(|e| MosesError::Other(format!("Failed to get position: {}", e)))?;
            
            analyze_partition_filesystem(file, first_lba, report)?;
            
            // Restore position for next entry
            file.seek(SeekFrom::Start(current_pos))
                .map_err(|e| MosesError::Other(format!("Failed to restore position: {}", e)))?;
        }
    }
    
    if found_partitions == 0 {
        report.push_str("\nNo active partitions found\n");
    }
    
    Ok(())
}

/// Analyze a boot sector for filesystem signatures
fn analyze_boot_sector(boot_sector: &[u8], report: &mut String) {
    // Check jump instruction
    if boot_sector[0] == 0xEB || boot_sector[0] == 0xE9 {
        report.push_str(&format!("Jump instruction: 0x{:02X}\n", boot_sector[0]));
    }
    
    // Check OEM name (bytes 3-10)
    if boot_sector.len() >= 11 {
        let oem = String::from_utf8_lossy(&boot_sector[3..11]);
        report.push_str(&format!("OEM Name: '{}'\n", oem.trim()));
        
        // Identify filesystem by OEM
        if oem.starts_with("NTFS") {
            report.push_str("**DETECTED: NTFS**\n");
        } else if oem.starts_with("EXFAT") {
            report.push_str("**DETECTED: exFAT**\n");
        } else if oem.starts_with("MSDOS") || oem.starts_with("MSWIN") {
            report.push_str("Likely FAT filesystem\n");
        }
    }
    
    // Check for FAT16/FAT32 signatures
    if boot_sector.len() >= 87 {
        // FAT32 at offset 82
        if &boot_sector[82..87] == b"FAT32" {
            report.push_str("**DETECTED: FAT32** (signature at offset 82)\n");
        }
        // FAT16 at offset 54
        else if boot_sector.len() >= 59 && &boot_sector[54..59] == b"FAT16" {
            report.push_str("**DETECTED: FAT16** (signature at offset 54)\n");
        } else if boot_sector.len() >= 57 && &boot_sector[54..57] == b"FAT" {
            report.push_str("**DETECTED: FAT12/16** (signature at offset 54)\n");
        }
    }
    
    // Parse BPB for FAT filesystems
    if boot_sector.len() >= 36 && (boot_sector[0] == 0xEB || boot_sector[0] == 0xE9) {
        let bytes_per_sector = u16::from_le_bytes([boot_sector[11], boot_sector[12]]);
        let sectors_per_cluster = boot_sector[13];
        let reserved_sectors = u16::from_le_bytes([boot_sector[14], boot_sector[15]]);
        let num_fats = boot_sector[16];
        let root_entries = u16::from_le_bytes([boot_sector[17], boot_sector[18]]);
        
        if bytes_per_sector > 0 && sectors_per_cluster > 0 {
            report.push_str("\nBIOS Parameter Block (BPB):\n");
            report.push_str(&format!("  Bytes per sector: {}\n", bytes_per_sector));
            report.push_str(&format!("  Sectors per cluster: {}\n", sectors_per_cluster));
            report.push_str(&format!("  Reserved sectors: {}\n", reserved_sectors));
            report.push_str(&format!("  Number of FATs: {}\n", num_fats));
            report.push_str(&format!("  Root entries: {}\n", root_entries));
            
            // Calculate cluster count to determine FAT type
            if boot_sector.len() >= 36 {
                let total_sectors_16 = u16::from_le_bytes([boot_sector[19], boot_sector[20]]);
                let sectors_per_fat = u16::from_le_bytes([boot_sector[22], boot_sector[23]]);
                let total_sectors_32 = if boot_sector.len() >= 36 {
                    u32::from_le_bytes([boot_sector[32], boot_sector[33], boot_sector[34], boot_sector[35]])
                } else { 0 };
                
                let total_sectors = if total_sectors_16 != 0 {
                    total_sectors_16 as u32
                } else {
                    total_sectors_32
                };
                
                if total_sectors > 0 && sectors_per_fat > 0 {
                    let root_dir_sectors = ((root_entries * 32) + (bytes_per_sector - 1)) / bytes_per_sector;
                    let data_start = reserved_sectors as u32 + (num_fats as u32 * sectors_per_fat as u32) + root_dir_sectors as u32;
                    
                    if data_start < total_sectors {
                        let data_sectors = total_sectors - data_start;
                        let total_clusters = data_sectors / sectors_per_cluster as u32;
                        
                        report.push_str(&format!("  Total clusters: {}\n", total_clusters));
                        
                        if total_clusters < 4085 {
                            report.push_str("  **Type: FAT12** (based on cluster count)\n");
                        } else if total_clusters < 65525 {
                            report.push_str("  **Type: FAT16** (based on cluster count)\n");
                        } else {
                            report.push_str("  **Type: FAT32** (based on cluster count)\n");
                        }
                    }
                }
            }
        }
    }
    
    // Check boot signature
    if boot_sector.len() >= 512 && boot_sector[510] == 0x55 && boot_sector[511] == 0xAA {
        report.push_str("Boot signature: 0x55AA (valid)\n");
    }
}

/// Extract ASCII strings from binary data
fn extract_ascii_strings(data: &[u8], report: &mut String) {
    let mut current_string = String::new();
    
    for &byte in data.iter().take(256) {
        if byte >= 0x20 && byte < 0x7F {
            current_string.push(byte as char);
        } else if !current_string.is_empty() {
            if current_string.len() >= 4 {
                report.push_str(&format!("  '{}'\n", current_string));
            }
            current_string.clear();
        }
    }
    
    if !current_string.is_empty() && current_string.len() >= 4 {
        report.push_str(&format!("  '{}'\n", current_string));
    }
}