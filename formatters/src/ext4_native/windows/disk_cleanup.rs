// Windows disk cleanup - dismount all volumes before formatting

#[cfg(target_os = "windows")]
use winapi::{
    um::fileapi::{CreateFileW, OPEN_EXISTING, FindFirstVolumeW, FindNextVolumeW, FindVolumeClose},
    um::winnt::{GENERIC_READ, GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE},
    um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE},
    um::errhandlingapi::GetLastError,
    um::ioapiset::DeviceIoControl,
    um::winioctl::{
        FSCTL_LOCK_VOLUME,
        FSCTL_DISMOUNT_VOLUME,
        IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
    },
    shared::minwindef::{DWORD, FALSE, MAX_PATH},
};
use std::ptr::null_mut;
use std::os::windows::ffi::OsStrExt;
use std::ffi::OsStr;

#[repr(C)]
#[cfg(target_os = "windows")]
struct DiskExtent {
    disk_number: DWORD,
    starting_offset: i64,
    extent_length: i64,
}

#[repr(C)]
#[cfg(target_os = "windows")]
struct VolumeDiskExtents {
    number_of_disk_extents: DWORD,
    extents: [DiskExtent; 1],
}

#[cfg(target_os = "windows")]
pub fn cleanup_disk_for_format(physical_drive_number: u32) -> Result<(), String> {
    eprintln!("DEBUG: Starting disk cleanup for PhysicalDrive{}", physical_drive_number);
    
    unsafe {
        // Find all volumes and check if they're on our target disk
        let mut volume_name = vec![0u16; MAX_PATH];
        let find_handle = FindFirstVolumeW(volume_name.as_mut_ptr(), MAX_PATH as DWORD);
        
        if find_handle == INVALID_HANDLE_VALUE {
            eprintln!("DEBUG: No volumes found to dismount");
            return Ok(());
        }
        
        loop {
            // Convert volume name to string and process it
            let volume_str = String::from_utf16_lossy(&volume_name)
                .trim_end_matches('\0')
                .to_string();
            
            eprintln!("DEBUG: Checking volume: {}", volume_str);
            
            // Open the volume
            let volume_path = volume_str.trim_end_matches('\\');
            let wide_path: Vec<u16> = OsStr::new(volume_path)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            
            let volume_handle = CreateFileW(
                wide_path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null_mut(),
                OPEN_EXISTING,
                0,
                null_mut(),
            );
            
            if volume_handle != INVALID_HANDLE_VALUE {
                // Check if this volume is on our target disk
                let mut extents: VolumeDiskExtents = std::mem::zeroed();
                let mut bytes_returned: DWORD = 0;
                
                let result = DeviceIoControl(
                    volume_handle,
                    IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
                    null_mut(),
                    0,
                    &mut extents as *mut _ as *mut _,
                    std::mem::size_of::<VolumeDiskExtents>() as DWORD,
                    &mut bytes_returned,
                    null_mut(),
                );
                
                if result != FALSE && extents.number_of_disk_extents > 0 {
                    if extents.extents[0].disk_number == physical_drive_number {
                        eprintln!("DEBUG: Found volume on target disk, dismounting: {}", volume_str);
                        
                        // Lock the volume
                        let mut lock_bytes: DWORD = 0;
                        DeviceIoControl(
                            volume_handle,
                            FSCTL_LOCK_VOLUME,
                            null_mut(),
                            0,
                            null_mut(),
                            0,
                            &mut lock_bytes,
                            null_mut(),
                        );
                        
                        // Dismount the volume
                        let mut dismount_bytes: DWORD = 0;
                        let dismount_result = DeviceIoControl(
                            volume_handle,
                            FSCTL_DISMOUNT_VOLUME,
                            null_mut(),
                            0,
                            null_mut(),
                            0,
                            &mut dismount_bytes,
                            null_mut(),
                        );
                        
                        if dismount_result != FALSE {
                            eprintln!("DEBUG: Successfully dismounted volume");
                        } else {
                            let error = GetLastError();
                            eprintln!("DEBUG: Failed to dismount volume, error: {} (0x{:X})", error, error);
                        }
                    }
                }
                
                CloseHandle(volume_handle);
            }
            
            // Find next volume
            if FindNextVolumeW(find_handle, volume_name.as_mut_ptr(), MAX_PATH as DWORD) == FALSE {
                break;
            }
        }
        
        FindVolumeClose(find_handle);
    }
    
    eprintln!("DEBUG: Disk cleanup completed");
    Ok(())
}

// Extract drive number from device path (e.g., \\.\PHYSICALDRIVE2 -> 2)
#[cfg(target_os = "windows")]
pub fn get_drive_number_from_path(device_path: &str) -> Option<u32> {
    if let Some(pos) = device_path.to_uppercase().find("PHYSICALDRIVE") {
        let number_str = &device_path[pos + 13..]; // Skip "PHYSICALDRIVE"
        number_str.parse().ok()
    } else {
        None
    }
}

#[cfg(not(target_os = "windows"))]
pub fn cleanup_disk_for_format(_physical_drive_number: u32) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn get_drive_number_from_path(_device_path: &str) -> Option<u32> {
    None
}