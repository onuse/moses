// Main ext4 formatter implementation
// Phase 1: Writing a valid superblock

use moses_core::{Device, FilesystemFormatter, FormatOptions, MosesError, Platform, SimulationReport};
use crate::ext4_native::core::{
    structures::Ext4Superblock,
    types::{FilesystemParams, FilesystemLayout},
    alignment::AlignedBuffer,
};
use std::fs::File;
use std::io::Write;

pub struct Ext4NativeFormatter;

#[async_trait::async_trait]
impl FilesystemFormatter for Ext4NativeFormatter {
    fn name(&self) -> &'static str {
        "ext4-native"
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        vec![Platform::Windows]
    }
    
    fn can_format(&self, device: &Device) -> bool {
        // Only format removable devices for safety
        !device.is_system && device.is_removable
    }
    
    fn requires_external_tools(&self) -> bool {
        false
    }
    
    fn bundled_tools(&self) -> Vec<&'static str> {
        vec![]
    }
    
    async fn format(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<(), MosesError> {
        // Phase 1: Write a valid superblock
        
        // Convert options to filesystem parameters
        let params = FilesystemParams {
            size_bytes: device.size,
            block_size: options.cluster_size.unwrap_or(4096) as u32,
            inode_size: 256,
            label: options.label.clone(),
            reserved_percent: 5,
            enable_checksums: true,
            enable_64bit: device.size > 16 * 1024 * 1024 * 1024, // >16GB
            enable_journal: false, // Phase 1: No journal yet
        };
        
        // Calculate filesystem layout
        let layout = FilesystemLayout::from_params(&params)
            .map_err(|e| MosesError::Other(e.to_string()))?;
        
        // Create and initialize superblock
        let mut superblock = Ext4Superblock::new();
        superblock.init_minimal(&params, &layout);
        superblock.update_checksum();
        
        // Validate the superblock before writing
        superblock.validate()
            .map_err(|e| MosesError::Other(format!("Superblock validation failed: {}", e)))?;
        
        // For Phase 1: Write to a test image file instead of actual device
        // This allows us to safely test with dumpe2fs
        let test_path = format!("{}_phase1.img", device.id);
        
        // Create minimal image (just first 8KB containing superblock)
        let mut buffer = AlignedBuffer::<8192>::new();
        
        // Write superblock at offset 1024
        superblock.write_to_buffer(&mut buffer[1024..2048])
            .map_err(|e| MosesError::Other(format!("Failed to serialize superblock: {}", e)))?;
        
        // Write to file
        let mut file = File::create(&test_path)
            .map_err(|e| MosesError::Other(format!("Failed to create image: {}", e)))?;
        
        file.write_all(&buffer[..])
            .map_err(|e| MosesError::Other(format!("Failed to write image: {}", e)))?;
        
        file.sync_all()
            .map_err(|e| MosesError::Other(format!("Failed to sync image: {}", e)))?;
        
        println!("Phase 1: Superblock written to {}", test_path);
        println!("Validate with: dumpe2fs {} 2>/dev/null | head -50", test_path);
        
        Ok(())
    }
    
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        if let Some(ref label) = options.label {
            if label.len() > 16 {
                return Err(MosesError::Other("Label must be 16 characters or less".to_string()));
            }
        }
        Ok(())
    }
    
    async fn dry_run(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<SimulationReport, MosesError> {
        Ok(SimulationReport {
            device: device.clone(),
            options: options.clone(),
            estimated_time: std::time::Duration::from_secs(5),
            warnings: vec!["Native ext4 formatter - Phase 0".to_string()],
            required_tools: vec![],
            will_erase_data: true,
            space_after_format: device.size * 95 / 100,
        })
    }
}