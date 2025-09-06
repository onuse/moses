// Test MFT DATA attribute creation
use moses_filesystems::families::ntfs::ntfs::mft_writer::MftRecordBuilder;
use moses_filesystems::families::ntfs::ntfs::structures::*;

fn main() {
    println!("Testing MFT DATA attribute creation...\n");
    
    // Test parameters
    let record_size = 1024u32;
    let mft_cluster = 4u64;
    let bytes_per_cluster = 4096u32;
    let mft_clusters = 4u64; // 16KB MFT initially
    
    // Create MFT record 0 with non-resident DATA attribute
    let result = MftRecordBuilder::new(0, record_size)
        .as_file()
        .with_standard_info(0, 0, 0, 0x06)
        .unwrap()
        .with_file_name(5, "$MFT", 3, 0, 0, 0, 16384, 16384, 0x06)
        .unwrap()
        .with_non_resident_data(mft_cluster, mft_clusters, bytes_per_cluster)
        .unwrap()
        .build();
    
    match result {
        Ok(record) => {
            println!("✓ MFT record created successfully");
            println!("  Record size: {} bytes", record.len());
            
            // Parse and check attributes
            let attrs_offset = 56; // Standard offset
            let mut offset = attrs_offset;
            let mut found_data = false;
            
            while offset < record.len() - 4 {
                // Read attribute type
                let attr_type = u32::from_le_bytes([
                    record[offset],
                    record[offset + 1],
                    record[offset + 2],
                    record[offset + 3],
                ]);
                
                // Check for end marker
                if attr_type == 0xFFFFFFFF {
                    break;
                }
                
                // Read attribute length
                let attr_len = u32::from_le_bytes([
                    record[offset + 4],
                    record[offset + 5],
                    record[offset + 6],
                    record[offset + 7],
                ]);
                
                // Check non-resident flag
                let non_resident = record[offset + 8];
                
                println!("\n  Attribute type: 0x{:08X}", attr_type);
                println!("  Attribute length: {} bytes", attr_len);
                println!("  Non-resident: {}", if non_resident == 1 { "Yes" } else { "No" });
                
                if attr_type == ATTR_TYPE_DATA {
                    found_data = true;
                    println!("  ✓ Found DATA attribute!");
                    
                    if non_resident == 1 {
                        // Read non-resident fields
                        let start_vcn = u64::from_le_bytes([
                            record[offset + 16],
                            record[offset + 17],
                            record[offset + 18],
                            record[offset + 19],
                            record[offset + 20],
                            record[offset + 21],
                            record[offset + 22],
                            record[offset + 23],
                        ]);
                        
                        let end_vcn = u64::from_le_bytes([
                            record[offset + 24],
                            record[offset + 25],
                            record[offset + 26],
                            record[offset + 27],
                            record[offset + 28],
                            record[offset + 29],
                            record[offset + 30],
                            record[offset + 31],
                        ]);
                        
                        println!("    Start VCN: {}", start_vcn);
                        println!("    End VCN: {}", end_vcn);
                        println!("    Clusters: {}", end_vcn - start_vcn + 1);
                    }
                }
                
                offset += attr_len as usize;
            }
            
            if found_data {
                println!("\n✓ MFT DATA attribute is properly formatted!");
            } else {
                println!("\n✗ MFT DATA attribute not found!");
            }
        }
        Err(e) => {
            println!("✗ Failed to create MFT record: {}", e);
        }
    }
}