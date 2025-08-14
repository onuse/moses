use moses_core::{Device, FilesystemFormatter, FormatOptions, MosesError, Platform, SimulationReport};
use std::process::Command;
use std::time::Duration;

pub struct Fat32Formatter;

impl Fat32Formatter {
    #[cfg(target_os = "windows")]
    async fn format_windows(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        // Try to get drive letter
        let drive_letter = device.mount_points.first()
            .and_then(|p| p.to_str())
            .and_then(|s| {
                if s.len() >= 2 && s.chars().nth(1) == Some(':') {
                    s.chars().next()
                } else {
                    None
                }
            });

        if let Some(letter) = drive_letter {
            println!("Formatting drive {}: as FAT32", letter);
            
            let mut cmd_args = vec![
                "/FS:FAT32".to_string(),
                "/Y".to_string(), // No confirmation
            ];
            
            if options.quick_format {
                cmd_args.push("/Q".to_string());
            }
            
            if let Some(ref label) = options.label {
                // FAT32 labels are max 11 characters
                let truncated_label: String = label.chars().take(11).collect();
                cmd_args.push(format!("/V:{}", truncated_label));
            }
            
            cmd_args.push(format!("{}:", letter));
            
            let output = Command::new("format.com")
                .args(&cmd_args)
                .output()
                .map_err(|e| MosesError::Other(format!("Failed to execute format.com: {}", e)))?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(MosesError::FormatError(format!("Format failed: {}", stderr)));
            }
            
            Ok(())
        } else {
            Err(MosesError::Other("Drive letter not found. Please ensure drive is mounted.".to_string()))
        }
    }
    
    #[cfg(target_os = "linux")]
    async fn format_linux(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        let mut cmd_args = vec!["-F", "32"]; // FAT32
        
        let truncated_label;
        if let Some(ref label) = options.label {
            truncated_label = label.chars().take(11).collect::<String>();
            cmd_args.push("-n");
            cmd_args.push(&truncated_label);
        }
        
        cmd_args.push(&device.id);
        
        let output = Command::new("mkfs.fat")
            .args(&cmd_args)
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to execute mkfs.fat: {}", e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(MosesError::FormatError(format!("mkfs.fat failed: {}", stderr)));
        }
        
        Ok(())
    }
    
    #[cfg(target_os = "macos")]
    async fn format_macos(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        let mut cmd_args = vec![
            "eraseDisk",
            "FAT32",
            options.label.as_deref().unwrap_or("UNTITLED"),
            &device.id,
        ];
        
        let output = Command::new("diskutil")
            .args(&cmd_args)
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to execute diskutil: {}", e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(MosesError::FormatError(format!("diskutil failed: {}", stderr)));
        }
        
        Ok(())
    }
}

#[async_trait::async_trait]
impl FilesystemFormatter for Fat32Formatter {
    fn name(&self) -> &'static str {
        "fat32"
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        vec![Platform::Windows, Platform::Linux, Platform::MacOS]
    }
    
    fn can_format(&self, device: &Device) -> bool {
        // FAT32 max size is 2TB (some implementations support up to 8TB)
        if device.is_system {
            return false;
        }
        
        // Check for critical mount points
        for mount in &device.mount_points {
            let mount_str = mount.to_string_lossy().to_lowercase();
            if mount_str == "/" || 
               mount_str == "c:\\" || 
               mount_str.starts_with("/boot") ||
               mount_str.starts_with("c:\\windows") ||
               mount_str.starts_with("c:\\program") {
                return false;
            }
        }
        
        device.size <= 2 * 1024_u64.pow(4)
    }
    
    fn requires_external_tools(&self) -> bool {
        false // All platforms have native FAT32 support
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        vec![]
    }
    
    async fn format(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<(), MosesError> {
        if device.is_system {
            return Err(MosesError::Other("Cannot format system drive".to_string()));
        }
        
        // Check size limit (2TB for FAT32)
        if device.size > 2 * 1024_u64.pow(4) {
            return Err(MosesError::Other(
                "Device too large for FAT32. Maximum size is 2TB.".to_string()
            ));
        }
        
        println!("Formatting {} as FAT32...", device.name);
        
        #[cfg(target_os = "windows")]
        return self.format_windows(device, options).await;
        
        #[cfg(target_os = "linux")]
        return self.format_linux(device, options).await;
        
        #[cfg(target_os = "macos")]
        return self.format_macos(device, options).await;
        
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        Err(MosesError::PlatformNotSupported("FAT32 formatting not supported on this platform".to_string()))
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        // FAT32 label validation - max 11 characters
        if let Some(ref label) = options.label {
            if label.len() > 11 {
                return Err(MosesError::Other(
                    "FAT32 label must be 11 characters or less".to_string()
                ));
            }
            // FAT32 labels must be uppercase alphanumeric
            if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                return Err(MosesError::Other(
                    "FAT32 label can only contain letters, numbers, underscore, and hyphen".to_string()
                ));
            }
        }
        
        // FAT32 doesn't support compression
        if options.enable_compression {
            return Err(MosesError::Other(
                "FAT32 does not support compression".to_string()
            ));
        }
        
        Ok(())
    }
    
    async fn dry_run(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<SimulationReport, MosesError> {
        let mut warnings = vec![];
        
        if device.is_system {
            warnings.push("WARNING: This is a system drive!".to_string());
        }
        
        if device.size > 32 * 1024_u64.pow(3) {
            warnings.push("Note: Windows may have issues with FAT32 volumes larger than 32GB".to_string());
        }
        
        if device.size > 2 * 1024_u64.pow(4) {
            warnings.push("ERROR: Device too large for FAT32 (max 2TB)".to_string());
        }
        
        warnings.push(format!("Will format {} as FAT32", device.name));
        warnings.push("FAT32 has a 4GB file size limit".to_string());
        
        if let Some(ref label) = options.label {
            if label.len() > 11 {
                warnings.push(format!("Label will be truncated to: {}", 
                    label.chars().take(11).collect::<String>()));
            }
        }
        
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: if options.quick_format {
                Duration::from_secs(5)
            } else {
                Duration::from_secs(30)
            },
            warnings,
            required_tools: vec![],
            will_erase_data: true,
            space_after_format: device.size * 98 / 100, // FAT32 has some overhead
        })
    }
}