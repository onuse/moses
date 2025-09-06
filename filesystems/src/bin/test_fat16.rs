// Test program for FAT16 compliant formatter
// NOTE: Fat16Validator has been moved/removed during refactoring
// This test needs to be updated to work with the new architecture

fn main() {
    eprintln!("FAT16 validator test is currently unavailable.");
    eprintln!("The Fat16Validator type was removed during filesystem refactoring.");
    eprintln!("This test needs to be rewritten to work with the new architecture.");
    std::process::exit(1);
}

/* Original code preserved for reference:
use moses_filesystems::families::fat::fat16::Fat16Validator;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <device_path>", args[0]);
        std::process::exit(1);
    }
    
    let device_path = &args[1];
    
    match Fat16Validator::validate(device_path, None) {
        Ok(result) => {
            println!("FAT16 Compliance Check Results:");
            println!("================================");
            println!("Compliant: {}", result.is_valid);
            
            if !result.info.is_empty() {
                println!("\nInformation:");
                for (key, value) in &result.info {
                    println!("  - {}: {}", key, value);
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
            
            println!("\nCluster Info:");
            println!("  Total clusters: {}", result.cluster_info.total_clusters);
            println!("  Free clusters: {}", result.cluster_info.free_clusters);
            println!("  Cluster size: {} bytes", result.cluster_info.cluster_size_bytes);
            
            println!("\nWindows Compatibility:");
            println!("  Drive number: {}", if result.windows_compatibility.drive_number_correct { "✓" } else { "✗" });
            println!("  Media descriptor: {}", if result.windows_compatibility.media_descriptor_correct { "✓" } else { "✗" });
            println!("  Volume ID: {}", if result.windows_compatibility.volume_id_present { "✓" } else { "✗" });
            println!("  OEM name: {}", if result.windows_compatibility.oem_name_valid { "✓" } else { "✗" });
            
            if result.is_valid {
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
*/