/// Tests for platform-specific device enumeration
/// Ensures devices are properly detected and categorized

#[cfg(test)]
mod device_enumeration_tests {
    use moses_core::{DeviceManager, DeviceType};
    use moses_platform::PlatformDeviceManager;

    #[tokio::test]
    async fn test_device_enumeration_returns_devices() {
        let manager = PlatformDeviceManager;
        let result = manager.enumerate_devices().await;
        
        assert!(result.is_ok(), "Device enumeration failed: {:?}", result);
        
        let devices = result.unwrap();
        // We should have at least one device (system drive)
        assert!(!devices.is_empty(), "No devices found!");
        
        // Log devices for debugging
        for device in &devices {
            println!("Found device: {} ({:?})", device.name, device.device_type);
        }
    }

    #[tokio::test]
    async fn test_system_drive_detected() {
        let manager = PlatformDeviceManager;
        let devices = manager.enumerate_devices().await.unwrap();
        
        // Should have at least one system drive
        let system_drives: Vec<_> = devices.iter()
            .filter(|d| d.is_system)
            .collect();
        
        assert!(!system_drives.is_empty(), 
            "No system drives detected! This is a critical safety issue!");
        
        // System drive should not be removable
        for drive in system_drives {
            assert!(!drive.is_removable,
                "System drive {} is marked as removable!", drive.name);
        }
    }

    #[tokio::test]
    async fn test_device_type_classification() {
        let manager = PlatformDeviceManager;
        let devices = manager.enumerate_devices().await.unwrap();
        
        for device in devices {
            // Device type should not be Unknown for real devices
            if !device.id.starts_with("mock://") {
                assert_ne!(device.device_type, DeviceType::Unknown,
                    "Device {} has Unknown type!", device.name);
            }
            
            // USB devices should be removable
            if device.device_type == DeviceType::USB {
                assert!(device.is_removable,
                    "USB device {} is not marked as removable!", device.name);
            }
        }
    }

    #[tokio::test]
    async fn test_device_info_retrieval() {
        let manager = PlatformDeviceManager;
        let devices = manager.enumerate_devices().await.unwrap();
        
        if let Some(first_device) = devices.first() {
            let info_result = manager.get_device_info(first_device).await;
            
            assert!(info_result.is_ok(),
                "Failed to get device info: {:?}", info_result);
            
            let info = info_result.unwrap();
            assert_eq!(info.device.id, first_device.id,
                "Device info mismatch!");
        }
    }

    #[tokio::test]
    async fn test_permission_checking() {
        let manager = PlatformDeviceManager;
        let devices = manager.enumerate_devices().await.unwrap();
        
        if let Some(device) = devices.first() {
            let perm_result = manager.check_permissions(device).await;
            
            assert!(perm_result.is_ok(),
                "Failed to check permissions: {:?}", perm_result);
            
            let perms = perm_result.unwrap();
            println!("Permission level for {}: {:?}", device.name, perms);
        }
    }

    #[tokio::test]
    async fn test_safety_check_blocks_system_drives() {
        let manager = PlatformDeviceManager;
        let devices = manager.enumerate_devices().await.unwrap();
        
        // Find system drives
        let system_drives: Vec<_> = devices.iter()
            .filter(|d| d.is_system)
            .collect();
        
        for system_drive in system_drives {
            let is_safe = manager.is_safe_to_format(system_drive).await.unwrap();
            assert!(!is_safe,
                "CRITICAL: System drive {} marked as safe to format!", 
                system_drive.name);
        }
    }

    #[tokio::test]
    async fn test_mount_point_detection() {
        let manager = PlatformDeviceManager;
        let devices = manager.enumerate_devices().await.unwrap();
        
        // System drives should have mount points
        let system_drives: Vec<_> = devices.iter()
            .filter(|d| d.is_system)
            .collect();
        
        for drive in system_drives {
            assert!(!drive.mount_points.is_empty(),
                "System drive {} has no mount points!", drive.name);
            
            // Check for critical mount points
            let has_critical_mount = drive.mount_points.iter().any(|mp| {
                let path = mp.to_string_lossy().to_uppercase();
                path == "C:" || path == "/" || path.starts_with("C:\\")
            });
            
            assert!(has_critical_mount,
                "System drive {} missing critical mount point!", drive.name);
        }
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn test_windows_specific_enumeration() {
        use moses_platform::windows::WindowsDeviceManager;
        
        let manager = WindowsDeviceManager;
        let devices = manager.enumerate_devices().await.unwrap();
        
        for device in devices {
            // Windows device paths should follow pattern
            assert!(device.id.starts_with("\\\\.\\PHYSICALDRIVE"),
                "Invalid Windows device path: {}", device.id);
            
            // Check for reasonable device sizes
            assert!(device.size > 0, "Device {} has zero size!", device.name);
            assert!(device.size < 100 * 1024_u64.pow(4), // < 100TB
                "Device {} has unreasonable size!", device.name);
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn test_linux_specific_enumeration() {
        use moses_platform::linux::LinuxDeviceManager;
        
        let manager = LinuxDeviceManager;
        let devices = manager.enumerate_devices().await.unwrap();
        
        for device in devices {
            // Linux device paths should follow pattern
            assert!(device.id.starts_with("/dev/"),
                "Invalid Linux device path: {}", device.id);
            
            // Common patterns: /dev/sda, /dev/nvme0n1, /dev/mmcblk0
            let valid_pattern = device.id.starts_with("/dev/sd") ||
                              device.id.starts_with("/dev/nvme") ||
                              device.id.starts_with("/dev/mmcblk");
            
            assert!(valid_pattern,
                "Unusual Linux device path: {}", device.id);
        }
    }
}