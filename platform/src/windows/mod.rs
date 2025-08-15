pub mod device;
pub mod elevation;

pub use device::WindowsDeviceManager;
pub use elevation::{is_elevated, request_elevation_for_operation, show_elevation_prompt};