// FAT16 module - formatter and reader

pub mod formatter_compliant;
pub mod reader;
pub mod validator;
pub mod detection;
pub mod root_directory;
pub mod ops;

#[cfg(test)]
mod tests;

// Use the compliant formatter as the default
pub use formatter_compliant::Fat16CompliantFormatter as Fat16Formatter;
pub use reader::Fat16Reader;
pub use ops::Fat16Ops;

// Use the new consolidated validator
pub use validator::{Fat16Validator, Fat16Validator as Fat16Verifier, ValidationReport as VerificationResult};

// Use the proper cluster-count-based detector
pub use detection::Fat16ProperDetector as Fat16Detector;