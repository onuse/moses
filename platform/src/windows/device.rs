use moses_core::{Device, DeviceInfo, DeviceManager, DeviceType, MosesError, Partition, PermissionLevel};
use std::fs::File;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[derive(Debug, Deserialize, Serialize)]
struct WindowsDisk {
    #[serde(rename = "Number")]
    number: u32,
    #[serde(rename = "FriendlyName")]
    friendly_name: Option<String>,
    #[serde(rename = "Size")]
    size: u64,
    #[serde(rename = "PartitionStyle")]
    #[allow(dead_code)]
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

#[derive(Debug, Default, Deserialize, Serialize)]
struct WindowsPartition {
    #[serde(rename = "DiskNumber")]
    #[allow(dead_code)]
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
    /// Detect filesystem type by reading boot sector signatures
    fn detect_filesystem(device_path: &str) -> Option<String> {
        log::debug!("Detecting filesystem for device: {}", device_path);
        
        let mut file = match File::open(device_path) {
            Ok(f) => f,
            Err(e) => {
                log::debug!("Failed to open device {} for filesystem detection: {} (this is normal without admin rights)", device_path, e);
                return None;
            }
        };
        
        // Use the unified detection system from formatters crate
        match moses_formatters::detection::detect_filesystem(&mut file) {
            Ok(fs_type) => {
                if fs_type != "unknown" {
                    log::info!("Detected {} filesystem on {}", fs_type, device_path);
                    Some(fs_type)
                } else {
                    None
                }
            }
            Err(e) => {
                log::debug!("Failed to detect filesystem on {}: {:?}", device_path, e);
                None
            }
        }
    }
    
    /// Detect partition table type (GPT/MBR) when no filesystem is detected
    fn detect_partition_table_type(device_path: &str) -> Option<String> {
        log::debug!("Detecting partition table type for device: {}", device_path);
        
        // Create a temporary Device struct for the diagnostics module
        let device = Device {
            id: device_path.to_string(),
            name: "Temp".to_string(),
            size: 0,
            device_type: DeviceType::Unknown,
            is_removable: false,
            is_system: false,
            mount_points: vec![],
            filesystem: None,
        };
        
        // Use the diagnostics module to get filesystem/partition table type
        match moses_formatters::diagnostics::get_filesystem_type(&device) {
            Ok(fs_type) => {
                // Only return if it's a partition table type, not a filesystem
                if fs_type == "gpt" || fs_type == "gpt-empty" || fs_type == "mbr" {
                    log::info!("Detected {} partition table on {}", fs_type, device_path);
                    Some(fs_type)
                } else if fs_type != "unknown" {
                    // It's actually a filesystem, return it
                    log::info!("Detected {} filesystem on {}", fs_type, device_path);
                    Some(fs_type)
                } else {
                    None
                }
            }
            Err(e) => {
                log::debug!("Failed to detect partition table on {}: {:?}", device_path, e);
                None
            }
        }
    }
    
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
        let mut cmd = Command::new("powershell.exe");
        
        #[cfg(target_os = "windows")]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        
        let output = cmd
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
        let mut cmd = Command::new("powershell.exe");
        
        #[cfg(target_os = "windows")]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        
        let output = cmd
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
    
    async fn get_all_volume_filesystems(&self) -> std::collections::HashMap<String, String> {
        let mut cmd = Command::new("powershell.exe");
        
        #[cfg(target_os = "windows")]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        
        // Get all volumes and their filesystems in one call
        let output = match cmd
            .args(&[
                "-NoProfile",
                "-Command",
                "Get-Volume | Where-Object { $_.DriveLetter -ne $null } | Select-Object DriveLetter, FileSystemType | ConvertTo-Json"
            ])
            .output() {
            Ok(o) => o,
            Err(e) => {
                log::warn!("Failed to get volume filesystems: {}", e);
                return std::collections::HashMap::new();
            }
        };
        
        if !output.status.success() {
            return std::collections::HashMap::new();
        }
        
        let json_str = String::from_utf8_lossy(&output.stdout);
        
        #[derive(Deserialize)]
        struct VolumeInfo {
            #[serde(rename = "DriveLetter")]
            drive_letter: Option<String>,
            #[serde(rename = "FileSystemType")]
            filesystem_type: Option<String>,
        }
        
        let volumes: Vec<VolumeInfo> = if json_str.trim().starts_with('[') {
            serde_json::from_str(&json_str).unwrap_or_default()
        } else {
            // Single volume
            serde_json::from_str::<VolumeInfo>(&json_str)
                .map(|v| vec![v])
                .unwrap_or_default()
        };
        
        let mut fs_map = std::collections::HashMap::new();
        for vol in volumes {
            if let (Some(letter), Some(fs_type)) = (vol.drive_letter, vol.filesystem_type) {
                if !letter.is_empty() && !fs_type.is_empty() && fs_type.to_lowercase() != "raw" {
                    fs_map.insert(format!("{}:", letter), fs_type.to_lowercase());
                }
            }
        }
        
        log::info!("Detected filesystems for {} volumes", fs_map.len());
        fs_map
    }

    async fn get_partitions(&self, disk_number: u32) -> Vec<Partition> {
        let mut cmd = Command::new("powershell.exe");
        
        #[cfg(target_os = "windows")]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        
        let output = cmd
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

#[async_trait]
impl DeviceManager for WindowsDeviceManager {
    async fn enumerate_devices(&self) -> Result<Vec<Device>, MosesError> {
        // Get disk info from PowerShell
        let ps_disks = self.get_disks_powershell().await?;
        
        // Get additional info from WMI for better model names
        let wmi_drives = self.get_wmi_disk_drives().await.unwrap_or_default();
        
        // Get all filesystem types in one batch call to avoid slow individual queries
        let volume_filesystems = self.get_all_volume_filesystems().await;
        log::info!("Pre-fetched filesystem types for {} volumes", volume_filesystems.len());
        
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
            
            // Detect filesystem type
            // Only try direct physical drive access if we have admin rights
            let mut filesystem = if crate::windows::elevation::is_elevated() {
                let device_path = format!("\\\\.\\PHYSICALDRIVE{}", disk.number);
                // First try to detect filesystem
                Self::detect_filesystem(&device_path).or_else(|| {
                    // If no filesystem detected, try to detect partition table type
                    Self::detect_partition_table_type(&device_path)
                })
            } else {
                None
            };
            
            // If we don't have admin rights or direct detection failed, and we have mount points, try to get filesystem from our pre-fetched map
            if filesystem.is_none() && !mount_points.is_empty() {
                for mount_point in &mount_points {
                    let mount_str = mount_point.to_string_lossy();
                    // Check if it's a drive letter like "E:" or "E:\"
                    if mount_str.len() >= 2 && mount_str.chars().nth(1) == Some(':') {
                        let drive_key = if mount_str.ends_with('\\') {
                            mount_str[..2].to_string()
                        } else {
                            mount_str.to_string()
                        };
                        
                        // Look up in our pre-fetched map
                        if let Some(fs_type) = volume_filesystems.get(&drive_key) {
                            // Windows might report "raw" for ext4 or other unrecognized filesystems
                            if fs_type == "raw" {
                                log::info!("Windows reports RAW filesystem for {} - might be ext4 or other Linux filesystem", drive_key);
                                // Don't set filesystem, keep trying other methods
                            } else {
                                filesystem = Some(fs_type.clone());
                                log::info!("Got filesystem type '{}' from batch query for {}", fs_type, drive_key);
                                break;
                            }
                        }
                    }
                }
            }
            
            // Log the result with appropriate context
            if filesystem.is_none() && !mount_points.is_empty() {
                log::info!("Device {} filesystem could not be detected without admin rights - showing as 'unknown'", 
                          format!("\\\\.\\PHYSICALDRIVE{}", disk.number));
                log::info!("To detect ext4/ext3/ext2 filesystems, please run with administrator privileges");
            } else {
                log::info!("Device {} filesystem detection result: {:?}", 
                          format!("\\\\.\\PHYSICALDRIVE{}", disk.number), filesystem);
            }
            
            devices.push(Device {
                id: format!("\\\\.\\PHYSICALDRIVE{}", disk.number),
                name,
                size: disk.size,
                device_type,
                mount_points,
                is_removable,
                is_system: disk.is_system || disk.is_boot,
                filesystem,
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
    

    async fn get_device_by_id(&self, device_id: &str) -> Result<Option<Device>, MosesError> {
        // Extract disk number from device ID
        let disk_number = if let Some(num_str) = device_id.strip_prefix("\\\\.\\PHYSICALDRIVE") {
            num_str.parse::<u32>().ok()
        } else {
            None
        };
        
        if let Some(disk_num) = disk_number {
            log::debug!("Getting single device info for disk {}", disk_num);
            
            // Get just this specific disk info from PowerShell
            let mut cmd = Command::new("powershell.exe");
            
            #[cfg(target_os = "windows")]
            {
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            
            let output = cmd
                .args(&[
                    "-NoProfile",
                    "-Command",
                    &format!("Get-Disk -Number {} | Select-Object Number, FriendlyName, Size, PartitionStyle, BusType, MediaType, IsSystem, IsBoot | ConvertTo-Json", disk_num)
                ])
                .output()
                .map_err(|e| MosesError::Other(format!("Failed to run PowerShell: {}", e)))?;
            
            if !output.status.success() {
                return Ok(None);
            }
            
            let json_str = String::from_utf8_lossy(&output.stdout);
            if json_str.trim().is_empty() {
                return Ok(None);
            }
            
            let disk: WindowsDisk = match serde_json::from_str(&json_str) {
                Ok(d) => d,
                Err(_) => return Ok(None)
            };
            
            // Get mount points from partitions
            let partitions = self.get_partitions(disk_num).await;
            let mount_points: Vec<PathBuf> = partitions.iter()
                .filter_map(|p| p.mount_point.clone())
                .collect();
            
            // Detect filesystem type if we have admin rights
            let filesystem = if crate::windows::elevation::is_elevated() {
                let device_path = format!("\\\\.\\PHYSICALDRIVE{}", disk_num);
                Self::detect_filesystem(&device_path).or_else(|| {
                    Self::detect_partition_table_type(&device_path)
                })
            } else {
                None
            };
            
            let device_type = Self::get_device_type(
                disk.bus_type.as_deref(),
                disk.media_type.as_deref(),
                None
            );
            
            let is_removable = Self::is_removable(disk.media_type.as_deref(), disk.bus_type.as_deref());
            
            let name = disk.friendly_name.unwrap_or_else(|| format!("Disk {}", disk_num));
            
            log::debug!("Found device: {} ({})", name, device_id);
            
            Ok(Some(Device {
                id: device_id.to_string(),
                name,
                size: disk.size,
                device_type,
                mount_points,
                is_removable,
                is_system: disk.is_system || disk.is_boot,
                filesystem,
            }))
        } else {
            Ok(None)
        }
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
        let mut cmd = Command::new("powershell.exe");
        
        #[cfg(target_os = "windows")]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        
        let output = cmd
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