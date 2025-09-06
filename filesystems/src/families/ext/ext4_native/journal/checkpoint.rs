// Checkpoint Management for EXT4 Journaling
// Handles writing committed journal data to final disk locations

use moses_core::MosesError;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Checkpoint manager
pub struct Checkpoint {
    /// List of transactions waiting for checkpoint
    pending: Arc<Mutex<VecDeque<CheckpointTransaction>>>,
    /// Maximum transactions before forced checkpoint
    max_pending: usize,
    /// Checkpoint in progress flag
    in_progress: Arc<Mutex<bool>>,
}

/// Transaction waiting for checkpoint
struct CheckpointTransaction {
    /// Transaction ID
    tid: u64,
    /// Blocks to write
    blocks: Vec<CheckpointBlock>,
    /// Time committed
    commit_time: std::time::Instant,
}

/// Block to checkpoint
struct CheckpointBlock {
    /// Destination block number
    block_num: u64,
    /// Data to write
    data: Vec<u8>,
}

impl Checkpoint {
    /// Create new checkpoint manager
    pub fn new(max_pending: usize) -> Self {
        Self {
            pending: Arc::new(Mutex::new(VecDeque::new())),
            max_pending,
            in_progress: Arc::new(Mutex::new(false)),
        }
    }
    
    /// Add transaction for checkpointing
    pub fn add_transaction(&self, tid: u64, blocks: Vec<(u64, Vec<u8>)>) -> Result<(), MosesError> {
        let mut pending = self.pending.lock().unwrap();
        
        let checkpoint_blocks = blocks.into_iter()
            .map(|(num, data)| CheckpointBlock {
                block_num: num,
                data,
            })
            .collect();
        
        pending.push_back(CheckpointTransaction {
            tid,
            blocks: checkpoint_blocks,
            commit_time: std::time::Instant::now(),
        });
        
        // Force checkpoint if too many pending
        if pending.len() >= self.max_pending {
            // Just log a warning for now - actual checkpoint needs device access
            log::warn!("Checkpoint needed: {} transactions pending", pending.len());
        }
        
        Ok(())
    }
    
    /// Perform checkpoint operation
    pub fn checkpoint(&self, device: &mut dyn super::jbd2::JournalDevice) -> Result<CheckpointStats, MosesError> {
        let mut in_progress = self.in_progress.lock().unwrap();
        if *in_progress {
            return Err(MosesError::Other("Checkpoint already in progress".to_string()));
        }
        *in_progress = true;
        drop(in_progress);
        
        let mut stats = CheckpointStats::default();
        let mut pending = self.pending.lock().unwrap();
        
        while let Some(trans) = pending.pop_front() {
            stats.transactions_checkpointed += 1;
            
            for block in &trans.blocks {
                device.write_block(block.block_num, &block.data)?;
                stats.blocks_written += 1;
            }
        }
        
        device.sync()?;
        
        let mut in_progress = self.in_progress.lock().unwrap();
        *in_progress = false;
        
        Ok(stats)
    }
    
    /// Force checkpoint of all pending transactions
    pub fn force_checkpoint(&self, device: &mut dyn super::jbd2::JournalDevice) -> Result<(), MosesError> {
        let mut in_progress = self.in_progress.lock().unwrap();
        if *in_progress {
            return Err(MosesError::Other("Checkpoint already in progress".to_string()));
        }
        *in_progress = true;
        drop(in_progress);
        
        let mut pending = self.pending.lock().unwrap();
        let mut total_written = 0u64;
        
        // Write all pending transactions to their final locations
        while let Some(trans) = pending.pop_front() {
            for block in &trans.blocks {
                device.write_block(block.block_num, &block.data)?;
                total_written += 1;
            }
        }
        
        // Sync device to ensure all writes are persisted
        if total_written > 0 {
            device.sync()?;
            log::debug!("Force checkpoint complete: {} blocks written", total_written);
        }
        
        let mut in_progress = self.in_progress.lock().unwrap();
        *in_progress = false;
        
        Ok(())
    }
    
    /// Get number of pending transactions
    pub fn pending_count(&self) -> usize {
        let pending = self.pending.lock().unwrap();
        pending.len()
    }
    
    /// Check if checkpoint is needed
    pub fn needs_checkpoint(&self) -> bool {
        let pending = self.pending.lock().unwrap();
        
        // Check various conditions
        if pending.len() >= self.max_pending {
            return true;
        }
        
        // Check age of oldest transaction
        if let Some(oldest) = pending.front() {
            if oldest.commit_time.elapsed().as_secs() > 30 {
                return true;
            }
        }
        
        false
    }
}

/// Checkpoint statistics
#[derive(Debug, Default)]
pub struct CheckpointStats {
    /// Number of transactions checkpointed
    pub transactions_checkpointed: u64,
    /// Number of blocks written
    pub blocks_written: u64,
    /// Time taken for checkpoint
    pub checkpoint_time_ms: u64,
}