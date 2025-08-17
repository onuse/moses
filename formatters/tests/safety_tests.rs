/// Critical safety tests for formatters
/// These tests ensure that formatters NEVER format system drives or critical devices

#[cfg(test)]
mod safety_tests {
    use moses_core::{Device, DeviceType, FilesystemFormatter, FormatOptions};
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// Helper to create a system drive that should NEVER be formatted
    fn create_system_drive() -> Device {
        Device {
            id: r"\\.\PHYSICALDRIVE0".to_string(),
            name: "System Drive".to_string(),
            size: 500 * 1_073_741_824,
            device_type: DeviceType::SSD,
            mount_points: vec![PathBuf::from("C:\\")],
            is_removable: false,
            is_system: true,
        filesystem: None,
        }
    }

    /// Helper to create a safe USB drive
    fn create_safe_usb() -> Device {
        Device {
            id: r"\\.\PHYSICALDRIVE2".to_string(),
            name: "USB Drive".to_string(),
            size: 16 * 1_073_741_824,
            device_type: DeviceType::USB,
            mount_points: vec![],
            is_removable: true,
            is_system: false,
        filesystem: None,
        }
    }

    /// Create format options for testing
    fn create_format_options() -> FormatOptions {
        FormatOptions {
            filesystem_type: "ext4".to_string(),
            label: Some("TestDrive".to_string()),
            quick_format: true,
            cluster_size: None,
            enable_compression: false,
            verify_after_format: false,
            additional_options: HashMap::new(),
        }
    }

    #[test]
    fn test_ext4_formatter_refuses_system_drive() {
        let formatter = moses_formatters::Ext4Formatter;
        let system_drive = create_system_drive();
        
        // The formatter should refuse to format system drives
        assert!(!formatter.can_format(&system_drive), 
            "CRITICAL: Formatter claims it can format a system drive!");
    }

    #[test]
    fn test_ntfs_formatter_refuses_system_drive() {
        let formatter = moses_formatters::NtfsFormatter;
        let system_drive = create_system_drive();
        
        assert!(!formatter.can_format(&system_drive),
            "CRITICAL: NTFS formatter claims it can format a system drive!");
    }

    #[test]
    fn test_formatters_allow_safe_usb() {
        let ext4_formatter = moses_formatters::Ext4Formatter;
        let ntfs_formatter = moses_formatters::NtfsFormatter;
        let safe_usb = create_safe_usb();
        
        // Formatters should allow formatting safe USB drives
        assert!(ext4_formatter.can_format(&safe_usb),
            "Formatter refuses to format a safe USB drive");
        assert!(ntfs_formatter.can_format(&safe_usb),
            "NTFS formatter refuses to format a safe USB drive");
    }

    #[tokio::test]
    async fn test_dry_run_warns_on_system_drive() {
        let formatter = moses_formatters::Ext4Formatter;
        let system_drive = create_system_drive();
        let options = create_format_options();
        
        let result = formatter.dry_run(&system_drive, &options).await;
        
        match result {
            Ok(report) => {
                // Should have warnings about system drive
                assert!(!report.warnings.is_empty(),
                    "No warnings generated for system drive dry run!");
                
                // Should indicate data will be erased
                assert!(report.will_erase_data,
                    "Dry run doesn't indicate data erasure!");
            },
            Err(_) => {
                // It's also acceptable to error on system drive dry run
            }
        }
    }

    #[test]
    fn test_critical_mount_points_blocked() {
        let formatter = moses_formatters::Ext4Formatter;
        
        // Test various critical mount points
        let critical_mounts = vec![
            PathBuf::from("C:\\"),
            PathBuf::from("C:\\Windows"),
            PathBuf::from("/"),
            PathBuf::from("/boot"),
            PathBuf::from("/System"),
        ];
        
        for mount in critical_mounts {
            let device = Device {
                id: "test".to_string(),
                name: "Test Drive".to_string(),
                size: 100 * 1_073_741_824,
                device_type: DeviceType::HardDisk,
                mount_points: vec![mount.clone()],
                is_removable: false,
                is_system: false,
        filesystem: None,
            };
            
            // Even if not marked as system, critical mount points should be protected
            assert!(!formatter.can_format(&device),
                "CRITICAL: Formatter allows formatting drive mounted at {:?}!", mount);
        }
    }

    #[test]
    fn test_label_validation() {
        use moses_core::test_utils::SafetyValidator;
        
        // Valid labels
        let valid_options = vec![
            FormatOptions {
                filesystem_type: "ext4".to_string(),
                label: Some("MyDrive".to_string()),
                quick_format: true,
                cluster_size: None,
                enable_compression: false,
                verify_after_format: false,
                additional_options: HashMap::new(),
            },
            FormatOptions {
                filesystem_type: "ntfs".to_string(),
                label: Some("USB_2024".to_string()),
                quick_format: false,
                cluster_size: Some(4096),
                enable_compression: false,
                verify_after_format: false,
                additional_options: HashMap::new(),
            },
        ];
        
        for options in valid_options {
            assert!(SafetyValidator::validate_format_options(&options).is_ok(),
                "Valid options rejected: {:?}", options);
        }
        
        // Invalid labels
        let invalid_options = vec![
            FormatOptions {
                filesystem_type: "ext4".to_string(),
                label: Some("Invalid/Label".to_string()), // Contains slash
                quick_format: true,
                cluster_size: None,
                enable_compression: false,
                verify_after_format: false,
                additional_options: HashMap::new(),
            },
            FormatOptions {
                filesystem_type: "ntfs".to_string(),
                label: Some("A".repeat(50)), // Too long
                quick_format: true,
                cluster_size: None,
                enable_compression: false,
                verify_after_format: false,
                additional_options: HashMap::new(),
            },
            FormatOptions {
                filesystem_type: "".to_string(), // Empty filesystem type
                label: Some("Valid".to_string()),
                quick_format: true,
                cluster_size: None,
                enable_compression: false,
                verify_after_format: false,
                additional_options: HashMap::new(),
            },
        ];
        
        for options in invalid_options {
            assert!(SafetyValidator::validate_format_options(&options).is_err(),
                "Invalid options accepted: {:?}", options);
        }
    }

    #[test]
    fn test_device_size_validation() {
        use moses_core::test_utils::SafetyValidator;
        
        // Zero size device
        let zero_size = Device {
            id: "test".to_string(),
            name: "Zero Size".to_string(),
            size: 0,
            device_type: DeviceType::USB,
            mount_points: vec![],
            is_removable: true,
            is_system: false,
        filesystem: None,
        };
        
        assert!(SafetyValidator::validate_device_safety(&zero_size).is_err(),
            "Zero size device accepted!");
        
        // Unreasonably large device (> 100TB)
        let huge_device = Device {
            id: "test".to_string(),
            name: "Huge Device".to_string(),
            size: 200 * 1024_u64.pow(4), // 200 TB
            device_type: DeviceType::HardDisk,
            mount_points: vec![],
            is_removable: false,
            is_system: false,
        filesystem: None,
        };
        
        assert!(SafetyValidator::validate_device_safety(&huge_device).is_err(),
            "Unreasonably large device accepted!");
        
        // Normal size device
        let normal_device = Device {
            id: "test".to_string(),
            name: "Normal Device".to_string(),
            size: 1 * 1024_u64.pow(4), // 1 TB
            device_type: DeviceType::HardDisk,
            mount_points: vec![],
            is_removable: false,
            is_system: false,
        filesystem: None,
        };
        
        assert!(SafetyValidator::validate_device_safety(&normal_device).is_ok(),
            "Normal size device rejected!");
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn test_windows_ext4_native_safety() {
        use moses_formatters::Ext4NativeFormatter;
        
        let formatter = Ext4NativeFormatter;
        
        // Should not format system drive with native implementation
        let system_drive = create_system_drive();
        assert!(!formatter.can_format(&system_drive),
            "CRITICAL: Windows EXT4 formatter allows system drive!");
        
        // Should allow safe USB
        let safe_usb = create_safe_usb();
        assert!(formatter.can_format(&safe_usb),
            "Windows EXT4 formatter rejects safe USB!");
        
        // Test dry run with native implementation
        let options = create_format_options();
        let result = formatter.dry_run(&safe_usb, &options).await;
        
        if let Ok(report) = result {
            // Native implementation should not require external tools
            assert!(report.required_tools.is_empty() || !report.required_tools.iter().any(|t| t.contains("WSL")),
                "Native EXT4 formatter should not require WSL!");
        }
    }
}