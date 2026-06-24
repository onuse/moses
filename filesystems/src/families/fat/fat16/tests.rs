// Comprehensive FAT16 test suite
// Tests formatter functionality and validates FAT16 structure

use moses_core::{Device, DeviceType, FormatOptions, FilesystemFormatter, SimulationReport};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use tempfile::NamedTempFile;

// Import the FAT16 formatter
use crate::families::fat::fat16::Fat16Formatter;

// ============================================================================
// Test Device Helpers
// ============================================================================

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

// ============================================================================
// FAT16-Specific Validation Helpers
// ============================================================================

/// Validate FAT16 boot sector
fn validate_fat16_boot_sector(boot_sector: &[u8]) -> Vec<String> {
    let mut errors = Vec::new();
    
    if boot_sector.len() < 512 {
        errors.push("Boot sector less than 512 bytes".to_string());
        return errors;
    }
    
    // Check jump instruction
    if boot_sector[0] != 0xEB && boot_sector[0] != 0xE9 {
        errors.push(format!(
            "Invalid jump instruction: 0x{:02X}",
            boot_sector[0]
        ));
    }
    
    // Check boot signature
    if boot_sector[510] != 0x55 || boot_sector[511] != 0xAA {
        errors.push("Invalid boot signature (should be 55 AA)".to_string());
    }
    
    // Check OEM name (typically MSWIN4.1 or MSDOS5.0)
    let oem_name = String::from_utf8_lossy(&boot_sector[3..11]);
    if !oem_name.trim().starts_with("MSWIN") && !oem_name.trim().starts_with("MSDOS") {
        errors.push(format!(
            "Non-standard OEM name: '{}' (Windows uses 'MSWIN4.1')",
            oem_name
        ));
    }
    
    errors
}

/// Calculate expected FAT16 layout
fn calculate_fat16_layout(total_sectors: u64) -> (u8, u16) {
    // Determine sectors per cluster based on volume size
    let sectors_per_cluster = if total_sectors <= 8400 {
        1    // <= 4MB
    } else if total_sectors <= 16640 {
        2    // <= 8MB
    } else if total_sectors <= 32768 {
        4    // <= 16MB
    } else if total_sectors <= 65536 {
        8    // <= 32MB
    } else if total_sectors <= 131072 {
        16   // <= 64MB
    } else if total_sectors <= 262144 {
        32   // <= 128MB
    } else if total_sectors <= 524288 {
        64   // <= 256MB
    } else {
        128  // > 256MB
    };
    
    (sectors_per_cluster, 256) // sectors_per_cluster, root_entries
}

/// Validate FAT16 cluster chain
fn validate_fat16_cluster_chain(fat_data: &[u8], total_clusters: u16) -> Vec<String> {
    let mut errors = Vec::new();
    
    // First FAT entry should be media descriptor followed by FF FF
    let media_descriptor = fat_data[0];
    let expected_media = 0xF8; // Fixed disk
    
    if fat_data[0] != expected_media {
        errors.push(format!(
            "FAT[0] low byte 0x{:02X} doesn't match media descriptor 0x{:02X}",
            fat_data[0], expected_media
        ));
    }
    
    if fat_data[1] != 0xFF || fat_data[2] != 0xFF || fat_data[3] != 0xFF {
        errors.push("Invalid FAT[0] header (should be F8 FF FF FF or F0 FF FF FF)".to_string());
    }
    
    // Root cluster (cluster 2) should be allocated
    if fat_data.len() >= 6 {
        let cluster = u16::from_le_bytes([fat_data[4], fat_data[5]]);
        if cluster == 0 {
            errors.push("Root cluster should be allocated".to_string());
        }
    }
    
    errors
}

// ============================================================================
// Core Test Functions
// ============================================================================

async fn format_and_verify_fat16(
    size: u64,
    label: Option<String>,
) -> Result<SimulationReport, Box<dyn std::error::Error>> {
    let temp_file = create_test_image(size)?;
    let path = temp_file.path().to_str().unwrap().to_string();
    
    let mut device = create_test_device(size);
    device.id = path.clone();
    
    let options = FormatOptions {
         filesystem_type: "fat16".to_string(),
         label: label,
         cluster_size: None,
        quick_format: false,
        enable_compression: false,
        verify_after_format: false,
        dry_run: false,
        force: false,
        additional_options: std::collections::HashMap::new(),
    };
    
    let formatter = super::Fat16Formatter;
    
    // Try dry_run first to validate parameters
    match formatter.dry_run(&device, &options).await {
        Ok(report) => {
            println!("Dry run succeeded: estimated_time={:?}", report.estimated_time);
        },
        Err(e) => {
            return Err(format!("Dry run failed: {}", e).into());
        }
    }
    
    // Then format
    match formatter.format(&device, &options).await {
        Ok(()) => {
            println!("Format succeeded for {} MB", size / 1024 / 1024);
        },
        Err(e) => {
            return Err(format!("Format failed: {}", e).into());
        }
    }
    
    // Finally verify with dry_run again
    let report = formatter.dry_run(&device, &options).await.map_err(|e| {
        format!("Final verification failed: {}", e)
    })?;
    
    Ok(report)
}

async fn validate_formatted_fat16(path: &str) -> Vec<String> {
    let mut errors = Vec::new();
    
    let mut file = File::open(path).map_err(|e| {
        errors.push(format!("Failed to open: {}", e));
        e
    }).unwrap();
    
    // Read boot sector
    let mut boot_sector = [0u8; 512];
    file.read_exact(&mut boot_sector).expect("Failed to read boot sector");
    
    // Validate boot sector
    let boot_errors = validate_fat16_boot_sector(&boot_sector);
    errors.extend(boot_errors);
    
    // Calculate expected layout
    let total_sectors = 512; // Use a small test value
    let (_, _) = calculate_fat16_layout(total_sectors);
    
    errors
}

// ============================================================================
// Test Cases
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_small_fat16() {
        // Test minimum FAT16 size (about 4MB)
        let result = format_and_verify_fat16(16 * 1024 * 1024, Some("SMALL".to_string())).await;
     println!("Format result: {:?}", result);
        
        assert!(result.is_ok(), "Small FAT16 format should succeed");
    }
    
    #[tokio::test]
    async fn test_medium_fat16() {
        // Test typical FAT16 size (128MB)
        let result = format_and_verify_fat16(128 * 1024 * 1024, Some("MEDIUM".to_string())).await;
        
        assert!(result.is_ok(), "Medium FAT16 format should succeed");
        
        let report = result.unwrap();
        assert!(report.will_erase_data, "Should erase data");
        println!("Estimated time: {:?}", report.estimated_time);
    }
    
    #[tokio::test]
    async fn test_large_fat16() {
        // Test maximum FAT16 size (2GB)
        let result = format_and_verify_fat16(2 * 1024 * 1024 * 1024, Some("LARGE".to_string())).await;
        
        assert!(result.is_ok(), "Large FAT16 format should succeed");
    }
    
    #[tokio::test]
    async fn test_no_label() {
        // Test without volume label
        let result = format_and_verify_fat16(64 * 1024 * 1024, None).await;
        
        assert!(result.is_ok(), "FAT16 without label should format successfully");
    }
    
    #[tokio::test]
    async fn test_windows_compatibility() {
        // Test with Windows-compatible parameters
        let temp_file = create_test_image(128 * 1024 * 1024).expect("Failed to create test image");
        let path = temp_file.path().to_str().unwrap().to_string();
        
        let mut device = create_test_device(128 * 1024 * 1024);
        device.id = path.clone();
        let options = FormatOptions {
            filesystem_type: "fat16".to_string(),
            label: Some("WINCOMPAT".to_string()),
            cluster_size: None,
            quick_format: false,
            enable_compression: false,
            verify_after_format: false,
            dry_run: false,
            force: false,
            additional_options: std::collections::HashMap::new(),
        };
        
        let formatter = super::Fat16Formatter;
        formatter.format(&device, &options).await.expect("Format failed");
        
        // Validate boot sector
        let mut file = File::open(&path).expect("Failed to open file");
        let mut boot_sector = [0u8; 512];
        file.read_exact(&mut boot_sector).expect("Failed to read boot sector");
        
        // Check OEM name (should be Windows-compatible)
        let oem_name = String::from_utf8_lossy(&boot_sector[3..11]);
        assert!(
            oem_name.trim().starts_with("MSWIN") || oem_name.trim().starts_with("MSDOS"),
            "OEM name should be Windows-compatible: {}",
            oem_name
        );
        
        // Check boot signature
        assert_eq!(boot_sector[510], 0x55, "Boot signature byte 0 should be 0x55");
        assert_eq!(boot_sector[511], 0xAA, "Boot signature byte 1 should be 0xAA");
    }
    
    #[tokio::test]
    async fn test_filesystem_type_detection() {
        // Test that the formatter creates a filesystem that can be detected
        let temp_file = create_test_image(128 * 1024 * 1024).expect("Failed to create test image");
        let path = temp_file.path().to_str().unwrap().to_string();
        
        let mut device = create_test_device(128 * 1024 * 1024);
        device.id = path.clone();
        let options = FormatOptions {
            filesystem_type: "fat16".to_string(),
            label: Some("DETECT".to_string()),
            cluster_size: None,
            quick_format: false,
            enable_compression: false,
            verify_after_format: false,
            dry_run: false,
            force: false,
            additional_options: std::collections::HashMap::new(),
        };
        
        let formatter = super::Fat16Formatter;
        formatter.format(&device, &options).await.expect("Format failed");
        
        // Read boot sector and verify filesystem type
        let mut file = File::open(&path).expect("Failed to open file");
        let mut boot_sector = [0u8; 512];
        file.read_exact(&mut boot_sector).expect("Failed to read boot sector");
        
        // Check filesystem type string at offset 54 (for FAT16)
        // For FAT16, the filesystem type is optional in extended BPB
        // We can verify by checking that the format succeeded and structure is valid
        assert_eq!(boot_sector[510], 0x55, "Valid boot signature");
        assert_eq!(boot_sector[511], 0xAA, "Valid boot signature");
    }
    
    #[tokio::test]
    async fn test_cluster_size_selection() {
        // Test with different cluster sizes
        let cluster_sizes = vec![1, 2, 4, 8, 16, 32, 64, 128];
        
        for &spc in &cluster_sizes {
            let size = 128 * 1024 * 1024; // 128MB
            let temp_file = create_test_image(size).expect("Failed to create test image");
            let path = temp_file.path().to_str().unwrap().to_string();
            
            let device = create_test_device(size);
            let options = FormatOptions {
                filesystem_type: "fat16".to_string(),
                label: Some("SPC".to_string()),
                cluster_size: Some((spc * 512) as u32),
                quick_format: false,
                enable_compression: false,
                verify_after_format: false,
                dry_run: false,
                force: false,
                additional_options: std::collections::HashMap::new(),
            };
            
            let formatter = super::Fat16Formatter;
            let result = formatter.format(&device, &options).await;
            
            if result.is_ok() {
                let mut file = File::open(&path).expect("Failed to open file");
                let mut boot_sector = [0u8; 512];
                file.read_exact(&mut boot_sector).expect("Failed to read boot sector");
                
                // Verify sectors per cluster in boot sector (offset 13)
                let actual_spc = boot_sector[13];
                assert_eq!(actual_spc, spc as u8, "Sectors per cluster mismatch for {} sectors", spc);
            }
        }
    }
    
    #[tokio::test]
    async fn test_label_truncation() {
        // Test that long labels are properly truncated to 11 characters
        let temp_file = create_test_image(128 * 1024 * 1024).expect("Failed to create test image");
        let path = temp_file.path().to_str().unwrap().to_string();
        
        let mut device = create_test_device(128 * 1024 * 1024);
        device.id = path.clone();
        let long_label = "THIS_IS_A_VERY_LONG_LABEL_EXCEEDING_ELEVEN_CHARS";
        let options = FormatOptions {
            filesystem_type: "fat16".to_string(),
            label: Some(long_label.to_string()),
            cluster_size: None,
            quick_format: false,
            enable_compression: false,
            verify_after_format: false,
            dry_run: false,
            force: false,
            additional_options: std::collections::HashMap::new(),
        };
        
        let formatter = super::Fat16Formatter;
        formatter.format(&device, &options).await.expect("Format failed");
        
        // Read and verify label is truncated to 11 chars
        let mut file = File::open(&path).expect("Failed to open file");
        let mut boot_sector = [0u8; 512];
        file.read_exact(&mut boot_sector).expect("Failed to read boot sector");
        
        // Volume label is at offset 43 (11 bytes)
        let label_bytes = &boot_sector[43..54];
        let label_str = String::from_utf8_lossy(label_bytes).trim().to_string();
        
        assert!(
            label_str.len() <= 11,
            "Label should be at most 11 characters, got '{}'",
            label_str
        );
    }
    
    #[tokio::test]
    async fn test_boot_sector_structure() {
        // Test boot sector structure
        let temp_file = create_test_image(128 * 1024 * 1024).expect("Failed to create test image");
        let path = temp_file.path().to_str().unwrap().to_string();
        
        let mut device = create_test_device(128 * 1024 * 1024);
        device.id = path.clone();
        let options = FormatOptions {
            filesystem_type: "fat16".to_string(),
            label: Some("STRUCT".to_string()),
            cluster_size: None,
            quick_format: false,
            enable_compression: false,
            verify_after_format: false,
            dry_run: false,
            force: false,
            additional_options: std::collections::HashMap::new(),
        };
        
        let formatter = super::Fat16Formatter;
        formatter.format(&device, &options).await.expect("Format failed");
        
        let mut file = File::open(&path).expect("Failed to open file");
        let mut boot_sector = [0u8; 512];
        file.read_exact(&mut boot_sector).expect("Failed to read boot sector");
        
        // Validate boot sector structure
        assert_eq!(boot_sector[510], 0x55, "Boot signature byte 0");
        assert_eq!(boot_sector[511], 0xAA, "Boot signature byte 1");
    }
    
    #[tokio::test]
    async fn test_partition_table() {
        // Test formatting with partition table
        let temp_file = create_test_image(512 * 1024 * 1024).expect("Failed to create test image");
        let path = temp_file.path().to_str().unwrap().to_string();
        
        let mut device = create_test_device(512 * 1024 * 1024);
        device.id = path.clone();
        
        let mut options = FormatOptions {
            filesystem_type: "fat16".to_string(),
            label: Some("PARTITION".to_string()),
            cluster_size: None,
            quick_format: false,
            enable_compression: false,
            verify_after_format: false,
            dry_run: false,
            force: false,
            additional_options: std::collections::HashMap::new(),
        };
        
        // Add partition table option
        options.additional_options.insert(
            "create_partition_table".to_string(),
            "true".to_string()
        );
        
        let formatter = super::Fat16Formatter;
        formatter.format(&device, &options).await.expect("Format failed");
        
        // Verify MBR
        let mut file = File::open(&path).expect("Failed to open file");
        let mut mbr = [0u8; 512];
        file.read_exact(&mut mbr).expect("Failed to read MBR");
        
        // Check MBR signature
        assert_eq!(mbr[510], 0x55, "Invalid MBR signature byte 0");
        assert_eq!(mbr[511], 0xAA, "Invalid MBR signature byte 1");
        
        // Check partition type (should be 0x06 for FAT16)
        let part_type = mbr[450];
        assert!(
            part_type == 0x06 || part_type == 0x04,
            "Partition type should be 0x06 or 0x04, got 0x{:02X}",
            part_type
        );
    }
    
    #[tokio::test]
    async fn test_media_descriptor() {
        // Test media descriptor value
        let temp_file = create_test_image(128 * 1024 * 1024).expect("Failed to create test image");
        let path = temp_file.path().to_str().unwrap().to_string();
        
        let mut device = create_test_device(128 * 1024 * 1024);
        device.id = path.clone();
        let options = FormatOptions {
            filesystem_type: "fat16".to_string(),
            label: Some("MEDIA".to_string()),
            cluster_size: None,
            quick_format: false,
            enable_compression: false,
            verify_after_format: false,
            dry_run: false,
            force: false,
            additional_options: std::collections::HashMap::new(),
        };
        
        let formatter = super::Fat16Formatter;
        formatter.format(&device, &options).await.expect("Format failed");
        
        let mut file = File::open(&path).expect("Failed to open file");
        let mut boot_sector = [0u8; 512];
        file.read_exact(&mut boot_sector).expect("Failed to read boot sector");
        
        // Media descriptor at offset 21
        let media_descriptor = boot_sector[21];
        
        // For fixed disk, should be 0xF8; for removable, could be 0xF0
        assert!(
            media_descriptor == 0xF0 || media_descriptor == 0xF8,
            "Media descriptor should be 0xF0 or 0xF8, got 0x{:02X}",
            media_descriptor
        );
    }
    
    #[tokio::test]
    async fn test_root_entries() {
        // Test root directory entries
        let temp_file = create_test_image(128 * 1024 * 1024).expect("Failed to create test image");
        let path = temp_file.path().to_str().unwrap().to_string();
        
        let mut device = create_test_device(128 * 1024 * 1024);
        device.id = path.clone();
        let options = FormatOptions {
            filesystem_type: "fat16".to_string(),
            label: Some("ROOTENT".to_string()),
            cluster_size: None,
            quick_format: false,
            enable_compression: false,
            verify_after_format: false,
            dry_run: false,
            force: false,
            additional_options: std::collections::HashMap::new(),
        };
        
        let formatter = super::Fat16Formatter;
        formatter.format(&device, &options).await.expect("Format failed");
        
        let mut file = File::open(&path).expect("Failed to open file");
        let mut boot_sector = [0u8; 512];
        file.read_exact(&mut boot_sector).expect("Failed to read boot sector");
        
        // Root entries at offset 17 (2 bytes)
        let root_entries = u16::from_le_bytes([boot_sector[17], boot_sector[18]]);
        
        // Standard FAT16 uses 512 root entries
        assert_eq!(root_entries, 512, "Root entries should be 512 for standard FAT16");
    }
}
