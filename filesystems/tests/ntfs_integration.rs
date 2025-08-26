// Integration tests for NTFS read-write operations
// Tests the complete NTFS implementation including formatting, reading, and writing

use moses_filesystems::ntfs::{NtfsFormatter, NtfsRwOps};
use moses_filesystems::ops::FilesystemOps;
use moses_core::{Device, DeviceType};
use std::path::Path;
use tempfile::NamedTempFile;

/// Create a test device for an image file
fn create_test_device(file_path: &str, size: u64) -> Device {
    Device {
        id: file_path.to_string(),
        name: format!("Test Device at {}", file_path),
        size,
        device_type: DeviceType::Virtual,
        mount_points: vec![],
        is_removable: false,
        is_system: false,
        filesystem: None,
    }
}

#[test]
fn test_ntfs_basic_operations() {
    // Create a temporary file for the NTFS image
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let file_path = temp_file.path().to_str().unwrap().to_string();
    
    // Create a 50MB NTFS image
    let size = 50 * 1024 * 1024;
    temp_file.as_file().set_len(size).expect("Failed to set file size");
    
    println!("Created test image: {} ({} MB)", file_path, size / (1024 * 1024));
    
    // Format as NTFS
    let mut formatter = NtfsFormatter::new();
    let mut device = create_test_device(&file_path, size);
    formatter.format(&mut device, "TestNTFS").expect("Failed to format NTFS");
    println!("Formatted as NTFS");
    
    // Mount with read-write support
    let mut ops = NtfsRwOps::new();
    ops.enable_writes(true);
    ops.init(&device).expect("Failed to initialize NTFS ops");
    println!("Mounted with read-write support");
    
    // Test 1: Create a file
    let test_file = Path::new("/test_file.txt");
    ops.create(test_file, 0o644).expect("Failed to create file");
    
    // Verify file exists
    let attrs = ops.stat(test_file).expect("Failed to stat created file");
    assert!(attrs.is_file);
    assert!(!attrs.is_directory);
    assert_eq!(attrs.size, 0);
    println!("✓ File creation works");
    
    // Test 2: Write data to file
    let test_data = b"Hello, NTFS world!";
    let written = ops.write(test_file, 0, test_data).expect("Failed to write to file");
    assert_eq!(written as usize, test_data.len());
    
    // Verify file size updated
    let attrs = ops.stat(test_file).expect("Failed to stat file after write");
    assert_eq!(attrs.size, test_data.len() as u64);
    println!("✓ File write works");
    
    // Test 3: Read data back
    let read_data = ops.read(test_file, 0, test_data.len() as u32)
        .expect("Failed to read from file");
    assert_eq!(read_data, test_data);
    println!("✓ File read works");
    
    // Test 4: List root directory
    let entries = ops.readdir(Path::new("/")).expect("Failed to list root directory");
    
    // Should contain our test file
    let test_entry = entries.iter()
        .find(|e| e.name == "test_file.txt")
        .expect("Test file not found in directory listing");
    assert!(test_entry.attributes.is_file);
    assert_eq!(test_entry.attributes.size, test_data.len() as u64);
    println!("✓ Directory listing works");
    
    // Test 5: Delete file
    ops.unlink(test_file).expect("Failed to delete file");
    
    // Verify file is gone
    assert!(ops.stat(test_file).is_err());
    
    // Verify it's not in directory listing
    let entries = ops.readdir(Path::new("/")).expect("Failed to list after delete");
    assert!(!entries.iter().any(|e| e.name == "test_file.txt"));
    println!("✓ File deletion works");
    
    println!("\nAll NTFS basic operations passed!");
}

#[test]
fn test_ntfs_large_file() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let file_path = temp_file.path().to_str().unwrap().to_string();
    temp_file.as_file().set_len(100 * 1024 * 1024).expect("Failed to set size");
    
    let mut formatter = NtfsFormatter::new();
    let mut device = create_test_device(&file_path, 100 * 1024 * 1024);
    formatter.format(&mut device, "LargeTest").expect("Failed to format");
    
    let mut ops = NtfsRwOps::new();
    ops.enable_writes(true);
    ops.init(&device).expect("Failed to init");
    
    // Create a file and write 4KB (should trigger non-resident conversion)
    let large_file = Path::new("/large.bin");
    ops.create(large_file, 0o644).expect("Failed to create large file");
    
    let large_data = vec![0x42; 4096];
    let written = ops.write(large_file, 0, &large_data)
        .expect("Failed to write large file");
    assert_eq!(written as usize, large_data.len());
    
    // Verify we can read it back
    let read_data = ops.read(large_file, 0, large_data.len() as u32)
        .expect("Failed to read large file");
    assert_eq!(read_data, large_data);
    
    println!("✓ Large file (non-resident) handling works");
}

#[test]
fn test_ntfs_multiple_files() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let file_path = temp_file.path().to_str().unwrap().to_string();
    temp_file.as_file().set_len(50 * 1024 * 1024).expect("Failed to set size");
    
    let mut formatter = NtfsFormatter::new();
    let mut device = create_test_device(&file_path, 50 * 1024 * 1024);
    formatter.format(&mut device, "MultiTest").expect("Failed to format");
    
    let mut ops = NtfsRwOps::new();
    ops.enable_writes(true);
    ops.init(&device).expect("Failed to init");
    
    // Create multiple files
    for i in 0..10 {
        let filename = format!("/file_{}.txt", i);
        let path = Path::new(&filename);
        ops.create(path, 0o644).expect(&format!("Failed to create {}", filename));
        
        let data = format!("This is file number {}", i);
        ops.write(path, 0, data.as_bytes())
            .expect(&format!("Failed to write to {}", filename));
    }
    
    // Verify all files exist
    let entries = ops.readdir(Path::new("/")).expect("Failed to list files");
    for i in 0..10 {
        let filename = format!("file_{}.txt", i);
        assert!(entries.iter().any(|e| e.name == filename));
    }
    
    println!("✓ Multiple file handling works");
}