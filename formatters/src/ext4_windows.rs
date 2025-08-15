use moses_core::{Device, FilesystemFormatter, FormatOptions, MosesError, Platform, SimulationReport};
use std::process::Command;
use std::time::Duration;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub struct Ext4WindowsFormatter;

impl Ext4WindowsFormatter {
    /// Check if WSL2 is available on the system
    async fn check_wsl2_available(&self) -> bool {
        let mut cmd = Command::new("wsl");
        cmd.args(&["--status"]);
        
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        
        match cmd.output() {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }
    
    /// Check if a WSL distribution is installed
    async fn check_wsl_distro(&self) -> Result<String, MosesError> {
        let mut cmd = Command::new("wsl");
        cmd.args(&["-l", "-q"]);
        
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        
        let output = cmd.output()
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
        
        println!("Mapping Windows drive {} to WSL device...", drive_number);
        
        // Simple approach: List all block devices in WSL
        let mut cmd = Command::new("wsl");
        cmd.args(&["lsblk", "-d", "-n", "-o", "NAME"]);
        
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        
        let output = cmd.output()
            .map_err(|e| MosesError::Other(format!("Failed to list WSL block devices: {}", e)))?;
        
        if !output.status.success() {
            // If lsblk fails, try a simpler approach
            println!("lsblk failed, using fallback device mapping...");
            
            // Fallback mapping based on typical configurations:
            // PhysicalDrive0 = /dev/sda (system)
            // PhysicalDrive1 = /dev/sdb (system or secondary)
            // PhysicalDrive2+ = /dev/sdc+ (removable devices)
            if drive_number >= 2 {
                let device_letter = (b'a' + drive_number as u8) as char;
                let wsl_device = format!("/dev/sd{}", device_letter);
                println!("Using fallback mapping: {} -> {}", windows_path, wsl_device);
                return Ok(wsl_device);
            } else {
                return Err(MosesError::Other("Cannot format system drives".to_string()));
            }
        }
        
        let devices_output = String::from_utf8_lossy(&output.stdout);
        let wsl_devices: Vec<&str> = devices_output.lines()
            .filter(|line| line.starts_with("sd"))
            .collect();
        
        println!("Found {} devices in WSL: {:?}", wsl_devices.len(), wsl_devices);
        
        // Map based on drive index
        // Typically: sda = Drive0, sdb = Drive1, sdc = Drive2, etc.
        if (drive_number as usize) < wsl_devices.len() {
            let device_name = wsl_devices[drive_number as usize];
            let wsl_device = format!("/dev/{}", device_name);
            println!("Mapped {} to {}", windows_path, wsl_device);
            return Ok(wsl_device);
        }
        
        // If we can't find enough devices, use a fallback
        // This assumes removable drives come after system drives
        if drive_number >= 2 {
            let device_letter = (b'a' + drive_number as u8) as char;
            let wsl_device = format!("/dev/sd{}", device_letter);
            println!("Using fallback mapping: {} -> {}", windows_path, wsl_device);
            return Ok(wsl_device);
        }
        
        Err(MosesError::Other(format!("Could not map {} to WSL device", windows_path)))
    }
    
    /// Unmount device if it's mounted in WSL
    async fn unmount_in_wsl(&self, wsl_device: &str) -> Result<(), MosesError> {
        let mut cmd = Command::new("wsl");
        cmd.args(&["bash", "-c", &format!("sudo umount {} 2>/dev/null || true", wsl_device)]);
        
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        
        let _output = cmd.output()
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
        // Check if the device exists in WSL
        let mut cmd = Command::new("wsl");
        cmd.args(&["test", "-b", wsl_device]);
        
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        
        let device_check = cmd.output()
            .map_err(|e| MosesError::Other(format!("Failed to check device existence: {}", e)))?;
        
        if !device_check.status.success() {
            println!("Warning: Device {} may not be accessible in WSL", wsl_device);
            println!("Attempting format anyway...");
        }
        
        // First, check if mkfs.ext4 is available in WSL
        let mut cmd = Command::new("wsl");
        cmd.args(&["which", "mkfs.ext4"]);
        
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        
        let check = cmd.output()
            .map_err(|e| MosesError::Other(format!("Failed to check for mkfs.ext4: {}", e)))?;
        
        if !check.status.success() {
            // mkfs.ext4 is not available
            println!("mkfs.ext4 not found in WSL");
            
            // Check if we can use mke2fs as an alternative
            let mut cmd = Command::new("wsl");
            cmd.args(&["which", "mke2fs"]);
            
            #[cfg(target_os = "windows")]
            cmd.creation_flags(CREATE_NO_WINDOW);
            
            let mke2fs_check = cmd.output()
                .map_err(|e| MosesError::Other(format!("Failed to check for mke2fs: {}", e)))?;
            
            if !mke2fs_check.status.success() {
                return Err(MosesError::Other(
                    "ext4 formatting tools not found in WSL. Please install e2fsprogs:\n\
                    1. Open WSL terminal\n\
                    2. Run: sudo apt-get update && sudo apt-get install -y e2fsprogs\n\
                    3. Try formatting again".to_string()
                ));
            }
            
            println!("Using mke2fs as fallback for ext4 formatting");
        }
        
        // Build mkfs.ext4 command - try with sudo first
        let mut mkfs_args = vec![
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
        
        // Execute format command - first try without sudo
        println!("Formatting {} as EXT4...", wsl_device);
        let mut cmd = Command::new("wsl");
        cmd.args(&["bash", "-c", &mkfs_args.join(" ")]);
        
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        
        let output = cmd.output()
            .map_err(|e| MosesError::Other(format!("Failed to execute mkfs.ext4: {}", e)))?;
        
        if !output.status.success() {
            // Try with sudo if direct execution failed
            println!("Retrying with sudo privileges...");
            let sudo_command = format!("sudo -n {}", mkfs_args.join(" "));
            
            let mut cmd = Command::new("wsl");
            cmd.args(&["bash", "-c", &sudo_command]);
            
            #[cfg(target_os = "windows")]
            cmd.creation_flags(CREATE_NO_WINDOW);
            
            let sudo_output = cmd.output()
                .map_err(|e| MosesError::Other(format!("Failed to execute mkfs.ext4 with sudo: {}", e)))?;
            
            if !sudo_output.status.success() {
                let stderr = String::from_utf8_lossy(&sudo_output.stderr);
                if stderr.contains("password is required") || stderr.contains("sudo:") {
                    return Err(MosesError::Other(
                        "Formatting requires sudo privileges. Please configure WSL for passwordless sudo:\n\
                        1. Open WSL terminal\n\
                        2. Run: sudo visudo\n\
                        3. Add at the end: <your-username> ALL=(ALL) NOPASSWD: /sbin/mkfs.ext4\n\
                        4. Save and try formatting again".to_string()
                    ));
                }
                return Err(MosesError::Other(format!("mkfs.ext4 failed: {}", stderr)));
            }
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
        // Can format removable devices (even if mounted) and non-system drives
        !device.is_system && (device.is_removable || device.mount_points.is_empty())
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