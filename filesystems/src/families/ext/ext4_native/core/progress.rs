// Progress reporting for ext4 formatting operations

use std::sync::Arc;

/// Progress information for formatting operations
#[derive(Debug, Clone)]
pub struct FormatProgress {
    /// Current step number (0-based)
    pub current_step: usize,
    /// Total number of steps
    pub total_steps: usize,
    /// Current step description
    pub step_description: String,
    /// Percentage complete (0-100)
    pub percentage: f32,
    /// Bytes written so far
    pub bytes_written: u64,
    /// Total bytes to write (estimated)
    pub total_bytes: u64,
}

impl FormatProgress {
    pub fn new(total_steps: usize, total_bytes: u64) -> Self {
        Self {
            current_step: 0,
            total_steps,
            step_description: String::new(),
            percentage: 0.0,
            bytes_written: 0,
            total_bytes,
        }
    }
    
    pub fn update_step(&mut self, step: usize, description: impl Into<String>) {
        self.current_step = step;
        self.step_description = description.into();
        self.percentage = (step as f32 / self.total_steps as f32) * 100.0;
    }
    
    pub fn update_bytes(&mut self, bytes: u64) {
        self.bytes_written += bytes;
        if self.total_bytes > 0 {
            let byte_percentage = (self.bytes_written as f32 / self.total_bytes as f32) * 100.0;
            // Use the higher of step percentage or byte percentage
            self.percentage = self.percentage.max(byte_percentage);
        }
    }
}

/// Trait for progress reporting callbacks
pub trait ProgressCallback: Send + Sync {
    fn on_progress(&self, progress: &FormatProgress);
}

/// No-op progress callback (does nothing)
pub struct NoOpProgress;

impl ProgressCallback for NoOpProgress {
    fn on_progress(&self, _progress: &FormatProgress) {
        // Do nothing
    }
}

/// Logging progress callback
pub struct LoggingProgress;

impl ProgressCallback for LoggingProgress {
    fn on_progress(&self, progress: &FormatProgress) {
        use log::info;
        info!("Format progress: {:.1}% - Step {}/{}: {}", 
              progress.percentage,
              progress.current_step + 1,
              progress.total_steps,
              progress.step_description);
    }
}

/// Function-based progress callback
pub struct FnProgress<F>
where
    F: Fn(&FormatProgress) + Send + Sync,
{
    callback: F,
}

impl<F> FnProgress<F>
where
    F: Fn(&FormatProgress) + Send + Sync,
{
    pub fn new(callback: F) -> Self {
        Self { callback }
    }
}

impl<F> ProgressCallback for FnProgress<F>
where
    F: Fn(&FormatProgress) + Send + Sync,
{
    fn on_progress(&self, progress: &FormatProgress) {
        (self.callback)(progress);
    }
}

/// Progress reporter that manages callbacks
pub struct ProgressReporter {
    progress: FormatProgress,
    callback: Arc<dyn ProgressCallback>,
}

impl ProgressReporter {
    pub fn new(total_steps: usize, total_bytes: u64, callback: Arc<dyn ProgressCallback>) -> Self {
        Self {
            progress: FormatProgress::new(total_steps, total_bytes),
            callback,
        }
    }
    
    pub fn with_noop(total_steps: usize, total_bytes: u64) -> Self {
        Self::new(total_steps, total_bytes, Arc::new(NoOpProgress))
    }
    
    pub fn with_logging(total_steps: usize, total_bytes: u64) -> Self {
        Self::new(total_steps, total_bytes, Arc::new(LoggingProgress))
    }
    
    pub fn start_step(&mut self, step: usize, description: impl Into<String>) {
        self.progress.update_step(step, description);
        self.callback.on_progress(&self.progress);
    }
    
    pub fn update_bytes(&mut self, bytes: u64) {
        self.progress.update_bytes(bytes);
        self.callback.on_progress(&self.progress);
    }
    
    pub fn complete(&mut self) {
        self.progress.percentage = 100.0;
        self.progress.current_step = self.progress.total_steps;
        self.callback.on_progress(&self.progress);
    }
}