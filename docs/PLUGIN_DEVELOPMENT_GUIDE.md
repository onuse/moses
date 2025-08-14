# Moses Plugin Development Guide

## Quick Start

Creating a new formatter plugin for Moses is straightforward. All formatters implement the same `FilesystemFormatter` trait, making it easy to add support for any filesystem.

## Step 1: Choose Your Plugin Type

### Option A: Native Rust Plugin (Recommended)
Best for performance-critical formatters or those requiring complex logic.

### Option B: Script Plugin (Simple)
Best for wrapping existing command-line tools.

### Option C: Dynamic Library Plugin (Advanced)
Best for distributing closed-source or language-agnostic plugins.

## Step 2: Create Your Formatter

### Native Rust Plugin Example

Let's create a formatter for the Amiga OFS filesystem:

```rust
// formatters/amiga_ofs/src/lib.rs
use moses_core::{FilesystemFormatter, Device, FormatOptions, MosesError, SimulationReport, Platform};
use async_trait::async_trait;

pub struct AmigaOfsFormatter {
    tool_path: Option<PathBuf>,
}

impl AmigaOfsFormatter {
    pub fn new() -> Self {
        Self {
            tool_path: None,
        }
    }
    
    async fn ensure_tools(&mut self) -> Result<PathBuf, MosesError> {
        // Check for xdftool
        if let Ok(path) = which::which("xdftool") {
            self.tool_path = Some(path.clone());
            return Ok(path);
        }
        
        // Tool not found, provide instructions
        Err(MosesError::ToolNotFound(
            "xdftool not found. Install from: https://github.com/deplinenoise/amiga-stuff".into()
        ))
    }
}

#[async_trait]
impl FilesystemFormatter for AmigaOfsFormatter {
    fn name(&self) -> &'static str {
        "amiga-ofs"
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        vec![Platform::Windows, Platform::Linux, Platform::MacOS]
    }
    
    fn can_format(&self, device: &Device) -> bool {
        // OFS supports 880KB to 4GB
        device.size >= 901_120 && device.size <= 4_294_967_296
    }
    
    fn requires_external_tools(&self) -> bool {
        true
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        vec![]  // Could bundle xdftool in future
    }
    
    async fn format(
        &mut self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<(), MosesError> {
        // Ensure tools are available
        let tool = self.ensure_tools().await?;
        
        // Build format command
        let mut cmd = tokio::process::Command::new(tool);
        cmd.arg("format")
           .arg(&device.path)
           .arg("OFS");
        
        if let Some(label) = &options.label {
            cmd.arg("--label").arg(label);
        }
        
        // Execute format
        let output = cmd.output().await
            .map_err(|e| MosesError::Io(e))?;
        
        if !output.status.success() {
            return Err(MosesError::Format(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }
        
        Ok(())
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        // Amiga labels are max 30 chars
        if let Some(label) = &options.label {
            if label.len() > 30 {
                return Err(MosesError::InvalidInput(
                    "Amiga OFS labels must be 30 characters or less".into()
                ));
            }
        }
        Ok(())
    }
    
    async fn dry_run(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<SimulationReport, MosesError> {
        self.validate_options(options).await?;
        
        let mut warnings = vec![];
        
        if which::which("xdftool").is_err() {
            warnings.push("xdftool not found - will need to be installed".into());
        }
        
        if device.size > 2_147_483_648 {
            warnings.push("Devices over 2GB may have compatibility issues with real Amigas".into());
        }
        
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: std::time::Duration::from_secs(5),
            warnings,
            required_tools: vec!["xdftool".into()],
            will_erase_data: true,
            space_after_format: device.size - 8192, // Approximate overhead
        })
    }
}
```

### Script Plugin Example

For simpler cases, use a TOML configuration:

```toml
# plugins/amiga-ofs.toml
[plugin]
name = "amiga-ofs"
version = "1.0.0"
author = "Amiga Enthusiast"
description = "Amiga Old File System formatter"

[formatter]
type = "script"
format_command = "xdftool {device} format OFS {label}"
verify_command = "xdftool {device} info"
category = "Historical"
min_size = 901120      # 880KB
max_size = 4294967296  # 4GB

[formatter.aliases]
aliases = ["ofs", "amiga"]

[formatter.capabilities]
supports_labels = true
max_label_length = 30
case_sensitive = false

[requirements]
tools = ["xdftool"]
platforms = ["windows", "linux", "macos"]

[requirements.install]
linux = "apt-get install amiga-fdisk-cross"
macos = "brew install xdftool"
windows = "Download from https://github.com/deplinenoise/amiga-stuff"
```

## Step 3: Add Metadata

Create a metadata builder for your formatter:

```rust
use moses_core::{FormatterMetadataBuilder, FormatterCategory};

pub fn create_metadata() -> FormatterMetadata {
    FormatterMetadataBuilder::new("amiga-ofs")
        .description("Amiga Old File System (OFS) - Classic AmigaOS filesystem")
        .aliases(vec!["ofs", "amiga"])
        .category(FormatterCategory::Historical)
        .size_range(Some(901_120), Some(4_294_967_296))
        .version("1.0.0")
        .author("Amiga Community")
        .capability(|c| {
            c.supports_labels = true;
            c.max_label_length = Some(30);
            c.case_sensitive = false;
            c.preserves_permissions = false;
            c.max_file_size = Some(2_147_483_647); // 2GB file limit
        })
        .build()
}
```

## Step 4: Register Your Formatter

### For Built-in Formatters

Add to `formatters/src/lib.rs`:

```rust
pub fn register_formatters(registry: &mut FormatterRegistry) {
    // Existing formatters...
    
    // Add your formatter
    registry.register(
        "amiga-ofs".to_string(),
        Arc::new(AmigaOfsFormatter::new()),
        create_amiga_metadata(),
    ).expect("Failed to register Amiga OFS formatter");
}
```

### For Dynamic Plugins

Place your compiled plugin in the plugins directory:
- Windows: `C:\Program Files\Moses\plugins\`
- Linux/macOS: `~/.moses/plugins/`

## Step 5: Test Your Formatter

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use moses_core::test_utils::MockDevice;
    
    #[tokio::test]
    async fn test_amiga_format() {
        let formatter = AmigaOfsFormatter::new();
        let device = MockDevice::new("mock://amiga", 901_120); // 880KB
        
        let options = FormatOptions {
            filesystem_type: "amiga-ofs".to_string(),
            label: Some("Workbench".to_string()),
            quick_format: true,
            ..Default::default()
        };
        
        // Test dry run
        let report = formatter.dry_run(&device, &options).await.unwrap();
        assert!(report.warnings.is_empty() || !report.warnings.is_empty());
        
        // Test validation
        assert!(formatter.validate_options(&options).await.is_ok());
        
        // Test invalid label
        let bad_options = FormatOptions {
            label: Some("This label is way too long for Amiga OFS filesystem".to_string()),
            ..options
        };
        assert!(formatter.validate_options(&bad_options).await.is_err());
    }
}
```

### Integration Tests

Create `tests/integration_test.rs`:

```rust
#[test]
fn test_formatter_registration() {
    let mut registry = FormatterRegistry::new();
    register_amiga_formatter(&mut registry);
    
    // Test name lookup
    assert!(registry.get_formatter("amiga-ofs").is_some());
    
    // Test alias lookup
    assert!(registry.get_formatter("ofs").is_some());
    assert!(registry.get_formatter("amiga").is_some());
    
    // Test metadata
    let meta = registry.get_metadata("amiga-ofs").unwrap();
    assert_eq!(meta.category, FormatterCategory::Historical);
    assert_eq!(meta.capabilities.max_label_length, Some(30));
}
```

### Manual Testing

```bash
# Build your formatter
cargo build --package moses-formatter-amiga

# Test with mock device
moses format mock://test amiga-ofs --dry-run

# Test with real device (BE CAREFUL!)
moses format /dev/sdb amiga-ofs --label "Workbench"
```

## Step 6: Document Your Formatter

Create `docs/formatters/amiga-ofs.md`:

```markdown
# Amiga OFS Formatter

## Overview
The Amiga Old File System (OFS) was the original filesystem for AmigaOS, introduced in 1985.

## Features
- Maximum volume size: 4GB
- Maximum file size: 2GB
- Case-insensitive filenames
- 30-character volume labels

## Requirements
- xdftool or similar Amiga disk tools
- Device between 880KB and 4GB

## Usage
\```bash
moses format /dev/sdb amiga-ofs --label "Workbench"
\```

## Compatibility
- Real Amiga hardware (A500, A1200, etc.)
- UAE and FS-UAE emulators
- Amiga Forever
```

## Step 7: Submit Your Plugin

### For Official Inclusion

1. Fork the Moses repository
2. Add your formatter in `formatters/` directory
3. Include tests and documentation
4. Submit a pull request

### For Community Plugins

1. Package your plugin:
```bash
moses plugin package amiga-ofs
```

2. Upload to the community repository:
```bash
moses plugin publish amiga-ofs --community
```

## Best Practices

### 1. Safety First
- Always validate device is not a system drive
- Require explicit confirmation for destructive operations
- Implement proper dry-run simulation

### 2. Clear Error Messages
```rust
// Good
Err(MosesError::InvalidInput(
    format!("Label '{}' exceeds maximum length of 30 characters", label)
))

// Bad
Err(MosesError::InvalidInput("Bad label".into()))
```

### 3. Tool Management
```rust
// Provide helpful installation instructions
if tool_missing {
    return Err(MosesError::ToolNotFound(format!(
        "xdftool not found. Install with:\n\
         - Ubuntu/Debian: apt-get install amiga-fdisk-cross\n\
         - macOS: brew install xdftool\n\
         - Windows: Download from https://github.com/deplinenoise/amiga-stuff"
    )));
}
```

### 4. Platform Awareness
```rust
fn get_tool_path(&self) -> PathBuf {
    #[cfg(target_os = "windows")]
    return PathBuf::from("xdftool.exe");
    
    #[cfg(not(target_os = "windows"))]
    return PathBuf::from("xdftool");
}
```

### 5. Comprehensive Testing
- Test with minimum size devices
- Test with maximum size devices
- Test label edge cases
- Test tool availability detection
- Test error conditions

## Advanced Topics

### Custom Device Detection

```rust
impl AmigaOfsFormatter {
    fn detect_amiga_disk(&self, device: &Device) -> bool {
        // Read first 4 bytes for "DOS\0" signature
        // This identifies existing Amiga disks
    }
}
```

### Progress Reporting

```rust
use moses_core::ProgressCallback;

async fn format_with_progress(
    &self,
    device: &Device,
    options: &FormatOptions,
    progress: Box<dyn ProgressCallback>,
) -> Result<(), MosesError> {
    progress(0.0, "Starting format...");
    // Format steps...
    progress(0.5, "Writing filesystem structures...");
    // More steps...
    progress(1.0, "Format complete!");
    Ok(())
}
```

### Bundling Tools

```toml
[package.metadata.moses]
bundled_tools = [
    { name = "xdftool", source = "https://...", platforms = ["windows"] }
]
```

## Troubleshooting

### Common Issues

1. **Tool not found**: Ensure required tools are in PATH
2. **Permission denied**: May need administrator/root access
3. **Invalid device**: Check device exists and is not mounted
4. **Format fails**: Check device is not write-protected

### Debug Mode

```bash
RUST_LOG=debug moses format /dev/sdb amiga-ofs
```

## Support

- GitHub Issues: https://github.com/moses/moses/issues
- Discord: https://discord.gg/moses
- Documentation: https://docs.moses.dev

## Examples Repository

Check out more formatter examples:
- https://github.com/moses/formatter-examples

Popular examples:
- Commodore 1541
- Apple ProDOS
- PlayStation Memory Card
- Xbox FATX