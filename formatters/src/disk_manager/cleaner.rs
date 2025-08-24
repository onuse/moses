// Disk Cleaner - Safely wipe partition structures and data
use std::fs::OpenOptions;
use std::io::{Write, Seek, SeekFrom};
use moses_core::{Device, MosesError};
use serde::{Serialize, Deserialize};

pub struct DiskCleaner;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanOptions {
    pub wipe_method: WipeMethod,
    pub zero_entire_disk: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum WipeMethod {
    /// Just zero critical sectors (MBR, GPT headers)
    Quick,
    /// Zero entire disk
    Zero,
    /// DoD 5220.22-M (3 passes)
    DoD5220,
    /// Random data (1 pass)
    Random,
}

impl DiskCleaner {
    /// Clean a disk according to the specified options
    pub fn clean(device: &Device, options: &CleanOptions) -> Result<(), MosesError> {
        log::info!("Cleaning disk: {} with method {:?}", device.name, options.wipe_method);
        
        // Safety check
        if device.is_system {
            return Err(MosesError::InvalidInput(
                "Cannot clean system disk - this would destroy your OS!".to_string()
            ));
        }
        
        #[cfg(target_os = "windows")]
        {
            Self::clean_windows(device, options)
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            Self::clean_unix(device, options)
        }
    }
    
    #[cfg(target_os = "windows")]
    fn clean_windows(device: &Device, options: &CleanOptions) -> Result<(), MosesError> {
        // First, try to dismount any volumes on this device
        // This is crucial for being able to write to the disk
        if !device.mount_points.is_empty() {
            log::info!("Attempting to dismount volumes before cleaning");
            for mount_point in &device.mount_points {
                if let Some(drive_letter) = mount_point.to_str() {
                    log::info!("Dismounting volume: {}", drive_letter);
                    // We'll try to open and lock the volume, but continue even if it fails
                    if let Ok(vol_handle) = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(format!(r"\\.\{}", drive_letter.trim_end_matches('\\')))
                    {
                        use std::os::windows::io::AsRawHandle;
                        use winapi::um::winioctl::{FSCTL_LOCK_VOLUME, FSCTL_DISMOUNT_VOLUME};
                        use winapi::um::ioapiset::DeviceIoControl;
                        
                        let handle = vol_handle.as_raw_handle();
                        let mut bytes_returned: u32 = 0;
                        
                        // Try to lock the volume
                        unsafe {
                            DeviceIoControl(
                                handle as *mut _,
                                FSCTL_LOCK_VOLUME,
                                std::ptr::null_mut(),
                                0,
                                std::ptr::null_mut(),
                                0,
                                &mut bytes_returned,
                                std::ptr::null_mut(),
                            );
                            
                            // Try to dismount
                            DeviceIoControl(
                                handle as *mut _,
                                FSCTL_DISMOUNT_VOLUME,
                                std::ptr::null_mut(),
                                0,
                                std::ptr::null_mut(),
                                0,
                                &mut bytes_returned,
                                std::ptr::null_mut(),
                            );
                        }
                    }
                }
            }
        }
        
        // Now open the physical device for cleaning
        // Use the same method as formatters
        use crate::utils::open_device_write;
        let mut file = open_device_write(device)?;
        
        // Clean based on options
        match options.wipe_method {
            WipeMethod::Quick => Self::quick_clean(&mut file, device.size)?,
            WipeMethod::Zero => Self::zero_wipe(&mut file, device.size)?,
            WipeMethod::DoD5220 => Self::dod_wipe(&mut file, device.size)?,
            WipeMethod::Random => Self::random_wipe(&mut file, device.size)?,
        }
        
        file.sync_all()
            .map_err(|e| MosesError::Other(format!("Failed to sync after clean: {}", e)))?;
        
        Ok(())
    }
    
    #[cfg(not(target_os = "windows"))]
    fn clean_unix(device: &Device, options: &CleanOptions) -> Result<(), MosesError> {
        let mut file = OpenOptions::new()
            .write(true)
            .open(&device.id)
            .map_err(|e| MosesError::IoError(e))?;
        
        match options.wipe_method {
            WipeMethod::Quick => Self::quick_clean(&mut file, device.size)?,
            WipeMethod::Zero => Self::zero_wipe(&mut file, device.size)?,
            WipeMethod::DoD5220 => Self::dod_wipe(&mut file, device.size)?,
            WipeMethod::Random => Self::random_wipe(&mut file, device.size)?,
        }
        
        file.sync_all()
            .map_err(|e| MosesError::Other(format!("Failed to sync after clean: {}", e)))?;
        
        Ok(())
    }
    
    /// Quick clean - just wipe critical sectors
    fn quick_clean<W: Write + Seek>(writer: &mut W, disk_size: u64) -> Result<(), MosesError> {
        let zero_buffer = vec![0u8; 512];
        
        // 1. Wipe MBR (sector 0)
        writer.seek(SeekFrom::Start(0))
            .map_err(|e| MosesError::Other(format!("Failed to seek to MBR: {}", e)))?;
        writer.write_all(&zero_buffer)
            .map_err(|e| MosesError::Other(format!("Failed to wipe MBR: {}", e)))?;
        
        // 2. Wipe primary GPT header (sector 1)
        writer.seek(SeekFrom::Start(512))
            .map_err(|e| MosesError::Other(format!("Failed to seek to GPT header: {}", e)))?;
        writer.write_all(&zero_buffer)
            .map_err(|e| MosesError::Other(format!("Failed to wipe GPT header: {}", e)))?;
        
        // 3. Wipe GPT partition entries (sectors 2-33)
        let gpt_entries_buffer = vec![0u8; 32 * 512]; // 32 sectors
        writer.seek(SeekFrom::Start(1024))
            .map_err(|e| MosesError::Other(format!("Failed to seek to GPT entries: {}", e)))?;
        writer.write_all(&gpt_entries_buffer)
            .map_err(|e| MosesError::Other(format!("Failed to wipe GPT entries: {}", e)))?;
        
        // 4. Wipe backup GPT (last 33 sectors)
        if disk_size > 33 * 512 {
            let backup_gpt_start = disk_size - (33 * 512);
            writer.seek(SeekFrom::Start(backup_gpt_start))
                .map_err(|e| MosesError::Other(format!("Failed to seek to backup GPT: {}", e)))?;
            
            let backup_buffer = vec![0u8; 33 * 512];
            writer.write_all(&backup_buffer)
                .map_err(|e| MosesError::Other(format!("Failed to wipe backup GPT: {}", e)))?;
        }
        
        // 5. Wipe first MB (for good measure - catches various boot loaders)
        writer.seek(SeekFrom::Start(0))
            .map_err(|e| MosesError::Other(format!("Failed to seek to start: {}", e)))?;
        let mb_buffer = vec![0u8; 1024 * 1024];
        writer.write_all(&mb_buffer)
            .map_err(|e| MosesError::Other(format!("Failed to wipe first MB: {}", e)))?;
        
        // 6. Wipe common partition start offset (1MB - sector 2048)
        // This is where Windows typically starts the first partition
        writer.seek(SeekFrom::Start(1024 * 1024))
            .map_err(|e| MosesError::Other(format!("Failed to seek to 1MB offset: {}", e)))?;
        let partition_wipe = vec![0u8; 64 * 1024]; // Wipe 64KB at partition start
        writer.write_all(&partition_wipe)
            .map_err(|e| MosesError::Other(format!("Failed to wipe partition offset: {}", e)))?;
        
        log::info!("Quick clean completed - wiped critical sectors including partition offset");
        Ok(())
    }
    
    /// Zero entire disk
    fn zero_wipe<W: Write + Seek>(writer: &mut W, disk_size: u64) -> Result<(), MosesError> {
        const CHUNK_SIZE: usize = 1024 * 1024; // 1MB chunks
        let zero_buffer = vec![0u8; CHUNK_SIZE];
        
        writer.seek(SeekFrom::Start(0))
            .map_err(|e| MosesError::Other(format!("Failed to seek to start: {}", e)))?;
        
        let mut written = 0u64;
        while written < disk_size {
            let to_write = std::cmp::min(CHUNK_SIZE as u64, disk_size - written);
            writer.write_all(&zero_buffer[..to_write as usize])
                .map_err(|e| MosesError::Other(format!("Failed to write zeros at {}: {}", written, e)))?;
            written += to_write;
            
            // Progress callback would go here
            if written % (100 * 1024 * 1024) == 0 {
                log::info!("Zero wipe progress: {}MB / {}MB", 
                    written / (1024 * 1024), 
                    disk_size / (1024 * 1024));
            }
        }
        
        log::info!("Zero wipe completed - entire disk zeroed");
        Ok(())
    }
    
    /// DoD 5220.22-M standard - 3 passes
    fn dod_wipe<W: Write + Seek>(writer: &mut W, disk_size: u64) -> Result<(), MosesError> {
        // Pass 1: Write zeros
        log::info!("DoD wipe pass 1/3: Writing zeros");
        Self::zero_wipe(writer, disk_size)?;
        
        // Pass 2: Write ones (0xFF)
        log::info!("DoD wipe pass 2/3: Writing ones");
        Self::pattern_wipe(writer, disk_size, 0xFF)?;
        
        // Pass 3: Write random data
        log::info!("DoD wipe pass 3/3: Writing random data");
        Self::random_wipe(writer, disk_size)?;
        
        log::info!("DoD 5220.22-M wipe completed");
        Ok(())
    }
    
    /// Write random data
    fn random_wipe<W: Write + Seek>(writer: &mut W, disk_size: u64) -> Result<(), MosesError> {
        use rand::Rng;
        const CHUNK_SIZE: usize = 1024 * 1024; // 1MB chunks
        
        writer.seek(SeekFrom::Start(0))
            .map_err(|e| MosesError::Other(format!("Failed to seek to start: {}", e)))?;
        
        let mut rng = rand::thread_rng();
        let mut buffer = vec![0u8; CHUNK_SIZE];
        
        let mut written = 0u64;
        while written < disk_size {
            rng.fill(&mut buffer[..]);
            let to_write = std::cmp::min(CHUNK_SIZE as u64, disk_size - written);
            writer.write_all(&buffer[..to_write as usize])
                .map_err(|e| MosesError::Other(format!("Failed to write random at {}: {}", written, e)))?;
            written += to_write;
            
            if written % (100 * 1024 * 1024) == 0 {
                log::info!("Random wipe progress: {}MB / {}MB", 
                    written / (1024 * 1024), 
                    disk_size / (1024 * 1024));
            }
        }
        
        log::info!("Random wipe completed");
        Ok(())
    }
    
    /// Write a repeating pattern
    fn pattern_wipe<W: Write + Seek>(writer: &mut W, disk_size: u64, pattern: u8) -> Result<(), MosesError> {
        const CHUNK_SIZE: usize = 1024 * 1024; // 1MB chunks
        let buffer = vec![pattern; CHUNK_SIZE];
        
        writer.seek(SeekFrom::Start(0))
            .map_err(|e| MosesError::Other(format!("Failed to seek to start: {}", e)))?;
        
        let mut written = 0u64;
        while written < disk_size {
            let to_write = std::cmp::min(CHUNK_SIZE as u64, disk_size - written);
            writer.write_all(&buffer[..to_write as usize])
                .map_err(|e| MosesError::Other(format!("Failed to write pattern at {}: {}", written, e)))?;
            written += to_write;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    
    #[test]
    fn test_quick_clean() {
        let mut buffer = vec![0xFF; 2 * 1024 * 1024]; // 2MB buffer filled with 0xFF
        let buffer_len = buffer.len() as u64;
        let mut cursor = Cursor::new(&mut buffer);
        
        // Simulate quick clean
        DiskCleaner::quick_clean(&mut cursor, buffer_len).unwrap();
        
        // Check that MBR is zeroed
        assert_eq!(buffer[0], 0);
        assert_eq!(buffer[511], 0);
        
        // Check that GPT header is zeroed
        assert_eq!(buffer[512], 0);
        assert_eq!(buffer[1023], 0);
        
        // Check that first MB is zeroed
        assert!(buffer[..1024*1024].iter().all(|&b| b == 0));
    }
}