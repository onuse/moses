// Elevated worker process for privileged operations
// This process gets elevated privileges via UAC to perform format operations

use std::env;
use std::fs;
use std::path::Path;
use std::io::Write;
use moses_core::{Device, FormatOptions, FilesystemFormatter};
use moses_formatters::{Fat16Formatter, Fat32Formatter, ExFatFormatter};
use moses_formatters::diagnostics::analyze_unknown_filesystem;
use serde_json;
use moses_formatters::disk_manager::{
    DiskManager, DiskCleaner, CleanOptions,
    PartitionStyleConverter, PartitionStyle,
};
#[cfg(target_os = "windows")]
use moses_formatters::{Ext2Formatter, Ext3Formatter};
use serde::{Deserialize, Serialize};
use log::{Record, Level, Metadata, LevelFilter};
use std::net::TcpStream;
use std::io::{BufReader, BufRead};
use std::sync::Mutex;


#[cfg(target_os = "windows")]
use moses_formatters::Ext4NativeFormatter;

#[cfg(target_os = "linux")]
use moses_formatters::Ext4LinuxFormatter;

// Global log file path for this worker instance
use std::sync::OnceLock;
static LOG_FILE_PATH: OnceLock<std::path::PathBuf> = OnceLock::new();

// Global socket stream for log streaming
static SOCKET_STREAM: OnceLock<Mutex<Option<TcpStream>>> = OnceLock::new();

// Simple file logging function
fn log_to_file(msg: &str) {
    // Try to send over socket first
    if let Some(stream_mutex) = SOCKET_STREAM.get() {
        if let Ok(mut guard) = stream_mutex.lock() {
            if let Some(ref mut stream) = *guard {
                // Don't send log messages about sending logs to avoid recursion
                if !msg.contains("Log message") && !msg.contains("Failed to send log") {
                    let log_response = WorkerResponse::Log {
                        level: "INFO".to_string(),
                        message: msg.to_string(),
                    };
                    if let Ok(json) = serde_json::to_string(&log_response) {
                        let _ = stream.write_all(json.as_bytes());
                        let _ = stream.write_all(b"\n");
                        let _ = stream.flush();
                    }
                }
            }
        }
    }
    
    // Also log to file
    if let Some(path) = LOG_FILE_PATH.get() {
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path) 
        {
            let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
            let _ = writeln!(file, "[{}] {}", timestamp, msg);
        }
    }
    // Also print to stderr (might not be visible with UAC)
    eprintln!("{}", msg);
}

// Custom logger that writes to our file
struct FileLogger;

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let level_str = record.level().to_string();
            let msg = format!("{}: {}", 
                record.target(), 
                record.args());
            
            // Send over socket if available
            if let Some(stream_mutex) = SOCKET_STREAM.get() {
                if let Ok(mut guard) = stream_mutex.lock() {
                    if let Some(ref mut stream) = *guard {
                        if !msg.contains("Log message") && !msg.contains("Failed to send log") {
                            let log_response = WorkerResponse::Log {
                                level: level_str.clone(),
                                message: msg.clone(),
                            };
                            if let Ok(json) = serde_json::to_string(&log_response) {
                                let _ = stream.write_all(json.as_bytes());
                                let _ = stream.write_all(b"\n");
                                let _ = stream.flush();
                            }
                        }
                    }
                }
            }
            
            // Also log to file
            let full_msg = format!("[{}] {}", level_str, msg);
            if let Some(path) = LOG_FILE_PATH.get() {
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path) 
                {
                    let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
                    let _ = writeln!(file, "[{}] {}", timestamp, full_msg);
                }
            }
            eprintln!("{}", full_msg);
        }
    }

    fn flush(&self) {}
}

static LOGGER: FileLogger = FileLogger;

// Helper function to show error messages on Windows
#[cfg(target_os = "windows")]
fn show_error_message(title: &str, message: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_OK, MB_ICONERROR};
    use windows::core::PCWSTR;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    
    let message_wide: Vec<u16> = OsStr::new(message)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let title_wide: Vec<u16> = OsStr::new(title)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    
    unsafe {
        MessageBoxW(
            None,
            PCWSTR::from_raw(message_wide.as_ptr()),
            PCWSTR::from_raw(title_wide.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

// Stub for non-Windows platforms
#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
fn show_error_message(_title: &str, _message: &str) {
    // No-op on non-Windows platforms
}

fn main() {
    // Increase stack size to prevent stack overflow with large buffers
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024) // 8MB stack
        .spawn(run_worker)
        .unwrap()
        .join()
        .unwrap();
}

fn run_worker() {
    // Set up file logging for the worker since UAC hides console output
    let log_file_path = env::temp_dir().join(format!("moses-worker-{}.log", std::process::id()));
    
    // Store the log file path globally
    let _ = LOG_FILE_PATH.set(log_file_path.clone());
    
    // Initialize the logger to capture log crate output
    #[cfg(debug_assertions)]
    let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(LevelFilter::Trace));
    
    #[cfg(not(debug_assertions))]
    let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(LevelFilter::Info));
    
    // In production, only log essential info
    #[cfg(debug_assertions)]
    {
        log_to_file("========================================");
        log_to_file(&format!("Moses Worker Started - PID: {}", std::process::id()));
        log_to_file(&format!("Log file: {}", log_file_path.display()));
        log_to_file(&format!("Working directory: {}", env::current_dir().unwrap_or_default().display()));
    }
    
    // Show log file location in a message box for debugging
    #[cfg(target_os = "windows")]
    #[cfg(debug_assertions)]
    {
        let msg = format!("Worker log file:\n{}", log_file_path.display());
        show_error_message("Moses Worker Log Location", &msg);
    }
    
    // Set up panic handler to log crashes
    std::panic::set_hook(Box::new(|panic_info| {
        let msg = format!("Worker panic: {}", panic_info);
        log_to_file(&msg);
        
        // Show error in message box on Windows for debugging
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_OK, MB_ICONERROR};
            use windows::core::PCWSTR;
            use std::ffi::OsStr;
            use std::os::windows::ffi::OsStrExt;
            
            let title = "Moses Formatter Crash";
            let message_wide: Vec<u16> = OsStr::new(&msg)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let title_wide: Vec<u16> = OsStr::new(title)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            
            unsafe {
                MessageBoxW(
                    None,
                    PCWSTR::from_raw(message_wide.as_ptr()),
                    PCWSTR::from_raw(title_wide.as_ptr()),
                    MB_OK | MB_ICONERROR,
                );
            }
        }
        
        // Try to write to a crash log file too
        if let Ok(exe_path) = env::current_exe() {
            if let Some(parent) = exe_path.parent() {
                let crash_log_path = parent.join("moses-worker-crash.log");
                let _ = fs::write(&crash_log_path, &msg);
            }
        }
    }));
    
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    
    // Debug: Log all arguments
    log_to_file(&format!("Worker started with {} arguments", args.len()));
    for (i, arg) in args.iter().enumerate() {
        log_to_file(&format!("Arg[{}]: {}", i, arg));
    }
    
    if args.len() < 2 {
        let error_msg = format!(
            "Error: Insufficient arguments\nUsage: moses-formatter <command> [args...]\nCommands: format, analyze\nReceived {} arguments:\n{}",
            args.len(),
            args.join("\n")
        );
        log_to_file(&error_msg);
        
        #[cfg(target_os = "windows")]
        show_error_message("Invalid Arguments", &error_msg);
        
        std::process::exit(1);
    }
    
    // Check if running in socket mode
    if args.len() >= 3 && args[1] == "--socket" {
        let port = args[2].parse::<u16>().unwrap_or_else(|_| {
            let error_msg = format!("Invalid port number: {}", args[2]);
            log_to_file(&error_msg);
            #[cfg(target_os = "windows")]
            show_error_message("Invalid Port", &error_msg);
            std::process::exit(1);
        });
        
        log_to_file(&format!("Starting in socket mode on port {}", port));
        handle_socket_mode(port);
        return;
    }
    
    // Check command type (legacy mode for backward compatibility)
    let command = &args[1];
    log_to_file(&format!("Command: {}", command));
    
    match command.as_str() {
        "format" => {
            // Format command needs device and options files
            if args.len() < 4 {
                let error_msg = "Error: format command requires <device-json-file> <options-json-file>";
                log_to_file(error_msg);
                #[cfg(target_os = "windows")]
                show_error_message("Invalid Arguments", error_msg);
                std::process::exit(1);
            }
            
            let device_path = &args[2];
            let options_path = &args[3];
            handle_format(device_path, options_path);
        }
        "analyze" => {
            // Analyze command needs device file
            if args.len() < 3 {
                let error_msg = "Error: analyze command requires <device-json-file>";
                log_to_file(error_msg);
                #[cfg(target_os = "windows")]
                show_error_message("Invalid Arguments", error_msg);
                std::process::exit(1);
            }
            
            let device_path = &args[2];
            handle_analyze(device_path);
        }
        "clean" => {
            // Clean command needs device and options files
            if args.len() < 4 {
                let error_msg = "Error: clean command requires <device-json-file> <options-json-file>";
                log_to_file(error_msg);
                #[cfg(target_os = "windows")]
                show_error_message("Invalid Arguments", error_msg);
                std::process::exit(1);
            }
            
            let device_path = &args[2];
            let options_path = &args[3];
            handle_clean(device_path, options_path);
        }
        "convert" => {
            // Convert command needs device file and target style
            if args.len() < 4 {
                let error_msg = "Error: convert command requires <device-json-file> <target-style>";
                log_to_file(error_msg);
                #[cfg(target_os = "windows")]
                show_error_message("Invalid Arguments", error_msg);
                std::process::exit(1);
            }
            
            let device_path = &args[2];
            let target_style = &args[3];
            handle_convert(device_path, target_style);
        }
        "prepare" => {
            // Prepare command needs device file, target style, and clean flag
            if args.len() < 5 {
                let error_msg = "Error: prepare command requires <device-json-file> <target-style> <clean-flag>";
                log_to_file(error_msg);
                #[cfg(target_os = "windows")]
                show_error_message("Invalid Arguments", error_msg);
                std::process::exit(1);
            }
            
            let device_path = &args[2];
            let target_style = &args[3];
            let clean_first = &args[4] == "clean";
            handle_prepare(device_path, target_style, clean_first);
        }
        "read_directory" => {
            // Read directory command needs device file and path
            if args.len() < 4 {
                let error_msg = "Error: read_directory command requires <device-json-file> <path>";
                log_to_file(error_msg);
                #[cfg(target_os = "windows")]
                show_error_message("Invalid Arguments", error_msg);
                std::process::exit(1);
            }
            
            let device_path = &args[2];
            let directory_path = &args[3];
            handle_read_directory(device_path, directory_path);
        }
        _ => {
            let error_msg = format!("Unknown command: {}", command);
            log_to_file(&error_msg);
            #[cfg(target_os = "windows")]
            show_error_message("Invalid Command", &error_msg);
            std::process::exit(1);
        }
    }
}

fn handle_format(device_path: &str, options_path: &str) {
    // Original format handling code
    
    log_to_file(&format!("Device file path: {}", device_path));
    log_to_file(&format!("Options file path: {}", options_path));
    
    // Check if files exist
    if !Path::new(device_path).exists() {
        let error_msg = format!("Device file does not exist: {}", device_path);
        log_to_file(&error_msg);
        
        #[cfg(target_os = "windows")]
        show_error_message("File Not Found", &error_msg);
        
        std::process::exit(1);
    }
    
    if !Path::new(options_path).exists() {
        let error_msg = format!("Options file does not exist: {}", options_path);
        log_to_file(&error_msg);
        
        #[cfg(target_os = "windows")]
        show_error_message("File Not Found", &error_msg);
        
        std::process::exit(1);
    }
    
    // Read and parse device JSON
    let device_json = fs::read_to_string(device_path)
        .unwrap_or_else(|e| {
            let error_msg = format!("Failed to read device file: {}\nPath: {}", e, device_path);
            log_to_file(&error_msg);
            
            #[cfg(target_os = "windows")]
            show_error_message("Read Error", &error_msg);
            
            std::process::exit(1);
        });
    
    log_to_file(&format!("Device JSON length: {} bytes", device_json.len()));
    log_to_file(&format!("Device JSON content: {}", device_json));
    
    let device: Device = serde_json::from_str(&device_json)
        .unwrap_or_else(|e| {
            let error_msg = format!("Failed to parse device JSON: {}", e);
            log_to_file(&error_msg);
            log_to_file(&format!("Full JSON that failed: {}", device_json));
            
            #[cfg(target_os = "windows")]
            show_error_message("Parse Error", &error_msg);
            
            std::process::exit(1);
        });
    
    // Read and parse options JSON
    let options_json = fs::read_to_string(options_path)
        .unwrap_or_else(|e| {
            let error_msg = format!("Failed to read options file: {}\nPath: {}", e, options_path);
            log_to_file(&error_msg);
            
            #[cfg(target_os = "windows")]
            show_error_message("Read Error", &error_msg);
            
            std::process::exit(1);
        });
    
    log_to_file(&format!("Options JSON length: {} bytes", options_json.len()));
    log_to_file(&format!("Options JSON content: {}", options_json));
    
    let options: FormatOptions = serde_json::from_str(&options_json)
        .unwrap_or_else(|e| {
            let error_msg = format!("Failed to parse options JSON: {}", e);
            log_to_file(&error_msg);
            log_to_file(&format!("Full JSON that failed: {}", options_json));
            
            #[cfg(target_os = "windows")]
            show_error_message("Parse Error", &error_msg);
            
            std::process::exit(1);
        });
    
    // Log operation details
    log_to_file("========================================");
    log_to_file(&format!("Starting format operation for device: {}", device.name));
    log_to_file(&format!("Device ID: {}", device.id));
    log_to_file(&format!("Device size: {} bytes ({} GB)", device.size, device.size / (1024*1024*1024)));
    log_to_file(&format!("Filesystem type: {}", options.filesystem_type));
    log_to_file(&format!("Cluster size: {:?}", options.cluster_size));
    log_to_file(&format!("Quick format: {}", options.quick_format));
    log_to_file(&format!("Verify after format: {}", options.verify_after_format));
    log_to_file("========================================");
    
    // Use tokio runtime for async operations
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let error_msg = format!("Failed to create tokio runtime: {}", e);
            log_to_file(&error_msg);
            std::process::exit(1);
        }
    };
    
    let result = runtime.block_on(async {
        execute_format(device, options).await
    });
    
    match result {
        Ok(msg) => {
            log_to_file(&format!("Format completed successfully: {}", msg));
            println!("{}", msg); // Success message to stdout
            std::process::exit(0);
        }
        Err(e) => {
            log_to_file(&format!("Format failed: {}", e));
            
            // Also show the log file location in the error
            let log_path = LOG_FILE_PATH.get().map(|p| p.display().to_string()).unwrap_or_else(|| "unknown".to_string());
            let error_with_log = format!(
                "Format failed: {}\n\nCheck log file for details:\n{}", 
                e, log_path
            );
            
            #[cfg(target_os = "windows")]
            show_error_message("Format Failed", &error_with_log);
            
            #[cfg(not(target_os = "windows"))]
            eprintln!("{}", error_with_log);
            
            std::process::exit(1);
        }
    }
} // End of run_worker()

async fn execute_format(device: Device, options: FormatOptions) -> Result<String, String> {
    // Safety checks
    if device.is_system {
        return Err("Cannot format system drive".to_string());
    }
    
    // Check critical mount points
    for mount in &device.mount_points {
        let mount_str = mount.to_string_lossy().to_lowercase();
        if mount_str == "/" || 
           mount_str == "c:\\" || 
           mount_str.starts_with("/boot") ||
           mount_str.starts_with("/system") ||
           mount_str.starts_with("c:\\windows") {
            return Err(format!("Cannot format drive with critical mount point: {}", mount_str));
        }
    }
    
    log_to_file(&format!("Executing format with filesystem type: {}", options.filesystem_type));
    
    // Clean disk first if there's an existing filesystem and we're creating a partition table
    let create_partition = options.additional_options
        .get("create_partition_table")
        .map(|v| v == "true")
        .unwrap_or(false);
    
    if create_partition && device.filesystem.is_some() {
        log_to_file(&format!("Existing filesystem detected ({}), cleaning disk first", 
                            device.filesystem.as_ref().unwrap()));
        
        use moses_formatters::disk_manager::{DiskCleaner, CleanOptions, WipeMethod};
        let clean_options = CleanOptions {
            wipe_method: WipeMethod::Quick,
            zero_entire_disk: false,
        };
        
        match DiskCleaner::clean(&device, &clean_options) {
            Ok(_) => {
                log_to_file("Disk cleaned successfully");
                // Small delay to let Windows process the clean
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            Err(e) => {
                log_to_file(&format!("Warning: Pre-format clean failed: {:?}, continuing anyway", e));
                // Continue anyway - the format might still work
            }
        }
    }
    
    // Execute format based on filesystem type
    match options.filesystem_type.as_str() {
        "ext2" => {
            #[cfg(target_os = "windows")]
            {
                log_to_file("Using Ext2Formatter");
                let formatter = Ext2Formatter;
                
                log_to_file("Validating options...");
                formatter.validate_options(&options)
                    .await
                    .map_err(|e| format!("Invalid options: {}", e))?;
                
                log_to_file("Checking if device can be formatted...");
                if !formatter.can_format(&device) {
                    return Err("Device cannot be formatted".to_string());
                }
                
                log_to_file("Starting format...");
                match formatter.format(&device, &options).await {
                    Ok(_) => {
                        log_to_file("Format completed successfully");
                        Ok(format!("Successfully formatted {} as ext2", device.name))
                    }
                    Err(e) => {
                        let error_msg = format!("Format failed: {:?}", e);
                        log_to_file(&error_msg);
                        Err(error_msg)
                    }
                }
            }
            
            #[cfg(not(target_os = "windows"))]
            {
                Err("ext2 formatting not yet implemented on this platform".to_string())
            }
        },
        
        "ext3" => {
            #[cfg(target_os = "windows")]
            {
                log_to_file("Using Ext3Formatter");
                let formatter = Ext3Formatter;
                
                log_to_file("Validating options...");
                formatter.validate_options(&options)
                    .await
                    .map_err(|e| format!("Invalid options: {}", e))?;
                
                log_to_file("Checking if device can be formatted...");
                if !formatter.can_format(&device) {
                    return Err("Device cannot be formatted".to_string());
                }
                
                log_to_file("Starting format...");
                match formatter.format(&device, &options).await {
                    Ok(_) => {
                        log_to_file("Format completed successfully");
                        Ok(format!("Successfully formatted {} as ext3", device.name))
                    }
                    Err(e) => {
                        let error_msg = format!("Format failed: {:?}", e);
                        log_to_file(&error_msg);
                        Err(error_msg)
                    }
                }
            }
            
            #[cfg(not(target_os = "windows"))]
            {
                Err("ext3 formatting not yet implemented on this platform".to_string())
            }
        },
        
        "ext4" => {
            #[cfg(target_os = "windows")]
            {
                log_to_file("Using Ext4NativeFormatter");
                let formatter = Ext4NativeFormatter;
                
                log_to_file("Validating options...");
                formatter.validate_options(&options)
                    .await
                    .map_err(|e| format!("Invalid options: {}", e))?;
                
                log_to_file("Checking if device can be formatted...");
                if !formatter.can_format(&device) {
                    return Err("Device cannot be formatted".to_string());
                }
                
                log_to_file("Starting format...");
                match formatter.format(&device, &options).await {
                    Ok(_) => {
                        log_to_file("Format completed successfully");
                        Ok(format!("Successfully formatted {} as EXT4", device.name))
                    }
                    Err(e) => {
                        let error_msg = format!("Format failed: {:?}", e);
                        log_to_file(&error_msg);
                        Err(error_msg)
                    }
                }
            }
            
            #[cfg(target_os = "linux")]
            {
                let formatter = Ext4LinuxFormatter;
                formatter.validate_options(&options)
                    .await
                    .map_err(|e| format!("Invalid options: {}", e))?;
                
                if !formatter.can_format(&device) {
                    return Err("Device cannot be formatted".to_string());
                }
                
                formatter.format(&device, &options)
                    .await
                    .map_err(|e| format!("Format failed: {}", e))?;
                
                Ok(format!("Successfully formatted {} as EXT4", device.name))
            }
            
            #[cfg(target_os = "macos")]
            {
                Err("EXT4 formatting not yet implemented on macOS".to_string())
            }
        },
        
        "ntfs" => {
            log_to_file("NTFS formatting not yet implemented");
            return Err("NTFS formatting is not yet implemented. Only NTFS reading is currently supported.".to_string());
        },
        
        "fat16" => {
            log_to_file("Using Fat16Formatter");
            let formatter = Fat16Formatter;
            
            formatter.validate_options(&options)
                .await
                .map_err(|e| format!("Invalid options: {}", e))?;
            
            if !formatter.can_format(&device) {
                return Err("Device cannot be formatted".to_string());
            }
            
            // Check size limit
            if device.size > 4 * 1024 * 1024 * 1024 {
                return Err("Device too large for FAT16. Maximum size is 4GB.".to_string());
            }
            
            formatter.format(&device, &options)
                .await
                .map_err(|e| format!("Format failed: {}", e))?;
            
            Ok(format!("Successfully formatted {} as FAT16", device.name))
        },
        
        "fat32" => {
            log_to_file("Using Fat32Formatter");
            let formatter = Fat32Formatter;
            
            formatter.validate_options(&options)
                .await
                .map_err(|e| format!("Invalid options: {}", e))?;
            
            if !formatter.can_format(&device) {
                return Err("Device cannot be formatted".to_string());
            }
            
            // Check size limit
            if device.size > 2 * 1024_u64.pow(4) {
                return Err("Device too large for FAT32. Maximum size is 2TB.".to_string());
            }
            
            formatter.format(&device, &options)
                .await
                .map_err(|e| format!("Format failed: {}", e))?;
            
            Ok(format!("Successfully formatted {} as FAT32", device.name))
        },
        
        "exfat" => {
            log_to_file("Using ExFatFormatter");
            let formatter = ExFatFormatter;
            
            formatter.validate_options(&options)
                .await
                .map_err(|e| format!("Invalid options: {}", e))?;
            
            if !formatter.can_format(&device) {
                return Err("Device cannot be formatted".to_string());
            }
            
            formatter.format(&device, &options)
                .await
                .map_err(|e| format!("Format failed: {}", e))?;
            
            Ok(format!("Successfully formatted {} as exFAT", device.name))
        },
        
        _ => {
            Err(format!("Unsupported filesystem type: {}", options.filesystem_type))
        }
    }
}

fn handle_analyze(device_path: &str) {
    log_to_file(&format!("Analyzing device from file: {}", device_path));
    
    // Check if file exists
    if !Path::new(device_path).exists() {
        let error_msg = format!("Device file not found: {}", device_path);
        log_to_file(&error_msg);
        
        #[cfg(target_os = "windows")]
        show_error_message("File Not Found", &error_msg);
        
        std::process::exit(1);
    }
    
    // Read device JSON from file
    let device_json = match fs::read_to_string(device_path) {
        Ok(json) => json,
        Err(e) => {
            let error_msg = format!("Failed to read device file: {}", e);
            log_to_file(&error_msg);
            
            #[cfg(target_os = "windows")]
            show_error_message("Read Error", &error_msg);
            
            std::process::exit(1);
        }
    };
    
    // Parse device from JSON
    let device: Device = match serde_json::from_str(&device_json) {
        Ok(dev) => dev,
        Err(e) => {
            let error_msg = format!("Failed to parse device JSON: {}", e);
            log_to_file(&error_msg);
            
            #[cfg(target_os = "windows")]
            show_error_message("Parse Error", &error_msg);
            
            std::process::exit(1);
        }
    };
    
    log_to_file(&format!("Analyzing device: {} ({})", device.name, device.id));
    
    // Perform the analysis
    match analyze_unknown_filesystem(&device) {
        Ok(report) => {
            log_to_file("Analysis completed successfully");
            
            // Write result to temp file for parent process to read
            let result_file = env::temp_dir().join(format!("moses_analysis_result_{}.txt", std::process::id()));
            if let Err(e) = fs::write(&result_file, &report) {
                let error_msg = format!("Failed to write result file: {}", e);
                log_to_file(&error_msg);
                
                #[cfg(target_os = "windows")]
                show_error_message("Write Error", &error_msg);
                
                std::process::exit(1);
            }
            
            // Output the result file path for parent process
            println!("{}", result_file.display());
            log_to_file(&format!("Result written to: {}", result_file.display()));
            std::process::exit(0);
        }
        Err(e) => {
            let error_msg = format!("Analysis failed: {:?}", e);
            log_to_file(&error_msg);
            
            #[cfg(target_os = "windows")]
            show_error_message("Analysis Failed", &error_msg);
            
            std::process::exit(1);
        }
    }
}

fn handle_clean(device_path: &str, options_path: &str) {
    log_to_file(&format!("Cleaning device from file: {}", device_path));
    
    // Read device JSON
    let device_json = match fs::read_to_string(device_path) {
        Ok(json) => json,
        Err(e) => {
            let error_msg = format!("Failed to read device file: {}", e);
            log_to_file(&error_msg);
            #[cfg(target_os = "windows")]
            show_error_message("Read Error", &error_msg);
            std::process::exit(1);
        }
    };
    
    let device: Device = match serde_json::from_str(&device_json) {
        Ok(dev) => dev,
        Err(e) => {
            let error_msg = format!("Failed to parse device JSON: {}", e);
            log_to_file(&error_msg);
            #[cfg(target_os = "windows")]
            show_error_message("Parse Error", &error_msg);
            std::process::exit(1);
        }
    };
    
    // Read options JSON
    let options_json = match fs::read_to_string(options_path) {
        Ok(json) => json,
        Err(e) => {
            let error_msg = format!("Failed to read options file: {}", e);
            log_to_file(&error_msg);
            #[cfg(target_os = "windows")]
            show_error_message("Read Error", &error_msg);
            std::process::exit(1);
        }
    };
    
    let options: CleanOptions = match serde_json::from_str(&options_json) {
        Ok(opts) => opts,
        Err(e) => {
            let error_msg = format!("Failed to parse options JSON: {}", e);
            log_to_file(&error_msg);
            #[cfg(target_os = "windows")]
            show_error_message("Parse Error", &error_msg);
            std::process::exit(1);
        }
    };
    
    log_to_file(&format!("Cleaning {} with method {:?}", device.name, options.wipe_method));
    
    // Perform the clean
    match DiskCleaner::clean(&device, &options) {
        Ok(_) => {
            log_to_file("Clean completed successfully");
            println!("Clean completed successfully");
            std::process::exit(0);
        }
        Err(e) => {
            let error_msg = format!("Clean failed: {:?}", e);
            log_to_file(&error_msg);
            #[cfg(target_os = "windows")]
            show_error_message("Clean Failed", &error_msg);
            std::process::exit(1);
        }
    }
}

fn handle_convert(device_path: &str, target_style: &str) {
    log_to_file(&format!("Converting device from file: {} to {}", device_path, target_style));
    
    // Read device JSON
    let device_json = match fs::read_to_string(device_path) {
        Ok(json) => json,
        Err(e) => {
            let error_msg = format!("Failed to read device file: {}", e);
            log_to_file(&error_msg);
            #[cfg(target_os = "windows")]
            show_error_message("Read Error", &error_msg);
            std::process::exit(1);
        }
    };
    
    let device: Device = match serde_json::from_str(&device_json) {
        Ok(dev) => dev,
        Err(e) => {
            let error_msg = format!("Failed to parse device JSON: {}", e);
            log_to_file(&error_msg);
            #[cfg(target_os = "windows")]
            show_error_message("Parse Error", &error_msg);
            std::process::exit(1);
        }
    };
    
    // Parse target style
    let style = match target_style {
        "mbr" => PartitionStyle::MBR,
        "gpt" => PartitionStyle::GPT,
        "uninitialized" => PartitionStyle::Uninitialized,
        _ => {
            let error_msg = format!("Invalid partition style: {}", target_style);
            log_to_file(&error_msg);
            #[cfg(target_os = "windows")]
            show_error_message("Invalid Style", &error_msg);
            std::process::exit(1);
        }
    };
    
    log_to_file(&format!("Converting {} to {:?}", device.name, style));
    
    // Perform the conversion
    match PartitionStyleConverter::convert(&device, style) {
        Ok(_) => {
            log_to_file("Conversion completed successfully");
            println!("Conversion completed successfully");
            std::process::exit(0);
        }
        Err(e) => {
            let error_msg = format!("Conversion failed: {:?}", e);
            log_to_file(&error_msg);
            #[cfg(target_os = "windows")]
            show_error_message("Conversion Failed", &error_msg);
            std::process::exit(1);
        }
    }
}

fn handle_read_directory(device_path: &str, directory_path: &str) {
    use moses_formatters::device_reader::FilesystemReader;
    
    log_to_file(&format!("Reading directory: device={}, path={}", device_path, directory_path));
    
    // Read device JSON
    let device_json = match fs::read_to_string(device_path) {
        Ok(json) => json,
        Err(e) => {
            let error_msg = format!("Failed to read device file: {}", e);
            log_to_file(&error_msg);
            #[cfg(target_os = "windows")]
            show_error_message("Read Error", &error_msg);
            std::process::exit(1);
        }
    };
    
    let device: Device = match serde_json::from_str(&device_json) {
        Ok(dev) => dev,
        Err(e) => {
            let error_msg = format!("Failed to parse device JSON: {}", e);
            log_to_file(&error_msg);
            #[cfg(target_os = "windows")]
            show_error_message("Parse Error", &error_msg);
            std::process::exit(1);
        }
    };
    
    // Detect filesystem type by opening the device
    use moses_formatters::detection::detect_filesystem;
    use moses_formatters::utils::open_device_with_fallback;
    
    let fs_type = match open_device_with_fallback(&device) {
        Ok(mut file) => {
            match detect_filesystem(&mut file) {
                Ok(fs) => fs,
                Err(e) => {
                    let error_msg = format!("Failed to detect filesystem: {:?}", e);
                    log_to_file(&error_msg);
                    
                    // Write error result to temp file
                    let result_path = env::temp_dir().join(format!("moses-read-result-{}.json", std::process::id()));
                    let error_result = serde_json::json!({
                        "success": false,
                        "error": error_msg
                    });
                    let _ = fs::write(&result_path, error_result.to_string());
                    println!("{}", result_path.display());
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            let error_msg = format!("Failed to open device: {:?}", e);
            log_to_file(&error_msg);
            
            // Write error result to temp file
            let result_path = env::temp_dir().join(format!("moses-read-result-{}.json", std::process::id()));
            let error_result = serde_json::json!({
                "success": false,
                "error": error_msg
            });
            let _ = fs::write(&result_path, error_result.to_string());
            println!("{}", result_path.display());
            std::process::exit(1);
        }
    };
    
    log_to_file(&format!("Detected filesystem: {}", fs_type));
    
    // Create appropriate reader based on filesystem type
    let result = match fs_type.as_str() {
        "ntfs" => {
            use moses_formatters::ntfs::NtfsReader;
            match NtfsReader::new(device.clone()) {
                Ok(mut reader) => {
                    reader.list_directory(directory_path)
                        .map_err(|e| format!("Failed to read directory: {:?}", e))
                }
                Err(e) => Err(format!("Failed to open NTFS: {:?}", e))
            }
        }
        "fat32" | "vfat" => {
            use moses_formatters::fat32::Fat32Reader;
            match Fat32Reader::new(device.clone()) {
                Ok(mut reader) => {
                    reader.list_directory(directory_path)
                        .map_err(|e| format!("Failed to read directory: {:?}", e))
                }
                Err(e) => Err(format!("Failed to open FAT32: {:?}", e))
            }
        }
        "fat16" | "fat12" => {
            use moses_formatters::fat16::Fat16Reader;
            match Fat16Reader::new(device.clone()) {
                Ok(mut reader) => {
                    reader.list_directory(directory_path)
                        .map_err(|e| format!("Failed to read directory: {:?}", e))
                }
                Err(e) => Err(format!("Failed to open FAT16: {:?}", e))
            }
        }
        "exfat" => {
            use moses_formatters::exfat::ExFatReader;
            match ExFatReader::new(device.clone()) {
                Ok(mut reader) => {
                    reader.list_directory(directory_path)
                        .map_err(|e| format!("Failed to read directory: {:?}", e))
                }
                Err(e) => Err(format!("Failed to open exFAT: {:?}", e))
            }
        }
        _ => {
            Err(format!("Unsupported filesystem type: {}", fs_type))
        }
    };
    
    // Write result to temp file for parent process to read
    let result_path = env::temp_dir().join(format!("moses-read-result-{}.json", std::process::id()));
    
    match result {
        Ok(entries) => {
            let success_result = serde_json::json!({
                "success": true,
                "entries": entries
            });
            
            match fs::write(&result_path, success_result.to_string()) {
                Ok(_) => {
                    log_to_file(&format!("Successfully read {} entries", entries.len()));
                    println!("{}", result_path.display());
                    std::process::exit(0);
                }
                Err(e) => {
                    let error_msg = format!("Failed to write result file: {}", e);
                    log_to_file(&error_msg);
                    #[cfg(target_os = "windows")]
                    show_error_message("Write Error", &error_msg);
                    std::process::exit(1);
                }
            }
        }
        Err(error) => {
            let error_result = serde_json::json!({
                "success": false,
                "error": error
            });
            
            let _ = fs::write(&result_path, error_result.to_string());
            log_to_file(&format!("Read failed: {}", error));
            println!("{}", result_path.display());
            std::process::exit(1);
        }
    }
}

// Socket mode structures (must match worker_server.rs)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", content = "params")]
enum WorkerCommand {
    Format {
        device: Device,
        options: FormatOptions,
    },
    Clean {
        device: Device,
        options: CleanOptions,
    },
    Analyze {
        device: Device,
    },
    Convert {
        device: Device,
        target_style: String,
    },
    Prepare {
        device: Device,
        target_style: String,
        clean_first: bool,
    },
    ReadDirectory {
        device: Device,
        path: String,
    },
    Ping,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "data")]
enum WorkerResponse {
    Success(String),
    Error(String),
    Progress { percent: u8, message: String },
    Log { level: String, message: String },
    DirectoryListing(String), // JSON serialized directory listing
    Pong,
}

fn handle_socket_mode(port: u16) {
    log_to_file(&format!("Starting socket mode on port {}", port));
    
    // CRITICAL: Check elevation FIRST before doing anything else
    #[cfg(target_os = "windows")]
    {
        use moses_platform::windows::elevation::is_elevated;
        
        if !is_elevated() {
            log_to_file("ERROR: Worker requires administrator privileges");
            log_to_file("The worker must be launched with elevation from Moses");
            
            // Show error to user
            show_error_message(
                "Administrator Required", 
                "This worker process requires administrator privileges.\n\
                 It should be launched from Moses with UAC elevation."
            );
            
            std::process::exit(1); // Exit immediately - no admin rights
        }
        
        log_to_file("Worker running with administrator privileges");
    }
    
    #[cfg(unix)]
    {
        // Check if we're root
        if unsafe { libc::geteuid() } != 0 {
            log_to_file("ERROR: Worker requires root privileges");
            std::process::exit(1);
        }
        log_to_file("Worker running as root");
    }
    
    // Now we're guaranteed to have admin rights, connect to Moses
    log_to_file(&format!("Connecting to Moses on port {}", port));
    
    // Connect to Moses server
    let mut stream = match TcpStream::connect(format!("127.0.0.1:{}", port)) {
        Ok(s) => s,
        Err(e) => {
            log_to_file(&format!("Failed to connect to Moses: {}", e));
            std::process::exit(1);
        }
    };
    
    // Store a clone of the stream for log streaming
    if let Ok(log_stream) = stream.try_clone() {
        let _ = SOCKET_STREAM.set(Mutex::new(Some(log_stream)));
    }
    
    log_to_file("Connected to Moses with admin rights, waiting for commands...");
    
    let reader = BufReader::new(stream.try_clone().expect("Failed to clone stream"));
    
    // Main command loop
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                log_to_file(&format!("Connection error: {}", e));
                break; // Moses closed connection
            }
        };
        
        // Parse command
        let command: WorkerCommand = match serde_json::from_str(&line) {
            Ok(cmd) => cmd,
            Err(e) => {
                log_to_file(&format!("Failed to parse command: {}", e));
                send_response(&mut stream, WorkerResponse::Error(format!("Invalid command: {}", e)));
                continue;
            }
        };
        
        log_to_file(&format!("Received command: {:?}", command));
        
        // Execute command and send response
        let response = match command {
            WorkerCommand::Ping => WorkerResponse::Pong,
            
            WorkerCommand::Shutdown => {
                log_to_file("Received shutdown command");
                send_response(&mut stream, WorkerResponse::Success("Shutting down".to_string()));
                break;
            }
            
            WorkerCommand::Format { device, options } => {
                log_to_file(&format!("Executing format for {}", device.name));
                
                // Use tokio runtime for async format operation
                let runtime = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(e) => {
                        send_response(&mut stream, WorkerResponse::Error(format!("Failed to create runtime: {}", e)));
                        continue;
                    }
                };
                
                let result = runtime.block_on(async {
                    execute_format(device, options).await
                });
                
                match result {
                    Ok(msg) => WorkerResponse::Success(msg),
                    Err(e) => WorkerResponse::Error(e),
                }
            }
            
            WorkerCommand::Clean { device, options } => {
                log_to_file(&format!("Executing clean for {}", device.name));
                match DiskCleaner::clean(&device, &options) {
                    Ok(_) => WorkerResponse::Success("Disk cleaned successfully".to_string()),
                    Err(e) => WorkerResponse::Error(format!("Clean failed: {:?}", e)),
                }
            }
            
            WorkerCommand::Analyze { device } => {
                log_to_file(&format!("Analyzing {}", device.name));
                match analyze_unknown_filesystem(&device) {
                    Ok(report) => WorkerResponse::Success(report),
                    Err(e) => WorkerResponse::Error(format!("Analysis failed: {:?}", e)),
                }
            }
            
            WorkerCommand::Convert { device, target_style } => {
                log_to_file(&format!("Converting {} to {}", device.name, target_style));
                let style = match target_style.as_str() {
                    "mbr" => PartitionStyle::MBR,
                    "gpt" => PartitionStyle::GPT,
                    "uninitialized" => PartitionStyle::Uninitialized,
                    _ => {
                        send_response(&mut stream, WorkerResponse::Error(format!("Invalid partition style: {}", target_style)));
                        continue;
                    }
                };
                
                match PartitionStyleConverter::convert(&device, style) {
                    Ok(_) => WorkerResponse::Success(format!("Converted to {} successfully", target_style)),
                    Err(e) => WorkerResponse::Error(format!("Conversion failed: {:?}", e)),
                }
            }
            
            WorkerCommand::Prepare { device, target_style, clean_first } => {
                log_to_file(&format!("Preparing {} for {}", device.name, target_style));
                let style = match target_style.as_str() {
                    "mbr" => PartitionStyle::MBR,
                    "gpt" => PartitionStyle::GPT,
                    "uninitialized" => PartitionStyle::Uninitialized,
                    _ => {
                        send_response(&mut stream, WorkerResponse::Error(format!("Invalid partition style: {}", target_style)));
                        continue;
                    }
                };
                
                match DiskManager::prepare_disk(&device, style, clean_first) {
                    Ok(report) => WorkerResponse::Success(format!("Disk prepared: {:?}", report)),
                    Err(e) => WorkerResponse::Error(format!("Preparation failed: {:?}", e)),
                }
            }
            
            WorkerCommand::ReadDirectory { device, path } => {
                log_to_file(&format!("Reading directory {} on {}", path, device.name));
                
                // Use the existing handle_read_directory function
                // First write device to temp file
                let device_json = match serde_json::to_string(&device) {
                    Ok(j) => j,
                    Err(e) => {
                        send_response(&mut stream, WorkerResponse::Error(format!("Failed to serialize device: {}", e)));
                        continue;
                    }
                };
                
                let temp_dir = std::env::temp_dir();
                let device_file = temp_dir.join(format!("moses-device-socket-{}.json", std::process::id()));
                
                if let Err(e) = std::fs::write(&device_file, device_json) {
                    send_response(&mut stream, WorkerResponse::Error(format!("Failed to write device file: {}", e)));
                    continue;
                }
                
                // Call the existing handler
                handle_read_directory(device_file.to_str().unwrap_or(""), &path);
                
                // Read the result file
                let result_file = temp_dir.join(format!("moses-read-result-{}.json", std::process::id()));
                
                match std::fs::read_to_string(&result_file) {
                    Ok(content) => {
                        // Clean up temp files
                        let _ = std::fs::remove_file(&device_file);
                        let _ = std::fs::remove_file(&result_file);
                        
                        WorkerResponse::DirectoryListing(content)
                    }
                    Err(e) => {
                        // Clean up temp files
                        let _ = std::fs::remove_file(&device_file);
                        
                        WorkerResponse::Error(format!("Failed to read directory: {}", e))
                    }
                }
            }
        };
        
        send_response(&mut stream, response);
    }
    
    log_to_file("Worker shutting down");
}

fn send_response(stream: &mut TcpStream, response: WorkerResponse) {
    let json = match serde_json::to_string(&response) {
        Ok(j) => j,
        Err(e) => {
            log_to_file(&format!("Failed to serialize response: {}", e));
            return;
        }
    };
    
    if let Err(e) = stream.write_all(json.as_bytes()) {
        log_to_file(&format!("Failed to send response: {}", e));
        return;
    }
    
    if let Err(e) = stream.write_all(b"\n") {
        log_to_file(&format!("Failed to send newline: {}", e));
        return;
    }
    
    if let Err(e) = stream.flush() {
        log_to_file(&format!("Failed to flush stream: {}", e));
    }
}

fn handle_prepare(device_path: &str, target_style: &str, clean_first: bool) {
    log_to_file(&format!("Preparing device from file: {} to {} (clean: {})", 
                         device_path, target_style, clean_first));
    
    // Read device JSON
    let device_json = match fs::read_to_string(device_path) {
        Ok(json) => json,
        Err(e) => {
            let error_msg = format!("Failed to read device file: {}", e);
            log_to_file(&error_msg);
            #[cfg(target_os = "windows")]
            show_error_message("Read Error", &error_msg);
            std::process::exit(1);
        }
    };
    
    let device: Device = match serde_json::from_str(&device_json) {
        Ok(dev) => dev,
        Err(e) => {
            let error_msg = format!("Failed to parse device JSON: {}", e);
            log_to_file(&error_msg);
            #[cfg(target_os = "windows")]
            show_error_message("Parse Error", &error_msg);
            std::process::exit(1);
        }
    };
    
    // Parse target style
    let style = match target_style {
        "mbr" => PartitionStyle::MBR,
        "gpt" => PartitionStyle::GPT,
        "uninitialized" => PartitionStyle::Uninitialized,
        _ => {
            let error_msg = format!("Invalid partition style: {}", target_style);
            log_to_file(&error_msg);
            #[cfg(target_os = "windows")]
            show_error_message("Invalid Style", &error_msg);
            std::process::exit(1);
        }
    };
    
    log_to_file(&format!("Preparing {} for {:?} (clean first: {})", 
                         device.name, style, clean_first));
    
    // Perform the preparation
    match DiskManager::prepare_disk(&device, style, clean_first) {
        Ok(report) => {
            log_to_file(&format!("Preparation completed successfully: {:?}", report));
            println!("Preparation completed successfully");
            std::process::exit(0);
        }
        Err(e) => {
            let error_msg = format!("Preparation failed: {:?}", e);
            log_to_file(&error_msg);
            #[cfg(target_os = "windows")]
            show_error_message("Preparation Failed", &error_msg);
            std::process::exit(1);
        }
    }
}