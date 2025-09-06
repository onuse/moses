// Standalone Phase 1 test program
// Creates a minimal ext4 image with just a superblock and validates it

use std::fs::File;
use std::io::{Write, Read};
use std::process::Command;

fn main() {
    println!("=== EXT4 Native Phase 1: Superblock Test ===\n");
    
    // Test parameters
    let image_path = "test_ext4_phase1.img";
    let image_size = 100 * 1024 * 1024; // 100MB
    
    println!("Creating test image: {}", image_path);
    println!("Size: {} MB", image_size / (1024 * 1024));
    
    // Create the image using our implementation
    if let Err(e) = create_phase1_image(image_path, image_size) {
        eprintln!("Failed to create image: {}", e);
        return;
    }
    
    println!("\n=== Hexdump of Superblock Region ===");
    dump_superblock_region(image_path);
    
    println!("\n=== Attempting validation with dumpe2fs ===");
    validate_with_dumpe2fs(image_path);
    
    println!("\n=== Attempting validation with e2fsck ===");
    validate_with_e2fsck(image_path);
}

fn create_phase1_image(path: &str, size_bytes: u64) -> Result<(), String> {
    use crate::families::ext::ext4_native::core::{
        structures::Ext4Superblock,
        types::{FilesystemParams, FilesystemLayout},
        alignment::AlignedBuffer,
    };
    
    // Create filesystem parameters
    let params = FilesystemParams {
        size_bytes,
        block_size: 4096,
        inode_size: 256,
        label: Some("Phase1Test".to_string()),
        reserved_percent: 5,
        enable_checksums: true,
        enable_64bit: false, // Keep simple for Phase 1
        enable_journal: false,
    };
    
    // Calculate layout
    let layout = FilesystemLayout::from_params(&params)?;
    
    println!("Filesystem layout:");
    println!("  Total blocks: {}", layout.total_blocks);
    println!("  Block groups: {}", layout.num_groups);
    println!("  Blocks per group: {}", layout.blocks_per_group);
    println!("  Inodes per group: {}", layout.inodes_per_group);
    
    // Create and initialize superblock
    let mut superblock = Ext4Superblock::new();
    superblock.init_minimal(&params, &layout);
    superblock.update_checksum();
    
    // Validate before writing
    superblock.validate()?;
    
    // Create image file with proper size
    let mut file = File::create(path)
        .map_err(|e| format!("Failed to create file: {}", e))?;
    
    // Write zeros for the entire image
    // In Phase 1, we only write the superblock
    let zeros = vec![0u8; 1024 * 1024]; // 1MB buffer
    let mut written = 0u64;
    while written < size_bytes {
        let to_write = ((size_bytes - written) as usize).min(zeros.len());
        file.write_all(&zeros[..to_write])
            .map_err(|e| format!("Failed to write zeros: {}", e))?;
        written += to_write as u64;
    }
    
    // Seek back to beginning
    use std::io::Seek;
    file.seek(std::io::SeekFrom::Start(0))
        .map_err(|e| format!("Failed to seek: {}", e))?;
    
    // Create buffer for first 8KB (contains superblock at offset 1024)
    let mut buffer = AlignedBuffer::<8192>::new();
    
    // Write superblock at offset 1024
    superblock.write_to_buffer(&mut buffer[1024..2048])
        .map_err(|e| format!("Failed to serialize superblock: {}", e))?;
    
    // Write the buffer to file
    file.write_all(&buffer[..])
        .map_err(|e| format!("Failed to write superblock: {}", e))?;
    
    file.sync_all()
        .map_err(|e| format!("Failed to sync: {}", e))?;
    
    println!("\nSuperblock written successfully!");
    println!("  Magic: 0x{:04X}", superblock.s_magic);
    println!("  State: 0x{:04X}", superblock.s_state);
    println!("  Block size: {} bytes", 1024 << superblock.s_log_block_size);
    println!("  Inode size: {} bytes", superblock.s_inode_size);
    println!("  Label: {:?}", 
        std::str::from_utf8(&superblock.s_volume_name)
            .unwrap_or("")
            .trim_end_matches('\0'));
    
    Ok(())
}

fn dump_superblock_region(path: &str) {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to open file: {}", e);
            return;
        }
    };
    
    let mut buffer = vec![0u8; 2048];
    if let Err(e) = file.read_exact(&mut buffer) {
        eprintln!("Failed to read file: {}", e);
        return;
    }
    
    // Show hexdump of superblock region (offset 1024-1280)
    println!("Offset 0x400 (1024) - Superblock start:");
    for i in 0..16 {
        let offset = 1024 + i * 16;
        print!("{:04X}: ", offset);
        
        for j in 0..16 {
            print!("{:02X} ", buffer[offset + j]);
        }
        
        print!("  ");
        for j in 0..16 {
            let byte = buffer[offset + j];
            if byte >= 0x20 && byte < 0x7F {
                print!("{}", byte as char);
            } else {
                print!(".");
            }
        }
        println!();
    }
    
    // Show specific fields
    println!("\nKey fields:");
    println!("  Magic (0x438): {:02X} {:02X}", buffer[0x438], buffer[0x439]);
    println!("  State (0x43A): {:02X} {:02X}", buffer[0x43A], buffer[0x43B]);
    println!("  Checksum (0x7FC): {:02X} {:02X} {:02X} {:02X}", 
             buffer[0x7FC], buffer[0x7FD], buffer[0x7FE], buffer[0x7FF]);
}

fn validate_with_dumpe2fs(path: &str) {
    let output = Command::new("dumpe2fs")
        .arg(path)
        .output();
    
    match output {
        Ok(result) => {
            if !result.stdout.is_empty() {
                let stdout = String::from_utf8_lossy(&result.stdout);
                // Show first few lines
                for line in stdout.lines().take(20) {
                    println!("{}", line);
                }
            }
            if !result.stderr.is_empty() {
                let stderr = String::from_utf8_lossy(&result.stderr);
                eprintln!("Errors: {}", stderr);
            }
        }
        Err(_) => {
            println!("dumpe2fs not available (expected on Windows)");
        }
    }
}

fn validate_with_e2fsck(path: &str) {
    let output = Command::new("e2fsck")
        .arg("-n") // Read-only check
        .arg(path)
        .output();
    
    match output {
        Ok(result) => {
            if !result.stdout.is_empty() {
                let stdout = String::from_utf8_lossy(&result.stdout);
                println!("{}", stdout);
            }
            if !result.stderr.is_empty() {
                let stderr = String::from_utf8_lossy(&result.stderr);
                eprintln!("{}", stderr);
            }
        }
        Err(_) => {
            println!("e2fsck not available (expected on Windows)");
        }
    }
}