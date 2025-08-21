// Test program for FAT32 formatter
// This directly tests the FAT32 formatter at a low level
use std::env;
// OpenOptions removed - not used
// IO traits removed - not used

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <device_path> [volume_label]", args[0]);
        eprintln!("Example: {} \\\\.\\PHYSICALDRIVE2", args[0]);
        eprintln!("Example: {} \\\\.\\PHYSICALDRIVE2 \"MYDISK\"", args[0]);
        std::process::exit(1);
    }
    
    let device_path = &args[1];
    let volume_label = if args.len() > 2 {
        Some(args[2].as_str())
    } else {
        None
    };
    
    println!("FAT32 Native Formatter Test");
    println!("============================");
    println!("Device: {}", device_path);
    if let Some(label) = volume_label {
        println!("Volume Label: {}", label);
    }
    println!();
    
    // Call the low-level format function directly
    println!("Starting FAT32 format...");
    match format_fat32(device_path, volume_label) {
        Ok(_) => {
            println!("✓ FAT32 format completed successfully!");
            println!();
            println!("The drive should now be recognized by Windows.");
            println!("You may need to unplug and replug the device.");
        }
        Err(e) => {
            eprintln!("✗ Error formatting device: {}", e);
            std::process::exit(1);
        }
    }
}

// Low-level FAT32 format function
fn format_fat32(device_path: &str, volume_label: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    use moses_formatters::fat32::formatter_native::Fat32NativeFormatter;
    use moses_core::{Device, DeviceType, FormatOptions, FilesystemFormatter};
    
    // Create a minimal device struct
    let device = Device {
        id: device_path.to_string(),
        name: "Test Device".to_string(),
        size: 0, // Will be determined by formatter
        device_type: DeviceType::USB,
        mount_points: vec![],
        is_removable: true,
        is_system: false,
        filesystem: None,
    };
    
    // Create format options
    let options = FormatOptions {
        filesystem_type: "fat32".to_string(),
        label: volume_label.map(String::from),
        cluster_size: None,
        quick_format: true,
        enable_compression: false,
        verify_after_format: false,
        additional_options: std::collections::HashMap::new(),
    };
    
    // Use tokio runtime to call async function
    let runtime = tokio::runtime::Runtime::new()?;
    let formatter = Fat32NativeFormatter;
    
    runtime.block_on(async {
        formatter.format(&device, &options).await
    })?;
    
    Ok(())
}