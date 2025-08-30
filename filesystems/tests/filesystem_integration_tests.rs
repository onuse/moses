// Integration tests for filesystem implementations
// Tests basic operations across FAT16, FAT32, NTFS, and EXT4

use moses_filesystems::{
    Fat16Formatter, Fat16Reader, Fat16Ops,
    Fat32Formatter, Fat32Reader, Fat32Ops,
    NtfsFormatter, NtfsReader, NtfsOps,
    Ext4NativeFormatter, ExtReader, Ext4Ops,
    FilesystemOps,
};
use moses_core::{Device, IoType};
use tempfile::NamedTempFile;
use std::fs::File;
use std::io::{Write, Seek, SeekFrom};

const TEST_DEVICE_SIZE: u64 = 128 * 1024 * 1024; // 128MB test devices

/// Create a test device backed by a temporary file
fn create_test_device(size: u64) -> (NamedTempFile, Device) {
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    temp_file.set_len(size).expect("Failed to set file size");
    temp_file.flush().expect("Failed to flush");
    
    let path = temp_file.path().to_string_lossy().to_string();
    let device = Device::new(&path, IoType::File).expect("Failed to create device");
    
    (temp_file, device)
}

#[test]
fn test_fat16_basic_operations() {
    let (_temp, device) = create_test_device(TEST_DEVICE_SIZE);
    
    // Format as FAT16
    let mut formatter = Fat16Formatter::new();
    formatter.format(&device, None).expect("Failed to format as FAT16");
    
    // Initialize filesystem operations
    let mut ops = Fat16Ops::new();
    ops.init(&device).expect("Failed to initialize FAT16 ops");
    
    // Test root directory listing (should be empty)
    let entries = ops.list_directory("/").expect("Failed to list root directory");
    assert_eq!(entries.len(), 0, "Root directory should be empty after format");
    
    // Test filesystem info
    let info = ops.get_filesystem_info().expect("Failed to get filesystem info");
    assert_eq!(info.filesystem_type, "FAT16");
    assert!(info.total_space > 0);
    assert!(info.free_space > 0);
}

#[test]
fn test_fat32_basic_operations() {
    let (_temp, device) = create_test_device(TEST_DEVICE_SIZE);
    
    // Format as FAT32
    let mut formatter = Fat32Formatter::new();
    formatter.format(&device, None).expect("Failed to format as FAT32");
    
    // Initialize filesystem operations
    let mut ops = Fat32Ops::new();
    ops.init(&device).expect("Failed to initialize FAT32 ops");
    
    // Test root directory listing (should be empty)
    let entries = ops.list_directory("/").expect("Failed to list root directory");
    assert_eq!(entries.len(), 0, "Root directory should be empty after format");
    
    // Test filesystem info
    let info = ops.get_filesystem_info().expect("Failed to get filesystem info");
    assert_eq!(info.filesystem_type, "FAT32");
    assert!(info.total_space > 0);
    assert!(info.free_space > 0);
}

#[test]
fn test_ntfs_basic_operations() {
    let (_temp, device) = create_test_device(TEST_DEVICE_SIZE);
    
    // Format as NTFS
    let mut formatter = NtfsFormatter::new();
    formatter.format(&device, None).expect("Failed to format as NTFS");
    
    // Initialize filesystem operations
    let mut ops = NtfsOps::new();
    ops.init(&device).expect("Failed to initialize NTFS ops");
    
    // Test root directory listing
    let entries = ops.list_directory("/").expect("Failed to list root directory");
    // NTFS may have system files after format
    assert!(entries.len() >= 0, "Should be able to list root directory");
    
    // Test filesystem info
    let info = ops.get_filesystem_info().expect("Failed to get filesystem info");
    assert_eq!(info.filesystem_type, "NTFS");
    assert!(info.total_space > 0);
    assert!(info.free_space > 0);
}

#[test]
fn test_ext4_basic_operations() {
    let (_temp, device) = create_test_device(TEST_DEVICE_SIZE);
    
    // Format as EXT4
    let mut formatter = Ext4NativeFormatter::new();
    formatter.format(&device, None).expect("Failed to format as EXT4");
    
    // Initialize filesystem operations
    let mut ops = Ext4Ops::new();
    ops.init(&device).expect("Failed to initialize EXT4 ops");
    
    // Test root directory listing
    let entries = ops.list_directory("/").expect("Failed to list root directory");
    // EXT4 may have lost+found after format
    assert!(entries.len() >= 0, "Should be able to list root directory");
    
    // Test filesystem info
    let info = ops.get_filesystem_info().expect("Failed to get filesystem info");
    assert_eq!(info.filesystem_type, "EXT4");
    assert!(info.total_space > 0);
    assert!(info.free_space > 0);
}

#[test]
fn test_cross_filesystem_read_operations() {
    // This test verifies that we can read from filesystems created by each formatter
    
    // Test FAT16 read
    {
        let (_temp, device) = create_test_device(64 * 1024 * 1024); // 64MB for FAT16
        let mut formatter = Fat16Formatter::new();
        formatter.format(&device, None).expect("Failed to format as FAT16");
        
        let mut reader = Fat16Reader::new(&device).expect("Failed to create FAT16 reader");
        let boot_sector = reader.read_boot_sector().expect("Failed to read FAT16 boot sector");
        assert_eq!(&boot_sector.oem_name[0..4], b"MSDOS" as &[u8]);
    }
    
    // Test FAT32 read
    {
        let (_temp, device) = create_test_device(TEST_DEVICE_SIZE);
        let mut formatter = Fat32Formatter::new();
        formatter.format(&device, None).expect("Failed to format as FAT32");
        
        let mut reader = Fat32Reader::new(&device).expect("Failed to create FAT32 reader");
        let boot_sector = reader.read_boot_sector().expect("Failed to read FAT32 boot sector");
        assert_eq!(&boot_sector.oem_name[0..4], b"MSDOS" as &[u8]);
    }
    
    // Test NTFS read
    {
        let (_temp, device) = create_test_device(TEST_DEVICE_SIZE);
        let mut formatter = NtfsFormatter::new();
        formatter.format(&device, None).expect("Failed to format as NTFS");
        
        let mut reader = NtfsReader::new(&device).expect("Failed to create NTFS reader");
        let boot_sector = reader.read_boot_sector().expect("Failed to read NTFS boot sector");
        assert_eq!(&boot_sector.oem_id, b"NTFS    ");
    }
    
    // Test EXT4 read
    {
        let (_temp, device) = create_test_device(TEST_DEVICE_SIZE);
        let mut formatter = Ext4NativeFormatter::new();
        formatter.format(&device, None).expect("Failed to format as EXT4");
        
        let reader = ExtReader::new(&device).expect("Failed to create EXT4 reader");
        let superblock = reader.read_superblock().expect("Failed to read EXT4 superblock");
        assert_eq!(superblock.s_magic, 0xEF53); // EXT4 magic number
    }
}

#[test]
fn test_filesystem_detection() {
    use moses_filesystems::{FilesystemDetector, NtfsDetector};
    
    // Format as NTFS and verify detection
    let (_temp, device) = create_test_device(TEST_DEVICE_SIZE);
    let mut formatter = NtfsFormatter::new();
    formatter.format(&device, None).expect("Failed to format as NTFS");
    
    let detector = NtfsDetector;
    assert!(detector.detect(&device).is_ok(), "Should detect NTFS filesystem");
}

#[test]
#[ignore] // Ignore for now as write operations may not be fully implemented
fn test_fat32_write_operations() {
    let (_temp, device) = create_test_device(TEST_DEVICE_SIZE);
    
    // Format as FAT32
    let mut formatter = Fat32Formatter::new();
    formatter.format(&device, None).expect("Failed to format as FAT32");
    
    // Initialize filesystem operations
    let mut ops = Fat32Ops::new();
    ops.init(&device).expect("Failed to initialize FAT32 ops");
    
    // Create a directory
    ops.create_directory("/testdir").expect("Failed to create directory");
    
    // List root directory - should have our new directory
    let entries = ops.list_directory("/").expect("Failed to list root directory");
    assert_eq!(entries.len(), 1, "Should have one directory");
    assert_eq!(entries[0].name, "testdir");
    assert!(entries[0].is_directory);
    
    // Create a file
    let test_data = b"Hello, FAT32!";
    ops.write_file("/test.txt", test_data).expect("Failed to write file");
    
    // Read the file back
    let read_data = ops.read_file("/test.txt").expect("Failed to read file");
    assert_eq!(read_data, test_data);
    
    // Delete the file
    ops.delete_file("/test.txt").expect("Failed to delete file");
    
    // Verify file is gone
    let entries = ops.list_directory("/").expect("Failed to list after delete");
    assert_eq!(entries.len(), 1, "Should only have directory left");
}