use clap::{Parser, Subcommand};
use moses_core::DeviceManager;
use moses_platform::PlatformDeviceManager;

#[derive(Parser)]
#[command(name = "moses")]
#[command(about = "Cross-platform drive formatting tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available drives
    List,
    /// Format a drive
    Format {
        /// Device identifier
        device: String,
        /// Filesystem type (ext4, ntfs, fat32, exfat)
        #[arg(short, long)]
        filesystem: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::List => {
            let manager = PlatformDeviceManager;
            match manager.enumerate_devices().await {
                Ok(devices) => {
                    if devices.is_empty() {
                        println!("No devices found.");
                    } else {
                        println!("Available devices:\n");
                        for device in devices {
                            println!("Device: {}", device.name);
                            println!("  Path: {}", device.id);
                            println!("  Size: {:.2} GB", device.size as f64 / 1_073_741_824.0);
                            println!("  Type: {:?}", device.device_type);
                            println!("  Removable: {}", if device.is_removable { "Yes" } else { "No" });
                            println!("  System: {}", if device.is_system { "Yes (⚠️ PROTECTED)" } else { "No" });
                            if !device.mount_points.is_empty() {
                                println!("  Mounted at: {:?}", device.mount_points);
                            }
                            println!();
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error enumerating devices: {}", e);
                }
            }
        }
        Commands::Format { device, filesystem } => {
            // Get the device manager
            let manager = PlatformDeviceManager;
            
            // Find the specified device
            let devices = manager.enumerate_devices().await?;
            let target_device = devices.iter()
                .find(|d| d.id == device || d.name.contains(&device))
                .ok_or_else(|| anyhow::anyhow!("Device not found: {}", device))?;
            
            // Safety check
            if target_device.is_system {
                eprintln!("Error: Cannot format system drive!");
                return Ok(());
            }
            
            println!("Target device: {}", target_device.name);
            println!("  Size: {:.2} GB", target_device.size as f64 / 1_073_741_824.0);
            println!("  Type: {:?}", target_device.device_type);
            println!();
            
            // Create format options
            let options = moses_core::FormatOptions {
                filesystem_type: filesystem.clone(),
                label: Some("MOSES_TEST".to_string()),
                quick_format: true,
                cluster_size: None,
                enable_compression: false,
                additional_options: std::collections::HashMap::new(),
            };
            
            // Use the appropriate formatter based on filesystem and platform
            match filesystem.as_str() {
                "ext4" => {
                    #[cfg(target_os = "windows")]
                    {
                        use moses_formatters::Ext4WindowsFormatter;
                        use moses_core::FilesystemFormatter;
                        
                        let formatter = Ext4WindowsFormatter;
                        
                        // First do a dry run
                        println!("Running simulation...");
                        let simulation = formatter.dry_run(target_device, &options).await?;
                        
                        println!("\nSimulation Report:");
                        println!("  Estimated time: {:?}", simulation.estimated_time);
                        println!("  Required tools: {:?}", simulation.required_tools);
                        if !simulation.warnings.is_empty() {
                            println!("  Warnings:");
                            for warning in &simulation.warnings {
                                println!("    - {}", warning);
                            }
                        }
                        
                        println!("\nWARNING: This will ERASE ALL DATA on {}!", target_device.name);
                        println!("Type 'yes' to continue: ");
                        
                        use std::io::{self, BufRead};
                        let stdin = io::stdin();
                        let mut line = String::new();
                        stdin.lock().read_line(&mut line)?;
                        
                        if line.trim() != "yes" {
                            println!("Format cancelled.");
                            return Ok(());
                        }
                        
                        println!("\nFormatting {} as EXT4...", target_device.name);
                        match formatter.format(target_device, &options).await {
                            Ok(_) => println!("Format completed successfully!"),
                            Err(e) => eprintln!("Format failed: {}", e),
                        }
                    }
                    
                    #[cfg(not(target_os = "windows"))]
                    {
                        println!("EXT4 formatting not yet implemented for this platform");
                    }
                }
                _ => {
                    println!("Filesystem '{}' not yet implemented", filesystem);
                }
            }
        }
    }
    
    Ok(())
}