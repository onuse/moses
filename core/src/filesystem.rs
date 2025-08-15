use crate::{Device, MosesError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatOptions {
    pub filesystem_type: String,
    pub label: Option<String>,
    pub cluster_size: Option<u32>,
    pub quick_format: bool,
    pub enable_compression: bool,
    pub verify_after_format: bool,
    pub additional_options: HashMap<String, String>,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            filesystem_type: String::new(),
            label: None,
            cluster_size: None,
            quick_format: true,
            enable_compression: false,
            verify_after_format: false,
            additional_options: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationReport {
    pub device: Device,
    pub options: FormatOptions,
    pub estimated_time: std::time::Duration,
    pub warnings: Vec<String>,
    pub required_tools: Vec<String>,
    pub will_erase_data: bool,
    pub space_after_format: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOS,
    Linux,
}

impl Platform {
    pub fn current() -> Self {
        #[cfg(target_os = "windows")]
        return Platform::Windows;
        
        #[cfg(target_os = "macos")]
        return Platform::MacOS;
        
        #[cfg(target_os = "linux")]
        return Platform::Linux;
        
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        compile_error!("Unsupported platform");
    }
}

#[async_trait::async_trait]
pub trait FilesystemFormatter: Send + Sync {
    fn name(&self) -> &'static str;
    fn supported_platforms(&self) -> Vec<Platform>;
    fn can_format(&self, device: &Device) -> bool;
    fn requires_external_tools(&self) -> bool;
    fn bundled_tools(&self) -> Vec<&'static str>;
    
    async fn format(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<(), MosesError>;
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError>;
    
    async fn dry_run(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<SimulationReport, MosesError>;
}