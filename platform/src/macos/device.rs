use moses_core::{Device, DeviceInfo, DeviceManager, DeviceType, MosesError};
use std::path::PathBuf;
use std::process::Command;

pub struct MacOSDeviceManager;

impl DeviceManager for MacOSDeviceManager {
    fn list_devices(&self) -> Result<Vec<Device>, MosesError> {
        // Use diskutil to list devices
        let output = Command::new("diskutil")
            .args(&["list", "-plist"])
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to run diskutil: {}", e)))?;

        if !output.status.success() {
            return Err(MosesError::Other("diskutil failed".to_string()));
        }

        // For now, return a simplified implementation
        // In production, this would parse the plist output
        let devices = vec![];
        
        Ok(devices)
    }

    fn get_device_info(&self, device_path: &str) -> Result<DeviceInfo, MosesError> {
        // Use diskutil info to get device details
        let output = Command::new("diskutil")
            .args(&["info", device_path])
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to get device info: {}", e)))?;

        if !output.status.success() {
            return Err(MosesError::Other("Failed to get device info".to_string()));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        
        // Parse the output to extract device information
        // This is a simplified implementation
        Ok(DeviceInfo {
            path: device_path.to_string(),
            name: extract_field(&output_str, "Device / Media Name:").unwrap_or_default(),
            size: extract_size(&output_str),
            device_type: detect_device_type(&output_str),
            mount_points: extract_mount_points(&output_str),
            is_removable: output_str.contains("Removable Media: Yes"),
            is_system: output_str.contains("Boot Volume: Yes"),
            filesystem: extract_field(&output_str, "File System Personality:"),
            label: extract_field(&output_str, "Volume Name:"),
            uuid: extract_field(&output_str, "Volume UUID:"),
        })
    }

    fn unmount(&self, device: &Device) -> Result<(), MosesError> {
        let output = Command::new("diskutil")
            .args(&["unmount", &device.id])
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to unmount: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(MosesError::Other(format!("Unmount failed: {}", stderr)));
        }

        Ok(())
    }

    fn mount(&self, device: &Device) -> Result<(), MosesError> {
        let output = Command::new("diskutil")
            .args(&["mount", &device.id])
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to mount: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(MosesError::Other(format!("Mount failed: {}", stderr)));
        }

        Ok(())
    }
}

// Helper functions for parsing diskutil output
fn extract_field(output: &str, field_name: &str) -> Option<String> {
    output.lines()
        .find(|line| line.contains(field_name))
        .and_then(|line| line.split(':').nth(1))
        .map(|s| s.trim().to_string())
}

fn extract_size(output: &str) -> u64 {
    // Look for "Disk Size" or "Total Size" in the output
    if let Some(size_line) = output.lines()
        .find(|line| line.contains("Total Size:") || line.contains("Disk Size:")) 
    {
        // Parse size from format like "500.1 GB (500107862016 Bytes)"
        if let Some(bytes_str) = size_line.split('(').nth(1) {
            if let Some(bytes) = bytes_str.split(' ').next() {
                return bytes.parse().unwrap_or(0);
            }
        }
    }
    0
}

fn detect_device_type(output: &str) -> DeviceType {
    if output.contains("Protocol: USB") {
        DeviceType::USB
    } else if output.contains("Solid State: Yes") {
        DeviceType::SSD
    } else if output.contains("Device Location: Internal") {
        DeviceType::HardDisk
    } else if output.contains("Protocol: Disk Image") {
        DeviceType::Virtual
    } else {
        DeviceType::Unknown
    }
}

fn extract_mount_points(output: &str) -> Vec<PathBuf> {
    if let Some(mount_line) = output.lines()
        .find(|line| line.contains("Mount Point:"))
    {
        if let Some(mount_point) = mount_line.split(':').nth(1) {
            let mount_str = mount_point.trim();
            if !mount_str.is_empty() && mount_str != "(not mounted)" {
                return vec![PathBuf::from(mount_str)];
            }
        }
    }
    vec![]
}