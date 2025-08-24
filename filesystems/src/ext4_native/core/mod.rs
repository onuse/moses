// Core EXT4 implementation modules

pub mod alignment;
pub mod bitmap;
pub mod checksum;
pub mod constants;
pub mod endian;
pub mod ext_config;
pub mod ext_builder;
pub mod formatter;
pub mod formatter_impl;
pub mod formatter_ext;
pub mod progress;
pub mod structures;
pub mod types;
pub mod verify;
pub mod tests;
pub mod test_overflow;
pub mod test_regression;
pub mod test_free_blocks_fix;
pub mod test_golden;
pub mod test_ext_family;

// Re-export commonly used items
pub use alignment::AlignedBuffer;
pub use checksum::crc32c_ext4;
pub use constants::*;
pub use endian::Ext4Endian;
pub use types::*;