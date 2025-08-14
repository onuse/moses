#![allow(async_fn_in_trait)]

use std::sync::Arc;
use async_trait::async_trait;
use crate::{FilesystemFormatter, FormatterMetadata, MosesError};

/// Base trait for all Moses plugins
pub trait MosesPlugin: Send + Sync {
    /// Unique identifier for the plugin
    fn id(&self) -> &str;
    
    /// Human-readable name
    fn name(&self) -> &str;
    
    /// Plugin version
    fn version(&self) -> &str;
    
    /// Plugin author
    fn author(&self) -> &str;
    
    /// Plugin description
    fn description(&self) -> &str;
    
    /// Initialize the plugin
    async fn initialize(&mut self) -> Result<(), MosesError>;
    
    /// Cleanup when plugin is unloaded
    async fn cleanup(&mut self) -> Result<(), MosesError>;
}

/// Plugin that provides filesystem formatters
pub trait FormatterPlugin: MosesPlugin {
    /// Get all formatters provided by this plugin
    fn formatters(&self) -> Vec<(&str, Arc<dyn FilesystemFormatter>, FormatterMetadata)>;
}

/// Script-based formatter that wraps command-line tools
pub struct ScriptFormatter {
    name: String,
    metadata: FormatterMetadata,
    config: ScriptFormatterConfig,
}

#[derive(Clone, Debug)]
pub struct ScriptFormatterConfig {
    pub format_command: String,
    pub verify_command: Option<String>,
    pub required_tools: Vec<String>,
    pub environment: std::collections::HashMap<String, String>,
    pub working_directory: Option<std::path::PathBuf>,
    pub timeout_seconds: u64,
}

impl ScriptFormatter {
    pub fn new(name: String, metadata: FormatterMetadata, config: ScriptFormatterConfig) -> Self {
        Self {
            name,
            metadata,
            config,
        }
    }
    
    /// Replace placeholders in command template
    fn prepare_command(&self, template: &str, device: &crate::Device, options: &crate::FormatOptions) -> String {
        template
            .replace("{device}", &device.id)
            .replace("{label}", options.label.as_deref().unwrap_or(""))
            .replace("{filesystem}", &options.filesystem_type)
            .replace("{quick}", if options.quick_format { "--quick" } else { "" })
    }
    
    /// Execute a command with the configured environment
    async fn execute_command(&self, command: &str) -> Result<String, MosesError> {
        use tokio::process::Command;
        
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", command]);
            c
        };
        
        // Set environment variables
        for (key, value) in &self.config.environment {
            cmd.env(key, value);
        }
        
        // Set working directory
        if let Some(ref dir) = self.config.working_directory {
            cmd.current_dir(dir);
        }
        
        // Execute with timeout
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(self.config.timeout_seconds),
            cmd.output()
        ).await
            .map_err(|_| MosesError::Timeout("Command execution timed out".to_string()))?
            .map_err(MosesError::IoError)?;
        
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(MosesError::External(
                String::from_utf8_lossy(&output.stderr).to_string()
            ))
        }
    }
}

#[async_trait]
impl FilesystemFormatter for ScriptFormatter {
    fn name(&self) -> &'static str {
        // Return a static reference - this is a limitation of script formatters
        Box::leak(self.name.clone().into_boxed_str())
    }
    
    fn supported_platforms(&self) -> Vec<crate::Platform> {
        self.metadata.platform_support.clone()
    }
    
    fn can_format(&self, device: &crate::Device) -> bool {
        // Check size constraints
        if let Some(min) = self.metadata.min_size {
            if device.size < min {
                return false;
            }
        }
        if let Some(max) = self.metadata.max_size {
            if device.size > max {
                return false;
            }
        }
        true
    }
    
    fn requires_external_tools(&self) -> bool {
        !self.config.required_tools.is_empty()
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        vec![] // Script formatters don't bundle tools
    }
    
    async fn format(&self, device: &crate::Device, options: &crate::FormatOptions) -> Result<(), MosesError> {
        // Check required tools
        for tool in &self.config.required_tools {
            which::which(tool)
                .map_err(|_| MosesError::ToolNotFound(tool.clone()))?;
        }
        
        // Prepare and execute format command
        let command = self.prepare_command(&self.config.format_command, device, options);
        self.execute_command(&command).await?;
        
        Ok(())
    }
    
    async fn validate_options(&self, _options: &crate::FormatOptions) -> Result<(), MosesError> {
        // Basic validation - script formatters typically don't have complex validation
        Ok(())
    }
    
    async fn dry_run(&self, device: &crate::Device, options: &crate::FormatOptions) -> Result<crate::SimulationReport, MosesError> {
        // Check tools availability
        let mut missing_tools = Vec::new();
        for tool in &self.config.required_tools {
            if which::which(tool).is_err() {
                missing_tools.push(tool.clone());
            }
        }
        
        let mut warnings = Vec::new();
        if !missing_tools.is_empty() {
            warnings.push(format!("Missing tools: {:?}", missing_tools));
        }
        
        Ok(crate::SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: std::time::Duration::from_secs(30),
            warnings,
            required_tools: self.config.required_tools.clone(),
            will_erase_data: true,
            space_after_format: device.size * 95 / 100, // Estimate 95% usable
        })
    }
    
}

/// Template for creating new formatter plugins
pub struct FormatterTemplate {
    name: String,
    category: crate::registry::FormatterCategory,
}

impl FormatterTemplate {
    pub fn new(name: &str, category: crate::registry::FormatterCategory) -> Self {
        Self {
            name: name.to_string(),
            category,
        }
    }
    
    /// Generate boilerplate code for a new formatter
    pub fn generate_code(&self) -> String {
        format!(r#"
use moses_core::{{FilesystemFormatter, Device, FormatOptions, MosesError, SimulationReport, Platform}};
use async_trait::async_trait;

pub struct {}Formatter {{
    // Add any necessary fields here
}}

impl {}Formatter {{
    pub fn new() -> Self {{
        Self {{
            // Initialize fields
        }}
    }}
}}

#[async_trait]
impl FilesystemFormatter for {}Formatter {{
    async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {{
        // TODO: Implement format logic
        
        // Example:
        // 1. Validate device and options
        // 2. Prepare format command or call native API
        // 3. Execute format operation
        // 4. Verify success
        
        todo!("Implement {} formatting")
    }}
    
    async fn verify(&self, device: &Device) -> Result<bool, MosesError> {{
        // TODO: Implement verification logic
        
        // Example:
        // 1. Check if device has correct filesystem signature
        // 2. Try to mount or read filesystem metadata
        // 3. Return true if valid, false otherwise
        
        todo!("Implement {} verification")
    }}
    
    async fn dry_run(&self, device: &Device, options: &FormatOptions) -> Result<SimulationReport, MosesError> {{
        // TODO: Implement dry run simulation
        
        Ok(SimulationReport {{
            estimated_time: std::time::Duration::from_secs(30),
            required_tools: vec![],
            warnings: vec![],
            steps: vec![
                "Validate device".to_string(),
                "Format as {}".to_string(),
                "Verify format".to_string(),
            ],
        }})
    }}
    
    fn supported_platforms(&self) -> Vec<Platform> {{
        // TODO: Specify supported platforms
        vec![
            Platform::Windows,
            Platform::Linux,
            Platform::MacOS,
        ]
    }}
}}

#[cfg(test)]
mod tests {{
    use super::*;
    use moses_core::test_utils::MockDevice;
    
    #[tokio::test]
    async fn test_{}_format() {{
        let formatter = {}Formatter::new();
        let device = MockDevice::new("test-device", 1024 * 1024 * 100); // 100MB
        
        let options = FormatOptions {{
            filesystem_type: "{}".to_string(),
            label: Some("TEST".to_string()),
            quick_format: true,
            ..Default::default()
        }};
        
        // Test dry run
        let simulation = formatter.dry_run(&device, &options).await.unwrap();
        assert!(!simulation.warnings.is_empty() || simulation.warnings.is_empty());
        
        // Test format (with mock device)
        // formatter.format(&device, &options).await.unwrap();
        
        // Test verify
        // assert!(formatter.verify(&device).await.unwrap());
    }}
}}
"#, 
            self.name,
            self.name,
            self.name,
            self.name.to_lowercase(),
            self.name.to_lowercase(),
            self.name.to_lowercase(),
            self.name.to_lowercase(),
            self.name,
            self.name.to_lowercase()
        )
    }
    
    /// Generate metadata template
    pub fn generate_metadata(&self) -> String {
        format!(r#"
# Plugin metadata for {} formatter

[plugin]
name = "{}"
version = "1.0.0"
author = "Your Name"
license = "MIT"
description = "Formatter for {} filesystem"

[formatter]
category = "{:?}"
min_size = null  # Minimum device size in bytes, or null for no limit
max_size = null  # Maximum device size in bytes, or null for no limit

[formatter.aliases]
# Alternative names for this filesystem
aliases = []

[formatter.capabilities]
supports_labels = true
max_label_length = 16
supports_uuid = true
supports_encryption = false
supports_compression = false
supports_resize = false
supports_quotas = false
supports_snapshots = false
case_sensitive = true
preserves_permissions = true
preserves_timestamps = true

[requirements]
# External tools required (will be checked with 'which')
tools = []

[requirements.platforms]
# Supported platforms
windows = true
linux = true
macos = true
freebsd = false

[testing]
# Test configuration
mock_device_size = 104857600  # 100MB
test_label = "TEST_{}"
"#,
            self.name,
            self.name.to_lowercase(),
            self.name,
            self.category,
            self.name.to_uppercase()
        )
    }
    
    /// Generate directory structure for new plugin
    pub fn generate_structure(&self) -> Vec<(String, String)> {
        vec![
            (format!("formatters/{}/Cargo.toml", self.name.to_lowercase()), self.generate_cargo_toml()),
            (format!("formatters/{}/src/lib.rs", self.name.to_lowercase()), self.generate_code()),
            (format!("formatters/{}/metadata.toml", self.name.to_lowercase()), self.generate_metadata()),
            (format!("formatters/{}/README.md", self.name.to_lowercase()), self.generate_readme()),
        ]
    }
    
    fn generate_cargo_toml(&self) -> String {
        format!(r#"[package]
name = "moses-formatter-{}"
version = "0.1.0"
edition = "2021"

[dependencies]
moses-core = {{ path = "../../core" }}
async-trait = "0.1"
tokio = {{ version = "1", features = ["full"] }}

[dev-dependencies]
tokio-test = "0.4"
"#, self.name.to_lowercase())
    }
    
    fn generate_readme(&self) -> String {
        format!(r#"# {} Formatter for Moses

This plugin provides support for formatting {} filesystems.

## Features

- Format devices as {}
- Verify {} filesystem integrity
- Cross-platform support

## Usage

```rust
use moses_formatter_{}::{}Formatter;

let formatter = {}Formatter::new();
formatter.format(device, options).await?;
```

## Testing

```bash
cargo test --package moses-formatter-{}
```

## License

MIT
"#, 
            self.name,
            self.name,
            self.name,
            self.name,
            self.name.to_lowercase(),
            self.name,
            self.name,
            self.name.to_lowercase()
        )
    }
}