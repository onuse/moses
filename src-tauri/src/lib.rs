use moses_core::{Device, DeviceManager, FilesystemFormatter, FormatOptions, SimulationReport};

use moses_platform::PlatformDeviceManager;
use moses_formatters::{NtfsFormatter, Fat32Formatter, ExFatFormatter};

#[cfg(target_os = "windows")]
use moses_platform::windows::elevation::is_elevated;

mod logging;

#[cfg(target_os = "linux")]
use moses_formatters::Ext4LinuxFormatter;

#[cfg(target_os = "windows")]
use moses_formatters::Ext4NativeFormatter;

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
        use std::process::Command;
        use std::env;
        
        // Get the path to the elevated worker executable
        let worker_exe = env::current_exe()
            .map_err(|e| format!("Failed to get executable path: {}", e))?
            .parent()
            .ok_or_else(|| "Failed to get executable directory".to_string())?
            .join("moses-formatter.exe");
        
        // Serialize device and options to JSON
        let device_json = serde_json::to_string(&device)
            .map_err(|e| format!("Failed to serialize device: {}", e))?;
        let options_json = serde_json::to_string(&options)
            .map_err(|e| format!("Failed to serialize options: {}", e))?;
        
        // Log what we're passing to the worker
        log::info!("Passing to elevated worker - Device: name={}, id={}, size={}", 
                   device.name, device.id, device.size);
        log::info!("Options: filesystem={}, cluster_size={:?}", 
                   options.filesystem_type, options.cluster_size);
        
        // If we're already elevated, run the worker directly
        if is_elevated() {
            let output = Command::new(&worker_exe)
                .arg(&device_json)
                .arg(&options_json)
                .output()
                .map_err(|e| format!("Failed to run worker: {}", e))?;
            
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                Ok(stdout.trim().to_string())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(stderr.trim().to_string())
            }
        } else {
            // Request elevation for the worker process
            // Write JSON to temp files to avoid escaping issues
            use std::fs;
            use std::os::windows::process::CommandExt;
            
            let temp_dir = env::temp_dir();
            let device_file = temp_dir.join(format!("moses_device_{}.json", std::process::id()));
            let options_file = temp_dir.join(format!("moses_options_{}.json", std::process::id()));
            let output_file = temp_dir.join(format!("moses_output_{}.json", std::process::id()));
            
            // Write JSON to temp files
            fs::write(&device_file, &device_json)
                .map_err(|e| format!("Failed to write device JSON to temp file: {}", e))?;
            fs::write(&options_file, &options_json)
                .map_err(|e| format!("Failed to write options JSON to temp file: {}", e))?;
            
            let ps_script = format!(
                r#"
                $worker = '{}'
                $deviceFile = '{}'
                $optionsFile = '{}'
                
                # Start the worker with elevation
                $startInfo = New-Object System.Diagnostics.ProcessStartInfo
                $startInfo.FileName = $worker
                $startInfo.Arguments = "`"$deviceFile`" `"$optionsFile`""
                $startInfo.Verb = 'runas'
                $startInfo.UseShellExecute = $true
                $startInfo.RedirectStandardOutput = $false
                $startInfo.RedirectStandardError = $false
                
                try {{
                    $process = [System.Diagnostics.Process]::Start($startInfo)
                    $process.WaitForExit()
                    
                    # Clean up temp files
                    Remove-Item -Path $deviceFile -ErrorAction SilentlyContinue
                    Remove-Item -Path $optionsFile -ErrorAction SilentlyContinue
                    
                    if ($process.ExitCode -eq 0) {{
                        Write-Output "Format completed successfully"
                        exit 0
                    }} else {{
                        Write-Error "Format failed with exit code: $($process.ExitCode)"
                        exit 1
                    }}
                }} catch {{
                    Write-Error "Failed to start elevated worker: $_"
                    # Clean up temp files on error
                    Remove-Item -Path $deviceFile -ErrorAction SilentlyContinue
                    Remove-Item -Path $optionsFile -ErrorAction SilentlyContinue
                    exit 1
                }}
                "#,
                worker_exe.display(),
                device_file.display(),
                options_file.display()
            );
            
            let output = Command::new("powershell")
                .args(&[
                    "-NoProfile",
                    "-ExecutionPolicy", "Bypass",
                    "-Command", &ps_script
                ])
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .output()
                .map_err(|e| format!("Failed to run PowerShell: {}", e))?;
            
            if output.status.success() {
                Ok("Format completed successfully".to_string())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("Format failed: {}", stderr.trim()))
            }
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
            
            // Note: We're not using tauri_plugin_log anymore since we have our own logger
            // that bridges the standard log crate to the UI console
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            check_elevation_status,
            enumerate_devices,
            simulate_format,
            execute_format,
            execute_format_elevated,
            check_formatter_requirements
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}