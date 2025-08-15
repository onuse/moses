// Windows device I/O implementation
// CRITICAL: Handles sector alignment requirements

#[cfg(target_os = "windows")]
use winapi::{
    um::fileapi::{CreateFileW, OPEN_EXISTING, SetFilePointerEx, WriteFile},
    um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE, HANDLE},
    um::handleapi::CloseHandle,
    um::errhandlingapi::GetLastError,
    shared::minwindef::DWORD,
    um::winbase::{FILE_FLAG_NO_BUFFERING, FILE_FLAG_WRITE_THROUGH, FILE_BEGIN},
};

use crate::ext4_native::core::{alignment::AlignedBuffer, types::*};
use std::ptr::null_mut;

#[cfg(target_os = "windows")]
pub struct WindowsDeviceIO {
    handle: HANDLE,
    sector_size: u32,
    device_size: u64,
}

#[cfg(target_os = "windows")]
impl WindowsDeviceIO {
    /// Open a device for writing
    pub fn open(device_path: &str) -> Ext4Result<Self> {
        use std::os::windows::ffi::OsStrExt;
        use std::ffi::OsStr;
        
        let wide_path: Vec<u16> = OsStr::new(device_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        
        unsafe {
            // Open with required flags for direct I/O
            let handle = CreateFileW(
                wide_path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null_mut(),
                OPEN_EXISTING,
                FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH,
                null_mut(),
            );
            
            if handle.is_null() || handle as isize == -1 {
                let error = GetLastError();
                return Err(Ext4Error::WindowsError(
                    format!("Failed to open device: error code {}", error)
                ));
            }
            
            // Get sector size
            let sector_size = crate::ext4_native::core::alignment::get_sector_size(device_path)?;
            
            // TODO: Get device size
            let device_size = 0; // Will implement proper size detection
            
            Ok(Self {
                handle,
                sector_size,
                device_size,
            })
        }
    }
    
    /// Write aligned data to device
    pub fn write_aligned(&mut self, offset: u64, data: &[u8]) -> Ext4Result<()> {
        // Verify alignment
        if offset % self.sector_size as u64 != 0 {
            return Err(Ext4Error::AlignmentError(
                format!("Offset {} not aligned to sector size {}", offset, self.sector_size)
            ));
        }
        
        // Calculate aligned size
        let aligned_size = crate::ext4_native::core::alignment::align_up(
            data.len(), 
            self.sector_size as usize
        );
        
        // Create aligned buffer
        const BUFFER_SIZE: usize = 1024 * 1024; // 1MB buffer
        let mut aligned_buffer = AlignedBuffer::<BUFFER_SIZE>::new();
        
        // Process in chunks
        let mut written = 0;
        while written < data.len() {
            let chunk_size = (data.len() - written).min(BUFFER_SIZE);
            let aligned_chunk_size = crate::ext4_native::core::alignment::align_up(
                chunk_size,
                self.sector_size as usize
            );
            
            // Copy data to aligned buffer
            aligned_buffer[..chunk_size].copy_from_slice(&data[written..written + chunk_size]);
            
            // Zero padding
            if aligned_chunk_size > chunk_size {
                aligned_buffer[chunk_size..aligned_chunk_size].fill(0);
            }
            
            // Seek to position
            unsafe {
                let mut new_pos = 0i64;
                let success = SetFilePointerEx(
                    self.handle,
                    (offset + written as u64) as i64,
                    &mut new_pos,
                    FILE_BEGIN,
                );
                
                if success == 0 {
                    let error = GetLastError();
                    return Err(Ext4Error::WindowsError(
                        format!("Failed to seek: error code {}", error)
                    ));
                }
                
                // Write data
                let mut bytes_written = 0u32;
                let success = WriteFile(
                    self.handle,
                    aligned_buffer.as_ptr() as *const _,
                    aligned_chunk_size as DWORD,
                    &mut bytes_written,
                    null_mut(),
                );
                
                if success == 0 {
                    let error = GetLastError();
                    return Err(Ext4Error::WindowsError(
                        format!("Failed to write: error code {}", error)
                    ));
                }
                
                if bytes_written != aligned_chunk_size as u32 {
                    return Err(Ext4Error::Io(
                        format!("Incomplete write: {} of {} bytes", 
                                bytes_written, aligned_chunk_size)
                    ));
                }
            }
            
            written += chunk_size;
        }
        
        Ok(())
    }
    
    /// Flush all pending writes
    pub fn flush(&self) -> Ext4Result<()> {
        // Windows FILE_FLAG_WRITE_THROUGH should handle this
        Ok(())
    }
}

#[cfg(target_os = "windows")]
impl Drop for WindowsDeviceIO {
    fn drop(&mut self) {
        unsafe {
            if !self.handle.is_null() && self.handle as isize != -1 {
                CloseHandle(self.handle);
            }
        }
    }
}

// Non-Windows stub implementation
#[cfg(not(target_os = "windows"))]
pub struct WindowsDeviceIO;

#[cfg(not(target_os = "windows"))]
impl WindowsDeviceIO {
    pub fn open(_device_path: &str) -> Ext4Result<Self> {
        Err(Ext4Error::Io("Windows device I/O only available on Windows".to_string()))
    }
    
    pub fn write_aligned(&mut self, _offset: u64, _data: &[u8]) -> Ext4Result<()> {
        Err(Ext4Error::Io("Windows device I/O only available on Windows".to_string()))
    }
    
    pub fn flush(&self) -> Ext4Result<()> {
        Err(Ext4Error::Io("Windows device I/O only available on Windows".to_string()))
    }
}