use moses_core::{Device, FilesystemFormatter, FormatOptions, MosesError, Platform, SimulationReport};
use std::process::Command;
use std::time::Duration;

pub struct NtfsFormatter;

impl NtfsFormatter {
    #[cfg(target_os = "windows")]
    async fn format_windows(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        // Extract disk number from device ID (e.g., "\\.\PHYSICALDRIVE2" -> "2")
        let disk_number = device.id
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u32>()
            .map_err(|_| MosesError::Other("Invalid device ID".to_string()))?;
        
        // Build format command for Windows
        // Using format.com directly requires drive letter, so we'll use diskpart
        let mut script = format!("select disk {}\n", disk_number);
        script.push_str("clean\n");
        script.push_str("create partition primary\n");
        script.push_str("format fs=ntfs ");
        
        if let Some(label) = &options.label {
            script.push_str(&format!("label=\"{}\" ", label));
        }
        
        if options.quick_format {
            script.push_str("quick ");
        }
        
        script.push_str("\nassign\n");
        script.push_str("exit\n");
        
        // Write script to temp file
        let temp_script = std::env::temp_dir().join("moses_ntfs_format.txt");
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
            return Err(MosesError::FormatError(format!("NTFS format failed: {}", stderr)));
        }
        
        Ok(())
    }
    
    #[cfg(target_os = "linux")]
    async fn format_linux(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        // Check if mkfs.ntfs is available
        let check = Command::new("which")
            .arg("mkfs.ntfs")
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to check for mkfs.ntfs: {}", e)))?;
        
        if !check.status.success() {
            return Err(MosesError::ExternalToolMissing(
                "mkfs.ntfs not found. Please install ntfs-3g package".to_string()
            ));
        }
        
        // Build mkfs.ntfs command
        let mut cmd = Command::new("mkfs.ntfs");
        
        if options.quick_format {
            cmd.arg("-f");  // Fast format
        }
        
        if let Some(label) = &options.label {
            cmd.arg("-L");
            cmd.arg(label);
        }
        
        // Add device path
        cmd.arg(&device.id);
        
        // Execute the format command
        let output = cmd.output()
            .map_err(|e| MosesError::Other(format!("Failed to run mkfs.ntfs: {}", e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Permission denied") {
                return Err(MosesError::InsufficientPrivileges(
                    "Root privileges required. Try running with sudo".to_string()
                ));
            }
            return Err(MosesError::FormatError(format!("NTFS format failed: {}", stderr)));
        }
        
        Ok(())
    }
    
    #[cfg(target_os = "macos")]
    async fn format_macos(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        // macOS doesn't have native NTFS write support
        // We can format using third-party tools or fallback to read-only
        
        // Check if ntfs-3g is installed via Homebrew
        let check = Command::new("which")
            .arg("mkfs.ntfs")
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to check for mkfs.ntfs: {}", e)))?;
        
        if check.status.success() {
            // Use ntfs-3g if available
            return self.format_linux(device, options).await;
        }
        
        // Otherwise, we can't format NTFS on macOS without third-party tools
        Err(MosesError::ExternalToolMissing(
            "NTFS formatting requires ntfs-3g. Install with: brew install ntfs-3g-mac".to_string()
        ))
    }
}

#[async_trait::async_trait]
impl FilesystemFormatter for NtfsFormatter {
    fn name(&self) -> &'static str {
        "ntfs"
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
               mount_str.starts_with("c:\\windows") ||
               mount_str.starts_with("c:\\program") {
                return false;
            }
        }
        
        true
    }
    
    fn requires_external_tools(&self) -> bool {
        cfg!(target_os = "linux") || cfg!(target_os = "macos")
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        if cfg!(target_os = "linux") || cfg!(target_os = "macos") {
            vec!["ntfs-3g"]
        } else {
            vec![]
        }
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
        
        // Platform-specific formatting
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
            Err(MosesError::Other("Platform not supported".to_string()))
        }
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        // Validate label length (NTFS supports up to 32 characters)
        if let Some(label) = &options.label {
            if label.len() > 32 {
                return Err(MosesError::InvalidInput(
                    "NTFS volume label cannot exceed 32 characters".to_string()
                ));
            }
            
            // Check for invalid characters in label
            let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
            for ch in invalid_chars {
                if label.contains(ch) {
                    return Err(MosesError::InvalidInput(
                        format!("NTFS volume label cannot contain character: {}", ch)
                    ));
                }
            }
        }
        
        Ok(())
    }
    
    async fn dry_run(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<SimulationReport, MosesError> {
        let mut warnings = vec![];
        
        // Add warnings based on device properties
        if device.is_system {
            warnings.push("WARNING: This appears to be a system drive!".to_string());
        }
        
        if !device.mount_points.is_empty() {
            warnings.push(format!("Device is currently mounted at: {:?}", device.mount_points));
        }
        
        if !device.is_removable {
            warnings.push("This is a non-removable drive - ensure you have backups".to_string());
        }
        
        // Platform-specific warnings
        #[cfg(target_os = "macos")]
        {
            warnings.push("Note: macOS has limited NTFS write support without third-party tools".to_string());
        }
        
        warnings.push("All data on this device will be permanently erased".to_string());
        
        // Estimate formatting time based on device size and quick format option
        let estimated_seconds = if options.quick_format {
            10 + (device.size / (10 * 1_073_741_824)) // Quick format: ~10s + 1s per 10GB
        } else {
            60 + (device.size / 1_073_741_824) // Full format: ~1 minute + 1s per GB
        };
        
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: Duration::from_secs(estimated_seconds),
            warnings,
            required_tools: self.bundled_tools().into_iter().map(String::from).collect(),
            will_erase_data: true,
            space_after_format: device.size * 96 / 100, // NTFS overhead ~4%
        })
    }
}