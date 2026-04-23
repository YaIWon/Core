// ======================================================================
// LOG VIEWER - Real-time log monitoring
// File: src/bin/view_logs.rs
// Description: View all logs in real-time
// ======================================================================

use anyhow::Result;
use std::path::PathBuf;
use std::io::{self, Write};
use tokio::fs;
use chrono::Utc;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=========================================");
    println!("MARISSELLE LOG VIEWER");
    println!("=========================================");
    println!("Commands:");
    println!("  conversation  - View conversation history");
    println!("  thoughts      - View LM thoughts");
    println!("  learning      - View learning history");
    println!("  api           - View API calls");
    println!("  all           - View all logs");
    println!("  tail          - Follow logs in real-time");
    println!("  export [path] - Export all logs to JSON");
    println!("  clear         - Clear screen");
    println!("  quit          - Exit");
    println!("=========================================\n");
    
    let logs_dir = PathBuf::from("logs");
    
    loop {
        print!("> ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        
        match input {
            "conversation" => {
                view_latest_logs(&logs_dir, "conversation").await?;
            }
            "thoughts" => {
                view_latest_logs(&logs_dir, "thoughts").await?;
            }
            "learning" => {
                view_latest_logs(&logs_dir, "learning").await?;
            }
            "api" => {
                view_latest_logs(&logs_dir, "api").await?;
            }
            "all" => {
                view_all_logs(&logs_dir).await?;
            }
            "tail" => {
                tail_logs(&logs_dir).await?;
            }
            cmd if cmd.starts_with("export") => {
                let path = cmd.split_whitespace().nth(1).unwrap_or("logs_export.json");
                export_logs(&logs_dir, path).await?;
            }
            "clear" => {
                print!("\x1B[2J\x1B[1;1H");
            }
            "quit" | "exit" => {
                println!("Goodbye!");
                break;
            }
            _ => {
                println!("Unknown command: {}", input);
            }
        }
    }
    
    Ok(())
}

async fn view_latest_logs(logs_dir: &PathBuf, category: &str) -> Result<()> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let pattern = format!("marisselle_{}_*.log", today);
    
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(logs_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&format!("marisselle_{}", today)) {
            let content = fs::read_to_string(entry.path()).await?;
            for line in content.lines() {
                if let Ok(log) = serde_json::from_str::<serde_json::Value>(line) {
                    entries.push(log);
                }
            }
        }
    }
    
    for entry in entries.iter().rev().take(50) {
        println!("{}", serde_json::to_string_pretty(entry)?);
    }
    
    Ok(())
}

async fn view_all_logs(logs_dir: &PathBuf) -> Result<()> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    
    for entry in std::fs::read_dir(logs_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&format!("marisselle_{}", today)) {
            println!("\n=== {} ===\n", name);
            let content = fs::read_to_string(entry.path()).await?;
            for line in content.lines().rev().take(100) {
                println!("{}", line);
            }
        }
    }
    
    Ok(())
}

async fn tail_logs(logs_dir: &PathBuf) -> Result<()> {
    use tokio::time::{sleep, Duration};
    use std::collections::HashSet;
    
    println!("Tailing logs... Press Ctrl+C to stop.\n");
    
    let mut seen_files = HashSet::new();
    
    loop {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        
        for entry in std::fs::read_dir(logs_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            
            if name.starts_with(&format!("marisselle_{}", today)) && !seen_files.contains(&name) {
                seen_files.insert(name.clone());
                
                let content = fs::read_to_string(entry.path()).await?;
                let lines: Vec<&str> = content.lines().collect();
                
                // Show last 10 lines of new files
                for line in lines.iter().rev().take(10).rev() {
                    println!("{}", line);
                }
            }
        }
        
        sleep(Duration::from_millis(500)).await;
    }
}

async fn export_logs(logs_dir: &PathBuf, output_path: &str) -> Result<()> {
    let mut all_logs = Vec::new();
    
    for entry in std::fs::read_dir(logs_dir)? {
        let entry = entry?;
        let content = fs::read_to_string(entry.path()).await?;
        for line in content.lines() {
            if let Ok(log) = serde_json::from_str::<serde_json::Value>(line) {
                all_logs.push(log);
            }
        }
    }
    
    let json = serde_json::to_string_pretty(&all_logs)?;
    fs::write(output_path, json).await?;
    
    println!("✅ Exported {} log entries to {}", all_logs.len(), output_path);
    
    Ok(())
}