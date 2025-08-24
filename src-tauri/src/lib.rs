use moses_core::{Device, DeviceManager, FilesystemFormatter, FormatOptions, SimulationReport};

use moses_platform::PlatformDeviceManager;
use moses_filesystems::{Fat16Formatter, Fat32Formatter, ExFatFormatter};

#[cfg(target_os = "windows")]
use moses_platform::windows::elevation::is_elevated;

mod logging;
pub mod commands;
mod filesystem_cache;
mod worker_server;

#[cfg(target_os = "linux")]
use moses_filesystems::Ext4LinuxFormatter;

#[cfg(target_os = "windows")]
use moses_filesystems::Ext4NativeFormatter;

#[tauri::command]
async fn detect_drives() -> Result<Vec<Device>, String> {
    #[cfg(target_os = "windows")]
    let manager = PlatformDeviceManager;
    
    #[cfg(target_os = "linux")]
    let manager = PlatformDeviceManager;
    
    #[cfg(target_os = "macos")]
    let manager = PlatformDeviceManager;
    
    manager.enumerate_devices()
        .await
        .map_err(|e| format!("Failed to detect drives: {}", e))
}

#[tauri::command]
async fn check_elevation_status() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        Ok(is_elevated())
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // On Linux/macOS, check if we have necessary permissions
        // This is a simplified check - you might want more sophisticated logic
        Ok(unsafe { libc::geteuid() } == 0)
    }
}

#[tauri::command]
async fn execute_format_elevated(
    device: Device,
    options: FormatOptions,
) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use crate::worker_server::{get_worker_server, WorkerCommand, WorkerResponse};
        
        log::info!("Executing format with elevation - Device: name={}, id={}, size={}", 
                   device.name, device.id, device.size);
        log::info!("Options: filesystem={}, cluster_size={:?}", 
                   options.filesystem_type, options.cluster_size);
        
        // Use the persistent socket-based worker
        let server = get_worker_server().await?;
        let mut server_guard = server.lock().await;
        
        if let Some(worker) = server_guard.as_mut() {
            // Send format command through the socket
            let command = WorkerCommand::Format { 
                device: device.clone(), 
                options: options.clone() 
            };
            
            match worker.execute_command(command).await {
                Ok(WorkerResponse::Success(result)) => Ok(result),
                Ok(WorkerResponse::Error(e)) => Err(format!("Format failed: {}", e)),
                Ok(_) => Err("Unexpected response from worker".to_string()),
                Err(e) => Err(format!("Worker communication failed: {}", e))
            }
        } else {
            Err("Worker server not initialized".to_string())
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // On non-Windows platforms, use sudo or pkexec
        execute_format(device, options).await
    }
}

#[tauri::command]
async fn enumerate_devices() -> Result<Vec<Device>, String> {
    let manager = PlatformDeviceManager;
    let mut devices = manager.enumerate_devices()
        .await
        .map_err(|e| format!("Failed to enumerate devices: {}", e))?;
    
    // Check cache for any devices that don't have filesystem info
    for device in &mut devices {
        if device.filesystem.is_none() || device.filesystem.as_deref() == Some("unknown") {
            // First check our in-memory cache (from recent format operations)
            use commands::filesystem::FILESYSTEM_CACHE;
            if let Ok(cache) = FILESYSTEM_CACHE.lock() {
                if let Some(fs_type) = cache.get(&device.id) {
                    log::info!("Using in-memory cached filesystem type for {}: {}", device.id, fs_type);
                    device.filesystem = Some(fs_type.clone());
                    continue;
                }
            }
            
            // Then check the persistent filesystem cache
            if let Some(cached_info) = filesystem_cache::get_cached_filesystem_info(&device.id) {
                if filesystem_cache::is_cache_fresh(&cached_info) {
                    log::info!("Using cached filesystem info for {}: {}", device.id, cached_info.filesystem);
                    device.filesystem = Some(cached_info.filesystem);
                } else {
                    log::debug!("Cached filesystem info for {} is stale", device.id);
                }
            }
        }
    }
    
    // Log detected devices and their filesystems
    for device in &devices {
        log::info!("Device: {} ({}), Size: {}, Filesystem: {:?}", 
                  device.name, device.id, device.size, device.filesystem);
    }
    
    Ok(devices)
}

#[tauri::command]
async fn simulate_format(
    device: Device,
    options: FormatOptions,
) -> Result<SimulationReport, String> {
    // Select the appropriate formatter based on filesystem type
    match options.filesystem_type.as_str() {
        "ext4" => {
            #[cfg(target_os = "linux")]
            {
                let formatter = Ext4LinuxFormatter;
                formatter.dry_run(&device, &options)
                    .await
                    .map_err(|e| format!("Simulation failed: {}", e))
            }
            
            #[cfg(target_os = "windows")]
            {
                let formatter = Ext4NativeFormatter;
                formatter.dry_run(&device, &options)
                    .await
                    .map_err(|e| format!("Simulation failed: {}", e))
            }
            
            #[cfg(target_os = "macos")]
            {
                Err("EXT4 formatting not yet implemented on macOS".to_string())
            }
        },
        
        "ntfs" => {
            // NTFS formatting not yet implemented
            return Err("NTFS formatting is not yet implemented. Only NTFS reading is currently supported.".to_string());
        },
        
        "fat16" => {
            let formatter = Fat16Formatter;
            formatter.dry_run(&device, &options)
                .await
                .map_err(|e| format!("Simulation failed: {}", e))
        },
        
        "fat32" => {
            let formatter = Fat32Formatter;
            formatter.dry_run(&device, &options)
                .await
                .map_err(|e| format!("Simulation failed: {}", e))
        },
        
        "exfat" => {
            let formatter = ExFatFormatter;
            formatter.dry_run(&device, &options)
                .await
                .map_err(|e| format!("Simulation failed: {}", e))
        },
        
        _ => {
            Err(format!("Unsupported filesystem type: {}", options.filesystem_type))
        }
    }
}

#[tauri::command]
async fn execute_format(
    device: Device,
    options: FormatOptions,
) -> Result<String, String> {
    // On Windows, use the elevated worker approach
    #[cfg(target_os = "windows")]
    {
        return execute_format_elevated(device, options).await;
    }
    
    // For non-Windows platforms, continue with the original implementation
    #[cfg(not(target_os = "windows"))]
    {
        // Safety check - never format system drives
        if device.is_system {
            return Err("Cannot format system drive. This would make your system unbootable!".to_string());
        }
        
        // Additional safety check for critical mount points
        for mount in &device.mount_points {
            let mount_str = mount.to_string_lossy().to_lowercase();
            if mount_str == "/" || 
               mount_str == "c:\\" || 
               mount_str.starts_with("/boot") ||
               mount_str.starts_with("/system") ||
               mount_str.starts_with("c:\\windows") {
                return Err(format!("Cannot format drive with critical mount point: {}", mount_str));
            }
        }
    
    // Select and execute the appropriate formatter
    match options.filesystem_type.as_str() {
        "ext2" => {
            #[cfg(target_os = "windows")]
            {
                let formatter = Ext2Formatter;
                
                formatter.validate_options(&options)
                    .await
                    .map_err(|e| format!("Invalid options: {}", e))?;
                
                if !formatter.can_format(&device) {
                    return Err("Device cannot be formatted (mounted or system device)".to_string());
                }
                
                formatter.format(&device, &options)
                    .await
                    .map_err(|e| format!("Format failed: {}", e))?;
                
                Ok(format!("Successfully formatted {} as ext2", device.name))
            }
            
            #[cfg(not(target_os = "windows"))]
            {
                Err("ext2 formatting not yet implemented on this platform".to_string())
            }
        },
        
        "ext3" => {
            #[cfg(target_os = "windows")]
            {
                let formatter = Ext3Formatter;
                
                formatter.validate_options(&options)
                    .await
                    .map_err(|e| format!("Invalid options: {}", e))?;
                
                if !formatter.can_format(&device) {
                    return Err("Device cannot be formatted (mounted or system device)".to_string());
                }
                
                formatter.format(&device, &options)
                    .await
                    .map_err(|e| format!("Format failed: {}", e))?;
                
                Ok(format!("Successfully formatted {} as ext3", device.name))
            }
            
            #[cfg(not(target_os = "windows"))]
            {
                Err("ext3 formatting not yet implemented on this platform".to_string())
            }
        },
        
        "ext4" => {
            #[cfg(target_os = "linux")]
            {
                let formatter = Ext4LinuxFormatter;
                
                // Validate options
                formatter.validate_options(&options)
                    .await
                    .map_err(|e| format!("Invalid options: {}", e))?;
                
                // Check if device can be formatted
                if !formatter.can_format(&device) {
                    return Err("Device cannot be formatted (mounted or system device)".to_string());
                }
                
                // Execute the format
                formatter.format(&device, &options)
                    .await
                    .map_err(|e| format!("Format failed: {}", e))?;
                
                Ok(format!("Successfully formatted {} as EXT4", device.name))
            }
            
            #[cfg(target_os = "windows")]
            {
                let formatter = Ext4NativeFormatter;
                
                // Validate options
                formatter.validate_options(&options)
                    .await
                    .map_err(|e| format!("Invalid options: {}", e))?;
                
                // Check if device can be formatted
                if !formatter.can_format(&device) {
                    return Err("Device cannot be formatted (mounted or system device)".to_string());
                }
                
                // Execute the format
                formatter.format(&device, &options)
                    .await
                    .map_err(|e| format!("Format failed: {}", e))?;
                
                Ok(format!("Successfully formatted {} as EXT4", device.name))
            }
            
            #[cfg(target_os = "macos")]
            {
                Err("EXT4 formatting not yet implemented on macOS".to_string())
            }
        },
        
        "ntfs" => {
            // NTFS formatting not yet implemented
            return Err("NTFS formatting is not yet implemented. Only NTFS reading is currently supported.".to_string());
        },
        
        "fat16" => {
            let formatter = Fat16Formatter;
            
            // Validate options
            formatter.validate_options(&options)
                .await
                .map_err(|e| format!("Invalid options: {}", e))?;
            
            // Check if device can be formatted
            if !formatter.can_format(&device) {
                return Err("Device cannot be formatted (system device or too large for FAT16)".to_string());
            }
            
            // Check size limit
            if device.size > 4 * 1024_u64.pow(3) {
                return Err("Device too large for FAT16. Maximum size is 4GB. Consider using FAT32 or exFAT.".to_string());
            }
            
            // Execute the format
            formatter.format(&device, &options)
                .await
                .map_err(|e| format!("Format failed: {}", e))?;
            
            Ok(format!("Successfully formatted {} as FAT16", device.name))
        },
        
        "fat32" => {
            let formatter = Fat32Formatter;
            
            // Validate options
            formatter.validate_options(&options)
                .await
                .map_err(|e| format!("Invalid options: {}", e))?;
            
            // Check if device can be formatted
            if !formatter.can_format(&device) {
                return Err("Device cannot be formatted (system device, critical mount points, or too large for FAT32)".to_string());
            }
            
            // Check size limit
            if device.size > 2 * 1024_u64.pow(4) {
                return Err("Device too large for FAT32. Maximum size is 2TB. Consider using exFAT or NTFS.".to_string());
            }
            
            // Warn about Windows 32GB limitation
            #[cfg(target_os = "windows")]
            {
                if device.size > 32 * 1024_u64.pow(3) {
                    eprintln!("Warning: Windows limits FAT32 formatting to 32GB. Format may fail for larger drives.");
                }
            }
            
            // Execute the format
            formatter.format(&device, &options)
                .await
                .map_err(|e| format!("Format failed: {}", e))?;
            
            Ok(format!("Successfully formatted {} as FAT32", device.name))
        },
        
        "exfat" => {
            let formatter = ExFatFormatter;
            
            // Validate options
            formatter.validate_options(&options)
                .await
                .map_err(|e| format!("Invalid options: {}", e))?;
            
            // Check if device can be formatted
            if !formatter.can_format(&device) {
                return Err("Device cannot be formatted (system device or critical mount points)".to_string());
            }
            
            // Execute the format
            formatter.format(&device, &options)
                .await
                .map_err(|e| format!("Format failed: {}", e))?;
            
            Ok(format!("Successfully formatted {} as exFAT", device.name))
        },
        
        _ => {
            Err(format!("Unsupported filesystem type: {}", options.filesystem_type))
        }
    }
    } // End of cfg(not(target_os = "windows")) block
}

#[tauri::command]
async fn check_formatter_requirements(filesystem_type: String) -> Result<Vec<String>, String> {
    // Check what tools are required for each filesystem
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let mut missing_tools = Vec::new();
    
    #[cfg(target_os = "windows")]
    let missing_tools = Vec::new();
    
    match filesystem_type.as_str() {
        "ext4" => {
            #[cfg(target_os = "windows")]
            {
                // Native ext4 support - no external tools required
            }
            
            #[cfg(target_os = "linux")]
            {
                // Check for mkfs.ext4
                let output = std::process::Command::new("which")
                    .arg("mkfs.ext4")
                    .output();
                
                if output.is_err() || !output.unwrap().status.success() {
                    missing_tools.push("e2fsprogs (mkfs.ext4)".to_string());
                }
            }
        },
        
        "ntfs" => {
            #[cfg(target_os = "linux")]
            {
                // Check for mkfs.ntfs
                let output = std::process::Command::new("which")
                    .arg("mkfs.ntfs")
                    .output();
                
                if output.is_err() || !output.unwrap().status.success() {
                    missing_tools.push("ntfs-3g (mkfs.ntfs)".to_string());
                }
            }
            
            #[cfg(target_os = "macos")]
            {
                // Check for ntfs-3g via Homebrew
                let output = std::process::Command::new("which")
                    .arg("mkfs.ntfs")
                    .output();
                
                if output.is_err() || !output.unwrap().status.success() {
                    missing_tools.push("ntfs-3g-mac (install with: brew install ntfs-3g-mac)".to_string());
                }
            }
        },
        
        "fat32" => {
            #[cfg(target_os = "linux")]
            {
                // Check for mkfs.fat
                let output = std::process::Command::new("which")
                    .arg("mkfs.fat")
                    .output();
                
                if output.is_err() || !output.unwrap().status.success() {
                    missing_tools.push("dosfstools (mkfs.fat)".to_string());
                }
            }
        },
        
        "exfat" => {
            #[cfg(target_os = "linux")]
            {
                // Check for mkfs.exfat
                let output = std::process::Command::new("which")
                    .arg("mkfs.exfat")
                    .output();
                
                if output.is_err() || !output.unwrap().status.success() {
                    missing_tools.push("exfatprogs or exfat-utils (mkfs.exfat)".to_string());
                }
            }
        },
        
        _ => {}
    }
    
    Ok(missing_tools)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Initialize our custom logger that sends logs to the UI
            logging::init_logger(app.handle().clone());
            
            // Initialize the worker server for socket-based operations
            tauri::async_runtime::spawn(async move {
                if let Err(e) = worker_server::init_worker_server().await {
                    log::error!("Failed to initialize worker server: {}", e);
                }
            });
            
            // Note: We're not using tauri_plugin_log anymore since we have our own logger
            // that bridges the standard log crate to the UI console
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            check_elevation_status,
            detect_drives,
            enumerate_devices,
            simulate_format,
            execute_format,
            execute_format_elevated,
            check_formatter_requirements,
            commands::filesystem::read_directory,
            commands::filesystem::read_directory_elevated,
            commands::filesystem::read_file,
            commands::filesystem::copy_files,
            // Old disk management commands (to be deprecated)
            commands::disk_management::clean_disk,
            commands::disk_management::detect_conflicts,
            commands::disk_management::convert_partition_style,
            commands::disk_management::prepare_disk,
            commands::disk_management::quick_clean,
            commands::disk_management::needs_cleaning,
            // Socket-based commands (preferred)
            commands::disk_management_socket::clean_disk_socket,
            commands::disk_management_socket::format_disk_socket,
            commands::disk_management_socket::detect_conflicts_socket,
            commands::disk_management_socket::analyze_filesystem_socket,
            commands::disk_management_socket::detect_filesystem_socket,
            commands::disk_management_socket::convert_partition_style_socket,
            commands::disk_management_socket::prepare_disk_socket,
            commands::filesystem::detect_filesystem_elevated,
            commands::filesystem::request_elevated_filesystem_detection,
            commands::filesystem::get_filesystem_type,
            commands::filesystem::analyze_filesystem,
            commands::filesystem::analyze_filesystem_elevated
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}