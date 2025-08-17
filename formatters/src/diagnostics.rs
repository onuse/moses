// Filesystem diagnostics and identification tools

use std::io::{Read, Seek, SeekFrom};
use moses_core::{Device, MosesError};
use log::info;

/// Analyze an unknown filesystem and provide diagnostic information
pub fn analyze_unknown_filesystem(device: &Device) -> Result<String, MosesError> {
    use crate::utils::open_device_with_fallback;
    
    info!("Analyzing unknown filesystem on device: {}", device.name);
    
    let mut file = open_device_with_fallback(device)?;
    let mut report = String::new();
    
    // Read first 512 bytes (boot sector)
    let mut boot_sector = vec![0u8; 512];
    file.read_exact(&mut boot_sector)
        .map_err(|e| MosesError::Other(format!("Failed to read boot sector: {}", e)))?;
    
    report.push_str(&format!("Device: {} ({})\n", device.name, device.id));
    report.push_str(&format!("Size: {:.2} GB\n\n", device.size as f64 / (1024.0 * 1024.0 * 1024.0)));
    
    // Check for common filesystem signatures
    report.push_str("=== Boot Sector Analysis ===\n");
    
    // Check for NTFS
    if boot_sector.len() >= 8 && &boot_sector[3..8] == b"NTFS " {
        report.push_str("DETECTED: NTFS signature found\n");
    }
    
    // Check for FAT32
    if boot_sector.len() >= 87 && &boot_sector[82..87] == b"FAT32" {
        report.push_str("DETECTED: FAT32 signature found\n");
    }
    
    // Check for FAT16
    if boot_sector.len() >= 62 && &boot_sector[54..59] == b"FAT16" {
        report.push_str("DETECTED: FAT16 signature found\n");
    } else if boot_sector.len() >= 57 && &boot_sector[54..57] == b"FAT" {
        report.push_str("DETECTED: FAT (possibly FAT12/16) signature found\n");
    }
    
    // Check for exFAT
    if boot_sector.len() >= 11 && &boot_sector[3..11] == b"EXFAT   " {
        report.push_str("DETECTED: exFAT signature found\n");
    }
    
    // Check jump instruction (common in FAT/NTFS)
    if boot_sector[0] == 0xEB || boot_sector[0] == 0xE9 {
        report.push_str(&format!("Jump instruction found: 0x{:02X} (typical of FAT/NTFS)\n", boot_sector[0]));
    }
    
    // Check for MBR signature
    if boot_sector.len() >= 512 && boot_sector[510] == 0x55 && boot_sector[511] == 0xAA {
        report.push_str("Boot signature 0x55AA found (valid boot sector or MBR)\n");
    }
    
    // Check partition table (if MBR)
    let mut has_partition_table = false;
    for i in 0..4 {
        let offset = 446 + i * 16;
        if offset + 16 <= boot_sector.len() {
            let partition_type = boot_sector[offset + 4];
            if partition_type != 0 {
                has_partition_table = true;
                report.push_str(&format!("Partition {} type: 0x{:02X}", i + 1, partition_type));
                let part_name = match partition_type {
                    0x01 => " (FAT12)",
                    0x04 | 0x06 | 0x0E => " (FAT16)",
                    0x0B | 0x0C => " (FAT32)",
                    0x07 => " (NTFS/exFAT)",
                    0x83 => " (Linux)",
                    0x82 => " (Linux swap)",
                    0x8E => " (Linux LVM)",
                    0xEE => " (GPT protective MBR)",
                    _ => "",
                };
                report.push_str(&format!("{}\n", part_name));
            }
        }
    }
    
    if has_partition_table {
        report.push_str("WARNING: This appears to be an MBR with partition table, not a filesystem\n");
        
        // Check if it's a GPT disk (protective MBR with type 0xEE)
        let has_gpt = (0..4).any(|i| {
            let offset = 446 + i * 16;
            offset + 16 <= boot_sector.len() && boot_sector[offset + 4] == 0xEE
        });
        
        if has_gpt {
            // Try to read GPT header at LBA 1 (offset 512)
            if file.seek(SeekFrom::Start(512)).is_ok() {
                let mut gpt_header = vec![0u8; 512];
                if file.read_exact(&mut gpt_header).is_ok() {
                    // Check GPT signature "EFI PART"
                    if &gpt_header[0..8] == b"EFI PART" {
                        report.push_str("\n=== GPT Header Found ===\n");
                        
                        // Parse GPT header fields
                        let partition_entries_lba = u64::from_le_bytes([
                            gpt_header[72], gpt_header[73], gpt_header[74], gpt_header[75],
                            gpt_header[76], gpt_header[77], gpt_header[78], gpt_header[79]
                        ]);
                        let num_partition_entries = u32::from_le_bytes([
                            gpt_header[80], gpt_header[81], gpt_header[82], gpt_header[83]
                        ]);
                        
                        report.push_str(&format!("Number of partitions: {}\n", num_partition_entries));
                        
                        // Try to read first partition entry
                        if num_partition_entries > 0 {
                            let partition_offset = partition_entries_lba * 512;
                            if file.seek(SeekFrom::Start(partition_offset)).is_ok() {
                                let mut partition_entry = vec![0u8; 128]; // GPT entries are 128 bytes
                                if file.read_exact(&mut partition_entry).is_ok() {
                                    // Check if partition exists (type GUID != all zeros)
                                    if partition_entry[0..16].iter().any(|&b| b != 0) {
                                        let first_lba = u64::from_le_bytes([
                                            partition_entry[32], partition_entry[33], partition_entry[34], partition_entry[35],
                                            partition_entry[36], partition_entry[37], partition_entry[38], partition_entry[39]
                                        ]);
                                        let last_lba = u64::from_le_bytes([
                                            partition_entry[40], partition_entry[41], partition_entry[42], partition_entry[43],
                                            partition_entry[44], partition_entry[45], partition_entry[46], partition_entry[47]
                                        ]);
                                        
                                        report.push_str(&format!("\nFirst partition starts at LBA {} (offset 0x{:X})\n", 
                                            first_lba, first_lba * 512));
                                        report.push_str(&format!("Partition size: {} MB\n", 
                                            (last_lba - first_lba + 1) * 512 / 1024 / 1024));
                                        
                                        // Try to identify filesystem in the partition
                                        let fs_offset = first_lba * 512;
                                        if file.seek(SeekFrom::Start(fs_offset)).is_ok() {
                                            let mut fs_boot = vec![0u8; 512];
                                            if file.read_exact(&mut fs_boot).is_ok() {
                                                report.push_str("\n=== Filesystem in first partition ===\n");
                                                
                                                // Check various filesystem signatures
                                                let oem = String::from_utf8_lossy(&fs_boot[3..11]);
                                                if oem.starts_with("NTFS") {
                                                    report.push_str("DETECTED: NTFS filesystem\n");
                                                } else if oem.starts_with("EXFAT") {
                                                    report.push_str("DETECTED: exFAT filesystem\n");
                                                } else if oem.starts_with("MSDOS") || oem.starts_with("FAT") {
                                                    report.push_str("DETECTED: FAT filesystem\n");
                                                } else if fs_boot[510] == 0x55 && fs_boot[511] == 0xAA {
                                                    report.push_str(&format!("Boot sector with OEM: '{}'\n", oem.trim()));
                                                }
                                                
                                                // Also check for ext at partition + 1024
                                                if file.seek(SeekFrom::Start(fs_offset + 1024)).is_ok() {
                                                    let mut ext_check = vec![0u8; 512];
                                                    if file.read_exact(&mut ext_check).is_ok() {
                                                        if ext_check[56] == 0x53 && ext_check[57] == 0xEF {
                                                            report.push_str("DETECTED: ext2/3/4 filesystem\n");
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Reset file position for further checks
            let _ = file.seek(SeekFrom::Start(0));
        }
    }
    
    // Check for ext2/3/4 at offset 1024
    file.seek(SeekFrom::Start(1024))
        .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
    
    let mut ext_superblock = vec![0u8; 512];
    if file.read_exact(&mut ext_superblock).is_ok() {
        // Check ext magic number at offset 56 in superblock
        if ext_superblock.len() >= 58 && ext_superblock[56] == 0x53 && ext_superblock[57] == 0xEF {
            report.push_str("DETECTED: ext2/3/4 filesystem signature found at offset 1024\n");
            
            // Check for ext variant
            if ext_superblock.len() >= 100 {
                let compat_features = u32::from_le_bytes([
                    ext_superblock[92], ext_superblock[93], ext_superblock[94], ext_superblock[95]
                ]);
                let incompat_features = u32::from_le_bytes([
                    ext_superblock[96], ext_superblock[97], ext_superblock[98], ext_superblock[99]
                ]);
                
                if incompat_features & 0x0040 != 0 {
                    report.push_str("  Variant: ext4 (extents feature)\n");
                } else if compat_features & 0x0004 != 0 {
                    report.push_str("  Variant: ext3 (has_journal feature)\n");
                } else {
                    report.push_str("  Variant: ext2\n");
                }
            }
        }
    }
    
    // Show hex dump of first 128 bytes
    report.push_str("\n=== First 128 bytes (hex) ===\n");
    for i in 0..8 {
        report.push_str(&format!("{:04X}: ", i * 16));
        for j in 0..16 {
            let idx = i * 16 + j;
            if idx < boot_sector.len() {
                report.push_str(&format!("{:02X} ", boot_sector[idx]));
            } else {
                report.push_str("   ");
            }
            if j == 7 {
                report.push_str(" ");
            }
        }
        report.push_str(" |");
        for j in 0..16 {
            let idx = i * 16 + j;
            if idx < boot_sector.len() {
                let byte = boot_sector[idx];
                if byte >= 0x20 && byte < 0x7F {
                    report.push_str(&format!("{}", byte as char));
                } else {
                    report.push_str(".");
                }
            }
        }
        report.push_str("|\n");
    }
    
    // Show ASCII strings found in boot sector
    report.push_str("\n=== ASCII strings found ===\n");
    let mut current_string = String::new();
    for byte in &boot_sector {
        if *byte >= 0x20 && *byte < 0x7F {
            current_string.push(*byte as char);
        } else if current_string.len() >= 4 {
            report.push_str(&format!("  \"{}\"\n", current_string));
            current_string.clear();
        } else {
            current_string.clear();
        }
    }
    if current_string.len() >= 4 {
        report.push_str(&format!("  \"{}\"\n", current_string));
    }
    
    Ok(report)
}

/// Try to identify filesystem with elevated privileges
pub fn identify_with_elevation(device: &Device) -> Result<String, MosesError> {
    // Note: Elevation check should be done at the command/UI level
    // The formatters crate shouldn't depend on platform-specific code
    analyze_unknown_filesystem(device)
}