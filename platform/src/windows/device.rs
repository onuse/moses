use moses_core::{Device, DeviceInfo, DeviceManager, DeviceType, MosesError, Partition, PermissionLevel};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Deserialize, Serialize)]
struct WindowsDisk {
    #[serde(rename = "Number")]
    number: u32,
    #[serde(rename = "FriendlyName")]
    friendly_name: Option<String>,
    #[serde(rename = "Size")]
    size: u64,
    #[serde(rename = "PartitionStyle")]
    partition_style: Option<String>,
    #[serde(rename = "BusType")]
    bus_type: Option<String>,
    #[serde(rename = "MediaType")]
    media_type: Option<String>,
    #[serde(rename = "IsSystem")]
    is_system: bool,
    #[serde(rename = "IsBoot")]
    is_boot: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct WindowsPartition {
    #[serde(rename = "DiskNumber")]
    disk_number: u32,
    #[serde(rename = "PartitionNumber")]
    partition_number: u32,
    #[serde(rename = "DriveLetter")]
    drive_letter: Option<String>,
    #[serde(rename = "Size")]
    size: u64,
    #[serde(rename = "Type")]
    partition_type: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct WmiDiskDrive {
    #[serde(rename = "DeviceID")]
    device_id: String,
    #[serde(rename = "Model")]
    model: Option<String>,
    #[serde(rename = "Size")]
    size: Option<u64>,
    #[serde(rename = "MediaType")]
    media_type: Option<String>,
    #[serde(rename = "InterfaceType")]
    interface_type: Option<String>,
}

pub struct WindowsDeviceManager;

impl WindowsDeviceManager {
    fn get_device_type(bus_type: Option<&str>, media_type: Option<&str>, interface_type: Option<&str>) -> DeviceType {
        // Check bus type first
        if let Some(bus) = bus_type {
            match bus.to_uppercase().as_str() {
                "USB" => return DeviceType::USB,
                "SD" | "MMC" => return DeviceType::SDCard,
                _ => {}
            }
        }
        
        // Check interface type
        if let Some(interface) = interface_type {
            if interface.to_uppercase() == "USB" {
                return DeviceType::USB;
            }
        }
        
        // Check media type
        if let Some(media) = media_type {
            let media_lower = media.to_lowercase();
            if media_lower.contains("removable") {
                return DeviceType::USB;
            } else if media_lower.contains("external") {
                return DeviceType::USB;
            } else if media_lower.contains("fixed") {
                // Could be SSD or HDD - in Windows it's hard to distinguish
                // We'll default to SSD for NVMe drives
                if let Some(bus) = bus_type {
                    if bus.to_uppercase() == "NVME" {
                        return DeviceType::SSD;
                    }
                }
                return DeviceType::HardDisk;
            }
        }
        
        DeviceType::Unknown
    }
    
    fn is_removable(media_type: Option<&str>, bus_type: Option<&str>) -> bool {
        if let Some(media) = media_type {
            if media.to_lowercase().contains("removable") || media.to_lowercase().contains("external") {
                return true;
            }
        }
        
        if let Some(bus) = bus_type {
            matches!(bus.to_uppercase().as_str(), "USB" | "SD" | "MMC")
        } else {
            false
        }
    }
    
    async fn get_disks_powershell(&self) -> Result<Vec<WindowsDisk>, MosesError> {
        let output = Command::new("powershell.exe")
            .args(&[
                "-NoProfile",
                "-Command",
                "Get-Disk | Select-Object Number, FriendlyName, Size, PartitionStyle, BusType, MediaType, IsSystem, IsBoot | ConvertTo-Json"
            ])
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to run PowerShell: {}", e)))?;
        
        if !output.status.success() {
            return Err(MosesError::Other("PowerShell command failed".to_string()));
        }
        
        let json_str = String::from_utf8_lossy(&output.stdout);
        
        // Handle both single object and array
        if json_str.trim().starts_with('[') {
            serde_json::from_str(&json_str)
                .map_err(|e| MosesError::Other(format!("Failed to parse JSON: {}", e)))
        } else {
            // Single disk, wrap in array
            let disk: WindowsDisk = serde_json::from_str(&json_str)
                .map_err(|e| MosesError::Other(format!("Failed to parse JSON: {}", e)))?;
            Ok(vec![disk])
        }
    }
    
    async fn get_wmi_disk_drives(&self) -> Result<Vec<WmiDiskDrive>, MosesError> {
        let output = Command::new("powershell.exe")
            .args(&[
                "-NoProfile",
                "-Command",
                "Get-WmiObject Win32_DiskDrive | Select-Object DeviceID, Model, Size, MediaType, InterfaceType | ConvertTo-Json"
            ])
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to run WMI query: {}", e)))?;
        
        if !output.status.success() {
            return Err(MosesError::Other("WMI query failed".to_string()));
        }
        
        let json_str = String::from_utf8_lossy(&output.stdout);
        
        // Handle both single object and array
        if json_str.trim().starts_with('[') {
            serde_json::from_str(&json_str)
                .map_err(|e| MosesError::Other(format!("Failed to parse WMI JSON: {}", e)))
        } else {
            let drive: WmiDiskDrive = serde_json::from_str(&json_str)
                .map_err(|e| MosesError::Other(format!("Failed to parse WMI JSON: {}", e)))?;
            Ok(vec![drive])
        }
    }
    
    async fn get_partitions(&self, disk_number: u32) -> Vec<Partition> {
        let output = Command::new("powershell.exe")
            .args(&[
                "-NoProfile",
                "-Command",
                &format!("Get-Partition | Where-Object {{$_.DiskNumber -eq {}}} | Select-Object DiskNumber, PartitionNumber, DriveLetter, Size, Type | ConvertTo-Json", disk_number)
            ])
            .output();
        
        if let Ok(output) = output {
            if output.status.success() {
                let json_str = String::from_utf8_lossy(&output.stdout);
                
                // Parse partitions
                let partitions: Vec<WindowsPartition> = if json_str.trim().starts_with('[') {
                    serde_json::from_str(&json_str).unwrap_or_default()
                } else if !json_str.trim().is_empty() {
                    vec![serde_json::from_str(&json_str).unwrap_or_default()]
                } else {
                    vec![]
                };
                
                return partitions.into_iter().map(|p| {
                    let mount_point = p.drive_letter.map(|letter| {
                        PathBuf::from(format!("{}:", letter))
                    });
                    
                    Partition {
                        id: format!("Partition{}", p.partition_number),
                        size: p.size,
                        filesystem: p.partition_type,
                        mount_point,
                    }
                }).collect();
            }
        }
        
        vec![]
    }
}

#[async_trait::async_trait]
impl DeviceManager for WindowsDeviceManager {
    async fn enumerate_devices(&self) -> Result<Vec<Device>, MosesError> {
        // Get disk info from PowerShell
        let ps_disks = self.get_disks_powershell().await?;
        
        // Get additional info from WMI for better model names
        let wmi_drives = self.get_wmi_disk_drives().await.unwrap_or_default();
        
        let mut devices = Vec::new();
        
        for disk in ps_disks {
            // Find corresponding WMI drive for model name
            let wmi_drive = wmi_drives.iter()
                .find(|d| d.device_id == format!("\\\\.\\PHYSICALDRIVE{}", disk.number));
            
            // Get mount points from partitions
            let partitions = self.get_partitions(disk.number).await;
            let mount_points: Vec<PathBuf> = partitions.iter()
                .filter_map(|p| p.mount_point.clone())
                .collect();
            
            // Determine device name
            let name = if let Some(wmi) = wmi_drive {
                wmi.model.clone().unwrap_or_else(|| {
                    disk.friendly_name.unwrap_or_else(|| format!("Disk {}", disk.number))
                })
            } else {
                disk.friendly_name.unwrap_or_else(|| format!("Disk {}", disk.number))
            };
            
            let device_type = if let Some(wmi) = wmi_drive {
                Self::get_device_type(
                    disk.bus_type.as_deref(),
                    wmi.media_type.as_deref(),
                    wmi.interface_type.as_deref()
                )
            } else {
                Self::get_device_type(
                    disk.bus_type.as_deref(),
                    disk.media_type.as_deref(),
                    None
                )
            };
            
            let is_removable = if let Some(wmi) = wmi_drive {
                Self::is_removable(wmi.media_type.as_deref(), disk.bus_type.as_deref())
            } else {
                Self::is_removable(disk.media_type.as_deref(), disk.bus_type.as_deref())
            };
            
            devices.push(Device {
                id: format!("\\\\.\\PHYSICALDRIVE{}", disk.number),
                name,
                size: disk.size,
                device_type,
                mount_points,
                is_removable,
                is_system: disk.is_system || disk.is_boot,
            });
        }
        
        // Sort devices: removable first, then by disk number
        devices.sort_by(|a, b| {
            match (a.is_removable, b.is_removable) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.id.cmp(&b.id),
            }
        });
        
        Ok(devices)
    }
    
    async fn get_device_info(&self, device: &Device) -> Result<DeviceInfo, MosesError> {
        // Extract disk number from device ID
        let disk_number = device.id
            .trim_start_matches("\\\\.\\PHYSICALDRIVE")
            .parse::<u32>()
            .map_err(|_| MosesError::Other("Invalid device ID".to_string()))?;
        
        let partitions = self.get_partitions(disk_number).await;
        
        // For Windows, we don't easily get filesystem info for the whole disk
        // We'd need to check the first partition
        let filesystem = partitions.first()
            .and_then(|p| p.filesystem.clone());
        
        Ok(DeviceInfo {
            device: device.clone(),
            filesystem,
            label: None, // Would need to query volume label
            used_space: None, // Would need to query volume info
            free_space: None,
            partitions,
        })
    }
    
    async fn is_safe_to_format(&self, device: &Device) -> Result<bool, MosesError> {
        // Don't format system disks
        if device.is_system {
            return Ok(false);
        }
        
        // Check if any partitions are mounted with critical paths
        for mount in &device.mount_points {
            let path_str = mount.to_string_lossy().to_uppercase();
            // C: is typically the system drive
            if path_str == "C:" || path_str.starts_with("C:\\") {
                return Ok(false);
            }
        }
        
        Ok(true)
    }
    
    async fn check_permissions(&self, _device: &Device) -> Result<PermissionLevel, MosesError> {
        // Check if running as Administrator
        let output = Command::new("powershell.exe")
            .args(&[
                "-NoProfile",
                "-Command",
                "([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole] 'Administrator')"
            ])
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to check admin status: {}", e)))?;
        
        let is_admin = String::from_utf8_lossy(&output.stdout).trim() == "True";
        
        Ok(if is_admin {
            PermissionLevel::FullAccess
        } else {
            PermissionLevel::ReadOnly
        })
    }
}