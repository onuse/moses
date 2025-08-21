// FAT16 formatter using system tools (format.com or diskpart)

use moses_core::{Device, MosesError, FormatOptions, FilesystemFormatter, SimulationReport, Platform};
use async_trait::async_trait;
use log::info;

pub struct Fat16SystemFormatter;

#[async_trait]
impl FilesystemFormatter for Fat16SystemFormatter {
    fn name(&self) -> &'static str {
        "FAT16 (System)"
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        vec![Platform::Windows]
    }
    
    fn requires_external_tools(&self) -> bool {
        true
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        vec![]
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        if options.filesystem_type != "fat16" {
            return Err(MosesError::Other("Invalid filesystem type for FAT16 formatter".to_string()));
        }
        
        // FAT16 is limited to 4GB max
        // We'll let format.com handle the actual cluster size calculation
        Ok(())
    }
    
    fn can_format(&self, device: &Device) -> bool {
        // Don't format system drives
        if device.is_system {
            return false;
        }
        
        // Check size limits (max 4GB for FAT16)
        if device.size > 4 * 1024 * 1024 * 1024 {
            return false;
        }
        
        true
    }
    
    async fn dry_run(&self, device: &Device, options: &FormatOptions) -> Result<SimulationReport, MosesError> {
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: std::time::Duration::from_secs(2),
            warnings: if device.size > 2 * 1024 * 1024 * 1024 {
                vec!["Volume larger than 2GB may have compatibility issues with FAT16".to_string()]
            } else {
                vec![]
            },
            required_tools: vec!["format.com".to_string()],
            will_erase_data: true,
            space_after_format: device.size - (64 * 1024), // Approximate overhead
        })
    }
    
    async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        info!("Formatting {} as FAT16 using system tools", device.name);
        
        // On Windows, use format.com to create FAT16
        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            use std::os::windows::process::CommandExt;
            
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            
            // Check if we should create a partition table first
            let create_partition = options.additional_options
                .get("create_partition_table")
                .map(|v| v == "true")
                .unwrap_or(false);
            
            // Get the drive letter from mount points if available
            let drive_letter = device.mount_points.first()
                .and_then(|p| p.to_str())
                .and_then(|s| {
                    if s.len() >= 2 && s.chars().nth(1) == Some(':') {
                        Some(s.chars().next().unwrap())
                    } else {
                        None
                    }
                });
            
            // Extract disk number from device ID
            let disk_number = if let Some(num_str) = device.id.strip_prefix("\\\\.\\PHYSICALDRIVE") {
                num_str.parse::<u32>().ok()
            } else {
                None
            };
            
            if let Some(disk_num) = disk_number {
                if create_partition {
                    info!("Creating MBR partition table and FAT16 partition");
                    
                    // Use diskpart to create partition table and format
                    let diskpart_script = format!(
                        "select disk {}\n\
                         clean\n\
                         create partition primary\n\
                         select partition 1\n\
                         active\n\
                         format fs=fat quick\n\
                         {}",
                        disk_num,
                        if let Some(ref label) = options.label {
                            format!("label={}", label)
                        } else {
                            String::new()
                        }
                    );
                    
                    // Write script to temp file
                    let temp_script = std::env::temp_dir().join(format!("moses_diskpart_{}.txt", std::process::id()));
                    std::fs::write(&temp_script, diskpart_script)
                        .map_err(|e| MosesError::Other(format!("Failed to write diskpart script: {}", e)))?;
                    
                    // Run diskpart
                    let output = Command::new("diskpart")
                        .arg("/s")
                        .arg(&temp_script)
                        .creation_flags(CREATE_NO_WINDOW)
                        .output()
                        .map_err(|e| MosesError::Other(format!("Failed to run diskpart: {}", e)))?;
                    
                    // Clean up script file
                    let _ = std::fs::remove_file(&temp_script);
                    
                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(MosesError::Other(format!("Diskpart failed: {}", stderr)));
                    }
                    
                    info!("FAT16 format with partition table completed");
                } else if let Some(letter) = drive_letter {
                    // Format existing partition with format.com
                    info!("Formatting existing partition as FAT16");
                    
                    let mut cmd = Command::new("cmd");
                    cmd.arg("/c")
                       .arg(format!("format {}:", letter))
                       .arg("/FS:FAT")  // This creates FAT16 for smaller drives
                       .arg("/Q")       // Quick format
                       .arg("/Y");      // Confirm automatically
                    
                    if let Some(ref label) = options.label {
                        cmd.arg(format!("/V:{}", label));
                    }
                    
                    cmd.creation_flags(CREATE_NO_WINDOW);
                    
                    let output = cmd.output()
                        .map_err(|e| MosesError::Other(format!("Failed to run format.com: {}", e)))?;
                    
                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(MosesError::Other(format!("Format failed: {}", stderr)));
                    }
                    
                    info!("FAT16 format completed");
                } else {
                    return Err(MosesError::Other("No drive letter found for device".to_string()));
                }
            } else {
                return Err(MosesError::Other("Could not determine disk number".to_string()));
            }
            
            Ok(())
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // On Linux/Mac, use mkfs.vfat or similar
            let _ = options; // Will be used when implemented
            return Err(MosesError::Other("FAT16 formatting not yet implemented for this platform".to_string()));
        }
    }
}