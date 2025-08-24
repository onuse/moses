use moses_core::Device;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Mutex;
use once_cell::sync::Lazy;
use moses_filesystems::device_reader::FilesystemReader;
use moses_filesystems::diagnostics::analyze_unknown_filesystem;
use crate::filesystem_cache;

// Cache for filesystem types to avoid repeated admin prompts
pub(crate) static FILESYSTEM_CACHE: Lazy<Mutex<HashMap<String, String>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

#[derive(Debug, Serialize, Deserialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub entry_type: EntryType,
    pub size: Option<u64>,
    pub modified: Option<DateTime<Utc>>,
    pub created: Option<DateTime<Utc>>,
    pub permissions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<FilesystemMetadata>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FilesystemMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compressed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sparse: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reparse_point: Option<String>, // Type of reparse point
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allocated_size: Option<u64>,   // Actual size on disk
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mft_record: Option<u64>,       // MFT record number for NTFS
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryType {
    File,
    Directory,
    Symlink,
    Device,
    Other,
}

#[derive(Debug, Serialize)]
pub struct DirectoryListing {
    pub path: String,
    pub entries: Vec<DirectoryEntry>,
    pub total_size: u64,
    pub item_count: usize,
}

/// Read directory contents from a filesystem with elevation if needed
#[tauri::command]
pub async fn read_directory_elevated(
    device_id: String,
    path: String,
    filesystem: String,
    mount_points: Option<Vec<String>>,
) -> Result<DirectoryListing, String> {
    use crate::worker_server::{get_worker_server, WorkerCommand, WorkerResponse};
    
    log::info!("Attempting elevated read of directory {} on {} filesystem", path, filesystem);
    
    // Create device object
    let device = if let Some(mounts) = mount_points {
        let mount_paths: Vec<std::path::PathBuf> = mounts.into_iter().map(std::path::PathBuf::from).collect();
        moses_core::Device {
            id: device_id.clone(),
            name: String::new(),
            size: 0,
            device_type: moses_core::DeviceType::HardDisk,
            mount_points: mount_paths,
            filesystem: Some(filesystem.clone()),
            is_removable: false,
            is_system: false,
        }
    } else {
        // Enumerate to find the device
        use moses_core::DeviceManager;
        use moses_platform::PlatformDeviceManager;
        let manager = PlatformDeviceManager;
        let devices = manager.enumerate_devices().await
            .map_err(|e| format!("Failed to enumerate devices: {}", e))?;
        
        devices.into_iter()
            .find(|d| d.id == device_id)
            .ok_or_else(|| format!("Device {} not found", device_id))?
    };
    
    // Use the persistent socket-based worker
    let server = get_worker_server().await?;
    let mut server_guard = server.lock().await;
    
    if server_guard.is_none() {
        return Err("Worker server not initialized".to_string());
    }
    
    let worker = server_guard.as_mut().unwrap();
    
    // Execute the read directory command via the persistent worker
    let command = WorkerCommand::ReadDirectory {
        device: device.clone(),
        path: path.clone(),
    };
    
    let response = worker.execute_command(command).await?;
    
    match response {
        WorkerResponse::DirectoryListing(json) => {
            // Parse the JSON result
            let result: serde_json::Value = serde_json::from_str(&json)
                .map_err(|e| format!("Failed to parse directory listing: {}", e))?;
            
            if result["success"].as_bool().unwrap_or(false) {
                let entries = result["entries"].as_array()
                    .ok_or("Missing entries in result")?;
                
                // Convert to DirectoryListing
                let mut total_size = 0u64;
                let mut converted_entries = Vec::new();
                
                for entry in entries {
                    let name = entry["name"].as_str().unwrap_or("").to_string();
                    let is_directory = entry["is_directory"].as_bool().unwrap_or(false);
                    let size = entry["size"].as_u64().unwrap_or(0);
                    
                    if !is_directory {
                        total_size += size;
                    }
                    
                    converted_entries.push(DirectoryEntry {
                        name: name.clone(),
                        path: format!("{}/{}", path.trim_end_matches('/'), name),
                        entry_type: if is_directory { EntryType::Directory } else { EntryType::File },
                        size: if is_directory { None } else { Some(size) },
                        modified: None,
                        created: None,
                        permissions: None,
                        metadata: None,
                    });
                }
                
                Ok(DirectoryListing {
                    path: path.clone(),
                    entries: converted_entries,
                    total_size,
                    item_count: entries.len(),
                })
            } else {
                Err(result["error"].as_str().unwrap_or("Unknown error").to_string())
            }
        }
        WorkerResponse::Error(msg) => Err(msg),
        _ => Err("Unexpected response from worker".to_string()),
    }
}

/// Read directory contents from a filesystem
#[tauri::command]
pub async fn read_directory(
    device_id: String,
    path: String,
    filesystem: String,
    mount_points: Option<Vec<String>>,
) -> Result<DirectoryListing, String> {
    log::info!("Reading directory {} on {} filesystem (device: {})", 
              path, filesystem, device_id);
    
    // Log mount points for debugging
    if let Some(ref mounts) = mount_points {
        log::info!("Mount points provided: {:?}", mounts);
    } else {
        log::info!("No mount points provided, will enumerate device");
    }
    
    // Create a minimal device object with mount points for drive letter access
    // This avoids re-enumerating devices and triggering filesystem detection again
    let device = if let Some(mounts) = mount_points {
        let mount_paths: Vec<std::path::PathBuf> = mounts.into_iter().map(std::path::PathBuf::from).collect();
        log::info!("Created device with mount points: {:?}", mount_paths);
        Device {
            id: device_id.clone(),
            name: String::new(), // Not needed for reading
            size: 0,             // Not needed for reading
            device_type: moses_core::DeviceType::HardDisk,
            mount_points: mount_paths,
            is_removable: false,
            is_system: false,
            filesystem: Some(filesystem.clone()),
        }
    } else {
        // Fallback to the old way if mount points not provided
        let dev = get_device(&device_id)
            .ok_or_else(|| format!("Device {} not found", device_id))?;
        log::info!("Enumerated device, mount points: {:?}", dev.mount_points);
        dev
    };
    
    // Route to appropriate filesystem reader
    match filesystem.as_str() {
        "ext4" | "ext3" | "ext2" => {
            read_ext_directory(&device, &path, &filesystem).await
        },
        "fat16" => {
            read_fat16_directory(&device, &path).await
        },
        "fat32" | "vfat" => {
            read_fat32_directory(&device, &path).await
        },
        "ntfs" => {
            read_ntfs_directory(&device, &path).await
        },
        "exfat" => {
            read_exfat_directory(&device, &path).await
        },
        "unknown" => {
            // For unknown filesystems, we need admin rights to detect the type
            Err("Unable to detect filesystem type. Administrator privileges may be required to read unmounted drives.".to_string())
        },
        _ => {
            Err(format!("Reading {} filesystem not yet implemented", filesystem))
        }
    }
}

/// Read a file's contents from a filesystem
#[tauri::command]
pub async fn read_file(
    device_id: String,
    file_path: String,
    filesystem: String,
    offset: Option<u64>,
    length: Option<u64>,
) -> Result<Vec<u8>, String> {
    log::info!("Reading file {} from {} filesystem", file_path, filesystem);
    
    let device = get_device(&device_id)
        .ok_or_else(|| format!("Device {} not found", device_id))?;
    
    match filesystem.as_str() {
        "ext4" | "ext3" | "ext2" => {
            read_ext_file(&device, &file_path, offset, length).await
        },
        _ => {
            Err(format!("Reading files from {} not yet implemented", filesystem))
        }
    }
}

/// Copy files from one filesystem to another
#[tauri::command]
pub async fn copy_files(
    _source_device: String,
    _source_fs: String,
    _source_paths: Vec<String>,
    _dest_device: String,
    _dest_fs: String,
    _dest_path: String,
) -> Result<CopyResult, String> {
    log::info!("Copying {} files from {} to {}", 
              _source_paths.len(), _source_fs, _dest_fs);
    
    // This would orchestrate the cross-filesystem copy
    todo!("Implement cross-filesystem copy")
}

#[derive(Debug, Serialize)]
pub struct CopyResult {
    pub files_copied: usize,
    pub bytes_copied: u64,
    pub errors: Vec<String>,
}

// Filesystem-specific implementations
async fn read_ext_directory(
    device: &Device,
    path: &str,
    variant: &str,
) -> Result<DirectoryListing, String> {
    use moses_filesystems::ext4_native::ExtReader;
    
    log::info!("Reading {} directory: {} on device {}", variant, path, device.id);
    
    // Create ext reader
    let mut reader = ExtReader::new(device.clone())
        .map_err(|e| format!("Failed to open {} filesystem: {:?}", variant, e))?;
    
    // Read directory
    let entries = reader.read_directory(path)
        .map_err(|e| format!("Failed to read directory {}: {:?}", path, e))?;
    
    // Convert to our format
    let mut total_size = 0u64;
    let converted_entries: Vec<DirectoryEntry> = entries.into_iter().map(|entry| {
        // Only count size for files, not directories
        let size = if entry.entry_type == moses_filesystems::ext4_native::reader::FileType::Regular {
            // We'd need to get the actual size from the inode
            // For now, just return 0 as we don't have size in DirEntry
            Some(0u64)
        } else {
            None
        };
        
        if let Some(s) = size {
            total_size += s;
        }
        
        DirectoryEntry {
            name: entry.name.clone(),
            path: if path == "/" || path.is_empty() {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", path.trim_end_matches('/'), entry.name)
            },
            entry_type: match entry.entry_type {
                moses_filesystems::ext4_native::reader::FileType::Directory => EntryType::Directory,
                moses_filesystems::ext4_native::reader::FileType::Regular => EntryType::File,
                moses_filesystems::ext4_native::reader::FileType::Symlink => EntryType::Symlink,
                moses_filesystems::ext4_native::reader::FileType::CharDevice |
                moses_filesystems::ext4_native::reader::FileType::BlockDevice => EntryType::Device,
                _ => EntryType::Other,
            },
            size,
            modified: None, // TODO: Get from inode
            created: None,
            permissions: None,
            metadata: None,
        }
    }).collect();
    
    let item_count = converted_entries.len();
    Ok(DirectoryListing {
        path: path.to_string(),
        entries: converted_entries,
        total_size,
        item_count,
    })
}

async fn read_ext_file(
    _device: &Device,
    _path: &str,
    _offset: Option<u64>,
    _length: Option<u64>,
) -> Result<Vec<u8>, String> {
    #[cfg(target_os = "windows")]
    {
        // TODO: Implement ext4 file reader for Windows
        // This would use the moses_filesystems::Ext4NativeFormatter reader functionality
        Err("ext4 file reading not yet implemented on Windows".to_string())
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Err("ext4 reading not yet implemented on this platform".to_string())
    }
}

async fn read_fat16_directory(
    device: &Device,
    path: &str,
) -> Result<DirectoryListing, String> {
    use moses_filesystems::Fat16Reader;
    
    log::info!("Reading FAT16 directory: {} on device {}", path, device.id);
    
    // Create reader
    let mut reader = Fat16Reader::new(device.clone())
        .map_err(|e| format!("Failed to open FAT16 filesystem: {:?}", e))?;
    
    // Read directory
    let entries = reader.list_directory(path)
        .map_err(|e| format!("Failed to read directory {}: {:?}", path, e))?;
    
    // Convert to our format
    let mut total_size = 0u64;
    let converted_entries: Vec<DirectoryEntry> = entries.into_iter().map(|entry| {
        if !entry.is_directory {
            total_size += entry.size;
        }
        DirectoryEntry {
            name: entry.name.clone(),
            path: if path == "/" || path.is_empty() {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", path.trim_end_matches('/'), entry.name)
            },
            entry_type: if entry.is_directory {
                EntryType::Directory
            } else {
                EntryType::File
            },
            size: if entry.is_directory { None } else { Some(entry.size) },
            modified: None,
            created: None,
            permissions: None,
            metadata: None,
        }
    }).collect();
    
    let item_count = converted_entries.len();
    Ok(DirectoryListing {
        path: path.to_string(),
        entries: converted_entries,
        total_size,
        item_count,
    })
}

async fn read_fat32_directory(
    device: &Device,
    path: &str,
) -> Result<DirectoryListing, String> {
    use moses_filesystems::Fat32Reader;
    
    log::info!("Reading FAT32 directory: {} on device {}", path, device.id);
    
    // Create reader
    let mut reader = Fat32Reader::new(device.clone())
        .map_err(|e| format!("Failed to open FAT32 filesystem: {:?}", e))?;
    
    // Read directory
    let entries = reader.list_directory(path)
        .map_err(|e| format!("Failed to read directory {}: {:?}", path, e))?;
    
    // Convert to our format
    let mut total_size = 0u64;
    let converted_entries: Vec<DirectoryEntry> = entries.into_iter().map(|entry| {
        if !entry.is_directory {
            total_size += entry.size;
        }
        DirectoryEntry {
            name: entry.name.clone(),
            path: if path == "/" || path.is_empty() {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", path.trim_end_matches('/'), entry.name)
            },
            entry_type: if entry.is_directory {
                EntryType::Directory
            } else {
                EntryType::File
            },
            size: if entry.is_directory { None } else { Some(entry.size) },
            modified: None, // TODO: Parse FAT32 timestamps
            created: None,
            permissions: None,
            metadata: None,
        }
    }).collect();
    
    let item_count = converted_entries.len();
    Ok(DirectoryListing {
        path: path.to_string(),
        entries: converted_entries,
        total_size,
        item_count,
    })
}

async fn read_ntfs_directory(
    device: &Device,
    path: &str,
) -> Result<DirectoryListing, String> {
    use moses_filesystems::ntfs::NtfsReader;
    
    log::info!("Reading NTFS directory: {} on device {}", path, device.id);
    
    // Create NTFS reader
    let mut reader = NtfsReader::new(device.clone())
        .map_err(|e| format!("Failed to open NTFS filesystem: {:?}", e))?;
    
    // Read directory
    let entries = reader.list_directory(path)
        .map_err(|e| format!("Failed to read directory {}: {:?}", path, e))?;
    
    // Convert to our format
    let mut total_size = 0u64;
    let converted_entries: Vec<DirectoryEntry> = entries.into_iter().map(|entry| {
        if !entry.is_directory {
            total_size += entry.size;
        }
        DirectoryEntry {
            name: entry.name.clone(),
            path: if path == "/" || path.is_empty() {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", path.trim_end_matches('/'), entry.name)
            },
            entry_type: if entry.is_directory {
                EntryType::Directory
            } else {
                EntryType::File
            },
            size: if entry.is_directory { None } else { Some(entry.size) },
            modified: None, // TODO: Parse NTFS timestamps
            created: None,
            permissions: None,
            metadata: None,
        }
    }).collect();
    
    let item_count = converted_entries.len();
    Ok(DirectoryListing {
        path: path.to_string(),
        entries: converted_entries,
        total_size,
        item_count,
    })
}

async fn read_exfat_directory(
    device: &Device,
    path: &str,
) -> Result<DirectoryListing, String> {
    use moses_filesystems::ExFatReader;
    
    // Create reader
    let mut reader = ExFatReader::new(device.clone())
        .map_err(|e| format!("Failed to open exFAT filesystem: {:?}", e))?;
    
    // Read directory using the common trait method
    let entries = reader.list_directory(path)
        .map_err(|e| format!("Failed to read directory {}: {:?}", path, e))?;
    
    // Convert to our format
    let mut total_size = 0u64;
    let converted_entries: Vec<DirectoryEntry> = entries.into_iter().map(|entry| {
        if !entry.is_directory {
            total_size += entry.size;
        }
        DirectoryEntry {
            name: entry.name.clone(),
            path: if path == "/" || path.is_empty() {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", path.trim_end_matches('/'), entry.name)
            },
            entry_type: if entry.is_directory {
                EntryType::Directory
            } else {
                EntryType::File
            },
            size: if entry.is_directory { None } else { Some(entry.size) },
            modified: None, // TODO: Parse timestamps
            created: None,
            permissions: None,
            metadata: None,
        }
    }).collect();
    
    let item_count = converted_entries.len();
    Ok(DirectoryListing {
        path: path.to_string(),
        entries: converted_entries,
        total_size,
        item_count,
    })
}

/// Detect filesystem type for a device (may require elevation)
#[tauri::command]
pub async fn detect_filesystem_elevated(
    device_id: String,
) -> Result<String, String> {
    // Check cache first
    if let Ok(cache) = FILESYSTEM_CACHE.lock() {
        if let Some(fs_type) = cache.get(&device_id) {
            log::info!("Using cached filesystem type for {}: {}", device_id, fs_type);
            return Ok(fs_type.clone());
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        use moses_platform::windows::elevation::is_elevated;
        
        if !is_elevated() {
            // Return a special error code that the frontend will recognize
            return Err("ELEVATION_REQUIRED".to_string());
        }
        
        // Try to detect filesystem with elevated privileges
        use std::fs::File;
        
        let mut file = File::open(&device_id)
            .map_err(|e| format!("Failed to open device {}: {}", device_id, e))?;
        
        // Use the unified detection system
        let fs_type = moses_filesystems::detection::detect_filesystem(&mut file)
            .map_err(|e| format!("Failed to detect filesystem: {:?}", e))?;
        
        // Cache the result
        if let Ok(mut cache) = FILESYSTEM_CACHE.lock() {
            cache.insert(device_id.clone(), fs_type.to_string());
        }
        
        Ok(fs_type.to_string())
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // On Linux/Mac, use native tools
        Err("Filesystem detection not yet implemented for this platform".to_string())
    }
}

/// Request filesystem detection with elevation (triggers UAC)
#[tauri::command]
pub async fn request_elevated_filesystem_detection(
    device_id: String,
) -> Result<String, String> {
    // This is a bit tricky - we need to somehow get elevated access
    // The best approach would be to restart the whole app elevated, 
    // but for now let's try a workaround
    
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        use std::os::windows::process::CommandExt;
        
        // First, let's just try using our unified detection directly
        // in case we somehow have access
        use std::fs::File;
        
        if let Ok(mut file) = File::open(&device_id) {
            match moses_filesystems::detection::detect_filesystem(&mut file) {
                Ok(fs_type) => {
                    // Cache the result
                    if let Ok(mut cache) = FILESYSTEM_CACHE.lock() {
                        cache.insert(device_id.clone(), fs_type.clone());
                    }
                    return Ok(fs_type);
                }
                Err(_) => {
                    // Fall through to elevation attempt
                }
            }
        }
        
        // We need elevation. The problem is that elevated processes can't easily
        // communicate back to non-elevated ones. 
        // For now, we'll create a simple elevated PowerShell that writes to a temp file
        
        let temp_file = std::env::temp_dir().join(format!("moses_detect_{}.txt", std::process::id()));
        let temp_path = temp_file.to_string_lossy().to_string();
        
        // Create a very simple script that just tries to open the device with elevation
        // and writes "success" if it works
        let ps_script = format!(r#"
            try {{
                $stream = [System.IO.File]::OpenRead('{}')
                $stream.Close()
                'success' | Out-File -FilePath '{}' -Encoding ASCII -NoNewline
            }} catch {{
                'failed' | Out-File -FilePath '{}' -Encoding ASCII -NoNewline
            }}
        "#, device_id, temp_path, temp_path);
        
        // Request elevation
        let elevated_command = format!(
            "Start-Process powershell -ArgumentList '-NoProfile', '-Command', '{}' -Verb RunAs -Wait",
            ps_script.replace('\'', "''").replace('"', "`\"").replace('\n', " ")
        );
        
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let _output = Command::new("powershell")
            .args(&["-NoProfile", "-Command", &elevated_command])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("Failed to request elevation: {}", e))?;
        
        // Check result
        std::thread::sleep(std::time::Duration::from_millis(1000));
        
        if let Ok(result) = std::fs::read_to_string(&temp_file) {
            let _ = std::fs::remove_file(&temp_file);
            
            if result == "success" {
                // Elevation worked, but we still can't detect from here
                // The user would need to restart the app elevated
                return Err("Elevation successful but detection requires restarting the application with administrator privileges".to_string());
            }
        }
        
        Err("Failed to detect filesystem. Please run the application as administrator.".to_string())
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        let _ = device_id; // Unused on non-Windows platforms
        Err("Filesystem detection not implemented for this platform".to_string())
    }
}

/// Get filesystem type quickly (returns short string like "ntfs", "gpt-empty", etc.)
#[tauri::command]
pub async fn get_filesystem_type(
    device_id: String,
) -> Result<String, String> {
    log::info!("Getting filesystem type for device: {}", device_id);
    
    let device = get_device(&device_id)
        .ok_or_else(|| format!("Device {} not found", device_id))?;
    
    match moses_filesystems::diagnostics::get_filesystem_type(&device) {
        Ok(fs_type) => {
            log::info!("Detected filesystem type: {}", fs_type);
            Ok(fs_type)
        }
        Err(e) => {
            log::error!("Failed to get filesystem type: {:?}", e);
            Err(format!("Failed to detect filesystem type: {:?}", e))
        }
    }
}

/// Analyze an unknown filesystem and return diagnostic information
#[tauri::command]
pub async fn analyze_filesystem(
    device_id: String,
) -> Result<String, String> {
    log::info!("Analyzing filesystem on device: {}", device_id);
    
    // Check if we're on Windows and need elevation
    #[cfg(target_os = "windows")]
    {
        use moses_platform::windows::elevation::is_elevated;
        
        // First try without elevation (in case we already have admin rights)
        let device = get_device(&device_id)
            .ok_or_else(|| format!("Device {} not found", device_id))?;
        
        // Try the analysis
        match analyze_unknown_filesystem(&device) {
            Ok(report) => {
                log::info!("Filesystem analysis completed successfully");
                
                // Cache the result
                cache_analysis_result(&device_id, &report);
                
                return Ok(report);
            }
            Err(e) => {
                // Check if it's an access denied error
                let error_str = format!("{:?}", e);
                if error_str.contains("os error 5") || error_str.contains("Access is denied") {
                    log::info!("Analysis requires elevation, checking admin status");
                    
                    if !is_elevated() {
                        // Return special error that UI can handle
                        return Err("ELEVATION_REQUIRED".to_string());
                    }
                }
                
                log::error!("Failed to analyze filesystem: {:?}", e);
                return Err(format!("Failed to analyze filesystem: {:?}", e));
            }
        }
    }
    
    // Non-Windows platforms
    #[cfg(not(target_os = "windows"))]
    {
        let device = get_device(&device_id)
            .ok_or_else(|| format!("Device {} not found", device_id))?;
        
        match analyze_unknown_filesystem(&device) {
            Ok(report) => {
                log::info!("Filesystem analysis completed successfully");
                
                // Cache the result
                cache_analysis_result(&device_id, &report);
                
                Ok(report)
            }
            Err(e) => {
                log::error!("Failed to analyze filesystem: {:?}", e);
                Err(format!("Failed to analyze filesystem: {:?}", e))
            }
        }
    }
}

/// Analyze filesystem with elevation (Windows only)
#[tauri::command]
pub async fn analyze_filesystem_elevated(
    device_id: String,
) -> Result<String, String> {
    log::info!("Requesting elevated analysis for device: {}", device_id);
    
    #[cfg(target_os = "windows")]
    {
        use crate::worker_server::{get_worker_server, WorkerCommand, WorkerResponse};
        
        // Get device info
        let device = get_device(&device_id)
            .ok_or_else(|| format!("Device {} not found", device_id))?;
        
        // Use the persistent socket-based worker
        let server = get_worker_server().await?;
        let mut server_guard = server.lock().await;
        
        if let Some(worker) = server_guard.as_mut() {
            // Send analyze command through the socket
            let command = WorkerCommand::Analyze { device: device.clone() };
            
            match worker.execute_command(command).await {
                Ok(WorkerResponse::Success(result)) => {
                    // Cache the result
                    cache_analysis_result(&device_id, &result);
                    Ok(result)
                }
                Ok(WorkerResponse::Error(e)) => {
                    Err(format!("Analysis failed: {}", e))
                }
                Ok(_) => {
                    Err("Unexpected response from worker".to_string())
                }
                Err(e) => {
                    Err(format!("Worker communication failed: {}", e))
                }
            }
        } else {
            return Err("Worker server not initialized".to_string());
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // On non-Windows, just call the regular analyze
        analyze_filesystem(device_id).await
    }
}

/// Cache the analysis result
fn cache_analysis_result(device_id: &str, report_json: &str) {
    // Try to parse the JSON report to extract filesystem info
    if let Ok(report) = serde_json::from_str::<serde_json::Value>(report_json) {
        let filesystem = report["filesystem"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        
        let partition_table = report["partition_table"]
            .as_str()
            .map(|s| s.to_string());
        
        let partitions = if let Some(parts) = report["partitions"].as_array() {
            parts.iter().map(|p| {
                filesystem_cache::PartitionInfo {
                    number: p["number"].as_u64().unwrap_or(0) as u32,
                    filesystem: p["filesystem"].as_str().map(|s| s.to_string()),
                    size: p["size"].as_u64().unwrap_or(0),
                    start_offset: p["start_offset"].as_u64().unwrap_or(0),
                }
            }).collect()
        } else {
            vec![]
        };
        
        let cached_info = filesystem_cache::CachedFilesystemInfo {
            filesystem,
            partition_table,
            partitions,
            detected_at: std::time::SystemTime::now(),
        };
        
        filesystem_cache::cache_filesystem_info(device_id, cached_info);
    }
}

fn get_device(device_id: &str) -> Option<Device> {
    use moses_platform::PlatformDeviceManager;
    use moses_core::DeviceManager;
    use futures::executor::block_on;
    
    // Get single device by ID instead of enumerating all devices
    log::debug!("Looking up device: {}", device_id);
    
    #[cfg(target_os = "windows")]
    let manager = PlatformDeviceManager;
    
    #[cfg(target_os = "linux")]
    let manager = PlatformDeviceManager;
    
    #[cfg(target_os = "macos")]
    let manager = PlatformDeviceManager;
    
    // Use the new get_device_by_id method to avoid enumerating all devices
    block_on(manager.get_device_by_id(device_id)).ok().flatten()
}