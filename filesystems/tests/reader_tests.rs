// Comprehensive test suite for all filesystem readers
// Tests that we can format a filesystem and then read it back

use moses_filesystems::{
    Ext4NativeFormatter, Ext2Formatter, Ext3Formatter,
    Fat32Formatter, Fat32Reader,
    ext4_native::{ExtReader, core::ext_config::ExtVersion},
};
use moses_core::{Device, FormatOptions, FilesystemFormatter};
use std::fs::{File, OpenOptions};
use std::io::{Write, Seek, SeekFrom};
use tempfile::NamedTempFile;

/// Create a test device backed by a temporary file
fn create_test_device(size: u64) -> (Device, NamedTempFile) {
    use moses_core::device::DeviceType;
    use std::fs::OpenOptions;
    
    let temp_file = NamedTempFile::new().unwrap();
    
    // Ensure the file is writable and of the specified size
    {
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
async fn test_format_and_read_ext4() {
    let (device, temp_file) = create_test_device(1024 * 1024 * 1024); // 1GB
    
    // Debug: Print the device path
    eprintln!("Test device path: {}", device.id);
    eprintln!("Temp file path: {:?}", temp_file.path());
    
    // Verify the file exists
    assert!(temp_file.path().exists(), "Temp file does not exist!");
    
    // Format as ext4
    let formatter = Ext4NativeFormatter;
    let options = FormatOptions {
        filesystem_type: "ext4".to_string(),
        label: Some("TEST_EXT4".to_string()),
        cluster_size: Some(4096),
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
    let reader_result = ExtReader::new(device.clone());
    assert!(reader_result.is_ok(), "ExtReader::new failed: {:?}", reader_result.err());
    let mut reader = reader_result.unwrap();
    
    // Should be able to read root directory
    let entries = reader.read_directory("/").unwrap();
    
    // Debug: print what we got
    eprintln!("Root directory entries: {:?}", entries);
    
    // Root should have at least . and .. entries
    assert!(entries.len() >= 2, "Expected at least 2 entries, got {}", entries.len());
    
    // Should find lost+found
    let lost_found = entries.iter().find(|e| e.name == "lost+found");
    assert!(lost_found.is_some());
    assert_eq!(lost_found.unwrap().entry_type, moses_filesystems::ext4_native::reader::FileType::Directory);
    
    // Keep temp_file alive until the end of the test
    drop(temp_file);
}

#[tokio::test]
async fn test_format_and_read_ext2() {
    let (device, temp_file) = create_test_device(1024 * 1024 * 1024); // 1GB
    
    // Format as ext2
    let formatter = Ext2Formatter;
    let options = FormatOptions {
        filesystem_type: "ext2".to_string(),
        label: Some("TEST_EXT2".to_string()),
        cluster_size: Some(4096),
        quick_format: true,
        enable_compression: false,
        verify_after_format: false,
        additional_options: Default::default(),
    };
    
    formatter.format(&device, &options).await.unwrap();
    
    // Read it back
    let mut reader = ExtReader::new(device.clone()).unwrap();
    
    // Verify it detected ext2
    assert_eq!(reader.version, ExtVersion::Ext2);
    
    // Should be able to read root
    let entries = reader.read_directory("/").unwrap();
    assert!(entries.len() >= 2);
    
    // Keep temp_file alive until the end of the test
    drop(temp_file);
}

#[tokio::test]
async fn test_format_and_read_ext3() {
    let (device, temp_file) = create_test_device(1024 * 1024 * 1024); // 1GB
    
    // Format as ext3
    let formatter = Ext3Formatter;
    let options = FormatOptions {
        filesystem_type: "ext3".to_string(),
        label: Some("TEST_EXT3".to_string()),
        cluster_size: Some(4096),
        quick_format: true,
        enable_compression: false,
        verify_after_format: false,
        additional_options: Default::default(),
    };
    
    formatter.format(&device, &options).await.unwrap();
    
    // Read it back
    let mut reader = ExtReader::new(device.clone()).unwrap();
    
    // Verify it detected ext3
    assert_eq!(reader.version, ExtVersion::Ext3);
    
    // Should be able to read root
    let entries = reader.read_directory("/").unwrap();
    assert!(entries.len() >= 2);
    
    // Keep temp_file alive until the end of the test
    drop(temp_file);
}

#[tokio::test]
async fn test_format_and_read_fat32() {
    let (device, temp_file) = create_test_device(1024 * 1024 * 1024); // 1GB
    
    // Format as FAT32
    let formatter = Fat32Formatter;
    let options = FormatOptions {
        filesystem_type: "fat32".to_string(),
        label: Some("TEST_FAT".to_string()),
        cluster_size: Some(4096),
        quick_format: true,
        enable_compression: false,
        verify_after_format: false,
        additional_options: Default::default(),
    };
    
    formatter.format(&device, &options).await.unwrap();
    
    // Read it back
    let mut reader = Fat32Reader::new(device.clone()).unwrap();
    
    // Should be able to read root
    let entries = reader.read_root().unwrap();
    
    // FAT32 root starts empty
    assert_eq!(entries.len(), 0);
    
    // Check filesystem info
    let info = reader.get_info();
    assert_eq!(info.filesystem_type, "FAT32");
    assert_eq!(info.label, Some("TEST_FAT".to_string()));
    
    // Keep temp_file alive until the end of the test
    drop(temp_file);
}

#[test]
fn test_ext_reader_file_operations() {
    // Test that we can navigate directories and read files
    // This would need a pre-formatted image with known content
    // For now, just ensure the module compiles
}

#[test]
fn test_fat32_reader_operations() {
    // Test FAT32 specific operations like 8.3 name parsing
    use moses_filesystems::fat32::reader::Fat32Reader;
    
    // Test 8.3 name parsing (this is a static method we'd need to make public)
    // assert_eq!(Fat32Reader::parse_83_name(b"README  TXT"), "README.TXT");
}

#[test]
fn test_cross_platform_reading() {
    // This test would verify that we can read filesystems
    // regardless of the host OS
    
    #[cfg(target_os = "windows")]
    {
        // Should be able to read ext4 on Windows
    }
    
    #[cfg(target_os = "linux")]
    {
        // Should be able to read NTFS on Linux (when implemented)
    }
}

/// Test that readers properly detect filesystem types
#[test]
fn test_filesystem_detection() {
    // Would test with various filesystem images
    // to ensure proper detection
}

/// Test that readers handle corrupted filesystems gracefully
#[test]
fn test_corrupted_filesystem_handling() {
    // Create intentionally corrupted filesystem images
    // and ensure readers don't crash
}

/// Test reading large files
#[test]
fn test_large_file_reading() {
    // Test reading files larger than available RAM
    // using streaming/chunked reads
}

/// Test unicode filename support
#[test]
fn test_unicode_filenames() {
    // Test that readers properly handle UTF-8/UTF-16 filenames
}