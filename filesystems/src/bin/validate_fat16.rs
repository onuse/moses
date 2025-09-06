// Command-line tool to validate FAT16 filesystems
// NOTE: Fat16Validator has been moved/removed during refactoring
// This tool needs to be updated to work with the new architecture

fn main() {
    eprintln!("FAT16 validator is currently unavailable.");
    eprintln!("The Fat16Validator type was removed during filesystem refactoring.");
    eprintln!("This tool needs to be rewritten to work with the new architecture.");
    std::process::exit(1);
}

/* Original code preserved for reference:
// Can validate at MBR offset or partition offset

use moses_filesystems::families::fat::fat16::Fat16Validator;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 || args.len() > 3 {
        eprintln!("Usage: {} <device_path> [partition_offset_sectors]", args[0]);
        eprintln!("\nExamples:");
        eprintln!("  {} /dev/sdb          - Validate FAT16 at sector 0 (superfloppy)", args[0]);
        eprintln!("  {} /dev/sdb 2048     - Validate FAT16 at sector 2048 (MBR partition)", args[0]);
        eprintln!("  {} \\\\.\\E:           - Validate Windows drive E:", args[0]);
        eprintln!("  {} \\\\.\\PHYSICALDRIVE2 2048 - Validate physical drive with MBR", args[0]);
        std::process::exit(1);
    }
    
    let device_path = &args[1];
    let partition_offset = if args.len() == 3 {
        match args[2].parse::<u64>() {
            Ok(offset) => Some(offset),
            Err(_) => {
                eprintln!("Error: Invalid partition offset: {}", args[2]);
                std::process::exit(1);
            }
        }
    } else {
        None
    };
    
    println!("Validating FAT16 filesystem...");
    println!("Device: {}", device_path);
    if let Some(offset) = partition_offset {
        println!("Partition offset: sector {} (byte {})", offset, offset * 512);
    } else {
        println!("Partition offset: none (reading from sector 0)");
    }
    println!();
    
    match Fat16Validator::validate(device_path, partition_offset) {
        Ok(report) => {
            // Print validation results
            println!("=== FAT16 VALIDATION REPORT ===\n");
            
            if report.is_valid {
                println!("✓ VALID FAT16 FILESYSTEM");
            } else {
                println!("✗ INVALID FAT16 FILESYSTEM");
            }
            
            // Print errors
            if !report.errors.is_empty() {
                println!("\nERRORS:");
                for error in &report.errors {
                    println!("  ✗ {}", error);
                }
            }
            
            // Print warnings
            if !report.warnings.is_empty() {
                println!("\nWARNINGS:");
                for warning in &report.warnings {
                    println!("  ⚠ {}", warning);
                }
            }
            
            // Print filesystem info
            println!("\nFILESYSTEM INFO:");
            for (key, value) in &report.info {
                println!("  {}: {}", key, value);
            }
            
            // Print cluster info
            println!("\nCLUSTER INFO:");
            println!("  Total clusters: {}", report.cluster_info.total_clusters);
            println!("  Free clusters: {}", report.cluster_info.free_clusters);
            println!("  Cluster size: {} bytes", report.cluster_info.cluster_size_bytes);
            
            // Print Windows compatibility
            println!("\nWINDOWS COMPATIBILITY:");
            println!("  Drive number: {}", if report.windows_compatibility.drive_number_correct { "✓" } else { "✗" });
            println!("  Media descriptor: {}", if report.windows_compatibility.media_descriptor_correct { "✓" } else { "✗" });
            println!("  Volume ID: {}", if report.windows_compatibility.volume_id_present { "✓" } else { "✗" });
            println!("  OEM name: {}", if report.windows_compatibility.oem_name_valid { "✓" } else { "✗" });
            
            if !report.is_valid {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Error validating filesystem: {}", e);
            std::process::exit(1);
        }
    }
}
*/