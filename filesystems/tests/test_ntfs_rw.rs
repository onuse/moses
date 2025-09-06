// Test NTFS read-write operations
// This test creates a test NTFS image and performs various read-write operations

use moses_filesystems::families::ntfs::ntfs::{NtfsFormatter, NtfsRwOps};
use moses_filesystems::ops::FilesystemOps;
use moses_core::Device;
use std::path::Path;
use tempfile::NamedTempFile;
use std::fs::File;
use std::io::{Write, Seek, SeekFrom};

#[test]
fn test_ntfs_read_write_operations() {
    env_logger::init();
    
    // Create a temporary file for the NTFS image
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let file_path = temp_file.path().to_str().unwrap().to_string();
    
    // Create a 100MB NTFS image
    let size = 100 * 1024 * 1024;
    temp_file.as_file_mut().set_len(size).expect("Failed to set file size");
    
    println!("Creating NTFS filesystem on test image...");
    
    // Format as NTFS
    let mut formatter = NtfsFormatter::new();
    let mut device = Device::new(&file_path).expect("Failed to open device");
    formatter.format(&mut device, "TestNTFS").expect("Failed to format NTFS");
    
    println!("NTFS filesystem created successfully");
    
    // Mount with read-write support
    let mut ops = NtfsRwOps::new();
    ops.enable_writes(true);
    ops.init(&device).expect("Failed to initialize NTFS ops");
    
    println!("Testing file creation...");
    
    // Test 1: Create a file
    let test_file = Path::new("/test_file.txt");
    ops.create(test_file, 0o644).expect("Failed to create file");
    
    // Verify file exists
    let attrs = ops.stat(test_file).expect("Failed to stat created file");
    assert!(attrs.is_file);
    assert!(!attrs.is_directory);
    assert_eq!(attrs.size, 0);
    
    println!("Testing file write...");
    
    // Test 2: Write data to file
    let test_data = b"Hello, NTFS world! This is a test file.";
    let written = ops.write(test_file, 0, test_data).expect("Failed to write to file");
    assert_eq!(written as usize, test_data.len());
    
    // Verify file size updated
    let attrs = ops.stat(test_file).expect("Failed to stat file after write");
    assert_eq!(attrs.size, test_data.len() as u64);
    
    println!("Testing file read...");
    
    // Test 3: Read data back
    let read_data = ops.read(test_file, 0, test_data.len() as u32)
        .expect("Failed to read from file");
    assert_eq!(read_data, test_data);
    
    // Test partial read
    let partial_data = ops.read(test_file, 7, 4)
        .expect("Failed to read partial data");
    assert_eq!(partial_data, b"NTFS");
    
    println!("Testing directory listing...");
    
    // Test 4: List root directory
    let entries = ops.readdir(Path::new("/")).expect("Failed to list root directory");
    
    // Should contain our test file
    let test_entry = entries.iter()
        .find(|e| e.name == "test_file.txt")
        .expect("Test file not found in directory listing");
    assert!(test_entry.attributes.is_file);
    assert_eq!(test_entry.attributes.size, test_data.len() as u64);
    
    println!("Testing file append...");
    
    // Test 5: Append data to file
    let append_data = b" More data appended.";
    let offset = test_data.len() as u64;
    let written = ops.write(test_file, offset, append_data)
        .expect("Failed to append to file");
    assert_eq!(written as usize, append_data.len());
    
    // Verify new size
    let attrs = ops.stat(test_file).expect("Failed to stat file after append");
    assert_eq!(attrs.size, (test_data.len() + append_data.len()) as u64);
    
    // Read entire file
    let full_data = ops.read(test_file, 0, (test_data.len() + append_data.len()) as u32)
        .expect("Failed to read full file");
    let expected = [test_data.as_ref(), append_data.as_ref()].concat();
    assert_eq!(full_data, expected);
    
    println!("Testing file overwrite...");
    
    // Test 6: Overwrite middle of file
    let overwrite_data = b"MODIFIED";
    let written = ops.write(test_file, 14, overwrite_data)
        .expect("Failed to overwrite file");
    assert_eq!(written as usize, overwrite_data.len());
    
    // Read and verify
    let read_data = ops.read(test_file, 0, attrs.size as u32)
        .expect("Failed to read modified file");
    assert_eq!(&read_data[14..22], overwrite_data);
    
    println!("Testing large file creation...");
    
    // Test 7: Create a larger file (test non-resident conversion)
    let large_file = Path::new("/large_file.bin");
    ops.create(large_file, 0o644).expect("Failed to create large file");
    
    // Write 4KB of data (should trigger non-resident conversion)
    let large_data = vec![0xAB; 4096];
    let written = ops.write(large_file, 0, &large_data)
        .expect("Failed to write large file");
    assert_eq!(written as usize, large_data.len());
    
    // Verify we can read it back
    let read_large = ops.read(large_file, 0, large_data.len() as u32)
        .expect("Failed to read large file");
    assert_eq!(read_large, large_data);
    
    println!("Testing file deletion...");
    
    // Test 8: Delete a file
    ops.unlink(test_file).expect("Failed to delete file");
    
    // Verify file is gone
    assert!(ops.stat(test_file).is_err());
    
    // Verify it's not in directory listing
    let entries = ops.readdir(Path::new("/")).expect("Failed to list after delete");
    assert!(!entries.iter().any(|e| e.name == "test_file.txt"));
    
    println!("Testing multiple files...");
    
    // Test 9: Create multiple files
    for i in 0..5 {
        let filename = format!("/file_{}.txt", i);
        let path = Path::new(&filename);
        ops.create(path, 0o644).expect(&format!("Failed to create {}", filename));
        
        let data = format!("This is file number {}", i);
        ops.write(path, 0, data.as_bytes())
            .expect(&format!("Failed to write to {}", filename));
    }
    
    // List and verify
    let entries = ops.readdir(Path::new("/")).expect("Failed to list multiple files");
    for i in 0..5 {
        let filename = format!("file_{}.txt", i);
        assert!(entries.iter().any(|e| e.name == filename));
    }
    
    println!("Testing sync operation...");
    
    // Test 10: Sync filesystem
    ops.sync().expect("Failed to sync filesystem");
    
    println!("\n✅ All NTFS read-write tests passed!");
}

#[test]
fn test_ntfs_edge_cases() {
    env_logger::init();
    
    // Create test NTFS image
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let file_path = temp_file.path().to_str().unwrap().to_string();
    temp_file.as_file_mut().set_len(50 * 1024 * 1024).expect("Failed to set size");
    
    let mut formatter = NtfsFormatter::new();
    let mut device = Device::new(&file_path).expect("Failed to open device");
    formatter.format(&mut device, "EdgeTest").expect("Failed to format");
    
    let mut ops = NtfsRwOps::new();
    ops.enable_writes(true);
    ops.init(&device).expect("Failed to init");
    
    println!("Testing empty file operations...");
    
    // Test empty file
    let empty_file = Path::new("/empty.txt");
    ops.create(empty_file, 0o644).expect("Failed to create empty file");
    
    let attrs = ops.stat(empty_file).expect("Failed to stat empty file");
    assert_eq!(attrs.size, 0);
    
    let data = ops.read(empty_file, 0, 100).expect("Failed to read empty file");
    assert_eq!(data.len(), 0);
    
    println!("Testing filename with spaces...");
    
    // Test filename with spaces
    let spaced_file = Path::new("/file with spaces.txt");
    ops.create(spaced_file, 0o644).expect("Failed to create file with spaces");
    ops.write(spaced_file, 0, b"spaces work").expect("Failed to write");
    
    let entries = ops.readdir(Path::new("/")).expect("Failed to list");
    assert!(entries.iter().any(|e| e.name == "file with spaces.txt"));
    
    println!("Testing long filename...");
    
    // Test long filename (NTFS supports up to 255 characters)
    let long_name = "a".repeat(200) + ".txt";
    let long_file = format!("/{}", long_name);
    let long_path = Path::new(&long_file);
    ops.create(long_path, 0o644).expect("Failed to create file with long name");
    
    let attrs = ops.stat(long_path).expect("Failed to stat long named file");
    assert!(attrs.is_file);
    
    println!("Testing read beyond EOF...");
    
    // Test read beyond end of file
    let test_file = Path::new("/test_eof.txt");
    ops.create(test_file, 0o644).expect("Failed to create");
    ops.write(test_file, 0, b"short").expect("Failed to write");
    
    let data = ops.read(test_file, 3, 10).expect("Failed to read beyond EOF");
    assert_eq!(data, b"rt"); // Should only get remaining 2 bytes
    
    let data = ops.read(test_file, 10, 10).expect("Failed to read past EOF");
    assert_eq!(data.len(), 0); // Should get nothing
    
    println!("Testing write at large offset (sparse file)...");
    
    // Test sparse file (write at large offset)
    let sparse_file = Path::new("/sparse.dat");
    ops.create(sparse_file, 0o644).expect("Failed to create sparse file");
    
    // Write at 1MB offset
    let offset = 1024 * 1024;
    ops.write(sparse_file, offset, b"sparse data")
        .expect("Failed to write sparse");
    
    // File size should reflect the sparse write
    let attrs = ops.stat(sparse_file).expect("Failed to stat sparse file");
    assert!(attrs.size >= offset + 11);
    
    // Read from beginning should return zeros
    let data = ops.read(sparse_file, 0, 10).expect("Failed to read sparse start");
    assert_eq!(data, vec![0; 10]);
    
    // Read from written offset
    let data = ops.read(sparse_file, offset, 11).expect("Failed to read sparse data");
    assert_eq!(data, b"sparse data");
    
    println!("\n✅ All edge case tests passed!");
}

#[test]
fn test_ntfs_readonly_ops() {
    env_logger::init();
    
    // Create and format test image
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let file_path = temp_file.path().to_str().unwrap().to_string();
    temp_file.as_file_mut().set_len(50 * 1024 * 1024).expect("Failed to set size");
    
    let mut formatter = NtfsFormatter::new();
    let mut device = Device::new(&file_path).expect("Failed to open device");
    formatter.format(&mut device, "ReadOnly").expect("Failed to format");
    
    // First, create some test files with write enabled
    let mut write_ops = NtfsRwOps::new();
    write_ops.enable_writes(true);
    write_ops.init(&device).expect("Failed to init write ops");
    
    // Create test files
    write_ops.create(Path::new("/test.txt"), 0o644).unwrap();
    write_ops.write(Path::new("/test.txt"), 0, b"test data").unwrap();
    write_ops.create(Path::new("/another.txt"), 0o644).unwrap();
    write_ops.write(Path::new("/another.txt"), 0, b"more data").unwrap();
    drop(write_ops); // Close write ops
    
    println!("Testing read-only operations...");
    
    // Now test with read-only ops (default)
    let mut readonly_ops = NtfsRwOps::new();
    // Don't enable writes - should be readonly by default
    readonly_ops.init(&device).expect("Failed to init readonly");
    
    assert!(readonly_ops.is_readonly());
    
    // Reading should work
    let data = readonly_ops.read(Path::new("/test.txt"), 0, 100)
        .expect("Failed to read in readonly mode");
    assert_eq!(data, b"test data");
    
    // Listing should work
    let entries = readonly_ops.readdir(Path::new("/"))
        .expect("Failed to list in readonly mode");
    assert!(entries.iter().any(|e| e.name == "test.txt"));
    assert!(entries.iter().any(|e| e.name == "another.txt"));
    
    // Stat should work
    let attrs = readonly_ops.stat(Path::new("/test.txt"))
        .expect("Failed to stat in readonly mode");
    assert_eq!(attrs.size, 9);
    assert!(attrs.is_file);
    
    // Write operations should fail
    assert!(readonly_ops.create(Path::new("/new.txt"), 0o644).is_err());
    assert!(readonly_ops.write(Path::new("/test.txt"), 0, b"fail").is_err());
    assert!(readonly_ops.unlink(Path::new("/test.txt")).is_err());
    assert!(readonly_ops.mkdir(Path::new("/newdir"), 0o755).is_err());
    
    println!("\n✅ Read-only tests passed!");
}