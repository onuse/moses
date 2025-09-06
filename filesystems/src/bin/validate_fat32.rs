// FAT32 validation tool
// NOTE: Fat32ComprehensiveValidator has been moved/removed during refactoring
// This tool needs to be updated to work with the new architecture

fn main() {
    eprintln!("FAT32 validator is currently unavailable.");
    eprintln!("The Fat32ComprehensiveValidator type was removed during filesystem refactoring.");
    eprintln!("This tool needs to be rewritten to work with the new architecture.");
    std::process::exit(1);
}

/* Original code preserved for reference:
// Usage: validate_fat32 <device_path>

use moses_filesystems::families::fat::fat32::validator::Fat32ComprehensiveValidator;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() != 2 {
        eprintln!("Usage: {} <device_path>", args[0]);
        eprintln!("Example: {} \\\\.\\PHYSICALDRIVE2", args[0]);
        std::process::exit(1);
    }
    
    let device_path = &args[1];
    
    println!("FAT32 Comprehensive Validator");
    println!("==============================");
    println!("Device: {}", device_path);
    println!();
    
    match Fat32ComprehensiveValidator::validate_filesystem(device_path) {
        Ok(report) => {
            println!("Overall Status: {:?}", report.overall_status);
            println!();
            
            println!("Common Boot Sector Fields:");
            println!("  Jump Instruction: {:?}", report.common_validation.jump_instruction);
            println!("  OEM Name: {:?}", report.common_validation.oem_name);
            println!("  Boot Signature: {:?}", report.common_validation.boot_signature);
            println!();
            
            println!("BPB Common Fields:");
            println!("  Bytes/Sector: {:?}", report.common_validation.common_bpb.bytes_per_sector);
            println!("  Sectors/Cluster: {:?}", report.common_validation.common_bpb.sectors_per_cluster);
            println!("  Reserved Sectors: {:?}", report.common_validation.common_bpb.reserved_sectors);
            println!("  Number of FATs: {:?}", report.common_validation.common_bpb.num_fats);
            println!("  Media Descriptor: {:?}", report.common_validation.common_bpb.media_descriptor);
            println!();
            
            println!("FAT32-Specific Fields:");
            for (field, result) in &report.specific_fields {
                println!("  {}: {:?}", field, result);
            }
            println!();
            
            println!("Cluster Count: {:?}", report.cluster_count);
            println!("Cluster Validation: {:?}", report.cluster_validation);
            println!();
            
            println!("FSInfo Validation: {:?}", report.fsinfo_validation);
            println!("FAT Table Validation: {:?}", report.fat_validation);
            
            // Print summary
            println!();
            match report.overall_status {
                moses_filesystems::families::fat::common::validator::ValidationStatus::Perfect => {
                    println!("✓ PERFECT: FAT32 filesystem is 100% compliant!");
                }
                moses_filesystems::families::fat::common::validator::ValidationStatus::Compliant => {
                    println!("✓ COMPLIANT: FAT32 filesystem meets specifications with minor warnings.");
                }
                moses_filesystems::families::fat::common::validator::ValidationStatus::PartiallyCompliant => {
                    println!("⚠ PARTIALLY COMPLIANT: FAT32 filesystem has some issues but may work.");
                }
                moses_filesystems::families::fat::common::validator::ValidationStatus::NonCompliant => {
                    println!("✗ NON-COMPLIANT: FAT32 filesystem has major violations!");
                }
                moses_filesystems::families::fat::common::validator::ValidationStatus::Corrupted => {
                    println!("✗ CORRUPTED: FAT32 filesystem is corrupted!");
                }
            }
        }
        Err(e) => {
            eprintln!("Error validating filesystem: {}", e);
            std::process::exit(1);
        }
    }
}
*/