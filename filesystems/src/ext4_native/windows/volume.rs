// Windows volume management for safe device access

#[cfg(target_os = "windows")]
use winapi::{
    um::winnt::HANDLE,
    um::errhandlingapi::GetLastError,
    um::ioapiset::DeviceIoControl,
    um::winioctl::{
        FSCTL_LOCK_VOLUME,
        FSCTL_DISMOUNT_VOLUME,
        IOCTL_DISK_DELETE_DRIVE_LAYOUT,
    },
    shared::minwindef::{DWORD, FALSE},
};
use std::ptr::null_mut;

#[cfg(target_os = "windows")]
pub struct VolumeManager {
    handle: HANDLE,
    locked: bool,
    dismounted: bool,
}

#[cfg(target_os = "windows")]
impl VolumeManager {
    /// Prepare a physical drive for formatting
    pub fn prepare_drive(handle: HANDLE) -> Result<(), String> {
        let mut manager = VolumeManager {
            handle,
            locked: false,
            dismounted: false,
        };
        
        // Try to lock and dismount
        manager.lock_volume()?;
        manager.dismount_volume()?;
        
        // Delete existing partition table to ensure clean state
        manager.delete_drive_layout()?;
        
        // Don't consume the handle, just prepare it
        Ok(())
    }
    
    /// Lock the volume for exclusive access
    fn lock_volume(&mut self) -> Result<(), String> {
        unsafe {
            let mut bytes_returned: DWORD = 0;
            
            eprintln!("DEBUG: Attempting to lock volume...");
            
            // Try to lock the volume up to 3 times
            for attempt in 1..=3 {
                let result = DeviceIoControl(
                    self.handle,
                    FSCTL_LOCK_VOLUME,
                    null_mut(),
                    0,
                    null_mut(),
                    0,
                    &mut bytes_returned,
                    null_mut(),
                );
                
                if result != FALSE {
                    eprintln!("DEBUG: Volume locked successfully on attempt {}", attempt);
                    self.locked = true;
                    return Ok(());
                }
                
                let error = GetLastError();
                eprintln!("DEBUG: Lock attempt {} failed with error {} (0x{:X})", attempt, error, error);
                
                // Wait a bit before retrying
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            
            // If we can't lock, continue anyway - some devices don't need locking
            eprintln!("DEBUG: Could not lock volume, continuing anyway");
            Ok(())
        }
    }
    
    /// Dismount any filesystems on the volume
    fn dismount_volume(&mut self) -> Result<(), String> {
        unsafe {
            let mut bytes_returned: DWORD = 0;
            
            eprintln!("DEBUG: Attempting to dismount volume...");
            
            let result = DeviceIoControl(
                self.handle,
                FSCTL_DISMOUNT_VOLUME,
                null_mut(),
                0,
                null_mut(),
                0,
                &mut bytes_returned,
                null_mut(),
            );
            
            if result != FALSE {
                eprintln!("DEBUG: Volume dismounted successfully");
                self.dismounted = true;
                Ok(())
            } else {
                let error = GetLastError();
                eprintln!("DEBUG: Dismount failed with error {} (0x{:X}), continuing", error, error);
                // Don't fail - device might not have mounted volumes
                Ok(())
            }
        }
    }
    
    /// Delete the drive layout (partition table)
    fn delete_drive_layout(&mut self) -> Result<(), String> {
        unsafe {
            let mut bytes_returned: DWORD = 0;
            
            eprintln!("DEBUG: Attempting to delete drive layout...");
            
            let result = DeviceIoControl(
                self.handle,
                IOCTL_DISK_DELETE_DRIVE_LAYOUT,
                null_mut(),
                0,
                null_mut(),
                0,
                &mut bytes_returned,
                null_mut(),
            );
            
            if result != FALSE {
                eprintln!("DEBUG: Drive layout deleted successfully");
                Ok(())
            } else {
                let error = GetLastError();
                eprintln!("DEBUG: Delete drive layout failed with error {} (0x{:X})", error, error);
                
                // This might fail if there's no partition table, which is fine
                if error == 1 || error == 87 {  // ERROR_INVALID_FUNCTION or ERROR_INVALID_PARAMETER
                    eprintln!("DEBUG: Drive may not have a partition table, continuing");
                    Ok(())
                } else {
                    Err(format!("Failed to delete drive layout: error {}", error))
                }
            }
        }
    }
}

// Non-Windows stub
#[cfg(not(target_os = "windows"))]
pub struct VolumeManager;

#[cfg(not(target_os = "windows"))]
impl VolumeManager {
    pub fn prepare_drive(_handle: *mut std::ffi::c_void) -> Result<(), String> {
        Ok(())
    }
}