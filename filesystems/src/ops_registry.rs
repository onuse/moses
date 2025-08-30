// Filesystem operations registry - enhanced version with all filesystems
// Includes read-write support for NTFS

use crate::ops::{FilesystemOps, FilesystemOpsRegistry};
use moses_core::{Device, MosesError};

/// Register all built-in filesystem operations
pub fn register_all_filesystems(registry: &mut FilesystemOpsRegistry, enable_write: bool) {
    use crate::ext4_native::{Ext4Ops, ExtOpsDetector};
    use crate::ntfs::{NtfsOps, NtfsRwOps};
    use crate::fat32::Fat32Ops;
    use crate::fat16::Fat16Ops;
    use crate::exfat::ExFatOps;
    
    // Register ext4 operations (read-only for now)
    registry.register_ops("ext4", |device| {
        let mut ops = Ext4Ops::new(device.clone())?;
        ops.init(device)?;
        Ok(Box::new(ops))
    });
    
    registry.register_ops("ext3", |device| {
        let mut ops = Ext4Ops::new(device.clone())?;
        ops.init(device)?;
        Ok(Box::new(ops))
    });
    
    registry.register_ops("ext2", |device| {
        let mut ops = Ext4Ops::new(device.clone())?;
        ops.init(device)?;
        Ok(Box::new(ops))
    });
    
    // Register NTFS operations
    if enable_write {
        // Use read-write version if writes are enabled
        registry.register_ops("ntfs", |device| {
            let mut ops = NtfsRwOps::new();
            ops.enable_writes(true);  // Enable write support
            ops.init(device)?;
            Ok(Box::new(ops))
        });
    } else {
        // Use read-only version by default
        registry.register_ops("ntfs", |device| {
            let mut ops = NtfsOps::new();
            ops.init(device)?;
            Ok(Box::new(ops))
        });
    }
    
    // Register FAT32 operations (read-only)
    registry.register_ops("fat32", |device| {
        let mut ops = Fat32Ops::new();
        ops.init(device)?;
        Ok(Box::new(ops))
    });
    
    // Register FAT16 operations (read-only)
    registry.register_ops("fat16", |device| {
        let mut ops = Fat16Ops::new();
        ops.init(device)?;
        Ok(Box::new(ops))
    });
    
    // Register exFAT operations (read-only)
    registry.register_ops("exfat", |device| {
        let mut ops = ExFatOps::new();
        ops.init(device)?;
        Ok(Box::new(ops))
    });
    
    // Register filesystem detectors
    registry.register_detector(Box::new(ExtOpsDetector));
    registry.register_detector(Box::new(NtfsDetector));
    registry.register_detector(Box::new(Fat32Detector));
    registry.register_detector(Box::new(Fat16Detector));
    registry.register_detector(Box::new(ExFatDetector));
}

// Filesystem detectors
struct NtfsDetector;
impl crate::ops::FilesystemDetector for NtfsDetector {
    fn detect(&self, device: &Device) -> Result<Option<String>, MosesError> {
        // Read boot sector and check for NTFS signature
        use crate::utils::open_device_with_fallback;
        use std::io::Read;
        
        let mut file = open_device_with_fallback(device)?;
        let mut buffer = vec![0u8; 512];
        file.read_exact(&mut buffer)?;
        
        // Check for NTFS signature at offset 3
        if buffer.len() >= 8 && &buffer[3..8] == b"NTFS " {
            Ok(Some("ntfs".to_string()))
        } else {
            Ok(None)
        }
    }
    
    fn priority(&self) -> i32 { 90 }
}

struct Fat32Detector;
impl crate::ops::FilesystemDetector for Fat32Detector {
    fn detect(&self, device: &Device) -> Result<Option<String>, MosesError> {
        use crate::utils::open_device_with_fallback;
        use std::io::Read;
        
        let mut file = open_device_with_fallback(device)?;
        let mut buffer = vec![0u8; 512];
        file.read_exact(&mut buffer)?;
        
        // Check for FAT32 signature at offset 82
        if buffer.len() >= 87 && &buffer[82..87] == b"FAT32" {
            Ok(Some("fat32".to_string()))
        } else {
            Ok(None)
        }
    }
    
    fn priority(&self) -> i32 { 80 }
}

struct Fat16Detector;
impl crate::ops::FilesystemDetector for Fat16Detector {
    fn detect(&self, device: &Device) -> Result<Option<String>, MosesError> {
        use crate::utils::open_device_with_fallback;
        use std::io::Read;
        
        let mut file = open_device_with_fallback(device)?;
        let mut buffer = vec![0u8; 512];
        file.read_exact(&mut buffer)?;
        
        // Check for FAT16 signature at offset 54
        if buffer.len() >= 62 && &buffer[54..57] == b"FAT" {
            // Additional check to distinguish from FAT32
            if buffer.len() >= 87 && &buffer[82..87] != b"FAT32" {
                Ok(Some("fat16".to_string()))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
    
    fn priority(&self) -> i32 { 70 }
}

struct ExFatDetector;
impl crate::ops::FilesystemDetector for ExFatDetector {
    fn detect(&self, device: &Device) -> Result<Option<String>, MosesError> {
        use crate::utils::open_device_with_fallback;
        use std::io::Read;
        
        let mut file = open_device_with_fallback(device)?;
        let mut buffer = vec![0u8; 512];
        file.read_exact(&mut buffer)?;
        
        // Check for exFAT signature at offset 3
        if buffer.len() >= 11 && &buffer[3..11] == b"EXFAT   " {
            Ok(Some("exfat".to_string()))
        } else {
            Ok(None)
        }
    }
    
    fn priority(&self) -> i32 { 85 }
}