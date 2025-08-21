// Proper FAT16 detection based on cluster count calculation
// This follows the Microsoft FAT specification for determining FAT type

use crate::detection::FilesystemDetector;

pub struct Fat16ProperDetector;

impl FilesystemDetector for Fat16ProperDetector {
    fn detect(boot_sector: &[u8], _ext_superblock: Option<&[u8]>) -> Option<String> {
        if boot_sector.len() < 512 {
            return None;
        }
        
        // Check for valid boot sector signature
        if boot_sector[0x1FE] != 0x55 || boot_sector[0x1FF] != 0xAA {
            return None;
        }
        
        // Parse BPB (BIOS Parameter Block) fields
        let bytes_per_sector = u16::from_le_bytes([boot_sector[0x0B], boot_sector[0x0C]]);
        let sectors_per_cluster = boot_sector[0x0D];
        let reserved_sectors = u16::from_le_bytes([boot_sector[0x0E], boot_sector[0x0F]]);
        let num_fats = boot_sector[0x10];
        let root_entries = u16::from_le_bytes([boot_sector[0x11], boot_sector[0x12]]);
        let total_sectors_16 = u16::from_le_bytes([boot_sector[0x13], boot_sector[0x14]]);
        let sectors_per_fat = u16::from_le_bytes([boot_sector[0x16], boot_sector[0x17]]);
        let total_sectors_32 = u32::from_le_bytes([
            boot_sector[0x20], boot_sector[0x21], boot_sector[0x22], boot_sector[0x23]
        ]);
        
        // Validate basic parameters
        if bytes_per_sector == 0 || sectors_per_cluster == 0 || num_fats == 0 {
            return None;
        }
        
        // Valid bytes per sector values
        if ![512, 1024, 2048, 4096].contains(&bytes_per_sector) {
            return None;
        }
        
        // Sectors per cluster must be power of 2
        if sectors_per_cluster & (sectors_per_cluster - 1) != 0 {
            return None;
        }
        
        // Determine total sectors
        let total_sectors = if total_sectors_16 != 0 {
            total_sectors_16 as u32
        } else {
            total_sectors_32
        };
        
        if total_sectors == 0 {
            return None;
        }
        
        // Calculate the count of clusters
        // This is the standard way to determine FAT type
        let root_dir_sectors = ((root_entries * 32) + (bytes_per_sector - 1)) / bytes_per_sector;
        let data_start = reserved_sectors as u32 + (num_fats as u32 * sectors_per_fat as u32) + root_dir_sectors as u32;
        
        if data_start >= total_sectors {
            return None; // Invalid filesystem
        }
        
        let data_sectors = total_sectors - data_start;
        let total_clusters = data_sectors / sectors_per_cluster as u32;
        
        // Determine FAT type based on cluster count
        // This is the definitive way according to Microsoft's specification
        if total_clusters < 4085 {
            // FAT12
            Some("fat12".to_string())
        } else if total_clusters < 65525 {
            // FAT16
            Some("fat16".to_string())
        } else {
            // FAT32 or invalid for standard FAT16
            // Check if this might be FAT32 (has extended fields)
            if root_entries == 0 && boot_sector.len() >= 90 {
                // FAT32 has root_entries = 0 and extended BPB
                let fs_type = &boot_sector[82..87];
                if fs_type == b"FAT32" {
                    Some("fat32".to_string())
                } else {
                    // Might still be FAT32 even without the string
                    Some("fat32".to_string())
                }
            } else {
                // Too many clusters for FAT16, but not FAT32 structure
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fat16_detection() {
        // Create a minimal valid FAT16 boot sector
        let mut boot_sector = vec![0u8; 512];
        
        // Boot jump
        boot_sector[0] = 0xEB;
        boot_sector[1] = 0x3C;
        boot_sector[2] = 0x90;
        
        // Bytes per sector (512)
        boot_sector[0x0B] = 0x00;
        boot_sector[0x0C] = 0x02;
        
        // Sectors per cluster (4)
        boot_sector[0x0D] = 4;
        
        // Reserved sectors (1)
        boot_sector[0x0E] = 1;
        boot_sector[0x0F] = 0;
        
        // Number of FATs (2)
        boot_sector[0x10] = 2;
        
        // Root entries (512)
        boot_sector[0x11] = 0x00;
        boot_sector[0x12] = 0x02;
        
        // Total sectors for ~100MB volume (204800 sectors)
        boot_sector[0x20] = 0x00;
        boot_sector[0x21] = 0x20;
        boot_sector[0x22] = 0x03;
        boot_sector[0x23] = 0x00;
        
        // Sectors per FAT (200)
        boot_sector[0x16] = 0xC8;
        boot_sector[0x17] = 0x00;
        
        // Boot signature
        boot_sector[0x1FE] = 0x55;
        boot_sector[0x1FF] = 0xAA;
        
        let result = Fat16ProperDetector::detect(&boot_sector, None);
        assert_eq!(result, Some("fat16".to_string()));
    }
    
    #[test]
    fn test_fat12_detection() {
        // Create a minimal valid FAT12 boot sector (small volume)
        let mut boot_sector = vec![0u8; 512];
        
        // Boot jump
        boot_sector[0] = 0xEB;
        boot_sector[1] = 0x3C;
        boot_sector[2] = 0x90;
        
        // Bytes per sector (512)
        boot_sector[0x0B] = 0x00;
        boot_sector[0x0C] = 0x02;
        
        // Sectors per cluster (1)
        boot_sector[0x0D] = 1;
        
        // Reserved sectors (1)
        boot_sector[0x0E] = 1;
        boot_sector[0x0F] = 0;
        
        // Number of FATs (2)
        boot_sector[0x10] = 2;
        
        // Root entries (224)
        boot_sector[0x11] = 0xE0;
        boot_sector[0x12] = 0x00;
        
        // Total sectors for 1.44MB floppy (2880)
        boot_sector[0x13] = 0x40;
        boot_sector[0x14] = 0x0B;
        
        // Sectors per FAT (9)
        boot_sector[0x16] = 0x09;
        boot_sector[0x17] = 0x00;
        
        // Boot signature
        boot_sector[0x1FE] = 0x55;
        boot_sector[0x1FF] = 0xAA;
        
        let result = Fat16ProperDetector::detect(&boot_sector, None);
        assert_eq!(result, Some("fat12".to_string()));
    }
}