// Simple test to verify ext4 formatting works
use moses_filesystems::{Ext4NativeFormatter};
use moses_core::{Device, FormatOptions, FilesystemFormatter};
use moses_core::device::DeviceType;
use tempfile::NamedTempFile;
use std::collections::HashMap;

#[tokio::test]
async fn test_ext4_format_simple() {
    // Create temp file
    let temp_file = NamedTempFile::new().unwrap();
    temp_file.as_file().set_len(1024 * 1024 * 1024).unwrap(); // 1GB
    
    let device = Device {
        id: temp_file.path().to_string_lossy().to_string(),
        name: "Test Device".to_string(),
        size: 1024 * 1024 * 1024,
        device_type: DeviceType::Virtual,
        mount_points: vec![],
        is_removable: true,
        is_system: false,
        filesystem: None,
    };
    
    println!("Device path: {}", device.id);
    
    // Format as ext4
    let formatter = Ext4NativeFormatter;
    let options = FormatOptions {
        filesystem_type: "ext4".to_string(),
        label: Some("TEST".to_string()),
        cluster_size: Some(4096),
        quick_format: true,
        enable_compression: false,
        verify_after_format: false,
        additional_options: HashMap::new(),
    };
    
    let result = formatter.format(&device, &options).await;
    assert!(result.is_ok(), "Format failed: {:?}", result);
    
    // Read superblock to verify it was written
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};
    
    let mut file = File::open(temp_file.path()).unwrap();
    file.seek(SeekFrom::Start(1024)).unwrap(); // Superblock at 1024
    
    let mut magic = [0u8; 2];
    file.seek(SeekFrom::Start(1024 + 0x38)).unwrap(); // Magic at offset 0x38
    file.read_exact(&mut magic).unwrap();
    
    // ext4 magic is 0xEF53
    assert_eq!(magic, [0x53, 0xEF], "Invalid ext4 magic number");
    
    println!("✓ Ext4 filesystem created successfully");
}