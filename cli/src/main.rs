use clap::{Parser, Subcommand};
use moses_core::{DeviceManager, FormatterRegistry, FormatterCategory};
use moses_platform::PlatformDeviceManager;
use moses_formatters::register_builtin_formatters;
use std::sync::Arc;

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
        /// Filesystem type (ext4, ntfs, fat32, exfat, etc.)
        #[arg(short, long)]
        filesystem: String,
    },
    /// List available formatters
    ListFormats {
        /// Filter by category (modern, legacy, historical, console, embedded, experimental)
        #[arg(short, long)]
        category: Option<String>,
    },
    /// Show detailed information about a formatter
    FormatInfo {
        /// Formatter name or alias
        name: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    
    // Initialize formatter registry
    let mut registry = FormatterRegistry::new();
    register_builtin_formatters(&mut registry)?;
    let registry = Arc::new(registry);
    
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
            // Check if formatter is available
            let formatter = registry.get_formatter(&filesystem)
                .ok_or_else(|| anyhow::anyhow!("Unknown filesystem type: '{}'. Use 'moses list-formats' to see available formats.", filesystem))?;
            
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
            
            // Check if formatter can handle this device
            if !formatter.can_format(target_device) {
                eprintln!("Error: {} formatter cannot format this device", filesystem);
                if let Some(meta) = registry.get_metadata(&filesystem) {
                    if let Some(min) = meta.min_size {
                        if target_device.size < min {
                            eprintln!("  Device too small. Minimum size: {} bytes", min);
                        }
                    }
                    if let Some(max) = meta.max_size {
                        if target_device.size > max {
                            eprintln!("  Device too large. Maximum size: {} bytes", max);
                        }
                    }
                }
                return Ok(());
            }
            
            println!("Target device: {}", target_device.name);
            println!("  Size: {:.2} GB", target_device.size as f64 / 1_073_741_824.0);
            println!("  Type: {:?}", target_device.device_type);
            
            // Show formatter info
            if let Some(meta) = registry.get_metadata(&filesystem) {
                println!("\nFormatter: {} ({})", meta.name, meta.description);
                println!("  Category: {:?}", meta.category);
                println!("  Version: {}", meta.version);
            }
            println!();
            
            // Create format options
            let options = moses_core::FormatOptions {
                filesystem_type: filesystem.clone(),
                label: Some("MOSES_TEST".to_string()),
                quick_format: true,
                cluster_size: None,
                enable_compression: false,
                verify_after_format: false,
                additional_options: std::collections::HashMap::new(),
            };
            
            // Run dry run first
            println!("Running simulation...");
            let simulation = formatter.dry_run(target_device, &options).await?;
            
            println!("\nSimulation Report:");
            println!("  Estimated time: {:?}", simulation.estimated_time);
            if !simulation.required_tools.is_empty() {
                println!("  Required tools: {:?}", simulation.required_tools);
            }
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
            
            println!("\nFormatting {} as {}...", target_device.name, filesystem.to_uppercase());
            match formatter.format(target_device, &options).await {
                Ok(_) => println!("Format completed successfully!"),
                Err(e) => eprintln!("Format failed: {}", e),
            }
        }
        Commands::ListFormats { category } => {
            println!("Available Formatters:\n");
            
            if let Some(cat_str) = category {
                // Parse category
                let cat = match cat_str.to_lowercase().as_str() {
                    "modern" => FormatterCategory::Modern,
                    "legacy" => FormatterCategory::Legacy,
                    "historical" => FormatterCategory::Historical,
                    "console" => FormatterCategory::Console,
                    "embedded" => FormatterCategory::Embedded,
                    "experimental" => FormatterCategory::Experimental,
                    _ => {
                        eprintln!("Unknown category: {}", cat_str);
                        return Ok(());
                    }
                };
                
                let formatters = registry.list_by_category(cat.clone());
                if formatters.is_empty() {
                    println!("No formatters found in category: {:?}", cat);
                } else {
                    for (name, meta) in formatters {
                        println!("  {} - {}", name, meta.description);
                        if !meta.aliases.is_empty() {
                            println!("    Aliases: {:?}", meta.aliases);
                        }
                    }
                }
            } else {
                // List all formatters by category
                let categories = [
                    FormatterCategory::Modern,
                    FormatterCategory::Legacy,
                    FormatterCategory::Historical,
                    FormatterCategory::Console,
                    FormatterCategory::Embedded,
                    FormatterCategory::Experimental,
                ];
                
                for cat in categories {
                    let formatters = registry.list_by_category(cat.clone());
                    if !formatters.is_empty() {
                        println!("{:?}:", cat);
                        for (name, meta) in formatters {
                            println!("  {} - {}", name, meta.description);
                            if !meta.aliases.is_empty() {
                                println!("    Aliases: {:?}", meta.aliases);
                            }
                        }
                        println!();
                    }
                }
            }
            
            println!("\nUse 'moses format-info <name>' for detailed information about a formatter.");
        }
        Commands::FormatInfo { name } => {
            if let Some(info) = moses_formatters::get_formatter_info(&registry, &name) {
                println!("{}", info);
            } else {
                eprintln!("Formatter '{}' not found.", name);
                eprintln!("Use 'moses list-formats' to see available formatters.");
            }
        }
    }
    
    Ok(())
}