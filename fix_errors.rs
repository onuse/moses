// Additional error variants needed for compilation
// Add these to core/src/error.rs before the closing brace

    #[error("Is a directory: {path}")]
    IsADirectory { path: PathBuf },
    
    #[error("External error: {0}")]
    External(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    
    #[error("Unsafe device: {0}")]
    UnsafeDevice(String),
    
    #[error("Format error: {0}")]
    FormatError(String),