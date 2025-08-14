/// Test utilities and mock implementations for safe testing
use crate::{Device, DeviceInfo, DeviceManager, DeviceType, MosesError, PermissionLevel};
use crate::{FilesystemFormatter, FormatOptions, SimulationReport};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Mock device for testing - NEVER touches real hardware
#[derive(Clone, Debug)]
pub struct MockDevice {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub device_type: DeviceType,
    pub mount_points: Vec<PathBuf>,
    pub is_removable: bool,
    pub is_system: bool,
    pub format_history: Arc<Mutex<Vec<String>>>,
}

impl MockDevice {
    pub fn new_usb(name: &str, size_gb: u64) -> Self {
        Self {
            id: format!("mock://usb/{}", name.to_lowercase().replace(" ", "-")),
            name: name.to_string(),
            size: size_gb * 1_073_741_824,
            device_type: DeviceType::USB,
            mount_points: vec![],
            is_removable: true,
            is_system: false,
            format_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn new_system_drive() -> Self {
        Self {
            id: "mock://system/drive0".to_string(),
            name: "System Drive".to_string(),
            size: 500 * 1_073_741_824,
            device_type: DeviceType::SSD,
            mount_points: vec![PathBuf::from("C:\\"), PathBuf::from("/")],
            is_removable: false,
            is_system: true,
            format_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn was_formatted(&self) -> bool {
        !self.format_history.lock().unwrap().is_empty()
    }

    pub fn format_count(&self) -> usize {
        self.format_history.lock().unwrap().len()
    }
}

/// Mock device manager that provides fake devices for testing
pub struct MockDeviceManager {
    devices: Vec<Device>,
    enumerate_call_count: Arc<Mutex<usize>>,
}

impl Default for MockDeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MockDeviceManager {
    pub fn new() -> Self {
        Self {
            devices: vec![
                Device {
                    id: "mock://system/drive0".to_string(),
                    name: "System Drive".to_string(),
                    size: 500 * 1_073_741_824,
                    device_type: DeviceType::SSD,
                    mount_points: vec![PathBuf::from("/")],
                    is_removable: false,
                    is_system: true,
                },
                Device {
                    id: "mock://usb/test-drive".to_string(),
                    name: "Test USB Drive".to_string(),
                    size: 16 * 1_073_741_824,
                    device_type: DeviceType::USB,
                    mount_points: vec![],
                    is_removable: true,
                    is_system: false,
                },
            ],
            enumerate_call_count: Arc::new(Mutex::new(0)),
        }
    }

    pub fn with_devices(devices: Vec<Device>) -> Self {
        Self {
            devices,
            enumerate_call_count: Arc::new(Mutex::new(0)),
        }
    }

    pub fn call_count(&self) -> usize {
        *self.enumerate_call_count.lock().unwrap()
    }
}

#[async_trait::async_trait]
impl DeviceManager for MockDeviceManager {
    async fn enumerate_devices(&self) -> Result<Vec<Device>, MosesError> {
        *self.enumerate_call_count.lock().unwrap() += 1;
        Ok(self.devices.clone())
    }

    async fn get_device_info(&self, device: &Device) -> Result<DeviceInfo, MosesError> {
        Ok(DeviceInfo {
            device: device.clone(),
            filesystem: Some("NTFS".to_string()),
            label: Some("Test Drive".to_string()),
            used_space: Some(device.size / 2),
            free_space: Some(device.size / 2),
            partitions: vec![],
        })
    }

    async fn is_safe_to_format(&self, device: &Device) -> Result<bool, MosesError> {
        // CRITICAL SAFETY: Never allow formatting system drives in tests
        if device.is_system {
            return Ok(false);
        }
        
        // Check for critical mount points
        for mount in &device.mount_points {
            let path_str = mount.to_string_lossy().to_uppercase();
            if path_str == "C:" || path_str == "/" || path_str.starts_with("C:\\") {
                return Ok(false);
            }
        }
        
        Ok(true)
    }

    async fn check_permissions(&self, _device: &Device) -> Result<PermissionLevel, MosesError> {
        Ok(PermissionLevel::FullAccess)
    }
}

/// Mock formatter that simulates formatting without touching real devices
pub struct MockFormatter {
    pub name: String,
    pub format_calls: Arc<Mutex<Vec<(String, FormatOptions)>>>,
    pub should_fail: bool,
}

impl MockFormatter {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            format_calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: false,
        }
    }

    pub fn with_failure(name: &str) -> Self {
        Self {
            name: name.to_string(),
            format_calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: true,
        }
    }

    pub fn format_call_count(&self) -> usize {
        self.format_calls.lock().unwrap().len()
    }

    pub fn was_called_with(&self, device_id: &str) -> bool {
        self.format_calls
            .lock()
            .unwrap()
            .iter()
            .any(|(id, _)| id == device_id)
    }
}

#[async_trait::async_trait]
impl FilesystemFormatter for MockFormatter {
    fn name(&self) -> &'static str {
        Box::leak(self.name.clone().into_boxed_str())
    }

    fn supported_platforms(&self) -> Vec<crate::Platform> {
        vec![crate::Platform::Windows, crate::Platform::Linux, crate::Platform::MacOS]
    }

    fn can_format(&self, device: &Device) -> bool {
        !device.is_system
    }

    fn requires_external_tools(&self) -> bool {
        false
    }

    fn bundled_tools(&self) -> Vec<&'static str> {
        vec![]
    }

    async fn format(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<(), MosesError> {
        // CRITICAL: Never format real devices in tests
        if !device.id.starts_with("mock://") {
            return Err(MosesError::Other(
                "SAFETY: Attempted to format non-mock device in test!".to_string()
            ));
        }

        // Record the format call
        self.format_calls
            .lock()
            .unwrap()
            .push((device.id.clone(), options.clone()));

        if self.should_fail {
            return Err(MosesError::FormatError("Mock format failure".to_string()));
        }

        // Simulate formatting delay
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        Ok(())
    }

    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        if let Some(ref label) = options.label {
            if label.len() > 32 {
                return Err(MosesError::Other("Label too long".to_string()));
            }
        }
        Ok(())
    }

    async fn dry_run(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<SimulationReport, MosesError> {
        let mut warnings = vec![];
        
        if device.is_system {
            warnings.push("This is a system drive!".to_string());
        }
        
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: Duration::from_secs(30),
            warnings,
            required_tools: vec![],
            will_erase_data: true,
            space_after_format: device.size * 95 / 100,
        })
    }
}

/// Safety validator for testing safety checks
pub struct SafetyValidator;

impl SafetyValidator {
    /// Validate that a device is safe to format
    pub fn validate_device_safety(device: &Device) -> Result<(), String> {
        // Rule 1: Never format system drives
        if device.is_system {
            return Err("Cannot format system drive".to_string());
        }

        // Rule 2: Never format mounted system paths
        for mount in &device.mount_points {
            let path = mount.to_string_lossy().to_uppercase();
            if path == "C:" || path == "/" || path == "/boot" || path == "/System" {
                return Err(format!("Cannot format drive mounted at critical path: {}", path));
            }
        }

        // Rule 3: Device ID must be valid
        if device.id.is_empty() {
            return Err("Device ID cannot be empty".to_string());
        }

        // Rule 4: Size must be reasonable (not 0, not impossibly large)
        if device.size == 0 {
            return Err("Device size cannot be zero".to_string());
        }
        
        if device.size > 100 * 1024_u64.pow(4) { // 100 TB seems reasonable max
            return Err("Device size unreasonably large".to_string());
        }

        Ok(())
    }

    /// Validate format options
    pub fn validate_format_options(options: &FormatOptions) -> Result<(), String> {
        // Filesystem type must be specified
        if options.filesystem_type.is_empty() {
            return Err("Filesystem type must be specified".to_string());
        }

        // Label validation
        if let Some(ref label) = options.label {
            if label.is_empty() {
                return Err("Label cannot be empty if specified".to_string());
            }
            if label.len() > 32 {
                return Err("Label too long (max 32 characters)".to_string());
            }
            // Check for invalid characters
            if label.contains(&['/', '\\', '\0', '\n', '\r'][..]) {
                return Err("Label contains invalid characters".to_string());
            }
        }

        // Cluster size validation
        if let Some(cluster_size) = options.cluster_size {
            let valid_sizes = [512, 1024, 2048, 4096, 8192, 16384, 32768, 65536];
            if !valid_sizes.contains(&cluster_size) {
                return Err(format!("Invalid cluster size: {}", cluster_size));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safety_validator_blocks_system_drives() {
        let system_drive = Device {
            id: "test".to_string(),
            name: "System".to_string(),
            size: 500 * 1_073_741_824,
            device_type: DeviceType::SSD,
            mount_points: vec![],
            is_removable: false,
            is_system: true,
        };

        let result = SafetyValidator::validate_device_safety(&system_drive);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Cannot format system drive");
    }

    #[test]
    fn test_safety_validator_blocks_critical_mounts() {
        let critical_drive = Device {
            id: "test".to_string(),
            name: "Drive".to_string(),
            size: 100 * 1_073_741_824,
            device_type: DeviceType::SSD,
            mount_points: vec![PathBuf::from("C:")],
            is_removable: false,
            is_system: false,
        };

        let result = SafetyValidator::validate_device_safety(&critical_drive);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("critical path"));
    }

    #[test]
    fn test_safety_validator_allows_safe_devices() {
        let safe_usb = Device {
            id: "usb123".to_string(),
            name: "USB Drive".to_string(),
            size: 16 * 1_073_741_824,
            device_type: DeviceType::USB,
            mount_points: vec![],
            is_removable: true,
            is_system: false,
        };

        let result = SafetyValidator::validate_device_safety(&safe_usb);
        assert!(result.is_ok());
    }
}