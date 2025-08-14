use moses_core::DeviceManager;
use moses_platform::PlatformDeviceManager;

#[tokio::main]
async fn main() {
    println!("Testing Linux device enumeration...\n");
    
    let manager = PlatformDeviceManager;
    match manager.enumerate_devices().await {
        Ok(devices) => {
            println!("Found {} devices:\n", devices.len());
            for device in devices {
                println!("Device: {}", device.name);
                println!("  ID: {}", device.id);
                println!("  Size: {} GB", device.size / 1_000_000_000);
                println!("  Type: {:?}", device.device_type);
                println!("  Removable: {}", device.is_removable);
                println!("  System: {}", device.is_system);
                println!("  Mount points: {:?}", device.mount_points);
                println!();
            }
        }
        Err(e) => {
            eprintln!("Error enumerating devices: {}", e);
        }
    }
}