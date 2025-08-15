use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use serde::{Serialize, Deserialize};
use log::{Log, Metadata, Record, Level};

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
}

// Global logger instance
lazy_static::lazy_static! {
    pub static ref LOGGER: Mutex<LogCapture> = Mutex::new(LogCapture::new());
}

// Bridge to standard log crate
pub struct TauriLogger;

impl Log for TauriLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let level = match record.level() {
                Level::Error => "ERROR",
                Level::Warn => "WARN",
                Level::Info => "INFO",
                Level::Debug => "DEBUG",
                Level::Trace => "DEBUG",
            };
            
            let source = record.target();
            let message = format!("{}", record.args());
            
            if let Ok(logger) = LOGGER.lock() {
                logger.log(level, &message, Some(source));
            }
        }
    }

    fn flush(&self) {}
}

pub fn init_logger(app_handle: AppHandle) {
    // Set up the Tauri app handle
    let mut logger = LOGGER.lock().unwrap();
    logger.set_app_handle(app_handle);
    drop(logger);
    
    // Initialize the log crate to use our TauriLogger
    let _ = log::set_boxed_logger(Box::new(TauriLogger));
    log::set_max_level(log::LevelFilter::Debug);
    
    // Log that the logger is initialized
    log::info!("Logger initialized and connected to UI console");
}