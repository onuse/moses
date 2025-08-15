use moses_core::{
    FormatterRegistry, FormatterMetadataBuilder, FormatterCategory, Platform,
    FilesystemFormatter,
};
use std::sync::Arc;

// Import all our formatters
#[cfg(not(target_os = "windows"))]
use crate::ntfs::NtfsFormatter;
use crate::fat32::Fat32Formatter;
use crate::exfat::ExFatFormatter;

// Import Ext4Formatter for non-Windows/Linux platforms (e.g., macOS)
#[cfg(not(any(target_os = "windows", target_os = "linux")))]
use crate::ext4::Ext4Formatter;

#[cfg(target_os = "windows")]
use crate::ext4_windows::Ext4WindowsFormatter;
#[cfg(target_os = "windows")]
use crate::ntfs_windows::NtfsWindowsFormatter;
#[cfg(target_os = "linux")]
use crate::ext4_linux::Ext4LinuxFormatter;

/// Register all built-in formatters with their metadata
/// This serves as an example of how to properly register formatters
pub fn register_builtin_formatters(registry: &mut FormatterRegistry) -> Result<(), moses_core::MosesError> {
    // EXT4 - Modern Linux filesystem
    #[cfg(target_os = "windows")]
    {
        registry.register(
            "ext4".to_string(),
            Arc::new(Ext4WindowsFormatter) as Arc<dyn FilesystemFormatter>,
            FormatterMetadataBuilder::new("ext4")
                .description("Fourth Extended Filesystem - Primary Linux filesystem")
                .aliases(vec!["ext", "linux"])
                .category(FormatterCategory::Modern)
                .size_range(Some(16 * 1024 * 1024), None) // 16MB minimum
                .version("1.0.0")
                .author("Moses Team")
                .capability(|c| {
                    c.supports_labels = true;
                    c.max_label_length = Some(16);
                    c.supports_uuid = true;
                    c.supports_encryption = false; // Can be added with LUKS
                    c.supports_compression = false;
                    c.supports_resize = true;
                    c.max_file_size = Some(16 * 1024_u64.pow(4)); // 16TB
                    c.case_sensitive = true;
                    c.preserves_permissions = true;
                })
                .build()
        )?;
    }
    
    #[cfg(target_os = "linux")]
    {
        registry.register(
            "ext4".to_string(),
            Arc::new(Ext4LinuxFormatter) as Arc<dyn FilesystemFormatter>,
            FormatterMetadataBuilder::new("ext4")
                .description("Fourth Extended Filesystem - Primary Linux filesystem")
                .aliases(vec!["ext", "linux"])
                .category(FormatterCategory::Modern)
                .size_range(Some(16 * 1024 * 1024), None)
                .version("1.0.0")
                .author("Moses Team")
                .capability(|c| {
                    c.supports_labels = true;
                    c.max_label_length = Some(16);
                    c.supports_uuid = true;
                    c.supports_encryption = false;
                    c.supports_compression = false;
                    c.supports_resize = true;
                    c.max_file_size = Some(16 * 1024_u64.pow(4));
                    c.case_sensitive = true;
                    c.preserves_permissions = true;
                })
                .build()
        )?;
    }
    
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        registry.register(
            "ext4".to_string(),
            Arc::new(Ext4Formatter) as Arc<dyn FilesystemFormatter>,
            FormatterMetadataBuilder::new("ext4")
                .description("Fourth Extended Filesystem - Primary Linux filesystem")
                .aliases(vec!["ext", "linux"])
                .category(FormatterCategory::Modern)
                .size_range(Some(16 * 1024 * 1024), None)
                .version("1.0.0")
                .author("Moses Team")
                .capability(|c| {
                    c.supports_labels = true;
                    c.max_label_length = Some(16);
                    c.supports_uuid = true;
                    c.supports_encryption = false;
                    c.supports_compression = false;
                    c.supports_resize = true;
                    c.max_file_size = Some(16 * 1024_u64.pow(4));
                    c.case_sensitive = true;
                    c.preserves_permissions = true;
                })
                .build()
        )?;
    }

    // NTFS - Windows filesystem
    #[cfg(target_os = "windows")]
    {
        registry.register(
            "ntfs".to_string(),
            Arc::new(NtfsWindowsFormatter) as Arc<dyn FilesystemFormatter>,
            FormatterMetadataBuilder::new("ntfs")
                .description("New Technology File System - Primary Windows filesystem")
                .aliases(vec!["windows", "nt"])
                .category(FormatterCategory::Legacy)
                .size_range(Some(1024 * 1024), None) // 1MB minimum
                .version("1.0.0")
                .author("Moses Team")
                .capability(|c| {
                    c.supports_labels = true;
                    c.max_label_length = Some(32);
                    c.supports_uuid = true;
                    c.supports_encryption = true;
                    c.supports_compression = true;
                    c.supports_resize = true;
                    c.max_file_size = Some(16 * 1024_u64.pow(4)); // 16TB
                    c.case_sensitive = false; // Can be enabled but not default
                    c.preserves_permissions = true;
                })
                .build()
        )?;
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        registry.register(
            "ntfs".to_string(),
            Arc::new(NtfsFormatter) as Arc<dyn FilesystemFormatter>,
            FormatterMetadataBuilder::new("ntfs")
                .description("New Technology File System - Primary Windows filesystem")
                .aliases(vec!["windows", "nt"])
                .category(FormatterCategory::Legacy)
                .size_range(Some(1024 * 1024), None)
                .version("1.0.0")
                .author("Moses Team")
                .capability(|c| {
                    c.supports_labels = true;
                    c.max_label_length = Some(32);
                    c.supports_uuid = true;
                    c.supports_encryption = true;
                    c.supports_compression = true;
                    c.supports_resize = true;
                    c.max_file_size = Some(16 * 1024_u64.pow(4));
                    c.case_sensitive = false;
                    c.preserves_permissions = true;
                })
                .build()
        )?;
    }

    // FAT32 - Universal legacy filesystem
    registry.register(
        "fat32".to_string(),
        Arc::new(Fat32Formatter) as Arc<dyn FilesystemFormatter>,
        FormatterMetadataBuilder::new("fat32")
            .description("File Allocation Table 32 - Universal compatibility filesystem")
            .aliases(vec!["fat", "msdos", "vfat"])
            .category(FormatterCategory::Legacy)
            .size_range(Some(32 * 1024 * 1024), Some(2 * 1024_u64.pow(4))) // 32MB to 2TB
            .version("1.0.0")
            .author("Moses Team")
            .capability(|c| {
                c.supports_labels = true;
                c.max_label_length = Some(11);
                c.supports_uuid = false;
                c.supports_encryption = false;
                c.supports_compression = false;
                c.supports_resize = false;
                c.max_file_size = Some(4 * 1024_u64.pow(3) - 1); // 4GB - 1 byte
                c.case_sensitive = false;
                c.preserves_permissions = false;
            })
            .build()
    )?;

    // exFAT - Modern universal filesystem
    registry.register(
        "exfat".to_string(),
        Arc::new(ExFatFormatter) as Arc<dyn FilesystemFormatter>,
        FormatterMetadataBuilder::new("exfat")
            .description("Extended FAT - Modern universal filesystem for large drives")
            .aliases(vec!["exf", "sdxc"])
            .category(FormatterCategory::Modern)
            .size_range(Some(7 * 1024 * 1024), Some(128 * 1024_u64.pow(4))) // 7MB to 128TB
            .version("1.0.0")
            .author("Moses Team")
            .capability(|c| {
                c.supports_labels = true;
                c.max_label_length = Some(15);
                c.supports_uuid = true;
                c.supports_encryption = false;
                c.supports_compression = false;
                c.supports_resize = false;
                c.max_file_size = Some(16 * 1024_u64.pow(5) - 1); // 16EB - 1 byte
                c.case_sensitive = false;
                c.preserves_permissions = false;
            })
            .build()
    )?;

    Ok(())
}

/// Get a list of all available formatters for the current platform
pub fn list_available_formatters(registry: &FormatterRegistry) -> Vec<String> {
    let current_platform = Platform::current();
    
    registry.find_by_platform(current_platform)
        .into_iter()
        .map(|(name, _)| name.to_string())
        .collect()
}

/// Get detailed information about a specific formatter
pub fn get_formatter_info(registry: &FormatterRegistry, name: &str) -> Option<String> {
    registry.get_metadata(name).map(|meta| {
        format!(
            "Formatter: {}\n\
             Description: {}\n\
             Aliases: {:?}\n\
             Category: {:?}\n\
             Version: {}\n\
             Author: {}\n\
             Min Size: {}\n\
             Max Size: {}\n\
             Capabilities:\n\
             - Supports Labels: {}\n\
             - Max Label Length: {:?}\n\
             - Supports UUID: {}\n\
             - Supports Encryption: {}\n\
             - Supports Compression: {}\n\
             - Case Sensitive: {}\n\
             - Preserves Permissions: {}\n\
             - Max File Size: {}",
            meta.name,
            meta.description,
            meta.aliases,
            meta.category,
            meta.version,
            meta.author,
            meta.min_size.map_or("None".to_string(), |s| format!("{} bytes", s)),
            meta.max_size.map_or("None".to_string(), |s| format!("{} bytes", s)),
            meta.capabilities.supports_labels,
            meta.capabilities.max_label_length,
            meta.capabilities.supports_uuid,
            meta.capabilities.supports_encryption,
            meta.capabilities.supports_compression,
            meta.capabilities.case_sensitive,
            meta.capabilities.preserves_permissions,
            meta.capabilities.max_file_size.map_or("No limit".to_string(), |s| format!("{} bytes", s))
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formatter_registration() {
        let mut registry = FormatterRegistry::new();
        assert!(register_builtin_formatters(&mut registry).is_ok());
        
        // Test that formatters are registered
        assert!(registry.is_supported("ext4"));
        assert!(registry.is_supported("ntfs"));
        assert!(registry.is_supported("fat32"));
        assert!(registry.is_supported("exfat"));
        
        // Test aliases work
        assert!(registry.is_supported("fat"));
        assert!(registry.is_supported("msdos"));
        assert!(registry.is_supported("windows"));
        assert!(registry.is_supported("linux"));
    }
    
    #[test]
    fn test_formatter_metadata() {
        let mut registry = FormatterRegistry::new();
        register_builtin_formatters(&mut registry).unwrap();
        
        // Test FAT32 metadata
        let fat32_meta = registry.get_metadata("fat32").unwrap();
        assert_eq!(fat32_meta.category, FormatterCategory::Legacy);
        assert_eq!(fat32_meta.capabilities.max_file_size, Some(4 * 1024_u64.pow(3) - 1));
        assert!(!fat32_meta.capabilities.case_sensitive);
        
        // Test ext4 metadata
        let ext4_meta = registry.get_metadata("ext4").unwrap();
        assert_eq!(ext4_meta.category, FormatterCategory::Modern);
        assert!(ext4_meta.capabilities.case_sensitive);
        assert!(ext4_meta.capabilities.preserves_permissions);
    }
    
    #[test]
    fn test_list_by_category() {
        let mut registry = FormatterRegistry::new();
        register_builtin_formatters(&mut registry).unwrap();
        
        let modern = registry.list_by_category(FormatterCategory::Modern);
        assert!(modern.iter().any(|(name, _)| *name == "ext4" || *name == "exfat"));
        
        let legacy = registry.list_by_category(FormatterCategory::Legacy);
        assert!(legacy.iter().any(|(name, _)| *name == "fat32" || *name == "ntfs"));
    }
}