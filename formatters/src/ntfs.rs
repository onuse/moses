use moses_core::{Device, FilesystemFormatter, FormatOptions, MosesError, Platform, SimulationReport};
use std::time::Duration;

pub struct NtfsFormatter;

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
        _device: &Device,
        _options: &FormatOptions,
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
        
        warnings.push("All data on this device will be permanently erased".to_string());
        
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: Duration::from_secs(45),
            warnings,
            required_tools: self.bundled_tools().into_iter().map(String::from).collect(),
            will_erase_data: true,
            space_after_format: device.size * 96 / 100,
        })
    }
}