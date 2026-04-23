// ======================================================================
// DEVICE MANAGER - FULL ADVANCED VERSION
// File: src/system/devices.rs
// Description: Complete access to USB, cameras, microphones, Bluetooth,
//              serial ports, storage devices, network interfaces, and all
//              peripherals (powered or not). ZERO LIMITATIONS.
// ======================================================================

use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{info, warn};
use serde::{Serialize, Deserialize};
use std::process::Command;

// ======================================================================
// DEVICE TYPES
// ======================================================================

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
    pub interfaces: Vec<USBInterface>,
    pub power_ma: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct USBInterface {
    pub number: u8,
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,
    pub driver: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraDevice {
    pub name: String,
    pub device_path: PathBuf,
    pub resolutions: Vec<CameraResolution>,
    pub formats: Vec<String>,
    pub is_available: bool,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraResolution {
    pub width: u32,
    pub height: u32,
    pub fps: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrophoneDevice {
    pub name: String,
    pub device_path: PathBuf,
    pub is_available: bool,
    pub is_muted: bool,
    pub volume: u8,
    pub channels: u8,
    pub sample_rates: Vec<u32>,
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
    pub manufacturer: Option<String>,
    pub firmware_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialDevice {
    pub name: String,
    pub device_path: PathBuf,
    pub baud_rate: Option<u32>,
    pub vendor: Option<String>,
    pub product: Option<String>,
    pub is_open: bool,
    pub available_baud_rates: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageDevice {
    pub name: String,
    pub device_path: PathBuf,
    pub mount_point: Option<PathBuf>,
    pub size_bytes: u64,
    pub size_human: String,
    pub used_bytes: u64,
    pub used_human: String,
    pub available_bytes: u64,
    pub available_human: String,
    pub filesystem: String,
    pub is_mounted: bool,
    pub is_removable: bool,
    pub uuid: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub mac_address: Option<String>,
    pub ip_addresses: Vec<IPAddress>,
    pub is_up: bool,
    pub is_wireless: bool,
    pub speed: Option<String>,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub tx_packets: u64,
    pub rx_packets: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IPAddress {
    pub address: String,
    pub netmask: Option<String>,
    pub broadcast: Option<String>,
    pub is_ipv6: bool,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GPUDevice {
    pub name: String,
    pub vendor: String,
    pub device_path: Option<PathBuf>,
    pub memory_total_mb: Option<u64>,
    pub memory_used_mb: Option<u64>,
    pub temperature_c: Option<f32>,
    pub utilization_percent: Option<u8>,
    pub driver_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub name: String,
    pub card_id: u32,
    pub device_id: u32,
    pub device_path: PathBuf,
    pub is_input: bool,
    pub is_output: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllDevices {
    pub usb: Vec<USBDevice>,
    pub cameras: Vec<CameraDevice>,
    pub microphones: Vec<MicrophoneDevice>,
    pub bluetooth: Vec<BluetoothDevice>,
    pub serial: Vec<SerialDevice>,
    pub storage: Vec<StorageDevice>,
    pub network: Vec<NetworkInterface>,
    pub gpu: Vec<GPUDevice>,
    pub audio: Vec<AudioDevice>,
    pub unpowered: Vec<String>,
}

// ======================================================================
// DEVICE MANAGER
// ======================================================================

pub struct DeviceManager {
    cache: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    cache_ttl: Duration,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            cache_ttl: Duration::from_secs(5),
        }
    }
    
    pub async fn get_all_devices(&self) -> Result<AllDevices> {
        Ok(AllDevices {
            usb: self.list_usb_devices().await?,
            cameras: self.list_cameras().await?,
            microphones: self.list_microphones().await?,
            bluetooth: self.list_bluetooth_devices().await?,
            serial: self.list_serial_ports().await?,
            storage: self.list_storage_devices().await?,
            network: self.list_network_interfaces().await?,
            gpu: self.list_gpu_devices().await?,
            audio: self.list_audio_devices().await?,
            unpowered: self.detect_unpowered_devices().await?,
        })
    }
    
    // ==================================================================
    // USB DEVICES
    // ==================================================================
    
    pub async fn list_usb_devices(&self) -> Result<Vec<USBDevice>> {
        let mut devices = Vec::new();
        
        #[cfg(target_os = "linux")]
        {
            let output = Command::new("lsusb").output()?;
            let output_str = String::from_utf8_lossy(&output.stdout);
            
            for line in output_str.lines() {
                if let Some(parsed) = self.parse_lsusb_line(line) {
                    devices.push(parsed);
                }
            }
            
            let sysfs_path = PathBuf::from("/sys/bus/usb/devices");
            if sysfs_path.exists() {
                for entry in std::fs::read_dir(sysfs_path)? {
                    let entry = entry?;
                    let path = entry.path();
                    
                    let vendor_path = path.join("idVendor");
                    let product_path = path.join("idProduct");
                    
                    if vendor_path.exists() && product_path.exists() {
                        let vendor_id = std::fs::read_to_string(vendor_path)?.trim().to_string();
                        let product_id = std::fs::read_to_string(product_path)?.trim().to_string();
                        
                        if !devices.iter().any(|d| d.vendor_id == vendor_id && d.product_id == product_id) {
                            let manufacturer = std::fs::read_to_string(path.join("manufacturer")).ok().map(|s| s.trim().to_string());
                            let product = std::fs::read_to_string(path.join("product")).ok().map(|s| s.trim().to_string());
                            let serial = std::fs::read_to_string(path.join("serial")).ok().map(|s| s.trim().to_string());
                            
                            let speed = std::fs::read_to_string(path.join("speed")).ok().map(|s| s.trim().to_string());
                            let power_ma = std::fs::read_to_string(path.join("bMaxPower"))
                                .ok().and_then(|s| s.trim().parse::<u32>().ok().map(|p| p * 2));
                            
                            devices.push(USBDevice {
                                vendor_id,
                                product_id,
                                manufacturer,
                                product,
                                serial,
                                bus: "unknown".to_string(),
                                port: path.file_name().unwrap().to_string_lossy().to_string(),
                                speed,
                                device_path: path,
                                is_connected: true,
                                can_power_on: true,
                                interfaces: Vec::new(),
                                power_ma,
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
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 6 {
            let bus = parts[1].to_string();
            let device = parts[3].trim_end_matches(':').to_string();
            let id_part = parts[5];
            
            if let Some((vendor, product)) = id_part.split_once(':') {
                let product_name = parts[6..].join(" ");
                
                Some(USBDevice {
                    vendor_id: vendor.to_string(),
                    product_id: product.to_string(),
                    manufacturer: None,
                    product: Some(product_name),
                    serial: None,
                    bus,
                    port: device,
                    speed: None,
                    device_path: PathBuf::new(),
                    is_connected: true,
                    can_power_on: true,
                    interfaces: Vec::new(),
                    power_ma: None,
                })
            } else {
                None
            }
        } else {
            None
        }
    }
    
    pub async fn reset_usb_device(&self, vendor_id: &str, product_id: &str) -> Result<()> {
        info!("Resetting USB device: {}:{}", vendor_id, product_id);
        
        #[cfg(target_os = "linux")]
        {
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
                            
                            let unbind_path = driver_path.join("unbind");
                            if unbind_path.exists() {
                                std::fs::write(&unbind_path, device_name.as_bytes())?;
                                tokio::time::sleep(Duration::from_secs(2)).await;
                            }
                            
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
            if let Ok(entries) = std::fs::read_dir(video_path) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("video") {
                        let device_path = entry.path();
                        let device_path_str = device_path.to_string_lossy().to_string();
                        
                        let mut capabilities = Vec::new();
                        let output = Command::new("v4l2-ctl")
                            .args(["-d", &device_path_str, "--all"])
                            .output();
                        
                        if let Ok(out) = output {
                            let info = String::from_utf8_lossy(&out.stdout);
                            if info.contains("Video Capture") {
                                capabilities.push("capture".to_string());
                            }
                            if info.contains("Video Output") {
                                capabilities.push("output".to_string());
                            }
                        }
                        
                        cameras.push(CameraDevice {
                            name: name.clone(),
                            device_path,
                            resolutions: Vec::new(),
                            formats: Vec::new(),
                            is_available: true,
                            capabilities,
                        });
                    }
                }
            }
        }
        
        info!("Found {} camera devices", cameras.len());
        Ok(cameras)
    }
    
    pub async fn capture_image(&self, camera: &CameraDevice, output_path: &Path) -> Result<()> {
        info!("Capturing image from {} to {}", camera.name, output_path.display());
        
        #[cfg(target_os = "linux")]
        {
            let device_path_str = camera.device_path.to_string_lossy().to_string();
            let output_path_str = output_path.to_string_lossy().to_string();
            
            let fswebcam_result = Command::new("fswebcam")
                .args(["-d", &device_path_str,
                       "-r", "1280x720",
                       "--no-banner",
                       &output_path_str])
                .output();
            
            if fswebcam_result.is_err() {
                Command::new("ffmpeg")
                    .args(["-f", "v4l2",
                           "-i", &device_path_str,
                           "-vframes", "1",
                           "-y",
                           &output_path_str])
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
            let output = Command::new("pactl")
                .args(["list", "sources", "short"])
                .output();
            
            if let Ok(out) = output {
                let output_str = String::from_utf8_lossy(&out.stdout);
                for line in output_str.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        microphones.push(MicrophoneDevice {
                            name: parts[1..].join(" "),
                            device_path: PathBuf::from(parts[0]),
                            is_available: true,
                            is_muted: false,
                            volume: 100,
                            channels: 2,
                            sample_rates: vec![44100, 48000],
                        });
                    }
                }
            }
        }
        
        Ok(microphones)
    }
    
    pub async fn record_audio(&self, microphone: &MicrophoneDevice, duration_secs: u64, output_path: &Path) -> Result<()> {
        info!("Recording audio for {} seconds to {}", duration_secs, output_path.display());
        
        #[cfg(target_os = "linux")]
        {
            let output_path_str = output_path.to_string_lossy().to_string();
            Command::new("arecord")
                .args(["-d", &duration_secs.to_string(),
                       "-f", "cd",
                       "-D", &microphone.device_path.to_string_lossy(),
                       &output_path_str])
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
            let output = Command::new("bluetoothctl").args(["devices"]).output();
            
            if let Ok(out) = output {
                let output_str = String::from_utf8_lossy(&out.stdout);
                for line in output_str.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 && parts[0] == "Device" {
                        let address = parts[1].to_string();
                        let name = parts[2..].join(" ");
                        
                        let info_output = Command::new("bluetoothctl")
                            .args(["info", &address])
                            .output()?;
                        let info_str = String::from_utf8_lossy(&info_output.stdout);
                        
                        let paired = info_str.contains("Paired: yes");
                        let connected = info_str.contains("Connected: yes");
                        let trusted = info_str.contains("Trusted: yes");
                        
                        let rssi = info_str.lines()
                            .find(|l| l.contains("RSSI:"))
                            .and_then(|l| l.split_whitespace().last()?.parse::<i16>().ok());
                        
                        devices.push(BluetoothDevice {
                            name,
                            address,
                            paired,
                            connected,
                            trusted,
                            rssi,
                            device_class: None,
                            services: Vec::new(),
                            manufacturer: None,
                            firmware_version: None,
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
            Command::new("bluetoothctl").args(["connect", address]).output()?;
        }
        
        Ok(())
    }
    
    pub async fn scan_bluetooth(&self, duration_secs: u64) -> Result<Vec<BluetoothDevice>> {
        info!("Scanning for Bluetooth devices ({}s)...", duration_secs);
        
        #[cfg(target_os = "linux")]
        {
            Command::new("bluetoothctl").args(["scan", "on"]).output()?;
            tokio::time::sleep(Duration::from_secs(duration_secs)).await;
            Command::new("bluetoothctl").args(["scan", "off"]).output()?;
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
            let patterns = ["/dev/ttyUSB*", "/dev/ttyACM*", "/dev/ttyS*", "/dev/ttyAMA*"];
            for pattern in patterns {
                if let Ok(entries) = glob::glob(pattern) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().unwrap().to_string_lossy().to_string();
                        
                        let sysfs_path = PathBuf::from("/sys/class/tty").join(&name);
                        let vendor = std::fs::read_to_string(sysfs_path.join("device/uevent"))
                            .ok()
                            .and_then(|s| s.lines()
                                .find(|l| l.starts_with("PRODUCT="))
                                .map(|l| l[8..].to_string()));
                        
                        ports.push(SerialDevice {
                            name,
                            device_path: entry,
                            baud_rate: None,
                            vendor,
                            product: None,
                            is_open: false,
                            available_baud_rates: vec![9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600],
                        });
                    }
                }
            }
        }
        
        Ok(ports)
    }
    
    pub async fn write_serial(&self, port: &SerialDevice, data: &[u8], baud_rate: u32) -> Result<()> {
        info!("Writing to serial port: {} ({} bytes at {} baud)", port.name, data.len(), baud_rate);
        
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            use std::io::Write;
            
            let mut options = std::fs::OpenOptions::new();
            options.read(true).write(true);
            
            let mut port_file = options.open(&port.device_path)?;
            
            let device_path_str = port.device_path.to_string_lossy().to_string();
            Command::new("stty")
                .args(["-F", &device_path_str, 
                       &baud_rate.to_string(), "raw", "-echo"])
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
            let mounts = std::fs::read_to_string("/proc/mounts")?;
            for line in mounts.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    let device = parts[0];
                    let mount_point = parts[1];
                    let fs_type = parts[2];
                    
                    if device.starts_with("/dev/") {
                        let df_output = Command::new("df")
                            .args(["-B1", mount_point])
                            .output();
                        
                        if let Ok(out) = df_output {
                            let df_str = String::from_utf8_lossy(&out.stdout);
                            for df_line in df_str.lines().skip(1) {
                                let df_parts: Vec<&str> = df_line.split_whitespace().collect();
                                if df_parts.len() >= 6 {
                                    let size_bytes = df_parts[1].parse::<u64>().unwrap_or(0);
                                    let used_bytes = df_parts[2].parse::<u64>().unwrap_or(0);
                                    let available_bytes = df_parts[3].parse::<u64>().unwrap_or(0);
                                    
                                    let blkid_output = Command::new("blkid").arg(device).output();
                                    
                                    let (uuid, label) = if let Ok(out) = blkid_output {
                                        let info = String::from_utf8_lossy(&out.stdout);
                                        let uuid_val = info.split("UUID=\"").nth(1).and_then(|s| s.split('"').next());
                                        let label_val = info.split("LABEL=\"").nth(1).and_then(|s| s.split('"').next());
                                        (uuid_val.map(String::from), label_val.map(String::from))
                                    } else {
                                        (None, None)
                                    };
                                    
                                    devices.push(StorageDevice {
                                        name: device.to_string(),
                                        device_path: PathBuf::from(device),
                                        mount_point: Some(PathBuf::from(mount_point)),
                                        size_bytes,
                                        size_human: Self::format_bytes(size_bytes),
                                        used_bytes,
                                        used_human: Self::format_bytes(used_bytes),
                                        available_bytes,
                                        available_human: Self::format_bytes(available_bytes),
                                        filesystem: fs_type.to_string(),
                                        is_mounted: true,
                                        is_removable: device.contains("sd") && !device.contains("nvme"),
                                        uuid,
                                        label,
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
    
    fn format_bytes(bytes: u64) -> String {
        const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
        let mut size = bytes as f64;
        let mut unit_idx = 0;
        
        while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
            size /= 1024.0;
            unit_idx += 1;
        }
        
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
    
    pub async fn mount_device(&self, device: &str, mount_point: &Path) -> Result<()> {
        info!("Mounting {} to {}", device, mount_point.display());
        std::fs::create_dir_all(mount_point)?;
        
        #[cfg(target_os = "linux")]
        {
            Command::new("mount").args([device, mount_point.to_str().unwrap()]).output()?;
        }
        
        Ok(())
    }
    
    pub async fn unmount_device(&self, mount_point: &Path) -> Result<()> {
        info!("Unmounting {}", mount_point.display());
        
        #[cfg(target_os = "linux")]
        {
            Command::new("umount").arg(mount_point.to_str().unwrap()).output()?;
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
            let output = std::process::Command::new("ip").args(["-j", "addr", "show"]).output();
            
            if let Ok(out) = output {
                if let Ok(json) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
                    for iface in json {
                        let name = iface["ifname"].as_str().unwrap_or("").to_string();
                        let name_for_later = name.clone();
                        let is_up = iface["operstate"].as_str() == Some("UP");
                        let mac = iface["address"].as_str().map(String::from);
                        
                        let mut ip_addresses = Vec::new();
                        if let Some(addrs) = iface["addr_info"].as_array() {
                            for addr in addrs {
                                ip_addresses.push(IPAddress {
                                    address: addr["local"].as_str().unwrap_or("").to_string(),
                                    netmask: Some(format!("{}", addr["prefixlen"].as_u64().unwrap_or(0))),
                                    broadcast: addr["broadcast"].as_str().map(String::from),
                                    is_ipv6: addr["family"].as_str() == Some("inet6"),
                                    scope: addr["scope"].as_str().unwrap_or("").to_string(),
                                });
                            }
                        }
                        
                        let stats = Self::get_network_stats(&name).unwrap_or((0, 0, 0, 0));
                        
                        interfaces.push(NetworkInterface {
                            name: name_for_later,
                            mac_address: mac,
                            ip_addresses,
                            is_up,
                            is_wireless: std::path::Path::new(&format!("/sys/class/net/{}/wireless", name)).exists(),
                            speed: std::fs::read_to_string(format!("/sys/class/net/{}/speed", name)).ok().map(|s| format!("{} Mbps", s.trim())),
                            tx_bytes: stats.0,
                            rx_bytes: stats.1,
                            tx_packets: stats.2,
                            rx_packets: stats.3,
                        });
                    }
                }
            }
        }
        
        Ok(interfaces)
    }
    
    fn get_network_stats(interface: &str) -> Result<(u64, u64, u64, u64)> {
        let content = std::fs::read_to_string("/proc/net/dev")?;
        for line in content.lines() {
            if line.contains(interface) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 10 {
                    let rx_bytes = parts[1].parse::<u64>().unwrap_or(0);
                    let rx_packets = parts[2].parse::<u64>().unwrap_or(0);
                    let tx_bytes = parts[9].parse::<u64>().unwrap_or(0);
                    let tx_packets = parts[10].parse::<u64>().unwrap_or(0);
                    return Ok((tx_bytes, rx_bytes, tx_packets, rx_packets));
                }
            }
        }
        Ok((0, 0, 0, 0))
    }
    
    // ==================================================================
    // GPU DEVICES
    // ==================================================================
    
    pub async fn list_gpu_devices(&self) -> Result<Vec<GPUDevice>> {
        let mut devices = Vec::new();
        
        #[cfg(target_os = "linux")]
        {
            let output = Command::new("nvidia-smi")
                .args(["--query-gpu=name,memory.total,memory.used,temperature.gpu,utilization.gpu,driver_version", "--format=csv,noheader"])
                .output();
            
            if let Ok(out) = output {
                let output_str = String::from_utf8_lossy(&out.stdout);
                for line in output_str.lines() {
                    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                    if parts.len() >= 6 {
                        devices.push(GPUDevice {
                            name: parts[0].to_string(),
                            vendor: "NVIDIA".to_string(),
                            device_path: None,
                            memory_total_mb: parts[1].split_whitespace().next().and_then(|s| s.parse().ok()),
                            memory_used_mb: parts[2].split_whitespace().next().and_then(|s| s.parse().ok()),
                            temperature_c: parts[3].split_whitespace().next().and_then(|s| s.parse().ok()),
                            utilization_percent: parts[4].split_whitespace().next().and_then(|s| s.parse().ok()),
                            driver_version: Some(parts[5].to_string()),
                        });
                    }
                }
            }
        }
        
        Ok(devices)
    }
    
    // ==================================================================
    // AUDIO DEVICES
    // ==================================================================
    
    pub async fn list_audio_devices(&self) -> Result<Vec<AudioDevice>> {
        let mut devices = Vec::new();
        
        #[cfg(target_os = "linux")]
        {
            let output = Command::new("aplay").args(["-l"]).output();
            if let Ok(out) = output {
                let output_str = String::from_utf8_lossy(&out.stdout);
                for line in output_str.lines() {
                    if line.starts_with("card") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            let card_str = parts[1].trim_end_matches(':');
                            if let Ok(card_id) = card_str.parse::<u32>() {
                                let name = parts[3..].join(" ");
                                
                                devices.push(AudioDevice {
                                    name,
                                    card_id,
                                    device_id: 0,
                                    device_path: PathBuf::from(format!("/dev/snd/pcmC{}D0p", card_id)),
                                    is_input: false,
                                    is_output: true,
                                });
                            }
                        }
                    }
                }
            }
            
            let output = Command::new("arecord").args(["-l"]).output();
            if let Ok(out) = output {
                let output_str = String::from_utf8_lossy(&out.stdout);
                for line in output_str.lines() {
                    if line.starts_with("card") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            let card_str = parts[1].trim_end_matches(':');
                            if let Ok(card_id) = card_str.parse::<u32>() {
                                let name = parts[3..].join(" ");
                                
                                devices.push(AudioDevice {
                                    name: format!("{} (Capture)", name),
                                    card_id,
                                    device_id: 0,
                                    device_path: PathBuf::from(format!("/dev/snd/pcmC{}D0c", card_id)),
                                    is_input: true,
                                    is_output: false,
                                });
                            }
                        }
                    }
                }
            }
        }
        
        Ok(devices)
    }
    
    // ==================================================================
    // POWERED-OFF DEVICE DETECTION
    // ==================================================================
    
    pub async fn detect_unpowered_devices(&self) -> Result<Vec<String>> {
        let mut devices = Vec::new();
        
        #[cfg(target_os = "linux")]
        {
            let sysfs_path = PathBuf::from("/sys/bus/usb/devices");
            if sysfs_path.exists() {
                for entry in std::fs::read_dir(sysfs_path)? {
                    let entry = entry?;
                    let path = entry.path();
                    
                    let power_path = path.join("power/runtime_status");
                    if power_path.exists() {
                        if let Ok(status) = std::fs::read_to_string(&power_path) {
                            if status.trim() == "suspended" {
                                if let Some(name) = path.file_name() {
                                    devices.push(name.to_string_lossy().to_string());
                                }
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
        info!("Powering on device: {}", device_name);
        
        #[cfg(target_os = "linux")]
        {
            let power_control = PathBuf::from("/sys/bus/usb/devices")
                .join(device_name)
                .join("power/control");
            
            if power_control.exists() {
                std::fs::write(power_control, "on")?;
            }
        }
        
        Ok(())
    }
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for DeviceManager {
    fn clone(&self) -> Self {
        Self {
            cache: Arc::clone(&self.cache),
            cache_ttl: self.cache_ttl,
        }
    }
}

// ======================================================================
// TESTS
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_list_usb_devices() {
        let manager = DeviceManager::new();
        let devices = manager.list_usb_devices().await;
        assert!(devices.is_ok());
    }
    
    #[tokio::test]
    async fn test_list_cameras() {
        let manager = DeviceManager::new();
        let cameras = manager.list_cameras().await;
        assert!(cameras.is_ok());
    }
    
    #[test]
    fn test_format_bytes() {
        assert_eq!(DeviceManager::format_bytes(1024), "1.00 KB");
        assert_eq!(DeviceManager::format_bytes(1048576), "1.00 MB");
        assert_eq!(DeviceManager::format_bytes(1073741824), "1.00 GB");
    }
}