use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub device_type: DeviceType,
    pub mount_points: Vec<PathBuf>,
    pub is_removable: bool,
    pub is_system: bool,
    pub filesystem: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeviceType {
    HardDisk,
    SSD,
    USB,
    SDCard,
    OpticalDrive,
    Virtual,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device: Device,
    pub filesystem: Option<String>,
    pub label: Option<String>,
    pub used_space: Option<u64>,
    pub free_space: Option<u64>,
    pub partitions: Vec<Partition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partition {
    pub id: String,
    pub size: u64,
    pub filesystem: Option<String>,
    pub mount_point: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PermissionLevel {
    ReadOnly,
    Simulate,
    FullAccess,
}

#[async_trait::async_trait]
pub trait DeviceManager: Send + Sync {
    async fn enumerate_devices(&self) -> Result<Vec<Device>, crate::MosesError>;
    async fn get_device_by_id(&self, device_id: &str) -> Result<Option<Device>, crate::MosesError>;
    async fn get_device_info(&self, device: &Device) -> Result<DeviceInfo, crate::MosesError>;
    async fn is_safe_to_format(&self, device: &Device) -> Result<bool, crate::MosesError>;
    async fn check_permissions(&self, device: &Device) -> Result<PermissionLevel, crate::MosesError>;
}