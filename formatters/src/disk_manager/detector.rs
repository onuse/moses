// Conflict Detector - Identify partition table conflicts and issues
use std::io::{Read, Seek, SeekFrom};
use moses_core::{Device, MosesError};
use super::converter::PartitionStyle;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskConflict {
    pub severity: ConflictSeverity,
    pub description: String,
    pub resolution: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ConflictSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictReport {
    pub current_state: String,
    pub detected_style: PartitionStyle,
    pub conflicts: Vec<DiskConflict>,
    pub recommendations: Vec<String>,
}

pub struct ConflictDetector;

impl ConflictDetector {
    /// Analyze a disk for partition table conflicts
    pub fn analyze(device: &Device) -> Result<ConflictReport, MosesError> {
        log::info!("Analyzing {} for partition table conflicts", device.name);
        
        #[cfg(target_os = "windows")]
        {
            Self::analyze_windows(device)
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            Self::analyze_unix(device)
        }
    }
    
    #[cfg(target_os = "windows")]
    fn analyze_windows(device: &Device) -> Result<ConflictReport, MosesError> {
        use std::fs::OpenOptions;
        use std::os::windows::fs::OpenOptionsExt;
        use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ};
        
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .access_mode(GENERIC_READ)
            .open(&device.id)
            .map_err(|e| MosesError::IoError(e))?;
        
        Self::perform_analysis(&mut file, device)
    }
    
    #[cfg(not(target_os = "windows"))]
    fn analyze_unix(device: &Device) -> Result<ConflictReport, MosesError> {
        use std::fs::OpenOptions;
        
        let mut file = OpenOptions::new()
            .read(true)
            .open(&device.id)
            .map_err(|e| MosesError::IoError(e))?;
        
        Self::perform_analysis(&mut file, device)
    }
    
    fn perform_analysis<R: Read + Seek>(reader: &mut R, device: &Device) -> Result<ConflictReport, MosesError> {
        let mut conflicts = Vec::new();
        let mut recommendations = Vec::new();
        
        // Read MBR
        reader.seek(SeekFrom::Start(0))
            .map_err(|e| MosesError::Other(format!("Failed to seek: {}", e)))?;
        
        let mut mbr = vec![0u8; 512];
        reader.read_exact(&mut mbr)
            .map_err(|e| MosesError::Other(format!("Failed to read MBR: {}", e)))?;
        
        // Read GPT header
        let mut gpt_header = vec![0u8; 512];
        reader.read_exact(&mut gpt_header)
            .map_err(|e| MosesError::Other(format!("Failed to read GPT header: {}", e)))?;
        
        // Analyze MBR
        let has_mbr_signature = mbr[0x1FE] == 0x55 && mbr[0x1FF] == 0xAA;
        let has_disk_signature = mbr[0x1B8..0x1BC] != [0, 0, 0, 0];
        let has_mbr_partitions = Self::check_mbr_partitions(&mbr);
        let has_protective_mbr = mbr[0x1BE + 4] == 0xEE;
        
        // Analyze GPT
        let has_gpt_signature = &gpt_header[0..8] == b"EFI PART";
        
        // Try to read backup GPT if disk is large enough
        let has_backup_gpt = if device.size > 34 * 512 {
            reader.seek(SeekFrom::Start(device.size - 512))
                .ok()
                .and_then(|_| {
                    let mut backup = vec![0u8; 512];
                    reader.read_exact(&mut backup).ok()?;
                    Some(&backup[0..8] == b"EFI PART")
                })
                .unwrap_or(false)
        } else {
            false
        };
        
        // Determine current state and conflicts
        let detected_style = if has_gpt_signature {
            PartitionStyle::GPT
        } else if has_mbr_signature && (has_mbr_partitions || has_disk_signature) {
            PartitionStyle::MBR
        } else {
            PartitionStyle::Uninitialized
        };
        
        let current_state = match (has_mbr_signature, has_gpt_signature, has_protective_mbr) {
            (false, false, _) => "Uninitialized disk (no partition table)".to_string(),
            (true, false, _) if has_mbr_partitions => "MBR disk with partitions".to_string(),
            (true, false, _) if !has_mbr_partitions => "MBR disk without partitions".to_string(),
            (true, true, true) => "GPT disk with protective MBR".to_string(),
            (true, true, false) => "HYBRID: GPT with non-protective MBR (conflict!)".to_string(),
            (false, true, _) => "INVALID: GPT without MBR (corrupt!)".to_string(),
            _ => "Unknown state".to_string(),
        };
        
        // Check for conflicts
        
        // 1. Missing MBR disk signature (Windows requirement)
        if has_mbr_signature && !has_disk_signature && !has_protective_mbr {
            conflicts.push(DiskConflict {
                severity: ConflictSeverity::Critical,
                description: "MBR disk signature is missing (all zeros)".to_string(),
                resolution: "Windows requires a disk signature to recognize MBR disks".to_string(),
            });
            recommendations.push("Add a disk signature using diskpart: UNIQUEID DISK ID=<signature>".to_string());
        }
        
        // 2. GPT without protective MBR
        if has_gpt_signature && !has_protective_mbr {
            conflicts.push(DiskConflict {
                severity: ConflictSeverity::Critical,
                description: "GPT disk missing protective MBR".to_string(),
                resolution: "GPT disks must have a protective MBR to prevent legacy tools from damaging them".to_string(),
            });
            recommendations.push("Repair GPT structures or convert to proper GPT".to_string());
        }
        
        // 3. Hybrid MBR/GPT (dangerous)
        if has_gpt_signature && has_mbr_partitions && !has_protective_mbr {
            conflicts.push(DiskConflict {
                severity: ConflictSeverity::Critical,
                description: "Hybrid MBR/GPT detected - both partition tables present".to_string(),
                resolution: "This configuration can cause data loss. Choose either MBR or GPT".to_string(),
            });
            recommendations.push("Convert to pure GPT or pure MBR using disk management tools".to_string());
        }
        
        // 4. GPT remnants on MBR disk
        if !has_gpt_signature && has_backup_gpt {
            conflicts.push(DiskConflict {
                severity: ConflictSeverity::Warning,
                description: "Backup GPT found but no primary GPT - possible remnants".to_string(),
                resolution: "Previous GPT formatting left backup structures".to_string(),
            });
            recommendations.push("Clean disk to remove GPT remnants".to_string());
        }
        
        // 5. Corrupted GPT
        if has_gpt_signature {
            // Check GPT revision
            let revision = u32::from_le_bytes([
                gpt_header[8], gpt_header[9], gpt_header[10], gpt_header[11]
            ]);
            if revision != 0x00010000 {
                conflicts.push(DiskConflict {
                    severity: ConflictSeverity::Warning,
                    description: format!("Non-standard GPT revision: 0x{:08X}", revision),
                    resolution: "GPT revision should be 1.0 (0x00010000)".to_string(),
                });
            }
            
            // Check if backup GPT is missing
            if !has_backup_gpt && device.size > 34 * 512 {
                conflicts.push(DiskConflict {
                    severity: ConflictSeverity::Warning,
                    description: "Primary GPT present but backup GPT missing or corrupt".to_string(),
                    resolution: "GPT should have a backup at the end of the disk".to_string(),
                });
                recommendations.push("Repair GPT using gdisk or similar tool".to_string());
            }
        }
        
        // 6. Check for superfloppy format on large disk
        if !has_mbr_partitions && !has_gpt_signature && device.size > 2 * 1024 * 1024 * 1024 {
            // Check if there's a filesystem directly at sector 0
            if Self::has_filesystem_at_start(&mbr) {
                conflicts.push(DiskConflict {
                    severity: ConflictSeverity::Warning,
                    description: "Superfloppy format detected (no partition table)".to_string(),
                    resolution: "Large disks should have a partition table for compatibility".to_string(),
                });
                recommendations.push("Create a partition table (MBR or GPT) for better compatibility".to_string());
            }
        }
        
        // 7. MBR on very large disk
        if detected_style == PartitionStyle::MBR && device.size > 2199023255552 { // 2TB
            conflicts.push(DiskConflict {
                severity: ConflictSeverity::Critical,
                description: "MBR partition table on disk larger than 2TB".to_string(),
                resolution: "MBR cannot address space beyond 2TB. Convert to GPT".to_string(),
            });
            recommendations.push("Convert to GPT to use full disk capacity".to_string());
        }
        
        // Add general recommendations based on state
        if conflicts.is_empty() {
            match detected_style {
                PartitionStyle::Uninitialized => {
                    recommendations.push("Disk is uninitialized. Create MBR or GPT partition table".to_string());
                },
                PartitionStyle::MBR => {
                    if !has_mbr_partitions {
                        recommendations.push("MBR disk has no partitions. Create a partition to use the disk".to_string());
                    }
                },
                PartitionStyle::GPT => {
                    recommendations.push("GPT disk appears healthy".to_string());
                },
            }
        } else {
            // Sort conflicts by severity
            conflicts.sort_by(|a, b| b.severity.cmp(&a.severity));
            
            if conflicts.iter().any(|c| c.severity == ConflictSeverity::Critical) {
                recommendations.insert(0, "CRITICAL: Disk has serious conflicts that need immediate attention".to_string());
            }
        }
        
        Ok(ConflictReport {
            current_state,
            detected_style,
            conflicts,
            recommendations,
        })
    }
    
    /// Check if MBR has valid partitions
    fn check_mbr_partitions(mbr: &[u8]) -> bool {
        for i in 0..4 {
            let offset = 0x1BE + (i * 16);
            let partition_type = mbr[offset + 4];
            
            // Check if partition type is non-zero (indicates partition exists)
            if partition_type != 0 && partition_type != 0xEE {
                // Also check that LBA values are reasonable
                let start_lba = u32::from_le_bytes([
                    mbr[offset + 8],
                    mbr[offset + 9],
                    mbr[offset + 10],
                    mbr[offset + 11],
                ]);
                
                let size_lba = u32::from_le_bytes([
                    mbr[offset + 12],
                    mbr[offset + 13],
                    mbr[offset + 14],
                    mbr[offset + 15],
                ]);
                
                if start_lba > 0 && size_lba > 0 {
                    return true;
                }
            }
        }
        false
    }
    
    /// Check if there's a filesystem signature at the start of the disk
    fn has_filesystem_at_start(mbr: &[u8]) -> bool {
        // Check for common filesystem signatures
        
        // FAT12/16/32 signatures
        if mbr.len() >= 512 {
            // Check for FAT boot sector signature
            if mbr[0x1FE] == 0x55 && mbr[0x1FF] == 0xAA {
                // Check for jump instruction (common in boot sectors)
                if mbr[0] == 0xEB || mbr[0] == 0xE9 {
                    // Check for OEM string area (should be ASCII)
                    let oem_valid = mbr[3..11].iter().all(|&b| b.is_ascii());
                    if oem_valid {
                        return true;
                    }
                }
            }
        }
        
        // NTFS signature
        if mbr.len() >= 8 && &mbr[3..8] == b"NTFS " {
            return true;
        }
        
        // exFAT signature
        if mbr.len() >= 11 && &mbr[3..11] == b"EXFAT   " {
            return true;
        }
        
        false
    }
    
    /// Quick check for specific conflict types
    pub fn has_gpt_mbr_conflict(device: &Device) -> Result<bool, MosesError> {
        let report = Self::analyze(device)?;
        Ok(report.conflicts.iter().any(|c| {
            c.description.contains("Hybrid MBR/GPT") || 
            c.description.contains("GPT remnants") ||
            c.description.contains("missing protective MBR")
        }))
    }
    
    /// Check if disk needs cleaning before formatting
    pub fn needs_cleaning(device: &Device) -> Result<bool, MosesError> {
        let report = Self::analyze(device)?;
        Ok(report.conflicts.iter().any(|c| c.severity == ConflictSeverity::Critical))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mbr_partition_check() {
        let mut mbr = vec![0u8; 512];
        
        // No partitions
        assert!(!ConflictDetector::check_mbr_partitions(&mbr));
        
        // Add a valid partition
        let offset = 0x1BE;
        mbr[offset + 4] = 0x06; // FAT16 type
        mbr[offset + 8..offset + 12].copy_from_slice(&2048u32.to_le_bytes()); // Start LBA
        mbr[offset + 12..offset + 16].copy_from_slice(&100000u32.to_le_bytes()); // Size
        
        assert!(ConflictDetector::check_mbr_partitions(&mbr));
        
        // Protective MBR partition (0xEE) should not count
        mbr[offset + 4] = 0xEE;
        assert!(!ConflictDetector::check_mbr_partitions(&mbr));
    }
    
    #[test]
    fn test_filesystem_detection() {
        // Test FAT signature
        let mut fat_boot = vec![0u8; 512];
        fat_boot[0] = 0xEB; // Jump instruction
        fat_boot[3..11].copy_from_slice(b"MSDOS5.0");
        fat_boot[0x1FE] = 0x55;
        fat_boot[0x1FF] = 0xAA;
        assert!(ConflictDetector::has_filesystem_at_start(&fat_boot));
        
        // Test NTFS signature
        let mut ntfs_boot = vec![0u8; 512];
        ntfs_boot[3..8].copy_from_slice(b"NTFS ");
        assert!(ConflictDetector::has_filesystem_at_start(&ntfs_boot));
        
        // Test empty/no filesystem
        let empty = vec![0u8; 512];
        assert!(!ConflictDetector::has_filesystem_at_start(&empty));
    }
}