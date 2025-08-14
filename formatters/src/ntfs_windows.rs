use moses_core::{Device, FilesystemFormatter, FormatOptions, MosesError, Platform, SimulationReport};
use std::process::Command;
use std::time::Duration;

pub struct NtfsWindowsFormatter;

impl NtfsWindowsFormatter {
    /// Format using Windows native format.com command
    async fn format_native(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<(), MosesError> {
        // Extract drive letter if available
        let drive_letter = device.mount_points.first()
            .and_then(|p| p.to_str())
            .and_then(|s| {
                // Extract letter from paths like "E:" or "E:\"
                if s.len() >= 2 && s.chars().nth(1) == Some(':') {
                    s.chars().next()
                } else {
                    None
                }
            });

        if let Some(letter) = drive_letter {
            // Use format.com for mounted drives
            println!("Formatting drive {}:", letter);
            
            let mut cmd_args = vec![
                "/FS:NTFS".to_string(),
                "/Y".to_string(), // Suppress confirmation
            ];
            
            if options.quick_format {
                cmd_args.push("/Q".to_string());
            }
            
            if let Some(ref label) = options.label {
                cmd_args.push(format!("/V:{}", label));
            }
            
            if let Some(cluster_size) = options.cluster_size {
                cmd_args.push(format!("/A:{}", cluster_size));
            }
            
            if options.enable_compression {
                cmd_args.push("/C".to_string());
            }
            
            // Add drive letter
            cmd_args.push(format!("{}:", letter));
            
            let output = Command::new("format.com")
                .args(&cmd_args)
                .output()
                .map_err(|e| MosesError::Other(format!("Failed to execute format.com: {}", e)))?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(MosesError::FormatError(format!("Format failed: {}", stderr)));
            }
            
            println!("NTFS format completed successfully!");
            Ok(())
        } else {
            // For unmounted drives, we need to use diskpart or PowerShell
            self.format_unmounted_drive(device, options).await
        }
    }
    
    /// Format unmounted drive using diskpart
    async fn format_unmounted_drive(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<(), MosesError> {
        // Extract disk number from device path
        let disk_number = device.id
            .trim_start_matches("\\\\.\\PHYSICALDRIVE")
            .parse::<u32>()
            .map_err(|_| MosesError::Other(format!("Invalid device path: {}", device.id)))?;
        
        println!("Formatting unmounted drive {} as NTFS", device.id);
        
        // Create diskpart script
        let mut script = format!(
            "select disk {}\n\
             clean\n\
             create partition primary\n\
             select partition 1\n",
            disk_number
        );
        
        // Format with options
        if options.quick_format {
            script.push_str("format fs=ntfs quick");
        } else {
            script.push_str("format fs=ntfs");
        }
        
        if let Some(ref label) = options.label {
            script.push_str(&format!(" label=\"{}\"", label));
        }
        
        script.push_str("\nassign\nexit\n");
        
        // Write script to temp file
        let temp_script = std::env::temp_dir().join("moses_diskpart.txt");
        std::fs::write(&temp_script, script)
            .map_err(|e| MosesError::Other(format!("Failed to write diskpart script: {}", e)))?;
        
        // Execute diskpart
        let output = Command::new("diskpart")
            .arg("/s")
            .arg(&temp_script)
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to execute diskpart: {}", e)))?;
        
        // Clean up temp file
        let _ = std::fs::remove_file(temp_script);
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(MosesError::FormatError(
                format!("Diskpart failed:\nStdout: {}\nStderr: {}", stdout, stderr)
            ));
        }
        
        println!("NTFS format completed successfully!");
        Ok(())
    }
}

#[async_trait::async_trait]
impl FilesystemFormatter for NtfsWindowsFormatter {
    fn name(&self) -> &'static str {
        "ntfs"
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        vec![Platform::Windows]
    }
    
    fn can_format(&self, device: &Device) -> bool {
        !device.is_system && device.is_removable
    }
    
    fn requires_external_tools(&self) -> bool {
        false // Windows has native NTFS support
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        vec![]
    }
    
    async fn format(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<(), MosesError> {
        // Safety check
        if device.is_system {
            return Err(MosesError::Other("Cannot format system drive".to_string()));
        }
        
        // Use native Windows formatting
        self.format_native(device, options).await
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        // NTFS label validation
        if let Some(ref label) = options.label {
            if label.len() > 32 {
                return Err(MosesError::Other("NTFS label must be 32 characters or less".to_string()));
            }
        }
        
        // Cluster size validation
        if let Some(cluster_size) = options.cluster_size {
            let valid_sizes = [512, 1024, 2048, 4096, 8192, 16384, 32768, 65536];
            if !valid_sizes.contains(&cluster_size) {
                return Err(MosesError::Other(format!("Invalid cluster size: {}", cluster_size)));
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
        
        if device.is_system {
            warnings.push("WARNING: This is a system drive!".to_string());
        }
        
        warnings.push(format!("Will format {} as NTFS", device.name));
        
        if options.enable_compression {
            warnings.push("Compression will be enabled".to_string());
        }
        
        if let Some(cluster) = options.cluster_size {
            warnings.push(format!("Cluster size: {} bytes", cluster));
        }
        
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: if options.quick_format {
                Duration::from_secs(10)
            } else {
                Duration::from_secs(60)
            },
            warnings,
            required_tools: vec![],
            will_erase_data: true,
            space_after_format: device.size * 99 / 100, // NTFS has minimal overhead
        })
    }
}