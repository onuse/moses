use moses_core::{Device, FilesystemFormatter, FormatOptions, MosesError, Platform, SimulationReport};
use std::process::{Command, Stdio};
use std::time::Duration;
use std::io::{BufRead, BufReader};

pub struct Ext4LinuxFormatter;

impl Ext4LinuxFormatter {
    fn check_mkfs_available() -> Result<(), MosesError> {
        let output = Command::new("which")
            .arg("mkfs.ext4")
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to check for mkfs.ext4: {}", e)))?;
        
        if !output.status.success() {
            return Err(MosesError::ExternalToolMissing("mkfs.ext4".to_string()));
        }
        
        Ok(())
    }
    
    fn unmount_device(device_path: &str) -> Result<(), MosesError> {
        // Try to unmount the device (may fail if not mounted, which is ok)
        let _ = Command::new("umount")
            .arg(device_path)
            .output();
        
        // Also unmount any partitions
        let _ = Command::new("umount")
            .arg(format!("{}*", device_path))
            .output();
        
        Ok(())
    }
    
    fn check_device_busy(device_path: &str) -> Result<(), MosesError> {
        // Check if device is in use
        let output = Command::new("lsof")
            .arg(device_path)
            .output()
            .or_else(|_| {
                // lsof might not be available, try fuser instead
                Command::new("fuser")
                    .arg(device_path)
                    .output()
            });
        
        if let Ok(output) = output {
            if output.status.success() && !output.stdout.is_empty() {
                return Err(MosesError::Other(format!("Device {} is in use", device_path)));
            }
        }
        
        Ok(())
    }
    
    fn execute_format_with_progress(
        device_path: &str,
        label: Option<&str>,
        quick: bool,
    ) -> Result<(), MosesError> {
        let mut args = vec![
            "-F".to_string(), // Force creation (don't ask for confirmation)
        ];
        
        if let Some(label) = label {
            args.push("-L".to_string());
            args.push(label.to_string());
        }
        
        if quick {
            args.push("-E".to_string());
            args.push("lazy_itable_init=0,lazy_journal_init=0".to_string());
        }
        
        args.push(device_path.to_string());
        
        // Check if we need sudo
        let needs_sudo = !nix::unistd::geteuid().is_root();
        
        let mut cmd = if needs_sudo {
            // Try pkexec first (GUI-friendly)
            let pkexec_check = Command::new("which")
                .arg("pkexec")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            
            if pkexec_check {
                let mut cmd = Command::new("pkexec");
                cmd.arg("mkfs.ext4");
                cmd
            } else {
                // Fall back to sudo
                let mut cmd = Command::new("sudo");
                cmd.arg("-n"); // Non-interactive
                cmd.arg("mkfs.ext4");
                cmd
            }
        } else {
            Command::new("mkfs.ext4")
        };
        
        cmd.args(&args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        
        let mut child = cmd.spawn()
            .map_err(|e| MosesError::FormatError(format!("Failed to start mkfs.ext4: {}", e)))?;
        
        // Read progress from stderr (mkfs.ext4 outputs progress there)
        if let Some(stderr) = child.stderr.take() {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    // Parse progress lines like "Writing inode tables: 142/256"
                    if line.contains("Writing") || line.contains("Creating") || line.contains("Allocating") {
                        // In a real implementation, we'd send this progress to the GUI
                        eprintln!("Progress: {}", line);
                    }
                }
            }
        }
        
        let status = child.wait()
            .map_err(|e| MosesError::FormatError(format!("Failed to wait for mkfs.ext4: {}", e)))?;
        
        if !status.success() {
            return Err(MosesError::FormatError(format!(
                "mkfs.ext4 failed with exit code: {:?}",
                status.code()
            )));
        }
        
        Ok(())
    }
}

#[async_trait::async_trait]
impl FilesystemFormatter for Ext4LinuxFormatter {
    fn name(&self) -> &'static str {
        "ext4-linux"
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        vec![Platform::Linux]
    }
    
    fn can_format(&self, device: &Device) -> bool {
        // Can format if not a system device and not mounted
        !device.is_system && device.mount_points.is_empty()
    }
    
    fn requires_external_tools(&self) -> bool {
        false // mkfs.ext4 is usually part of the base system
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        vec![]
    }
    
    async fn format(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<(), MosesError> {
        // Safety checks
        if device.is_system {
            return Err(MosesError::UnsafeDevice("Cannot format system device".to_string()));
        }
        
        if !device.mount_points.is_empty() {
            // Try to unmount first
            Self::unmount_device(&device.id)?;
            
            // Check if still mounted
            Self::check_device_busy(&device.id)?;
        }
        
        // Check tool availability
        Self::check_mkfs_available()?;
        
        // Execute format
        Self::execute_format_with_progress(
            &device.id,
            options.label.as_deref(),
            options.quick_format,
        )?;
        
        Ok(())
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        if options.filesystem_type != "ext4" {
            return Err(MosesError::Other(format!(
                "This formatter only supports ext4, got {}",
                options.filesystem_type
            )));
        }
        
        // Validate label length (max 16 chars for ext4)
        if let Some(ref label) = options.label {
            if label.len() > 16 {
                return Err(MosesError::Other(
                    "EXT4 volume label must be 16 characters or less".to_string()
                ));
            }
        }
        
        Ok(())
    }
    
    async fn dry_run(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<SimulationReport, MosesError> {
        // Validate options first
        self.validate_options(options).await?;
        
        let mut warnings = Vec::new();
        
        // Check if mkfs.ext4 is available
        if Self::check_mkfs_available().is_err() {
            warnings.push("mkfs.ext4 is not installed".to_string());
        }
        
        // Check if device is mounted
        if !device.mount_points.is_empty() {
            warnings.push(format!(
                "Device is currently mounted at {:?}. It will be unmounted before formatting.",
                device.mount_points
            ));
        }
        
        // Check if we need elevated privileges
        if !nix::unistd::geteuid().is_root() {
            warnings.push("Administrative privileges will be required to format the device".to_string());
        }
        
        // Estimate time based on device size and quick format option
        let estimated_seconds = if options.quick_format {
            // Quick format: ~1 second per 10GB
            (device.size / 10_000_000_000) as u64 + 5
        } else {
            // Full format: ~10 seconds per 10GB
            (device.size / 1_000_000_000) as u64 + 10
        };
        
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: Duration::from_secs(estimated_seconds.max(5)),
            warnings,
            required_tools: if Self::check_mkfs_available().is_err() {
                vec!["mkfs.ext4".to_string()]
            } else {
                vec![]
            },
            will_erase_data: true,
            space_after_format: device.size * 95 / 100, // ~5% overhead for ext4
        })
    }
}