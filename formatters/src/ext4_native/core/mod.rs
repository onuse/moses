// Core EXT4 implementation modules

pub mod alignment;
pub mod bitmap;
pub mod checksum;
pub mod constants;
pub mod endian;
pub mod formatter;
pub mod formatter_impl;
pub mod structures;
pub mod types;

// Re-export commonly used items
pub use alignment::AlignedBuffer;
pub use checksum::crc32c_ext4;
pub use constants::*;
pub use endian::Ext4Endian;
pub use types::*;