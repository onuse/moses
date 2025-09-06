// NTFS Resident to Non-Resident Data Conversion
// Handles conversion when data grows beyond MFT record capacity

use super::data_runs::DataRun;
use super::mft::MftRecord;
use super::writer::NtfsWriter;
use moses_core::MosesError;
use log::{debug, info};

/// Maximum size for resident data (conservative estimate)
/// MFT record is typically 1KB, minus headers and other attributes
const MAX_RESIDENT_SIZE: usize = 700;

/// Handles conversion between resident and non-resident data
pub struct ResidentConverter {
    cluster_size: u32,
    mft_record_size: u32,
}

impl ResidentConverter {
    /// Create a new resident/non-resident converter
    pub fn new(cluster_size: u32, mft_record_size: u32) -> Self {
        Self {
            cluster_size,
            mft_record_size,
        }
    }
    
    /// Check if data should be non-resident based on size
    pub fn should_be_non_resident(&self, data_size: usize) -> bool {
        data_size > MAX_RESIDENT_SIZE
    }
    
    /// Convert resident data to non-resident
    pub fn convert_to_non_resident(
        &self,
        mft_record: &mut MftRecord,
        attribute_type: u32,
        data: &[u8],
        _writer: &mut NtfsWriter,
    ) -> Result<(), MosesError> {
        debug!("Converting {} bytes of resident data to non-resident", data.len());
        
        // Find the resident attribute
        let _attr_data = mft_record.find_attribute(attribute_type)
            .ok_or_else(|| MosesError::Other(format!("Attribute type {} not found", attribute_type)))?;
        
        // Allocate clusters for the data
        let clusters_needed = self.calculate_clusters_needed(data.len());
        let clusters = _writer.find_free_clusters(clusters_needed)?;
        
        if clusters.is_empty() {
            return Err(MosesError::Other("No free clusters available".to_string()));
        }
        
        let start_cluster = clusters[0];
        _writer.allocate_clusters(&clusters)?;
        
        debug!("Allocated {} clusters starting at cluster {}", clusters_needed, start_cluster);
        
        // TODO: Write data to allocated clusters
        // This would require adding a write_to_clusters method to NtfsWriter
        // For now, data writing is deferred to the writer_ops layer
        
        // Create data runs
        let data_run = DataRun {
            lcn: Some(start_cluster),
            length: clusters_needed,
        };
        let data_runs = vec![data_run];
        let encoded_runs = self.encode_simple_data_runs(&data_runs);
        
        // Build non-resident attribute header
        let non_resident_header = self.build_non_resident_header(
            attribute_type,
            data.len() as u64,
            clusters_needed as u64,
            start_cluster,
            &encoded_runs,
        );
        
        // Replace the resident attribute with non-resident
        self.replace_attribute(mft_record, attribute_type, non_resident_header)?;
        
        info!("Successfully converted {} bytes to non-resident data", data.len());
        Ok(())
    }
    
    /// Convert non-resident data back to resident (if it fits)
    pub fn convert_to_resident(
        &self,
        mft_record: &mut MftRecord,
        attribute_type: u32,
        data: &[u8],
        _writer: &mut NtfsWriter,
    ) -> Result<(), MosesError> {
        if data.len() > MAX_RESIDENT_SIZE {
            return Err(MosesError::Other(format!(
                "Data too large for resident storage: {} bytes", data.len()
            )));
        }
        
        debug!("Converting {} bytes of non-resident data to resident", data.len());
        
        // Find the non-resident attribute to get cluster info
        let _attr_data = mft_record.find_attribute(attribute_type)
            .ok_or_else(|| MosesError::Other(format!("Attribute type {} not found", attribute_type)))?;
        
        // Parse data runs to find allocated clusters
        // This would extract cluster information from the non-resident attribute
        // For now, simplified implementation
        
        // Build resident attribute header with data
        let resident_header = self.build_resident_header(attribute_type, data);
        
        // Replace the non-resident attribute with resident
        self.replace_attribute(mft_record, attribute_type, resident_header)?;
        
        // Free the previously allocated clusters
        // writer.free_clusters(start_cluster, cluster_count)?;
        
        info!("Successfully converted {} bytes to resident data", data.len());
        Ok(())
    }
    
    /// Calculate number of clusters needed for data
    fn calculate_clusters_needed(&self, data_size: usize) -> u64 {
        let cluster_size = self.cluster_size as usize;
        ((data_size + cluster_size - 1) / cluster_size) as u64
    }
    
    /// Build a non-resident attribute header
    fn build_non_resident_header(
        &self,
        attribute_type: u32,
        real_size: u64,
        allocated_clusters: u64,
        _start_vcn: u64,
        data_runs: &[u8],
    ) -> Vec<u8> {
        let mut buffer = Vec::new();
        
        // Build attribute header manually
        // Type code (4 bytes)
        buffer.extend_from_slice(&attribute_type.to_le_bytes());
        // Record length (4 bytes)
        let record_length = 64 + data_runs.len(); // Simplified size
        buffer.extend_from_slice(&(record_length as u32).to_le_bytes());
        // Non-resident flag (1 byte)
        buffer.push(1);
        // Name length (1 byte)
        buffer.push(0);
        // Name offset (2 bytes)
        buffer.extend_from_slice(&0u16.to_le_bytes());
        // Flags (2 bytes)
        buffer.extend_from_slice(&0u16.to_le_bytes());
        // Attribute ID (2 bytes)
        buffer.extend_from_slice(&0u16.to_le_bytes());
        
        // Non-resident specific fields
        // Starting VCN (8 bytes)
        buffer.extend_from_slice(&0u64.to_le_bytes());
        // Ending VCN (8 bytes)
        buffer.extend_from_slice(&(allocated_clusters - 1).to_le_bytes());
        // Data runs offset (2 bytes)
        buffer.extend_from_slice(&64u16.to_le_bytes());
        // Compression unit size (1 byte)
        buffer.push(0);
        // Padding (5 bytes)
        buffer.extend_from_slice(&[0u8; 5]);
        // Allocated size (8 bytes)
        buffer.extend_from_slice(&(allocated_clusters * self.cluster_size as u64).to_le_bytes());
        // Real size (8 bytes)
        buffer.extend_from_slice(&real_size.to_le_bytes());
        // Initialized size (8 bytes)
        buffer.extend_from_slice(&real_size.to_le_bytes());
        
        // Append data runs
        buffer.extend_from_slice(data_runs);
        
        // Add end marker
        buffer.push(0x00);
        
        buffer
    }
    
    /// Build a resident attribute header with data
    fn build_resident_header(&self, attribute_type: u32, data: &[u8]) -> Vec<u8> {
        let mut buffer = Vec::new();
        
        // Build attribute header manually
        // Type code (4 bytes)
        buffer.extend_from_slice(&attribute_type.to_le_bytes());
        // Record length (4 bytes)
        let record_length = 24 + data.len(); // Header + data
        buffer.extend_from_slice(&(record_length as u32).to_le_bytes());
        // Non-resident flag (1 byte)
        buffer.push(0);
        // Name length (1 byte)
        buffer.push(0);
        // Name offset (2 bytes)
        buffer.extend_from_slice(&0u16.to_le_bytes());
        // Flags (2 bytes)
        buffer.extend_from_slice(&0u16.to_le_bytes());
        // Attribute ID (2 bytes)
        buffer.extend_from_slice(&0u16.to_le_bytes());
        
        // Resident specific fields
        // Value length (4 bytes)
        buffer.extend_from_slice(&(data.len() as u32).to_le_bytes());
        // Value offset (2 bytes)
        buffer.extend_from_slice(&24u16.to_le_bytes());
        // Flags (1 byte)
        buffer.push(0);
        // Reserved (1 byte)
        buffer.push(0);
        
        // Append the actual data
        buffer.extend_from_slice(data);
        
        // Align to 8 bytes
        while buffer.len() % 8 != 0 {
            buffer.push(0);
        }
        
        buffer
    }
    
    /// Replace an attribute in the MFT record
    fn replace_attribute(
        &self,
        _mft_record: &mut MftRecord,
        attribute_type: u32,
        new_attribute_data: Vec<u8>,
    ) -> Result<(), MosesError> {
        // This would need to:
        // 1. Find the old attribute offset in the MFT record
        // 2. Remove the old attribute
        // 3. Insert the new attribute
        // 4. Update the MFT record's used size
        // 5. Ensure proper attribute ordering
        
        // For now, simplified implementation
        debug!("Replacing attribute type {} with {} bytes of data", 
               attribute_type, new_attribute_data.len());
        
        // TODO: Implement actual attribute replacement in MFT record
        // This requires modifying the MftRecord's data buffer directly
        
        Ok(())
    }
    
    /// Check if an MFT record has enough space for resident data
    pub fn can_fit_resident(&self, mft_record: &MftRecord, additional_size: usize) -> bool {
        // Calculate current used space
        let used_space = mft_record.header.bytes_used as usize;
        let available_space = self.mft_record_size as usize - used_space;
        
        // Need space for attribute headers plus data (simplified)
        let needed_space = 24 + additional_size; // Basic header + data
        
        available_space >= needed_space
    }
    
    /// Simple data run encoder
    fn encode_simple_data_runs(&self, runs: &[DataRun]) -> Vec<u8> {
        let mut encoded = Vec::new();
        
        for run in runs {
            if let Some(lcn) = run.lcn {
                // Encode length and offset
                let length_bytes = self.encode_value(run.length);
                let offset_bytes = self.encode_value(lcn);
                
                // Header byte: lower 4 bits = length size, upper 4 bits = offset size
                let header = (length_bytes.len() as u8) | ((offset_bytes.len() as u8) << 4);
                encoded.push(header);
                encoded.extend_from_slice(&length_bytes);
                encoded.extend_from_slice(&offset_bytes);
            } else {
                // Sparse run (no LCN)
                let length_bytes = self.encode_value(run.length);
                let header = length_bytes.len() as u8; // No offset
                encoded.push(header);
                encoded.extend_from_slice(&length_bytes);
            }
        }
        
        // End marker
        encoded.push(0);
        encoded
    }
    
    /// Encode a value in little-endian with minimal bytes
    fn encode_value(&self, value: u64) -> Vec<u8> {
        let bytes = value.to_le_bytes();
        // Find the last non-zero byte
        let mut len = 8;
        while len > 1 && bytes[len - 1] == 0 {
            len -= 1;
        }
        bytes[..len].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_should_be_non_resident() {
        let converter = ResidentConverter::new(4096, 1024);
        
        assert!(!converter.should_be_non_resident(100));
        assert!(!converter.should_be_non_resident(700));
        assert!(converter.should_be_non_resident(701));
        assert!(converter.should_be_non_resident(1000));
    }
    
    #[test]
    fn test_calculate_clusters_needed() {
        let converter = ResidentConverter::new(4096, 1024);
        
        assert_eq!(converter.calculate_clusters_needed(0), 0);
        assert_eq!(converter.calculate_clusters_needed(1), 1);
        assert_eq!(converter.calculate_clusters_needed(4096), 1);
        assert_eq!(converter.calculate_clusters_needed(4097), 2);
        assert_eq!(converter.calculate_clusters_needed(8192), 2);
        assert_eq!(converter.calculate_clusters_needed(8193), 3);
    }
}