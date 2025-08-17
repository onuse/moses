// Base traits and shared components for ext filesystem family
// This module defines the common interface that ext2/ext3/ext4 all implement

use crate::ext4_native::core::{MosesError, constants::*};

pub mod traits;
pub mod common;

// Re-export main traits
pub use traits::{ExtSuperblock, ExtInode, ExtGroupDesc, ExtParams, ExtLayout};