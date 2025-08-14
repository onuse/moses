use moses_core::{Device, FilesystemFormatter, FormatOptions, MosesError, Platform, SimulationReport};
use std::process::Command;
use std::time::Duration;

pub struct Fat32Formatter;

impl Fat32Formatter {
    #[cfg(target_os = "windows")]
    async fn format_windows(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        // First try to use drive letter if available
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
            // Use format.com for mounted drives
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
            // Use diskpart for unmounted drives
            let disk_number = device.id
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<u32>()
                .map_err(|_| MosesError::Other("Invalid device ID".to_string()))?;
            
            // Build diskpart script
            let mut script = format!("select disk {}\n", disk_number);
            script.push_str("clean\n");
            script.push_str("create partition primary\n");
            script.push_str("format fs=fat32 ");
            
            if let Some(label) = &options.label {
                let truncated_label: String = label.chars().take(11).collect();
                script.push_str(&format!("label=\"{}\" ", truncated_label));
            }
            
            if options.quick_format {
                script.push_str("quick ");
            }
            
            script.push_str("\nassign\n");
            script.push_str("exit\n");
            
            // Write script to temp file
            let temp_script = std::env::temp_dir().join("moses_fat32_format.txt");
            std::fs::write(&temp_script, script)
                .map_err(|e| MosesError::IoError(e))?;
            
            // Execute diskpart with the script
            let output = Command::new("diskpart")
                .arg("/s")
                .arg(&temp_script)
                .output()
                .map_err(|e| MosesError::Other(format!("Failed to run diskpart: {}", e)))?;
            
            // Clean up temp file
            let _ = std::fs::remove_file(&temp_script);
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                
                // Check for common issues
                if stdout.contains("The volume size is too big") || stderr.contains("too big") {
                    return Err(MosesError::InvalidInput(
                        "Device too large for FAT32. Windows limits FAT32 to 32GB via format command".to_string()
                    ));
                }
                
                return Err(MosesError::FormatError(format!("FAT32 format failed: {}", stderr)));
            }
            
            Ok(())
        }
    }
    
    #[cfg(target_os = "linux")]
    async fn format_linux(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        // Check if mkfs.fat is available
        let check = Command::new("which")
            .arg("mkfs.fat")
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to check for mkfs.fat: {}", e)))?;
        
        if !check.status.success() {
            return Err(MosesError::ExternalToolMissing(
                "mkfs.fat not found. Please install dosfstools package".to_string()
            ));
        }
        
        let mut cmd = Command::new("mkfs.fat");
        cmd.arg("-F").arg("32"); // FAT32
        
        if let Some(ref label) = options.label {
            let truncated_label: String = label.chars().take(11).collect();
            cmd.arg("-n").arg(truncated_label);
        }
        
        // Add verbose flag for progress
        cmd.arg("-v");
        
        cmd.arg(&device.id);
        
        let output = cmd.output()
            .map_err(|e| MosesError::Other(format!("Failed to execute mkfs.fat: {}", e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Permission denied") {
                return Err(MosesError::InsufficientPrivileges(
                    "Root privileges required. Try running with sudo".to_string()
                ));
            }
            return Err(MosesError::FormatError(format!("mkfs.fat failed: {}", stderr)));
        }
        
        Ok(())
    }
    
    #[cfg(target_os = "macos")]
    async fn format_macos(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        // macOS uses diskutil for formatting
        let mut cmd = Command::new("diskutil");
        
        cmd.arg("eraseDisk");
        cmd.arg("FAT32");
        
        // Volume name
        let label = options.label.as_deref()
            .map(|l| l.chars().take(11).collect::<String>())
            .unwrap_or_else(|| "UNTITLED".to_string());
        cmd.arg(label);
        
        // Device identifier
        cmd.arg(&device.id);
        
        let output = cmd.output()
            .map_err(|e| MosesError::Other(format!("Failed to execute diskutil: {}", e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            
            if stderr.contains("requires root") || stdout.contains("requires root") {
                return Err(MosesError::InsufficientPrivileges(
                    "Root privileges required. Try running with sudo".to_string()
                ));
            }
            
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
        // Never format system drives
        if device.is_system {
            return false;
        }
        
        // Check for critical mount points
        for mount in &device.mount_points {
            let mount_str = mount.to_string_lossy().to_lowercase();
            if mount_str == "/" || 
               mount_str == "c:\\" || 
               mount_str.starts_with("/boot") ||
               mount_str.starts_with("/system") ||
               mount_str.starts_with("c:\\windows") ||
               mount_str.starts_with("c:\\program") {
                return false;
            }
        }
        
        // FAT32 theoretical limit is 2TB (some implementations support up to 8TB)
        // But Windows format.com limits it to 32GB
        #[cfg(target_os = "windows")]
        {
            // Allow up to 2TB - we'll handle the 32GB Windows limitation with a warning
            device.size <= 2 * 1024_u64.pow(4)
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            device.size <= 2 * 1024_u64.pow(4)
        }
    }
    
    fn requires_external_tools(&self) -> bool {
        #[cfg(target_os = "linux")]
        return true; // Requires dosfstools
        
        #[cfg(not(target_os = "linux"))]
        return false;
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        #[cfg(target_os = "linux")]
        return vec!["dosfstools"];
        
        #[cfg(not(target_os = "linux"))]
        return vec![];
    }
    
    async fn format(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<(), MosesError> {
        // Safety check
        if !self.can_format(device) {
            return Err(MosesError::UnsafeDevice(
                "Cannot format this device - it may be a system drive or have critical mount points".to_string()
            ));
        }
        
        // Validate options
        self.validate_options(options).await?;
        
        // Check size limit (2TB for FAT32)
        if device.size > 2 * 1024_u64.pow(4) {
            return Err(MosesError::InvalidInput(
                "Device too large for FAT32. Maximum size is 2TB.".to_string()
            ));
        }
        
        println!("Formatting {} as FAT32...", device.name);
        
        #[cfg(target_os = "windows")]
        {
            self.format_windows(device, options).await
        }
        
        #[cfg(target_os = "linux")]
        {
            self.format_linux(device, options).await
        }
        
        #[cfg(target_os = "macos")]
        {
            self.format_macos(device, options).await
        }
        
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            Err(MosesError::PlatformNotSupported("FAT32 formatting not supported on this platform".to_string()))
        }
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        // FAT32 label validation - max 11 characters
        if let Some(ref label) = options.label {
            if label.len() > 11 {
                // We'll truncate it rather than error
                println!("Warning: FAT32 label will be truncated to 11 characters");
            }
            
            // FAT32 labels must be uppercase alphanumeric (we'll convert)
            for c in label.chars().take(11) {
                if !c.is_ascii_alphanumeric() && c != '_' && c != '-' && c != ' ' {
                    return Err(MosesError::InvalidInput(
                        format!("FAT32 label cannot contain character: '{}'", c)
                    ));
                }
            }
        }
        
        // FAT32 doesn't support compression
        if options.enable_compression {
            return Err(MosesError::InvalidInput(
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
        
        if !device.mount_points.is_empty() {
            warnings.push(format!("Device is currently mounted at: {:?}", device.mount_points));
        }
        
        // Windows-specific 32GB limitation warning
        #[cfg(target_os = "windows")]
        {
            if device.size > 32 * 1024_u64.pow(3) {
                warnings.push("WARNING: Windows format command limits FAT32 to 32GB".to_string());
                warnings.push("Consider using exFAT for drives larger than 32GB on Windows".to_string());
            }
        }
        
        if device.size > 2 * 1024_u64.pow(4) {
            warnings.push("ERROR: Device too large for FAT32 (max 2TB)".to_string());
        }
        
        // FAT32 limitations
        warnings.push("FAT32 limitations:".to_string());
        warnings.push("• Maximum file size: 4GB".to_string());
        warnings.push("• Maximum volume size: 2TB".to_string());
        
        if let Some(ref label) = options.label {
            if label.len() > 11 {
                warnings.push(format!("Label will be truncated to: {}", 
                    label.chars().take(11).collect::<String>()));
            }
        }
        
        warnings.push("All data on this device will be permanently erased".to_string());
        
        // Estimate formatting time
        let estimated_seconds = if options.quick_format {
            5 + (device.size / (50 * 1_073_741_824)) // Quick format: ~5s + 1s per 50GB
        } else {
            30 + (device.size / (5 * 1_073_741_824)) // Full format: ~30s + 1s per 5GB
        };
        
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: Duration::from_secs(estimated_seconds),
            warnings,
            required_tools: self.bundled_tools().into_iter().map(String::from).collect(),
            will_erase_data: true,
            space_after_format: device.size * 98 / 100, // FAT32 overhead ~2%
        })
    }
}