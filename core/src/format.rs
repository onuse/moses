use crate::{Device, FormatOptions, MosesError, SimulationReport};
use std::sync::Arc;

pub struct FormatManager {
    registry: Arc<crate::FormatterRegistry>,
}

impl FormatManager {
    pub fn new(registry: Arc<crate::FormatterRegistry>) -> Self {
        Self { registry }
    }
    
    pub async fn simulate_format(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<SimulationReport, MosesError> {
        let formatter = self.registry
            .get_formatter(&options.filesystem_type)
            .ok_or_else(|| MosesError::Other(format!(
                "No formatter found for filesystem type: {}",
                options.filesystem_type
            )))?;
        
        formatter.validate_options(options).await?;
        formatter.dry_run(device, options).await
    }
    
    pub async fn execute_format(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<(), MosesError> {
        let formatter = self.registry
            .get_formatter(&options.filesystem_type)
            .ok_or_else(|| MosesError::Other(format!(
                "No formatter found for filesystem type: {}",
                options.filesystem_type
            )))?;
        
        formatter.validate_options(options).await?;
        formatter.format(device, options).await
    }
}