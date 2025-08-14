use moses_core::{Device, DeviceInfo, DeviceManager, DeviceType, MosesError, Partition, PermissionLevel};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct LinuxDeviceManager;

impl LinuxDeviceManager {
    fn parse_size(size_str: &str) -> Option<u64> {
        // Parse sizes like "16G", "500M", "1.5T"
        let size_str = size_str.trim();
        if size_str.is_empty() {
            return None;
        }
        
        let (number_part, unit) = size_str.split_at(size_str.len() - 1);
        let number = number_part.parse::<f64>().ok()?;
        
        let multiplier = match unit {
            "K" => 1024,
            "M" => 1024 * 1024,
            "G" => 1024 * 1024 * 1024,
            "T" => 1024_u64.pow(4),
            _ => return size_str.parse::<u64>().ok(),
        };
        
        Some((number * multiplier as f64) as u64)
    }
    
    fn is_removable(device_name: &str) -> bool {
        let removable_path = format!("/sys/block/{}/removable", device_name);
        fs::read_to_string(&removable_path)
            .map(|content| content.trim() == "1")
            .unwrap_or(false)
    }
    
    fn get_device_type(device_name: &str) -> DeviceType {
        // Check if it's removable first
        if Self::is_removable(device_name) {
            // Could be USB or SD card
            // Check if it's likely an SD card (mmcblk devices)
            if device_name.starts_with("mmcblk") {
                return DeviceType::SDCard;
            }
            return DeviceType::USB;
        }
        
        // Check rotational flag for SSD vs HDD
        let rotational_path = format!("/sys/block/{}/queue/rotational", device_name);
        let is_rotational = fs::read_to_string(&rotational_path)
            .map(|content| content.trim() == "1")
            .unwrap_or(true);
        
        if is_rotational {
            DeviceType::HardDisk
        } else {
            DeviceType::SSD
        }
    }
    
    fn get_mount_points(device_path: &str) -> Vec<PathBuf> {
        let mut mount_points = Vec::new();
        
        if let Ok(mounts) = fs::read_to_string("/proc/mounts") {
            for line in mounts.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[0].starts_with(device_path) {
                    mount_points.push(PathBuf::from(parts[1]));
                }
            }
        }
        
        mount_points
    }
    
    fn is_system_disk(device_path: &str, mount_points: &[PathBuf]) -> bool {
        // Check if any mount point is a system critical path
        for mount in mount_points {
            let path_str = mount.to_string_lossy();
            if path_str == "/" || 
               path_str == "/boot" || 
               path_str == "/boot/efi" ||
               path_str.starts_with("/sys") ||
               path_str.starts_with("/proc") {
                return true;
            }
        }
        
        // Also check if the device contains the current root filesystem
        if let Ok(cmdline) = fs::read_to_string("/proc/cmdline") {
            if cmdline.contains(device_path) {
                return true;
            }
        }
        
        false
    }
    
    fn get_device_model(device_name: &str) -> String {
        // Try to get model from /sys/block/{device}/device/model
        let model_path = format!("/sys/block/{}/device/model", device_name);
        if let Ok(model) = fs::read_to_string(&model_path) {
            return model.trim().to_string();
        }
        
        // Fallback to vendor + model
        let vendor_path = format!("/sys/block/{}/device/vendor", device_name);
        if let Ok(vendor) = fs::read_to_string(&vendor_path) {
            return vendor.trim().to_string();
        }
        
        // Default to device name
        device_name.to_uppercase()
    }
    
    async fn parse_lsblk_output(&self) -> Result<Vec<Device>, MosesError> {
        // Run lsblk to get device information
        let output = Command::new("lsblk")
            .args(&["-b", "-P", "-o", "NAME,SIZE,TYPE,MOUNTPOINT,FSTYPE,MODEL,VENDOR,RM,RO"])
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to run lsblk: {}", e)))?;
        
        if !output.status.success() {
            return Err(MosesError::Other("lsblk command failed".to_string()));
        }
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut devices = Vec::new();
        
        for line in output_str.lines() {
            let mut fields = HashMap::new();
            
            // Parse key="value" pairs
            let mut current_key = String::new();
            let mut current_value = String::new();
            let mut in_quotes = false;
            let mut escape_next = false;
            
            for ch in line.chars() {
                if escape_next {
                    current_value.push(ch);
                    escape_next = false;
                    continue;
                }
                
                if ch == '\\' {
                    escape_next = true;
                    continue;
                }
                
                if ch == '"' {
                    in_quotes = !in_quotes;
                    if !in_quotes && !current_key.is_empty() {
                        fields.insert(current_key.clone(), current_value.clone());
                        current_key.clear();
                        current_value.clear();
                    }
                } else if ch == '=' && !in_quotes {
                    // We're at the = between key and value
                    // Continue to build the key
                } else if ch == ' ' && !in_quotes {
                    // Space outside quotes, skip
                } else if in_quotes {
                    current_value.push(ch);
                } else {
                    current_key.push(ch);
                }
            }
            
            // Only process disk devices (not partitions)
            if let Some(device_type) = fields.get("TYPE") {
                if device_type != "disk" {
                    continue;
                }
            }
            
            let name = fields.get("NAME").unwrap_or(&String::new()).clone();
            if name.is_empty() {
                continue;
            }
            
            let device_path = format!("/dev/{}", name);
            let size = fields.get("SIZE")
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            
            let mount_points = Self::get_mount_points(&device_path);
            let is_system = Self::is_system_disk(&device_path, &mount_points);
            
            let model = fields.get("MODEL")
                .map(|m| m.trim().to_string())
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| Self::get_device_model(&name));
            
            let is_removable = fields.get("RM")
                .map(|rm| rm == "1")
                .unwrap_or_else(|| Self::is_removable(&name));
            
            let device = Device {
                id: device_path.clone(),
                name: if !model.is_empty() { 
                    format!("{} ({})", model, name)
                } else { 
                    name.clone() 
                },
                size,
                device_type: Self::get_device_type(&name),
                mount_points,
                is_removable,
                is_system,
            };
            
            devices.push(device);
        }
        
        // Sort devices: removable first, then by name
        devices.sort_by(|a, b| {
            match (a.is_removable, b.is_removable) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });
        
        Ok(devices)
    }
    
    async fn get_partitions(&self, device_path: &str) -> Vec<Partition> {
        let mut partitions = Vec::new();
        
        // Get device name without /dev/
        let _device_name = device_path.trim_start_matches("/dev/");
        
        // List partitions using lsblk
        if let Ok(output) = Command::new("lsblk")
            .args(&["-b", "-n", "-o", "NAME,SIZE,FSTYPE,MOUNTPOINT", device_path])
            .output() {
            
            let output_str = String::from_utf8_lossy(&output.stdout);
            for (i, line) in output_str.lines().enumerate() {
                if i == 0 { continue; } // Skip the parent device
                
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let name = parts[0].trim_start_matches('├').trim_start_matches('└').trim_start_matches('─');
                    let size = parts[1].parse::<u64>().unwrap_or(0);
                    let filesystem = if parts.len() > 2 { Some(parts[2].to_string()) } else { None };
                    let mount_point = if parts.len() > 3 { Some(PathBuf::from(parts[3])) } else { None };
                    
                    partitions.push(Partition {
                        id: format!("/dev/{}", name),
                        size,
                        filesystem,
                        mount_point,
                    });
                }
            }
        }
        
        partitions
    }
}

#[async_trait::async_trait]
impl DeviceManager for LinuxDeviceManager {
    async fn enumerate_devices(&self) -> Result<Vec<Device>, MosesError> {
        // First try lsblk which is more reliable
        if let Ok(devices) = self.parse_lsblk_output().await {
            if !devices.is_empty() {
                return Ok(devices);
            }
        }
        
        // Fallback to reading /sys/block
        let mut devices = Vec::new();
        let sys_block = Path::new("/sys/block");
        
        if !sys_block.exists() {
            return Err(MosesError::Other("Cannot access /sys/block".to_string()));
        }
        
        let entries = fs::read_dir(sys_block)
            .map_err(|e| MosesError::Other(format!("Failed to read /sys/block: {}", e)))?;
        
        for entry in entries {
            let entry = entry.map_err(|e| MosesError::Other(format!("Failed to read entry: {}", e)))?;
            let device_name = entry.file_name().to_string_lossy().to_string();
            
            // Skip loop devices and ram disks
            if device_name.starts_with("loop") || device_name.starts_with("ram") {
                continue;
            }
            
            // Read size from /sys/block/{device}/size (in 512-byte sectors)
            let size_path = format!("/sys/block/{}/size", device_name);
            let size = fs::read_to_string(&size_path)
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0) * 512;
            
            // Skip devices with 0 size
            if size == 0 {
                continue;
            }
            
            let device_path = format!("/dev/{}", device_name);
            let mount_points = Self::get_mount_points(&device_path);
            let is_system = Self::is_system_disk(&device_path, &mount_points);
            
            devices.push(Device {
                id: device_path.clone(),
                name: Self::get_device_model(&device_name),
                size,
                device_type: Self::get_device_type(&device_name),
                mount_points,
                is_removable: Self::is_removable(&device_name),
                is_system,
            });
        }
        
        Ok(devices)
    }
    
    async fn get_device_info(&self, device: &Device) -> Result<DeviceInfo, MosesError> {
        let partitions = self.get_partitions(&device.id).await;
        
        // Try to get filesystem info for the whole device
        let output = Command::new("blkid")
            .arg(&device.id)
            .output()
            .ok();
        
        let blkid_stdout = output.as_ref().and_then(|o| String::from_utf8(o.stdout.clone()).ok());
        
        let filesystem = blkid_stdout.as_ref()
            .and_then(|s| {
                s.split_whitespace()
                    .find(|part| part.starts_with("TYPE="))
                    .map(|t| t.trim_start_matches("TYPE=").trim_matches('"').to_string())
            });
        
        let label = blkid_stdout.as_ref()
            .and_then(|s| {
                s.split_whitespace()
                    .find(|part| part.starts_with("LABEL="))
                    .map(|t| t.trim_start_matches("LABEL=").trim_matches('"').to_string())
            });
        
        // Calculate used/free space if mounted
        let (used_space, free_space) = if !device.mount_points.is_empty() {
            if let Ok(output) = Command::new("df")
                .args(&["-B1", device.mount_points[0].to_str().unwrap_or("")])
                .output() {
                
                let output_str = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = output_str.lines().nth(1) {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        let used = parts[2].parse::<u64>().ok();
                        let available = parts[3].parse::<u64>().ok();
                        (used, available)
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };
        
        Ok(DeviceInfo {
            device: device.clone(),
            filesystem,
            label,
            used_space,
            free_space,
            partitions,
        })
    }
    
    async fn is_safe_to_format(&self, device: &Device) -> Result<bool, MosesError> {
        // Don't format system disks
        if device.is_system {
            return Ok(false);
        }
        
        // Don't format mounted devices
        if !device.mount_points.is_empty() {
            return Ok(false);
        }
        
        // Additional safety: check if device is in use
        let device_name = device.id.trim_start_matches("/dev/");
        let holders_path = format!("/sys/block/{}/holders", device_name);
        if let Ok(entries) = fs::read_dir(&holders_path) {
            if entries.count() > 0 {
                return Ok(false); // Device has holders (e.g., LVM, RAID)
            }
        }
        
        Ok(true)
    }
    
    async fn check_permissions(&self, _device: &Device) -> Result<PermissionLevel, MosesError> {
        // Check if running as root
        if nix::unistd::geteuid().is_root() {
            return Ok(PermissionLevel::FullAccess);
        }
        
        // Check if user is in disk group
        if let Ok(groups) = nix::unistd::getgroups() {
            let disk_gid = nix::unistd::Group::from_name("disk")
                .map_err(|e| MosesError::Other(format!("Failed to get disk group: {}", e)))?
                .map(|g| g.gid);
            
            if let Some(gid) = disk_gid {
                if groups.contains(&gid) {
                    return Ok(PermissionLevel::Simulate);
                }
            }
        }
        
        Ok(PermissionLevel::ReadOnly)
    }
}