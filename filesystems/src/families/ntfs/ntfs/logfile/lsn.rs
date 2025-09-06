// LSN (Log Sequence Number) Management for NTFS
// Handles LSN generation and tracking

use std::sync::atomic::{AtomicU64, Ordering};

/// Log Sequence Number
/// Format: High 32 bits = sequence number, Low 32 bits = offset in log file
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Lsn(pub u64);

impl Lsn {
    /// Invalid/null LSN
    pub const INVALID: Lsn = Lsn(0);
    
    /// Create a new LSN from sequence and offset
    pub fn new(sequence: u32, offset: u32) -> Self {
        Lsn(((sequence as u64) << 32) | (offset as u64))
    }
    
    /// Get the sequence number (high 32 bits)
    pub fn sequence(&self) -> u32 {
        (self.0 >> 32) as u32
    }
    
    /// Get the offset (low 32 bits)
    pub fn offset(&self) -> u32 {
        (self.0 & 0xFFFFFFFF) as u32
    }
    
    /// Check if this is a valid LSN
    pub fn is_valid(&self) -> bool {
        self.0 != 0
    }
    
    /// Calculate the physical offset in the log file
    pub fn to_physical_offset(&self, _log_page_size: u32) -> u64 {
        // The offset is in units of 8 bytes
        (self.offset() as u64) * 8
    }
    
    /// Create LSN from physical offset
    pub fn from_physical_offset(sequence: u32, physical_offset: u64) -> Self {
        // Convert physical offset to LSN offset (divide by 8)
        let lsn_offset = (physical_offset / 8) as u32;
        Lsn::new(sequence, lsn_offset)
    }
}

impl std::fmt::Display for Lsn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LSN({:#x}:{:#x})", self.sequence(), self.offset())
    }
}

/// LSN Manager - generates and tracks LSNs
pub struct LsnManager {
    /// Current sequence number
    current_sequence: AtomicU64,
    /// Current offset in log
    current_offset: AtomicU64,
    /// Log file size
    log_size: u64,
    /// Size of restart area (to skip)
    restart_area_size: u64,
}

impl LsnManager {
    /// Create a new LSN manager
    pub fn new(log_size: u64, restart_area_size: u64) -> Self {
        Self {
            current_sequence: AtomicU64::new(1),
            current_offset: AtomicU64::new(restart_area_size),
            log_size,
            restart_area_size,
        }
    }
    
    /// Initialize from existing LSN (for recovery)
    pub fn from_lsn(last_lsn: Lsn, log_size: u64, restart_area_size: u64) -> Self {
        Self {
            current_sequence: AtomicU64::new(last_lsn.sequence() as u64),
            current_offset: AtomicU64::new(last_lsn.offset() as u64),
            log_size,
            restart_area_size,
        }
    }
    
    /// Allocate a new LSN for a record of given size
    pub fn allocate(&self, record_size: u64) -> Lsn {
        // Align record size to 8 bytes
        let aligned_size = (record_size + 7) & !7;
        let lsn_size = aligned_size / 8;
        
        // Atomically allocate space
        let offset = self.current_offset.fetch_add(lsn_size, Ordering::SeqCst);
        let sequence = self.current_sequence.load(Ordering::SeqCst);
        
        // Check for wrap-around
        let physical_offset = offset * 8;
        if physical_offset + aligned_size > self.log_size {
            // Wrap to beginning (after restart area)
            self.current_offset.store(self.restart_area_size / 8, Ordering::SeqCst);
            self.current_sequence.fetch_add(1, Ordering::SeqCst);
            
            // Return LSN with new sequence
            let new_sequence = self.current_sequence.load(Ordering::SeqCst);
            return Lsn::new(new_sequence as u32, (self.restart_area_size / 8) as u32);
        }
        
        Lsn::new(sequence as u32, offset as u32)
    }
    
    /// Get the current LSN without allocating
    pub fn current_lsn(&self) -> Lsn {
        let sequence = self.current_sequence.load(Ordering::SeqCst);
        let offset = self.current_offset.load(Ordering::SeqCst);
        Lsn::new(sequence as u32, offset as u32)
    }
    
    /// Reset to a specific LSN (for recovery)
    pub fn reset_to(&self, lsn: Lsn) {
        self.current_sequence.store(lsn.sequence() as u64, Ordering::SeqCst);
        self.current_offset.store(lsn.offset() as u64, Ordering::SeqCst);
    }
    
    /// Check if we need to wrap around soon
    pub fn needs_checkpoint(&self, threshold: u64) -> bool {
        let offset = self.current_offset.load(Ordering::SeqCst);
        let physical_offset = offset * 8;
        
        // Check if we're within threshold of the end
        physical_offset + threshold > self.log_size
    }
    
    /// Calculate distance between two LSNs (in bytes)
    pub fn distance(&self, from: Lsn, to: Lsn) -> i64 {
        if from.sequence() == to.sequence() {
            // Same sequence, simple difference
            ((to.offset() - from.offset()) * 8) as i64
        } else if to.sequence() > from.sequence() {
            // Wrapped around
            let to_end = (self.log_size - (from.offset() as u64 * 8)) as i64;
            let from_start = ((to.offset() as u64 * 8) - self.restart_area_size) as i64;
            to_end + from_start
        } else {
            // to is older than from
            -self.distance(to, from)
        }
    }
    
    /// Check if an LSN is still in the active log
    pub fn is_active(&self, lsn: Lsn, oldest_lsn: Lsn) -> bool {
        if !lsn.is_valid() || !oldest_lsn.is_valid() {
            return false;
        }
        
        let current = self.current_lsn();
        
        // Check sequence numbers
        if lsn.sequence() < oldest_lsn.sequence() {
            return false;
        }
        
        if lsn.sequence() > current.sequence() {
            return false;
        }
        
        // If same sequence as oldest, check offset
        if lsn.sequence() == oldest_lsn.sequence() && lsn.offset() < oldest_lsn.offset() {
            return false;
        }
        
        // If same sequence as current, check offset
        if lsn.sequence() == current.sequence() && lsn.offset() > current.offset() {
            return false;
        }
        
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_lsn_creation() {
        let lsn = Lsn::new(0x1234, 0x5678);
        assert_eq!(lsn.sequence(), 0x1234);
        assert_eq!(lsn.offset(), 0x5678);
        assert_eq!(lsn.0, 0x0000123400005678);
    }
    
    #[test]
    fn test_lsn_physical_offset() {
        let lsn = Lsn::new(1, 100);
        assert_eq!(lsn.to_physical_offset(4096), 800); // 100 * 8
        
        let lsn2 = Lsn::from_physical_offset(1, 800);
        assert_eq!(lsn2.offset(), 100);
    }
    
    #[test]
    fn test_lsn_manager() {
        let manager = LsnManager::new(1024 * 1024, 8192);
        
        let lsn1 = manager.allocate(64);
        assert_eq!(lsn1.sequence(), 1);
        assert_eq!(lsn1.offset(), 1024); // 8192 / 8
        
        let lsn2 = manager.allocate(128);
        assert_eq!(lsn2.sequence(), 1);
        assert_eq!(lsn2.offset(), 1032); // 1024 + 64/8
        
        let current = manager.current_lsn();
        assert_eq!(current.sequence(), 1);
        assert_eq!(current.offset(), 1048); // 1032 + 128/8
    }
}