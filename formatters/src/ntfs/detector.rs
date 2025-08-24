// NTFS filesystem detector
// Identifies NTFS volumes from boot sector

use crate::detection::FilesystemDetector;

pub struct NtfsDetector;

impl FilesystemDetector for NtfsDetector {
    fn detect(boot_sector: &[u8], _ext_superblock: Option<&[u8]>) -> Option<String> {
        if boot_sector.len() < 512 {
            return None;
        }
        
        // Check for boot sector signature
        if boot_sector[0x1FE] != 0x55 || boot_sector[0x1FF] != 0xAA {
            return None;
        }
        
        // Check for NTFS OEM ID at offset 0x03
        if &boot_sector[0x03..0x0B] != b"NTFS    " {
            return None;
        }
        
        // Additional validation to avoid false positives
        
        // Bytes per sector should be a valid value
        let bytes_per_sector = u16::from_le_bytes([boot_sector[0x0B], boot_sector[0x0C]]);
        if ![512, 1024, 2048, 4096].contains(&bytes_per_sector) {
            return None;
        }
        
        // Sectors per cluster must be power of 2
        let sectors_per_cluster = boot_sector[0x0D];
        if sectors_per_cluster == 0 || sectors_per_cluster & (sectors_per_cluster - 1) != 0 {
            return None;
        }
        
        // Reserved sectors must be 0 for NTFS
        let reserved_sectors = u16::from_le_bytes([boot_sector[0x0E], boot_sector[0x0F]]);
        if reserved_sectors != 0 {
            return None;
        }
        
        // Media descriptor should be 0xF8 for hard disk or 0xF0 for removable
        let media_descriptor = boot_sector[0x15];
        if media_descriptor != 0xF8 && media_descriptor != 0xF0 {
            return None;
        }
        
        // MFT cluster should be reasonable (usually starts early in the volume)
        let mft_lcn = u64::from_le_bytes([
            boot_sector[0x30], boot_sector[0x31], boot_sector[0x32], boot_sector[0x33],
            boot_sector[0x34], boot_sector[0x35], boot_sector[0x36], boot_sector[0x37],
        ]);
        
        // MFT typically starts at cluster 4 or near the beginning
        if mft_lcn == 0 || mft_lcn > 1000000 {
            // Arbitrary upper limit to catch corrupt data
            return None;
        }
        
        Some("ntfs".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ntfs_detection() {
        // Create a minimal valid NTFS boot sector
        let mut boot_sector = vec![0u8; 512];
        
        // OEM ID
        boot_sector[3..11].copy_from_slice(b"NTFS    ");
        
        // Bytes per sector (512)
        boot_sector[0x0B] = 0x00;
        boot_sector[0x0C] = 0x02;
        
        // Sectors per cluster (8)
        boot_sector[0x0D] = 8;
        
        // Reserved sectors (0)
        boot_sector[0x0E] = 0;
        boot_sector[0x0F] = 0;
        
        // Media descriptor (0xF8)
        boot_sector[0x15] = 0xF8;
        
        // MFT LCN (4)
        boot_sector[0x30] = 4;
        
        // Boot signature
        boot_sector[0x1FE] = 0x55;
        boot_sector[0x1FF] = 0xAA;
        
        assert_eq!(NtfsDetector::detect(&boot_sector, None), Some("ntfs".to_string()));
    }
    
    #[test]
    fn test_not_ntfs() {
        // Test with FAT32 boot sector
        let mut boot_sector = vec![0u8; 512];
        boot_sector[3..11].copy_from_slice(b"MSDOS5.0");
        boot_sector[0x1FE] = 0x55;
        boot_sector[0x1FF] = 0xAA;
        
        assert_eq!(NtfsDetector::detect(&boot_sector, None), None);
    }
    
    #[test]
    fn test_invalid_ntfs() {
        // NTFS signature but invalid parameters
        let mut boot_sector = vec![0u8; 512];
        boot_sector[3..11].copy_from_slice(b"NTFS    ");
        boot_sector[0x0D] = 3; // Invalid sectors per cluster (not power of 2)
        boot_sector[0x1FE] = 0x55;
        boot_sector[0x1FF] = 0xAA;
        
        assert_eq!(NtfsDetector::detect(&boot_sector, None), None);
    }
}