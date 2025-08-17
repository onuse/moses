// Common utilities for filesystem formatters and readers

use moses_core::{Device, MosesError};
use std::fs::File;
use std::io::{Read, Write, Seek, SeekFrom};

/// Get the best path to access a device on the current platform.
/// On Windows, prefers drive letters (which don't require admin rights) over physical drive paths.
/// On other platforms, returns the appropriate device path.
pub fn get_device_path(device: &Device) -> String {
    #[cfg(target_os = "windows")]
    {
        // On Windows, prefer drive letter access (doesn't require admin rights)
        if !device.mount_points.is_empty() {
            let mount = &device.mount_points[0];
            let mount_str = mount.to_string_lossy();
            
            // Check if it's a drive letter like "E:" or "E:\"
            if mount_str.len() >= 2 && mount_str.chars().nth(1) == Some(':') {
                // Windows requires exactly "\\.\X:" format (no trailing backslash)
                let clean_mount = mount_str.trim_end_matches('\\').trim_end_matches('/');
                
                // Ensure it's just the drive letter and colon
                if clean_mount.len() == 2 {
                    log::info!("Using drive letter access: \\\\.\\{}", clean_mount);
                    return format!(r"\\.\{}", clean_mount);
                } else if clean_mount.len() == 3 && clean_mount.ends_with(':') {
                    // Handle "E:\\" -> "E:"
                    let drive_letter = &clean_mount[0..2];
                    log::info!("Using drive letter access: \\\\.\\{}", drive_letter);
                    return format!(r"\\.\{}", drive_letter);
                }
            }
        }
        
        // Fall back to physical drive path (requires admin rights)
        log::info!("Using physical drive access: {}", device.id);
        if device.id.starts_with(r"\\.\") {
            device.id.clone()
        } else {
            format!(r"\\.\{}", device.id)
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // On Unix-like systems, use /dev/ paths
        if device.id.starts_with('/') {
            device.id.clone()
        } else {
            format!("/dev/{}", device.id)
        }
    }
}

// Common filesystem constants
pub const SECTOR_SIZE: usize = 512;
pub const DEFAULT_CLUSTER_SIZE: usize = 4096;

/// Open a device for reading
pub fn open_device_read(device: &Device) -> Result<File, MosesError> {
    let path = get_device_path(device);
    log::info!("Attempting to open device for reading: {}", path);
    log::info!("Device ID: {}", device.id);
    log::info!("Device mount points: {:?}", device.mount_points);
    
    File::open(&path)
        .map_err(|e| {
            log::error!("Failed to open device {}: {} (OS error code: {:?})", path, e, e.raw_os_error());
            MosesError::Other(format!("Failed to open device {}: {}", path, e))
        })
}

/// Open a device for writing (formatting)
pub fn open_device_write(device: &Device) -> Result<File, MosesError> {
    let path = get_device_path(device);
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|e| MosesError::Other(format!("Failed to open device {} for writing: {}", path, e)))
}

/// Read a sector (512 bytes) from a specific offset
pub fn read_sector(file: &mut File, sector_number: u64) -> Result<Vec<u8>, MosesError> {
    let offset = sector_number * SECTOR_SIZE as u64;
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| MosesError::Other(format!("Failed to seek to sector {}: {}", sector_number, e)))?;
    
    let mut buffer = vec![0u8; SECTOR_SIZE];
    file.read_exact(&mut buffer)
        .map_err(|e| MosesError::Other(format!("Failed to read sector {}: {}", sector_number, e)))?;
    
    Ok(buffer)
}

/// Write a sector (512 bytes) to a specific offset
pub fn write_sector(file: &mut File, sector_number: u64, data: &[u8]) -> Result<(), MosesError> {
    if data.len() != SECTOR_SIZE {
        return Err(MosesError::Other(format!(
            "Invalid sector size: expected {}, got {}", 
            SECTOR_SIZE, 
            data.len()
        )));
    }
    
    let offset = sector_number * SECTOR_SIZE as u64;
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| MosesError::Other(format!("Failed to seek to sector {}: {}", sector_number, e)))?;
    
    file.write_all(data)
        .map_err(|e| MosesError::Other(format!("Failed to write sector {}: {}", sector_number, e)))?;
    
    Ok(())
}

/// Read a block of arbitrary size from a specific offset
pub fn read_block(file: &mut File, offset: u64, size: usize) -> Result<Vec<u8>, MosesError> {
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| MosesError::Other(format!("Failed to seek to offset {}: {}", offset, e)))?;
    
    let mut buffer = vec![0u8; size];
    file.read_exact(&mut buffer)
        .map_err(|e| MosesError::Other(format!("Failed to read {} bytes at offset {}: {}", size, offset, e)))?;
    
    Ok(buffer)
}

/// Calculate CRC32 checksum (commonly used in filesystems)
pub fn crc32(data: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

/// Convert a UTF-16LE string (common in Windows filesystems) to Rust String
pub fn utf16le_to_string(data: &[u8]) -> String {
    let u16_vec: Vec<u16> = data
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    
    String::from_utf16_lossy(&u16_vec)
        .trim_end_matches('\0')
        .to_string()
}

/// Convert a Rust String to UTF-16LE bytes
pub fn string_to_utf16le(s: &str, max_bytes: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(max_bytes);
    
    for ch in s.encode_utf16() {
        let bytes = ch.to_le_bytes();
        if result.len() + 2 <= max_bytes {
            result.extend_from_slice(&bytes);
        } else {
            break;
        }
    }
    
    // Pad with zeros
    while result.len() < max_bytes {
        result.push(0);
    }
    
    result
}

/// Check if a device is likely to be a system drive
pub fn is_likely_system_drive(device: &Device) -> bool {
    // Check explicit system flag
    if device.is_system {
        return true;
    }
    
    // Check for system mount points
    for mount in &device.mount_points {
        let mount_str = mount.to_string_lossy().to_lowercase();
        if mount_str == "/" || 
           mount_str == "c:\\" || 
           mount_str == "c:" ||
           mount_str.starts_with("/boot") ||
           mount_str.starts_with("/system") {
            return true;
        }
    }
    
    false
}

/// Common error handler for filesystem operations
pub fn handle_fs_error<T>(result: Result<T, MosesError>, context: &str) -> Result<T, String> {
    result.map_err(|e| {
        let error_msg = match e {
            MosesError::Other(msg) => msg,
            _ => format!("{:?}", e),
        };
        
        // Check for permission errors
        if error_msg.contains("Access is denied") || 
           error_msg.contains("Permission denied") ||
           error_msg.contains("Operation not permitted") {
            format!("{}: Administrator privileges may be required", context)
        } else {
            format!("{}: {}", context, error_msg)
        }
    })
}

/// Try to open a device, falling back to mount points if direct access fails
pub fn open_device_with_fallback(device: &Device) -> Result<File, MosesError> {
    // First try the preferred path (drive letter on Windows)
    let primary_path = get_device_path(device);
    
    log::info!("Trying primary path: {}", primary_path);
    match File::open(&primary_path) {
        Ok(file) => {
            log::info!("Successfully opened device at: {}", primary_path);
            Ok(file)
        },
        Err(primary_err) => {
            log::warn!("Failed to open {}: {} (error code: {:?})", primary_path, primary_err, primary_err.raw_os_error());
            
            // On Windows, if we failed with a drive letter, try physical path
            #[cfg(target_os = "windows")]
            {
                // Try the physical drive path directly
                log::info!("Trying physical path: {}", device.id);
                if let Ok(file) = File::open(&device.id) {
                    log::info!("Successfully opened device at physical path: {}", device.id);
                    return Ok(file);
                }
                
                // Also try without the \\.\ prefix if it has one
                if device.id.starts_with(r"\\.\") {
                    let without_prefix = &device.id[4..];
                    log::info!("Trying without prefix: {}", without_prefix);
                    if let Ok(file) = File::open(without_prefix) {
                        log::info!("Successfully opened device at: {}", without_prefix);
                        return Ok(file);
                    }
                }
            }
            
            // Try any other mount points
            for mount in &device.mount_points {
                let mount_str = mount.to_string_lossy();
                log::info!("Trying mount point: {}", mount_str);
                
                // Try the mount point directly
                if let Ok(file) = File::open(mount.as_path()) {
                    log::info!("Successfully opened device at mount point: {}", mount_str);
                    return Ok(file);
                }
                
                // On Windows, also try with \\.\ prefix
                #[cfg(target_os = "windows")]
                {
                    let with_prefix = format!(r"\\.\{}", mount_str.trim_end_matches('\\'));
                    if with_prefix != primary_path {
                        log::info!("Trying mount with prefix: {}", with_prefix);
                        if let Ok(file) = File::open(&with_prefix) {
                            log::info!("Successfully opened device at: {}", with_prefix);
                            return Ok(file);
                        }
                    }
                }
            }
            
            Err(MosesError::Other(format!(
                "Failed to open device: {}. Tried {} and {} mount points", 
                primary_err, 
                primary_path,
                device.mount_points.len()
            )))
        }
    }
}