// Comprehensive MBR verification tests
use super::mbr_verifier::*;

#[test]
fn test_valid_mbr_with_disk_signature() {
    let mut mbr = vec![0u8; 512];
    
    // Add MBR signature
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    
    // Add disk signature
    mbr[440] = 0x12;
    mbr[441] = 0x34;
    mbr[442] = 0x56;
    mbr[443] = 0x78;
    
    // Add a FAT16 partition
    let offset = 446;
    mbr[offset] = 0x80;  // Bootable
    mbr[offset + 4] = 0x06;  // FAT16
    mbr[offset + 8] = 0x00;  // Start LBA 2048
    mbr[offset + 9] = 0x08;
    mbr[offset + 12] = 0x00;  // Size 1048576 sectors (512MB)
    mbr[offset + 13] = 0x00;
    mbr[offset + 14] = 0x10;
    mbr[offset + 15] = 0x00;
    
    let result = MbrVerifier::verify_mbr(&mbr);
    
    assert!(result.is_valid);
    assert!(result.has_disk_signature);
    assert_eq!(result.partitions.len(), 1);
    assert_eq!(result.partitions[0].partition_type, 0x06);
    assert_eq!(result.partitions[0].start_lba, 2048);
    assert!(result.partitions[0].bootable);
}

#[test]
fn test_missing_mbr_signature() {
    let mbr = vec![0u8; 512];  // No 55AA signature
    
    let result = MbrVerifier::verify_mbr(&mbr);
    
    assert!(!result.is_valid);
    assert!(result.errors.iter().any(|e| e.contains("Invalid MBR signature")));
}

#[test]
fn test_missing_disk_signature_warning() {
    let mut mbr = vec![0u8; 512];
    
    // Add MBR signature but no disk signature
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    
    let result = MbrVerifier::verify_mbr(&mbr);
    
    assert!(result.is_valid);  // Still valid, but with warning
    assert!(!result.has_disk_signature);
    assert!(result.warnings.iter().any(|w| w.contains("No disk signature")));
}

#[test]
fn test_overlapping_partitions() {
    let mut mbr = vec![0u8; 512];
    
    // Add MBR signature
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    
    // Add disk signature
    mbr[440] = 0xAB;
    mbr[441] = 0xCD;
    mbr[442] = 0xEF;
    mbr[443] = 0x01;
    
    // Partition 1: starts at LBA 2048, size 10000
    let offset1 = 446;
    mbr[offset1 + 4] = 0x06;  // FAT16
    mbr[offset1 + 8] = 0x00;   // Start LBA 2048
    mbr[offset1 + 9] = 0x08;
    mbr[offset1 + 12] = 0x10;  // Size 10000
    mbr[offset1 + 13] = 0x27;
    
    // Partition 2: starts at LBA 8000, size 10000 (overlaps with partition 1)
    let offset2 = 446 + 16;
    mbr[offset2 + 4] = 0x07;  // NTFS
    mbr[offset2 + 8] = 0x40;   // Start LBA 8000
    mbr[offset2 + 9] = 0x1F;
    mbr[offset2 + 12] = 0x10;  // Size 10000
    mbr[offset2 + 13] = 0x27;
    
    let result = MbrVerifier::verify_mbr(&mbr);
    
    assert!(!result.is_valid);
    assert!(result.errors.iter().any(|e| e.contains("overlap")));
}

#[test]
fn test_multiple_non_overlapping_partitions() {
    let mut mbr = vec![0u8; 512];
    
    // Add MBR signature
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    
    // Add disk signature
    mbr[440] = 0x11;
    mbr[441] = 0x22;
    mbr[442] = 0x33;
    mbr[443] = 0x44;
    
    // Partition 1: FAT16 at LBA 2048, size 100000
    let offset1 = 446;
    mbr[offset1] = 0x80;  // Bootable
    mbr[offset1 + 4] = 0x06;  // FAT16
    mbr[offset1 + 8] = 0x00;   // Start LBA 2048
    mbr[offset1 + 9] = 0x08;
    mbr[offset1 + 12] = 0xA0;  // Size 100000
    mbr[offset1 + 13] = 0x86;
    mbr[offset1 + 14] = 0x01;
    
    // Partition 2: NTFS at LBA 102048, size 200000
    let offset2 = 446 + 16;
    mbr[offset2 + 4] = 0x07;  // NTFS
    mbr[offset2 + 8] = 0xA0;   // Start LBA 102048
    mbr[offset2 + 9] = 0x8E;
    mbr[offset2 + 10] = 0x01;
    mbr[offset2 + 12] = 0x40;  // Size 200000
    mbr[offset2 + 13] = 0x0D;
    mbr[offset2 + 14] = 0x03;
    
    // Partition 3: Linux at LBA 302048, size 300000
    let offset3 = 446 + 32;
    mbr[offset3 + 4] = 0x83;  // Linux
    mbr[offset3 + 8] = 0xE0;   // Start LBA 302048
    mbr[offset3 + 9] = 0x9C;
    mbr[offset3 + 10] = 0x04;
    mbr[offset3 + 12] = 0xE0;  // Size 300000
    mbr[offset3 + 13] = 0x93;
    mbr[offset3 + 14] = 0x04;
    
    let result = MbrVerifier::verify_mbr(&mbr);
    
    assert!(result.is_valid);
    assert!(result.has_disk_signature);
    assert_eq!(result.partitions.len(), 3);
    assert_eq!(result.partitions[0].partition_type, 0x06);
    assert_eq!(result.partitions[1].partition_type, 0x07);
    assert_eq!(result.partitions[2].partition_type, 0x83);
    assert!(result.partitions[0].bootable);
    assert!(!result.partitions[1].bootable);
    assert!(!result.partitions[2].bootable);
}

#[test]
fn test_empty_mbr_with_signature() {
    let mut mbr = vec![0u8; 512];
    
    // Add MBR signature
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    
    // Add disk signature
    mbr[440] = 0xDE;
    mbr[441] = 0xAD;
    mbr[442] = 0xBE;
    mbr[443] = 0xEF;
    
    // No partitions
    
    let result = MbrVerifier::verify_mbr(&mbr);
    
    assert!(result.is_valid);
    assert!(result.has_disk_signature);
    assert_eq!(result.partitions.len(), 0);
    assert!(result.warnings.iter().any(|w| w.contains("No partitions")));
}

#[test]
fn test_invalid_mbr_size() {
    let mbr = vec![0u8; 256];  // Wrong size
    
    let result = MbrVerifier::verify_mbr(&mbr);
    
    assert!(!result.is_valid);
    assert!(result.errors.iter().any(|e| e.contains("Invalid MBR size")));
}

#[test]
fn test_partition_alignment_warning() {
    let mut mbr = vec![0u8; 512];
    
    // Add MBR signature
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    
    // Add disk signature
    mbr[440] = 0x99;
    mbr[441] = 0x88;
    mbr[442] = 0x77;
    mbr[443] = 0x66;
    
    // Add partition with non-standard alignment (starts at LBA 100)
    let offset = 446;
    mbr[offset + 4] = 0x06;  // FAT16
    mbr[offset + 8] = 0x64;  // Start LBA 100 (not aligned to 2048 or 63)
    mbr[offset + 9] = 0x00;
    mbr[offset + 12] = 0x00;  // Size
    mbr[offset + 13] = 0x10;
    
    let result = MbrVerifier::verify_mbr(&mbr);
    
    assert!(result.is_valid);
    assert!(result.warnings.iter().any(|w| w.contains("not aligned")));
}

#[test]
fn test_report_generation() {
    let mut mbr = vec![0u8; 512];
    
    // Add MBR signature
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    
    // Add disk signature
    mbr[440] = 0xCA;
    mbr[441] = 0xFE;
    mbr[442] = 0xBA;
    mbr[443] = 0xBE;
    
    // Add a FAT16 partition
    let offset = 446;
    mbr[offset] = 0x80;  // Bootable
    mbr[offset + 4] = 0x06;  // FAT16
    mbr[offset + 8] = 0x00;  // Start LBA 2048
    mbr[offset + 9] = 0x08;
    mbr[offset + 12] = 0x00;  // Size 1048576 sectors (512MB)
    mbr[offset + 13] = 0x00;
    mbr[offset + 14] = 0x10;
    
    let result = MbrVerifier::verify_mbr(&mbr);
    let report = MbrVerifier::generate_report(&result);
    
    assert!(report.contains("VALID"));
    assert!(report.contains("Disk Signature: Present"));
    assert!(report.contains("Partitions: 1"));
    assert!(report.contains("FAT16"));
    assert!(report.contains("Bootable: true"));
}

#[test]
fn test_all_partition_types() {
    let mut mbr = vec![0u8; 512];
    
    // Add MBR signature
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    
    // Add disk signature
    mbr[440] = 0xAA;
    mbr[441] = 0xBB;
    mbr[442] = 0xCC;
    mbr[443] = 0xDD;
    
    // Test different partition types
    let test_cases = vec![
        (0x06, "FAT16"),
        (0x07, "NTFS/exFAT"),
        (0x0C, "FAT32 (LBA)"),
        (0x83, "Linux"),
    ];
    
    for (i, (type_id, expected_name)) in test_cases.iter().enumerate() {
        let mut test_mbr = mbr.clone();
        let offset = 446 + (i * 16);
        test_mbr[offset + 4] = *type_id;
        test_mbr[offset + 8] = 0x00;  // Start LBA
        test_mbr[offset + 9] = 0x08;
        test_mbr[offset + 12] = 0x00;  // Size
        test_mbr[offset + 13] = 0x10;
        
        let result = MbrVerifier::verify_mbr(&test_mbr);
        assert!(result.is_valid);
        assert_eq!(result.partitions[0].partition_type, *type_id);
        assert!(result.partitions[0].type_name.contains(expected_name));
    }
}

#[test]
fn test_gpt_protective_mbr() {
    let mut mbr = vec![0u8; 512];
    
    // Add MBR signature
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    
    // Add disk signature
    mbr[440] = 0x01;
    mbr[441] = 0x23;
    mbr[442] = 0x45;
    mbr[443] = 0x67;
    
    // GPT Protective partition
    let offset = 446;
    mbr[offset + 4] = 0xEE;  // GPT Protective
    mbr[offset + 8] = 0x01;  // Start LBA 1
    mbr[offset + 12] = 0xFF;  // Maximum size
    mbr[offset + 13] = 0xFF;
    mbr[offset + 14] = 0xFF;
    mbr[offset + 15] = 0xFF;
    
    let result = MbrVerifier::verify_mbr(&mbr);
    
    assert!(result.is_valid);
    assert_eq!(result.partitions[0].partition_type, 0xEE);
    assert!(result.partitions[0].type_name.contains("GPT Protective"));
}