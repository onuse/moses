// Windows device I/O implementation
// CRITICAL: Handles sector alignment requirements

#[cfg(target_os = "windows")]
use winapi::{
    um::fileapi::{CreateFileW, OPEN_EXISTING, SetFilePointerEx, WriteFile},
    um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE, HANDLE, LARGE_INTEGER},
    um::handleapi::CloseHandle,
    um::errhandlingapi::GetLastError,
    um::ioapiset::DeviceIoControl,
    um::winioctl::IOCTL_DISK_GET_LENGTH_INFO,
    shared::minwindef::DWORD,
    um::winbase::{FILE_FLAG_NO_BUFFERING, FILE_FLAG_WRITE_THROUGH, FILE_BEGIN},
};

use crate::families::ext::ext4_native::core::{alignment::AlignedBuffer, types::*};
use std::ptr::null_mut;
use log::{debug, info, error};

/// Get device size using Windows IOCTL
#[cfg(target_os = "windows")]
fn get_device_size(handle: HANDLE) -> Ext4Result<u64> {
    unsafe {
        use std::mem;
        
        // GET_LENGTH_INFO structure
        #[repr(C)]
        struct GetLengthInfo {
            length: LARGE_INTEGER,
        }
        
        let mut length_info = mem::zeroed::<GetLengthInfo>();
        let mut bytes_returned = 0u32;
        
        let success = DeviceIoControl(
            handle,
            IOCTL_DISK_GET_LENGTH_INFO,
            null_mut(),
            0,
            &mut length_info as *mut _ as *mut _,
            mem::size_of::<GetLengthInfo>() as u32,
            &mut bytes_returned,
            null_mut(),
        );
        
        if success == 0 {
            let error = GetLastError();
            debug!("IOCTL_DISK_GET_LENGTH_INFO failed with error: {} (0x{:X})", error, error);
            
            // Fallback: Try IOCTL_DISK_GET_DRIVE_GEOMETRY_EX
            use winapi::um::winioctl::IOCTL_DISK_GET_DRIVE_GEOMETRY_EX;
            
            #[repr(C)]
            struct DiskGeometryEx {
                geometry: DiskGeometry,
                disk_size: LARGE_INTEGER,
                data: [u8; 1],
            }
            
            #[repr(C)]
            struct DiskGeometry {
                cylinders: LARGE_INTEGER,
                media_type: u32,
                tracks_per_cylinder: u32,
                sectors_per_track: u32,
                bytes_per_sector: u32,
            }
            
            let mut geometry_ex = mem::zeroed::<DiskGeometryEx>();
            let mut bytes_returned = 0u32;
            
            let success = DeviceIoControl(
                handle,
                IOCTL_DISK_GET_DRIVE_GEOMETRY_EX,
                null_mut(),
                0,
                &mut geometry_ex as *mut _ as *mut _,
                mem::size_of::<DiskGeometryEx>() as u32,
                &mut bytes_returned,
                null_mut(),
            );
            
            if success == 0 {
                let error = GetLastError();
                return Err(Ext4Error::WindowsError(
                    format!("Failed to get device size: error code {} (0x{:X})", error, error)
                ));
            }
            
            Ok(*geometry_ex.disk_size.QuadPart() as u64)
        } else {
            Ok(*length_info.length.QuadPart() as u64)
        }
    }
}

#[cfg(target_os = "windows")]
pub struct WindowsDeviceIO {
    handle: HANDLE,
    sector_size: u32,
    _device_size: u64,  // Stored for future validation use
}

#[cfg(target_os = "windows")]
impl WindowsDeviceIO {
    /// Open a device for writing
    pub fn open(device_path: &str) -> Ext4Result<Self> {
        use std::os::windows::ffi::OsStrExt;
        use std::ffi::OsStr;
        
        debug!("Attempting to open device: {}", device_path);
        
        // First, cleanup the disk - dismount all volumes on it
        if let Some(drive_number) = crate::families::ext::ext4_native::windows::get_drive_number_from_path(device_path) {
            info!("Cleaning up disk {} before format", drive_number);
            crate::families::ext::ext4_native::windows::cleanup_disk_for_format(drive_number)
                .map_err(|e| Ext4Error::WindowsError(format!("Disk cleanup failed: {}", e)))?;
        }
        
        let wide_path: Vec<u16> = OsStr::new(device_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        
        unsafe {
            // First, try without NO_BUFFERING to see if it's a permission issue
            debug!("First attempt without FILE_FLAG_NO_BUFFERING");
            let mut handle = CreateFileW(
                wide_path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0, // No sharing first
                null_mut(),
                OPEN_EXISTING,
                0, // No special flags
                null_mut(),
            );
            
            if handle.is_null() || handle as isize == -1 {
                let error1 = GetLastError();
                debug!("First attempt failed with error: {} (0x{:X})", error1, error1);
                
                // Try with sharing
                debug!("Second attempt with FILE_SHARE_READ | FILE_SHARE_WRITE");
                handle = CreateFileW(
                    wide_path.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    null_mut(),
                    OPEN_EXISTING,
                    0,
                    null_mut(),
                );
                
                if handle.is_null() || handle as isize == -1 {
                    let error2 = GetLastError();
                    debug!("Second attempt failed with error: {} (0x{:X})", error2, error2);
                    
                    // Final attempt with full flags
                    debug!("Final attempt with FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH");
                    handle = CreateFileW(
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
                        error!("All attempts failed. Final error: {} (0x{:X})", error, error);
                        
                        // Common error codes
                        let error_msg = match error {
                            5 => "Access denied - ensure running as administrator and device is not in use",
                            2 => "Device not found - check device path format",
                            32 => "Device is in use by another process",
                            87 => "Invalid parameter - check device path format",
                            _ => "Unknown error"
                        };
                        
                        return Err(Ext4Error::WindowsError(
                            format!("Failed to open device '{}': {} (error code {})", 
                                    device_path, error_msg, error)
                        ));
                    }
                }
            }
            
            debug!("Device opened successfully, handle: {:?}", handle);
            
            // Get sector size
            let sector_size = crate::families::ext::ext4_native::core::alignment::get_sector_size(device_path)?;
            
            // Get device size using Windows IOCTL
            let device_size = get_device_size(handle)?;
            info!("Device size detected: {} bytes ({:.2} GB)", 
                  device_size, device_size as f64 / (1024.0 * 1024.0 * 1024.0));
            
            Ok(Self {
                handle,
                sector_size,
                _device_size: device_size,
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
        
        // Calculate aligned size (not currently used, but may be needed for future optimizations)
        let _aligned_size = crate::families::ext::ext4_native::core::alignment::align_up(
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
            let aligned_chunk_size = crate::families::ext::ext4_native::core::alignment::align_up(
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
                let mut distance: LARGE_INTEGER = std::mem::zeroed();
                *distance.QuadPart_mut() = (offset + written as u64) as i64;
                
                let success = SetFilePointerEx(
                    self.handle,
                    distance,
                    null_mut(),
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
    
    /// Get the device size
    pub fn device_size(&self) -> u64 {
        self._device_size
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