// Core EXT4 implementation modules

pub mod alignment;
pub mod bitmap;
pub mod block_allocator;
pub mod checksum;
pub mod constants;
pub mod endian;
pub mod ext_config;
pub mod ext_builder;
pub mod formatter;
pub mod formatter_impl;
pub mod formatter_ext;
pub mod inode_allocator;
pub mod progress;
pub mod structures;
pub mod transaction;
pub mod types;
pub mod verify;

#[cfg(test)]
pub mod tests;
#[cfg(test)]
pub mod test_overflow;
#[cfg(test)]
pub mod test_regression;
#[cfg(test)]
pub mod test_free_blocks_fix;
#[cfg(test)]
pub mod test_golden;
#[cfg(test)]
pub mod test_ext_family;

// Re-export commonly used items
pub use alignment::AlignedBuffer;
pub use checksum::crc32c_ext4;
pub use constants::*;
pub use endian::Ext4Endian;
pub use types::*;