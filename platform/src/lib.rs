#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub use linux::LinuxDeviceManager as PlatformDeviceManager;

#[cfg(target_os = "windows")]
pub use windows::WindowsDeviceManager as PlatformDeviceManager;

#[cfg(target_os = "macos")]
pub use macos::device::MacOSDeviceManager as PlatformDeviceManager;