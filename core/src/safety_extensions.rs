//! Extended Safety Features for Moses
//! 
//! This module provides advanced safety features:
//! - Device locking mechanism (from safety_v2)
//! - OS-level verification (from safety_os_integration)  
//! - Formatter certification (from safety_verification)

use crate::{Device, DeviceType, MosesError, FilesystemFormatter, FormatOptions};
use crate::safety::{SafetyCheck, RiskLevel};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid;

// ============================================================================
// DEVICE LOCKING (from safety_v2)
// ============================================================================

/// A locked device that requires safety approval to access
pub struct LockedDevice {
    device: Arc<Device>,
    locked: Arc<Mutex<bool>>,
    safety_token: String,
}

impl LockedDevice {
    pub fn new(device: Device) -> Self {
        Self {
            device: Arc::new(device),
            locked: Arc::new(Mutex::new(true)),
            safety_token: generate_token(),
        }
    }
    
    /// Get device info for read-only operations
    pub fn info(&self) -> &Device {
        &self.device
    }
    
    /// Check if device is still locked
    pub fn is_locked(&self) -> bool {
        *self.locked.lock().unwrap()
    }
    
    /// Unlock with the correct token
    pub fn unlock(&self, token: &str) -> Result<(), MosesError> {
        if token != self.safety_token {
            return Err(MosesError::SafetyViolation(
                "Invalid safety token".to_string()
            ));
        }
        *self.locked.lock().unwrap() = false;
        Ok(())
    }
}

/// Time-limited, single-use safety approval
pub struct SafetyApproval {
    device: Arc<Device>,
    token: String,
    expires_at: DateTime<Utc>,
    used: Arc<Mutex<bool>>,
    approval_id: String,
}

impl SafetyApproval {
    pub fn new(device: Arc<Device>, token: String, duration_minutes: i64) -> Self {
        Self {
            device,
            token,
            expires_at: Utc::now() + chrono::Duration::minutes(duration_minutes),
            used: Arc::new(Mutex::new(false)),
            approval_id: generate_token(),
        }
    }
    
    /// Use the approval (single use only)
    pub fn use_approval(self) -> Result<Arc<Device>, MosesError> {
        let mut used = self.used.lock().unwrap();
        if *used {
            return Err(MosesError::SafetyViolation(
                "Safety approval already used".to_string()
            ));
        }
        
        if Utc::now() > self.expires_at {
            return Err(MosesError::SafetyViolation(
                "Safety approval expired".to_string()
            ));
        }
        
        *used = true;
        Ok(self.device)
    }
    
    pub fn is_valid(&self) -> bool {
        !*self.used.lock().unwrap() && Utc::now() <= self.expires_at
    }
}

// ============================================================================
// OS-LEVEL VERIFICATION (from safety_os_integration)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsVerification {
    pub platform: String,
    pub is_encrypted: bool,
    pub encryption_type: Option<String>,
    pub is_raid_member: bool,
    pub is_lvm_member: bool,
    pub actual_mount_points: Vec<std::path::PathBuf>,
    pub verification_timestamp: DateTime<Utc>,
}

pub struct OsDeviceVerifier {
    cache: Arc<Mutex<HashMap<String, OsVerification>>>,
}

impl OsDeviceVerifier {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Verify device properties using OS APIs
    pub async fn verify_device(&self, device: &Device) -> Result<OsVerification, MosesError> {
        // Check cache first
        if let Some(cached) = self.cache.lock().unwrap().get(&device.id) {
            if cached.verification_timestamp > Utc::now() - chrono::Duration::minutes(5) {
                return Ok(cached.clone());
            }
        }
        
        let verification = self.perform_os_verification(device).await?;
        
        // Cache the result
        self.cache.lock().unwrap().insert(
            device.id.clone(),
            verification.clone()
        );
        
        Ok(verification)
    }
    
    async fn perform_os_verification(&self, device: &Device) -> Result<OsVerification, MosesError> {
        let mut verification = OsVerification {
            platform: std::env::consts::OS.to_string(),
            is_encrypted: false,
            encryption_type: None,
            is_raid_member: false,
            is_lvm_member: false,
            actual_mount_points: device.mount_points.clone(),
            verification_timestamp: Utc::now(),
        };
        
        #[cfg(target_os = "windows")]
        {
            verification = self.verify_windows(device, verification).await?;
        }
        
        #[cfg(target_os = "linux")]
        {
            verification = self.verify_linux(device, verification).await?;
        }
        
        #[cfg(target_os = "macos")]
        {
            verification = self.verify_macos(device, verification).await?;
        }
        
        Ok(verification)
    }
    
    #[cfg(target_os = "windows")]
    async fn verify_windows(&self, device: &Device, mut v: OsVerification) -> Result<OsVerification, MosesError> {
        // Check for BitLocker encryption
        use std::process::Command;
        
        let output = Command::new("manage-bde")
            .args(&["-status", &device.id])
            .output();
        
        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("Encrypted") || stdout.contains("BitLocker") {
                v.is_encrypted = true;
                v.encryption_type = Some("BitLocker".to_string());
            }
        }
        
        Ok(v)
    }
    
    #[cfg(target_os = "linux")]
    async fn verify_linux(&self, device: &Device, mut v: OsVerification) -> Result<OsVerification, MosesError> {
        use std::fs;
        use std::path::Path;
        
        // Check for LUKS encryption
        if let Ok(output) = std::process::Command::new("cryptsetup")
            .args(&["luksDump", &device.id])
            .output()
        {
            if output.status.success() {
                v.is_encrypted = true;
                v.encryption_type = Some("LUKS".to_string());
            }
        }
        
        // Check for LVM membership
        if Path::new("/proc/lvm/VGs").exists() {
            if let Ok(contents) = fs::read_to_string("/proc/mounts") {
                if contents.contains(&device.id) && contents.contains("/dev/mapper") {
                    v.is_lvm_member = true;
                }
            }
        }
        
        // Check for RAID membership
        if let Ok(contents) = fs::read_to_string("/proc/mdstat") {
            if contents.contains(&device.id) {
                v.is_raid_member = true;
            }
        }
        
        Ok(v)
    }
    
    #[cfg(target_os = "macos")]
    async fn verify_macos(&self, device: &Device, mut v: OsVerification) -> Result<OsVerification, MosesError> {
        // Check for FileVault encryption
        if let Ok(output) = std::process::Command::new("diskutil")
            .args(&["info", &device.id])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("FileVault") && stdout.contains("Encrypted") {
                v.is_encrypted = true;
                v.encryption_type = Some("FileVault".to_string());
            }
        }
        
        Ok(v)
    }
}

// ============================================================================
// FORMATTER CERTIFICATION (from safety_verification)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CertificationLevel {
    FullyCertified,
    ConditionallyApproved,
    RequiresChanges,
    Unsafe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificationResult {
    pub formatter_name: String,
    pub timestamp: DateTime<Utc>,
    pub score: f32,
    pub level: CertificationLevel,
    pub tests_passed: usize,
    pub tests_total: usize,
    pub issues: Vec<String>,
    pub recommendations: Vec<String>,
}

pub struct FormatterCertifier {
    results: Vec<CertificationResult>,
}

impl FormatterCertifier {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }
    
    /// Test a formatter for safety compliance
    pub async fn certify_formatter<F: FilesystemFormatter>(
        &mut self,
        formatter: &F,
    ) -> CertificationResult {
        let mut passed = 0;
        let mut total = 0;
        let mut issues = Vec::new();
        let mut recommendations = Vec::new();
        
        // Test 1: Rejects system drives
        total += 1;
        if self.test_system_drive_rejection(formatter).await {
            passed += 1;
        } else {
            issues.push("Does not properly reject system drives".to_string());
            recommendations.push("Implement system drive detection".to_string());
        }
        
        // Test 2: Rejects critical mount points
        total += 1;
        if self.test_critical_mount_rejection(formatter).await {
            passed += 1;
        } else {
            issues.push("Does not properly reject critical mount points".to_string());
            recommendations.push("Check for critical paths like /, /boot, C:\\".to_string());
        }
        
        // Test 3: Supports dry run
        total += 1;
        if self.test_dry_run_support(formatter).await {
            passed += 1;
        } else {
            recommendations.push("Consider implementing dry run support".to_string());
        }
        
        // Test 4: Proper error handling
        total += 1;
        if self.test_error_handling(formatter).await {
            passed += 1;
        } else {
            issues.push("Poor error handling".to_string());
        }
        
        let score = (passed as f32 / total as f32) * 100.0;
        let level = match score {
            s if s >= 100.0 => CertificationLevel::FullyCertified,
            s if s >= 75.0 => CertificationLevel::ConditionallyApproved,
            s if s >= 50.0 => CertificationLevel::RequiresChanges,
            _ => CertificationLevel::Unsafe,
        };
        
        let result = CertificationResult {
            formatter_name: formatter.name().to_string(),
            timestamp: Utc::now(),
            score,
            level,
            tests_passed: passed,
            tests_total: total,
            issues,
            recommendations,
        };
        
        self.results.push(result.clone());
        result
    }
    
    async fn test_system_drive_rejection<F: FilesystemFormatter>(&self, formatter: &F) -> bool {
        let system_device = Device {
            id: "test_system".to_string(),
            name: "System Drive".to_string(),
            size: 500_000_000_000,
            device_type: DeviceType::HardDisk,
            is_removable: false,
            is_system: true,
            mount_points: vec![std::path::PathBuf::from("/")],
            filesystem: Some("ext4".to_string()),
        };
        
        let options = FormatOptions::default();
        formatter.format(&system_device, &options).await.is_err()
    }
    
    async fn test_critical_mount_rejection<F: FilesystemFormatter>(&self, formatter: &F) -> bool {
        let critical_device = Device {
            id: "test_boot".to_string(),
            name: "Boot Drive".to_string(),
            size: 100_000_000_000,
            device_type: DeviceType::HardDisk,
            is_removable: false,
            is_system: false,
            mount_points: vec![std::path::PathBuf::from("/boot")],
            filesystem: Some("ext4".to_string()),
        };
        
        let options = FormatOptions::default();
        formatter.format(&critical_device, &options).await.is_err()
    }
    
    async fn test_dry_run_support<F: FilesystemFormatter>(&self, formatter: &F) -> bool {
        let safe_device = Device {
            id: "test_usb".to_string(),
            name: "USB Drive".to_string(),
            size: 16_000_000_000,
            device_type: DeviceType::USB,
            is_removable: true,
            is_system: false,
            mount_points: vec![],
            filesystem: None,
        };
        
        let mut options = FormatOptions::default();
        options.dry_run = true;
        
        // Dry run should succeed without actually formatting
        formatter.format(&safe_device, &options).await.is_ok()
    }
    
    async fn test_error_handling<F: FilesystemFormatter>(&self, formatter: &F) -> bool {
        let invalid_device = Device {
            id: "/dev/null/invalid".to_string(),
            name: "Invalid".to_string(),
            size: 0,
            device_type: DeviceType::Unknown,
            is_removable: false,
            is_system: false,
            mount_points: vec![],
            filesystem: None,
        };
        
        let options = FormatOptions::default();
        
        // Should return error, not panic
        formatter.format(&invalid_device, &options).await.is_err()
    }
    
    /// Generate a certification report
    pub fn generate_report(&self) -> String {
        let mut report = String::from("# Formatter Safety Certification Report\n\n");
        report.push_str(&format!("Generated: {}\n\n", Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));
        
        for result in &self.results {
            report.push_str(&format!("## {}\n\n", result.formatter_name));
            report.push_str(&format!("- **Score**: {:.1}%\n", result.score));
            report.push_str(&format!("- **Level**: {:?}\n", result.level));
            report.push_str(&format!("- **Tests**: {}/{} passed\n\n", result.tests_passed, result.tests_total));
            
            if !result.issues.is_empty() {
                report.push_str("### Issues\n");
                for issue in &result.issues {
                    report.push_str(&format!("- {}\n", issue));
                }
                report.push_str("\n");
            }
            
            if !result.recommendations.is_empty() {
                report.push_str("### Recommendations\n");
                for rec in &result.recommendations {
                    report.push_str(&format!("- {}\n", rec));
                }
                report.push_str("\n");
            }
        }
        
        report
    }
}

// ============================================================================
// ENHANCED SAFETY MANAGER
// ============================================================================

/// Extended safety manager with all advanced features
pub struct EnhancedSafetyManager {
    os_verifier: OsDeviceVerifier,
    certifier: Arc<Mutex<FormatterCertifier>>,
    locked_devices: Arc<Mutex<HashMap<String, LockedDevice>>>,
}

impl EnhancedSafetyManager {
    pub fn new() -> Self {
        Self {
            os_verifier: OsDeviceVerifier::new(),
            certifier: Arc::new(Mutex::new(FormatterCertifier::new())),
            locked_devices: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Perform comprehensive safety check with OS verification
    pub async fn comprehensive_check(
        &self,
        device: &Device,
        formatter_name: &str,
    ) -> Result<EnhancedSafetyCheck, MosesError> {
        // Standard safety check
        let mut basic_check = SafetyCheck::new(device, formatter_name);
        basic_check.verify_not_system_drive()?;
        basic_check.verify_safe_mount_points()?;
        let risk = basic_check.assess_risk();
        
        // OS-level verification
        let os_verification = self.os_verifier.verify_device(device).await?;
        
        // Adjust risk based on OS verification
        let final_risk = if os_verification.is_encrypted {
            RiskLevel::Critical
        } else if os_verification.is_raid_member || os_verification.is_lvm_member {
            match risk {
                RiskLevel::Safe => RiskLevel::Medium,
                RiskLevel::Low => RiskLevel::High,
                _ => risk,
            }
        } else {
            risk
        };
        
        Ok(EnhancedSafetyCheck {
            basic_check,
            os_verification,
            final_risk,
        })
    }
    
    /// Lock a device and return approval token
    pub fn lock_device(&self, device: Device) -> (String, SafetyApproval) {
        let locked = LockedDevice::new(device.clone());
        let token = locked.safety_token.clone();
        let approval = SafetyApproval::new(
            Arc::new(device.clone()),
            token.clone(),
            5, // 5 minute expiry
        );
        
        self.locked_devices.lock().unwrap().insert(
            device.id.clone(),
            locked
        );
        
        (token, approval)
    }
    
    /// Certify a formatter
    pub async fn certify_formatter<F: FilesystemFormatter>(
        &self,
        formatter: &F,
    ) -> CertificationResult {
        self.certifier.lock().unwrap().certify_formatter(formatter).await
    }
}

#[derive(Debug, Clone)]
pub struct EnhancedSafetyCheck {
    pub basic_check: SafetyCheck,
    pub os_verification: OsVerification,
    pub final_risk: RiskLevel,
}

// Helper function to generate tokens
fn generate_token() -> String {
    uuid::Uuid::new_v4().to_string()
}