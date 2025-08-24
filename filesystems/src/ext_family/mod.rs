// Unified ext2/ext3/ext4 formatter that reuses ext4_native implementation
// This does NOT break or modify the existing ext4 formatter

use moses_core::{Device, FormatOptions, MosesError, FilesystemFormatter, SimulationReport, Platform};
use async_trait::async_trait;
use crate::ext4_native::core::ext_config::ExtConfig;

/// Formats ext2 filesystems using the ext4_native codebase
pub struct Ext2Formatter;

/// Formats ext3 filesystems using the ext4_native codebase  
pub struct Ext3Formatter;

// The existing Ext4NativeFormatter continues to work unchanged

#[async_trait]
impl FilesystemFormatter for Ext2Formatter {
    fn name(&self) -> &'static str {
        "ext2"
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        vec![Platform::Windows, Platform::Linux]
    }
    
    fn requires_external_tools(&self) -> bool {
        false
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        vec![]
    }
    
    async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        // Create ext2 config
        let config = ExtConfig::ext2();
        format_with_config(device, options, config).await
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        // ext2 specific validation
        if let Some(size) = options.additional_options.get("device_size") {
            if let Ok(size_bytes) = size.parse::<u64>() {
                if size_bytes > 2 * 1024_u64.pow(4) {
                    return Err(MosesError::Other(
                        "ext2 has a 2TB limit without 64-bit support".to_string()
                    ));
                }
            }
        }
        Ok(())
    }
    
    fn can_format(&self, device: &Device) -> bool {
        !device.is_system && device.mount_points.is_empty()
    }
    
    async fn dry_run(&self, device: &Device, options: &FormatOptions) -> Result<SimulationReport, MosesError> {
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: std::time::Duration::from_secs(30),
            warnings: if device.size > 1024_u64.pow(4) {
                vec!["Large device - consider ext4 for better performance".to_string()]
            } else {
                vec![]
            },
            required_tools: vec![],
            will_erase_data: true,
            space_after_format: (device.size as f64 * 0.95) as u64, // ~95% usable
        })
    }
}

#[async_trait]
impl FilesystemFormatter for Ext3Formatter {
    fn name(&self) -> &'static str {
        "ext3"
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        vec![Platform::Windows, Platform::Linux]
    }
    
    fn requires_external_tools(&self) -> bool {
        false
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        vec![]
    }
    
    async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        // Create ext3 config
        let config = ExtConfig::ext3();
        format_with_config(device, options, config).await
    }
    
    async fn validate_options(&self, _options: &FormatOptions) -> Result<(), MosesError> {
        Ok(())
    }
    
    fn can_format(&self, device: &Device) -> bool {
        !device.is_system && device.mount_points.is_empty()
    }
    
    async fn dry_run(&self, device: &Device, options: &FormatOptions) -> Result<SimulationReport, MosesError> {
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: std::time::Duration::from_secs(35), // Slightly longer due to journal
            warnings: vec![],
            required_tools: vec![],
            will_erase_data: true,
            space_after_format: (device.size as f64 * 0.92) as u64, // ~92% usable (journal takes space)
        })
    }
}

// Internal function that calls ext4_native with different configs
async fn format_with_config(
    device: &Device,
    options: &FormatOptions,
    config: ExtConfig,
) -> Result<(), MosesError> {
    // We'll create a custom formatter that uses the builder
    match config.version {
        crate::ext4_native::core::ext_config::ExtVersion::Ext2 => {
            format_ext2_impl(device, options).await
        }
        crate::ext4_native::core::ext_config::ExtVersion::Ext3 => {
            format_ext3_impl(device, options).await
        }
        crate::ext4_native::core::ext_config::ExtVersion::Ext4 => {
            // Use the standard ext4 formatter
            use crate::ext4_native::core::{
                formatter_impl::format_device_with_progress,
                progress::LoggingProgress,
            };
            use std::sync::Arc;
            format_device_with_progress(device, options, Arc::new(LoggingProgress)).await
        }
    }
}

// Format as ext2
async fn format_ext2_impl(
    device: &Device,
    options: &FormatOptions,
) -> Result<(), MosesError> {
    use crate::ext4_native::core::{
        ext_builder::ExtFilesystemBuilder,
        formatter_ext::format_device_ext_version,
        progress::LoggingProgress,
    };
    use std::sync::Arc;
    
    log::info!("Formatting {} as ext2", device.name);
    
    // Create ext2 builder
    let builder = ExtFilesystemBuilder::ext2(device.size)
        .block_size(options.cluster_size.unwrap_or(4096) as u32)
        .label(options.label.clone().unwrap_or_default());
    
    // Use the generic formatter with ext2 parameters
    format_device_ext_version(device, options, builder, Arc::new(LoggingProgress)).await
}

// Format as ext3
async fn format_ext3_impl(
    device: &Device,
    options: &FormatOptions,
) -> Result<(), MosesError> {
    use crate::ext4_native::core::{
        ext_builder::ExtFilesystemBuilder,
        formatter_ext::format_device_ext_version,
        progress::LoggingProgress,
    };
    use std::sync::Arc;
    
    log::info!("Formatting {} as ext3", device.name);
    
    // Create ext3 builder
    let builder = ExtFilesystemBuilder::ext3(device.size)
        .block_size(options.cluster_size.unwrap_or(4096) as u32)
        .label(options.label.clone().unwrap_or_default());
    
    // Use the generic formatter with ext3 parameters
    format_device_ext_version(device, options, builder, Arc::new(LoggingProgress)).await
}