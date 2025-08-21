// FAT16 module - formatter and reader

pub mod formatter;
pub mod formatter_fixed;
pub mod formatter_compliant;
pub mod system_formatter;
pub mod reader;
pub mod verifier;
pub mod spec_compliance_test;
pub mod root_directory;
pub mod detection;
pub mod comprehensive_validator;
pub mod ultimate_validator;
pub mod formatter_validator;

#[cfg(test)]
mod tests;

// Use the compliant formatter as the default
pub use formatter_compliant::Fat16CompliantFormatter as Fat16Formatter;
// Keep the old ones available for testing
pub use formatter_fixed::Fat16FormatterFixed;
pub use formatter::Fat16Formatter as Fat16FormatterOriginal;
pub use reader::Fat16Reader;
pub use verifier::{Fat16Verifier, VerificationResult};

// Use the proper cluster-count-based detector
pub use detection::Fat16ProperDetector as Fat16Detector;