use moses_core::{Device, FilesystemFormatter, FormatOptions, MosesError, Platform, SimulationReport};
use std::process::Command;
use std::time::Duration;

pub struct Ext4WindowsFormatter;

impl Ext4WindowsFormatter {
    /// Check if WSL2 is available on the system
    async fn check_wsl2_available(&self) -> bool {
        match Command::new("wsl")
            .args(&["--status"])
            .output()
        {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }
    
    /// Check if a WSL distribution is installed
    async fn check_wsl_distro(&self) -> Result<String, MosesError> {
        let output = Command::new("wsl")
            .args(&["-l", "-q"])
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to list WSL distributions: {}", e)))?;
        
        if !output.status.success() {
            return Err(MosesError::Other("No WSL distributions found".to_string()));
        }
        
        let distros = String::from_utf8_lossy(&output.stdout);
        let first_distro = distros.lines()
            .next()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| MosesError::Other("No WSL distributions installed".to_string()))?;
        
        Ok(first_distro.trim().to_string())
    }
    
    /// Translate Windows device path to WSL device path
    /// E.g., \\.\PHYSICALDRIVE2 -> /dev/sdc
    async fn translate_device_path(&self, windows_path: &str) -> Result<String, MosesError> {
        // Extract drive number from Windows path
        let drive_number = windows_path
            .trim_start_matches("\\\\.\\PHYSICALDRIVE")
            .parse::<u32>()
            .map_err(|_| MosesError::Other(format!("Invalid device path: {}", windows_path)))?;
        
        // Get block devices from WSL
        let output = Command::new("wsl")
            .args(&["lsblk", "-d", "-n", "-o", "NAME,SIZE,TYPE,VENDOR,MODEL"])
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to list WSL block devices: {}", e)))?;
        
        if !output.status.success() {
            return Err(MosesError::Other("Failed to enumerate WSL devices".to_string()));
        }
        
        let devices_output = String::from_utf8_lossy(&output.stdout);
        
        // For USB devices, we need to match by size or other characteristics
        // This is a simplified approach - in production we'd need more robust matching
        
        // Try to find removable devices in WSL
        let output = Command::new("wsl")
            .args(&["bash", "-c", "ls /dev/sd* | grep -E '/dev/sd[a-z]$' | while read dev; do echo -n \"$dev \"; lsblk -n -o SIZE,TRAN,VENDOR,MODEL $dev 2>/dev/null | head -1; done"])
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to enumerate devices in WSL: {}", e)))?;
        
        let wsl_devices = String::from_utf8_lossy(&output.stdout);
        
        // For now, we'll use a heuristic: map by order for removable devices
        // PhysicalDrive0 = system, PhysicalDrive1 = system, PhysicalDrive2+ = removable
        // This matches typical Windows enumeration
        
        // Count removable/USB devices in WSL
        let removable_devices: Vec<&str> = wsl_devices.lines()
            .filter(|line| line.contains("usb") || line.contains("USB"))
            .collect();
        
        if drive_number >= 2 {
            // Assume drives 2+ are removable
            let removable_index = (drive_number - 2) as usize;
            if removable_index < removable_devices.len() {
                if let Some(device_path) = removable_devices[removable_index].split_whitespace().next() {
                    return Ok(device_path.to_string());
                }
            }
            
            // Fallback: try common device paths
            // Usually removable devices start from /dev/sdc on systems with 2 internal drives
            let device_letter = (b'c' + (drive_number - 2) as u8) as char;
            return Ok(format!("/dev/sd{}", device_letter));
        }
        
        Err(MosesError::Other(format!("Could not map {} to WSL device", windows_path)))
    }
    
    /// Unmount device if it's mounted in WSL
    async fn unmount_in_wsl(&self, wsl_device: &str) -> Result<(), MosesError> {
        let output = Command::new("wsl")
            .args(&["bash", "-c", &format!("sudo umount {} 2>/dev/null || true", wsl_device)])
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to unmount device: {}", e)))?;
        
        Ok(())
    }
    
    /// Format device using WSL2
    async fn format_via_wsl(
        &self,
        wsl_device: &str,
        label: &str,
        quick_format: bool,
    ) -> Result<(), MosesError> {
        // First, check if mkfs.ext4 is available in WSL
        let check = Command::new("wsl")
            .args(&["which", "mkfs.ext4"])
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to check for mkfs.ext4: {}", e)))?;
        
        if !check.status.success() {
            // Try to install e2fsprogs
            println!("Installing e2fsprogs in WSL...");
            let install = Command::new("wsl")
                .args(&["bash", "-c", "sudo apt-get update && sudo apt-get install -y e2fsprogs"])
                .output()
                .map_err(|e| MosesError::Other(format!("Failed to install e2fsprogs: {}", e)))?;
            
            if !install.status.success() {
                return Err(MosesError::Other("Failed to install required tools in WSL".to_string()));
            }
        }
        
        // Build mkfs.ext4 command
        let mut mkfs_args = vec![
            "sudo".to_string(),
            "mkfs.ext4".to_string(),
            "-F".to_string(), // Force creation
        ];
        
        if !label.is_empty() {
            mkfs_args.push("-L".to_string());
            mkfs_args.push(label.to_string());
        }
        
        if quick_format {
            mkfs_args.push("-E".to_string());
            mkfs_args.push("lazy_itable_init=1,lazy_journal_init=1".to_string());
        }
        
        mkfs_args.push(wsl_device.to_string());
        
        // Execute format command
        println!("Formatting {} as EXT4...", wsl_device);
        let output = Command::new("wsl")
            .args(&["bash", "-c", &mkfs_args.join(" ")])
            .output()
            .map_err(|e| MosesError::Other(format!("Failed to execute mkfs.ext4: {}", e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(MosesError::Other(format!("mkfs.ext4 failed: {}", stderr)));
        }
        
        println!("Format completed successfully!");
        Ok(())
    }
}

#[async_trait::async_trait]
impl FilesystemFormatter for Ext4WindowsFormatter {
    fn name(&self) -> &'static str {
        "ext4"
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        vec![Platform::Windows]
    }
    
    fn can_format(&self, device: &Device) -> bool {
        // Can format removable devices and non-system drives
        !device.is_system && device.mount_points.is_empty()
    }
    
    fn requires_external_tools(&self) -> bool {
        true
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        vec!["WSL2", "mkfs.ext4"]
    }
    
    async fn format(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<(), MosesError> {
        // Check if WSL2 is available
        if !self.check_wsl2_available().await {
            return Err(MosesError::ExternalToolMissing(
                "WSL2 is not installed. Please install WSL2 from Microsoft Store or run: wsl --install".to_string()
            ));
        }
        
        // Check if a distro is installed
        let _distro = self.check_wsl_distro().await?;
        
        // Translate Windows device path to WSL path
        println!("Translating device path {} to WSL...", device.id);
        let wsl_device = self.translate_device_path(&device.id).await?;
        println!("WSL device path: {}", wsl_device);
        
        // Unmount if mounted
        self.unmount_in_wsl(&wsl_device).await?;
        
        // Format the device
        let label = options.label.as_deref().unwrap_or("");
        self.format_via_wsl(&wsl_device, label, options.quick_format).await?;
        
        Ok(())
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        // Validate label length
        if let Some(ref label) = options.label {
            if label.len() > 16 {
                return Err(MosesError::Other("EXT4 label must be 16 characters or less".to_string()));
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
        
        // Check WSL2 availability
        if !self.check_wsl2_available().await {
            warnings.push("WSL2 is not installed. You'll need to install it before formatting.".to_string());
        } else {
            // Check distro
            match self.check_wsl_distro().await {
                Ok(distro) => {
                    warnings.push(format!("Will use WSL distribution: {}", distro));
                },
                Err(_) => {
                    warnings.push("No WSL distribution found. You'll need to install one (run: wsl --install -d Ubuntu)".to_string());
                }
            }
        }
        
        if device.is_system {
            warnings.push("WARNING: This appears to be a system drive!".to_string());
        }
        
        warnings.push(format!("Device {} will be formatted as EXT4", device.name));
        warnings.push("All data on this device will be permanently erased!".to_string());
        
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: if options.quick_format {
                Duration::from_secs(30)
            } else {
                Duration::from_secs(120)
            },
            warnings,
            required_tools: vec!["WSL2".to_string(), "mkfs.ext4".to_string()],
            will_erase_data: true,
            space_after_format: device.size * 95 / 100, // ~95% usable space
        })
    }
}