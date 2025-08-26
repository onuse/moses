#!/usr/bin/env rust-script
//! Basic NTFS read-write test
//! 
//! ```cargo
//! [dependencies]
//! moses-filesystems = { path = "filesystems" }
//! moses-core = { path = "core" }
//! tempfile = "3.8"
//! env_logger = "0.10"
//! ```

use moses_filesystems::ntfs::{NtfsFormatter, NtfsRwOps};
use moses_filesystems::ops::FilesystemOps;
use moses_core::Device;
use std::path::Path;
use tempfile::NamedTempFile;

fn main() {
    env_logger::init();
    
    println!("🧪 NTFS Basic Read-Write Test");
    println!("================================\n");
    
    // Create a temporary file for the NTFS image
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let file_path = temp_file.path().to_str().unwrap().to_string();
    
    // Create a 50MB NTFS image
    let size = 50 * 1024 * 1024;
    temp_file.as_file().set_len(size).expect("Failed to set file size");
    
    println!("✅ Created test image: {}", file_path);
    println!("   Size: {} MB\n", size / (1024 * 1024));
    
    // Format as NTFS
    println!("📝 Formatting as NTFS...");
    let mut formatter = NtfsFormatter::new();
    let mut device = Device::new(&file_path).expect("Failed to open device");
    formatter.format(&mut device, "TestNTFS").expect("Failed to format NTFS");
    println!("✅ NTFS filesystem created\n");
    
    // Mount with read-write support
    println!("📂 Mounting with read-write support...");
    let mut ops = NtfsRwOps::new();
    ops.enable_writes(true);
    ops.init(&device).expect("Failed to initialize NTFS ops");
    println!("✅ Mounted successfully\n");
    
    // Test 1: Create a file
    println!("Test 1: File Creation");
    println!("---------------------");
    let test_file = Path::new("/test_file.txt");
    match ops.create(test_file, 0o644) {
        Ok(_) => println!("✅ Created file: /test_file.txt"),
        Err(e) => {
            println!("❌ Failed to create file: {}", e);
            return;
        }
    }
    
    // Verify file exists
    match ops.stat(test_file) {
        Ok(attrs) => {
            println!("✅ File stats:");
            println!("   - Is file: {}", attrs.is_file);
            println!("   - Size: {} bytes", attrs.size);
        }
        Err(e) => println!("❌ Failed to stat file: {}", e),
    }
    println!();
    
    // Test 2: Write data to file
    println!("Test 2: File Write");
    println!("------------------");
    let test_data = b"Hello, NTFS world! This is a test file.";
    match ops.write(test_file, 0, test_data) {
        Ok(written) => {
            println!("✅ Wrote {} bytes to file", written);
            
            // Verify size updated
            if let Ok(attrs) = ops.stat(test_file) {
                println!("✅ File size after write: {} bytes", attrs.size);
            }
        }
        Err(e) => println!("❌ Failed to write: {}", e),
    }
    println!();
    
    // Test 3: Read data back
    println!("Test 3: File Read");
    println!("-----------------");
    match ops.read(test_file, 0, test_data.len() as u32) {
        Ok(read_data) => {
            if read_data == test_data {
                println!("✅ Read data matches written data");
                println!("   Content: \"{}\"", String::from_utf8_lossy(&read_data));
            } else {
                println!("❌ Read data doesn't match!");
            }
        }
        Err(e) => println!("❌ Failed to read: {}", e),
    }
    println!();
    
    // Test 4: List directory
    println!("Test 4: Directory Listing");
    println!("-------------------------");
    match ops.readdir(Path::new("/")) {
        Ok(entries) => {
            println!("✅ Root directory contains {} entries:", entries.len());
            for entry in &entries {
                let type_str = if entry.attributes.is_directory { "DIR " } else { "FILE" };
                println!("   [{}] {} ({} bytes)", 
                    type_str, 
                    entry.name, 
                    entry.attributes.size
                );
            }
        }
        Err(e) => println!("❌ Failed to list directory: {}", e),
    }
    println!();
    
    // Test 5: Delete file
    println!("Test 5: File Deletion");
    println!("---------------------");
    match ops.unlink(test_file) {
        Ok(_) => {
            println!("✅ Deleted file: /test_file.txt");
            
            // Verify it's gone
            match ops.stat(test_file) {
                Err(_) => println!("✅ File no longer exists"),
                Ok(_) => println!("❌ File still exists after delete!"),
            }
        }
        Err(e) => println!("❌ Failed to delete: {}", e),
    }
    println!();
    
    // Summary
    println!("================================");
    println!("🎉 NTFS Basic Test Complete!");
    println!("================================");
}