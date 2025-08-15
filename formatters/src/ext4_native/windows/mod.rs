// Windows-specific implementation
// Handles sector-aligned I/O and device access

pub mod device_io;

pub use device_io::WindowsDeviceIO;