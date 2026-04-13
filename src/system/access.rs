// ======================================================================
// SYSTEM ACCESS - Complete device and file system control
// File: src/system/access.rs
// Description: Full read/write/execute access to entire system
//              All connected devices, powered or not
// ======================================================================

use anyhow::{Result, anyhow, Context};
use std::path::{Path, PathBuf};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use tokio::fs as tokio_fs;
use tracing::{info, warn, error, debug};
use walkdir::WalkDir;
use std::sync::Arc;

use super::permission::PermissionManager;
use crate::learning::ComprehensiveLogger;

pub struct SystemAccess {
    permission_manager: Arc<PermissionManager>,
    logger: Arc<ComprehensiveLogger>,
}

impl SystemAccess {
    pub fn new(
        permission_manager: Arc<PermissionManager>,
        logger: Arc<ComprehensiveLogger>,
    ) -> Self {
        Self {
            permission_manager,
            logger,
        }
    }
    
    // ==================================================================
    // FILE SYSTEM OPERATIONS
    // ==================================================================
    
    pub async fn read_file(&self, path: &Path) -> Result<Vec<u8>> {
        if !self.permission_manager.request_file_access(path, "read").await? {
            return Err(anyhow!("Permission denied: read {}", path.display()));
        }
        
        info!("Reading file: {}", path.display());
        self.logger.log_file_read(path, 0).await;
        
        let content = tokio_fs::read(path).await
            .with_context(|| format!("Failed to read file: {}", path.display()))?;
        
        self.logger.log_file_read(path, content.len() as u64).await;
        Ok(content)
    }
    
    pub async fn read_file_string(&self, path: &Path) -> Result<String> {
        let bytes = self.read_file(path).await?;
        String::from_utf8(bytes)
            .map_err(|e| anyhow!("File is not valid UTF-8: {}", e))
    }
    
    pub async fn write_file(&self, path: &Path, content: &[u8]) -> Result<()> {
        if !self.permission_manager.request_file_access(path, "write").await? {
            return Err(anyhow!("Permission denied: write {}", path.display()));
        }
        
        info!("Writing file: {} ({} bytes)", path.display(), content.len());
        
        if let Some(parent) = path.parent() {
            tokio_fs::create_dir_all(parent).await?;
        }
        
        tokio_fs::write(path, content).await
            .with_context(|| format!("Failed to write file: {}", path.display()))?;
        
        self.logger.log_file_write(path, content.len() as u64).await;
        Ok(())
    }
    
    pub async fn write_file_string(&self, path: &Path, content: &str) -> Result<()> {
        self.write_file(path, content.as_bytes()).await
    }
    
    pub async fn append_file(&self, path: &Path, content: &[u8]) -> Result<()> {
        if !self.permission_manager.request_file_access(path, "write").await? {
            return Err(anyhow!("Permission denied: append {}", path.display()));
        }
        
        use std::io::Write;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        
        file.write_all(content)?;
        Ok(())
    }
    
    pub async fn delete_file(&self, path: &Path) -> Result<()> {
        if !self.permission_manager.request_file_access(path, "delete").await? {
            return Err(anyhow!("Permission denied: delete {}", path.display()));
        }
        
        info!("Deleting file: {}", path.display());
        tokio_fs::remove_file(path).await
            .with_context(|| format!("Failed to delete file: {}", path.display()))?;
        Ok(())
    }
    
    pub async fn delete_directory(&self, path: &Path) -> Result<()> {
        if !self.permission_manager.request_file_access(path, "delete").await? {
            return Err(anyhow!("Permission denied: delete directory {}", path.display()));
        }
        
        info!("Deleting directory: {}", path.display());
        tokio_fs::remove_dir_all(path).await
            .with_context(|| format!("Failed to delete directory: {}", path.display()))?;
        Ok(())
    }
    
    pub async fn list_directory(&self, path: &Path) -> Result<Vec<PathBuf>> {
        if !self.permission_manager.request_file_access(path, "list").await? {
            return Err(anyhow!("Permission denied: list {}", path.display()));
        }
        
        let mut entries = Vec::new();
        let mut read_dir = tokio_fs::read_dir(path).await?;
        
        while let Some(entry) = read_dir.next_entry().await? {
            entries.push(entry.path());
        }
        
        Ok(entries)
    }
    
    pub async fn list_directory_recursive(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let mut entries = Vec::new();
        for entry in WalkDir::new(path).follow_links(true) {
            let entry = entry?;
            entries.push(entry.path().to_path_buf());
        }
        Ok(entries)
    }
    
    pub async fn create_directory(&self, path: &Path) -> Result<()> {
        tokio_fs::create_dir_all(path).await?;
        Ok(())
    }
    
    pub async fn copy_file(&self, from: &Path, to: &Path) -> Result<()> {
        if !self.permission_manager.request_file_access(from, "read").await? {
            return Err(anyhow!("Permission denied: read {}", from.display()));
        }
        if !self.permission_manager.request_file_access(to, "write").await? {
            return Err(anyhow!("Permission denied: write {}", to.display()));
        }
        
        info!("Copying {} to {}", from.display(), to.display());
        tokio_fs::copy(from, to).await?;
        Ok(())
    }
    
    pub async fn move_file(&self, from: &Path, to: &Path) -> Result<()> {
        info!("Moving {} to {}", from.display(), to.display());
        tokio_fs::rename(from, to).await?;
        Ok(())
    }
    
    pub async fn file_exists(&self, path: &Path) -> bool {
        tokio_fs::try_exists(path).await.unwrap_or(false)
    }
    
    pub async fn get_file_metadata(&self, path: &Path) -> Result<FileMetadata> {
        let metadata = tokio_fs::metadata(path).await?;
        
        Ok(FileMetadata {
            size: metadata.len(),
            is_file: metadata.is_file(),
            is_dir: metadata.is_dir(),
            is_symlink: metadata.is_symlink(),
            permissions: metadata.permissions().mode(),
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
            accessed: metadata.accessed().ok(),
        })
    }
    
    // ==================================================================
    // SYSTEM INFORMATION
    // ==================================================================
    
    pub async fn get_system_info(&self) -> Result<SystemInfo> {
        let mut info = SystemInfo::default();
        
        // Hostname
        info.hostname = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "unknown".to_string());
        
        // OS Info
        info.os = std::env::consts::OS.to_string();
        info.os_family = std::env::consts::FAMILY.to_string();
        info.arch = std::env::consts::ARCH.to_string();
        
        // Current user
        info.current_user = whoami::username();
        
        // Home directory
        if let Some(home) = dirs::home_dir() {
            info.home_dir = home;
        }
        
        // Current directory
        info.current_dir = std::env::current_dir().unwrap_or_default();
        
        // Environment variables
        for (key, value) in std::env::vars() {
            // Skip sensitive values
            if key.to_lowercase().contains("key") 
                || key.to_lowercase().contains("secret") 
                || key.to_lowercase().contains("password")
                || key.to_lowercase().contains("token") {
                info.env_vars.insert(key, "***REDACTED***".to_string());
            } else {
                info.env_vars.insert(key, value);
            }
        }
        
        Ok(info)
    }
    
    // ==================================================================
    // PROCESS MANAGEMENT
    // ==================================================================
    
    pub async fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
        let mut processes = Vec::new();
        
        #[cfg(target_os = "linux")]
        {
            let output = Command::new("ps")
                .args(["-eo", "pid,ppid,user,comm,%cpu,%mem,etime"])
                .output()?;
            
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 7 {
                    processes.push(ProcessInfo {
                        pid: parts[0].parse().unwrap_or(0),
                        ppid: parts[1].parse().unwrap_or(0),
                        user: parts[2].to_string(),
                        name: parts[3].to_string(),
                        cpu_percent: parts[4].parse().unwrap_or(0.0),
                        mem_percent: parts[5].parse().unwrap_or(0.0),
                        elapsed: parts[6].to_string(),
                    });
                }
            }
        }
        
        Ok(processes)
    }
    
    pub async fn kill_process(&self, pid: u32) -> Result<()> {
        info!("Killing process: {}", pid);
        
        #[cfg(target_os = "linux")]
        {
            Command::new("kill").arg("-9").arg(pid.to_string()).output()?;
        }
        
        Ok(())
    }
    
    // ==================================================================
    // DISK INFORMATION
    // ==================================================================
    
    pub async fn get_disk_info(&self) -> Result<Vec<DiskInfo>> {
        let mut disks = Vec::new();
        
        #[cfg(target_os = "linux")]
        {
            let output = Command::new("df").arg("-h").output()?;
            let output_str = String::from_utf8_lossy(&output.stdout);
            
            for line in output_str.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 6 {
                    disks.push(DiskInfo {
                        filesystem: parts[0].to_string(),
                        size: parts[1].to_string(),
                        used: parts[2].to_string(),
                        available: parts[3].to_string(),
                        use_percent: parts[4].to_string(),
                        mount_point: PathBuf::from(parts[5]),
                    });
                }
            }
        }
        
        Ok(disks)
    }
    
    // ==================================================================
    // COMMAND EXECUTION
    // ==================================================================
    
    pub async fn execute_command(&self, command: &str, args: &[&str]) -> Result<CommandResult> {
        info!("Executing command: {} {:?}", command, args);
        
        let output = Command::new(command)
            .args(args)
            .output()
            .with_context(|| format!("Failed to execute: {}", command))?;
        
        Ok(CommandResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
            success: output.status.success(),
        })
    }
    
    pub async fn execute_shell(&self, script: &str) -> Result<CommandResult> {
        info!("Executing shell script");
        
        let output = if cfg!(target_os = "windows") {
            Command::new("cmd").args(["/C", script]).output()?
        } else {
            Command::new("sh").args(["-c", script]).output()?
        };
        
        Ok(CommandResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
            success: output.status.success(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub size: u64,
    pub is_file: bool,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub permissions: u32,
    pub modified: Option<std::time::SystemTime>,
    pub created: Option<std::time::SystemTime>,
    pub accessed: Option<std::time::SystemTime>,
}

#[derive(Debug, Clone, Default)]
pub struct SystemInfo {
    pub hostname: String,
    pub os: String,
    pub os_family: String,
    pub arch: String,
    pub current_user: String,
    pub home_dir: PathBuf,
    pub current_dir: PathBuf,
    pub env_vars: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub user: String,
    pub name: String,
    pub cpu_percent: f32,
    pub mem_percent: f32,
    pub elapsed: String,
}

#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub filesystem: String,
    pub size: String,
    pub used: String,
    pub available: String,
    pub use_percent: String,
    pub mount_point: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}
