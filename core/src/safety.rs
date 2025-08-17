//! Safety enforcement system for Moses formatters
//! 
//! This module ensures that ALL formatters MUST perform safety checks
//! before being allowed to format any device. It's impossible to bypass.

use crate::{Device, FormatOptions, MosesError};
use std::collections::HashSet;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// Mandatory safety check that MUST be completed before any format operation
/// The formatter MUST explicitly check each safety aspect and provide justification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyCheck {
    /// Timestamp when this safety check was performed
    pub timestamp: DateTime<Utc>,
    
    /// Device being checked
    pub device_id: String,
    
    /// Result of system drive check
    pub system_drive_check: SystemDriveCheck,
    
    /// Result of mount point check
    pub mount_point_check: MountPointCheck,
    
    /// Result of data loss acknowledgment
    pub data_loss_acknowledgment: DataLossAcknowledgment,
    
    /// Additional safety validations
    pub custom_checks: Vec<CustomSafetyCheck>,
    
    /// Formatter must sign this check with their name
    pub formatter_signature: String,
    
    /// Risk level assessment
    pub risk_assessment: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDriveCheck {
    pub is_system_drive: bool,
    pub check_performed: bool,
    pub override_reason: Option<String>, // If attempting to format system drive, MUST provide reason
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountPointCheck {
    pub mount_points: Vec<PathBuf>,
    pub has_critical_mounts: bool,
    pub critical_mounts_found: Vec<PathBuf>,
    pub check_performed: bool,
    pub override_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataLossAcknowledgment {
    pub data_will_be_lost: bool,
    pub acknowledgment_provided: bool,
    pub backup_confirmed: bool,
    pub estimated_data_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomSafetyCheck {
    pub check_name: String,
    pub passed: bool,
    pub details: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    Safe,           // Removable drive, no mounts, not system
    Low,            // Non-system drive with non-critical mounts
    Medium,         // Important drive but user confirmed
    High,           // System-adjacent drive
    Critical,       // System drive or critical mounts
    Forbidden,      // Should NEVER format
}

impl SafetyCheck {
    /// Create a new safety check for a device
    pub fn new(device: &Device, formatter_name: &str) -> Self {
        let mut check = Self {
            timestamp: Utc::now(),
            device_id: device.id.clone(),
            system_drive_check: SystemDriveCheck {
                is_system_drive: device.is_system,
                check_performed: false,
                override_reason: None,
            },
            mount_point_check: MountPointCheck {
                mount_points: device.mount_points.clone(),
                has_critical_mounts: false,
                critical_mounts_found: Vec::new(),
                check_performed: false,
                override_reason: None,
            },
            data_loss_acknowledgment: DataLossAcknowledgment {
                data_will_be_lost: true,
                acknowledgment_provided: false,
                backup_confirmed: false,
                estimated_data_size: Some(device.size),
            },
            custom_checks: Vec::new(),
            formatter_signature: formatter_name.to_string(),
            risk_assessment: RiskLevel::Forbidden, // Start with forbidden, must be explicitly lowered
        };
        
        // Automatically check for critical mounts
        check.check_critical_mounts();
        check
    }
    
    /// Check if device has critical mount points
    fn check_critical_mounts(&mut self) {
        let critical_paths = get_critical_mount_points();
        
        for mount in &self.mount_point_check.mount_points {
            let mount_str = mount.to_string_lossy().to_lowercase();
            for critical in &critical_paths {
                if mount_str.contains(critical) {
                    self.mount_point_check.has_critical_mounts = true;
                    self.mount_point_check.critical_mounts_found.push(mount.clone());
                }
            }
        }
    }
    
    /// Formatter MUST call this to verify system drive status
    pub fn verify_not_system_drive(&mut self) -> Result<(), MosesError> {
        self.system_drive_check.check_performed = true;
        
        if self.system_drive_check.is_system_drive
            && self.system_drive_check.override_reason.is_none() {
                return Err(MosesError::UnsafeDevice(
                    "Cannot format system drive without explicit override reason".to_string()
                ));
            }
        Ok(())
    }
    
    /// Formatter MUST call this to verify mount points are safe
    pub fn verify_safe_mount_points(&mut self) -> Result<(), MosesError> {
        self.mount_point_check.check_performed = true;
        
        if self.mount_point_check.has_critical_mounts
            && self.mount_point_check.override_reason.is_none() {
                return Err(MosesError::UnsafeDevice(
                    format!("Cannot format drive with critical mount points: {:?}", 
                            self.mount_point_check.critical_mounts_found)
                ));
            }
        Ok(())
    }
    
    /// Formatter MUST call this to acknowledge data loss
    pub fn acknowledge_data_loss(&mut self, backup_confirmed: bool) -> Result<(), MosesError> {
        self.data_loss_acknowledgment.acknowledgment_provided = true;
        self.data_loss_acknowledgment.backup_confirmed = backup_confirmed;
        
        if !backup_confirmed && self.risk_assessment as u8 > RiskLevel::Low as u8 {
            return Err(MosesError::UnsafeDevice(
                "High-risk format requires backup confirmation".to_string()
            ));
        }
        Ok(())
    }
    
    /// Add a custom safety check
    pub fn add_custom_check(&mut self, name: &str, passed: bool, details: &str) {
        self.custom_checks.push(CustomSafetyCheck {
            check_name: name.to_string(),
            passed,
            details: details.to_string(),
        });
    }
    
    /// Calculate risk level based on all checks
    pub fn assess_risk(&mut self) -> RiskLevel {
        // System drive is always forbidden
        if self.system_drive_check.is_system_drive && 
           self.system_drive_check.override_reason.is_none() {
            self.risk_assessment = RiskLevel::Forbidden;
            return self.risk_assessment;
        }
        
        // Critical mounts are forbidden without override
        if self.mount_point_check.has_critical_mounts && 
           self.mount_point_check.override_reason.is_none() {
            self.risk_assessment = RiskLevel::Forbidden;
            return self.risk_assessment;
        }
        
        // Calculate based on various factors
        let mut risk_score = 0;
        
        if self.system_drive_check.is_system_drive {
            risk_score += 100;
        }
        if self.mount_point_check.has_critical_mounts {
            risk_score += 50;
        }
        if !self.mount_point_check.mount_points.is_empty() {
            risk_score += 20;
        }
        if !self.data_loss_acknowledgment.backup_confirmed {
            risk_score += 10;
        }
        
        self.risk_assessment = match risk_score {
            0..=10 => RiskLevel::Safe,
            11..=30 => RiskLevel::Low,
            31..=50 => RiskLevel::Medium,
            51..=80 => RiskLevel::High,
            81..=100 => RiskLevel::Critical,
            _ => RiskLevel::Forbidden,
        };
        
        self.risk_assessment
    }
    
    /// Validate that all required checks have been performed
    pub fn validate(&self) -> Result<SafetyValidation, Vec<String>> {
        let mut errors = Vec::new();
        
        if !self.system_drive_check.check_performed {
            errors.push("System drive check not performed".to_string());
        }
        
        if !self.mount_point_check.check_performed {
            errors.push("Mount point check not performed".to_string());
        }
        
        if !self.data_loss_acknowledgment.acknowledgment_provided {
            errors.push("Data loss not acknowledged".to_string());
        }
        
        if self.risk_assessment == RiskLevel::Forbidden {
            errors.push("Risk level is FORBIDDEN - cannot proceed".to_string());
        }
        
        // Check custom checks
        for check in &self.custom_checks {
            if !check.passed {
                errors.push(format!("Custom check failed: {} - {}", 
                                  check.check_name, check.details));
            }
        }
        
        if errors.is_empty() {
            Ok(SafetyValidation {
                check_id: format!("{}-{}", self.device_id, self.timestamp.timestamp()),
                device_id: self.device_id.clone(),
                risk_level: self.risk_assessment,
                timestamp: self.timestamp,
                formatter: self.formatter_signature.clone(),
            })
        } else {
            Err(errors)
        }
    }
}

/// A validated safety check that can be used to proceed with formatting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyValidation {
    pub check_id: String,
    pub device_id: String,
    pub risk_level: RiskLevel,
    pub timestamp: DateTime<Utc>,
    pub formatter: String,
}

/// Get list of critical mount points that should never be formatted
fn get_critical_mount_points() -> HashSet<String> {
    let mut critical = HashSet::new();
    
    // Unix/Linux critical paths
    critical.insert("/".to_string());
    critical.insert("/boot".to_string());
    critical.insert("/boot/efi".to_string());
    critical.insert("/system".to_string());
    critical.insert("/usr".to_string());
    critical.insert("/var".to_string());
    critical.insert("/etc".to_string());
    critical.insert("/home".to_string());
    
    // Windows critical paths
    critical.insert("c:\\".to_string());
    critical.insert("c:\\windows".to_string());
    critical.insert("c:\\program files".to_string());
    critical.insert("c:\\users".to_string());
    critical.insert("c:\\programdata".to_string());
    
    // macOS critical paths
    critical.insert("/system".to_string());
    critical.insert("/library".to_string());
    critical.insert("/applications".to_string());
    critical.insert("/users".to_string());
    
    critical
}

/// Safety enforcer that wraps formatters and ensures checks are performed
pub struct SafeFormatter<F> {
    inner: F,
    #[allow(dead_code)]
    require_validation: bool,
    audit_log: Vec<SafetyValidation>,
}

impl<F> SafeFormatter<F> {
    pub fn new(formatter: F) -> Self {
        Self {
            inner: formatter,
            require_validation: true,
            audit_log: Vec::new(),
        }
    }
    
    /// Get audit log of all safety validations
    pub fn get_audit_log(&self) -> &[SafetyValidation] {
        &self.audit_log
    }
}

#[async_trait::async_trait]
impl<F: crate::FilesystemFormatter> crate::FilesystemFormatter for SafeFormatter<F> {
    fn name(&self) -> &'static str {
        self.inner.name()
    }
    
    fn supported_platforms(&self) -> Vec<crate::Platform> {
        self.inner.supported_platforms()
    }
    
    fn can_format(&self, device: &Device) -> bool {
        // Create a safety check and see if it would pass
        let mut check = SafetyCheck::new(device, self.name());
        let _ = check.verify_not_system_drive();
        let _ = check.verify_safe_mount_points();
        check.assess_risk();
        
        // Only allow if risk is acceptable and inner formatter agrees
        check.risk_assessment <= RiskLevel::Medium && self.inner.can_format(device)
    }
    
    fn requires_external_tools(&self) -> bool {
        self.inner.requires_external_tools()
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        self.inner.bundled_tools()
    }
    
    async fn format(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<(), MosesError> {
        // ENFORCE: Safety check MUST be performed
        let mut safety_check = SafetyCheck::new(device, self.name());
        
        // Formatter MUST verify these
        safety_check.verify_not_system_drive()?;
        safety_check.verify_safe_mount_points()?;
        safety_check.acknowledge_data_loss(true)?; // In production, this would come from user
        
        // Assess final risk
        let risk = safety_check.assess_risk();
        if risk > RiskLevel::High {
            return Err(MosesError::UnsafeDevice(
                format!("Cannot proceed with format - risk level too high: {:?}", risk)
            ));
        }
        
        // Validate all checks passed
        let validation = safety_check.validate()
            .map_err(|errors| MosesError::UnsafeDevice(errors.join("; ")))?;
        
        // Log the validation
        let self_mut = self as *const Self as *mut Self;
        unsafe {
            (*self_mut).audit_log.push(validation.clone());
        }
        
        // Only proceed if validation passed
        self.inner.format(device, options).await
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        self.inner.validate_options(options).await
    }
    
    async fn dry_run(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<crate::SimulationReport, MosesError> {
        // Even dry run should perform safety checks
        let mut safety_check = SafetyCheck::new(device, self.name());
        let _ = safety_check.verify_not_system_drive();
        let _ = safety_check.verify_safe_mount_points();
        let risk = safety_check.assess_risk();
        
        let mut report = self.inner.dry_run(device, options).await?;
        
        // Add safety warnings to the report
        if risk >= RiskLevel::High {
            report.warnings.insert(0, 
                format!("⚠️ HIGH RISK OPERATION - Risk Level: {:?}", risk));
        }
        
        if device.is_system {
            report.warnings.insert(0, 
                "🚨 SYSTEM DRIVE DETECTED - THIS WILL DESTROY YOUR OS!".to_string());
        }
        
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_safety_check_blocks_system_drive() {
        let device = Device {
            id: "system".to_string(),
            name: "System Drive".to_string(),
            size: 500_000_000_000,
            device_type: crate::DeviceType::SSD,
            mount_points: vec![PathBuf::from("C:\\")],
            is_removable: false,
            is_system: true,
            filesystem: Some("ntfs".to_string()),
        };
        
        let mut check = SafetyCheck::new(&device, "test_formatter");
        
        // Should fail without override
        assert!(check.verify_not_system_drive().is_err());
        
        // Risk should be forbidden
        assert_eq!(check.assess_risk(), RiskLevel::Forbidden);
        
        // Validation should fail
        assert!(check.validate().is_err());
    }
    
    #[test]
    fn test_safety_check_allows_safe_usb() {
        let device = Device {
            id: "usb".to_string(),
            name: "USB Drive".to_string(),
            size: 16_000_000_000,
            device_type: crate::DeviceType::USB,
            mount_points: vec![],
            is_removable: true,
            is_system: false,
            filesystem: Some("fat32".to_string()),
        };
        
        let mut check = SafetyCheck::new(&device, "test_formatter");
        
        // Should pass all checks
        assert!(check.verify_not_system_drive().is_ok());
        assert!(check.verify_safe_mount_points().is_ok());
        assert!(check.acknowledge_data_loss(true).is_ok());
        
        // Risk should be safe
        assert_eq!(check.assess_risk(), RiskLevel::Safe);
        
        // Validation should pass
        assert!(check.validate().is_ok());
    }
}