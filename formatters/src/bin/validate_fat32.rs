// FAT32 validation tool
// Usage: validate_fat32 <device_path>

use moses_formatters::fat32::validator::Fat32ComprehensiveValidator;
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
                moses_formatters::fat_common::validator::ValidationStatus::Perfect => {
                    println!("✓ PERFECT: FAT32 filesystem is 100% compliant!");
                }
                moses_formatters::fat_common::validator::ValidationStatus::Compliant => {
                    println!("✓ COMPLIANT: FAT32 filesystem meets specifications with minor warnings.");
                }
                moses_formatters::fat_common::validator::ValidationStatus::PartiallyCompliant => {
                    println!("⚠ PARTIALLY COMPLIANT: FAT32 filesystem has some issues but may work.");
                }
                moses_formatters::fat_common::validator::ValidationStatus::NonCompliant => {
                    println!("✗ NON-COMPLIANT: FAT32 filesystem has major violations!");
                }
                moses_formatters::fat_common::validator::ValidationStatus::Corrupted => {
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