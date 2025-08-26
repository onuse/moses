use clap::{Parser, Subcommand};
use moses_core::{DeviceManager, FormatterRegistry, FormatterCategory};
use moses_platform::PlatformDeviceManager;
use moses_filesystems::register_builtin_formatters;
#[cfg(any(feature = "mount-windows", feature = "mount-unix"))]
use moses_filesystems::mount::{get_mount_provider, MountOptions};
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
    /// Mount a filesystem (reads any filesystem on any platform!)
    Mount {
        /// Source device (e.g., E:, /dev/sdb1)
        source: String,
        /// Mount point (e.g., M:, /mnt/ext4)
        target: String,
        /// Force specific filesystem type (auto-detect if not specified)
        #[arg(short = 't', long)]
        fs_type: Option<String>,
        /// Mount as read-only
        #[arg(short = 'r', long)]
        readonly: bool,
    },
    /// Unmount a filesystem
    Unmount {
        /// Mount point to unmount
        target: String,
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
                dry_run: false,
                force: false,
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
            if let Some(info) = moses_filesystems::get_formatter_info(&registry, &name) {
                println!("{}", info);
            } else {
                eprintln!("Formatter '{}' not found.", name);
                eprintln!("Use 'moses list-formats' to see available formatters.");
            }
        }
        Commands::Mount { source, target, fs_type, readonly } => {
            println!("🔧 Moses Mount - Universal Filesystem Access");
            println!("================================================");
            
            use moses_filesystems::{MountSource, HostFolderOps, SubfolderOps, FilesystemOpsRegistry, register_all_filesystems};
            use std::path::PathBuf;
            
            // Intelligently determine what we're mounting
            let mount_source = if source.contains(':') && !source.starts_with('/') {
                // Windows drive letter (E:) or device with path (E:\Users)
                if source.len() == 2 && source.ends_with(':') {
                    // Just a drive letter like "E:"
                    let manager = PlatformDeviceManager;
                    let devices = manager.enumerate_devices().await?;
                    let device = devices.iter()
                        .find(|d| d.id == source || d.name.contains(&source))
                        .ok_or_else(|| anyhow::anyhow!("Device not found: {}", source))?;
                    MountSource::Device(device.clone())
                } else {
                    // Path like "E:\Users" - treat as host folder on Windows
                    let path = PathBuf::from(&source);
                    if path.exists() {
                        MountSource::HostPath(path)
                    } else {
                        return Err(anyhow::anyhow!("Path does not exist: {}", source));
                    }
                }
            } else if source.starts_with('/') {
                // Unix-style path
                let path = PathBuf::from(&source);
                if path.exists() && path.is_dir() {
                    // It's a local directory
                    MountSource::HostPath(path)
                } else if source.contains(':') {
                    // Format: /dev/sdb1:/home/user
                    let parts: Vec<&str> = source.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        let manager = PlatformDeviceManager;
                        let devices = manager.enumerate_devices().await?;
                        let device = devices.iter()
                            .find(|d| d.id == parts[0])
                            .ok_or_else(|| anyhow::anyhow!("Device not found: {}", parts[0]))?;
                        MountSource::DevicePath {
                            device: device.clone(),
                            base_path: PathBuf::from(parts[1]),
                        }
                    } else {
                        // Try as device
                        let manager = PlatformDeviceManager;
                        let devices = manager.enumerate_devices().await?;
                        let device = devices.iter()
                            .find(|d| d.id == source)
                            .ok_or_else(|| anyhow::anyhow!("Device not found: {}", source))?;
                        MountSource::Device(device.clone())
                    }
                } else {
                    // Assume it's a device path
                    let manager = PlatformDeviceManager;
                    let devices = manager.enumerate_devices().await?;
                    let device = devices.iter()
                        .find(|d| d.id == source || d.name.contains(&source))
                        .ok_or_else(|| anyhow::anyhow!("Device not found: {}", source))?;
                    MountSource::Device(device.clone())
                }
            } else {
                // Try to find as a device name
                let manager = PlatformDeviceManager;
                let devices = manager.enumerate_devices().await?;
                let device = devices.iter()
                    .find(|d| d.name.contains(&source))
                    .ok_or_else(|| anyhow::anyhow!("Source not found: {}", source))?;
                MountSource::Device(device.clone())
            };
            
            // Display what we're mounting
            match &mount_source {
                MountSource::Device(device) => {
                    println!("Source: {} (device)", device.name);
                }
                MountSource::DevicePath { device, base_path } => {
                    println!("Source: {}:{} (device subfolder)", device.name, base_path.display());
                }
                MountSource::HostPath(path) => {
                    println!("Source: {} (host folder)", path.display());
                }
            }
            println!("Target: {}", target);
            
            // Create filesystem operations based on mount source
            let ops_result = match mount_source {
                MountSource::Device(ref device) => {
                    // Standard device mounting
                    let mut ops_registry = FilesystemOpsRegistry::new();
                    register_all_filesystems(&mut ops_registry, !readonly);
                    ops_registry.create_ops(device, fs_type.as_deref())
                }
                MountSource::DevicePath { ref device, ref base_path } => {
                    // Mount subfolder from device
                    let mut ops_registry = FilesystemOpsRegistry::new();
                    register_all_filesystems(&mut ops_registry, !readonly);
                    match ops_registry.create_ops(device, fs_type.as_deref()) {
                        Ok(inner_ops) => {
                            SubfolderOps::new(inner_ops, device, base_path.clone())
                                .map(|ops| Box::new(ops) as Box<dyn moses_filesystems::FilesystemOps>)
                        }
                        Err(e) => Err(e)
                    }
                }
                MountSource::HostPath(ref path) => {
                    // Mount host folder
                    HostFolderOps::new(path.clone())
                        .map(|ops| Box::new(ops) as Box<dyn moses_filesystems::FilesystemOps>)
                }
            };
            
            match ops_result {
                Ok(ops) => {
                    let fs_type = ops.filesystem_type();
                    println!("Detected filesystem: {}", fs_type);
                    
                    // Try to actually mount if the feature is available
                    #[cfg(any(feature = "mount-windows", feature = "mount-unix"))]
                    {
                        println!("\nAttempting to mount filesystem...");
                        
                        match get_mount_provider() {
                            Ok(mut provider) => {
                                let mount_opts = MountOptions {
                                    readonly,
                                    mount_point: target.clone(),
                                    filesystem_type: fs_type.clone(),
                                    ..Default::default()
                                };
                                
                                // Get the device for mounting (create a dummy one for host paths)
                                let mount_device = match &mount_source {
                                    MountSource::Device(device) => device.clone(),
                                    MountSource::DevicePath { device, .. } => device.clone(),
                                    MountSource::HostPath(path) => {
                                        // Create a virtual device for host path mounting
                                        moses_core::Device {
                                            name: path.file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or("folder")
                                                .to_string(),
                                            id: path.to_string_lossy().to_string(),
                                            size: 0, // Would need platform-specific code
                                            device_type: moses_core::DeviceType::Fixed,
                                            is_removable: false,
                                            is_system: false,
                                            mount_points: vec![],
                                            partitions: vec![],
                                        }
                                    }
                                };
                                
                                match provider.mount(&mount_device, ops, &mount_opts) {
                                    Ok(()) => {
                                        println!("\n✅ Successfully mounted {} at {}", source, target);
                                        println!("\nYou can now:");
                                        println!("  - Browse {} files in Windows Explorer", fs_type);
                                        println!("  - Use any Windows application to read the files");
                                        println!("  - Access the filesystem as if it were native!");
                                        println!("\nTo unmount, run: moses unmount {}", target);
                                    }
                                    Err(e) => {
                                        eprintln!("\n❌ Failed to mount: {}", e);
                                        eprintln!("\nMake sure:");
                                        eprintln!("  1. WinFsp is installed (http://www.secfs.net/winfsp/)");
                                        eprintln!("  2. You're running as administrator");
                                        eprintln!("  3. The mount point {} is available", target);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("\n❌ Mount provider not available: {}", e);
                                eprintln!("\nInstall WinFsp from: http://www.secfs.net/winfsp/");
                            }
                        }
                    }
                    
                    #[cfg(not(any(feature = "mount-windows", feature = "mount-unix")))]
                    {
                        let _ = readonly;  // Unused in preview mode
                        // Get filesystem info for preview
                        if let Ok(info) = ops.statfs() {
                            println!("\nFilesystem Information:");
                            println!("  Total space: {:.2} GB", info.total_space as f64 / 1_073_741_824.0);
                            println!("  Block size: {} bytes", info.block_size);
                            if let Some(label) = info.volume_label {
                                println!("  Volume label: {}", label);
                            }
                        }
                        
                        println!("\n⚠️  Mounting functionality requires WinFsp (Windows) or FUSE (Linux/macOS)");
                        println!("This is a preview of the mounting capability.");
                        println!("\nTo mount {} filesystems on Windows:", fs_type);
                        println!("  1. Install WinFsp from http://www.secfs.net/winfsp/");
                        println!("  2. Run: moses mount {} {}", source, target);
                        println!("\nOnce mounted, you'll be able to:");
                        println!("  - Browse {} files in Windows Explorer", fs_type);
                        println!("  - Use any Windows application to read the files");
                        println!("  - Access the filesystem as if it were native NTFS!");
                    }
                }
                Err(e) => {
                    eprintln!("Error: Could not read filesystem on {}: {}", source, e);
                    eprintln!("\nSupported filesystems for reading:");
                    eprintln!("  - ext4, ext3, ext2");
                    eprintln!("  - Host folders (any local directory)");
                    eprintln!("\nExamples:");
                    eprintln!("  moses mount E: M:                    # Mount entire ext4 drive");
                    eprintln!("  moses mount /dev/sdb1:/home M:       # Mount subfolder from device");  
                    eprintln!("  moses mount C:\\Projects P:           # Mount local folder as drive");
                    eprintln!("  moses mount ~/Documents D:           # Mount home folder as drive");
                }
            }
        }
        Commands::Unmount { target } => {
            println!("Unmounting {}", target);
            println!("⚠️  Unmount functionality requires WinFsp/FUSE integration");
            println!("This feature is coming soon!");
        }
    }
    
    Ok(())
}