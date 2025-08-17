// Filesystem detection trait and utilities

use moses_core::MosesError;

/// Trait for filesystem-specific detection logic
pub trait FilesystemDetector {
    /// Check if the given boot sector and optional extended data matches this filesystem
    /// 
    /// # Parameters
    /// - `boot_sector`: First 512 bytes (sector 0)
    /// - `ext_superblock`: Optional extended data (e.g., bytes at offset 1024 for ext filesystems)
    /// 
    /// # Returns
    /// - `Some(variant)` if detected (e.g., "ntfs", "ext4", "fat32")
    /// - `None` if not this filesystem
    fn detect(boot_sector: &[u8], ext_superblock: Option<&[u8]>) -> Option<String>;
}

/// Helper to read common detection data from a device
pub fn read_detection_data(file: &mut std::fs::File) -> Result<(Vec<u8>, Option<Vec<u8>>), MosesError> {
    use std::io::{Read, Seek, SeekFrom};
    
    // Read boot sector (first 512 bytes)
    let mut boot_sector = vec![0u8; 512];
    file.read_exact(&mut boot_sector)
        .map_err(|e| MosesError::Other(format!("Failed to read boot sector: {}", e)))?;
    
    // Try to read extended superblock (for ext filesystems)
    // This is at offset 1024
    let ext_superblock = if file.seek(SeekFrom::Start(1024)).is_ok() {
        let mut buffer = vec![0u8; 512];
        if file.read_exact(&mut buffer).is_ok() {
            Some(buffer)
        } else {
            None
        }
    } else {
        None
    };
    
    // Reset file position
    let _ = file.seek(SeekFrom::Start(0));
    
    Ok((boot_sector, ext_superblock))
}

/// Detect filesystem type using all registered detectors
pub fn detect_filesystem(file: &mut std::fs::File) -> Result<String, MosesError> {
    let (boot_sector, ext_superblock) = read_detection_data(file)?;
    
    // Try each filesystem detector
    // NTFS
    if let Some(fs) = crate::ntfs::NtfsDetector::detect(&boot_sector, ext_superblock.as_deref()) {
        return Ok(fs);
    }
    
    // exFAT
    if let Some(fs) = crate::exfat::ExFatDetector::detect(&boot_sector, ext_superblock.as_deref()) {
        return Ok(fs);
    }
    
    // FAT32 (check before FAT16 since FAT32 is more specific)
    if let Some(fs) = crate::fat32::Fat32Detector::detect(&boot_sector, ext_superblock.as_deref()) {
        return Ok(fs);
    }
    
    // FAT16
    if let Some(fs) = crate::fat16::Fat16Detector::detect(&boot_sector, ext_superblock.as_deref()) {
        return Ok(fs);
    }
    
    // ext family (ext2/3/4)
    if let Some(fs) = crate::ext4_native::ExtDetector::detect(&boot_sector, ext_superblock.as_deref()) {
        return Ok(fs);
    }
    
    Ok("unknown".to_string())
}