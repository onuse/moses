// Disk management commands using socket-based worker
use moses_core::Device;
use moses_filesystems::disk_manager::{
    CleanOptions, WipeMethod,
    ConflictDetector, ConflictReport
};
use serde::{Deserialize, Serialize};
use crate::worker_server::{WorkerCommand, WorkerResponse, get_worker_server};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanDiskRequest {
    pub device_id: String,
    pub wipe_method: String,
}

// Helper function to get device by ID
async fn get_device_by_id(device_id: &str) -> Option<Device> {
    use moses_core::DeviceManager;
    use moses_platform::PlatformDeviceManager;
    
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

/// Clean a disk using the persistent worker
#[tauri::command]
pub async fn clean_disk_socket(
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
    
    // Get the worker server
    let server_arc = get_worker_server().await
        .map_err(|e| format!("Failed to get worker server: {}", e))?;
    
    let mut server_guard = server_arc.lock().await;
    let server = server_guard.as_mut()
        .ok_or_else(|| "Worker server not initialized".to_string())?;
    
    // Send clean command to worker
    let command = WorkerCommand::Clean {
        device,
        options,
    };
    
    match server.execute_command(command).await {
        Ok(WorkerResponse::Success(msg)) => Ok(msg),
        Ok(WorkerResponse::Error(err)) => Err(err),
        Ok(_) => Err("Unexpected response from worker".to_string()),
        Err(e) => Err(format!("Worker communication failed: {}", e)),
    }
}

/// Format a disk using the persistent worker
#[tauri::command]
pub async fn format_disk_socket(
    device: Device,
    options: moses_core::FormatOptions,
) -> Result<String, String> {
    // Safety check
    if device.is_system {
        return Err("Cannot format system disk".to_string());
    }
    
    // Get the worker server
    let server_arc = get_worker_server().await
        .map_err(|e| format!("Failed to get worker server: {}", e))?;
    
    let mut server_guard = server_arc.lock().await;
    let server = server_guard.as_mut()
        .ok_or_else(|| "Worker server not initialized".to_string())?;
    
    // Send format command to worker
    let command = WorkerCommand::Format {
        device: device.clone(),
        options: options.clone(),
    };
    
    match server.execute_command(command).await {
        Ok(WorkerResponse::Success(msg)) => {
            // After successful format, update both caches
            // This ensures Moses immediately recognizes the new filesystem
            
            // Update in-memory cache
            use crate::commands::filesystem::FILESYSTEM_CACHE;
            if let Ok(mut cache) = FILESYSTEM_CACHE.lock() {
                cache.insert(device.id.clone(), options.filesystem_type.clone());
                log::info!("Updated in-memory cache for {} to {}", device.id, options.filesystem_type);
            }
            
            // Update persistent cache
            use crate::filesystem_cache::{self, CachedFilesystemInfo};
            let cache_info = CachedFilesystemInfo {
                filesystem: options.filesystem_type.clone(),
                partition_table: Some("mbr".to_string()), // Assume MBR for now
                partitions: vec![],
                detected_at: std::time::SystemTime::now(),
            };
            filesystem_cache::cache_filesystem_info(&device.id, cache_info);
            log::info!("Updated persistent cache for {} to {}", device.id, options.filesystem_type);
            
            Ok(msg)
        }
        Ok(WorkerResponse::Error(err)) => Err(err),
        Ok(_) => Err("Unexpected response from worker".to_string()),
        Err(e) => Err(format!("Worker communication failed: {}", e)),
    }
}

/// Detect conflicts
#[tauri::command]
pub async fn detect_conflicts_socket(
    device_id: String,
) -> Result<ConflictReport, String> {
    // Get the device by ID
    let device = get_device_by_id(&device_id)
        .await
        .ok_or_else(|| format!("Device not found: {}", device_id))?;
    
    // Run conflict detection locally (doesn't need elevation)
    ConflictDetector::analyze(&device)
        .map_err(|e| format!("Analysis failed: {:?}", e))
}

/// Analyze filesystem using the persistent worker
#[tauri::command]
pub async fn analyze_filesystem_socket(
    device_id: String,
) -> Result<String, String> {
    // Get the device by ID
    let device = get_device_by_id(&device_id)
        .await
        .ok_or_else(|| format!("Device not found: {}", device_id))?;
    
    // Get the worker server
    let server_arc = get_worker_server().await
        .map_err(|e| format!("Failed to get worker server: {}", e))?;
    
    let mut server_guard = server_arc.lock().await;
    let server = server_guard.as_mut()
        .ok_or_else(|| "Worker server not initialized".to_string())?;
    
    // Send analyze command to worker
    let command = WorkerCommand::Analyze { device };
    
    match server.execute_command(command).await {
        Ok(WorkerResponse::Success(analysis_json)) => Ok(analysis_json),
        Ok(WorkerResponse::Error(err)) => Err(err),
        Ok(_) => Err("Unexpected response from worker".to_string()),
        Err(e) => Err(format!("Worker communication failed: {}", e)),
    }
}

/// Detect filesystem type using the persistent worker
#[tauri::command]
pub async fn detect_filesystem_socket(
    device_id: String,
) -> Result<String, String> {
    // Try cache first
    use crate::commands::filesystem::FILESYSTEM_CACHE;
    if let Ok(cache) = FILESYSTEM_CACHE.lock() {
        if let Some(fs_type) = cache.get(&device_id) {
            log::info!("Using cached filesystem type for {}: {}", device_id, fs_type);
            return Ok(fs_type.clone());
        }
    }
    
    // Get the device by ID
    let device = get_device_by_id(&device_id)
        .await
        .ok_or_else(|| format!("Device not found: {}", device_id))?;
    
    // Get the worker server
    let server_arc = get_worker_server().await
        .map_err(|e| format!("Failed to get worker server: {}", e))?;
    
    let mut server_guard = server_arc.lock().await;
    let server = server_guard.as_mut()
        .ok_or_else(|| "Worker server not initialized".to_string())?;
    
    // Send detect command to worker (reuse Analyze command)
    let command = WorkerCommand::Analyze { device };
    
    match server.execute_command(command).await {
        Ok(WorkerResponse::Success(result)) => {
            // Parse the analysis result to get filesystem type
            if let Ok(analysis) = serde_json::from_str::<serde_json::Value>(&result) {
                if let Some(fs_type) = analysis.get("filesystem_type").and_then(|v| v.as_str()) {
                    // Cache the result
                    if let Ok(mut cache) = FILESYSTEM_CACHE.lock() {
                        cache.insert(device_id.clone(), fs_type.to_string());
                    }
                    return Ok(fs_type.to_string());
                }
            }
            
            // If we can't parse, try direct filesystem detection
            Err("Could not determine filesystem type".to_string())
        }
        Ok(WorkerResponse::Error(err)) => Err(err),
        Ok(_) => Err("Unexpected response from worker".to_string()),
        Err(e) => Err(format!("Worker communication failed: {}", e)),
    }
}/// Convert partition table style using the persistent worker
#[tauri::command]
pub async fn convert_partition_style_socket(
    device_id: String,
    target_style: String,
) -> Result<String, String> {
    // Get the device by ID
    let device = get_device_by_id(&device_id)
        .await
        .ok_or_else(|| format!("Device not found: {}", device_id))?;
    
    // Safety check
    if device.is_system {
        return Err("Cannot convert system disk partition style".to_string());
    }
    
    // Validate target style
    match target_style.as_str() {
        "mbr" | "gpt" | "uninitialized" => {},
        _ => return Err(format!("Invalid partition style: {}", target_style)),
    }
    
    // Get the worker server
    let server_arc = get_worker_server().await
        .map_err(|e| format!("Failed to get worker server: {}", e))?;
    
    let mut server_guard = server_arc.lock().await;
    let server = server_guard.as_mut()
        .ok_or_else(|| "Worker server not initialized".to_string())?;
    
    // Send convert command to worker
    let command = WorkerCommand::Convert {
        device,
        target_style: target_style.clone(),
    };
    
    match server.execute_command(command).await {
        Ok(WorkerResponse::Success(msg)) => Ok(msg),
        Ok(WorkerResponse::Error(err)) => Err(err),
        Ok(_) => Err("Unexpected response from worker".to_string()),
        Err(e) => Err(format!("Worker communication failed: {}", e)),
    }
}

/// Prepare a disk for formatting using the persistent worker
#[tauri::command]
pub async fn prepare_disk_socket(
    device_id: String,
    target_style: String,
    clean_first: bool,
) -> Result<String, String> {
    // Get the device by ID
    let device = get_device_by_id(&device_id)
        .await
        .ok_or_else(|| format!("Device not found: {}", device_id))?;
    
    // Safety check
    if device.is_system {
        return Err("Cannot prepare system disk".to_string());
    }
    
    // Validate target style
    match target_style.as_str() {
        "mbr" | "gpt" | "uninitialized" => {},
        _ => return Err(format!("Invalid partition style: {}", target_style)),
    }
    
    // Get the worker server
    let server_arc = get_worker_server().await
        .map_err(|e| format!("Failed to get worker server: {}", e))?;
    
    let mut server_guard = server_arc.lock().await;
    let server = server_guard.as_mut()
        .ok_or_else(|| "Worker server not initialized".to_string())?;
    
    // Send prepare command to worker
    let command = WorkerCommand::Prepare {
        device,
        target_style: target_style.clone(),
        clean_first,
    };
    
    match server.execute_command(command).await {
        Ok(WorkerResponse::Success(msg)) => Ok(msg),
        Ok(WorkerResponse::Error(err)) => Err(err),
        Ok(_) => Err("Unexpected response from worker".to_string()),
        Err(e) => Err(format!("Worker communication failed: {}", e)),
    }
}