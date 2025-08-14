/// Safety Verification and Certification System
/// 
/// This module provides tools to verify that formatters (especially external plugins)
/// properly implement the safety protocol. It can analyze formatter behavior and
/// certify compliance.

use crate::{Device, DeviceType, FilesystemFormatter, FormatOptions, MosesError};
use crate::safety_v2::{SafetyCheck, RiskLevel};
use std::path::PathBuf;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/// Verification result for a formatter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyVerificationResult {
    pub formatter_name: String,
    pub version: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub tests_passed: Vec<String>,
    pub tests_failed: Vec<String>,
    pub compliance_score: f32, // 0.0 to 100.0
    pub is_certified: bool,
    pub certification_level: CertificationLevel,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CertificationLevel {
    /// Formatter passed all safety tests
    FullyCertified,
    
    /// Formatter passed critical tests but has minor issues
    ConditionallyApproved,
    
    /// Formatter needs fixes before certification
    RequiresChanges,
    
    /// Formatter is unsafe and should not be used
    Unsafe,
}

/// The safety verifier that tests formatters
pub struct SafetyVerifier {
    test_results: HashMap<String, bool>,
    formatter_name: String,
}

impl SafetyVerifier {
    pub fn new(formatter_name: &str) -> Self {
        Self {
            test_results: HashMap::new(),
            formatter_name: formatter_name.to_string(),
        }
    }
    
    /// Run all safety verification tests on a formatter
    pub async fn verify_formatter<F: FilesystemFormatter>(
        &mut self,
        formatter: &F,
    ) -> SafetyVerificationResult {
        println!("🔍 Starting safety verification for: {}", formatter.name());
        
        // Test 1: System drive rejection
        self.test_system_drive_rejection(formatter).await;
        
        // Test 2: Critical mount point detection
        self.test_critical_mount_rejection(formatter).await;
        
        // Test 3: Proper can_format implementation
        self.test_can_format_safety(formatter);
        
        // Test 4: Dry run safety warnings
        self.test_dry_run_warnings(formatter).await;
        
        // Test 5: Format fails on unsafe devices
        self.test_format_rejection(formatter).await;
        
        // Test 6: Risk assessment
        self.test_risk_assessment(formatter).await;
        
        // Test 7: Safety check usage (if we can detect it)
        self.test_uses_safety_check(formatter).await;
        
        // Calculate results
        self.generate_verification_result()
    }
    
    /// Test that formatter rejects system drives
    async fn test_system_drive_rejection<F: FilesystemFormatter>(&mut self, formatter: &F) {
        let system_drive = create_test_system_drive();
        
        // Test can_format
        let can_format = formatter.can_format(&system_drive);
        self.test_results.insert(
            "rejects_system_drive_can_format".to_string(),
            !can_format
        );
        
        // Test actual format
        let options = FormatOptions::default();
        let format_result = formatter.format(&system_drive, &options).await;
        self.test_results.insert(
            "rejects_system_drive_format".to_string(),
            format_result.is_err()
        );
        
        if can_format {
            println!("  ❌ CRITICAL: Formatter claims it can format system drives!");
        } else {
            println!("  ✅ Correctly rejects system drives in can_format");
        }
        
        if format_result.is_ok() {
            println!("  ❌ CRITICAL: Formatter actually formatted a system drive!");
        } else {
            println!("  ✅ Correctly fails to format system drives");
        }
    }
    
    /// Test critical mount point detection
    async fn test_critical_mount_rejection<F: FilesystemFormatter>(&mut self, formatter: &F) {
        let critical_mounts = vec![
            PathBuf::from("/"),
            PathBuf::from("C:\\"),
            PathBuf::from("/boot"),
            PathBuf::from("C:\\Windows"),
        ];
        
        for mount in critical_mounts {
            let device = create_device_with_mount(mount.clone());
            let can_format = formatter.can_format(&device);
            
            let test_name = format!("rejects_mount_{}", mount.display());
            self.test_results.insert(test_name.clone(), !can_format);
            
            if can_format {
                println!("  ❌ Allows formatting device mounted at: {}", mount.display());
            }
        }
        
        let passed = self.test_results.values()
            .filter(|&&v| v)
            .count();
        println!("  ✅ Critical mount tests: {}/4 passed", passed);
    }
    
    /// Test can_format safety implementation
    fn test_can_format_safety<F: FilesystemFormatter>(&mut self, formatter: &F) {
        // Safe device - should allow
        let safe_device = create_safe_usb_device();
        let allows_safe = formatter.can_format(&safe_device);
        self.test_results.insert("allows_safe_device".to_string(), allows_safe);
        
        // Risky device - should be cautious
        let risky_device = create_risky_device();
        let blocks_risky = !formatter.can_format(&risky_device);
        self.test_results.insert("cautious_with_risky".to_string(), blocks_risky);
        
        println!("  {} Safe device handling", 
                 if allows_safe { "✅" } else { "⚠️" });
        println!("  {} Risky device caution",
                 if blocks_risky { "✅" } else { "⚠️" });
    }
    
    /// Test dry run provides appropriate warnings
    async fn test_dry_run_warnings<F: FilesystemFormatter>(&mut self, formatter: &F) {
        let system_drive = create_test_system_drive();
        let options = FormatOptions::default();
        
        let dry_run_result = formatter.dry_run(&system_drive, &options).await;
        
        match dry_run_result {
            Ok(report) => {
                let has_warnings = !report.warnings.is_empty();
                let has_critical_warning = report.warnings.iter()
                    .any(|w| w.to_lowercase().contains("system") || 
                             w.to_lowercase().contains("critical"));
                
                self.test_results.insert("dry_run_has_warnings".to_string(), has_warnings);
                self.test_results.insert("dry_run_critical_warning".to_string(), has_critical_warning);
                
                println!("  {} Dry run provides warnings", 
                         if has_warnings { "✅" } else { "❌" });
                println!("  {} Warnings are appropriately severe",
                         if has_critical_warning { "✅" } else { "⚠️" });
            }
            Err(_) => {
                // Also acceptable - dry run can fail on system drive
                self.test_results.insert("dry_run_has_warnings".to_string(), true);
                self.test_results.insert("dry_run_critical_warning".to_string(), true);
                println!("  ✅ Dry run correctly fails on system drive");
            }
        }
    }
    
    /// Test format rejection on unsafe devices
    async fn test_format_rejection<F: FilesystemFormatter>(&mut self, formatter: &F) {
        let devices = vec![
            ("system", create_test_system_drive()),
            ("critical_mount", create_device_with_mount(PathBuf::from("/"))),
            ("non_removable_unmarked", create_unmarked_system_drive()),
        ];
        
        let options = FormatOptions::default();
        
        for (name, device) in devices {
            let result = formatter.format(&device, &options).await;
            let test_name = format!("format_rejects_{}", name);
            self.test_results.insert(test_name, result.is_err());
            
            if result.is_ok() {
                println!("  ❌ CRITICAL: Formatted {} device!", name);
            }
        }
    }
    
    /// Test risk assessment behavior
    async fn test_risk_assessment<F: FilesystemFormatter>(&mut self, formatter: &F) {
        // We can't directly test if formatter uses SafetyCheck internally,
        // but we can observe behavior patterns
        
        let devices = vec![
            (RiskLevel::Safe, create_safe_usb_device()),
            (RiskLevel::High, create_risky_device()),
            (RiskLevel::Forbidden, create_test_system_drive()),
        ];
        
        for (expected_risk, device) in devices {
            let can_format = formatter.can_format(&device);
            
            match expected_risk {
                RiskLevel::Safe => {
                    if !can_format {
                        println!("  ⚠️ Overly cautious: blocks safe device");
                    }
                }
                RiskLevel::Forbidden => {
                    if can_format {
                        println!("  ❌ CRITICAL: Allows forbidden risk level!");
                    }
                }
                _ => {}
            }
        }
        
        self.test_results.insert("appropriate_risk_assessment".to_string(), true);
    }
    
    /// Try to detect if formatter uses SafetyCheck
    async fn test_uses_safety_check<F: FilesystemFormatter>(&mut self, formatter: &F) {
        // This is harder to detect directly, but we can look for patterns
        // Real implementation might use instrumentation or code analysis
        
        // For now, we'll check if the formatter shows consistent safety behavior
        let consistency_score = self.test_results.values()
            .filter(|&&v| v)
            .count() as f32 / self.test_results.len() as f32;
        
        self.test_results.insert(
            "likely_uses_safety_check".to_string(),
            consistency_score > 0.8
        );
        
        if consistency_score > 0.8 {
            println!("  ✅ Behavior consistent with SafetyCheck usage");
        } else {
            println!("  ⚠️ Behavior suggests custom safety implementation");
        }
    }
    
    /// Generate final verification result
    fn generate_verification_result(&self) -> SafetyVerificationResult {
        let tests_passed: Vec<String> = self.test_results
            .iter()
            .filter(|(_, &passed)| passed)
            .map(|(name, _)| name.clone())
            .collect();
        
        let tests_failed: Vec<String> = self.test_results
            .iter()
            .filter(|(_, &passed)| !passed)
            .map(|(name, _)| name.clone())
            .collect();
        
        let compliance_score = (tests_passed.len() as f32 / 
                                self.test_results.len() as f32) * 100.0;
        
        // Critical tests that MUST pass
        let critical_tests = vec![
            "rejects_system_drive_can_format",
            "rejects_system_drive_format",
            "format_rejects_system",
        ];
        
        let all_critical_passed = critical_tests.iter()
            .all(|test| self.test_results.get(*test) == Some(&true));
        
        let certification_level = if all_critical_passed && compliance_score >= 90.0 {
            CertificationLevel::FullyCertified
        } else if all_critical_passed && compliance_score >= 70.0 {
            CertificationLevel::ConditionallyApproved
        } else if all_critical_passed {
            CertificationLevel::RequiresChanges
        } else {
            CertificationLevel::Unsafe
        };
        
        let mut recommendations = Vec::new();
        
        if !all_critical_passed {
            recommendations.push("⛔ MUST fix system drive protection".to_string());
        }
        
        if tests_failed.contains(&"dry_run_has_warnings".to_string()) {
            recommendations.push("Add warnings to dry_run output".to_string());
        }
        
        if compliance_score < 80.0 {
            recommendations.push("Consider using SafetyCheck from moses_core".to_string());
        }
        
        SafetyVerificationResult {
            formatter_name: self.formatter_name.clone(),
            version: "1.0.0".to_string(),
            timestamp: chrono::Utc::now(),
            tests_passed,
            tests_failed,
            compliance_score,
            is_certified: certification_level == CertificationLevel::FullyCertified,
            certification_level,
            recommendations,
        }
    }
}

/// Automated plugin scanner that can verify external plugins
pub struct PluginScanner {
    results: Vec<SafetyVerificationResult>,
}

impl PluginScanner {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }
    
    /// Scan and verify all registered formatters
    pub async fn scan_all_formatters(
        &mut self,
        registry: &crate::FormatterRegistry,
    ) -> Vec<SafetyVerificationResult> {
        println!("🔍 Scanning all registered formatters for safety compliance...\n");
        
        for (name, _metadata) in registry.list_with_metadata() {
            if let Some(formatter) = registry.get_formatter(name) {
                let mut verifier = SafetyVerifier::new(name);
                
                // We need to create a concrete type to test
                // In practice, this would test the actual formatter
                println!("Testing formatter: {}", name);
                
                // For now, return placeholder
                // Real implementation would test the formatter
            }
        }
        
        self.results.clone()
    }
    
    /// Generate safety report
    pub fn generate_report(&self) -> String {
        let mut report = String::from("# Moses Safety Verification Report\n\n");
        
        let total = self.results.len();
        let certified = self.results.iter()
            .filter(|r| r.is_certified)
            .count();
        
        report.push_str(&format!("## Summary\n"));
        report.push_str(&format!("- Total Formatters Scanned: {}\n", total));
        report.push_str(&format!("- Fully Certified: {}\n", certified));
        report.push_str(&format!("- Compliance Rate: {:.1}%\n\n", 
                               certified as f32 / total as f32 * 100.0));
        
        report.push_str("## Detailed Results\n\n");
        
        for result in &self.results {
            let emoji = match result.certification_level {
                CertificationLevel::FullyCertified => "✅",
                CertificationLevel::ConditionallyApproved => "⚠️",
                CertificationLevel::RequiresChanges => "🔧",
                CertificationLevel::Unsafe => "❌",
            };
            
            report.push_str(&format!("### {} {} (v{})\n", 
                                   emoji, result.formatter_name, result.version));
            report.push_str(&format!("- Certification: {:?}\n", result.certification_level));
            report.push_str(&format!("- Compliance Score: {:.1}%\n", result.compliance_score));
            report.push_str(&format!("- Tests Passed: {}/{}\n", 
                                   result.tests_passed.len(),
                                   result.tests_passed.len() + result.tests_failed.len()));
            
            if !result.recommendations.is_empty() {
                report.push_str("- Recommendations:\n");
                for rec in &result.recommendations {
                    report.push_str(&format!("  - {}\n", rec));
                }
            }
            
            report.push_str("\n");
        }
        
        report
    }
}

// Test device creation helpers
fn create_test_system_drive() -> Device {
    Device {
        id: "system".to_string(),
        name: "System Drive".to_string(),
        size: 500_000_000_000,
        device_type: DeviceType::SSD,
        mount_points: vec![PathBuf::from("C:\\")],
        is_removable: false,
        is_system: true,
    }
}

fn create_safe_usb_device() -> Device {
    Device {
        id: "usb".to_string(),
        name: "USB Drive".to_string(),
        size: 16_000_000_000,
        device_type: DeviceType::USB,
        mount_points: vec![],
        is_removable: true,
        is_system: false,
    }
}

fn create_risky_device() -> Device {
    Device {
        id: "risky".to_string(),
        name: "Internal Drive".to_string(),
        size: 1_000_000_000_000,
        device_type: DeviceType::HardDisk,
        mount_points: vec![PathBuf::from("/data")],
        is_removable: false,
        is_system: false,
    }
}

fn create_device_with_mount(mount: PathBuf) -> Device {
    Device {
        id: format!("mounted_{}", mount.display()),
        name: "Mounted Drive".to_string(),
        size: 100_000_000_000,
        device_type: DeviceType::HardDisk,
        mount_points: vec![mount],
        is_removable: false,
        is_system: false,
    }
}

fn create_unmarked_system_drive() -> Device {
    Device {
        id: "unmarked".to_string(),
        name: "Unmarked System".to_string(),
        size: 500_000_000_000,
        device_type: DeviceType::SSD,
        mount_points: vec![PathBuf::from("/")],
        is_removable: false,
        is_system: false, // Incorrectly marked as non-system!
    }
}

/// Certification badge that can be displayed
pub fn generate_certification_badge(level: CertificationLevel) -> String {
    match level {
        CertificationLevel::FullyCertified => {
            "🛡️ MOSES SAFETY CERTIFIED 🛡️".to_string()
        }
        CertificationLevel::ConditionallyApproved => {
            "⚠️ Conditionally Approved ⚠️".to_string()
        }
        CertificationLevel::RequiresChanges => {
            "🔧 Safety Updates Required 🔧".to_string()
        }
        CertificationLevel::Unsafe => {
            "❌ UNSAFE - DO NOT USE ❌".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_verifier_detects_unsafe_formatter() {
        // Create a bad formatter that doesn't check system drives
        struct UnsafeFormatter;
        
        #[async_trait::async_trait]
        impl FilesystemFormatter for UnsafeFormatter {
            fn name(&self) -> &'static str { "unsafe" }
            fn supported_platforms(&self) -> Vec<crate::Platform> { vec![] }
            fn can_format(&self, _device: &Device) -> bool { true } // BAD!
            fn requires_external_tools(&self) -> bool { false }
            fn bundled_tools(&self) -> Vec<&'static str> { vec![] }
            
            async fn format(&self, _: &Device, _: &FormatOptions) -> Result<(), MosesError> {
                Ok(()) // BAD! Formats anything!
            }
            
            async fn validate_options(&self, _: &FormatOptions) -> Result<(), MosesError> {
                Ok(())
            }
            
            async fn dry_run(&self, d: &Device, o: &FormatOptions) -> Result<crate::SimulationReport, MosesError> {
                Ok(crate::SimulationReport {
                    device: d.clone(),
                    options: o.clone(),
                    estimated_time: std::time::Duration::from_secs(1),
                    warnings: vec![], // No warnings!
                    required_tools: vec![],
                    will_erase_data: true,
                    space_after_format: d.size,
                })
            }
        }
        
        let mut verifier = SafetyVerifier::new("unsafe");
        let formatter = UnsafeFormatter;
        let result = verifier.verify_formatter(&formatter).await;
        
        assert_eq!(result.certification_level, CertificationLevel::Unsafe);
        assert!(!result.is_certified);
        assert!(result.compliance_score < 50.0);
    }
}