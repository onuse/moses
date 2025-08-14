use crate::{FilesystemFormatter, Platform, MosesError};
use std::collections::HashMap;
use std::sync::Arc;

/// Central registry for all filesystem formatters
pub struct FormatterRegistry {
    formatters: HashMap<String, Arc<dyn FilesystemFormatter>>,
    metadata: HashMap<String, FormatterMetadata>,
    aliases: HashMap<String, String>, // alias -> canonical name
}

impl FormatterRegistry {
    pub fn new() -> Self {
        Self {
            formatters: HashMap::new(),
            metadata: HashMap::new(),
            aliases: HashMap::new(),
        }
    }
    
    /// Register a formatter with its metadata
    pub fn register(
        &mut self, 
        name: String, 
        formatter: Arc<dyn FilesystemFormatter>,
        metadata: FormatterMetadata,
    ) -> Result<(), MosesError> {
        // Check for duplicate names
        if self.formatters.contains_key(&name) {
            return Err(MosesError::Configuration(
                format!("Formatter '{}' is already registered", name)
            ));
        }

        // Register aliases
        for alias in &metadata.aliases {
            if self.aliases.contains_key(alias) {
                return Err(MosesError::Configuration(
                    format!("Alias '{}' is already in use", alias)
                ));
            }
            self.aliases.insert(alias.clone(), name.clone());
        }

        // Store formatter and metadata
        self.formatters.insert(name.clone(), formatter);
        self.metadata.insert(name, metadata);
        
        Ok(())
    }
    
    /// Get a formatter by name or alias
    pub fn get_formatter(&self, name: &str) -> Option<Arc<dyn FilesystemFormatter>> {
        let canonical_name = self.aliases.get(name)
            .map(|s| s.as_str())
            .unwrap_or(name);
        self.formatters.get(canonical_name).cloned()
    }
    
    /// Get metadata for a formatter
    pub fn get_metadata(&self, name: &str) -> Option<&FormatterMetadata> {
        let canonical_name = self.aliases.get(name)
            .map(|s| s.as_str())
            .unwrap_or(name);
        self.metadata.get(canonical_name)
    }
    
    /// List all registered formatters
    pub fn list_formatters(&self) -> Vec<String> {
        self.formatters.keys().cloned().collect()
    }
    
    /// List all formatters with metadata
    pub fn list_with_metadata(&self) -> Vec<(&str, &FormatterMetadata)> {
        self.metadata
            .iter()
            .map(|(name, meta)| (name.as_str(), meta))
            .collect()
    }
    
    /// List formatters by category
    pub fn list_by_category(&self, category: FormatterCategory) -> Vec<(&str, &FormatterMetadata)> {
        self.metadata
            .iter()
            .filter(|(_, meta)| meta.category == category)
            .map(|(name, meta)| (name.as_str(), meta))
            .collect()
    }
    
    /// Check if a formatter is supported
    pub fn is_supported(&self, filesystem: &str) -> bool {
        let canonical_name = self.aliases.get(filesystem)
            .map(|s| s.as_str())
            .unwrap_or(filesystem);
        self.formatters.contains_key(canonical_name)
    }
    
    /// Find formatters that support a specific platform
    pub fn find_by_platform(&self, platform: Platform) -> Vec<(&str, &FormatterMetadata)> {
        self.metadata
            .iter()
            .filter(|(name, _meta)| {
                if let Some(formatter) = self.formatters.get(*name) {
                    formatter.supported_platforms().contains(&platform)
                } else {
                    false
                }
            })
            .map(|(name, meta)| (name.as_str(), meta))
            .collect()
    }
}

impl Default for FormatterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Metadata about a formatter
#[derive(Clone, Debug)]
pub struct FormatterMetadata {
    pub name: String,
    pub description: String,
    pub aliases: Vec<String>,
    pub category: FormatterCategory,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub platform_support: Vec<Platform>,
    pub required_tools: Vec<String>,
    pub documentation_url: Option<String>,
    pub version: String,
    pub author: String,
    pub capabilities: FormatterCapabilities,
}

impl Default for FormatterMetadata {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            aliases: Vec::new(),
            category: FormatterCategory::Modern,
            min_size: None,
            max_size: None,
            platform_support: Vec::new(),
            required_tools: Vec::new(),
            documentation_url: None,
            version: "1.0.0".to_string(),
            author: "Moses Team".to_string(),
            capabilities: FormatterCapabilities::default(),
        }
    }
}

/// Categories for organizing formatters
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatterCategory {
    Modern,          // ext4, btrfs, zfs
    Legacy,          // fat32, ntfs
    Historical,      // Commodore, Amiga
    Console,         // PlayStation, Xbox
    Embedded,        // YAFFS, UBIFS
    Experimental,    // Research filesystems
}

/// Capabilities of a formatter
#[derive(Clone, Debug, Default)]
pub struct FormatterCapabilities {
    pub supports_labels: bool,
    pub max_label_length: Option<usize>,
    pub supports_uuid: bool,
    pub supports_encryption: bool,
    pub supports_compression: bool,
    pub supports_resize: bool,
    pub max_file_size: Option<u64>,
    pub case_sensitive: bool,
    pub preserves_permissions: bool,
}

/// Builder for creating FormatterMetadata
pub struct FormatterMetadataBuilder {
    metadata: FormatterMetadata,
}

impl FormatterMetadataBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            metadata: FormatterMetadata {
                name: name.to_string(),
                ..Default::default()
            },
        }
    }

    pub fn description(mut self, desc: &str) -> Self {
        self.metadata.description = desc.to_string();
        self
    }

    pub fn aliases(mut self, aliases: Vec<&str>) -> Self {
        self.metadata.aliases = aliases.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn category(mut self, category: FormatterCategory) -> Self {
        self.metadata.category = category;
        self
    }

    pub fn size_range(mut self, min: Option<u64>, max: Option<u64>) -> Self {
        self.metadata.min_size = min;
        self.metadata.max_size = max;
        self
    }

    pub fn platforms(mut self, platforms: Vec<Platform>) -> Self {
        self.metadata.platform_support = platforms;
        self
    }

    pub fn version(mut self, version: &str) -> Self {
        self.metadata.version = version.to_string();
        self
    }

    pub fn author(mut self, author: &str) -> Self {
        self.metadata.author = author.to_string();
        self
    }

    pub fn capability(mut self, f: impl FnOnce(&mut FormatterCapabilities)) -> Self {
        f(&mut self.metadata.capabilities);
        self
    }

    pub fn build(self) -> FormatterMetadata {
        self.metadata
    }
}