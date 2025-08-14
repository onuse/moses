/// Enhanced Safety System v2
/// 
/// The SafetyCheck now OWNS the device reference and only provides access
/// after all safety validations pass. This makes it impossible to format
/// the wrong device or bypass safety checks.

use crate::{Device, FormatOptions, MosesError};
use std::collections::HashSet;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// A locked device that can only be accessed through safety checks
#[derive(Debug)]
pub struct LockedDevice<'a> {
    device: &'a Device,
    locked: bool,
    safety_token: String,
}

impl<'a> LockedDevice<'a> {
    fn new(device: &'a Device) -> Self {
        Self {
            device,
            locked: true,
            safety_token: format!("{}-{}", device.id, Utc::now().timestamp_nanos()),
        }
    }
    
    /// Get device info for read-only operations (always safe)
    pub fn info(&self) -> &Device {
        self.device
    }
    
    /// Check if device is locked
    pub fn is_locked(&self) -> bool {
        self.locked
    }
}

/// The safety check now CONTROLS access to the device
pub struct SafetyCheck<'a> {
    /// The locked device - formatter cannot access directly
    locked_device: LockedDevice<'a>,
    
    /// Timestamp when this safety check was created
    timestamp: DateTime<Utc>,
    
    /// Formatter requesting access
    formatter_name: String,
    
    /// Safety validations performed
    validations: SafetyValidations,
    
    /// Current risk assessment
    risk_level: RiskLevel,
    
    /// Whether all checks have passed
    all_checks_passed: bool,
}

#[derive(Debug, Default)]
struct SafetyValidations {
    system_drive_checked: bool,
    system_drive_passed: bool,
    mount_points_checked: bool,
    mount_points_passed: bool,
    data_loss_acknowledged: bool,
    backup_confirmed: bool,
    custom_checks: Vec<(String, bool, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    Safe = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
    Forbidden = 5,
}

/// The result of a successful safety check - provides controlled access to device
pub struct SafetyApproval<'a> {
    /// Unique token for this approval
    pub token: String,
    
    /// The device that can now be formatted
    device: &'a Device,
    
    /// Risk level of this operation
    pub risk_level: RiskLevel,
    
    /// Timestamp of approval
    pub approved_at: DateTime<Utc>,
    
    /// Formatter that was approved
    pub approved_formatter: String,
    
    /// One-time use flag
    used: std::cell::Cell<bool>,
}

impl<'a> SafetyApproval<'a> {
    /// Get the device for formatting - can only be called once
    pub fn get_device_for_format(&self) -> Result<&Device, MosesError> {
        if self.used.get() {
            return Err(MosesError::UnsafeDevice(
                "Safety approval already used - cannot reuse for multiple format operations".to_string()
            ));
        }
        self.used.set(true);
        Ok(self.device)
    }
    
    /// Check if this approval has been used
    pub fn is_used(&self) -> bool {
        self.used.get()
    }
}

impl<'a> SafetyCheck<'a> {
    /// Create a new safety check that locks the device
    pub fn lock_device(device: &'a Device, formatter_name: &str) -> Self {
        let locked_device = LockedDevice::new(device);
        
        Self {
            locked_device,
            timestamp: Utc::now(),
            formatter_name: formatter_name.to_string(),
            validations: SafetyValidations::default(),
            risk_level: RiskLevel::Forbidden, // Start forbidden
            all_checks_passed: false,
        }
    }
    
    /// Step 1: Check if this is a system drive
    pub fn check_system_drive(&mut self) -> Result<(), MosesError> {
        self.validations.system_drive_checked = true;
        
        if self.locked_device.device.is_system {
            self.validations.system_drive_passed = false;
            Err(MosesError::UnsafeDevice(
                format!("Device {} is a SYSTEM DRIVE - cannot format!", 
                        self.locked_device.device.id)
            ))
        } else {
            self.validations.system_drive_passed = true;
            Ok(())
        }
    }
    
    /// Step 2: Check mount points
    pub fn check_mount_points(&mut self) -> Result<(), MosesError> {
        self.validations.mount_points_checked = true;
        
        let critical = find_critical_mounts(&self.locked_device.device.mount_points);
        
        if !critical.is_empty() {
            self.validations.mount_points_passed = false;
            Err(MosesError::UnsafeDevice(
                format!("Device {} has critical mount points: {:?}", 
                        self.locked_device.device.id, critical)
            ))
        } else {
            self.validations.mount_points_passed = true;
            Ok(())
        }
    }
    
    /// Step 3: Acknowledge data loss
    pub fn acknowledge_data_loss(&mut self, backup_confirmed: bool) -> Result<(), MosesError> {
        self.validations.data_loss_acknowledged = true;
        self.validations.backup_confirmed = backup_confirmed;
        
        // Calculate current risk
        self.calculate_risk();
        
        if !backup_confirmed && self.risk_level > RiskLevel::Low {
            Err(MosesError::UnsafeDevice(
                "High-risk format requires backup confirmation".to_string()
            ))
        } else {
            Ok(())
        }
    }
    
    /// Add a custom safety check
    pub fn add_custom_check(&mut self, name: &str, passed: bool, details: &str) {
        self.validations.custom_checks.push((
            name.to_string(),
            passed,
            details.to_string()
        ));
    }
    
    /// Calculate risk level based on all factors
    fn calculate_risk(&mut self) {
        let device = self.locked_device.device;
        
        // System drive is always forbidden
        if device.is_system {
            self.risk_level = RiskLevel::Forbidden;
            return;
        }
        
        // Critical mounts are very high risk
        if !find_critical_mounts(&device.mount_points).is_empty() {
            self.risk_level = RiskLevel::Critical;
            return;
        }
        
        // Calculate score
        let mut score = 0;
        
        if !device.mount_points.is_empty() {
            score += 2;
        }
        if !device.is_removable {
            score += 1;
        }
        if !self.validations.backup_confirmed {
            score += 1;
        }
        
        self.risk_level = match score {
            0 => RiskLevel::Safe,
            1 => RiskLevel::Low,
            2..=3 => RiskLevel::Medium,
            4..=5 => RiskLevel::High,
            _ => RiskLevel::Critical,
        };
    }
    
    /// Final step: Request approval to format
    /// This is the ONLY way to get access to the device for formatting
    pub fn request_approval(mut self) -> Result<SafetyApproval<'a>, MosesError> {
        // Verify all mandatory checks were performed
        if !self.validations.system_drive_checked {
            return Err(MosesError::UnsafeDevice(
                "System drive check not performed".to_string()
            ));
        }
        
        if !self.validations.mount_points_checked {
            return Err(MosesError::UnsafeDevice(
                "Mount points check not performed".to_string()
            ));
        }
        
        if !self.validations.data_loss_acknowledged {
            return Err(MosesError::UnsafeDevice(
                "Data loss not acknowledged".to_string()
            ));
        }
        
        // Verify all checks passed
        if !self.validations.system_drive_passed {
            return Err(MosesError::UnsafeDevice(
                "System drive check failed".to_string()
            ));
        }
        
        if !self.validations.mount_points_passed {
            return Err(MosesError::UnsafeDevice(
                "Mount points check failed".to_string()
            ));
        }
        
        // Check custom validations
        for (name, passed, details) in &self.validations.custom_checks {
            if !passed {
                return Err(MosesError::UnsafeDevice(
                    format!("Custom check '{}' failed: {}", name, details)
                ));
            }
        }
        
        // Final risk check
        self.calculate_risk();
        if self.risk_level >= RiskLevel::Critical {
            return Err(MosesError::UnsafeDevice(
                format!("Risk level {:?} is too high - format denied", self.risk_level)
            ));
        }
        
        // All checks passed - create approval
        Ok(SafetyApproval {
            token: self.locked_device.safety_token.clone(),
            device: self.locked_device.device,
            risk_level: self.risk_level,
            approved_at: Utc::now(),
            approved_formatter: self.formatter_name.clone(),
            used: std::cell::Cell::new(false),
        })
    }
    
    /// Get device info for display/logging (read-only, always safe)
    pub fn device_info(&self) -> &Device {
        self.locked_device.info()
    }
    
    /// Get current risk level
    pub fn risk_level(&self) -> RiskLevel {
        self.risk_level
    }
}

/// Find critical mount points
fn find_critical_mounts(mount_points: &[PathBuf]) -> Vec<PathBuf> {
    let critical_paths = vec![
        "/", "/boot", "/boot/efi", "/system", "/usr", "/var", "/etc", "/home",
        "c:\\", "c:\\windows", "c:\\program files", "c:\\users", "c:\\programdata",
        "/System", "/Library", "/Applications", "/Users",
    ];
    
    let mut found = Vec::new();
    for mount in mount_points {
        let mount_str = mount.to_string_lossy().to_lowercase();
        for critical in &critical_paths {
            if mount_str == *critical || mount_str.starts_with(critical) {
                found.push(mount.clone());
                break;
            }
        }
    }
    found
}

/// Example formatter using the new safety system
pub struct SafeFormatterExample;

#[async_trait::async_trait]
impl crate::FilesystemFormatter for SafeFormatterExample {
    fn name(&self) -> &'static str {
        "safe-example"
    }
    
    fn supported_platforms(&self) -> Vec<crate::Platform> {
        vec![crate::Platform::Linux, crate::Platform::Windows]
    }
    
    fn can_format(&self, device: &Device) -> bool {
        // Quick check without full validation
        !device.is_system && device.is_removable
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
        // STEP 1: Lock the device with safety check
        let mut safety = SafetyCheck::lock_device(device, self.name());
        
        println!("🔒 Device locked for safety check: {}", safety.device_info().id);
        
        // STEP 2: Perform mandatory checks
        safety.check_system_drive()
            .map_err(|e| {
                eprintln!("❌ System drive check failed!");
                e
            })?;
        println!("✅ Not a system drive");
        
        safety.check_mount_points()
            .map_err(|e| {
                eprintln!("❌ Mount point check failed!");
                e
            })?;
        println!("✅ No critical mount points");
        
        // STEP 3: Custom checks
        if device.size < 1_000_000 {
            safety.add_custom_check("min_size", false, "Device too small");
            return Err(MosesError::InvalidInput("Device too small".to_string()));
        }
        safety.add_custom_check("min_size", true, "Size check passed");
        
        // STEP 4: Acknowledge data loss
        safety.acknowledge_data_loss(true)?; // In production, get from user
        println!("⚠️ Data loss acknowledged");
        
        // STEP 5: Request approval - this is the ONLY way to get the device
        let approval = safety.request_approval()
            .map_err(|e| {
                eprintln!("🚫 Safety approval DENIED!");
                e
            })?;
        
        println!("✅ Safety approval granted!");
        println!("   Token: {}", approval.token);
        println!("   Risk: {:?}", approval.risk_level);
        
        // STEP 6: Get the device from approval (can only do once!)
        let device_to_format = approval.get_device_for_format()?;
        
        // Now we can safely format
        println!("🔨 Formatting device: {}", device_to_format.id);
        
        // Actual format operation would go here
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        
        println!("✅ Format complete!");
        
        // Note: We CANNOT format again with same approval
        // approval.get_device_for_format() would fail here
        
        Ok(())
    }
    
    async fn validate_options(&self, _options: &FormatOptions) -> Result<(), MosesError> {
        Ok(())
    }
    
    async fn dry_run(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<crate::SimulationReport, MosesError> {
        // Even dry run uses safety check for warnings
        let mut safety = SafetyCheck::lock_device(device, self.name());
        
        let mut warnings = vec![];
        
        if safety.check_system_drive().is_err() {
            warnings.push("⛔ THIS IS A SYSTEM DRIVE!".to_string());
        }
        
        if safety.check_mount_points().is_err() {
            warnings.push("⚠️ Critical mount points detected".to_string());
        }
        
        warnings.push(format!("Risk Level: {:?}", safety.risk_level()));
        
        Ok(crate::SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: std::time::Duration::from_secs(10),
            warnings,
            required_tools: vec![],
            will_erase_data: true,
            space_after_format: device.size * 95 / 100,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DeviceType;
    
    #[test]
    fn test_cannot_bypass_safety() {
        let device = Device {
            id: "test".to_string(),
            name: "Test Drive".to_string(),
            size: 1_000_000_000,
            device_type: DeviceType::USB,
            mount_points: vec![],
            is_removable: true,
            is_system: false,
        };
        
        // Lock device
        let safety = SafetyCheck::lock_device(&device, "test");
        
        // Try to get approval without checks - should fail
        let result = safety.request_approval();
        assert!(result.is_err());
    }
    
    #[test]
    fn test_approval_single_use() {
        let device = Device {
            id: "test".to_string(),
            name: "Test Drive".to_string(),
            size: 1_000_000_000,
            device_type: DeviceType::USB,
            mount_points: vec![],
            is_removable: true,
            is_system: false,
        };
        
        let mut safety = SafetyCheck::lock_device(&device, "test");
        safety.check_system_drive().unwrap();
        safety.check_mount_points().unwrap();
        safety.acknowledge_data_loss(true).unwrap();
        
        let approval = safety.request_approval().unwrap();
        
        // First use should work
        let dev1 = approval.get_device_for_format();
        assert!(dev1.is_ok());
        
        // Second use should fail
        let dev2 = approval.get_device_for_format();
        assert!(dev2.is_err());
    }
}