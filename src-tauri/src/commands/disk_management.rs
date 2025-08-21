// Tauri commands for disk management operations
use moses_core::{Device, DeviceManager};
use moses_formatters::disk_manager::{
    CleanOptions, WipeMethod,
    ConflictDetector, ConflictReport
};
use moses_platform::PlatformDeviceManager;

#[cfg(not(target_os = "windows"))]
use moses_formatters::disk_manager::{
    DiskManager, DiskCleaner,
    PartitionStyleConverter, PartitionStyle,
};
use serde::{Deserialize, Serialize};

// Helper function to get device by ID
async fn get_device_by_id(device_id: &str) -> Option<Device> {
    let manager = PlatformDeviceManager;
    
    // First try to get the specific device
    if let Ok(Some(device)) = manager.get_device_by_id(device_id).await {
        return Some(device);
    }
    
    // Fallback to enumerating all devices and finding by ID
    if let Ok(devices) = manager.enumerate_devices().await {
        return devices.into_iter().find(|d| d.id == device_id);
    }
    
    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanDiskRequest {
    pub device_id: String,
    pub wipe_method: String, // "quick", "zero", "dod", "random"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertPartitionStyleRequest {
    pub device_id: String,
    pub target_style: String, // "mbr", "gpt", "uninitialized"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareDiskRequest {
    pub device_id: String,
    pub target_style: String,
    pub clean_first: bool,
}

/// Clean a disk (remove all partitions and data)
#[tauri::command]
pub async fn clean_disk(
    request: CleanDiskRequest,
) -> Result<String, String> {
    // Get the device by ID
    let device = get_device_by_id(&request.device_id)
        .await
        .ok_or_else(|| format!("Device not found: {}", request.device_id))?;
    
    // Safety check
    if device.is_system {
        return Err("Cannot clean system disk".to_string());
    }
    
    // Parse wipe method
    let wipe_method = match request.wipe_method.as_str() {
        "quick" => WipeMethod::Quick,
        "zero" => WipeMethod::Zero,
        "dod" => WipeMethod::DoD5220,
        "random" => WipeMethod::Random,
        _ => return Err(format!("Invalid wipe method: {}", request.wipe_method)),
    };
    
    let options = CleanOptions {
        wipe_method,
        zero_entire_disk: wipe_method != WipeMethod::Quick,
    };
    
    // Execute clean operation (needs elevation)
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        use std::env;
        use std::os::windows::process::CommandExt;
        use moses_platform::windows::elevation::is_elevated;
        
        // Get the path to the elevated worker
        let worker_exe = env::current_exe()
            .map_err(|e| format!("Failed to get executable path: {}", e))?
            .parent()
            .ok_or_else(|| "Failed to get executable directory".to_string())?
            .join("moses-worker.exe");
        
        // Serialize device and options
        let device_json = serde_json::to_string(&device)
            .map_err(|e| format!("Failed to serialize device: {}", e))?;
        let options_json = serde_json::to_string(&options)
            .map_err(|e| format!("Failed to serialize options: {}", e))?;
        
        // Write to temp files
        let temp_dir = env::temp_dir();
        let device_file = temp_dir.join(format!("moses_device_{}.json", std::process::id()));
        let options_file = temp_dir.join(format!("moses_clean_options_{}.json", std::process::id()));
        
        std::fs::write(&device_file, device_json)
            .map_err(|e| format!("Failed to write device file: {}", e))?;
        std::fs::write(&options_file, options_json)
            .map_err(|e| format!("Failed to write options file: {}", e))?;
        
        // Check if we're already elevated
        if is_elevated() {
            // Run directly without elevation
            let output = Command::new(&worker_exe)
                .arg("clean")
                .arg(&device_file)
                .arg(&options_file)
                .output()
                .map_err(|e| format!("Failed to run worker: {}", e))?;
            
            // Clean up temp files
            let _ = std::fs::remove_file(device_file);
            let _ = std::fs::remove_file(options_file);
            
            if output.status.success() {
                Ok("Disk cleaned successfully".to_string())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                Err(format!("Clean failed: {}{}", stdout, stderr))
            }
        } else {
            // Request elevation using PowerShell
            let ps_script = format!(
                r#"
                $worker = '{}'
                $deviceFile = '{}'
                $optionsFile = '{}'
                
                # Start the worker with elevation
                $startInfo = New-Object System.Diagnostics.ProcessStartInfo
                $startInfo.FileName = $worker
                $startInfo.Arguments = "clean `"$deviceFile`" `"$optionsFile`""
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
                        Write-Output "Disk cleaned successfully"
                        exit 0
                    }} else {{
                        Write-Error "Clean failed with exit code: $($process.ExitCode)"
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
            
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            let output = Command::new("powershell")
                .args(&[
                    "-NoProfile",
                    "-ExecutionPolicy", "Bypass",
                    "-Command", &ps_script
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| format!("Failed to run PowerShell: {}", e))?;
            
            if output.status.success() {
                Ok("Disk cleaned successfully".to_string())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("Clean failed: {}", stderr.trim()))
            }
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // On Unix systems, attempt direct clean (requires root)
        DiskCleaner::clean(&device, &options)
            .map(|_| "Disk cleaned successfully".to_string())
            .map_err(|e| format!("Clean failed: {:?}", e))
    }
}

/// Detect partition table conflicts
#[tauri::command]
pub async fn detect_conflicts(
    device_id: String,
) -> Result<ConflictReport, String> {
    // Get the device by ID
    let device = get_device_by_id(&device_id)
        .await
        .ok_or_else(|| format!("Device not found: {}", device_id))?;
    
    // Run conflict detection
    ConflictDetector::analyze(&device)
        .map_err(|e| format!("Analysis failed: {:?}", e))
}

/// Convert partition table style
#[tauri::command]
pub async fn convert_partition_style(
    request: ConvertPartitionStyleRequest,
) -> Result<String, String> {
    // Get the device by ID
    let device = get_device_by_id(&request.device_id)
        .await
        .ok_or_else(|| format!("Device not found: {}", request.device_id))?;
    
    // Safety check
    if device.is_system {
        return Err("Cannot convert system disk partition style".to_string());
    }
    
    // Execute conversion (needs elevation)
    #[cfg(target_os = "windows")]
    {
        // Validate target style
        match request.target_style.as_str() {
            "mbr" | "gpt" | "uninitialized" => {},
            _ => return Err(format!("Invalid partition style: {}", request.target_style)),
        }
        use std::process::Command;
        use std::env;
        
        let worker_exe = env::current_exe()
            .map_err(|e| format!("Failed to get executable path: {}", e))?
            .parent()
            .ok_or_else(|| "Failed to get executable directory".to_string())?
            .join("moses-worker.exe");
        
        let device_json = serde_json::to_string(&device)
            .map_err(|e| format!("Failed to serialize device: {}", e))?;
        
        let temp_dir = env::temp_dir();
        let device_file = temp_dir.join(format!("moses_device_{}.json", std::process::id()));
        
        std::fs::write(&device_file, device_json)
            .map_err(|e| format!("Failed to write device file: {}", e))?;
        
        let output = Command::new(&worker_exe)
            .arg("convert")
            .arg(&device_file)
            .arg(&request.target_style)
            .output()
            .map_err(|e| format!("Failed to run elevated worker: {}", e))?;
        
        let _ = std::fs::remove_file(device_file);
        
        if output.status.success() {
            Ok(format!("Converted to {} successfully", request.target_style))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Conversion failed: {}", stderr))
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        let target_style = match request.target_style.as_str() {
            "mbr" => PartitionStyle::MBR,
            "gpt" => PartitionStyle::GPT,
            "uninitialized" => PartitionStyle::Uninitialized,
            _ => return Err(format!("Invalid partition style: {}", request.target_style)),
        };
        
        PartitionStyleConverter::convert(&device, target_style)
            .map(|_| format!("Converted to {:?} successfully", target_style))
            .map_err(|e| format!("Conversion failed: {:?}", e))
    }
}

/// Prepare a disk for formatting (resolve conflicts automatically)
#[tauri::command]
pub async fn prepare_disk(
    request: PrepareDiskRequest,
) -> Result<String, String> {
    // Get the device by ID
    let device = get_device_by_id(&request.device_id)
        .await
        .ok_or_else(|| format!("Device not found: {}", request.device_id))?;
    
    // Safety check
    if device.is_system {
        return Err("Cannot prepare system disk".to_string());
    }
    
    // Execute preparation (needs elevation)
    #[cfg(target_os = "windows")]
    {
        // Validate target style
        match request.target_style.as_str() {
            "mbr" | "gpt" | "uninitialized" => {},
            _ => return Err(format!("Invalid partition style: {}", request.target_style)),
        }
        use std::process::Command;
        use std::env;
        
        let worker_exe = env::current_exe()
            .map_err(|e| format!("Failed to get executable path: {}", e))?
            .parent()
            .ok_or_else(|| "Failed to get executable directory".to_string())?
            .join("moses-worker.exe");
        
        let device_json = serde_json::to_string(&device)
            .map_err(|e| format!("Failed to serialize device: {}", e))?;
        
        let temp_dir = env::temp_dir();
        let device_file = temp_dir.join(format!("moses_device_{}.json", std::process::id()));
        
        std::fs::write(&device_file, device_json)
            .map_err(|e| format!("Failed to write device file: {}", e))?;
        
        let output = Command::new(&worker_exe)
            .arg("prepare")
            .arg(&device_file)
            .arg(&request.target_style)
            .arg(if request.clean_first { "clean" } else { "no-clean" })
            .output()
            .map_err(|e| format!("Failed to run elevated worker: {}", e))?;
        
        let _ = std::fs::remove_file(device_file);
        
        if output.status.success() {
            Ok("Disk prepared successfully".to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Preparation failed: {}", stderr))
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        let target_style = match request.target_style.as_str() {
            "mbr" => PartitionStyle::MBR,
            "gpt" => PartitionStyle::GPT,
            "uninitialized" => PartitionStyle::Uninitialized,
            _ => return Err(format!("Invalid partition style: {}", request.target_style)),
        };
        
        let report = DiskManager::prepare_disk(&device, target_style, request.clean_first)
            .map_err(|e| format!("Preparation failed: {:?}", e))?;
        
        let mut message = "Disk prepared successfully.\n".to_string();
        if !report.conflicts_found.is_empty() {
            message.push_str(&format!("Resolved {} conflicts.\n", report.conflicts_found.len()));
        }
        if let Some(style) = report.final_style {
            message.push_str(&format!("Final partition style: {:?}", style));
        }
        
        Ok(message)
    }
}

/// Quick clean - just removes partition structures
#[tauri::command]
pub async fn quick_clean(
    device_id: String,
) -> Result<String, String> {
    clean_disk(
        CleanDiskRequest {
            device_id,
            wipe_method: "quick".to_string(),
        },
    ).await
}

/// Check if a disk needs cleaning before formatting
#[tauri::command]
pub async fn needs_cleaning(
    device_id: String,
) -> Result<bool, String> {
    let device = get_device_by_id(&device_id)
        .await
        .ok_or_else(|| format!("Device not found: {}", device_id))?;
    
    ConflictDetector::needs_cleaning(&device)
        .map_err(|e| format!("Check failed: {:?}", e))
}