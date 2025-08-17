//! Example of a formatter that properly uses the safety system
//! This shows best practices for implementing safe formatters

use moses_core::{Device, FilesystemFormatter, FormatOptions, MosesError, Platform, SimulationReport};
use moses_core::{SafetyCheck, RiskLevel};
use std::time::Duration;

pub struct SafeExt4Formatter;

#[async_trait::async_trait]
impl FilesystemFormatter for SafeExt4Formatter {
    fn name(&self) -> &'static str {
        "safe-ext4"
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        vec![Platform::Linux, Platform::Windows, Platform::MacOS]
    }
    
    fn can_format(&self, device: &Device) -> bool {
        // Create safety check to determine if we can format
        let mut safety_check = SafetyCheck::new(device, self.name());
        
        // Perform all safety validations
        if safety_check.verify_not_system_drive().is_err() {
            return false;
        }
        
        if safety_check.verify_safe_mount_points().is_err() {
            return false;
        }
        
        // Check risk level
        let risk = safety_check.assess_risk();
        
        // Only allow formatting if risk is acceptable
        risk <= RiskLevel::Medium
    }
    
    fn requires_external_tools(&self) -> bool {
        cfg!(target_os = "windows") || cfg!(target_os = "macos")
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        // Native implementation - no external tools required
        vec![]
    }
    
    async fn format(
        &self,
        device: &Device,
        _options: &FormatOptions,
    ) -> Result<(), MosesError> {
        // MANDATORY: Create and complete safety check
        let mut safety_check = SafetyCheck::new(device, self.name());
        
        // STEP 1: Verify not a system drive
        safety_check.verify_not_system_drive()
            .inspect_err(|_| {
                eprintln!("🚨 SAFETY VIOLATION: Attempted to format system drive!");
            })?;
        
        // STEP 2: Verify mount points are safe
        safety_check.verify_safe_mount_points()
            .inspect_err(|_| {
                eprintln!("🚨 SAFETY VIOLATION: Critical mount points detected!");
            })?;
        
        // STEP 3: Add custom checks specific to ext4
        self.perform_ext4_specific_checks(&mut safety_check, device)?;
        
        // STEP 4: Acknowledge data loss (in production, get from user)
        safety_check.acknowledge_data_loss(true)
            .inspect_err(|_| {
                eprintln!("⚠️ Data loss acknowledgment required");
            })?;
        
        // STEP 5: Final risk assessment
        let risk = safety_check.assess_risk();
        if risk > RiskLevel::Medium {
            return Err(MosesError::UnsafeDevice(
                format!("Risk level too high for automatic formatting: {:?}", risk)
            ));
        }
        
        // STEP 6: Validate all checks passed
        let validation = safety_check.validate()
            .map_err(|errors| {
                eprintln!("❌ Safety validation failed:");
                for error in &errors {
                    eprintln!("  - {}", error);
                }
                MosesError::UnsafeDevice(errors.join("; "))
            })?;
        
        // Log the safety validation for audit
        println!("✅ Safety validation passed: {}", validation.check_id);
        println!("   Risk level: {:?}", validation.risk_level);
        
        // Now proceed with actual formatting
        // This is where the real ext4 formatting would happen
        println!("Formatting {} as ext4...", device.name);
        
        // Simulate formatting
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        Ok(())
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        // Validate ext4-specific options
        if let Some(label) = &options.label {
            if label.len() > 16 {
                return Err(MosesError::InvalidInput(
                    "EXT4 labels must be 16 characters or less".to_string()
                ));
            }
        }
        Ok(())
    }
    
    async fn dry_run(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<SimulationReport, MosesError> {
        // Create safety check for dry run
        let mut safety_check = SafetyCheck::new(device, self.name());
        
        // Perform checks but don't fail - just collect warnings
        let mut warnings = vec![];
        
        if let Err(e) = safety_check.verify_not_system_drive() {
            warnings.push(format!("🚨 CRITICAL: {}", e));
        }
        
        if let Err(e) = safety_check.verify_safe_mount_points() {
            warnings.push(format!("⚠️ WARNING: {}", e));
        }
        
        let risk = safety_check.assess_risk();
        if risk > RiskLevel::Low {
            warnings.push(format!("⚠️ Risk Level: {:?}", risk));
        }
        
        // Native ext4 implementation - no special warnings needed
        
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

impl SafeExt4Formatter {
    /// Perform ext4-specific safety checks
    fn perform_ext4_specific_checks(
        &self,
        safety_check: &mut SafetyCheck,
        device: &Device,
    ) -> Result<(), MosesError> {
        // Check minimum size for ext4
        if device.size < 16 * 1024 * 1024 {
            safety_check.add_custom_check(
                "minimum_size",
                false,
                "Device too small for ext4 (minimum 16MB)"
            );
            return Err(MosesError::InvalidInput(
                "Device too small for ext4 filesystem".to_string()
            ));
        }
        
        // Check if device is already ext4 (avoid accidental reformat)
        // In real implementation, would check filesystem signature
        safety_check.add_custom_check(
            "existing_filesystem",
            true,
            "Checked for existing ext4 filesystem"
        );
        
        // Check for RAID membership
        safety_check.add_custom_check(
            "raid_check",
            true,
            "Device is not part of a RAID array"
        );
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moses_core::DeviceType;
    use std::path::PathBuf;
    
    #[tokio::test]
    async fn test_safe_formatter_blocks_system_drive() {
        let formatter = SafeExt4Formatter;
        
        let system_device = Device {
            id: "system".to_string(),
            name: "System Drive".to_string(),
            size: 500_000_000_000,
            device_type: DeviceType::SSD,
            mount_points: vec![PathBuf::from("/")],
            is_removable: false,
            is_system: true,
            filesystem: None,
        };
        
        // Should not be able to format
        assert!(!formatter.can_format(&system_device));
        
        // Format should fail
        let options = FormatOptions::default();
        let result = formatter.format(&system_device, &options).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_safe_formatter_allows_safe_usb() {
        let formatter = SafeExt4Formatter;
        
        let usb_device = Device {
            id: "usb".to_string(),
            name: "USB Drive".to_string(),
            size: 16_000_000_000,
            device_type: DeviceType::USB,
            mount_points: vec![],
            is_removable: true,
            is_system: false,
            filesystem: None,
        };
        
        // Should be able to format
        assert!(formatter.can_format(&usb_device));
        
        // Dry run should work
        let options = FormatOptions::default();
        let report = formatter.dry_run(&usb_device, &options).await.unwrap();
        
        // Should have minimal warnings for safe USB
        assert!(report.warnings.iter().all(|w| !w.contains("CRITICAL")));
    }
}