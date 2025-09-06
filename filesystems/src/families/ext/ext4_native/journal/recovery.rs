// Journal Recovery for EXT4
// Handles recovery after system crashes

use moses_core::MosesError;
use super::jbd2::{JournalSuperblock, JournalHeader, JournalBlockTag, JournalDevice};
use std::collections::HashMap;

/// Dummy device for simplified recovery
struct DummyDevice;

impl JournalDevice for DummyDevice {
    fn read_block(&mut self, _block: u64) -> Result<Vec<u8>, MosesError> {
        Err(MosesError::Other("Dummy device".to_string()))
    }
    
    fn write_block(&mut self, _block: u64, _data: &[u8]) -> Result<(), MosesError> {
        Err(MosesError::Other("Dummy device".to_string()))
    }
    
    fn sync(&mut self) -> Result<(), MosesError> {
        Ok(())
    }
}

const JBD2_MAGIC_NUMBER: u32 = 0xC03B3998;
const JBD2_DESCRIPTOR_BLOCK: u32 = 1;
const JBD2_COMMIT_BLOCK: u32 = 2;
const JBD2_REVOKE_BLOCK: u32 = 5;

/// Journal recovery handler
pub struct JournalRecovery {
    /// Journal device
    device: Box<dyn JournalDevice>,
    /// Journal superblock
    superblock: JournalSuperblock,
    /// Revoked blocks
    revoked: HashMap<u64, u64>,
}

/// Recovery pass types
#[derive(Debug, Clone, Copy)]
enum RecoveryPass {
    /// Scan for the end of the journal
    Scan,
    /// Replay transactions
    Replay,
    /// Revoke blocks
    Revoke,
}

/// Transaction info during recovery
struct RecoveryTransaction {
    /// Transaction ID
    tid: u64,
    /// Start block in journal
    start_block: u32,
    /// End block in journal
    end_block: u32,
    /// Blocks to replay
    blocks: Vec<RecoveryBlock>,
    /// Is this transaction complete?
    complete: bool,
}

/// Block info during recovery
struct RecoveryBlock {
    /// Destination block number
    dest_block: u64,
    /// Journal block number
    journal_block: u32,
    /// Flags
    flags: u32,
}

impl JournalRecovery {
    /// Create a new recovery handler
    pub fn new(mut device: Box<dyn JournalDevice>) -> Result<Self, MosesError> {
        // Read superblock
        let sb_data = device.read_block(0)?;
        if sb_data.len() < std::mem::size_of::<JournalSuperblock>() {
            return Err(MosesError::Other("Invalid journal superblock".to_string()));
        }
        
        let superblock = unsafe {
            std::ptr::read_unaligned(sb_data.as_ptr() as *const JournalSuperblock)
        };
        
        // Verify magic
        if superblock.s_header.h_magic != JBD2_MAGIC_NUMBER {
            return Err(MosesError::Other("Invalid journal magic".to_string()));
        }
        
        Ok(Self {
            device,
            superblock,
            revoked: HashMap::new(),
        })
    }
    
    /// Perform journal recovery
    pub fn recover(&mut self) -> Result<RecoveryStats, MosesError> {
        let mut stats = RecoveryStats::default();
        
        // Pass 1: Scan for journal end
        let (start_tid, end_tid) = self.scan_journal()?;
        stats.transactions_found = end_tid - start_tid;
        
        if stats.transactions_found == 0 {
            return Ok(stats);
        }
        
        // Pass 2: Build revoke table
        self.build_revoke_table(start_tid, end_tid)?;
        stats.blocks_revoked = self.revoked.len() as u64;
        
        // Pass 3: Replay transactions
        let replayed = self.replay_transactions(start_tid, end_tid)?;
        stats.transactions_replayed = replayed;
        
        // Update superblock
        self.update_superblock(end_tid)?;
        
        Ok(stats)
    }
    
    /// Scan journal to find valid transaction range
    fn scan_journal(&mut self) -> Result<(u64, u64), MosesError> {
        let mut current_block = self.superblock.s_start;
        let mut start_tid = 0u64;
        let mut end_tid = 0u64;
        let mut found_start = false;
        
        // Scan entire journal
        for _ in 0..self.superblock.s_maxlen {
            let block_data = self.device.read_block(current_block as u64)?;
            
            if block_data.len() < std::mem::size_of::<JournalHeader>() {
                break;
            }
            
            let header = unsafe {
                std::ptr::read_unaligned(block_data.as_ptr() as *const JournalHeader)
            };
            
            if header.h_magic != JBD2_MAGIC_NUMBER {
                // Not a valid journal block
                if found_start {
                    // We've reached the end of valid journal entries
                    break;
                }
            } else {
                let tid = header.h_sequence as u64;
                
                if !found_start {
                    start_tid = tid;
                    found_start = true;
                }
                
                end_tid = tid;
                
                // Skip to next block based on type
                match header.h_blocktype {
                    JBD2_DESCRIPTOR_BLOCK => {
                        // Count data blocks that follow
                        let tags = self.parse_descriptor_block(&block_data)?;
                        current_block = (current_block + 1 + tags.len() as u32) % self.superblock.s_maxlen;
                        continue;
                    }
                    JBD2_COMMIT_BLOCK => {
                        // Transaction complete
                    }
                    JBD2_REVOKE_BLOCK => {
                        // Revoke block
                    }
                    _ => {}
                }
            }
            
            current_block = (current_block + 1) % self.superblock.s_maxlen;
        }
        
        Ok((start_tid, end_tid))
    }
    
    /// Build revoke table from journal
    fn build_revoke_table(&mut self, start_tid: u64, end_tid: u64) -> Result<(), MosesError> {
        let mut current_block = self.superblock.s_start;
        
        for _ in 0..self.superblock.s_maxlen {
            let block_data = self.device.read_block(current_block as u64)?;
            
            if block_data.len() < std::mem::size_of::<JournalHeader>() {
                break;
            }
            
            let header = unsafe {
                std::ptr::read_unaligned(block_data.as_ptr() as *const JournalHeader)
            };
            
            if header.h_magic != JBD2_MAGIC_NUMBER {
                break;
            }
            
            let tid = header.h_sequence as u64;
            if tid < start_tid || tid > end_tid {
                break;
            }
            
            if header.h_blocktype == JBD2_REVOKE_BLOCK {
                // Parse revoke entries
                let count_offset = std::mem::size_of::<JournalHeader>();
                if block_data.len() >= count_offset + 4 {
                    let count = u32::from_le_bytes([
                        block_data[count_offset],
                        block_data[count_offset + 1],
                        block_data[count_offset + 2],
                        block_data[count_offset + 3],
                    ]);
                    
                    let entry_size = 8usize; // 64-bit block numbers
                    let entries_offset = count_offset + 4;
                    
                    for i in 0..count as usize {
                        let offset = entries_offset + i * entry_size;
                        if offset + entry_size <= block_data.len() {
                            let block_num = u64::from_le_bytes([
                                block_data[offset],
                                block_data[offset + 1],
                                block_data[offset + 2],
                                block_data[offset + 3],
                                block_data[offset + 4],
                                block_data[offset + 5],
                                block_data[offset + 6],
                                block_data[offset + 7],
                            ]);
                            
                            self.revoked.insert(block_num, tid);
                        }
                    }
                }
            }
            
            current_block = (current_block + 1) % self.superblock.s_maxlen;
        }
        
        Ok(())
    }
    
    /// Replay committed transactions
    fn replay_transactions(&mut self, start_tid: u64, end_tid: u64) -> Result<u64, MosesError> {
        let mut replayed = 0u64;
        let mut current_block = self.superblock.s_start;
        let mut current_trans: Option<RecoveryTransaction> = None;
        
        for _ in 0..self.superblock.s_maxlen {
            let block_data = self.device.read_block(current_block as u64)?;
            
            if block_data.len() < std::mem::size_of::<JournalHeader>() {
                break;
            }
            
            let header = unsafe {
                std::ptr::read_unaligned(block_data.as_ptr() as *const JournalHeader)
            };
            
            if header.h_magic != JBD2_MAGIC_NUMBER {
                break;
            }
            
            let tid = header.h_sequence as u64;
            if tid < start_tid || tid > end_tid {
                break;
            }
            
            match header.h_blocktype {
                JBD2_DESCRIPTOR_BLOCK => {
                    // Start new transaction
                    let tags = self.parse_descriptor_block(&block_data)?;
                    let mut blocks = Vec::new();
                    
                    for (i, tag) in tags.iter().enumerate() {
                        let dest_block = tag.t_blocknr as u64 | ((tag.t_blocknr_high as u64) << 32);
                        
                        // Check if block is revoked
                        if let Some(&revoke_tid) = self.revoked.get(&dest_block) {
                            if revoke_tid >= tid {
                                continue; // Skip revoked block
                            }
                        }
                        
                        blocks.push(RecoveryBlock {
                            dest_block,
                            journal_block: current_block + 1 + i as u32,
                            flags: tag.t_flags,
                        });
                    }
                    
                    current_trans = Some(RecoveryTransaction {
                        tid,
                        start_block: current_block,
                        end_block: current_block + 1 + tags.len() as u32,
                        blocks,
                        complete: false,
                    });
                    
                    current_block = (current_block + 1 + tags.len() as u32) % self.superblock.s_maxlen;
                    continue;
                }
                JBD2_COMMIT_BLOCK => {
                    // Transaction is complete, replay it
                    if let Some(trans) = current_trans.take() {
                        if trans.tid == tid {
                            // Replay blocks
                            for block in &trans.blocks {
                                let data = self.device.read_block(block.journal_block as u64)?;
                                self.device.write_block(block.dest_block, &data)?;
                            }
                            
                            replayed += 1;
                        }
                    }
                }
                _ => {}
            }
            
            current_block = (current_block + 1) % self.superblock.s_maxlen;
        }
        
        // Sync device
        self.device.sync()?;
        
        Ok(replayed)
    }
    
    /// Parse descriptor block to get tags
    fn parse_descriptor_block(&self, block_data: &[u8]) -> Result<Vec<JournalBlockTag>, MosesError> {
        let mut tags = Vec::new();
        let header_size = std::mem::size_of::<JournalHeader>();
        let tag_size = std::mem::size_of::<JournalBlockTag>();
        
        let mut offset = header_size;
        while offset + tag_size <= block_data.len() {
            let tag = unsafe {
                std::ptr::read_unaligned(
                    block_data.as_ptr().add(offset) as *const JournalBlockTag
                )
            };
            
            tags.push(tag);
            
            // Check for last tag
            if tag.t_flags & 8 != 0 { // JBD2_FLAG_LAST
                break;
            }
            
            offset += tag_size;
        }
        
        Ok(tags)
    }
    
    /// Update journal superblock after recovery
    fn update_superblock(&mut self, end_tid: u64) -> Result<(), MosesError> {
        self.superblock.s_sequence = end_tid as u32;
        self.superblock.s_start = 0; // Reset to beginning
        self.superblock.s_errno = 0; // Clear any errors
        
        // Write updated superblock
        let sb_bytes = unsafe {
            std::slice::from_raw_parts(
                &self.superblock as *const _ as *const u8,
                std::mem::size_of::<JournalSuperblock>()
            )
        };
        
        self.device.write_block(0, sb_bytes)?;
        self.device.sync()?;
        
        Ok(())
    }
    
    /// Create recovery handler with existing superblock (device provided separately)
    pub fn new_with_superblock(
        superblock: JournalSuperblock,
    ) -> Self {
        Self {
            device: Box::new(DummyDevice),
            superblock,
            revoked: HashMap::new(),
        }
    }
    
    /// Get the last transaction ID after recovery
    pub fn get_last_tid(&self) -> u64 {
        self.superblock.s_sequence as u64
    }
    
    /// Get the oldest transaction ID
    pub fn get_oldest_tid(&self) -> u64 {
        // This would typically be tracked during recovery
        // For now, return a reasonable default
        self.superblock.s_sequence.saturating_sub(100) as u64
    }
}

/// Recovery statistics
#[derive(Debug, Default)]
pub struct RecoveryStats {
    /// Number of transactions found
    pub transactions_found: u64,
    /// Number of transactions replayed
    pub transactions_replayed: u64,
    /// Number of blocks revoked
    pub blocks_revoked: u64,
    /// Number of blocks recovered
    pub blocks_recovered: u64,
}