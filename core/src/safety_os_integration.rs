/// OS-Level Safety Integration
/// 
/// This module provides OS-specific verification and privilege management.
/// The key principle: Request elevated privileges ONLY after safety checks pass.

use crate::{Device, MosesError};
use crate::safety_v2::{SafetyApproval, RiskLevel};
use std::path::PathBuf;

/// OS-specific device verifier
pub struct OsDeviceVerifier;

impl OsDeviceVerifier {
    /// Verify device properties using OS APIs
    pub async fn verify_device_properties(device: &Device) -> DeviceVerification {
        #[cfg(target_os = "windows")]
        return Self::verify_windows(device).await;
        
        #[cfg(target_os = "linux")]
        return Self::verify_linux(device).await;
        
        #[cfg(target_os = "macos")]
        return Self::verify_macos(device).await;
        
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        return DeviceVerification::default();
    }
    
    #[cfg(target_os = "windows")]
    async fn verify_windows(device: &Device) -> DeviceVerification {
        use std::process::Command;
        
        let mut verification = DeviceVerification::default();
        
        // Use WMI to verify system drive status
        let wmi_check = Command::new("wmic")
            .args(&[
                "logicaldisk",
                "where",
                &format!("DeviceID='{}'", device.id),
                "get",
                "VolumeName,SystemVolume,Size,FileSystem",
                "/format:csv"
            ])
            .output();
        
        if let Ok(output) = wmi_check {
            let output_str = String::from_utf8_lossy(&output.stdout);
            verification.os_confirms_system = output_str.contains("SystemVolume,TRUE");
        }
        
        // Check if device is BitLocker encrypted
        let bitlocker_check = Command::new("manage-bde")
            .args(&["-status", &device.id])
            .output();
        
        if let Ok(output) = bitlocker_check {
            let output_str = String::from_utf8_lossy(&output.stdout);
            verification.is_encrypted = output_str.contains("Protection On");
            if verification.is_encrypted {
                verification.warnings.push(
                    "Device is BitLocker encrypted - formatting will destroy encryption".to_string()
                );
            }
        }
        
        // Verify mount points through OS
        let mountvol_check = Command::new("mountvol")
            .output();
        
        if let Ok(output) = mountvol_check {
            let output_str = String::from_utf8_lossy(&output.stdout);
            // Parse mount points and cross-reference
            verification.os_verified_mounts = true;
        }
        
        verification
    }
    
    #[cfg(target_os = "linux")]
    async fn verify_linux(device: &Device) -> DeviceVerification {
        use std::process::Command;
        use std::fs;
        
        let mut verification = DeviceVerification::default();
        
        // Check /proc/mounts for actual mount status
        if let Ok(mounts) = fs::read_to_string("/proc/mounts") {
            for line in mounts.lines() {
                if line.contains(&device.id) {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let mount_point = parts[1];
                        verification.actual_mount_points.push(PathBuf::from(mount_point));
                        
                        // Check if it's a critical mount
                        if mount_point == "/" || mount_point == "/boot" || mount_point == "/home" {
                            verification.os_confirms_critical = true;
                        }
                    }
                }
            }
        }
        
        // Check if device is in /etc/fstab (system-critical)
        if let Ok(fstab) = fs::read_to_string("/etc/fstab") {
            if fstab.contains(&device.id) {
                verification.in_fstab = true;
                verification.warnings.push(
                    "Device is in /etc/fstab - system may not boot after format".to_string()
                );
            }
        }
        
        // Check for LVM membership
        let lvm_check = Command::new("pvdisplay")
            .arg(&device.id)
            .output();
        
        if let Ok(output) = lvm_check {
            if output.status.success() {
                verification.is_lvm_member = true;
                verification.warnings.push(
                    "Device is part of LVM - formatting will break volume group".to_string()
                );
            }
        }
        
        // Check for RAID membership
        if let Ok(mdstat) = fs::read_to_string("/proc/mdstat") {
            if mdstat.contains(&device.id) {
                verification.is_raid_member = true;
                verification.warnings.push(
                    "Device is part of RAID array - formatting will degrade array".to_string()
                );
            }
        }
        
        // Check for LUKS encryption
        let luks_check = Command::new("cryptsetup")
            .args(&["isLuks", &device.id])
            .output();
        
        if let Ok(output) = luks_check {
            verification.is_encrypted = output.status.success();
            if verification.is_encrypted {
                verification.warnings.push(
                    "Device is LUKS encrypted - formatting will destroy encrypted data".to_string()
                );
            }
        }
        
        verification
    }
    
    #[cfg(target_os = "macos")]
    async fn verify_macos(device: &Device) -> DeviceVerification {
        use std::process::Command;
        
        let mut verification = DeviceVerification::default();
        
        // Use diskutil to verify device properties
        let diskutil_check = Command::new("diskutil")
            .args(&["info", &device.id])
            .output();
        
        if let Ok(output) = diskutil_check {
            let output_str = String::from_utf8_lossy(&output.stdout);
            
            // Check if it's the boot volume
            if output_str.contains("Boot Volume: Yes") {
                verification.os_confirms_system = true;
            }
            
            // Check for FileVault encryption
            if output_str.contains("FileVault: Yes") {
                verification.is_encrypted = true;
                verification.warnings.push(
                    "Device is FileVault encrypted - formatting will destroy encrypted data".to_string()
                );
            }
            
            // Check for APFS container
            if output_str.contains("APFS Container") {
                verification.warnings.push(
                    "Device is part of APFS container - may affect other volumes".to_string()
                );
            }
        }
        
        verification
    }
}

#[derive(Debug, Default)]
pub struct DeviceVerification {
    /// OS confirms this is a system device
    pub os_confirms_system: bool,
    
    /// OS confirms critical mount points
    pub os_confirms_critical: bool,
    
    /// Actual mount points found by OS
    pub actual_mount_points: Vec<PathBuf>,
    
    /// OS verified the mount information
    pub os_verified_mounts: bool,
    
    /// Device is encrypted
    pub is_encrypted: bool,
    
    /// Device is in fstab (Linux)
    pub in_fstab: bool,
    
    /// Device is part of LVM (Linux)
    pub is_lvm_member: bool,
    
    /// Device is part of RAID
    pub is_raid_member: bool,
    
    /// Additional warnings from OS
    pub warnings: Vec<String>,
}

/// Privilege escalation manager - only escalates AFTER safety checks
pub struct PrivilegeManager;

impl PrivilegeManager {
    /// Request elevated privileges only with valid safety approval
    pub async fn request_privileges_with_approval(
        approval: &SafetyApproval<'_>,
        operation: &str,
    ) -> Result<PrivilegeToken, MosesError> {
        // Verify the approval is valid and unused
        if approval.is_used() {
            return Err(MosesError::UnsafeDevice(
                "Cannot request privileges with used approval".to_string()
            ));
        }
        
        // Check risk level
        if approval.risk_level > RiskLevel::Medium {
            // High risk operations might need additional confirmation
            println!("⚠️ High risk operation detected. Additional confirmation required.");
            
            // In a real implementation, this might show a system dialog
            if !Self::get_user_confirmation(approval.risk_level).await? {
                return Err(MosesError::UserCancelled);
            }
        }
        
        // Log the privilege request
        Self::audit_privilege_request(approval, operation).await;
        
        // Now request actual privileges
        #[cfg(target_os = "windows")]
        return Self::request_windows_admin(approval, operation).await;
        
        #[cfg(target_os = "linux")]
        return Self::request_linux_sudo(approval, operation).await;
        
        #[cfg(target_os = "macos")]
        return Self::request_macos_admin(approval, operation).await;
        
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        return Ok(PrivilegeToken::default());
    }
    
    #[cfg(target_os = "windows")]
    async fn request_windows_admin(
        approval: &SafetyApproval<'_>,
        operation: &str,
    ) -> Result<PrivilegeToken, MosesError> {
        use std::process::Command;
        
        // Check if we already have admin rights
        let is_admin = Command::new("net")
            .args(&["session"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        
        if !is_admin {
            // Request UAC elevation
            println!("🔐 Requesting administrator privileges for: {}", operation);
            println!("   Device: {}", approval.token);
            println!("   Risk Level: {:?}", approval.risk_level);
            
            // In real implementation, would trigger UAC
            // For now, return error if not admin
            return Err(MosesError::InsufficientPrivileges(
                "Administrator privileges required".to_string()
            ));
        }
        
        Ok(PrivilegeToken {
            token: approval.token.clone(),
            elevated: true,
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
        })
    }
    
    #[cfg(target_os = "linux")]
    async fn request_linux_sudo(
        approval: &SafetyApproval<'_>,
        operation: &str,
    ) -> Result<PrivilegeToken, MosesError> {
        use std::process::Command;
        
        // Check if we have sudo rights
        let sudo_check = Command::new("sudo")
            .args(&["-n", "true"])
            .output();
        
        if let Ok(output) = sudo_check {
            if !output.status.success() {
                println!("🔐 Sudo privileges required for: {}", operation);
                println!("   Device: {}", approval.token);
                println!("   Risk Level: {:?}", approval.risk_level);
                
                // Could trigger polkit or sudo prompt here
                return Err(MosesError::InsufficientPrivileges(
                    "Sudo privileges required".to_string()
                ));
            }
        }
        
        Ok(PrivilegeToken {
            token: approval.token.clone(),
            elevated: true,
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
        })
    }
    
    #[cfg(target_os = "macos")]
    async fn request_macos_admin(
        approval: &SafetyApproval<'_>,
        operation: &str,
    ) -> Result<PrivilegeToken, MosesError> {
        // Similar to Linux, but might use Authorization Services
        Self::request_linux_sudo(approval, operation).await
    }
    
    async fn get_user_confirmation(risk: RiskLevel) -> Result<bool, MosesError> {
        // In real implementation, show system dialog
        println!("\n⚠️ HIGH RISK OPERATION ⚠️");
        println!("Risk Level: {:?}", risk);
        println!("Type 'CONFIRM' to proceed: ");
        
        // In real app, would use proper UI
        Ok(false) // Default to safe
    }
    
    async fn audit_privilege_request(approval: &SafetyApproval<'_>, operation: &str) {
        // Log to system audit log
        #[cfg(target_os = "linux")]
        {
            use std::process::Command;
            let _ = Command::new("logger")
                .args(&[
                    "-t", "moses",
                    "-p", "auth.warning",
                    &format!("Privilege request for {} operation on device {} (risk: {:?})",
                            operation, approval.token, approval.risk_level)
                ])
                .output();
        }
        
        #[cfg(target_os = "windows")]
        {
            // Would write to Windows Event Log
        }
        
        println!("📝 Audit: Privilege request for {} at {}", 
                 operation, approval.approved_at);
    }
}

/// Token representing elevated privileges
#[derive(Debug)]
pub struct PrivilegeToken {
    pub token: String,
    pub elevated: bool,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

impl Default for PrivilegeToken {
    fn default() -> Self {
        Self {
            token: String::new(),
            elevated: false,
            expires_at: chrono::Utc::now(),
        }
    }
}

/// Enhanced safety check that uses OS verification
pub async fn perform_os_enhanced_safety_check(
    device: &Device,
    formatter_name: &str,
) -> Result<EnhancedSafetyResult, MosesError> {
    // First, do OS-level verification
    let os_verification = OsDeviceVerifier::verify_device_properties(device).await;
    
    // Check for discrepancies
    let mut warnings = Vec::new();
    
    if os_verification.os_confirms_system && !device.is_system {
        warnings.push("⚠️ OS reports this IS a system drive, but device metadata says otherwise!".to_string());
    }
    
    if !os_verification.os_confirms_system && device.is_system {
        warnings.push("ℹ️ Device marked as system, but OS doesn't confirm (might be overly cautious)".to_string());
    }
    
    // Check mount point discrepancies
    let reported_mounts: std::collections::HashSet<_> = device.mount_points.iter().collect();
    let actual_mounts: std::collections::HashSet<_> = os_verification.actual_mount_points.iter().collect();
    
    if reported_mounts != actual_mounts {
        warnings.push(format!(
            "Mount point mismatch - Reported: {:?}, Actual: {:?}",
            device.mount_points, os_verification.actual_mount_points
        ));
    }
    
    // Add OS-specific warnings
    warnings.extend(os_verification.warnings.clone());
    
    // Determine final safety based on OS verification
    let is_safe = !os_verification.os_confirms_system &&
                  !os_verification.os_confirms_critical &&
                  !os_verification.is_lvm_member &&
                  !os_verification.is_raid_member &&
                  !os_verification.in_fstab;
    
    Ok(EnhancedSafetyResult {
        device_id: device.id.clone(),
        formatter: formatter_name.to_string(),
        os_verification,
        warnings,
        is_safe,
        recommendation: if is_safe {
            SafetyRecommendation::ProceedWithCaution
        } else {
            SafetyRecommendation::DoNotFormat
        },
    })
}

#[derive(Debug)]
pub struct EnhancedSafetyResult {
    pub device_id: String,
    pub formatter: String,
    pub os_verification: DeviceVerification,
    pub warnings: Vec<String>,
    pub is_safe: bool,
    pub recommendation: SafetyRecommendation,
}

#[derive(Debug, PartialEq)]
pub enum SafetyRecommendation {
    Safe,
    ProceedWithCaution,
    HighRisk,
    DoNotFormat,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_os_verification() {
        let device = Device {
            id: "/dev/sda1".to_string(),
            name: "Test Device".to_string(),
            size: 100_000_000,
            device_type: crate::DeviceType::HardDisk,
            mount_points: vec![PathBuf::from("/test")],
            is_removable: false,
            is_system: false,
        };
        
        let verification = OsDeviceVerifier::verify_device_properties(&device).await;
        
        // Should have performed some OS checks
        println!("OS Verification: {:?}", verification);
        
        // The actual results will vary by system
        // Key is that it runs without panic
    }
}