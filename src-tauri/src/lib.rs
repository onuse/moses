use moses_core::{Device, DeviceManager, DeviceType, FilesystemFormatter, FormatOptions, SimulationReport};
use std::sync::Arc;
use std::time::Duration;

use moses_platform::PlatformDeviceManager;

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
    // Use real formatter's dry_run for EXT4
    if options.filesystem_type == "ext4" {
        #[cfg(target_os = "linux")]
        {
            let formatter = Ext4LinuxFormatter;
            return formatter.dry_run(&device, &options)
                .await
                .map_err(|e| format!("Simulation failed: {}", e));
        }
        
        #[cfg(target_os = "windows")]
        {
            let formatter = Ext4WindowsFormatter;
            return formatter.dry_run(&device, &options)
                .await
                .map_err(|e| format!("Simulation failed: {}", e));
        }
    }
    
    // Fallback mock simulation for other cases
    let mut warnings = Vec::new();
    
    if device.is_system {
        warnings.push("This is a system drive. Formatting will make your system unbootable!".to_string());
    }
    
    if options.filesystem_type == "ext4" && cfg!(target_os = "windows") {
        warnings.push("EXT4 formatting on Windows requires bundled tools".to_string());
    }
    
    Ok(SimulationReport {
        device: device.clone(),
        options: options.clone(),
        estimated_time: Duration::from_secs(if options.quick_format { 30 } else { 300 }),
        warnings,
        required_tools: if options.filesystem_type == "ext4" && cfg!(target_os = "windows") {
            vec!["ext2fsd".to_string()]
        } else {
            vec![]
        },
        will_erase_data: true,
        space_after_format: device.size - (device.size / 100), // ~99% available
    })
}

#[tauri::command]
async fn execute_format(
    device: Device,
    options: FormatOptions,
) -> Result<String, String> {
    // Check if system drive
    if device.is_system {
        return Err("Cannot format system drive".to_string());
    }
    
    // Handle EXT4 formatting
    if options.filesystem_type == "ext4" {
        #[cfg(target_os = "linux")]
        {
            let formatter = Ext4LinuxFormatter;
            
            // First validate the options
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
            
            return Ok(format!("Successfully formatted {} as EXT4", device.name));
        }
        
        #[cfg(target_os = "windows")]
        {
            let formatter = Ext4WindowsFormatter;
            
            // First validate the options
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
            
            return Ok(format!("Successfully formatted {} as EXT4", device.name));
        }
    }
    
    // Fallback mock implementation for unsupported filesystems
    tokio::time::sleep(Duration::from_secs(2)).await;
    Ok(format!("Mock format: {} as {} (not yet implemented)", device.name, options.filesystem_type))
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
            execute_format
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}