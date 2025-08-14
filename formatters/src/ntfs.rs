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
    
    fn can_format(&self, _device: &Device) -> bool {
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
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: Duration::from_secs(45),
            warnings: vec![],
            required_tools: self.bundled_tools().into_iter().map(String::from).collect(),
            will_erase_data: true,
            space_after_format: device.size * 96 / 100,
        })
    }
}