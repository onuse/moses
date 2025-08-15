// Alignment utilities for ext4 structures and Windows I/O
// CRITICAL: Get this wrong and nothing works!

use std::ops::{Deref, DerefMut};

/// A buffer with guaranteed alignment for Windows sector I/O
/// Windows requires 512-byte alignment for FILE_FLAG_NO_BUFFERING
#[repr(C)]
#[repr(align(512))]
#[derive(Clone)]
pub struct AlignedBuffer<const N: usize> {
    data: [u8; N],
}

impl<const N: usize> AlignedBuffer<N> {
    /// Create a new zeroed aligned buffer
    pub fn new() -> Self {
        Self { data: [0u8; N] }
    }

    /// Create from existing data (copies)
    pub fn from_slice(data: &[u8]) -> Self {
        let mut buffer = Self::new();
        let len = data.len().min(N);
        buffer.data[..len].copy_from_slice(&data[..len]);
        buffer
    }

    /// Get the alignment of this buffer
    pub fn alignment(&self) -> usize {
        self.data.as_ptr() as usize % 512
    }

    /// Verify the buffer is properly aligned
    pub fn is_aligned(&self) -> bool {
        self.alignment() == 0
    }

    /// Get as byte slice
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Get as mutable byte slice
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

impl<const N: usize> Default for AlignedBuffer<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Deref for AlignedBuffer<N> {
    type Target = [u8; N];

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<const N: usize> DerefMut for AlignedBuffer<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

/// Round up to next multiple of alignment
pub fn align_up(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) / alignment * alignment
}

/// Round down to previous multiple of alignment
pub fn align_down(value: usize, alignment: usize) -> usize {
    value / alignment * alignment
}

/// Check if a value is aligned
pub fn is_aligned(value: usize, alignment: usize) -> bool {
    value % alignment == 0
}

/// Calculate padding needed for alignment
pub fn padding_for_alignment(current_size: usize, alignment: usize) -> usize {
    let aligned = align_up(current_size, alignment);
    aligned - current_size
}

/// Verify structure alignment at compile time
#[macro_export]
macro_rules! assert_aligned {
    ($type:ty, $align:expr) => {
        const _: () = {
            assert!(std::mem::align_of::<$type>() >= $align);
        };
    };
}

/// Verify structure size at compile time
#[macro_export]
macro_rules! assert_size {
    ($type:ty, $size:expr) => {
        const _: () = {
            assert!(std::mem::size_of::<$type>() == $size);
        };
    };
}

/// Windows sector size detection
#[cfg(target_os = "windows")]
pub fn get_sector_size(device_path: &str) -> Result<u32, String> {
    use std::mem;
    use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
    use winapi::um::winioctl::IOCTL_DISK_GET_DRIVE_GEOMETRY;
    use winapi::um::ioapiset::DeviceIoControl;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::winnt::{GENERIC_READ, FILE_SHARE_READ, FILE_SHARE_WRITE};
    use std::ptr::null_mut;
    use std::os::windows::ffi::OsStrExt;
    use std::ffi::OsStr;

    #[repr(C)]
    struct DiskGeometry {
        cylinders: i64,
        media_type: u32,
        tracks_per_cylinder: u32,
        sectors_per_track: u32,
        bytes_per_sector: u32,
    }

    unsafe {
        let wide_path: Vec<u16> = OsStr::new(device_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let handle = CreateFileW(
            wide_path.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null_mut(),
            OPEN_EXISTING,
            0,
            null_mut(),
        );

        if handle.is_null() {
            return Err("Failed to open device".to_string());
        }

        let mut geometry = mem::zeroed::<DiskGeometry>();
        let mut bytes_returned = 0u32;

        let success = DeviceIoControl(
            handle,
            IOCTL_DISK_GET_DRIVE_GEOMETRY,
            null_mut(),
            0,
            &mut geometry as *mut _ as *mut _,
            mem::size_of::<DiskGeometry>() as u32,
            &mut bytes_returned,
            null_mut(),
        );

        CloseHandle(handle);

        if success == 0 {
            return Err("Failed to get disk geometry".to_string());
        }

        Ok(geometry.bytes_per_sector)
    }
}

#[cfg(not(target_os = "windows"))]
pub fn get_sector_size(_device_path: &str) -> Result<u32, String> {
    // Default to 512 for non-Windows platforms
    Ok(512)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aligned_buffer() {
        let buffer = AlignedBuffer::<4096>::new();
        assert!(buffer.is_aligned());
        assert_eq!(buffer.alignment(), 0);
    }

    #[test]
    fn test_alignment_functions() {
        assert_eq!(align_up(1000, 512), 1024);
        assert_eq!(align_up(512, 512), 512);
        assert_eq!(align_down(1000, 512), 512);
        assert_eq!(align_down(512, 512), 512);
        
        assert!(is_aligned(512, 512));
        assert!(!is_aligned(513, 512));
        
        assert_eq!(padding_for_alignment(1000, 512), 24);
        assert_eq!(padding_for_alignment(512, 512), 0);
    }
}