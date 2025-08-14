use moses_core::{Device, FilesystemFormatter, FormatOptions, MosesError, Platform, SimulationReport};
use std::process::Command;
use std::time::Duration;

pub struct ExFatFormatter;

impl ExFatFormatter {
    #[cfg(target_os = "windows")]
    async fn format_windows(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        // Get drive letter
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
            println!("Formatting drive {}: as exFAT", letter);
            
            let mut cmd_args = vec![
                "/FS:EXFAT".to_string(),
                "/Y".to_string(), // No confirmation
            ];
            
            if options.quick_format {
                cmd_args.push("/Q".to_string());
            }
            
            if let Some(ref label) = options.label {
                cmd_args.push(format!("/V:{}", label));
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
            self.format_unmounted_windows(device, options).await
        }
    }
    
    async fn format_unmounted_windows(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        let disk_number = device.id
            .trim_start_matches("\\\\.\\PHYSICALDRIVE")
            .parse::<u32>()
            .map_err(|_| MosesError::Other(format!("Invalid device path: {}", device.id)))?;
        
        let mut script = format!(
            "select disk {}\n\
             clean\n\
             create partition primary\n\
             select partition 1\n",
            disk_number
        );
        
        if options.quick_format {
            script.push_str("format fs=exfat quick");
        } else {
            script.push_str("format fs=exfat");
        }
        
        if let Some(ref label) = options.label {
            script.push_str(&format!(" label=\"{}\"", label));
        }
        
        script.push_str("\nassign\nexit\n");
        
        let temp_script = std::env::temp_dir().join("moses_exfat_diskpart.txt");
        std::fs::write(&temp_script, script)
            .map_err(|e| MosesError::Other(format!("Failed to write diskpart script: {}", e)))?;
        
        let output = Command::new("diskpart")
            .arg("/s")
            .arg(&temp_script)
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to execute diskpart: {}", e)))?;
        
        let _ = std::fs::remove_file(temp_script);
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(MosesError::FormatError(
                format!("Diskpart failed:\nStdout: {}\nStderr: {}", stdout, stderr)
            ));
        }
        
        Ok(())
    }
    
    #[cfg(target_os = "linux")]
    async fn format_linux(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        // Linux uses mkfs.exfat or mkexfatfs
        let mut cmd_args = vec![];
        
        if let Some(ref label) = options.label {
            cmd_args.push("-n".to_string());
            cmd_args.push(label.clone());
        }
        
        cmd_args.push(device.id.clone());
        
        // Try mkfs.exfat first, then mkexfatfs
        let result = Command::new("mkfs.exfat")
            .args(&cmd_args)
            .output();
        
        let output = match result {
            Ok(out) => out,
            Err(_) => {
                // Fallback to mkexfatfs
                Command::new("mkexfatfs")
                    .args(&cmd_args)
                    .output()
                    .map_err(|e| MosesError::Other(
                        format!("Neither mkfs.exfat nor mkexfatfs found. Install exfat-utils: {}", e)
                    ))?
            }
        };
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(MosesError::FormatError(format!("exFAT format failed: {}", stderr)));
        }
        
        Ok(())
    }
    
    #[cfg(target_os = "macos")]
    async fn format_macos(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        let cmd_args = vec![
            "eraseDisk",
            "ExFAT",
            options.label.as_deref().unwrap_or("Untitled"),
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
impl FilesystemFormatter for ExFatFormatter {
    fn name(&self) -> &'static str {
        "exfat"
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        vec![Platform::Windows, Platform::Linux, Platform::MacOS]
    }
    
    fn can_format(&self, device: &Device) -> bool {
        // exFAT supports very large drives (up to 128 PB theoretical)
        !device.is_system
    }
    
    fn requires_external_tools(&self) -> bool {
        cfg!(target_os = "linux") // Linux may need exfat-utils
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        if cfg!(target_os = "linux") {
            vec!["mkfs.exfat", "exfat-utils"]
        } else {
            vec![]
        }
    }
    
    async fn format(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<(), MosesError> {
        if device.is_system {
            return Err(MosesError::Other("Cannot format system drive".to_string()));
        }
        
        println!("Formatting {} as exFAT...", device.name);
        
        #[cfg(target_os = "windows")]
        return self.format_windows(device, options).await;
        
        #[cfg(target_os = "linux")]
        return self.format_linux(device, options).await;
        
        #[cfg(target_os = "macos")]
        return self.format_macos(device, options).await;
        
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        Err(MosesError::PlatformNotSupported("exFAT formatting not supported on this platform".to_string()))
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        // exFAT label validation
        if let Some(ref label) = options.label {
            if label.len() > 15 {
                return Err(MosesError::Other(
                    "exFAT label must be 15 characters or less".to_string()
                ));
            }
        }
        
        // exFAT doesn't support built-in compression
        if options.enable_compression {
            return Err(MosesError::Other(
                "exFAT does not support compression".to_string()
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
        
        warnings.push(format!("Will format {} as exFAT", device.name));
        warnings.push("exFAT is ideal for large files and cross-platform compatibility".to_string());
        warnings.push("No file size limitations (unlike FAT32's 4GB limit)".to_string());
        
        #[cfg(target_os = "linux")]
        {
            warnings.push("Note: Ensure exfat-utils is installed on Linux".to_string());
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
            required_tools: if cfg!(target_os = "linux") {
                vec!["mkfs.exfat or mkexfatfs".to_string()]
            } else {
                vec![]
            },
            will_erase_data: true,
            space_after_format: device.size * 99 / 100, // exFAT has minimal overhead
        })
    }
}