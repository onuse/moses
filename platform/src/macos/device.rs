use moses_core::{Device, DeviceInfo, DeviceManager, DeviceType, MosesError, PermissionLevel};
use async_trait::async_trait;

pub struct MacOSDeviceManager;

#[async_trait]
impl DeviceManager for MacOSDeviceManager {
    async fn enumerate_devices(&self) -> Result<Vec<Device>, MosesError> {
        // macOS implementation using diskutil
        // This is a placeholder that compiles on all platforms
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            
            let output = Command::new("diskutil")
                .args(&["list"])
                .output()
                .map_err(|e| MosesError::Other(format!("Failed to run diskutil: {}", e)))?;

            if !output.status.success() {
                return Err(MosesError::Other("diskutil failed".to_string()));
            }

            // Parse diskutil output here
            // For now, return empty list
            Ok(vec![])
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            // Return empty list on non-macOS platforms for compilation
            Ok(vec![])
        }
    }

    async fn get_device_info(&self, device: &Device) -> Result<DeviceInfo, MosesError> {
        // Return mock device info for now
        Ok(DeviceInfo {
            path: device.id.clone(),
            name: device.name.clone(),
            size: device.size,
            device_type: device.device_type.clone(),
            mount_points: device.mount_points.clone(),
            is_removable: device.is_removable,
            is_system: device.is_system,
            filesystem: None,
            label: None,
            uuid: None,
        })
    }

    async fn is_safe_to_format(&self, device: &Device) -> Result<bool, MosesError> {
        // Check if device is safe to format
        if device.is_system {
            return Ok(false);
        }
        
        // Check for critical mount points
        for mount in &device.mount_points {
            let mount_str = mount.to_string_lossy().to_lowercase();
            if mount_str == "/" || mount_str.starts_with("/system") || mount_str.starts_with("/library") {
                return Ok(false);
            }
        }
        
        Ok(true)
    }

    async fn check_permissions(&self, device: &Device) -> Result<PermissionLevel, MosesError> {
        // On macOS, check if we have admin rights
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            
            // Try to run a command that requires admin
            let output = Command::new("diskutil")
                .args(&["info", &device.id])
                .output();
            
            match output {
                Ok(output) if output.status.success() => Ok(PermissionLevel::Admin),
                _ => Ok(PermissionLevel::User),
            }
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            Ok(PermissionLevel::User)
        }
    }
}