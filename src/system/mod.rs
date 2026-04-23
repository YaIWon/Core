// ======================================================================
// SYSTEM ACCESS MODULE - COMPLETE DEVICE CONTROL
// File: src/system/mod.rs
// Description: Full system access for Marisselle LM
//              Files, devices, USB, network, cameras, microphones,
//              Bluetooth, serial ports, and all connected peripherals
// ======================================================================

pub mod access;
pub mod network;
pub mod permission;
pub mod devices;
pub mod commands;

// ======================================================================
// ACCESS RE-EXPORTS
// ======================================================================

pub use access::{
    SystemAccess,
    FileMetadata,
    SystemInfo,
    ProcessInfo,
    DiskInfo,
    CommandResult as SystemCommandResult,
};

// ======================================================================
// NETWORK RE-EXPORTS
// ======================================================================

pub use network::{
    NetworkAccess,
    NetworkConfig,
    ProxyConfig,
    HttpResponse,
    WebSocketConnection,
    DnsRecord,
};

// ======================================================================
// PERMISSION RE-EXPORTS
// ======================================================================

pub use permission::{
    PermissionManager,
    Permission,
    PermissionLevel,
    PermissionRule,
    PermissionEvent,
};

// ======================================================================
// DEVICES RE-EXPORTS
// ======================================================================

pub use devices::{
    DeviceManager,
    USBDevice,
    USBInterface,
    CameraDevice,
    CameraResolution,
    MicrophoneDevice,
    BluetoothDevice,
    SerialDevice,
    StorageDevice,
    NetworkInterface,
    IPAddress,
    GPUDevice,
    AudioDevice,
    AllDevices,
};

// ======================================================================
// COMMANDS RE-EXPORTS
// ======================================================================

pub use commands::{
    CommandExecutor,
    CommandOutput,
    CommandConfig,
    ProcessInfo as CommandProcessInfo,
    ProcessStatus,
    OutputChunk,
    OutputStream,
};

// ======================================================================
// PRELUDE - Commonly used system types
// ======================================================================

pub mod prelude {
    pub use super::access::SystemAccess;
    pub use super::network::NetworkAccess;
    pub use super::devices::DeviceManager;
    pub use super::commands::CommandExecutor;
    pub use super::permission::{PermissionManager, Permission, PermissionLevel};
}

// ======================================================================
// UTILITY FUNCTIONS
// ======================================================================

/// Check if running with root/administrator privileges
pub fn is_root() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// Get system memory info
pub fn get_memory_info() -> (u64, u64, u64) {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        if let Ok(contents) = fs::read_to_string("/proc/meminfo") {
            let mut total = 0;
            let mut available = 0;
            let mut free = 0;
            
            for line in contents.lines() {
                if line.starts_with("MemTotal:") {
                    total = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
                } else if line.starts_with("MemAvailable:") {
                    available = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
                } else if line.starts_with("MemFree:") {
                    free = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
                }
            }
            return (total, available, free);
        }
    }
    (0, 0, 0)
}

/// Get CPU count
pub fn cpu_count() -> usize {
    num_cpus::get()
}

/// Get system uptime in seconds
pub fn system_uptime() -> u64 {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        if let Ok(contents) = fs::read_to_string("/proc/uptime") {
            if let Some(uptime_str) = contents.split_whitespace().next() {
                return uptime_str.parse::<f64>().unwrap_or(0.0) as u64;
            }
        }
    }
    0
}

/// Get hostname
pub fn hostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Get current username
pub fn current_user() -> String {
    whoami::username()
}

/// Get OS information
pub fn os_info() -> String {
    format!("{} {} {}", 
        std::env::consts::OS, 
        std::env::consts::ARCH,
        std::env::consts::FAMILY
    )
}

/// Check if running in a container
pub fn in_container() -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::path::Path;
        Path::new("/.dockerenv").exists() || 
        Path::new("/run/.containerenv").exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Check if running in GitHub Codespaces
pub fn in_codespaces() -> bool {
    std::env::var("CODESPACES").is_ok() || 
    std::env::var("GITHUB_CODESPACE_TOKEN").is_ok()
}

/// Get available disk space for a path
pub fn disk_space(path: &std::path::Path) -> (u64, u64) {
    #[cfg(unix)]
    {
        use nix::sys::statvfs::statvfs;
        if let Ok(stat) = statvfs(path) {
            let total = stat.blocks() as u64 * stat.fragment_size() as u64;
            let available = stat.blocks_available() as u64 * stat.fragment_size() as u64;
            return (total, available);
        }
    }
    (0, 0)
}

// ======================================================================
// SYSTEM STATS STRUCT
// ======================================================================

#[derive(Debug, Clone)]
pub struct SystemStats {
    pub hostname: String,
    pub os: String,
    pub cpu_count: usize,
    pub memory_total_kb: u64,
    pub memory_available_kb: u64,
    pub memory_free_kb: u64,
    pub uptime_seconds: u64,
    pub is_root: bool,
    pub in_container: bool,
    pub in_codespaces: bool,
    pub current_user: String,
}

impl SystemStats {
    pub fn collect() -> Self {
        let (total, available, free) = get_memory_info();
        Self {
            hostname: hostname(),
            os: os_info(),
            cpu_count: cpu_count(),
            memory_total_kb: total,
            memory_available_kb: available,
            memory_free_kb: free,
            uptime_seconds: system_uptime(),
            is_root: is_root(),
            in_container: in_container(),
            in_codespaces: in_codespaces(),
            current_user: current_user(),
        }
    }
    
    pub fn print(&self) {
        println!("=== SYSTEM STATS ===");
        println!("Hostname: {}", self.hostname);
        println!("OS: {}", self.os);
        println!("CPU Count: {}", self.cpu_count);
        println!("Memory Total: {} MB", self.memory_total_kb / 1024);
        println!("Memory Available: {} MB", self.memory_available_kb / 1024);
        println!("Uptime: {} seconds", self.uptime_seconds);
        println!("Root: {}", self.is_root);
        println!("Container: {}", self.in_container);
        println!("Codespaces: {}", self.in_codespaces);
        println!("User: {}", self.current_user);
        println!("====================");
    }
}

// ======================================================================
// TESTS
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hostname() {
        let name = hostname();
        assert!(!name.is_empty());
    }
    
    #[test]
    fn test_current_user() {
        let user = current_user();
        assert!(!user.is_empty());
    }
    
    #[test]
    fn test_os_info() {
        let info = os_info();
        assert!(!info.is_empty());
    }
    
    #[test]
    fn test_system_stats() {
        let stats = SystemStats::collect();
        assert!(!stats.hostname.is_empty());
        assert!(stats.cpu_count > 0);
    }
}