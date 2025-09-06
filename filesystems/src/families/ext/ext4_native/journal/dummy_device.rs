// Dummy journal device for testing
// This provides a simple in-memory journal device implementation

use super::jbd2::JournalDevice;
use moses_core::{Device, MosesError};
use std::collections::HashMap;

/// Dummy journal device for development
pub struct DummyJournalDevice {
    device: Device,
    blocks: HashMap<u64, Vec<u8>>,
    block_size: usize,
}

impl DummyJournalDevice {
    pub fn new(device: Device) -> Self {
        Self {
            device,
            blocks: HashMap::new(),
            block_size: 4096,
        }
    }
}

impl JournalDevice for DummyJournalDevice {
    fn read_block(&mut self, block: u64) -> Result<Vec<u8>, MosesError> {
        if let Some(data) = self.blocks.get(&block) {
            Ok(data.clone())
        } else {
            // Return zeros for uninitialized blocks
            Ok(vec![0u8; self.block_size])
        }
    }
    
    fn write_block(&mut self, block: u64, data: &[u8]) -> Result<(), MosesError> {
        if data.len() != self.block_size {
            return Err(MosesError::Other(format!(
                "Invalid block size: expected {}, got {}",
                self.block_size,
                data.len()
            )));
        }
        
        self.blocks.insert(block, data.to_vec());
        Ok(())
    }
    
    fn sync(&mut self) -> Result<(), MosesError> {
        // No-op for in-memory device
        Ok(())
    }
}