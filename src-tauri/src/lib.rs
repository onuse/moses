use moses_core::{Device, DeviceManager, FilesystemFormatter, FormatOptions, SimulationReport};

use moses_platform::PlatformDeviceManager;
use moses_formatters::{NtfsFormatter, Fat32Formatter, ExFatFormatter};

#[cfg(target_os = "linux")]
use moses_formatters::Ext4LinuxFormatter;

#[cfg(target_os = "windows")]
use moses_formatters::Ext4WindowsFormatter;

#[tauri::command]
async fn enumerate_devices() -> Result<Vec<Device>, String> {
    let manager = PlatformDeviceManager;
    manager.enumerate_devices()
        .await
        .map_err(|e| format!("Failed to enumerate devices: {}", e))
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
                let formatter = Ext4WindowsFormatter;
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
            let formatter = NtfsFormatter;
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
                let formatter = Ext4WindowsFormatter;
                
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
                
                Ok(format!("Successfully formatted {} as EXT4 via WSL2", device.name))
            }
            
            #[cfg(target_os = "macos")]
            {
                Err("EXT4 formatting not yet implemented on macOS".to_string())
            }
        },
        
        "ntfs" => {
            let formatter = NtfsFormatter;
            
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
            
            Ok(format!("Successfully formatted {} as NTFS", device.name))
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
}

#[tauri::command]
async fn check_formatter_requirements(filesystem_type: String) -> Result<Vec<String>, String> {
    // Check what tools are required for each filesystem
    let mut missing_tools = Vec::new();
    
    match filesystem_type.as_str() {
        "ext4" => {
            #[cfg(target_os = "windows")]
            {
                // Check for WSL2
                let output = std::process::Command::new("wsl")
                    .arg("--list")
                    .output();
                
                if output.is_err() || !output.unwrap().status.success() {
                    missing_tools.push("WSL2 (Windows Subsystem for Linux)".to_string());
                }
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
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            enumerate_devices,
            simulate_format,
            execute_format,
            check_formatter_requirements
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}