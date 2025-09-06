// NTFS $LogFile Implementation
// Provides journaling support for NTFS filesystem operations

pub mod structures;
pub mod lsn;
pub mod writer;
pub mod reader;
pub mod recovery;

pub use structures::*;
pub use lsn::{Lsn, LsnManager};
pub use writer::LogFileWriter;
pub use reader::LogFileReader;
pub use recovery::LogFileRecovery;

/// $LogFile configuration
pub struct LogFileConfig {
    /// Size of the log file in bytes
    pub log_size: u64,
    /// Size of each log page (typically 4096)
    pub page_size: u32,
    /// Size of log record pages (typically 4096)
    pub record_page_size: u32,
    /// Number of restart areas (typically 2)
    pub restart_area_count: u32,
}

impl Default for LogFileConfig {
    fn default() -> Self {
        Self {
            log_size: 64 * 1024 * 1024,  // 64MB default
            page_size: 4096,
            record_page_size: 4096,
            restart_area_count: 2,
        }
    }
}

/// Log operation types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogOperation {
    /// No operation
    Noop,
    /// Compensation log record
    CompensationLogRecord,
    /// Initialize file record segment
    InitializeFileRecordSegment,
    /// Deallocate file record segment
    DeallocateFileRecordSegment,
    /// Write end of file record segment
    WriteEndOfFileRecordSegment,
    /// Set attribute value
    SetAttributeValue,
    /// Add attribute
    AddAttribute,
    /// Delete attribute
    DeleteAttribute,
    /// Update resident value
    UpdateResidentValue,
    /// Update non-resident value
    UpdateNonResidentValue,
    /// Update mapping pairs
    UpdateMappingPairs,
    /// Set new attribute sizes
    SetNewAttributeSizes,
    /// Add index entry to root
    AddIndexEntryRoot,
    /// Delete index entry from root
    DeleteIndexEntryRoot,
    /// Add index entry to allocation
    AddIndexEntryAllocation,
    /// Delete index entry from allocation
    DeleteIndexEntryAllocation,
    /// Set index entry VCN
    SetIndexEntryVcn,
    /// Update file name in root
    UpdateFileNameRoot,
    /// Update file name in allocation
    UpdateFileNameAllocation,
    /// Set bits in bitmap
    SetBitsInBitmap,
    /// Clear bits in bitmap
    ClearBitsInBitmap,
    /// Prepare transaction
    PrepareTransaction,
    /// Commit transaction
    CommitTransaction,
    /// Forget transaction
    ForgetTransaction,
    /// Open attribute table dump
    OpenAttributeTableDump,
    /// Attribute names dump
    AttributeNamesDump,
    /// Dirty page table dump
    DirtyPageTableDump,
    /// Transaction table dump
    TransactionTableDump,
}

/// Transaction state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransactionState {
    /// Transaction is active
    Active,
    /// Transaction is prepared (for 2-phase commit)
    Prepared,
    /// Transaction is committed
    Committed,
    /// Transaction is aborted
    Aborted,
}

/// Log file statistics
#[derive(Debug, Default)]
pub struct LogFileStats {
    /// Current LSN
    pub current_lsn: u64,
    /// Oldest LSN in log
    pub oldest_lsn: u64,
    /// Number of active transactions
    pub active_transactions: u32,
    /// Number of log records written
    pub records_written: u64,
    /// Number of checkpoints
    pub checkpoints: u64,
    /// Log file usage percentage
    pub usage_percent: f32,
}