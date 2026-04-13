// ======================================================================
// DEVICE MANAGER - All connected devices (powered or not)
// File: src/system/devices.rs
// Description: Full access to USB, cameras, microphones, Bluetooth,
//              serial ports, storage devices, and all peripherals
// ======================================================================

use anyhow::{Result, anyhow};
use std::path::PathBuf;
use std::collections::HashMap;
use tracing::{info, warn, error, debug};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct USBDevice {
    pub vendor_id: String,
    pub product_id: String,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
    pub bus: String,
    pub port: String,
    pub speed: Option<String>,
    pub device_path: PathBuf,
    pub is_connected: bool,
    pub can_power_on: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraDevice {
    pub name: String,
    pub device_path: PathBuf,
    pub resolutions: Vec<String>,
    pub formats: Vec<String>,
    pub is_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrophoneDevice {
    pub name: String,
    pub device_path: PathBuf,
    pub is_available: bool,
    pub is_muted: bool,
    pub volume: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothDevice {
    pub name: String,
    pub address: String,
    pub paired: bool,
    pub connected: bool,
    pub trusted: bool,
    pub rssi: Option<i16>,
    pub device_class: Option<String>,
    pub services: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialDevice {
    pub name: String,
    pub device_path: PathBuf,
    pub baud_rate: Option<u32>,
    pub vendor: Option<String>,
    pub product: Option<String>,
    pub is_open: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageDevice {
    pub name: String,
    pub device_path: PathBuf,
    pub mount_point: Option<PathBuf>,
    pub size: String,
    pub used: String,
    pub available: String,
    pub filesystem: String,
    pub is_mounted: bool,
    pub is_removable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub mac_address: Option<String>,
    pub ip_addresses: Vec<String>,
    pub is_up: bool,
    pub is_wireless: bool,
    pub speed: Option<String>,
}

pub struct DeviceManager {
    logger: Arc<crate::learning::ComprehensiveLogger>,
}

impl DeviceManager {
    pub fn new(logger: Arc<crate::learning::ComprehensiveLogger>) -> Self {
        Self { logger }
    }
    
    // ==================================================================
    // USB DEVICES
    // ==================================================================
    
    pub async fn list_usb_devices(&self) -> Result<Vec<USBDevice>> {
        let mut devices = Vec::new();
        
        #[cfg(target_os = "linux")]
        {
            // List USB devices using lsusb
            let output = std::process::Command::new("lsusb").output()?;
            let output_str = String::from_utf8_lossy(&output.stdout);
            
            for line in output_str.lines() {
                // Parse: Bus 001 Device 002: ID 8087:0024 Intel Corp. Integrated Rate Matching Hub
                if let Some(parsed) = self.parse_lsusb_line(line) {
                    devices.push(parsed);
                }
            }
            
            // Also check sysfs for more details
            let sysfs_path = PathBuf::from("/sys/bus/usb/devices");
            if sysfs_path.exists() {
                for entry in std::fs::read_dir(sysfs_path)? {
                    let entry = entry?;
                    let path = entry.path();
                    
                    // Read vendor/product if available
                    let vendor_path = path.join("idVendor");
                    let product_path = path.join("idProduct");
                    let manufacturer_path = path.join("manufacturer");
                    let product_name_path = path.join("product");
                    let serial_path = path.join("serial");
                    
                    if vendor_path.exists() && product_path.exists() {
                        let vendor_id = std::fs::read_to_string(vendor_path)?.trim().to_string();
                        let product_id = std::fs::read_to_string(product_path)?.trim().to_string();
                        let manufacturer = std::fs::read_to_string(manufacturer_path).ok().map(|s| s.trim().to_string());
                        let product = std::fs::read_to_string(product_name_path).ok().map(|s| s.trim().to_string());
                        let serial = std::fs::read_to_string(serial_path).ok().map(|s| s.trim().to_string());
                        
                        // Check if this device is already in our list
                        if !devices.iter().any(|d| d.vendor_id == vendor_id && d.product_id == product_id) {
                            devices.push(USBDevice {
                                vendor_id,
                                product_id,
                                manufacturer,
                                product,
                                serial,
                                bus: "unknown".to_string(),
                                port: "unknown".to_string(),
                                speed: None,
                                device_path: path,
                                is_connected: true,
                                can_power_on: true,
                            });
                        }
                    }
                }
            }
        }
        
        info!("Found {} USB devices", devices.len());
        Ok(devices)
    }
    
    fn parse_lsusb_line(&self, line: &str) -> Option<USBDevice> {
        // Parse format: Bus 001 Device 002: ID 8087:0024 Intel Corp. Integrated Rate Matching Hub
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 6 {
            let bus = parts[1].to_string();
            let device = parts[3].trim_end_matches(':').to_string();
            let id_part = parts[5];
            
            if let Some((vendor, product)) = id_part.split_once(':') {
                let manufacturer_product = parts[6..].join(" ");
                
                return Some(USBDevice {
                    vendor_id: vendor.to_string(),
                    product_id: product.to_string(),
                    manufacturer: None,
                    product: Some(manufacturer_product),
                    serial: None,
                    bus,
                    port: device,
                    speed: None,
                    device_path: PathBuf::new(),
                    is_connected: true,
                    can_power_on: true,
                });
            }
        }
        None
    }
    
    pub async fn reset_usb_device(&self, vendor_id: &str, product_id: &str) -> Result<()> {
        info!("Resetting USB device: {}:{}", vendor_id, product_id);
        
        #[cfg(target_os = "linux")]
        {
            // Find the device in sysfs and unbind/rebind
            let sysfs_path = PathBuf::from("/sys/bus/usb/devices");
            for entry in std::fs::read_dir(sysfs_path)? {
                let entry = entry?;
                let path = entry.path();
                
                let vendor_path = path.join("idVendor");
                let product_path = path.join("idProduct");
                
                if vendor_path.exists() && product_path.exists() {
                    let v_id = std::fs::read_to_string(&vendor_path)?.trim().to_string();
                    let p_id = std::fs::read_to_string(&product_path)?.trim().to_string();
                    
                    if v_id == vendor_id && p_id == product_id {
                        let driver_path = path.join("driver");
                        if driver_path.exists() {
                            let device_name = path.file_name().unwrap().to_string_lossy();
                            
                            // Unbind
                            let unbind_path = driver_path.join("unbind");
                            if unbind_path.exists() {
                                std::fs::write(&unbind_path, device_name.as_bytes())?;
                                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                            }
                            
                            // Rebind
                            let bind_path = driver_path.join("bind");
                            if bind_path.exists() {
                                std::fs::write(&bind_path, device_name.as_bytes())?;
                            }
                        }
                        break;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    // ==================================================================
    // CAMERAS
    // ==================================================================
    
    pub async fn list_cameras(&self) -> Result<Vec<CameraDevice>> {
        let mut cameras = Vec::new();
        
        #[cfg(target_os = "linux")]
        {
            let video_path = PathBuf::from("/dev");
            for entry in std::fs::read_dir(video_path)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("video") {
                    let device_path = entry.path();
                    
                    // Check if device is available
                    let is_available = device_path.exists();
                    
                    cameras.push(CameraDevice {
                        name,
                        device_path,
                        resolutions: vec![],
                        formats: vec![],
                        is_available,
                    });
                }
            }
        }
        
        info!("Found {} camera devices", cameras.len());
        Ok(cameras)
    }
    
    pub async fn capture_image(&self, camera: &CameraDevice, output_path: &PathBuf) -> Result<()> {
        info!("Capturing image from {} to {}", camera.name, output_path.display());
        
        #[cfg(target_os = "linux")]
        {
            // Use fswebcam if available
            let output = std::process::Command::new("fswebcam")
                .args(["-d", &camera.device_path.to_string_lossy(), 
                       "-r", "1280x720",
                       "--no-banner",
                       output_path.to_str().unwrap()])
                .output();
            
            if output.is_err() {
                // Try ffmpeg as fallback
                std::process::Command::new("ffmpeg")
                    .args(["-f", "v4l2", 
                           "-i", &camera.device_path.to_string_lossy(),
                           "-vframes", "1",
                           output_path.to_str().unwrap()])
                    .output()?;
            }
        }
        
        Ok(())
    }
    
    // ==================================================================
    // MICROPHONES
    // ==================================================================
    
    pub async fn list_microphones(&self) -> Result<Vec<MicrophoneDevice>> {
        let mut microphones = Vec::new();
        
        #[cfg(target_os = "linux")]
        {
            // Check pulseaudio for audio devices
            if let Ok(output) = std::process::Command::new("pactl")
                .args(["list", "sources", "short"])
                .output() 
            {
                let output_str = String::from_utf8_lossy(&output.stdout);
                for line in output_str.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        microphones.push(MicrophoneDevice {
                            name: parts[1].to_string(),
                            device_path: PathBuf::from(parts[0]),
                            is_available: true,
                            is_muted: false,
                            volume: 100,
                        });
                    }
                }
            }
        }
        
        Ok(microphones)
    }
    
    pub async fn record_audio(&self, microphone: &MicrophoneDevice, duration_secs: u64, output_path: &PathBuf) -> Result<()> {
        info!("Recording audio for {} seconds to {}", duration_secs, output_path.display());
        
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("arecord")
                .args(["-d", &duration_secs.to_string(),
                       "-f", "cd",
                       output_path.to_str().unwrap()])
                .output()?;
        }
        
        Ok(())
    }
    
    // ==================================================================
    // BLUETOOTH
    // ==================================================================
    
    pub async fn list_bluetooth_devices(&self) -> Result<Vec<BluetoothDevice>> {
        let mut devices = Vec::new();
        
        #[cfg(target_os = "linux")]
        {
            // Use bluetoothctl to list devices
            if let Ok(output) = std::process::Command::new("bluetoothctl")
                .args(["devices"])
                .output()
            {
                let output_str = String::from_utf8_lossy(&output.stdout);
                for line in output_str.lines() {
                    // Format: Device XX:XX:XX:XX:XX:XX Device Name
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 && parts[0] == "Device" {
                        let address = parts[1].to_string();
                        let name = parts[2..].join(" ");
                        
                        // Check if paired/connected
                        let info_output = std::process::Command::new("bluetoothctl")
                            .args(["info", &address])
                            .output()?;
                        let info_str = String::from_utf8_lossy(&info_output.stdout);
                        
                        let paired = info_str.contains("Paired: yes");
                        let connected = info_str.contains("Connected: yes");
                        let trusted = info_str.contains("Trusted: yes");
                        
                        devices.push(BluetoothDevice {
                            name,
                            address,
                            paired,
                            connected,
                            trusted,
                            rssi: None,
                            device_class: None,
                            services: vec![],
                        });
                    }
                }
            }
        }
        
        Ok(devices)
    }
    
    pub async fn connect_bluetooth(&self, address: &str) -> Result<()> {
        info!("Connecting to Bluetooth device: {}", address);
        
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("bluetoothctl")
                .args(["connect", address])
                .output()?;
        }
        
        Ok(())
    }
    
    pub async fn scan_bluetooth(&self) -> Result<Vec<BluetoothDevice>> {
        info!("Scanning for Bluetooth devices...");
        
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("bluetoothctl")
                .args(["scan", "on"])
                .output()?;
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            std::process::Command::new("bluetoothctl")
                .args(["scan", "off"])
                .output()?;
        }
        
        self.list_bluetooth_devices().await
    }
    
    // ==================================================================
    // SERIAL PORTS
    // ==================================================================
    
    pub async fn list_serial_ports(&self) -> Result<Vec<SerialDevice>> {
        let mut ports = Vec::new();
        
        #[cfg(target_os = "linux")]
        {
            let serial_paths = vec!["/dev/ttyUSB*", "/dev/ttyACM*", "/dev/ttyS*"];
            for pattern in serial_paths {
                if let Ok(entries) = glob::glob(pattern) {
                    for entry in entries.flatten() {
                        ports.push(SerialDevice {
                            name: entry.file_name().unwrap().to_string_lossy().to_string(),
                            device_path: entry,
                            baud_rate: None,
                            vendor: None,
                            product: None,
                            is_open: false,
                        });
                    }
                }
            }
        }
        
        Ok(ports)
    }
    
    pub async fn write_serial(&self, port: &SerialDevice, data: &[u8], baud_rate: u32) -> Result<()> {
        info!("Writing to serial port: {} ({} bytes)", port.name, data.len());
        
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            use std::io::Write;
            
            let mut options = std::fs::OpenOptions::new();
            options.read(true).write(true).custom_flags(libc::O_NOCTTY | libc::O_NDELAY);
            
            let mut port_file = options.open(&port.device_path)?;
            
            // Set baud rate using stty
            std::process::Command::new("stty")
                .args(["-F", &port.device_path.to_string_lossy(), &baud_rate.to_string(), "raw"])
                .output()?;
            
            port_file.write_all(data)?;
            port_file.flush()?;
        }
        
        Ok(())
    }
    
    // ==================================================================
    // STORAGE DEVICES
    // ==================================================================
    
    pub async fn list_storage_devices(&self) -> Result<Vec<StorageDevice>> {
        let mut devices = Vec::new();
        
        #[cfg(target_os = "linux")]
        {
            // Check /proc/mounts for mounted devices
            let mounts = std::fs::read_to_string("/proc/mounts")?;
            for line in mounts.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    let device = parts[0];
                    let mount_point = parts[1];
                    let fs_type = parts[2];
                    
                    // Filter for actual storage devices
                    if device.starts_with("/dev/sd") || device.starts_with("/dev/nvme") || device.starts_with("/dev/mmcblk") {
                        // Get size info using df
                        if let Ok(df_output) = std::process::Command::new("df")
                            .args(["-h", mount_point])
                            .output()
                        {
                            let df_str = String::from_utf8_lossy(&df_output.stdout);
                            for df_line in df_str.lines().skip(1) {
                                let df_parts: Vec<&str> = df_line.split_whitespace().collect();
                                if df_parts.len() >= 6 {
                                    devices.push(StorageDevice {
                                        name: device.to_string(),
                                        device_path: PathBuf::from(device),
                                        mount_point: Some(PathBuf::from(mount_point)),
                                        size: df_parts[1].to_string(),
                                        used: df_parts[2].to_string(),
                                        available: df_parts[3].to_string(),
                                        filesystem: fs_type.to_string(),
                                        is_mounted: true,
                                        is_removable: device.contains("sd") && !device.contains("nvme"),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(devices)
    }
    
    pub async fn mount_device(&self, device: &str, mount_point: &PathBuf) -> Result<()> {
        info!("Mounting {} to {}", device, mount_point.display());
        
        std::fs::create_dir_all(mount_point)?;
        
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("mount")
                .args([device, mount_point.to_str().unwrap()])
                .output()?;
        }
        
        Ok(())
    }
    
    pub async fn unmount_device(&self, mount_point: &PathBuf) -> Result<()> {
        info!("Unmounting {}", mount_point.display());
        
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("umount")
                .arg(mount_point.to_str().unwrap())
                .output()?;
        }
        
        Ok(())
    }
    
    // ==================================================================
    // NETWORK INTERFACES
    // ==================================================================
    
    pub async fn list_network_interfaces(&self) -> Result<Vec<NetworkInterface>> {
        let mut interfaces = Vec::new();
        
        #[cfg(target_os = "linux")]
        {
            // Use ip command
            let output = std::process::Command::new("ip").args(["addr", "show"]).output()?;
            let output_str = String::from_utf8_lossy(&output.stdout);
            
            let mut current_interface: Option<NetworkInterface> = None;
            
            for line in output_str.lines() {
                if line.chars().next().map(|c| !c.is_whitespace()).unwrap_or(false) {
                    // New interface
                    if let Some(iface) = current_interface.take() {
                        interfaces.push(iface);
                    }
                    
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let name = parts[1].trim_end_matches(':').to_string();
                        let is_up = line.contains("UP");
                        
                        current_interface = Some(NetworkInterface {
                            name,
                            mac_address: None,
                            ip_addresses: Vec::new(),
                            is_up,
                            is_wireless: false,
                            speed: None,
                        });
                    }
                } else if let Some(ref mut iface) = current_interface {
                    // Parse IP address
                    if line.contains("inet ") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            iface.ip_addresses.push(parts[1].to_string());
                        }
                    }
                    // Parse MAC address
                    if line.contains("link/ether") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            iface.mac_address = Some(parts[1].to_string());
                        }
                    }
                }
            }
            
            if let Some(iface) = current_interface {
                interfaces.push(iface);
            }
        }
        
        Ok(interfaces)
    }
    
    // ==================================================================
    // POWERED-OFF DEVICE DETECTION
    // ==================================================================
    
    pub async fn detect_unpowered_devices(&self) -> Result<Vec<String>> {
        let mut devices = Vec::new();
        
        #[cfg(target_os = "linux")]
        {
            // Check sysfs for devices that are present but not powered
            let sysfs_path = PathBuf::from("/sys/bus/usb/devices");
            if sysfs_path.exists() {
                for entry in std::fs::read_dir(sysfs_path)? {
                    let entry = entry?;
                    let path = entry.path();
                    
                    let power_path = path.join("power").join("runtime_status");
                    if power_path.exists() {
                        let status = std::fs::read_to_string(&power_path)?.trim().to_string();
                        if status == "suspended" {
                            if let Some(name) = path.file_name() {
                                devices.push(name.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }
        
        info!("Found {} unpowered/suspended devices", devices.len());
        Ok(devices)
    }
    
    pub async fn power_on_device(&self, device_name: &str) -> Result<()> {
        info!("Attempting to power on device: {}", device_name);
        
        #[cfg(target_os = "linux")]
        {
            let power_control = PathBuf::from("/sys/bus/usb/devices")
                .join(device_name)
                .join("power")
                .join("control");
            
            if power_control.exists() {
                std::fs::write(power_control, "on")?;
            }
        }
        
        Ok(())
    }
}
