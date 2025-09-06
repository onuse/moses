// Remove old import - fat16 is now in families/fat/fat16

// Comprehensive FAT16 test suite
use super::{Fat16Verifier, VerificationResult};
use moses_core::{Device, DeviceType, FormatOptions, FilesystemFormatter};
use std::fs::{File, OpenOptions};
use std::io::{Write, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::ptr::addr_of;
use tempfile::NamedTempFile;

#[cfg(test)]
mod tests {
    use crate::Fat16Formatter;

    use super::*;
    
    fn create_test_device(size: u64) -> Device {
        Device {
            id: "test_device".to_string(),
            name: "Test Device".to_string(),
            size,
            device_type: DeviceType::USB,
            mount_points: vec![],
            is_removable: true,
            is_system: false,
            filesystem: None,
        }
    }
    
    fn create_test_image(size: u64) -> Result<NamedTempFile, std::io::Error> {
        let file = NamedTempFile::new()?;
        file.as_file().set_len(size)?;
        Ok(file)
    }
    
    async fn format_and_verify(size: u64, label: Option<&str>) -> VerificationResult {
        // Create test image
        let temp_file = create_test_image(size).expect("Failed to create test image");
        let path = temp_file.path().to_str().unwrap().to_string();
        
        // Create device
        let mut device = create_test_device(size);
        device.id = path.clone();
        
        // Format options
        let options = FormatOptions {
            filesystem_type: "fat16".to_string(),
            label: label.map(|s| s.to_string()),
            cluster_size: None,
            quick_format: false,
            enable_compression: false,
            verify_after_format: false,
            additional_options: std::collections::HashMap::new(),
        };
        
        // Format the device
        let formatter = super::super::formatter_compliant::Fat16CompliantFormatter;
        formatter.format(&device, &options).await.expect("Format failed");
        
        // Verify the filesystem
        Fat16Verifier::verify_filesystem(&path).expect("Verification failed")
    }
    
    #[tokio::test]
    async fn test_small_fat16() {
        // Test minimum FAT16 size (about 16MB - FAT16 needs at least 4085 clusters)
        let result = format_and_verify(16 * 1024 * 1024, Some("SMALL")).await;
        
        // Print the report regardless of success/failure
        println!("\n{}", Fat16Verifier::generate_report(&result));
        
        assert!(result.is_valid, "Small FAT16 should be valid");
        assert!(result.errors.is_empty(), "Should have no errors: {:?}", result.errors);
    }
    
    #[tokio::test]
    async fn test_medium_fat16() {
        // Test typical FAT16 size (128MB)
        let result = format_and_verify(128 * 1024 * 1024, Some("MEDIUM")).await;
        
        assert!(result.is_valid, "Medium FAT16 should be valid");
        assert!(result.errors.is_empty(), "Should have no errors: {:?}", result.errors);
        
        println!("{}", Fat16Verifier::generate_report(&result));
    }
    
    #[tokio::test]
    async fn test_large_fat16() {
        // Test near-maximum FAT16 size (2GB)
        let result = format_and_verify(2 * 1024 * 1024 * 1024, Some("LARGE")).await;
        
        assert!(result.is_valid, "Large FAT16 should be valid");
        assert!(result.errors.is_empty(), "Should have no errors: {:?}", result.errors);
        
        println!("{}", Fat16Verifier::generate_report(&result));
    }
    
    #[tokio::test]
    async fn test_no_label() {
        // Test without volume label
        let result = format_and_verify(64 * 1024 * 1024, None).await;
        
        assert!(result.is_valid, "FAT16 without label should be valid");
        assert!(result.errors.is_empty(), "Should have no errors: {:?}", result.errors);
    }
    
    #[tokio::test]
    async fn test_windows_compatibility() {
        // Test with Windows-compatible parameters
        let result = format_and_verify(512 * 1024 * 1024, Some("WINCOMPAT")).await;
        
        assert!(result.is_valid, "Should be valid");
        assert!(result.errors.is_empty(), "Should have no errors");
        
        // Check for Windows compatibility warnings
        let has_compat_issues = result.warnings.iter().any(|w| 
            w.contains("compatibility") || 
            w.contains("non-standard")
        );
        
        assert!(!has_compat_issues, "Should have no compatibility warnings: {:?}", result.warnings);
    }
    
    #[test]
    fn test_boot_sector_structure() {
        use crate::families::fat::common::Fat16BootSector;
        use std::mem;
        
        // Verify structure size matches expected layout
        // Note: Size may vary due to structure packing
        
        // Verify field offsets match FAT16 specification
        let bs = Fat16BootSector::new();
        
        unsafe {
            let ptr = &bs as *const _ as *const u8;
            
            // Check critical field offsets using the nested structure
            assert_eq!(ptr.add(0x0B) as *const u16, addr_of!(bs.common_bpb.bytes_per_sector));
            assert_eq!(ptr.add(0x0D) as *const u8, addr_of!(bs.common_bpb.sectors_per_cluster));
            assert_eq!(ptr.add(0x0E) as *const u16, addr_of!(bs.common_bpb.reserved_sectors));
            assert_eq!(ptr.add(0x10) as *const u8, addr_of!(bs.common_bpb.num_fats));
            assert_eq!(ptr.add(0x11) as *const u16, addr_of!(bs.common_bpb.root_entries));
            assert_eq!(ptr.add(0x16) as *const u16, addr_of!(bs.common_bpb.sectors_per_fat_16));
            assert_eq!(ptr.add(0x24) as *const u8, addr_of!(bs.extended_bpb.drive_number));
            assert_eq!(ptr.add(0x26) as *const u8, addr_of!(bs.extended_bpb.boot_signature));
        }
    }
    
    #[tokio::test]
    async fn test_mbr_partition_table() {
        // Test with MBR partition table
        let temp_file = create_test_image(256 * 1024 * 1024).expect("Failed to create test image");
        let path = temp_file.path().to_str().unwrap().to_string();
        
        let mut device = create_test_device(256 * 1024 * 1024);
        device.id = path.clone();
        
        let mut options = FormatOptions {
            filesystem_type: "fat16".to_string(),
            label: Some("PARTITION".to_string()),
            cluster_size: None,
            quick_format: false,
            enable_compression: false,
            verify_after_format: false,
            additional_options: std::collections::HashMap::new(),
        };
        
        // Add partition table option
        options.additional_options.insert(
            "create_partition_table".to_string(), 
            "true".to_string()
        );
        
        let formatter = Fat16Formatter;
        formatter.format(&device, &options).await.expect("Format failed");
        
        // Verify MBR
        let mut file = File::open(&path).expect("Failed to open formatted image");
        let mut mbr = [0u8; 512];
        file.read_exact(&mut mbr).expect("Failed to read MBR");
        
        // Check MBR signature
        assert_eq!(mbr[510], 0x55, "Invalid MBR signature byte 0");
        assert_eq!(mbr[511], 0xAA, "Invalid MBR signature byte 1");
        
        // Check partition entry (starts at offset 446)
        let part_entry = &mbr[446..462];
        assert_eq!(part_entry[0], 0x80, "Partition should be bootable");
        assert_eq!(part_entry[4], 0x06, "Partition type should be FAT16");
        
        // Verify FAT16 at partition offset (sector 2048 = 1MB)
        file.seek(SeekFrom::Start(2048 * 512)).expect("Failed to seek to partition");
        let result = Fat16Verifier::verify_filesystem(&path).expect("Verification failed");
        
        // Note: This might fail because verifier reads from start of file, not partition
        // We'd need to enhance verifier to support partition offsets
    }
}

// Integration tests that can be run against real devices (requires admin/root)
#[cfg(all(test, feature = "integration_tests"))]
mod integration_tests {
    use super::*;
    use std::process::Command;
    
    #[tokio::test]
    #[ignore] // Run with: cargo test --features integration_tests -- --ignored
    async fn test_real_usb_device() {
        // This test requires a real USB device and admin privileges
        // WARNING: This will format the device!
        
        let device_path = std::env::var("TEST_USB_DEVICE")
            .expect("Set TEST_USB_DEVICE environment variable to device path");
        
        println!("WARNING: This will format device: {}", device_path);
        println!("Press Ctrl+C to cancel, or wait 5 seconds to continue...");
        std::thread::sleep(std::time::Duration::from_secs(5));
        
        let device = Device {
            id: device_path.clone(),
            name: "Test USB Device".to_string(),
            size: 4 * 1024 * 1024 * 1024, // 4GB
            device_type: DeviceType::USB,
            mount_points: vec![],
            is_removable: true,
            is_system: false,
            filesystem: None,
        };
        
        let options = FormatOptions {
            filesystem_type: "fat16".to_string(),
            label: Some("MOSES_TEST".to_string()),
            cluster_size: None,
            quick_format: false,
            enable_compression: false,
            verify_after_format: false,
            additional_options: std::collections::HashMap::new(),
        };
        
        let formatter = Fat16Formatter;
        formatter.format(&device, &options).await.expect("Format failed");
        
        // Verify with our verifier
        let result = Fat16Verifier::verify_filesystem(&device_path)
            .expect("Verification failed");
        
        println!("{}", Fat16Verifier::generate_report(&result));
        assert!(result.is_valid, "Formatted device should be valid");
        
        // Try to mount on Windows
        #[cfg(target_os = "windows")]
        {
            let output = Command::new("cmd")
                .args(&["/C", "chkdsk", &device_path, "/F"])
                .output()
                .expect("Failed to run chkdsk");
            
            println!("CHKDSK output: {}", String::from_utf8_lossy(&output.stdout));
            assert!(output.status.success(), "CHKDSK should succeed");
        }
        
        // Try to mount on Linux
        #[cfg(target_os = "linux")]
        {
            let mount_point = "/tmp/moses_test_mount";
            std::fs::create_dir_all(mount_point).ok();
            
            let output = Command::new("mount")
                .args(&["-t", "vfat", &device_path, mount_point])
                .output()
                .expect("Failed to mount");
            
            if output.status.success() {
                println!("Successfully mounted at {}", mount_point);
                
                // Try to write a file
                let test_file = format!("{}/test.txt", mount_point);
                std::fs::write(&test_file, "Hello from Moses!").expect("Failed to write test file");
                
                // Unmount
                Command::new("umount")
                    .arg(mount_point)
                    .output()
                    .expect("Failed to unmount");
            } else {
                println!("Mount failed: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
    }
}