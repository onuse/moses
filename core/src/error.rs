use thiserror::Error;

#[derive(Debug, Error)]
pub enum MosesError {
    #[error("Device not found: {0}")]
    DeviceNotFound(String),
    
    #[error("Insufficient privileges: {0}")]
    InsufficientPrivileges(String),
    
    #[error("Formatting failed: {0}")]
    FormatError(String),
    
    #[error("Platform not supported: {0}")]
    PlatformNotSupported(String),
    
    #[error("External tool missing: {0}")]
    ExternalToolMissing(String),
    
    #[error("Operation cancelled by user")]
    UserCancelled,
    
    #[error("Simulation mode: {0}")]
    SimulationOnly(String),
    
    #[error("Device is not safe to format: {0}")]
    UnsafeDevice(String),
    
    #[error("Safety violation: {0}")]
    SafetyViolation(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("Format failed: {0}")]
    Format(String),
    
    #[error("Timeout: {0}")]
    Timeout(String),
    
    #[error("External command failed: {0}")]
    External(String),
    
    #[error("Not supported: {0}")]
    NotSupported(String),
    
    #[error("Other error: {0}")]
    Other(String),
}