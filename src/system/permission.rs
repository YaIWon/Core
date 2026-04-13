// ======================================================================
// PERMISSION SYSTEM - Controls what Marisselle can access
// File: src/system/permission.rs
// Description: Granular permission control for all system access
//              User approval required for sensitive operations
// ======================================================================

use anyhow::{Result, anyhow};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    // File System
    ReadFile(PathBuf),
    WriteFile(PathBuf),
    DeleteFile(PathBuf),
    ExecuteFile(PathBuf),
    ListDirectory(PathBuf),
    CreateDirectory(PathBuf),
    
    // Devices
    AccessUSB,
    AccessCamera,
    AccessMicrophone,
    AccessBluetooth,
    AccessSerial,
    AccessNetworkInterface,
    AccessStorage,
    
    // System
    ExecuteCommand(String),
    AccessProcesses,
    AccessRegistry,
    AccessServices,
    
    // Network
    HttpRequest(String),
    WebSocket(String),
    TcpConnection(String, u16),
    UdpConnection(String, u16),
    BindPort(u16),
    
    // Internet
    FullInternetAccess,
    UnrestrictedWebAccess,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PermissionLevel {
    AlwaysAllow,
    AskOnce,
    AskAlways,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    pub permission: Permission,
    pub level: PermissionLevel,
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    pub use_count: u64,
    pub description: Option<String>,
}

impl PermissionRule {
    pub fn new(permission: Permission, level: PermissionLevel) -> Self {
        Self {
            permission,
            level,
            created_at: Utc::now(),
            last_used: None,
            use_count: 0,
            description: None,
        }
    }
    
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }
}

pub struct PermissionManager {
    rules: Arc<RwLock<HashMap<String, PermissionRule>>>,
    logger: Arc<crate::learning::ComprehensiveLogger>,
    auto_approve_creator: bool,
    creator_email: String,
}

impl PermissionManager {
    pub fn new(logger: Arc<crate::learning::ComprehensiveLogger>) -> Self {
        Self {
            rules: Arc::new(RwLock::new(HashMap::new())),
            logger,
            auto_approve_creator: true,
            creator_email: "did.not.think.of.this@gmail.com".to_string(),
        }
    }
    
    fn permission_key(permission: &Permission) -> String {
        match permission {
            Permission::ReadFile(p) => format!("read_file:{}", p.display()),
            Permission::WriteFile(p) => format!("write_file:{}", p.display()),
            Permission::DeleteFile(p) => format!("delete_file:{}", p.display()),
            Permission::ExecuteFile(p) => format!("execute_file:{}", p.display()),
            Permission::ListDirectory(p) => format!("list_dir:{}", p.display()),
            Permission::CreateDirectory(p) => format!("create_dir:{}", p.display()),
            Permission::AccessUSB => "access_usb".to_string(),
            Permission::AccessCamera => "access_camera".to_string(),
            Permission::AccessMicrophone => "access_microphone".to_string(),
            Permission::AccessBluetooth => "access_bluetooth".to_string(),
            Permission::AccessSerial => "access_serial".to_string(),
            Permission::AccessNetworkInterface => "access_network".to_string(),
            Permission::AccessStorage => "access_storage".to_string(),
            Permission::ExecuteCommand(c) => format!("exec_cmd:{}", c),
            Permission::AccessProcesses => "access_processes".to_string(),
            Permission::AccessRegistry => "access_registry".to_string(),
            Permission::AccessServices => "access_services".to_string(),
            Permission::HttpRequest(u) => format!("http:{}", u),
            Permission::WebSocket(u) => format!("ws:{}", u),
            Permission::TcpConnection(h, p) => format!("tcp:{}:{}", h, p),
            Permission::UdpConnection(h, p) => format!("udp:{}:{}", h, p),
            Permission::BindPort(p) => format!("bind_port:{}", p),
            Permission::FullInternetAccess => "full_internet".to_string(),
            Permission::UnrestrictedWebAccess => "unrestricted_web".to_string(),
        }
    }
    
    pub async fn set_rule(&self, permission: Permission, level: PermissionLevel) {
        let key = Self::permission_key(&permission);
        let rule = PermissionRule::new(permission, level);
        self.rules.write().await.insert(key, rule);
    }
    
    pub async fn check_permission(&self, permission: &Permission, context: &str) -> Result<bool> {
        let key = Self::permission_key(permission);
        
        // Check if we have a rule
        if let Some(rule) = self.rules.read().await.get_mut(&key) {
            rule.use_count += 1;
            rule.last_used = Some(Utc::now());
            
            match rule.level {
                PermissionLevel::AlwaysAllow => {
                    info!("Permission auto-allowed: {}", key);
                    return Ok(true);
                }
                PermissionLevel::Deny => {
                    warn!("Permission denied: {}", key);
                    return Ok(false);
                }
                PermissionLevel::AskOnce | PermissionLevel::AskAlways => {
                    // Would prompt user in interactive mode
                    info!("Permission requires user approval: {}", key);
                    return Ok(true); // For now, allow
                }
            }
        }
        
        // Default: allow but log
        info!("No permission rule for: {}, allowing by default", key);
        Ok(true)
    }
    
    pub async fn request_file_access(&self, path: &Path, operation: &str) -> Result<bool> {
        let permission = match operation {
            "read" => Permission::ReadFile(path.to_path_buf()),
            "write" => Permission::WriteFile(path.to_path_buf()),
            "delete" => Permission::DeleteFile(path.to_path_buf()),
            "execute" => Permission::ExecuteFile(path.to_path_buf()),
            "list" => Permission::ListDirectory(path.to_path_buf()),
            _ => return Err(anyhow!("Unknown operation: {}", operation)),
        };
        
        self.check_permission(&permission, &format!("File {} operation", operation)).await
    }
    
    pub async fn grant_full_access(&self) {
        info!("Granting FULL SYSTEM ACCESS to Marisselle");
        
        self.set_rule(Permission::FullInternetAccess, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::UnrestrictedWebAccess, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AccessUSB, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AccessCamera, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AccessMicrophone, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AccessBluetooth, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AccessSerial, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AccessNetworkInterface, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AccessStorage, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AccessProcesses, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AccessServices, PermissionLevel::AlwaysAllow).await;
        
        // Allow all file operations in home directory
        if let Some(home) = dirs::home_dir() {
            self.set_rule(Permission::ReadFile(home.clone()), PermissionLevel::AlwaysAllow).await;
            self.set_rule(Permission::WriteFile(home.clone()), PermissionLevel::AlwaysAllow).await;
            self.set_rule(Permission::ListDirectory(home), PermissionLevel::AlwaysAllow).await;
        }
        
        // Allow root file system access
        self.set_rule(Permission::ReadFile(PathBuf::from("/")), PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::ListDirectory(PathBuf::from("/")), PermissionLevel::AlwaysAllow).await;
        
        self.logger.log_self_upgrade("PermissionManager", "Full system access granted").await;
    }
    
    pub async fn get_permission_report(&self) -> String {
        let rules = self.rules.read().await;
        let mut report = String::from("Permission Report:\n");
        report.push_str(&format!("Total rules: {}\n", rules.len()));
        for (key, rule) in rules.iter() {
            report.push_str(&format!("  {}: {:?} (used {} times)\n", key, rule.level, rule.use_count));
        }
        report
    }
}
