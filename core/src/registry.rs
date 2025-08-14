use crate::FilesystemFormatter;
use std::collections::HashMap;
use std::sync::Arc;

pub struct FormatterRegistry {
    formatters: HashMap<String, Arc<dyn FilesystemFormatter>>,
}

impl FormatterRegistry {
    pub fn new() -> Self {
        Self {
            formatters: HashMap::new(),
        }
    }
    
    pub fn register(&mut self, name: String, formatter: Arc<dyn FilesystemFormatter>) {
        self.formatters.insert(name, formatter);
    }
    
    pub fn get_formatter(&self, name: &str) -> Option<Arc<dyn FilesystemFormatter>> {
        self.formatters.get(name).cloned()
    }
    
    pub fn list_formatters(&self) -> Vec<String> {
        self.formatters.keys().cloned().collect()
    }
    
    pub fn is_supported(&self, filesystem: &str) -> bool {
        self.formatters.contains_key(filesystem)
    }
}

impl Default for FormatterRegistry {
    fn default() -> Self {
        Self::new()
    }
}