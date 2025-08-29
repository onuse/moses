// Simple test program for NTFS writer functionality
// Creates a test NTFS volume and attempts to create a file

use moses_filesystems::ntfs::{NtfsFormatter, NtfsWriter, NtfsReader, NtfsWriteConfig};
use moses_core::{Device, DeviceType};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    println!("NTFS Writer Test Program");
    println!("========================");
    
    // Create a temporary file for testing
    let mut temp_file = NamedTempFile::new()?;
    let file_path = temp_file.path().to_str().unwrap().to_string();
    
    // Create a 50MB test volume
    let volume_size = 50 * 1024 * 1024;
    temp_file.as_file_mut().set_len(volume_size)?;
    
    println!("Created test volume: {} ({} MB)", file_path, volume_size / 1024 / 1024);
    
    // Step 1: Format as NTFS
    println!("\n1. Formatting as NTFS...");
    let mut formatter = NtfsFormatter::new();
    let device = Device {
        id: file_path.clone(),
        name: "Test NTFS Volume".to_string(),
        size: volume_size,
        device_type: DeviceType::USB,
        mount_points: vec![],
        is_removable: true,
        is_system: false,
        filesystem: None,
    };
    formatter.format(&device, "TestVolume")?;
    println!("   ✓ Format complete");
    
    // Step 2: Open with reader to verify format
    println!("\n2. Verifying format with reader...");
    let reader = NtfsReader::new(device.clone())?;
    let info = reader.filesystem_info()?;
    println!("   Volume Label: {}", info.label.unwrap_or_else(|| "None".to_string()));
    println!("   Total Space: {} MB", info.total_bytes / 1024 / 1024);
    println!("   ✓ Volume readable");
    
    // Step 3: List root directory (should be empty)
    println!("\n3. Listing root directory...");
    let entries = reader.list_directory("/")?;
    println!("   Files in root: {}", entries.len());
    for entry in &entries {
        println!("   - {}", entry.name);
    }
    
    // Step 4: Open with writer
    println!("\n4. Opening with writer...");
    let mut config = NtfsWriteConfig::default();
    config.enable_writes = true;  // Enable actual writes
    config.verify_writes = true;  // Verify after writing
    
    let mut writer = NtfsWriter::new(device.clone(), config)?;
    println!("   ✓ Writer initialized");
    
    // Step 5: Create a test file
    println!("\n5. Creating test file...");
    let mft_num = writer.create_file("test.txt", 0)?;
    println!("   ✓ Created 'test.txt' with MFT record #{}", mft_num);
    
    // Step 6: Try to list directory again
    println!("\n6. Listing directory after file creation...");
    // Re-open reader to get fresh view
    drop(writer);  // Close writer first
    let reader2 = NtfsReader::new(device.clone())?;
    let entries2 = reader2.list_directory("/")?;
    println!("   Files in root: {}", entries2.len());
    for entry in &entries2 {
        println!("   - {} (size: {} bytes)", entry.name, entry.size);
    }
    
    if entries2.iter().any(|e| e.name == "test.txt") {
        println!("   ✓ SUCCESS: File 'test.txt' is visible!");
    } else {
        println!("   ✗ WARNING: File 'test.txt' not visible in listing");
        println!("   This indicates the directory index update needs more work");
    }
    
    // Step 7: Try to write data to the file (if we can find it)
    println!("\n7. Testing file write operations...");
    let mut writer2 = NtfsWriter::new(device.clone(), config)?;
    
    // Try to write some data
    let test_data = b"Hello, NTFS World!";
    match writer2.write_file_data(mft_num, 0, test_data) {
        Ok(bytes) => {
            println!("   ✓ Wrote {} bytes to file", bytes);
        }
        Err(e) => {
            println!("   ✗ Could not write data: {}", e);
        }
    }
    
    println!("\n=========================");
    println!("Test Summary:");
    println!("- Format: ✓ Success");
    println!("- Read: ✓ Success");
    println!("- Create File: ✓ Success");
    println!("- Directory Index: {} Working", 
             if entries2.iter().any(|e| e.name == "test.txt") { "✓" } else { "✗ Not" });
    
    // Keep the temp file for debugging
    let (_file, path) = temp_file.into_parts();
    println!("\nTest volume saved to: {}", path.display());
    println!("You can examine it with a hex editor or mount it to verify");
    
    Ok(())
}