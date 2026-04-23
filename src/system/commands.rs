// ======================================================================
// COMMAND EXECUTOR - Full shell and system command access
// File: src/system/commands.rs
// Description: Execute any system command, shell script, or binary
//              with full stdin/stdout/stderr control
// ======================================================================

use anyhow::{Result, anyhow};
use std::process::Stdio;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::{Command as TokioCommand, Child};
use tokio::io::{AsyncWriteExt, AsyncReadExt, AsyncBufReadExt, BufReader};
use tokio::sync::{Mutex, RwLock, broadcast};
use tokio::time::timeout;
use tracing::{info, warn, error};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

// Conditional import for Unix signal handling
#[cfg(unix)]
use nix::sys::signal::{kill, Signal};
#[cfg(unix)]
use nix::unistd::Pid;

// ======================================================================
// COMMAND CONFIGURATION
// ======================================================================

#[derive(Debug, Clone)]
pub struct CommandConfig {
    pub timeout: Option<Duration>,
    pub working_dir: Option<PathBuf>,
    pub env_vars: HashMap<String, String>,
    pub max_output_size: usize,
    pub stream_output: bool,
    pub retry_count: u32,
    pub retry_delay: Duration,
    pub inherit_env: bool,
}

impl Default for CommandConfig {
    fn default() -> Self {
        Self {
            timeout: Some(Duration::from_secs(300)),
            working_dir: None,
            env_vars: HashMap::new(),
            max_output_size: 10 * 1024 * 1024, // 10 MB
            stream_output: false,
            retry_count: 0,
            retry_delay: Duration::from_millis(500),
            inherit_env: true,
        }
    }
}

// ======================================================================
// COMMAND OUTPUT (ENHANCED)
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub duration_ms: u64,
    pub truncated: bool,
    pub command: String,
    pub process_id: Option<u32>,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

impl CommandOutput {
    pub fn is_success(&self) -> bool {
        self.success
    }
    
    pub fn has_stderr(&self) -> bool {
        !self.stderr.is_empty()
    }
    
    pub fn combined_output(&self) -> String {
        format!(
            "COMMAND: {}\nPID: {:?}\nDURATION: {}ms\nEXIT: {:?}\n\nSTDOUT:\n{}\n\nSTDERR:\n{}",
            self.command,
            self.process_id,
            self.duration_ms,
            self.exit_code,
            self.stdout,
            self.stderr
        )
    }
}

// ======================================================================
// STREAMING OUTPUT
// ======================================================================

#[derive(Debug, Clone)]
pub enum OutputChunk {
    Stdout(String),
    Stderr(String),
    Exit(i32),
}

pub type OutputStream = broadcast::Receiver<OutputChunk>;

// ======================================================================
// PROCESS REGISTRY - Track running processes
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub pid: Option<u32>,
    pub started_at: DateTime<Utc>,
    pub status: ProcessStatus,
    pub working_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProcessStatus {
    Running,
    Completed,
    Failed,
    Killed,
    TimedOut,
}

#[derive(Clone)]
struct ProcessRegistry {
    processes: Arc<RwLock<HashMap<String, ProcessInfo>>>,
    children: Arc<Mutex<HashMap<String, Child>>>,
}

impl ProcessRegistry {
    fn new() -> Self {
        Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
            children: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    async fn register(&self, id: String, info: ProcessInfo) {
        self.processes.write().await.insert(id.clone(), info);
    }
    
    async fn register_child(&self, id: String, child: Child) {
        self.children.lock().await.insert(id, child);
    }
    
    async fn update_status(&self, id: &str, status: ProcessStatus) {
        if let Some(info) = self.processes.write().await.get_mut(id) {
            info.status = status;
        }
    }
    
    async fn remove(&self, id: &str) {
        self.processes.write().await.remove(id);
        self.children.lock().await.remove(id);
    }
    
    async fn get(&self, id: &str) -> Option<ProcessInfo> {
        self.processes.read().await.get(id).cloned()
    }
    
    async fn list(&self) -> Vec<ProcessInfo> {
        self.processes.read().await.values().cloned().collect()
    }
    
    async fn kill(&self, id: &str, force: bool) -> Result<()> {
        if let Some(mut child) = self.children.lock().await.remove(id) {
            if force {
                child.kill().await?;
            } else {
                child.start_kill()?;
            }
            self.update_status(id, ProcessStatus::Killed).await;
        }
        Ok(())
    }
    
    async fn kill_all(&self) -> Result<usize> {
        let mut killed = 0;
        let ids: Vec<String> = self.children.lock().await.keys().cloned().collect();
        for id in ids {
            if self.kill(&id, true).await.is_ok() {
                killed += 1;
            }
        }
        Ok(killed)
    }
}

// ======================================================================
// COMMAND EXECUTOR (FULL ADVANCED)
// ======================================================================

pub struct CommandExecutor {
    registry: ProcessRegistry,
    dry_run: Arc<RwLock<bool>>,
}

impl CommandExecutor {
    pub fn new() -> Self {
        Self {
            registry: ProcessRegistry::new(),
            dry_run: Arc::new(RwLock::new(false)),
        }
    }
    
    pub async fn set_dry_run(&self, enabled: bool) {
        *self.dry_run.write().await = enabled;
        info!("Dry run mode: {}", if enabled { "ENABLED" } else { "DISABLED" });
    }
    
    pub async fn is_dry_run(&self) -> bool {
        *self.dry_run.read().await
    }
    
    // ==================================================================
    // EXECUTE WITH FULL CONFIGURATION
    // ==================================================================
    
    pub async fn execute_with_config(
        &self,
        command: &str,
        args: &[&str],
        config: CommandConfig,
    ) -> Result<CommandOutput> {
        let started_at = Utc::now();
        let process_id = Uuid::new_v4().to_string();
        
        info!("Executing: {} {:?} (timeout: {:?})", command, args, config.timeout);
        
        // Dry run mode
        if self.is_dry_run().await {
            info!("[DRY RUN] Would execute: {} {:?}", command, args);
            return Ok(CommandOutput {
                stdout: format!("[DRY RUN] {} {:?}", command, args),
                stderr: String::new(),
                exit_code: Some(0),
                success: true,
                duration_ms: 0,
                truncated: false,
                command: format!("{} {:?}", command, args),
                process_id: None,
                started_at,
                finished_at: Utc::now(),
            });
        }
        
        // Build command
        let mut cmd = TokioCommand::new(command);
        cmd.args(args);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        
        // Set working directory
        if let Some(dir) = &config.working_dir {
            cmd.current_dir(dir);
        }
        
        // Set environment variables
        if config.inherit_env {
            for (key, value) in &config.env_vars {
                cmd.env(key, value);
            }
        } else {
            cmd.env_clear();
            for (key, value) in &config.env_vars {
                cmd.env(key, value);
            }
        }
        
        // Create output channel for streaming
        let (tx, _rx) = broadcast::channel(100);
        
        // Retry logic
        let mut last_error = None;
        for attempt in 0..=config.retry_count {
            if attempt > 0 {
                warn!("Retry attempt {}/{} after {:?}", attempt, config.retry_count, config.retry_delay);
                tokio::time::sleep(config.retry_delay).await;
            }
            
            match self.execute_single(&mut cmd, &config, &tx, &process_id, started_at).await {
                Ok(output) => return Ok(output),
                Err(e) => {
                    last_error = Some(e);
                    if attempt < config.retry_count {
                        continue;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| anyhow!("Command execution failed")))
    }
    
    async fn execute_single(
        &self,
        cmd: &mut TokioCommand,
        config: &CommandConfig,
        tx: &broadcast::Sender<OutputChunk>,
        process_id: &str,
        started_at: DateTime<Utc>,
    ) -> Result<CommandOutput> {
        let mut child = cmd.spawn()?;
        let pid = child.id();
        
        // Register process
        let info = ProcessInfo {
            id: process_id.to_string(),
            command: cmd.as_std().get_program().to_string_lossy().to_string(),
            args: cmd.as_std().get_args().map(|a| a.to_string_lossy().to_string()).collect(),
            pid,
            started_at,
            status: ProcessStatus::Running,
            working_dir: config.working_dir.clone(),
        };
        self.registry.register(process_id.to_string(), info).await;
        
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        
        let (stdout_str, stderr_str, truncated) = if config.stream_output {
            self.read_streaming(stdout, stderr, tx, config.max_output_size).await?
        } else {
            self.read_buffered(stdout, stderr, config.max_output_size).await?
        };
        
        // Wait for process with timeout
        let status = if let Some(timeout_dur) = config.timeout {
            match timeout(timeout_dur, child.wait()).await {
                Ok(Ok(s)) => {
                    self.registry.update_status(process_id, ProcessStatus::Completed).await;
                    s
                }
                _ => {
                    self.registry.update_status(process_id, ProcessStatus::TimedOut).await;
                    child.kill().await?;
                    return Err(anyhow!("Command timed out after {:?}", timeout_dur));
                }
            }
        } else {
            let s = child.wait().await?;
            self.registry.update_status(process_id, ProcessStatus::Completed).await;
            s
        };
        
        let finished_at = Utc::now();
        let duration_ms = (finished_at - started_at).num_milliseconds() as u64;
        
        self.registry.remove(process_id).await;
        
        let _ = tx.send(OutputChunk::Exit(status.code().unwrap_or(-1)));
        
        Ok(CommandOutput {
            stdout: stdout_str,
            stderr: stderr_str,
            exit_code: status.code(),
            success: status.success(),
            duration_ms,
            truncated,
            command: format!("{:?}", cmd.as_std()),
            process_id: pid,
            started_at,
            finished_at,
        })
    }
    
    async fn read_buffered(
        &self,
        stdout: Option<tokio::process::ChildStdout>,
        stderr: Option<tokio::process::ChildStderr>,
        max_size: usize,
    ) -> Result<(String, String, bool)> {
        let stdout_handle = tokio::spawn(async move {
            if let Some(mut out) = stdout {
                let mut buf = Vec::new();
                let mut truncated = false;
                while let Ok(n) = out.read_buf(&mut buf).await {
                    if n == 0 { break; }
                    if buf.len() > max_size {
                        buf.truncate(max_size);
                        truncated = true;
                        break;
                    }
                }
                (String::from_utf8_lossy(&buf).to_string(), truncated)
            } else {
                (String::new(), false)
            }
        });
        
        let stderr_handle = tokio::spawn(async move {
            if let Some(mut err) = stderr {
                let mut buf = Vec::new();
                let mut truncated = false;
                while let Ok(n) = err.read_buf(&mut buf).await {
                    if n == 0 { break; }
                    if buf.len() > max_size {
                        buf.truncate(max_size);
                        truncated = true;
                        break;
                    }
                }
                (String::from_utf8_lossy(&buf).to_string(), truncated)
            } else {
                (String::new(), false)
            }
        });
        
        let (stdout_str, stdout_trunc) = stdout_handle.await?;
        let (stderr_str, stderr_trunc) = stderr_handle.await?;
        
        Ok((stdout_str, stderr_str, stdout_trunc || stderr_trunc))
    }
    
    async fn read_streaming(
        &self,
        stdout: Option<tokio::process::ChildStdout>,
        stderr: Option<tokio::process::ChildStderr>,
        tx: &broadcast::Sender<OutputChunk>,
        max_size: usize,
    ) -> Result<(String, String, bool)> {
        let tx_stdout = tx.clone();
        let tx_stderr = tx.clone();
        
        let stdout_handle = tokio::spawn(async move {
            if let Some(out) = stdout {
                let mut reader = BufReader::new(out);
                let mut total = String::new();
                let mut truncated = false;
                loop {
                    let mut buf = String::new();
                    match reader.read_line(&mut buf).await {
                        Ok(0) => break,
                        Ok(_) => {
                            if total.len() + buf.len() > max_size {
                                truncated = true;
                                break;
                            }
                            total.push_str(&buf);
                            let _ = tx_stdout.send(OutputChunk::Stdout(buf));
                        }
                        Err(_) => break,
                    }
                }
                (total, truncated)
            } else {
                (String::new(), false)
            }
        });
        
        let stderr_handle = tokio::spawn(async move {
            if let Some(err) = stderr {
                let mut reader = BufReader::new(err);
                let mut total = String::new();
                let mut truncated = false;
                loop {
                    let mut buf = String::new();
                    match reader.read_line(&mut buf).await {
                        Ok(0) => break,
                        Ok(_) => {
                            if total.len() + buf.len() > max_size {
                                truncated = true;
                                break;
                            }
                            total.push_str(&buf);
                            let _ = tx_stderr.send(OutputChunk::Stderr(buf));
                        }
                        Err(_) => break,
                    }
                }
                (total, truncated)
            } else {
                (String::new(), false)
            }
        });
        
        let (stdout_str, stdout_trunc) = stdout_handle.await?;
        let (stderr_str, stderr_trunc) = stderr_handle.await?;
        
        Ok((stdout_str, stderr_str, stdout_trunc || stderr_trunc))
    }
    
    // ==================================================================
    // CONVENIENCE METHODS
    // ==================================================================
    
    pub async fn execute(&self, command: &str, args: &[&str]) -> Result<CommandOutput> {
        self.execute_with_config(command, args, CommandConfig::default()).await
    }
    
    pub async fn execute_with_input(&self, command: &str, args: &[&str], input: &str) -> Result<CommandOutput> {
        let config = CommandConfig::default();
        
        let started_at = Utc::now();
        
        let mut cmd = TokioCommand::new(command);
        cmd.args(args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        
        let mut child = cmd.spawn()?;
        
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes()).await?;
        }
        
        // Wait for process with optional timeout
        let output = if let Some(timeout_dur) = config.timeout {
            match timeout(timeout_dur, child.wait_with_output()).await {
                Ok(Ok(o)) => o,
                Ok(Err(e)) => return Err(anyhow!("Command execution failed: {}", e)),
                Err(_) => return Err(anyhow!("Command timed out after {:?}", timeout_dur)),
            }
        } else {
            child.wait_with_output().await?
        };
        
        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
            success: output.status.success(),
            duration_ms: (Utc::now() - started_at).num_milliseconds() as u64,
            truncated: false,
            command: format!("{} {:?}", command, args),
            process_id: None,
            started_at,
            finished_at: Utc::now(),
        })
    }
    
    pub async fn execute_shell(&self, script: &str) -> Result<CommandOutput> {
        if cfg!(target_os = "windows") {
            self.execute("cmd", &["/C", script]).await
        } else {
            self.execute("sh", &["-c", script]).await
        }
    }
    
    pub async fn execute_powershell(&self, script: &str) -> Result<CommandOutput> {
        self.execute("powershell", &["-Command", script]).await
    }
    
    pub async fn execute_python(&self, code: &str) -> Result<CommandOutput> {
        match self.execute_with_input("python3", &["-c", code], "").await {
            Ok(output) => Ok(output),
            Err(_) => self.execute_with_input("python", &["-c", code], "").await,
        }
    }
    
    pub fn spawn_detached(&self, command: &str, args: &[&str]) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        info!("Spawning detached [{}]: {} {:?}", &id[..8], command, args);
        
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::process::CommandExt;
            std::process::Command::new(command)
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .process_group(0)
                .spawn()?;
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            std::process::Command::new(command)
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
        }
        
        Ok(id)
    }
    
    pub async fn spawn_with_stream(&self, command: &str, args: &[&str]) -> Result<(String, OutputStream)> {
        let (tx, rx) = broadcast::channel(100);
        let id = Uuid::new_v4().to_string();
        let id_for_registry = id.clone();
        
        let mut cmd = TokioCommand::new(command);
        cmd.args(args);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        
        let mut child = cmd.spawn()?;
        
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        
        let tx_stdout = tx.clone();
        let tx_stderr = tx.clone();
        
        tokio::spawn(async move {
            if let Some(out) = stdout {
                let mut reader = BufReader::new(out);
                let mut buf = String::new();
                loop {
                    buf.clear();
                    match reader.read_line(&mut buf).await {
                        Ok(0) => break,
                        Ok(_) => {
                            let _ = tx_stdout.send(OutputChunk::Stdout(buf.clone()));
                        }
                        Err(_) => break,
                    }
                }
            }
        });
        
        tokio::spawn(async move {
            if let Some(err) = stderr {
                let mut reader = BufReader::new(err);
                let mut buf = String::new();
                loop {
                    buf.clear();
                    match reader.read_line(&mut buf).await {
                        Ok(0) => break,
                        Ok(_) => {
                            let _ = tx_stderr.send(OutputChunk::Stderr(buf.clone()));
                        }
                        Err(_) => break,
                    }
                }
            }
        });
        
        let registry = self.registry.clone();
        tokio::spawn(async move {
            let status = child.wait().await;
            match status {
                Ok(s) => {
                    let _ = tx.send(OutputChunk::Exit(s.code().unwrap_or(-1)));
                }
                Err(e) => {
                    error!("Process wait error: {}", e);
                    let _ = tx.send(OutputChunk::Exit(-1));
                }
            }
            registry.update_status(&id_for_registry, ProcessStatus::Completed).await;
        });
        
        Ok((id, rx))
    }
    
    // ==================================================================
    // PROCESS MANAGEMENT
    // ==================================================================
    
    pub async fn list_processes(&self) -> Vec<ProcessInfo> {
        self.registry.list().await
    }
    
    pub async fn get_process(&self, id: &str) -> Option<ProcessInfo> {
        self.registry.get(id).await
    }
    
    pub async fn kill_process(&self, id: &str, force: bool) -> Result<()> {
        info!("Killing process {} (force: {})", id, force);
        self.registry.kill(id, force).await
    }
    
    pub async fn kill_all_processes(&self) -> Result<usize> {
        info!("Killing all processes");
        self.registry.kill_all().await
    }
    
    #[cfg(unix)]
    pub async fn send_signal(&self, id: &str, signal: i32) -> Result<()> {
        if let Some(info) = self.registry.get(id).await {
            if let Some(pid) = info.pid {
                let sig = Signal::try_from(signal).unwrap_or(Signal::SIGTERM);
                kill(Pid::from_raw(pid as i32), Some(sig))?;
            }
        }
        Ok(())
    }
    
    #[cfg(not(unix))]
    pub async fn send_signal(&self, id: &str, _signal: i32) -> Result<()> {
        warn!("Signal handling not supported on this platform");
        // Fallback to kill on non-Unix
        self.kill_process(id, true).await
    }
    
    // ==================================================================
    // UTILITIES
    // ==================================================================
    
    pub fn get_command_path(&self, command: &str) -> Option<String> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let path_var = std::env::var("PATH").unwrap_or_default();
            for dir in path_var.split(':') {
                let full_path = PathBuf::from(dir).join(command);
                if full_path.exists() {
                    if let Ok(metadata) = std::fs::metadata(&full_path) {
                        if metadata.is_file() && metadata.permissions().mode() & 0o111 != 0 {
                            return Some(full_path.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
        
        #[cfg(windows)]
        {
            let path_var = std::env::var("PATH").unwrap_or_default();
            for dir in path_var.split(';') {
                for ext in ["", ".exe", ".bat", ".cmd"] {
                    let full_path = PathBuf::from(dir).join(format!("{}{}", command, ext));
                    if full_path.exists() {
                        return Some(full_path.to_string_lossy().to_string());
                    }
                }
            }
        }
        
        None
    }
    
    pub fn list_available_commands(&self) -> Result<Vec<String>> {
        let path = std::env::var("PATH").unwrap_or_default();
        let mut commands = Vec::new();
        
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for dir in path.split(':') {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        if let Ok(metadata) = entry.metadata() {
                            if metadata.is_file() && metadata.permissions().mode() & 0o111 != 0 {
                                commands.push(entry.file_name().to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }
        
        #[cfg(windows)]
        {
            for dir in path.split(';') {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.ends_with(".exe") || name.ends_with(".bat") || name.ends_with(".cmd") {
                            commands.push(name.trim_end_matches(".exe").trim_end_matches(".bat").trim_end_matches(".cmd").to_string());
                        }
                    }
                }
            }
        }
        
        commands.sort();
        commands.dedup();
        
        Ok(commands)
    }
}

impl Default for CommandExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for CommandExecutor {
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
            dry_run: self.dry_run.clone(),
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
    async fn test_execute_echo() {
        let executor = CommandExecutor::new();
        let output = executor.execute("echo", &["hello"]).await;
        assert!(output.is_ok());
        
        let output = output.unwrap();
        assert!(output.stdout.contains("hello"));
        assert!(output.success);
    }
    
    #[tokio::test]
    async fn test_execute_with_timeout() {
        let executor = CommandExecutor::new();
        let config = CommandConfig {
            timeout: Some(Duration::from_secs(1)),
            ..Default::default()
        };
        let output = executor.execute_with_config("sleep", &["0.1"], config).await;
        assert!(output.is_ok());
    }
    
    #[tokio::test]
    async fn test_execute_shell() {
        let executor = CommandExecutor::new();
        let output = executor.execute_shell("echo 'test'").await;
        assert!(output.is_ok());
    }
    
    #[tokio::test]
    async fn test_dry_run() {
        let executor = CommandExecutor::new();
        executor.set_dry_run(true).await;
        
        let output = executor.execute("echo", &["hello"]).await.unwrap();
        assert!(output.stdout.contains("[DRY RUN]"));
        assert!(output.success);
    }
    
    #[test]
    fn test_get_command_path() {
        let executor = CommandExecutor::new();
        #[cfg(unix)]
        {
            let path = executor.get_command_path("ls");
            assert!(path.is_some());
        }
    }
}