use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub source: Option<String>,
    pub timestamp: String,
}

pub struct LogCapture {
    app_handle: Option<AppHandle>,
}

impl LogCapture {
    pub fn new() -> Self {
        Self { app_handle: None }
    }
    
    pub fn set_app_handle(&mut self, handle: AppHandle) {
        self.app_handle = Some(handle);
    }
    
    pub fn log(&self, level: &str, message: &str, source: Option<&str>) {
        if let Some(handle) = &self.app_handle {
            let entry = LogEntry {
                level: level.to_string(),
                message: message.to_string(),
                source: source.map(String::from),
                timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
            };
            
            // Emit log event to frontend
            let _ = handle.emit("backend-log", entry);
        }
        
        // Also log to console
        eprintln!("[{}] {} {}", level, source.unwrap_or(""), message);
    }
    
    pub fn debug(&self, message: &str, source: Option<&str>) {
        self.log("DEBUG", message, source);
    }
    
    pub fn info(&self, message: &str, source: Option<&str>) {
        self.log("INFO", message, source);
    }
    
    pub fn warn(&self, message: &str, source: Option<&str>) {
        self.log("WARN", message, source);
    }
    
    pub fn error(&self, message: &str, source: Option<&str>) {
        self.log("ERROR", message, source);
    }
}

// Global logger instance
lazy_static::lazy_static! {
    pub static ref LOGGER: Mutex<LogCapture> = Mutex::new(LogCapture::new());
}

pub fn init_logger(app_handle: AppHandle) {
    let mut logger = LOGGER.lock().unwrap();
    logger.set_app_handle(app_handle);
}

// Convenience macros
#[macro_export]
macro_rules! log_debug {
    ($msg:expr) => {
        $crate::logging::LOGGER.lock().unwrap().debug($msg, None)
    };
    ($msg:expr, $src:expr) => {
        $crate::logging::LOGGER.lock().unwrap().debug($msg, Some($src))
    };
}

#[macro_export]
macro_rules! log_info {
    ($msg:expr) => {
        $crate::logging::LOGGER.lock().unwrap().info($msg, None)
    };
    ($msg:expr, $src:expr) => {
        $crate::logging::LOGGER.lock().unwrap().info($msg, Some($src))
    };
}

#[macro_export]
macro_rules! log_error {
    ($msg:expr) => {
        $crate::logging::LOGGER.lock().unwrap().error($msg, None)
    };
    ($msg:expr, $src:expr) => {
        $crate::logging::LOGGER.lock().unwrap().error($msg, Some($src))
    };
}