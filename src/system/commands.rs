// ======================================================================
// COMMAND EXECUTOR - Full shell and system command access
// File: src/system/commands.rs
// Description: Execute any system command, shell script, or binary
//              with full stdin/stdout/stderr control
// ======================================================================

use anyhow::{Result, anyhow};
use std::process::{Command, Stdio};
use std::io::Write;
use tokio::process::Command as TokioCommand;
use tracing::{info, warn, error};
use std::sync::Arc;

use crate::learning::ComprehensiveLogger;

pub struct CommandExecutor {
    logger: Arc<ComprehensiveLogger>,
}

impl CommandExecutor {
    pub fn new(logger: Arc<ComprehensiveLogger>) -> Self {
        Self { logger }
    }
    
    pub async fn execute(&self, command: &str, args: &[&str]) -> Result<CommandOutput> {
        info!("Executing: {} {:?}", command, args);
        
        let output = TokioCommand::new(command)
            .args(args)
            .output()
            .await?;
        
        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
            success: output.status.success(),
        })
    }
    
    pub async fn execute_with_input(&self, command: &str, args: &[&str], input: &str) -> Result<CommandOutput> {
        info!("Executing with input: {} {:?}", command, args);
        
        let mut child = TokioCommand::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes()).await?;
        }
        
        let output = child.wait_with_output().await?;
        
        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
            success: output.status.success(),
        })
    }
    
    pub async fn execute_shell(&self, script: &str) -> Result<CommandOutput> {
        info!("Executing shell script: {}", &script[..script.len().min(100)]);
        
        let output = if cfg!(target_os = "windows") {
            TokioCommand::new("cmd").args(["/C", script]).output().await?
        } else {
            TokioCommand::new("sh").args(["-c", script]).output().await?
        };
        
        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
            success: output.status.success(),
        })
    }
    
    pub async fn execute_powershell(&self, script: &str) -> Result<CommandOutput> {
        info!("Executing PowerShell script");
        
        let output = TokioCommand::new("powershell")
            .args(["-Command", script])
            .output()
            .await?;
        
        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
            success: output.status.success(),
        })
    }
    
    pub async fn execute_python(&self, code: &str) -> Result<CommandOutput> {
        info!("Executing Python code");
        
        self.execute_with_input("python3", &["-c", code], "").await
            .or_else(|_| self.execute_with_input("python", &["-c", code], "").await)
    }
    
    pub async fn spawn_detached(&self, command: &str, args: &[&str]) -> Result<()> {
        info!("Spawning detached: {} {:?}", command, args);
        
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::process::CommandExt;
            Command::new(command)
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .process_group(0)
                .spawn()?;
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            Command::new(command)
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
        }
        
        Ok(())
    }
    
    pub async fn get_command_path(&self, command: &str) -> Option<String> {
        which::which(command).ok().map(|p| p.to_string_lossy().to_string())
    }
    
    pub async fn list_available_commands(&self) -> Result<Vec<String>> {
        let path = std::env::var("PATH").unwrap_or_default();
        let mut commands = Vec::new();
        
        for dir in path.split(':') {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.is_file() {
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                if metadata.permissions().mode() & 0o111 != 0 {
                                    commands.push(entry.file_name().to_string_lossy().to_string());
                                }
                            }
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

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub success: bool,
}
