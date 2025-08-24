// ext2 Native Implementation for Windows
// This is a simplified version of ext4 without journal, extents, or checksums

use moses_core::{Device, FormatOptions, MosesError, FilesystemFormatter, SimulationReport};
use async_trait::async_trait;

pub struct Ext2NativeFormatter;

#[async_trait]
impl FilesystemFormatter for Ext2NativeFormatter {
    async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        // Use the ext4 formatter with ext2-compatible options
        let mut ext2_options = options.clone();
        
        // Force ext2-compatible settings
        ext2_options.additional_options.insert("filesystem_revision".to_string(), "0".to_string()); // EXT2_GOOD_OLD_REV
        ext2_options.additional_options.insert("has_journal".to_string(), "false".to_string());
        ext2_options.additional_options.insert("use_extents".to_string(), "false".to_string());
        ext2_options.additional_options.insert("use_64bit".to_string(), "false".to_string());
        ext2_options.additional_options.insert("use_checksums".to_string(), "false".to_string());
        
        // Call the shared implementation
        format_ext_family(device, &ext2_options, ExtVersion::Ext2).await
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        // ext2 has 2TB limit without 64-bit support
        if let Some(device_size) = options.additional_options.get("device_size") {
            if let Ok(size) = device_size.parse::<u64>() {
                if size > 2 * 1024_u64.pow(4) { // 2TB
                    return Err(MosesError::InvalidOptions(
                        "ext2 does not support devices larger than 2TB".to_string()
                    ));
                }
            }
        }
        Ok(())
    }
    
    fn can_format(&self, device: &Device) -> bool {
        // Same requirements as ext4
        !device.is_system && device.mount_points.is_empty()
    }
    
    async fn dry_run(&self, device: &Device, options: &FormatOptions) -> Result<SimulationReport, MosesError> {
        Ok(SimulationReport {
            success: true,
            messages: vec![
                format!("Would format {} as ext2", device.name),
                format!("Filesystem size: {} bytes", device.size),
                format!("Block size: {} bytes", options.cluster_size.unwrap_or(4096)),
                "No journal (ext2)".to_string(),
                "Using indirect blocks (no extents)".to_string(),
                "No checksums".to_string(),
            ],
            warnings: if device.size > 1024_u64.pow(4) {
                vec!["Large device - consider ext4 for better performance".to_string()]
            } else {
                vec![]
            },
        })
    }
}

// Shared implementation for ext family
enum ExtVersion {
    Ext2,
    Ext3,
    Ext4,
}

async fn format_ext_family(
    device: &Device,
    options: &FormatOptions,
    version: ExtVersion,
) -> Result<(), MosesError> {
    // This would use the existing ext4_native core with different parameters
    // For now, we'll import and modify the ext4 implementation
    
    use crate::ext4_native::core::{
        formatter_impl::format_device_with_progress,
        progress::LoggingProgress,
    };
    use std::sync::Arc;
    
    // The actual implementation would modify the parameters based on version
    match version {
        ExtVersion::Ext2 => {
            // Set ext2-specific parameters in the formatter
            // This is where we'd disable journal, extents, etc.
            log::info!("Formatting as ext2 (no journal, no extents)");
        }
        ExtVersion::Ext3 => {
            log::info!("Formatting as ext3 (journal, no extents)");
        }
        ExtVersion::Ext4 => {
            log::info!("Formatting as ext4 (full features)");
        }
    }
    
    // Call the shared formatter with appropriate parameters
    format_device_with_progress(device, options, Arc::new(LoggingProgress)).await
}