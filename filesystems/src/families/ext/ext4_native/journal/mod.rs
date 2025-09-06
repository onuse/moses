// EXT4 JBD2 (Journaling Block Device 2) Implementation
// Provides journaling support for EXT4 filesystem operations

pub mod jbd2;
pub mod transaction;
pub mod recovery;
pub mod checkpoint;
pub mod device;
pub mod checksum;
pub mod barrier;
pub mod dummy_device;

pub use jbd2::{Jbd2Journal, JournalSuperblock, JournalDevice};
pub use transaction::{Transaction, Handle};
pub use jbd2::TransactionState;
pub use recovery::JournalRecovery;
pub use checkpoint::Checkpoint;
pub use barrier::{TransactionBarrier, BarrierTransactionManager, BarrierState, BarrierStats};
pub use dummy_device::DummyJournalDevice;


/// Journal configuration and capabilities
pub struct JournalConfig {
    /// Journal inode number (typically 8)
    pub journal_inode: u32,
    /// Journal device (if external)
    pub journal_device: Option<String>,
    /// Journal size in blocks
    pub journal_blocks: u32,
    /// Transaction commit interval (in seconds)
    pub commit_interval: u32,
    /// Journal mode
    pub mode: JournalMode,
}

/// Journal modes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JournalMode {
    /// Journal data and metadata
    Journal,
    /// Journal metadata only, data written before metadata
    Ordered,
    /// Journal metadata only, data can be written anytime
    Writeback,
}

impl Default for JournalConfig {
    fn default() -> Self {
        Self {
            journal_inode: 8,
            journal_device: None,
            journal_blocks: 32768, // 128MB with 4K blocks
            commit_interval: 5,
            mode: JournalMode::Ordered,
        }
    }
}

/// Journal statistics
#[derive(Debug, Default)]
pub struct JournalStats {
    /// Total transactions started
    pub transactions_started: u64,
    /// Total transactions committed
    pub transactions_committed: u64,
    /// Total transactions aborted
    pub transactions_aborted: u64,
    /// Total blocks logged
    pub blocks_logged: u64,
    /// Current transaction ID
    pub current_tid: u64,
    /// Oldest transaction ID in journal
    pub oldest_tid: u64,
}