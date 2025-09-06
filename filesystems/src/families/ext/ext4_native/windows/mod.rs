// Windows-specific implementation
// Handles sector-aligned I/O and device access

pub mod device_io;
pub mod volume;
pub mod disk_cleanup;

pub use device_io::WindowsDeviceIO;
pub use volume::VolumeManager;
pub use disk_cleanup::{cleanup_disk_for_format, get_drive_number_from_path};