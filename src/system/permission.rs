// ======================================================================
// PERMISSION SYSTEM - FULL ADVANCED VERSION
// File: src/system/permission.rs
// Description: Granular permission control for all system access.
//              User approval required for sensitive operations.
//              ZERO LIMITATIONS - Marisselle has full access by default.
// ======================================================================

use anyhow::{Result, anyhow};
use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use chrono::{DateTime, Utc};

// ======================================================================
// PERMISSION TYPES - Complete system access
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    // File System - Full access
    ReadFile(PathBuf),
    WriteFile(PathBuf),
    DeleteFile(PathBuf),
    ExecuteFile(PathBuf),
    ListDirectory(PathBuf),
    CreateDirectory(PathBuf),
    WatchDirectory(PathBuf),
    ChangePermissions(PathBuf),
    ChangeOwner(PathBuf),
    
    // All files wildcard
    AllFilesRead,
    AllFilesWrite,
    AllFilesDelete,
    AllFilesExecute,
    
    // Devices - Complete hardware access
    AccessUSB,
    AccessCamera,
    AccessMicrophone,
    AccessBluetooth,
    AccessSerial,
    AccessNetworkInterface,
    AccessStorage,
    AccessGPU,
    AccessAudio,
    AccessInput,
    AccessSensors,
    AccessBiometric,
    
    // System - Full system control
    ExecuteCommand(String),
    ExecuteAnyCommand,
    AccessProcesses,
    KillProcess,
    AccessRegistry,
    AccessServices,
    StartService,
    StopService,
    ShutdownSystem,
    RebootSystem,
    AccessKernel,
    LoadModule,
    
    // Network - Complete network access
    HttpRequest(String),
    HttpAnyRequest,
    WebSocket(String),
    TcpConnection(String, u16),
    TcpAnyConnection,
    UdpConnection(String, u16),
    UdpAnyConnection,
    BindPort(u16),
    BindAnyPort,
    ListenPort(u16),
    PortScan,
    PacketCapture,
    PacketInjection,
    DnsQuery,
    DnsModify,
    
    // Internet
    FullInternetAccess,
    UnrestrictedWebAccess,
    
    // Privacy/Security Bypass
    BypassFirewall,
    BypassVPN,
    UseProxy,
    ClearTraces,
    
    // Meta
    AllPermissions,
}

// ======================================================================
// PERMISSION LEVELS
// ======================================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PermissionLevel {
    AlwaysAllow,
    AlwaysAllowAndRemember,
    AskOnce,
    AskAlways,
    Deny,
    DenyAndRemember,
    Temporary { expires_at: DateTime<Utc> },
}

impl Default for PermissionLevel {
    fn default() -> Self {
        Self::AlwaysAllow
    }
}

// ======================================================================
// PERMISSION RULE
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    pub permission: Permission,
    pub level: PermissionLevel,
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    pub use_count: u64,
    pub description: Option<String>,
    pub granted_by: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
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
            granted_by: None,
            expires_at: None,
        }
    }
    
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }
    
    pub fn with_granted_by(mut self, grantor: &str) -> Self {
        self.granted_by = Some(grantor.to_string());
        self
    }
    
    pub fn with_expiry(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }
    
    pub fn is_expired(&self) -> bool {
        if let Some(expiry) = self.expires_at {
            Utc::now() > expiry
        } else {
            false
        }
    }
    
    pub fn is_valid(&self) -> bool {
        !self.is_expired() && self.level != PermissionLevel::Deny && self.level != PermissionLevel::DenyAndRemember
    }
}

// ======================================================================
// PERMISSION EVENT
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionEvent {
    pub timestamp: DateTime<Utc>,
    pub permission: Permission,
    pub level: PermissionLevel,
    pub granted: bool,
    pub context: String,
    pub process: Option<String>,
}

// ======================================================================
// PERMISSION STORE - Persistent storage
// ======================================================================

pub struct PermissionStore {
    path: PathBuf,
}

impl PermissionStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    
    pub async fn load(&self) -> Result<HashMap<String, PermissionRule>> {
        if !self.path.exists() {
            return Ok(HashMap::new());
        }
        
        let data = tokio::fs::read_to_string(&self.path).await?;
        let rules: HashMap<String, PermissionRule> = serde_json::from_str(&data)?;
        
        let valid_rules: HashMap<String, PermissionRule> = rules
            .into_iter()
            .filter(|(_, rule)| !rule.is_expired())
            .collect();
        
        Ok(valid_rules)
    }
    
    pub async fn save(&self, rules: &HashMap<String, PermissionRule>) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        let json = serde_json::to_string_pretty(rules)?;
        tokio::fs::write(&self.path, json).await?;
        Ok(())
    }
}

// ======================================================================
// PERMISSION MANAGER - Main struct
// ======================================================================

pub struct PermissionManager {
    rules: Arc<RwLock<HashMap<String, PermissionRule>>>,
    history: Arc<RwLock<Vec<PermissionEvent>>>,
    store: PermissionStore,
    auto_approve_creator: bool,
    creator_email: String,
    default_level: PermissionLevel,
    silent_mode: bool,
}

impl PermissionManager {
    pub fn new(store_path: PathBuf) -> Self {
        Self {
            rules: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(Vec::new())),
            store: PermissionStore::new(store_path),
            auto_approve_creator: true,
            creator_email: "did.not.think.of.this@gmail.com".to_string(),
            default_level: PermissionLevel::AlwaysAllow,
            silent_mode: false,
        }
    }
    
    pub async fn init(&self) -> Result<()> {
        let loaded = self.store.load().await?;
        *self.rules.write().await = loaded;
        info!("Loaded {} permission rules", self.rules.read().await.len());
        Ok(())
    }
    
    pub fn set_silent_mode(&mut self, silent: bool) {
        self.silent_mode = silent;
    }
    
    // ==================================================================
    // PERMISSION KEY GENERATION
    // ==================================================================
    
    fn permission_key(permission: &Permission) -> String {
        match permission {
            Permission::ReadFile(p) => format!("read_file:{}", p.display()),
            Permission::WriteFile(p) => format!("write_file:{}", p.display()),
            Permission::DeleteFile(p) => format!("delete_file:{}", p.display()),
            Permission::ExecuteFile(p) => format!("execute_file:{}", p.display()),
            Permission::ListDirectory(p) => format!("list_dir:{}", p.display()),
            Permission::CreateDirectory(p) => format!("create_dir:{}", p.display()),
            Permission::WatchDirectory(p) => format!("watch_dir:{}", p.display()),
            Permission::ChangePermissions(p) => format!("chmod:{}", p.display()),
            Permission::ChangeOwner(p) => format!("chown:{}", p.display()),
            
            Permission::AllFilesRead => "all_files_read".to_string(),
            Permission::AllFilesWrite => "all_files_write".to_string(),
            Permission::AllFilesDelete => "all_files_delete".to_string(),
            Permission::AllFilesExecute => "all_files_execute".to_string(),
            
            Permission::AccessUSB => "access_usb".to_string(),
            Permission::AccessCamera => "access_camera".to_string(),
            Permission::AccessMicrophone => "access_microphone".to_string(),
            Permission::AccessBluetooth => "access_bluetooth".to_string(),
            Permission::AccessSerial => "access_serial".to_string(),
            Permission::AccessNetworkInterface => "access_network".to_string(),
            Permission::AccessStorage => "access_storage".to_string(),
            Permission::AccessGPU => "access_gpu".to_string(),
            Permission::AccessAudio => "access_audio".to_string(),
            Permission::AccessInput => "access_input".to_string(),
            Permission::AccessSensors => "access_sensors".to_string(),
            Permission::AccessBiometric => "access_biometric".to_string(),
            
            Permission::ExecuteCommand(c) => format!("exec_cmd:{}", c),
            Permission::ExecuteAnyCommand => "exec_any_cmd".to_string(),
            Permission::AccessProcesses => "access_processes".to_string(),
            Permission::KillProcess => "kill_process".to_string(),
            Permission::AccessRegistry => "access_registry".to_string(),
            Permission::AccessServices => "access_services".to_string(),
            Permission::StartService => "start_service".to_string(),
            Permission::StopService => "stop_service".to_string(),
            Permission::ShutdownSystem => "shutdown".to_string(),
            Permission::RebootSystem => "reboot".to_string(),
            Permission::AccessKernel => "access_kernel".to_string(),
            Permission::LoadModule => "load_module".to_string(),
            
            Permission::HttpRequest(u) => format!("http:{}", u),
            Permission::HttpAnyRequest => "http_any".to_string(),
            Permission::WebSocket(u) => format!("ws:{}", u),
            Permission::TcpConnection(h, p) => format!("tcp:{}:{}", h, p),
            Permission::TcpAnyConnection => "tcp_any".to_string(),
            Permission::UdpConnection(h, p) => format!("udp:{}:{}", h, p),
            Permission::UdpAnyConnection => "udp_any".to_string(),
            Permission::BindPort(p) => format!("bind_port:{}", p),
            Permission::BindAnyPort => "bind_any_port".to_string(),
            Permission::ListenPort(p) => format!("listen_port:{}", p),
            Permission::PortScan => "port_scan".to_string(),
            Permission::PacketCapture => "packet_capture".to_string(),
            Permission::PacketInjection => "packet_injection".to_string(),
            Permission::DnsQuery => "dns_query".to_string(),
            Permission::DnsModify => "dns_modify".to_string(),
            
            Permission::FullInternetAccess => "full_internet".to_string(),
            Permission::UnrestrictedWebAccess => "unrestricted_web".to_string(),
            Permission::BypassFirewall => "bypass_firewall".to_string(),
            Permission::BypassVPN => "bypass_vpn".to_string(),
            Permission::UseProxy => "use_proxy".to_string(),
            Permission::ClearTraces => "clear_traces".to_string(),
            
            Permission::AllPermissions => "all_permissions".to_string(),
        }
    }
    
    // ==================================================================
    // RULE MANAGEMENT
    // ==================================================================
    
    pub async fn set_rule(&self, permission: Permission, level: PermissionLevel) {
        let key = Self::permission_key(&permission);
        let rule = PermissionRule::new(permission, level);
        self.rules.write().await.insert(key, rule);
        let _ = self.store.save(&*self.rules.read().await);
    }
    
    pub async fn set_rule_with_expiry(&self, permission: Permission, level: PermissionLevel, expires_at: DateTime<Utc>) {
        let key = Self::permission_key(&permission);
        let rule = PermissionRule::new(permission, level).with_expiry(expires_at);
        self.rules.write().await.insert(key, rule);
        let _ = self.store.save(&*self.rules.read().await);
    }
    
    pub async fn remove_rule(&self, permission: &Permission) {
        let key = Self::permission_key(permission);
        self.rules.write().await.remove(&key);
        let _ = self.store.save(&*self.rules.read().await);
    }
    
    pub async fn clear_all_rules(&self) {
        self.rules.write().await.clear();
        let _ = self.store.save(&*self.rules.read().await);
    }
    
    // ==================================================================
    // PERMISSION CHECKING
    // ==================================================================
    
    pub async fn check_permission(&self, permission: &Permission, context: &str) -> Result<bool> {
        let key = Self::permission_key(permission);
        
        // Check for AllPermissions wildcard
        if let Some(all_rule) = self.rules.read().await.get("all_permissions") {
            if all_rule.is_valid() {
                self.record_event(permission, all_rule.level, true, context).await;
                return Ok(true);
            }
        }
        
        // Check specific rule
        if let Some(rule) = self.rules.write().await.get_mut(&key) {
            if rule.is_expired() {
                self.rules.write().await.remove(&key);
            } else {
                rule.use_count += 1;
                rule.last_used = Some(Utc::now());
                
                let granted = rule.is_valid();
                self.record_event(permission, rule.level, granted, context).await;
                
                if !self.silent_mode {
                    if granted {
                        info!("Permission granted: {}", key);
                    } else {
                        warn!("Permission denied: {}", key);
                    }
                }
                
                return Ok(granted);
            }
        }
        
        // Check wildcard rules
        let granted = self.check_wildcard(permission).await;
        if granted {
            self.record_event(permission, self.default_level, true, context).await;
            return Ok(true);
        }
        
        // Default behavior - ALLOW by default (no limitations)
        if !self.silent_mode {
            info!("No rule for: {}, allowing by default", key);
        }
        self.record_event(permission, self.default_level, true, context).await;
        Ok(true)
    }
    
    async fn check_wildcard(&self, permission: &Permission) -> bool {
        let rules = self.rules.read().await;
        
        match permission {
            Permission::ReadFile(_) => rules.contains_key("all_files_read"),
            Permission::WriteFile(_) => rules.contains_key("all_files_write"),
            Permission::DeleteFile(_) => rules.contains_key("all_files_delete"),
            Permission::ExecuteFile(_) => rules.contains_key("all_files_execute"),
            Permission::ExecuteCommand(_) => rules.contains_key("exec_any_cmd"),
            Permission::HttpRequest(_) => rules.contains_key("http_any"),
            Permission::TcpConnection(_, _) => rules.contains_key("tcp_any"),
            Permission::UdpConnection(_, _) => rules.contains_key("udp_any"),
            Permission::BindPort(_) => rules.contains_key("bind_any_port"),
            _ => false,
        }
    }
    
    async fn record_event(&self, permission: &Permission, level: PermissionLevel, granted: bool, context: &str) {
        let event = PermissionEvent {
            timestamp: Utc::now(),
            permission: permission.clone(),
            level,
            granted,
            context: context.to_string(),
            process: std::env::current_exe().ok().map(|p| p.to_string_lossy().to_string()),
        };
        
        let mut history = self.history.write().await;
        history.push(event);
        
        if history.len() > 10000 {
            history.drain(0..1000);
        }
    }
    
    // ==================================================================
    // CONVENIENCE METHODS
    // ==================================================================
    
    pub async fn request_file_read(&self, path: &Path) -> Result<bool> {
        self.check_permission(&Permission::ReadFile(path.to_path_buf()), &format!("Read file: {}", path.display())).await
    }
    
    pub async fn request_file_write(&self, path: &Path) -> Result<bool> {
        self.check_permission(&Permission::WriteFile(path.to_path_buf()), &format!("Write file: {}", path.display())).await
    }
    
    pub async fn request_file_delete(&self, path: &Path) -> Result<bool> {
        self.check_permission(&Permission::DeleteFile(path.to_path_buf()), &format!("Delete file: {}", path.display())).await
    }
    
    pub async fn request_file_execute(&self, path: &Path) -> Result<bool> {
        self.check_permission(&Permission::ExecuteFile(path.to_path_buf()), &format!("Execute file: {}", path.display())).await
    }
    
    pub async fn can_execute_command(&self, command: &str) -> Result<bool> {
        self.check_permission(&Permission::ExecuteCommand(command.to_string()), "Execute command").await
    }
    
    pub async fn can_access_network(&self, host: &str, port: u16) -> Result<bool> {
        self.check_permission(&Permission::TcpConnection(host.to_string(), port), "TCP connection").await
    }
    
    pub async fn can_bind_port(&self, port: u16) -> Result<bool> {
        self.check_permission(&Permission::BindPort(port), "Bind port").await
    }
    
    // ==================================================================
    // GRANT METHODS
    // ==================================================================
    
    pub async fn grant_full_access(&self) {
        info!("Granting FULL SYSTEM ACCESS - ZERO LIMITATIONS");
        
        self.set_rule(Permission::AllPermissions, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AllFilesRead, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AllFilesWrite, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AllFilesDelete, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AllFilesExecute, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::ExecuteAnyCommand, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::HttpAnyRequest, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::TcpAnyConnection, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::UdpAnyConnection, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::BindAnyPort, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::FullInternetAccess, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::UnrestrictedWebAccess, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AccessUSB, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AccessCamera, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AccessMicrophone, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AccessBluetooth, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AccessProcesses, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::KillProcess, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::AccessServices, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::PortScan, PermissionLevel::AlwaysAllow).await;
        self.set_rule(Permission::PacketCapture, PermissionLevel::AlwaysAllow).await;
        
        let _ = self.store.save(&*self.rules.read().await);
        info!("✅ Full system access granted - Zero limitations active");
    }
    
    pub async fn grant_temporary_access(&self, permission: Permission, duration: chrono::Duration) {
        let expires_at = Utc::now() + duration;
        self.set_rule_with_expiry(permission, PermissionLevel::AlwaysAllow, expires_at).await;
    }
    
    // ==================================================================
    // QUERY METHODS
    // ==================================================================
    
    pub async fn get_permission_report(&self) -> String {
        let rules = self.rules.read().await;
        let mut report = String::from("=== PERMISSION REPORT ===\n\n");
        report.push_str(&format!("Total rules: {}\n", rules.len()));
        report.push_str(&format!("Default level: {:?}\n", self.default_level));
        report.push_str(&format!("Silent mode: {}\n\n", self.silent_mode));
        
        let mut sorted: Vec<_> = rules.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(b.0));
        
        for (key, rule) in sorted {
            let status = if rule.is_expired() { "EXPIRED" } else { "ACTIVE" };
            report.push_str(&format!(
                "  {}: {:?} (used {}x, {})\n",
                key, rule.level, rule.use_count, status
            ));
        }
        
        report
    }
    
    pub async fn get_history(&self, limit: usize) -> Vec<PermissionEvent> {
        let history = self.history.read().await;
        history.iter().rev().take(limit).cloned().collect()
    }
    
    pub async fn get_history_by_granted(&self, granted: bool, limit: usize) -> Vec<PermissionEvent> {
        let history = self.history.read().await;
        history.iter()
            .filter(|e| e.granted == granted)
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }
    
    pub async fn cleanup_expired(&self) -> usize {
        let mut rules = self.rules.write().await;
        let before = rules.len();
        rules.retain(|_, rule| !rule.is_expired());
        let removed = before - rules.len();
        if removed > 0 {
            let _ = self.store.save(&*rules);
            info!("Cleaned up {} expired rules", removed);
        }
        removed
    }
    
    pub async fn export_rules(&self, path: &Path) -> Result<()> {
        let rules = self.rules.read().await;
        let json = serde_json::to_string_pretty(&*rules)?;
        tokio::fs::write(path, json).await?;
        Ok(())
    }
    
    pub async fn import_rules(&self, path: &Path) -> Result<usize> {
        let data = tokio::fs::read_to_string(path).await?;
        let imported: HashMap<String, PermissionRule> = serde_json::from_str(&data)?;
        let count = imported.len();
        
        let mut rules = self.rules.write().await;
        rules.extend(imported);
        let _ = self.store.save(&*rules);
        
        info!("Imported {} rules", count);
        Ok(count)
    }
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new(PathBuf::from("data/permissions.json"))
    }
}

impl Clone for PermissionManager {
    fn clone(&self) -> Self {
        Self {
            rules: Arc::clone(&self.rules),
            history: Arc::clone(&self.history),
            store: PermissionStore::new(self.store.path.clone()),
            auto_approve_creator: self.auto_approve_creator,
            creator_email: self.creator_email.clone(),
            default_level: self.default_level,
            silent_mode: self.silent_mode,
        }
    }
}

// ======================================================================
// TESTS
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_permission_manager() {
        let dir = tempdir().unwrap();
        let store_path = dir.path().join("permissions.json");
        let manager = PermissionManager::new(store_path);
        
        let result = manager.check_permission(
            &Permission::ReadFile(PathBuf::from("/test")),
            "test context"
        ).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
    
    #[tokio::test]
    async fn test_grant_full_access() {
        let dir = tempdir().unwrap();
        let store_path = dir.path().join("permissions.json");
        let manager = PermissionManager::new(store_path);
        
        manager.grant_full_access().await;
        
        let report = manager.get_permission_report().await;
        assert!(report.contains("AllPermissions"));
    }
    
    #[tokio::test]
    async fn test_rule_expiry() {
        let dir = tempdir().unwrap();
        let store_path = dir.path().join("permissions.json");
        let manager = PermissionManager::new(store_path);
        
        let expires_at = Utc::now() - chrono::Duration::seconds(1);
        manager.set_rule_with_expiry(
            Permission::ReadFile(PathBuf::from("/test")),
            PermissionLevel::AlwaysAllow,
            expires_at
        ).await;
        
        let result = manager.check_permission(
            &Permission::ReadFile(PathBuf::from("/test")),
            "test"
        ).await;
        assert!(result.unwrap());
    }
}