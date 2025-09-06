// EXT4 JBD2 Transaction Barriers
// Implements blocking barriers for transaction synchronization

use moses_core::MosesError;
use std::sync::{Arc, Condvar, Mutex};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Transaction barrier state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BarrierState {
    /// Barrier is idle
    Idle,
    /// Barrier is active (blocking new operations)
    Active,
    /// Barrier is draining (waiting for in-flight operations)
    Draining,
    /// Barrier is committing
    Committing,
}

/// Transaction barrier statistics
#[derive(Debug, Default, Clone)]
pub struct BarrierStats {
    /// Total number of barriers created
    pub total_barriers: u64,
    /// Total wait time in milliseconds
    pub total_wait_ms: u64,
    /// Maximum wait time in milliseconds
    pub max_wait_ms: u64,
    /// Number of operations blocked
    pub operations_blocked: u64,
    /// Number of operations drained
    pub operations_drained: u64,
}

/// Barrier request
struct BarrierRequest {
    /// Request ID
    id: u64,
    /// Timestamp when requested
    requested_at: Instant,
    /// Whether this is a forced barrier
    forced: bool,
}

/// Transaction barrier manager
pub struct TransactionBarrier {
    /// Current state
    state: Arc<Mutex<BarrierState>>,
    /// Condition variable for state changes
    state_cond: Arc<Condvar>,
    /// In-flight operation counter
    in_flight_ops: Arc<Mutex<u32>>,
    /// Pending barrier requests
    pending_barriers: Arc<Mutex<VecDeque<BarrierRequest>>>,
    /// Barrier statistics
    stats: Arc<Mutex<BarrierStats>>,
    /// Next barrier ID
    next_barrier_id: Arc<Mutex<u64>>,
    /// Maximum operations before forcing barrier
    max_operations: u32,
    /// Maximum time before forcing barrier
    max_time: Duration,
}

impl TransactionBarrier {
    /// Create a new transaction barrier
    pub fn new(max_operations: u32, max_time_secs: u64) -> Self {
        Self {
            state: Arc::new(Mutex::new(BarrierState::Idle)),
            state_cond: Arc::new(Condvar::new()),
            in_flight_ops: Arc::new(Mutex::new(0)),
            pending_barriers: Arc::new(Mutex::new(VecDeque::new())),
            stats: Arc::new(Mutex::new(BarrierStats::default())),
            next_barrier_id: Arc::new(Mutex::new(1)),
            max_operations,
            max_time: Duration::from_secs(max_time_secs),
        }
    }
    
    /// Begin an operation (blocks if barrier is active)
    pub fn begin_operation(&self) -> Result<OperationGuard<'_>, MosesError> {
        let start_time = Instant::now();
        
        // Wait for barrier to complete if active
        {
            let mut state = self.state.lock().unwrap();
            let mut stats = self.stats.lock().unwrap();
            
            while *state != BarrierState::Idle {
                stats.operations_blocked += 1;
                state = self.state_cond.wait(state).unwrap();
            }
            
            // Record wait time
            let wait_ms = start_time.elapsed().as_millis() as u64;
            stats.total_wait_ms += wait_ms;
            if wait_ms > stats.max_wait_ms {
                stats.max_wait_ms = wait_ms;
            }
        }
        
        // Increment in-flight counter
        {
            let mut in_flight = self.in_flight_ops.lock().unwrap();
            *in_flight += 1;
            
            // Check if we need to trigger a barrier
            if *in_flight >= self.max_operations {
                self.request_barrier(false)?;
            }
        }
        
        Ok(OperationGuard {
            barrier: self,
            completed: false,
        })
    }
    
    /// Request a transaction barrier
    pub fn request_barrier(&self, forced: bool) -> Result<u64, MosesError> {
        let barrier_id = {
            let mut next_id = self.next_barrier_id.lock().unwrap();
            let id = *next_id;
            *next_id += 1;
            id
        };
        
        let request = BarrierRequest {
            id: barrier_id,
            requested_at: Instant::now(),
            forced,
        };
        
        {
            let mut pending = self.pending_barriers.lock().unwrap();
            pending.push_back(request);
        }
        
        // Try to activate barrier if idle
        self.try_activate_barrier()?;
        
        Ok(barrier_id)
    }
    
    /// Try to activate a pending barrier
    fn try_activate_barrier(&self) -> Result<(), MosesError> {
        let mut state = self.state.lock().unwrap();
        
        if *state != BarrierState::Idle {
            return Ok(()); // Already processing a barrier
        }
        
        let request = {
            let mut pending = self.pending_barriers.lock().unwrap();
            pending.pop_front()
        };
        
        if let Some(request) = request {
            log::debug!("Activating barrier {} (forced: {})", request.id, request.forced);
            
            *state = BarrierState::Active;
            drop(state);
            
            // Start draining in-flight operations
            self.drain_operations()?;
            
            // Update stats
            {
                let mut stats = self.stats.lock().unwrap();
                stats.total_barriers += 1;
            }
        }
        
        Ok(())
    }
    
    /// Drain in-flight operations
    fn drain_operations(&self) -> Result<(), MosesError> {
        {
            let mut state = self.state.lock().unwrap();
            *state = BarrierState::Draining;
        }
        
        log::trace!("Draining in-flight operations");
        
        // Wait for all in-flight operations to complete
        let drain_start = Instant::now();
        loop {
            let in_flight = {
                let count = self.in_flight_ops.lock().unwrap();
                *count
            };
            
            if in_flight == 0 {
                break;
            }
            
            // Check timeout
            if drain_start.elapsed() > Duration::from_secs(30) {
                log::warn!("Barrier drain timeout with {} operations remaining", in_flight);
                return Err(MosesError::Other("Barrier drain timeout".to_string()));
            }
            
            std::thread::sleep(Duration::from_millis(10));
        }
        
        {
            let mut stats = self.stats.lock().unwrap();
            stats.operations_drained += 1;
        }
        
        // Move to committing state
        {
            let mut state = self.state.lock().unwrap();
            *state = BarrierState::Committing;
        }
        
        Ok(())
    }
    
    /// Complete the barrier
    pub fn complete_barrier(&self) -> Result<(), MosesError> {
        let mut state = self.state.lock().unwrap();
        
        if *state != BarrierState::Committing {
            return Err(MosesError::Other("Cannot complete barrier in current state".to_string()));
        }
        
        log::trace!("Completing barrier");
        
        *state = BarrierState::Idle;
        
        // Wake up waiting operations
        self.state_cond.notify_all();
        
        // Try to activate next barrier if any
        drop(state);
        self.try_activate_barrier()?;
        
        Ok(())
    }
    
    /// Wait for a specific barrier to complete
    pub fn wait_for_barrier(&self, barrier_id: u64) -> Result<(), MosesError> {
        let start_time = Instant::now();
        
        loop {
            // Check if barrier is still pending or active
            let is_pending = {
                let pending = self.pending_barriers.lock().unwrap();
                pending.iter().any(|r| r.id == barrier_id)
            };
            
            if !is_pending {
                // Check if we're currently processing this barrier
                let state = self.state.lock().unwrap();
                if *state == BarrierState::Idle {
                    // Barrier must have completed
                    break;
                }
            }
            
            // Check timeout
            if start_time.elapsed() > Duration::from_secs(60) {
                return Err(MosesError::Other("Barrier wait timeout".to_string()));
            }
            
            std::thread::sleep(Duration::from_millis(10));
        }
        
        Ok(())
    }
    
    /// Get current state
    pub fn state(&self) -> BarrierState {
        *self.state.lock().unwrap()
    }
    
    /// Get statistics
    pub fn stats(&self) -> BarrierStats {
        self.stats.lock().unwrap().clone()
    }
    
    /// Check if we should force a barrier due to time
    pub fn check_time_barrier(&self) -> Result<(), MosesError> {
        let should_force = {
            let pending = self.pending_barriers.lock().unwrap();
            if let Some(oldest) = pending.front() {
                oldest.requested_at.elapsed() > self.max_time
            } else {
                false
            }
        };
        
        if should_force {
            self.request_barrier(true)?;
        }
        
        Ok(())
    }
}

/// Guard for an in-flight operation
pub struct OperationGuard<'a> {
    barrier: &'a TransactionBarrier,
    completed: bool,
}

impl<'a> OperationGuard<'a> {
    /// Mark operation as completed
    pub fn complete(mut self) {
        self.completed = true;
        let mut in_flight = self.barrier.in_flight_ops.lock().unwrap();
        *in_flight -= 1;
    }
}

impl<'a> Drop for OperationGuard<'a> {
    fn drop(&mut self) {
        if !self.completed {
            // Decrement counter on drop
            let mut in_flight = self.barrier.in_flight_ops.lock().unwrap();
            *in_flight -= 1;
        }
    }
}

/// Barrier-aware transaction manager
pub struct BarrierTransactionManager {
    /// Transaction barrier
    barrier: Arc<TransactionBarrier>,
    /// Background thread handle
    monitor_thread: Option<std::thread::JoinHandle<()>>,
    /// Shutdown flag
    shutdown: Arc<Mutex<bool>>,
}

impl BarrierTransactionManager {
    /// Create a new barrier transaction manager
    pub fn new(max_operations: u32, max_time_secs: u64) -> Self {
        let barrier = Arc::new(TransactionBarrier::new(max_operations, max_time_secs));
        let shutdown = Arc::new(Mutex::new(false));
        
        // Start background monitoring thread
        let monitor_barrier = barrier.clone();
        let monitor_shutdown = shutdown.clone();
        let monitor_thread = std::thread::spawn(move || {
            Self::monitor_loop(monitor_barrier, monitor_shutdown);
        });
        
        Self {
            barrier,
            monitor_thread: Some(monitor_thread),
            shutdown,
        }
    }
    
    /// Background monitoring loop
    fn monitor_loop(barrier: Arc<TransactionBarrier>, shutdown: Arc<Mutex<bool>>) {
        loop {
            // Check shutdown
            {
                let shutdown = shutdown.lock().unwrap();
                if *shutdown {
                    break;
                }
            }
            
            // Check for time-based barriers
            if let Err(e) = barrier.check_time_barrier() {
                log::warn!("Error checking time barrier: {}", e);
            }
            
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    
    /// Begin a new operation
    pub fn begin_operation(&self) -> Result<OperationGuard<'_>, MosesError> {
        self.barrier.begin_operation()
    }
    
    /// Force a transaction barrier
    pub fn force_barrier(&self) -> Result<(), MosesError> {
        let barrier_id = self.barrier.request_barrier(true)?;
        self.barrier.wait_for_barrier(barrier_id)
    }
    
    /// Get barrier statistics
    pub fn stats(&self) -> BarrierStats {
        self.barrier.stats()
    }
    
    /// Shutdown the manager
    pub fn shutdown(&mut self) {
        {
            let mut shutdown = self.shutdown.lock().unwrap();
            *shutdown = true;
        }
        
        if let Some(thread) = self.monitor_thread.take() {
            thread.join().ok();
        }
    }
}

impl Drop for BarrierTransactionManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_barrier_basic() {
        let barrier = TransactionBarrier::new(10, 60);
        
        // Start an operation
        let guard = barrier.begin_operation().unwrap();
        assert_eq!(barrier.state(), BarrierState::Idle);
        
        // Complete the operation
        guard.complete();
    }
    
    #[test]
    fn test_barrier_blocking() {
        let barrier = Arc::new(TransactionBarrier::new(10, 60));
        
        // Request a barrier
        barrier.request_barrier(true).unwrap();
        
        // Try to start operation (should block until barrier completes)
        let barrier_clone = barrier.clone();
        let handle = std::thread::spawn(move || {
            let _guard = barrier_clone.begin_operation().unwrap();
        });
        
        // Give thread time to block
        std::thread::sleep(Duration::from_millis(100));
        
        // Complete the barrier
        barrier.complete_barrier().unwrap();
        
        // Thread should now complete
        handle.join().unwrap();
    }
}