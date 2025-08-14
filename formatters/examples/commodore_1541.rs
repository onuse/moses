/// Example plugin: Commodore 1541 disk formatter
/// 
/// This demonstrates how to create a formatter for historical filesystems
/// using the Moses plugin architecture.
/// 
/// The Commodore 1541 was a 5.25" floppy disk drive for the Commodore 64
/// that used a proprietary disk format storing 170KB per disk.

use moses_core::{
    FilesystemFormatter, Device, FormatOptions, MosesError, 
    SimulationReport, Platform, FormatterMetadataBuilder, 
    FormatterCategory, FormatterRegistry
};
use async_trait::async_trait;
use std::sync::Arc;

pub struct Commodore1541Formatter {
    tool_path: Option<std::path::PathBuf>,
}

impl Commodore1541Formatter {
    pub fn new() -> Self {
        Self {
            tool_path: None,
        }
    }
    
    /// Check for cc1541 tool or provide installation instructions
    async fn ensure_tool(&mut self) -> Result<std::path::PathBuf, MosesError> {
        if let Some(ref path) = self.tool_path {
            return Ok(path.clone());
        }
        
        // Check for cc1541 tool
        if let Ok(path) = which::which("cc1541") {
            self.tool_path = Some(path.clone());
            return Ok(path);
        }
        
        // Tool not found, provide helpful instructions
        Err(MosesError::ToolNotFound(
            "cc1541 not found. Please install it:\n\
             - Windows: Download from https://github.com/claus/cc1541/releases\n\
             - macOS: brew install cc1541\n\
             - Linux: Build from source at https://github.com/claus/cc1541".to_string()
        ))
    }
    
    /// Validate Commodore DOS filename
    fn validate_label(label: &str) -> Result<(), MosesError> {
        if label.len() > 16 {
            return Err(MosesError::InvalidInput(
                format!("C64 disk labels must be 16 characters or less (got {})", label.len())
            ));
        }
        
        // Check for valid PETSCII characters
        for ch in label.chars() {
            if !ch.is_ascii() {
                return Err(MosesError::InvalidInput(
                    "C64 labels must use ASCII characters only".to_string()
                ));
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl FilesystemFormatter for Commodore1541Formatter {
    fn name(&self) -> &'static str {
        "c64-1541"
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        // cc1541 runs on all major platforms
        vec![Platform::Windows, Platform::Linux, Platform::MacOS]
    }
    
    fn can_format(&self, device: &Device) -> bool {
        // 1541 disks are exactly 174,848 bytes (683 blocks of 256 bytes)
        // In practice, we might be formatting disk images or modern media
        // so we'll accept anything from 170KB to 200KB
        device.size >= 170_000 && device.size <= 200_000
    }
    
    fn requires_external_tools(&self) -> bool {
        true
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        vec![] // Could bundle cc1541 in future releases
    }
    
    async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        // Ensure tool is available
        let mut self_mut = Self::new();
        let tool_path = self_mut.ensure_tool().await?;
        
        // Validate options
        self.validate_options(options).await?;
        
        // Build command
        let mut cmd = tokio::process::Command::new(tool_path);
        
        // cc1541 command format:
        // cc1541 -n "disk name" -i "disk id" -f d64 output.d64
        cmd.arg("-f").arg("d64");
        
        if let Some(label) = &options.label {
            cmd.arg("-n").arg(label);
        } else {
            cmd.arg("-n").arg("MOSES DISK");
        }
        
        // Add disk ID (2 characters)
        cmd.arg("-i").arg("MD");
        
        // Output file (device path)
        cmd.arg(&device.path);
        
        // Execute format command
        let output = cmd.output().await
            .map_err(|e| MosesError::Io(e))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(MosesError::Format(
                format!("Failed to format as C64 1541: {}", stderr)
            ));
        }
        
        Ok(())
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        if let Some(ref label) = options.label {
            Self::validate_label(label)?;
        }
        
        // Check for C64-specific options
        if let Some(disk_id) = options.additional_options.get("disk_id") {
            if disk_id.len() != 2 {
                return Err(MosesError::InvalidInput(
                    "Disk ID must be exactly 2 characters".to_string()
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
        // Validate options first
        self.validate_options(options).await?;
        
        let mut warnings = Vec::new();
        let mut required_tools = vec!["cc1541".to_string()];
        
        // Check if tool is available
        if which::which("cc1541").is_err() {
            warnings.push("cc1541 tool not found - will need to be installed".to_string());
        }
        
        // Check device size
        if device.size != 174_848 {
            warnings.push(format!(
                "Device size ({} bytes) differs from standard 1541 disk (174,848 bytes)",
                device.size
            ));
        }
        
        // Historical note
        warnings.push("This format is for Commodore 64 disk images (D64 format)".to_string());
        
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: std::time::Duration::from_secs(1), // Very fast for 170KB
            warnings,
            required_tools,
            will_erase_data: true,
            space_after_format: 144_896, // Usable space after BAM and directory
        })
    }
}

/// Create metadata for the Commodore 1541 formatter
pub fn create_c64_metadata() -> moses_core::FormatterMetadata {
    FormatterMetadataBuilder::new("c64-1541")
        .description("Commodore 1541 5.25\" disk format (D64)")
        .aliases(vec!["1541", "d64", "commodore", "c64"])
        .category(FormatterCategory::Historical)
        .size_range(Some(170_000), Some(200_000))
        .version("1.0.0")
        .author("Retro Computing Community")
        .capability(|c| {
            c.supports_labels = true;
            c.max_label_length = Some(16);
            c.supports_uuid = false; // No UUID in 1980s!
            c.supports_encryption = false;
            c.supports_compression = false;
            c.supports_resize = false;
            c.max_file_size = Some(168_656); // Max sequential file size
            c.case_sensitive = false; // PETSCII is not case sensitive for filenames
            c.preserves_permissions = false; // No permissions in CBM DOS
        })
        .build()
}

/// Register the Commodore 1541 formatter
pub fn register_c64_formatter(registry: &mut FormatterRegistry) -> Result<(), MosesError> {
    registry.register(
        "c64-1541".to_string(),
        Arc::new(Commodore1541Formatter::new()),
        create_c64_metadata(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use moses_core::test_utils::MockDevice;
    
    #[tokio::test]
    async fn test_c64_formatter() {
        let formatter = Commodore1541Formatter::new();
        
        // Test with correct size device
        let device = MockDevice::new("mock://c64disk", 174_848);
        assert!(formatter.can_format(&device));
        
        // Test with wrong size device
        let large_device = MockDevice::new("mock://large", 1024 * 1024);
        assert!(!formatter.can_format(&large_device));
        
        // Test label validation
        let options = FormatOptions {
            filesystem_type: "c64-1541".to_string(),
            label: Some("GAMES DISK".to_string()),
            ..Default::default()
        };
        assert!(formatter.validate_options(&options).await.is_ok());
        
        // Test invalid label (too long)
        let bad_options = FormatOptions {
            filesystem_type: "c64-1541".to_string(),
            label: Some("THIS LABEL IS WAY TOO LONG FOR C64".to_string()),
            ..Default::default()
        };
        assert!(formatter.validate_options(&bad_options).await.is_err());
        
        // Test dry run
        let report = formatter.dry_run(&device, &options).await.unwrap();
        assert!(!report.warnings.is_empty()); // Should warn about tool or format
        assert_eq!(report.required_tools, vec!["cc1541"]);
    }
    
    #[test]
    fn test_c64_metadata() {
        let metadata = create_c64_metadata();
        
        assert_eq!(metadata.name, "c64-1541");
        assert_eq!(metadata.category, FormatterCategory::Historical);
        assert!(metadata.aliases.contains(&"d64".to_string()));
        assert!(metadata.aliases.contains(&"commodore".to_string()));
        
        // Check capabilities match C64 limitations
        assert_eq!(metadata.capabilities.max_label_length, Some(16));
        assert!(!metadata.capabilities.supports_uuid);
        assert!(!metadata.capabilities.case_sensitive);
        assert!(!metadata.capabilities.preserves_permissions);
    }
    
    #[test]
    fn test_c64_registration() {
        let mut registry = FormatterRegistry::new();
        
        // Register formatter
        register_c64_formatter(&mut registry).unwrap();
        
        // Test that it's registered
        assert!(registry.is_supported("c64-1541"));
        
        // Test aliases work
        assert!(registry.is_supported("1541"));
        assert!(registry.is_supported("d64"));
        assert!(registry.is_supported("commodore"));
        assert!(registry.is_supported("c64"));
        
        // Test metadata retrieval
        let meta = registry.get_metadata("c64-1541").unwrap();
        assert_eq!(meta.category, FormatterCategory::Historical);
        
        // Test category listing
        let historical = registry.list_by_category(FormatterCategory::Historical);
        assert!(historical.iter().any(|(name, _)| *name == "c64-1541"));
    }
}