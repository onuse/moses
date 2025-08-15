// Elevated worker process for privileged operations
// This process gets elevated privileges via UAC to perform format operations

use std::env;
use std::fs;
use std::path::Path;
use std::io::Write;
use moses_core::{Device, FormatOptions, FilesystemFormatter};
use moses_formatters::{NtfsFormatter, Fat32Formatter, ExFatFormatter};
use serde_json;
use log::{Record, Level, Metadata, LevelFilter};

#[cfg(target_os = "windows")]
use moses_formatters::Ext4NativeFormatter;

#[cfg(target_os = "linux")]
use moses_formatters::Ext4LinuxFormatter;

// Global log file path for this worker instance
static mut LOG_FILE_PATH: Option<std::path::PathBuf> = None;

// Simple file logging function
fn log_to_file(msg: &str) {
    unsafe {
        if let Some(ref path) = LOG_FILE_PATH {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path) 
            {
                let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
                let _ = writeln!(file, "[{}] {}", timestamp, msg);
            }
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
            let msg = format!("[{}] {}: {}", 
                record.level(), 
                record.target(), 
                record.args());
            log_to_file(&msg);
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
fn show_error_message(_title: &str, _message: &str) {
    // No-op on non-Windows platforms
}

fn main() {
    // Increase stack size to prevent stack overflow with large buffers
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024) // 8MB stack
        .spawn(|| run_worker())
        .unwrap()
        .join()
        .unwrap();
}

fn run_worker() {
    // Set up file logging for the worker since UAC hides console output
    let log_file_path = env::temp_dir().join(format!("moses-worker-{}.log", std::process::id()));
    
    // Store the log file path globally
    unsafe {
        LOG_FILE_PATH = Some(log_file_path.clone());
    }
    
    // Initialize the logger to capture log crate output
    #[cfg(debug_assertions)]
    let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(LevelFilter::Trace));
    
    #[cfg(not(debug_assertions))]
    let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(LevelFilter::Info));
    
    // In production, only log essential info
    #[cfg(debug_assertions)]
    {
        log_to_file(&format!("========================================"));
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
    
    if args.len() < 3 {
        let error_msg = format!(
            "Error: Insufficient arguments\nUsage: moses-formatter <device-json-file> <options-json-file>\nReceived {} arguments:\n{}",
            args.len(),
            args.join("\n")
        );
        log_to_file(&error_msg);
        
        #[cfg(target_os = "windows")]
        show_error_message("Invalid Arguments", &error_msg);
        
        std::process::exit(1);
    }
    
    // Parse device and options from JSON
    // We expect file paths from the parent process
    let device_path = &args[1];
    let options_path = &args[2];
    
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
    log_to_file(&format!("========================================"));
    log_to_file(&format!("Starting format operation for device: {}", device.name));
    log_to_file(&format!("Device ID: {}", device.id));
    log_to_file(&format!("Device size: {} bytes ({} GB)", device.size, device.size / (1024*1024*1024)));
    log_to_file(&format!("Filesystem type: {}", options.filesystem_type));
    log_to_file(&format!("Cluster size: {:?}", options.cluster_size));
    log_to_file(&format!("Quick format: {}", options.quick_format));
    log_to_file(&format!("Verify after format: {}", options.verify_after_format));
    log_to_file(&format!("========================================"));
    
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
            let error_with_log = format!(
                "Format failed: {}\n\nCheck log file for details:\n{}", 
                e, log_file_path.display()
            );
            
            #[cfg(target_os = "windows")]
            show_error_message("Format Failed", &error_with_log);
            
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
    
    // Execute format based on filesystem type
    match options.filesystem_type.as_str() {
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
            log_to_file("Using NtfsFormatter");
            let formatter = NtfsFormatter;
            
            log_to_file("Validating options...");
            formatter.validate_options(&options)
                .await
                .map_err(|e| format!("Invalid options: {}", e))?;
            
            log_to_file("Checking if device can be formatted...");
            if !formatter.can_format(&device) {
                return Err("Device cannot be formatted".to_string());
            }
            
            log_to_file("Starting format...");
            formatter.format(&device, &options)
                .await
                .map_err(|e| format!("Format failed: {}", e))?;
            
            Ok(format!("Successfully formatted {} as NTFS", device.name))
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