// FAT16 module - formatter, reader, and writer

pub mod formatter_compliant;
pub mod reader;
pub mod writer;
pub mod path_resolver;
pub mod file_ops;
pub mod validator;
pub mod detection;
pub mod root_directory;
pub mod ops;
pub mod lfn_support;
pub mod subdirectory_ops;

#[cfg(test)]
mod tests;

// Use the compliant formatter as the default
pub use formatter_compliant::Fat16CompliantFormatter as Fat16Formatter;
pub use reader::Fat16Reader;
pub use writer::Fat16Writer;
pub use ops::Fat16Ops;

// Use the new consolidated validator
pub use validator::{Fat16Validator, Fat16Validator as Fat16Verifier, ValidationReport as VerificationResult};

// Use the proper cluster-count-based detector
pub use detection::Fat16ProperDetector as Fat16Detector;