use moses_core::{Device, FilesystemFormatter, FormatOptions, MosesError, Platform, SimulationReport};
use std::time::Duration;

pub struct Ext4Formatter;

#[async_trait::async_trait]
impl FilesystemFormatter for Ext4Formatter {
    fn name(&self) -> &'static str {
        "ext4"
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        vec![Platform::Linux, Platform::Windows, Platform::MacOS]
    }
    
    fn can_format(&self, device: &Device) -> bool {
        // Never format system drives
        if device.is_system {
            return false;
        }
        
        // Never format drives with critical mount points
        for mount in &device.mount_points {
            let mount_str = mount.to_string_lossy().to_lowercase();
            if mount_str == "/" || mount_str == "c:\\" || 
               mount_str.contains("/boot") || mount_str.contains("/system") ||
               mount_str.contains("c:\\windows") {
                return false;
            }
        }
        
        true
    }
    
    fn requires_external_tools(&self) -> bool {
        cfg!(target_os = "windows") || cfg!(target_os = "macos")
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        if cfg!(target_os = "windows") {
            vec!["ext2fsd", "mkfs.ext4"]
        } else if cfg!(target_os = "macos") {
            vec!["e2fsprogs"]
        } else {
            vec![]
        }
    }
    
    async fn format(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<(), MosesError> {
        // Mock implementation
        Err(MosesError::Other("Not yet implemented".to_string()))
    }
    
    async fn validate_options(&self, _options: &FormatOptions) -> Result<(), MosesError> {
        Ok(())
    }
    
    async fn dry_run(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<SimulationReport, MosesError> {
        let mut warnings = vec![];
        
        // Warn about system drives
        if device.is_system {
            warnings.push("WARNING: This is a system drive! Formatting will destroy the operating system!".to_string());
        }
        
        // Warn about mounted drives
        if !device.mount_points.is_empty() {
            warnings.push(format!("WARNING: Drive is mounted at: {:?}", device.mount_points));
        }
        
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: Duration::from_secs(60),
            warnings,
            required_tools: self.bundled_tools().into_iter().map(String::from).collect(),
            will_erase_data: true,
            space_after_format: device.size * 95 / 100,
        })
    }
}