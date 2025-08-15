// Main ext4 formatter implementation
// Complete ext4 filesystem with root directory and lost+found

use moses_core::{Device, FilesystemFormatter, FormatOptions, MosesError, Platform, SimulationReport};

pub struct Ext4NativeFormatter;

#[async_trait::async_trait]
impl FilesystemFormatter for Ext4NativeFormatter {
    fn name(&self) -> &'static str {
        "ext4-native"
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        vec![Platform::Windows, Platform::Linux, Platform::MacOS]
    }
    
    fn can_format(&self, device: &Device) -> bool {
        // Never format system drives
        if device.is_system {
            return false;
        }
        
        // Only format removable devices for extra safety
        device.is_removable
    }
    
    fn requires_external_tools(&self) -> bool {
        false
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        vec![]
    }
    
    async fn format(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<(), MosesError> {
        // Use the complete implementation
        crate::ext4_native::core::formatter_impl::format_device(device, options).await
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        if let Some(ref label) = options.label {
            if label.len() > 16 {
                return Err(MosesError::Other("Label must be 16 characters or less".to_string()));
            }
        }
        Ok(())
    }
    
    async fn dry_run(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<SimulationReport, MosesError> {
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: std::time::Duration::from_secs(5),
            warnings: vec!["Native ext4 formatter - Phase 0".to_string()],
            required_tools: vec![],
            will_erase_data: true,
            space_after_format: device.size * 95 / 100,
        })
    }
}