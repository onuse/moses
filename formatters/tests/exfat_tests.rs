// Tests for exFAT formatter and reader

use moses_formatters::{ExFatFormatter, ExFatReader};
use moses_core::{Device, FormatOptions, FilesystemFormatter};
use moses_core::device::DeviceType;
use tempfile::NamedTempFile;
use std::collections::HashMap;

/// Create a test device backed by a temporary file
fn create_test_device(size: u64) -> (Device, NamedTempFile) {
    let temp_file = NamedTempFile::new().unwrap();
    
    // Ensure the file is writable and of the specified size
    {
        use std::fs::OpenOptions;
        let file = OpenOptions::new()
            .write(true)
            .read(true)
            .open(temp_file.path())
            .unwrap();
        file.set_len(size).unwrap();
    }
    
    let device = Device {
        id: temp_file.path().to_string_lossy().to_string(),
        name: "Test Device".to_string(),
        size,
        device_type: DeviceType::Virtual,
        mount_points: vec![],
        is_removable: true,
        is_system: false,
        filesystem: None,
    };
    
    (device, temp_file)
}

#[tokio::test]
async fn test_exfat_formatter_safety() {
    // Test that exFAT formatter refuses system drives
    let system_device = Device {
        id: "SystemDrive".to_string(),
        name: "System Drive".to_string(),
        size: 500 * 1_073_741_824, // 500GB
        device_type: DeviceType::SSD,
        mount_points: vec![std::path::PathBuf::from("/")],
        is_removable: false,
        is_system: true,
        filesystem: None,
    };
    
    let formatter = ExFatFormatter;
    assert!(!formatter.can_format(&system_device), 
        "exFAT formatter should refuse system drives");
}

#[tokio::test]
async fn test_exfat_validate_options() {
    let formatter = ExFatFormatter;
    
    // Test valid options
    let valid_options = FormatOptions {
        filesystem_type: "exfat".to_string(),
        label: Some("MyDrive".to_string()),
        quick_format: true,
        cluster_size: None,
        enable_compression: false,
        verify_after_format: false,
        additional_options: HashMap::new(),
    };
    
    assert!(formatter.validate_options(&valid_options).await.is_ok());
    
    // Test compression not supported
    let invalid_options = FormatOptions {
        filesystem_type: "exfat".to_string(),
        label: Some("MyDrive".to_string()),
        quick_format: true,
        cluster_size: None,
        enable_compression: true, // exFAT doesn't support compression
        verify_after_format: false,
        additional_options: HashMap::new(),
    };
    
    assert!(formatter.validate_options(&invalid_options).await.is_err(),
        "exFAT should reject compression option");
}

#[tokio::test]
async fn test_exfat_label_truncation() {
    let formatter = ExFatFormatter;
    
    // Label longer than 15 characters (should truncate but not error)
    let long_label_options = FormatOptions {
        filesystem_type: "exfat".to_string(),
        label: Some("ThisIsAVeryLongLabelThatExceedsFifteenCharacters".to_string()),
        quick_format: true,
        cluster_size: None,
        enable_compression: false,
        verify_after_format: false,
        additional_options: HashMap::new(),
    };
    
    // Should succeed with warning (not error)
    assert!(formatter.validate_options(&long_label_options).await.is_ok(),
        "exFAT should allow long labels (with truncation)");
}

// Test would require actual formatting which needs admin privileges
// and a real or large temp device, so we'll skip it for CI
#[tokio::test]
#[ignore]
async fn test_format_and_read_exfat() {
    let (device, temp_file) = create_test_device(2 * 1024 * 1024 * 1024); // 2GB
    
    // Format as exFAT
    let formatter = ExFatFormatter;
    let options = FormatOptions {
        filesystem_type: "exfat".to_string(),
        label: Some("TEST_EXFAT".to_string()),
        cluster_size: None,
        quick_format: true,
        enable_compression: false,
        verify_after_format: false,
        additional_options: Default::default(),
    };
    
    let format_result = formatter.format(&device, &options).await;
    assert!(format_result.is_ok(), "Format failed: {:?}", format_result.err());
    
    // Check file still exists after formatting
    assert!(temp_file.path().exists(), "Temp file disappeared after formatting!");
    
    // Now try to read it back
    let reader_result = ExFatReader::new(device.clone());
    assert!(reader_result.is_ok(), "ExFatReader::new failed: {:?}", reader_result.err());
    let mut reader = reader_result.unwrap();
    
    // Should be able to read root directory
    let entries = reader.read_root().unwrap();
    
    // exFAT root starts empty (no . or .. entries like ext4)
    // Just verify we can read it without error
    eprintln!("exFAT root directory entries: {:?}", entries);
    
    // Check filesystem info
    let info = reader.get_info();
    assert_eq!(info.filesystem_type, "exFAT");
    
    // Keep temp_file alive until the end of the test
    drop(temp_file);
}

#[test]
fn test_exfat_supported_platforms() {
    use moses_core::Platform;
    
    let formatter = ExFatFormatter;
    let platforms = formatter.supported_platforms();
    
    assert!(platforms.contains(&Platform::Windows));
    assert!(platforms.contains(&Platform::Linux));
    assert!(platforms.contains(&Platform::MacOS));
}

#[test]
fn test_exfat_external_tools() {
    let formatter = ExFatFormatter;
    
    #[cfg(target_os = "linux")]
    assert!(formatter.requires_external_tools());
    
    #[cfg(not(target_os = "linux"))]
    assert!(!formatter.requires_external_tools());
}

#[tokio::test]
async fn test_dry_run() {
    let (device, _temp_file) = create_test_device(10 * 1_073_741_824); // 10GB
    
    let formatter = ExFatFormatter;
    let options = FormatOptions {
        filesystem_type: "exfat".to_string(),
        label: Some("TestDrive".to_string()),
        quick_format: true,
        cluster_size: None,
        enable_compression: false,
        verify_after_format: false,
        additional_options: HashMap::new(),
    };
    
    let report = formatter.dry_run(&device, &options).await.unwrap();
    
    assert_eq!(report.device.id, device.id);
    assert!(report.will_erase_data);
    assert!(report.warnings.len() > 0);
    
    // Check that space after format is reasonable (should be ~99% of device size)
    let efficiency = (report.space_after_format as f64) / (device.size as f64);
    assert!(efficiency > 0.98 && efficiency <= 1.0, 
        "exFAT should have minimal overhead, got {:.2}%", efficiency * 100.0);
}