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
        // Use the complete implementation with optional verification
        if options.verify_after_format {
            use std::sync::Arc;
            use crate::ext4_native::core::progress::LoggingProgress;
            crate::ext4_native::core::formatter_impl::format_device_with_verification(
                device, 
                options, 
                Arc::new(LoggingProgress)
            ).await
        } else {
            crate::ext4_native::core::formatter_impl::format_device(device, options).await
        }
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
        // Validate options first
        self.validate_options(options).await?;
        
        // Check if device can be formatted
        if !self.can_format(device) {
            return Err(MosesError::UnsafeDevice(
                "Device cannot be formatted (system device or not removable)".to_string()
            ));
        }
        
        let mut warnings = Vec::new();
        
        // Add warnings based on device characteristics
        if device.size < 100 * 1024 * 1024 {
            warnings.push("⚠️ Device is very small (< 100MB). EXT4 may not be optimal.".to_string());
        }
        
        if !device.mount_points.is_empty() {
            warnings.push(format!("⚠️ Device is currently mounted at: {:?}", device.mount_points));
            warnings.push("Device will be unmounted before formatting.".to_string());
        }
        
        // Estimate time based on device size and type
        let estimated_seconds = match device.device_type {
            moses_core::DeviceType::USB => {
                // USB 2.0 ~30MB/s, USB 3.0 ~100MB/s - assume USB 2.0 for safety
                (device.size / (30 * 1024 * 1024)) as u64 + 5
            },
            moses_core::DeviceType::SSD => {
                // SSD typically faster
                (device.size / (200 * 1024 * 1024)) as u64 + 3
            },
            _ => {
                // Default conservative estimate
                (device.size / (50 * 1024 * 1024)) as u64 + 5
            }
        };
        
        // Calculate overhead (ext4 uses ~5% for filesystem structures)
        let overhead_percent = 5;
        let usable_space = device.size * (100 - overhead_percent) / 100;
        
        // Add informational messages
        warnings.push(format!("✅ Native EXT4 implementation - no external tools required"));
        warnings.push(format!("📊 Filesystem overhead: ~{}%", overhead_percent));
        
        if options.quick_format {
            warnings.push("⚡ Quick format selected - only metadata will be written".to_string());
        } else {
            warnings.push("🔍 Full format selected - all sectors will be zeroed".to_string());
        }
        
        if options.verify_after_format {
            warnings.push("✔️ Post-format verification enabled - filesystem will be validated".to_string());
        }
        
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: std::time::Duration::from_secs(estimated_seconds.min(300)), // Cap at 5 minutes
            warnings,
            required_tools: vec![],
            will_erase_data: true,
            space_after_format: usable_space,
        })
    }
}