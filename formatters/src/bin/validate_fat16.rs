// Command-line tool to validate FAT16 filesystems
// Can validate at MBR offset or partition offset

use moses_formatters::fat16::comprehensive_validator::Fat16ComprehensiveValidator;
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
    
    match Fat16ComprehensiveValidator::validate(device_path, partition_offset) {
        Ok(report) => {
            let formatted = Fat16ComprehensiveValidator::format_report(&report);
            println!("{}", formatted);
            
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