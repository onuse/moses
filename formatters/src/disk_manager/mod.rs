// Disk Management Module - Clean, Convert, and Prepare operations
// These are lower-level than formatting - they prepare disks for formatting

pub mod cleaner;
pub mod converter;
pub mod detector;

pub use cleaner::{DiskCleaner, CleanOptions, WipeMethod};
pub use converter::{PartitionStyleConverter, PartitionStyle};
pub use detector::{ConflictDetector, DiskConflict, ConflictSeverity, ConflictReport};

/// High-level disk preparation API
pub struct DiskManager;

impl DiskManager {
    /// Prepare a disk for formatting by resolving conflicts
    pub fn prepare_disk(
        device: &moses_core::Device,
        target_style: PartitionStyle,
        clean_first: bool,
    ) -> Result<PreparationReport, moses_core::MosesError> {
        let mut report = PreparationReport::default();
        
        // 1. Detect current state and conflicts
        let conflicts = ConflictDetector::analyze(device)?;
        report.initial_state = conflicts.current_state.clone();
        report.conflicts_found = conflicts.conflicts.clone();
        
        // 2. Clean if requested or if conflicts exist
        if clean_first || !conflicts.conflicts.is_empty() {
            let clean_options = CleanOptions {
                wipe_method: WipeMethod::Quick,
                zero_entire_disk: false,
            };
            
            DiskCleaner::clean(device, &clean_options)?;
            report.cleaned = true;
        }
        
        // 3. Convert to target partition style
        if target_style != PartitionStyle::Uninitialized {
            PartitionStyleConverter::convert(device, target_style)?;
            report.final_style = Some(target_style);
        }
        
        report.success = true;
        Ok(report)
    }
    
    /// Quick clean - removes all partition structures
    pub fn quick_clean(device: &moses_core::Device) -> Result<(), moses_core::MosesError> {
        let options = CleanOptions {
            wipe_method: WipeMethod::Quick,
            zero_entire_disk: false,
        };
        DiskCleaner::clean(device, &options)
    }
    
    /// Secure wipe - DoD 5220.22-M standard
    pub fn secure_wipe(device: &moses_core::Device) -> Result<(), moses_core::MosesError> {
        let options = CleanOptions {
            wipe_method: WipeMethod::DoD5220,
            zero_entire_disk: true,
        };
        DiskCleaner::clean(device, &options)
    }
}

#[derive(Debug, Default)]
pub struct PreparationReport {
    pub success: bool,
    pub initial_state: String,
    pub conflicts_found: Vec<DiskConflict>,
    pub cleaned: bool,
    pub final_style: Option<PartitionStyle>,
}