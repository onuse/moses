// EXT4 filesystem comparator
// Compares our output with mkfs.ext4 for validation

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use crate::families::ext::ext4_native::core::types::*;

pub struct Ext4Comparator {
    verbose: bool,
}

#[derive(Debug, Default)]
pub struct ComparisonReport {
    pub superblock_match: bool,
    pub gdt_match: bool,
    pub differences: Vec<String>,
}

impl Ext4Comparator {
    pub fn new() -> Self {
        Self { verbose: false }
    }
    
    pub fn verbose(mut self) -> Self {
        self.verbose = true;
        self
    }
    
    /// Compare two filesystem images
    pub fn compare_images(&self, our_path: &str, reference_path: &str) -> Ext4Result<ComparisonReport> {
        let mut report = ComparisonReport::default();
        
        // Compare superblock at offset 1024
        match self.compare_region(our_path, reference_path, 1024, 1024) {
            Ok(matches) => {
                report.superblock_match = matches;
                if !matches {
                    report.differences.push("Superblock differs".to_string());
                }
            }
            Err(e) => {
                report.differences.push(format!("Failed to compare superblock: {}", e));
            }
        }
        
        // Compare GDT (would need to calculate size)
        // For now, just compare first 4KB after superblock
        match self.compare_region(our_path, reference_path, 2048, 4096) {
            Ok(matches) => {
                report.gdt_match = matches;
                if !matches {
                    report.differences.push("Group descriptors differ".to_string());
                }
            }
            Err(e) => {
                report.differences.push(format!("Failed to compare GDT: {}", e));
            }
        }
        
        Ok(report)
    }
    
    /// Compare a specific region of two files
    fn compare_region(&self, path1: &str, path2: &str, offset: u64, size: usize) -> Ext4Result<bool> {
        let mut file1 = File::open(path1)?;
        let mut file2 = File::open(path2)?;
        
        file1.seek(SeekFrom::Start(offset))?;
        file2.seek(SeekFrom::Start(offset))?;
        
        let mut buf1 = vec![0u8; size];
        let mut buf2 = vec![0u8; size];
        
        file1.read_exact(&mut buf1)?;
        file2.read_exact(&mut buf2)?;
        
        if self.verbose && buf1 != buf2 {
            // Find first difference
            for i in 0..size {
                if buf1[i] != buf2[i] {
                    println!("First difference at offset {:#X}: {:#02X} vs {:#02X}", 
                             offset + i as u64, buf1[i], buf2[i]);
                    break;
                }
            }
        }
        
        Ok(buf1 == buf2)
    }
    
    /// Dump a region as hex for debugging
    pub fn dump_hex(&self, path: &str, offset: u64, size: usize) -> Ext4Result<String> {
        let mut file = File::open(path)?;
        file.seek(SeekFrom::Start(offset))?;
        
        let mut buf = vec![0u8; size];
        file.read_exact(&mut buf)?;
        
        let hex_str = hex::encode(&buf);
        Ok(hex_str)
    }
}

impl Default for Ext4Comparator {
    fn default() -> Self {
        Self::new()
    }
}