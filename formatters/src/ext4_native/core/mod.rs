// Core EXT4 implementation modules

pub mod alignment;
pub mod bitmap;
pub mod checksum;
pub mod constants;
pub mod endian;
pub mod formatter;
pub mod formatter_impl;
pub mod progress;
pub mod structures;
pub mod types;
pub mod verify;
pub mod tests;
pub mod test_overflow;
pub mod test_regression;
pub mod test_free_blocks_fix;

// Re-export commonly used items
pub use alignment::AlignedBuffer;
pub use checksum::crc32c_ext4;
pub use constants::*;
pub use endian::Ext4Endian;
pub use types::*;