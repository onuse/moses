// Test helpers for filesystem testing

use moses_core::{Device, DeviceType};
use std::path::PathBuf;

/// Create a test device for an image file
pub fn create_test_device(file_path: &str, size: u64) -> Device {
    Device {
        id: file_path.to_string(),
        name: format!("Test Device at {}", file_path),
        size,
        device_type: DeviceType::Virtual,
        mount_points: vec![],
        is_removable: false,
        is_system: false,
        filesystem: None,
    }
}

/// Create a test device with default size
pub fn create_default_test_device(file_path: &str) -> Device {
    create_test_device(file_path, 100 * 1024 * 1024) // 100MB default
}