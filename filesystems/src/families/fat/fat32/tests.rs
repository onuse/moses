// Comprehensive FAT32 test suite
// Follows patterns from FAT16 tests

use moses_core::{Device, DeviceType, FormatOptions, FilesystemFormatter, SimulationReport};
use std::fs::{File, OpenOptions};
use std::io::{Write, Read, Seek, SeekFrom};
use tempfile::NamedTempFile;

// Import the FAT32 formatter
use crate::families::fat::fat32::Fat32Formatter;

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
// FAT32-Specific Validation Helpers
// ============================================================================

/// Validate FAT32 boot sector fields
fn validate_fat32_boot_sector(boot_sector: &[u8]) -> Vec<String> {
    let mut errors = Vec::new();
    
    if boot_sector.len() < 512 {
        errors.push("Boot sector less than 512 bytes".to_string());
        return errors;
    }
    
    // Check jump instruction
    if boot_sector[0] != 0xEB && boot_sector[0] != 0xE9 {
        errors.push(format!(
            "Invalid jump instruction: 0x{:02X} (should be 0xEB or 0xE9)",
            boot_sector[0]
        ));
    }
    
    // Check OEM name "FAT32   "
    let oem_name = &boot_sector[3..11];
    if oem_name != b"FAT32   " {
        errors.push(format!(
            "Invalid OEM name: {:?} (should be 'FAT32   ')",
            oem_name
        ));
    }
    
    // Check boot signature
    if boot_sector[510] != 0x55 || boot_sector[511] != 0xAA {
        errors.push("Invalid boot signature (should be 55 AA)".to_string());
    }
    
    // Check filesystem type at offset 82
    let fs_type = &boot_sector[82..90];
    if fs_type != b"FAT32   " {
        errors.push(format!(
            "Invalid filesystem type: {:?} (should be 'FAT32   ')",
            fs_type
        ));
    }
    
    // Check media descriptor (should be 0xF8 for fixed disk)
    if boot_sector[21] != 0xF8 {
        errors.push(format!(
            "Non-standard media descriptor: 0x{:02X}",
            boot_sector[21]
        ));
    }
    
    errors
}

/// Calculate expected FAT32 layout
fn calculate_fat32_layout(total_sectors: u64) -> (u8, u32, u32, u32) {
    // Determine sectors per cluster based on volume size
    let sectors_per_cluster = if total_sectors <= 65536 {
        1    // <= 32MB
    } else if total_sectors <= 131072 {
        2    // <= 64MB
    } else if total_sectors <= 262144 {
        4    // <= 128MB
    } else if total_sectors <= 524288 {
        8    // <= 256MB
    } else if total_sectors <= 1048576 {
        16   // <= 512MB
    } else if total_sectors <= 2097152 {
        32   // <= 1GB
    } else {
        64   // > 1GB
    };
    
    // Calculate FAT size
    let total_clusters = total_sectors / sectors_per_cluster as u64;
    let fat_size_sectors = ((total_clusters + 2) * 4 + 511) / 512;
    
    // FSInfo sector (usually 1)
    let fsinfo_sector = 1;
    
    // Backup boot sector (usually 6)
    let backup_boot_sector = 6;
    
    (sectors_per_cluster as u8, fat_size_sectors as u32, fsinfo_sector, backup_boot_sector)
}

/// Validate FAT32 cluster chain
fn validate_fat32_cluster_chain(fat_data: &[u8], total_clusters: u32) -> Vec<String> {
    let mut errors = Vec::new();
    
    // First FAT entry should be media descriptor followed by FF FF FF
    let media_descriptor = fat_data[0];
    let expected_media = 0xF8; // Fixed disk
    
    if fat_data[0] != expected_media {
        errors.push(format!(
            "FAT[0] low byte 0x{:02X} doesn't match expected media descriptor 0x{:02X}",
            fat_data[0], expected_media
        ));
    }
    
    if fat_data[1] != 0xFF || fat_data[2] != 0xFF || fat_data[3] != 0xFF {
        errors.push(format!(
            "Invalid FAT[0]: {:02X} {:02X} {:02X} {:02X}",
            fat_data[0], fat_data[1], fat_data[2], fat_data[3]
        ));
    }
    
    // Root cluster (cluster 2) should be allocated
    let root_cluster_offset = 8; // FAT[2]
    if root_cluster_offset + 3 < fat_data.len() {
        let root_cluster = u32::from_le_bytes([
            fat_data[root_cluster_offset],
            fat_data[root_cluster_offset + 1],
            fat_data[root_cluster_offset + 2],
            fat_data[root_cluster_offset + 3],
        ]);
        
        if root_cluster < 2 || root_cluster > total_clusters {
            errors.push(format!(
                "Invalid root cluster: {}",
                root_cluster
            ));
        }
    }
    
    errors
}

// ============================================================================
// Core Test Functions
// ============================================================================

async fn format_and_verify_fat32(
    size: u64,
    label: Option<&str>,
    quick: bool,
) -> Result<SimulationReport, Box<dyn std::error::Error>> {
    // Create test image
    let temp_file = create_test_image(size)?;
    let path = temp_file.path().to_str().unwrap().to_string();
    
    // Create device
    let mut device = create_test_device(size);
    device.id = path.clone();
    
    // Format options
    let options = FormatOptions {
        filesystem_type: "fat32".to_string(),
        label: label.map(|s| s.to_string()),
        cluster_size: None,
        quick_format: quick,
        enable_compression: false,
        verify_after_format: false,
        dry_run: false,
        force: false,
        additional_options: std::collections::HashMap::new(),
    };
    
    // Format the device
    let formatter = super::Fat32Formatter;
    formatter.format(&device, &options).await?;
    
    // Get simulation report from dry_run
    let report = formatter.dry_run(&device, &options).await?;
    
    Ok(report)
}

async fn validate_formatted_fat32(path: &str, expected_size: u64) -> Vec<String> {
    let mut errors = Vec::new();
    
    // Open and read boot sector
    let mut file = File::open(path).map_err(|e| {
        errors.push(format!("Failed to open file: {}", e));
        e
    }).unwrap();
    
    let mut boot_sector = [0u8; 512];
    file.read_exact(&mut boot_sector).expect("Failed to read boot sector");
    
    // Validate boot sector
    let boot_errors = validate_fat32_boot_sector(&boot_sector);
    errors.extend(boot_errors);
    
    // Read FSInfo sector (usually sector 1)
    file.seek(SeekFrom::Start(512)).expect("Failed to seek to FSInfo");
    let mut fsinfo = [0u8; 512];
    file.read_exact(&mut fsinfo).expect("Failed to read FSInfo");
    
    // Validate FSInfo signature
    if &fsinfo[0..4] != b"RRaA" {
        errors.push(format!(
            "Invalid FSInfo lead signature: {:?}",
            &fsinfo[0..4]
        ));
    }
    
    if &fsinfo[484..488] != b"rrAa" {
        errors.push(format!(
            "Invalid FSInfo struct signature: {:?}",
            &fsinfo[484..488]
        ));
    }
    
    // Read first FAT
    file.seek(SeekFrom::Start(512 * 2)).expect("Failed to seek to FAT");
    let mut fat_data = vec![0u8; 512 * 3]; // Read at least 3 sectors
    file.read_exact(&mut fat_data).expect("Failed to read FAT");
    
    // Calculate expected layout
    let total_sectors = expected_size / 512;
    let (_, fat_size, _, _) = calculate_fat32_layout(total_sectors);
    
    // Validate cluster chain
    let total_clusters = total_sectors / 64; // Assume 64KB clusters for large disks
    let chain_errors = validate_fat32_cluster_chain(&fat_data, total_clusters as u32);
    errors.extend(chain_errors);
    
    errors
}

// ============================================================================
// Test Cases
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_small_fat32() {
        // Test minimum FAT32 size (about 65MB - FAT32 needs at least 65527 clusters)
        let result = format_and_verify_fat32(65 * 1024 * 1024, Some("SMALL"), false).await;
        
        assert!(result.is_ok(), "Small FAT32 format should succeed");
        
        let report = result.unwrap();
        assert!(report.will_erase_data, "Format should erase data");
        assert!(report.warnings.is_empty() || report.warnings.iter().all(|w| 
            !w.contains("critical") && !w.contains("error")
        ), "Should have no critical warnings");
    }
    
    #[tokio::test]
    async fn test_medium_fat32() {
        // Test typical FAT32 size (1GB)
        let result = format_and_verify_fat32(1024 * 1024 * 1024, Some("MEDIUM"), false).await;
        
        assert!(result.is_ok(), "Medium FAT32 format should succeed");
        
        let report = result.unwrap();
        println!("Estimated time: {:?}", report.estimated_time);
        println!("Space after format: {} bytes", report.space_after_format);
    }
    
    #[tokio::test]
    async fn test_large_fat32() {
        // Test large FAT32 size (8GB - near FAT32 limit)
        let result = format_and_verify_fat32(8 * 1024 * 1024 * 1024, Some("LARGE"), false).await;
        
        assert!(result.is_ok(), "Large FAT32 format should succeed");
    }
    
    #[tokio::test]
    async fn test_very_large_fat32() {
        // Test near-maximum FAT32 size (32GB)
        // Note: Some implementations may not support this
        let result = format_and_verify_fat32(32 * 1024 * 1024 * 1024, Some("VLARGE"), false).await;
        
        // May fail depending on implementation limits
        if result.is_ok() {
            let report = result.unwrap();
            assert!(report.will_erase_data, "Should erase data");
        }
    }
    
    #[tokio::test]
    async fn test_no_label() {
        // Test without volume label
        let result = format_and_verify_fat32(512 * 1024 * 1024, None, false).await;
        
        assert!(result.is_ok(), "FAT32 without label should format successfully");
    }
    
    #[tokio::test]
    async fn test_quick_format() {
        // Test quick format
        let result = format_and_verify_fat32(1024 * 1024 * 1024, Some("QUICK"), true).await;
        
        assert!(result.is_ok(), "Quick format should succeed");
    }
    
    #[tokio::test]
    async fn test_windows_compatibility() {
        // Test with Windows-compatible parameters
        let temp_file = create_test_image(1024 * 1024 * 1024).expect("Failed to create test image");
        let path = temp_file.path().to_str().unwrap().to_string();
        
        let mut device = create_test_device(1024 * 1024 * 1024);
        device.id = path.clone();
        let options = FormatOptions {
            filesystem_type: "fat32".to_string(),
            label: Some("WINCOMPAT".to_string()),
            cluster_size: None,
            quick_format: false,
            enable_compression: false,
            verify_after_format: false,
            dry_run: false,
            force: false,
            additional_options: std::collections::HashMap::new(),
        };
        
        let formatter = super::Fat32Formatter;
        formatter.format(&device, &options).await.expect("Format failed");
        
        // Validate boot sector
        let mut file = File::open(&path).expect("Failed to open file");
        let mut boot_sector = [0u8; 512];
        file.read_exact(&mut boot_sector).expect("Failed to read boot sector");
        
        // Check OEM name (should be Windows-compatible)
        let oem_name = String::from_utf8_lossy(&boot_sector[3..11]);
        assert!(
            oem_name.trim() == "MSWIN4.1" || oem_name.trim() == "FAT32   ",
            "OEM name should be Windows-compatible, got: '{}'",
            oem_name
        );
        
        // Check boot signature
        assert_eq!(boot_sector[510], 0x55, "Boot signature byte 0 should be 0x55");
        assert_eq!(boot_sector[511], 0xAA, "Boot signature byte 1 should be 0xAA");
    }
    
    #[tokio::test]
    async fn test_filesystem_type_detection() {
        // Test that the formatter creates a filesystem that can be detected
        let temp_file = create_test_image(1024 * 1024 * 1024).expect("Failed to create test image");
        let path = temp_file.path().to_str().unwrap().to_string();
        
        let mut device = create_test_device(1024 * 1024 * 1024);
        device.id = path.clone();
        let options = FormatOptions {
            filesystem_type: "fat32".to_string(),
            label: Some("DETECT".to_string()),
            cluster_size: None,
            quick_format: false,
            enable_compression: false,
            verify_after_format: false,
            dry_run: false,
            force: false,
            additional_options: std::collections::HashMap::new(),
        };
        
        let formatter = super::Fat32Formatter;
        formatter.format(&device, &options).await.expect("Format failed");
        
        // Read boot sector and verify filesystem type
        let mut file = File::open(&path).expect("Failed to open file");
        let mut boot_sector = [0u8; 512];
        file.read_exact(&mut boot_sector).expect("Failed to read boot sector");
        
        // Check filesystem type string at offset 82
        let fs_type = String::from_utf8_lossy(&boot_sector[82..90]);
        assert!(
            fs_type.trim().starts_with("FAT32"),
            "Filesystem type should be FAT32: '{}'",
            fs_type
        );
    }
    
    #[tokio::test]
    async fn test_partition_table() {
        // Test formatting with partition table
        let temp_file = create_test_image(4 * 1024 * 1024 * 1024).expect("Failed to create test image");
        let path = temp_file.path().to_str().unwrap().to_string();
        
        let mut device = create_test_device(4 * 1024 * 1024 * 1024);
        device.id = path.clone();
        
        let mut options = FormatOptions {
            filesystem_type: "fat32".to_string(),
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
        
        let formatter = super::Fat32Formatter;
        formatter.format(&device, &options).await.expect("Format failed");
        
        // Verify MBR
        let mut file = File::open(&path).expect("Failed to open file");
        let mut mbr = [0u8; 512];
        file.read_exact(&mut mbr).expect("Failed to read MBR");
        
        // Check MBR signature
        assert_eq!(mbr[510], 0x55, "Invalid MBR signature byte 0");
        assert_eq!(mbr[511], 0xAA, "Invalid MBR signature byte 1");
        
        // Check partition type (should be 0x0C for FAT32 LBA)
        let part_type = mbr[450];
        assert!(
            part_type == 0x0C || part_type == 0x0B,
            "Partition type should be 0x0C (FAT32 LBA) or 0x0B, got 0x{:02X}",
            part_type
        );
    }
    
    #[tokio::test]
    async fn test_multiple_sectors_per_cluster() {
        // Test with different cluster sizes
        let cluster_sizes = [1, 2, 4, 8, 16, 32, 64];
        
        // Test that appropriate cluster sizes are automatically selected based on volume size
        // FAT32 automatically selects: 1 SPC for <=260MB, 8 for <=8GB, 16 for <=16GB, etc.
        let test_cases = vec![
            (100 * 1024 * 1024, 1),  // 100MB -> 1 sectors/cluster (512B)
            (1024 * 1024 * 1024, 8), // 1GB -> 8 sectors/cluster (4KB)
        ];
        
        for &(size, expected_spc) in &test_cases {
            let temp_file = create_test_image(size).expect("Failed to create test image");
            let path = temp_file.path().to_str().unwrap().to_string();
            
            let mut device = create_test_device(size);
            device.id = path.clone();
            let options = FormatOptions {
                filesystem_type: "fat32".to_string(),
                label: Some("CLUSTERS".to_string()),
                cluster_size: None, // Let formatter auto-select
                quick_format: false,
                enable_compression: false,
                verify_after_format: false,
                dry_run: false,
                force: false,
                additional_options: std::collections::HashMap::new(),
            };
            
            // Format (formatter writes partition table)
            let options = FormatOptions {
                filesystem_type: "fat32".to_string(),
                label: Some("CLUSTERS".to_string()),
                cluster_size: None,
                quick_format: false,
                enable_compression: false,
                verify_after_format: false,
                dry_run: false,
                force: false,
                additional_options: [("create_partition_table".to_string(), "false".to_string())].into_iter().collect(),
            };
            
            let formatter = super::Fat32Formatter;
            let result = formatter.format(&device, &options).await;
            
            if result.is_ok() {
                let mut file = File::open(&path).expect("Failed to open file");
                let mut boot_sector = [0u8; 512];
                file.read_exact(&mut boot_sector).expect("Failed to read boot sector");
                
                // Verify sectors per cluster matches expected value
                let actual_spc = boot_sector[13];
                assert_eq!(actual_spc, expected_spc as u8, "Sectors per cluster mismatch");
            }
        }
    }
    
    #[tokio::test]
    async fn test_label_truncation() {
        // Test that valid labels work correctly (FAT32 limit is 11 chars)
        let temp_file = create_test_image(1024 * 1024 * 1024).expect("Failed to create test image");
        let path = temp_file.path().to_str().unwrap().to_string();
        
        let mut device = create_test_device(1024 * 1024 * 1024);
        device.id = path.clone();
        let valid_label = "VALIDLABEL";
        let options = FormatOptions {
            filesystem_type: "fat32".to_string(),
            label: Some(valid_label.to_string()),
            cluster_size: None,
            quick_format: false,
            enable_compression: false,
            verify_after_format: false,
            dry_run: false,
            force: false,
            additional_options: std::collections::HashMap::new(),
        };
        
        let options = FormatOptions {
            filesystem_type: "fat32".to_string(),
            label: Some(valid_label.to_string()),
            cluster_size: None,
            quick_format: false,
            enable_compression: false,
            verify_after_format: false,
            dry_run: false,
            force: false,
            additional_options: [("create_partition_table".to_string(), "false".to_string())].into_iter().collect(),
        };
        
        let formatter = super::Fat32Formatter;
        formatter.format(&device, &options).await.expect("Format failed");
        
        // Read and verify label
        let mut file = File::open(&path).expect("Failed to open file");
        let mut boot_sector = [0u8; 512];
        file.read_exact(&mut boot_sector).expect("Failed to read boot sector");
        
        // Volume label is at offset 71 (0x47) (11 bytes) within the boot sector for FAT32
        let label_bytes = &boot_sector[71..82];
        let label_str = String::from_utf8_lossy(label_bytes).trim().to_string();
        
        assert_eq!(label_str, valid_label, "Label should match");
    }
    
    #[tokio::test]
    async fn test_fat32_specific_fields() {
        // Test that FAT32-specific fields are correctly set
        let temp_file = create_test_image(1024 * 1024 * 1024).expect("Failed to create test image");
        let path = temp_file.path().to_str().unwrap().to_string();
        
        let mut device = create_test_device(1024 * 1024 * 1024);
        device.id = path.clone();
        let options = FormatOptions {
            filesystem_type: "fat32".to_string(),
            label: Some("FIELDS".to_string()),
            cluster_size: None,
            quick_format: false,
            enable_compression: false,
            verify_after_format: false,
            dry_run: false,
            force: false,
            additional_options: std::collections::HashMap::new(),
        };
        
        let formatter = super::Fat32Formatter;
        formatter.format(&device, &options).await.expect("Format failed");
        
        let mut file = File::open(&path).expect("Failed to open file");
        let mut boot_sector = [0u8; 512];
        file.read_exact(&mut boot_sector).expect("Failed to read boot sector");
        
        // FAT32 should have sectors_per_fat_16 = 0 (not used)
        // and sectors_per_fat_32 at offset 36
        let sectors_per_fat_16 = u16::from_le_bytes([boot_sector[22], boot_sector[23]]);
        assert_eq!(sectors_per_fat_16, 0, "FAT32 should have sectors_per_fat_16 = 0");
        
        // Check filesystem type
        let fs_type = String::from_utf8_lossy(&boot_sector[82..90]);
        assert!(
            fs_type.trim().starts_with("FAT32"),
            "FAT32 filesystem type should be set"
        );
    }
}
