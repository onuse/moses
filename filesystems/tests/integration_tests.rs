// Integration tests for the complete format → read → verify cycle
// These tests ensure that our formatters and readers work together correctly

use moses_filesystems::*;
use moses_core::{Device, FormatOptions, FilesystemFormatter};
use std::collections::HashMap;

/// Test data to write and read back
struct TestData {
    files: HashMap<String, Vec<u8>>,
    directories: Vec<String>,
}

impl TestData {
    fn new() -> Self {
        let mut files = HashMap::new();
        files.insert("test.txt".to_string(), b"Hello, Moses!".to_vec());
        files.insert("data.bin".to_string(), vec![0xFF; 1024]);
        files.insert("unicode_文件.txt".to_string(), "Unicode content 你好".as_bytes().to_vec());
        
        let directories = vec![
            "documents".to_string(),
            "photos".to_string(),
            "documents/work".to_string(),
        ];
        
        TestData { files, directories }
    }
}

/// Format a device, write test data, read it back, and verify
async fn test_format_write_read_cycle(
    filesystem_type: &str,
    formatter: impl FilesystemFormatter,
) {
    // 1. Create test device
    let device = create_test_device(500 * 1024 * 1024); // 500MB
    
    // 2. Format the device
    let options = FormatOptions {
        filesystem_type: filesystem_type.to_string(),
        label: Some(format!("TEST_{}", filesystem_type.to_uppercase())),
        cluster_size: Some(4096),
        quick_format: true,
        enable_compression: false,
        verify_after_format: true,
        dry_run: false,
        force: false,
        additional_options: Default::default(),
    };
    
    formatter.format(&device, &options).await.expect("Format should succeed");
    
    // 3. Mount and write test data (would need write support)
    // For now, we're testing read-only
    
    // 4. Read back and verify
    match filesystem_type {
        "ext2" | "ext3" | "ext4" => {
            verify_ext_filesystem(&device, filesystem_type).await;
        },
        "fat32" => {
            verify_fat32_filesystem(&device).await;
        },
        "ntfs" => {
            verify_ntfs_filesystem(&device).await;
        },
        _ => panic!("Unknown filesystem type"),
    }
}

async fn verify_ext_filesystem(device: &Device, expected_version: &str) {
    use moses_filesystems::families::ext::ext4_native::{ExtReader, core::ext_config::ExtVersion, reader::FileType};
    
    let mut reader = ExtReader::new(device.clone()).expect("Should open ext filesystem");
    
    // Verify version detection
    let detected_version = match reader.version {
        ExtVersion::Ext2 => "ext2",
        ExtVersion::Ext3 => "ext3",
        ExtVersion::Ext4 => "ext4",
    };
    assert_eq!(detected_version, expected_version);
    
    // Verify we can read root directory
    let root_entries = reader.read_directory("/").expect("Should read root");
    
    // Check for expected entries
    assert!(root_entries.iter().any(|e| e.name == "."));
    assert!(root_entries.iter().any(|e| e.name == ".."));
    assert!(root_entries.iter().any(|e| e.name == "lost+found"));
    
    // Verify lost+found is a directory
    let lost_found = root_entries.iter()
        .find(|e| e.name == "lost+found")
        .expect("Should have lost+found");
    assert_eq!(lost_found.entry_type, FileType::Directory);
}

async fn verify_fat32_filesystem(device: &Device) {
    use moses_filesystems::Fat32Reader;
    
    let mut reader = Fat32Reader::new(device.clone()).expect("Should open FAT32 filesystem");
    
    // Verify filesystem info
    let info = reader.get_info();
    assert_eq!(info.filesystem_type, "FAT32");
    assert!(info.label.is_some());
    
    // Verify we can read root directory
    let root_entries = reader.read_root().expect("Should read root");
    
    // New FAT32 filesystem should have empty root
    assert_eq!(root_entries.len(), 0);
}

async fn verify_ntfs_filesystem(_device: &Device) {
    // When NTFS reader is fully implemented
    // use moses_filesystems::NtfsReaderNative;
    // let mut reader = NtfsReaderNative::new(device.clone()).expect("Should open NTFS");
}

#[tokio::test]
async fn test_ext4_full_cycle() {
    test_format_write_read_cycle("ext4", Ext4NativeFormatter).await;
}

#[tokio::test]
async fn test_ext3_full_cycle() {
    test_format_write_read_cycle("ext3", Ext3Formatter).await;
}

#[tokio::test]
async fn test_ext2_full_cycle() {
    test_format_write_read_cycle("ext2", Ext2Formatter).await;
}

#[tokio::test]
async fn test_fat32_full_cycle() {
    test_format_write_read_cycle("fat32", Fat32Formatter).await;
}

/// Test cross-filesystem operations
#[tokio::test]
async fn test_cross_filesystem_copy() {
    // Format two devices with different filesystems
    let ext4_device = create_test_device(200 * 1024 * 1024);
    let fat32_device = create_test_device(200 * 1024 * 1024);
    
    // Format as ext4 and FAT32
    let ext4_formatter = Ext4NativeFormatter;
    let fat32_formatter = Fat32Formatter;
    
    let ext4_options = FormatOptions {
        filesystem_type: "ext4".to_string(),
        label: Some("SOURCE".to_string()),
        cluster_size: Some(4096),
        quick_format: true,
        enable_compression: false,
        verify_after_format: false,
        dry_run: false,
        force: false,
        additional_options: Default::default(),
    };
    
    let fat32_options = FormatOptions {
        filesystem_type: "fat32".to_string(),
        label: Some("DEST".to_string()),
        cluster_size: Some(4096),
        quick_format: true,
        enable_compression: false,
        verify_after_format: false,
        dry_run: false,
        force: false,
        additional_options: Default::default(),
    };
    
    ext4_formatter.format(&ext4_device, &ext4_options).await.unwrap();
    fat32_formatter.format(&fat32_device, &fat32_options).await.unwrap();
    
    // In a complete implementation, we would:
    // 1. Write files to ext4
    // 2. Read them with ExtReader
    // 3. Write them to FAT32 with Fat32Writer (when implemented)
    // 4. Verify the files match
}

/// Performance benchmarks
#[test]
fn bench_read_performance() {
    // Benchmark reading large files from different filesystems
}

#[test]
fn bench_directory_traversal() {
    // Benchmark traversing large directory structures
}

// Helper function (would be in a test utilities module)
fn create_test_device(size: u64) -> Device {
    use tempfile::NamedTempFile;
    use moses_core::device::DeviceType;
    
    let temp_file = NamedTempFile::new().unwrap();
    temp_file.as_file().set_len(size).unwrap();
    
    Device {
        id: temp_file.path().to_string_lossy().to_string(),
        name: "Test Device".to_string(),
        size,
        device_type: DeviceType::Virtual,
        mount_points: vec![],
        is_removable: true,
        is_system: false,
        filesystem: None,
    }
}