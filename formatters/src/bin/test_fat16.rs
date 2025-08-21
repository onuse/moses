// Test program for FAT16 compliant formatter
use moses_formatters::fat16::spec_compliance_test::check_fat16_compliance;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <device_path>", args[0]);
        std::process::exit(1);
    }
    
    let device_path = &args[1];
    
    match check_fat16_compliance(device_path) {
        Ok(result) => {
            println!("FAT16 Compliance Check Results:");
            println!("================================");
            println!("Compliant: {}", result.is_compliant);
            
            if !result.info.is_empty() {
                println!("\nInformation:");
                for info in &result.info {
                    println!("  - {}", info);
                }
            }
            
            if !result.warnings.is_empty() {
                println!("\nWarnings:");
                for warning in &result.warnings {
                    println!("  ⚠ {}", warning);
                }
            }
            
            if !result.errors.is_empty() {
                println!("\nErrors:");
                for error in &result.errors {
                    println!("  ✗ {}", error);
                }
            }
            
            if result.is_compliant {
                println!("\n✓ The filesystem is FAT16 compliant!");
            } else {
                println!("\n✗ The filesystem is NOT FAT16 compliant!");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Error checking FAT16 compliance: {}", e);
            std::process::exit(1);
        }
    }
}