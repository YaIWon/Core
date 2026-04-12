// ======================================================================
// FILE WATCHER - PRODUCTION READY
// File: src/scanner/watcher.rs
// Description: Real-time file watching using notify crate (no polling)
//              Detects added, modified, deleted files instantly
//              Handles recursive directory watching
// ======================================================================

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use anyhow::{Result, Context};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherConfig {
    pub watch_path: PathBuf,
    pub debounce_delay_ms: u64,
    pub recursive: bool,
    pub ignore_hidden: bool,
    pub ignore_patterns: Vec<String>,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            watch_path: PathBuf::from("training_data"),
            debounce_delay_ms: 500,
            recursive: true,
            ignore_hidden: true,
            ignore_patterns: vec![
                r"\.DS_Store".to_string(),
                r"\.git".to_string(),
                r"\.tmp".to_string(),
                r"~$".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChangeType {
    Added,
    Modified,
    Removed,
    Renamed(PathBuf, PathBuf),
}

#[derive(Debug, Clone)]
pub struct FileEvent {
    pub path: PathBuf,
    pub change_type: ChangeType,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct FileWatcher {
    config: WatcherConfig,
    event_tx: mpsc::UnboundedSender<FileEvent>,
    debounce_map: Arc<tokio::sync::Mutex<debounce::DebounceMap>>,
}

mod debounce {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};
    use tokio::sync::Mutex;
    
    pub struct DebounceMap {
        inner: HashMap<PathBuf, Instant>,
        delay: Duration,
    }
    
    impl DebounceMap {
        pub fn new(delay: Duration) -> Self {
            Self {
                inner: HashMap::new(),
                delay,
            }
        }
        
        pub fn should_process(&mut self, path: &PathBuf) -> bool {
            let now = Instant::now();
            if let Some(last) = self.inner.get(path) {
                if now.duration_since(*last) < self.delay {
                    return false;
                }
            }
            self.inner.insert(path.clone(), now);
            true
        }
        
        pub fn remove(&mut self, path: &PathBuf) {
            self.inner.remove(path);
        }
    }
}

impl FileWatcher {
    pub fn new(config: WatcherConfig) -> (Self, mpsc::UnboundedReceiver<FileEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let debounce_map = Arc::new(tokio::sync::Mutex::new(
            debounce::DebounceMap::new(Duration::from_millis(config.debounce_delay_ms))
        ));
        
        let watcher = Self {
            config,
            event_tx: tx,
            debounce_map,
        };
        
        (watcher, rx)
    }
    
    pub async fn start(mut self) -> Result<()> {
        let watch_path = self.config.watch_path.clone();
        
        // Ensure watch directory exists
        if !watch_path.exists() {
            info!("Creating watch directory: {:?}", watch_path);
            std::fs::create_dir_all(&watch_path)
                .with_context(|| format!("Failed to create {:?}", watch_path))?;
        }
        
        info!("Starting file watcher on {:?}", watch_path);
        
        // Create a channel for notify events
        let (notify_tx, mut notify_rx) = mpsc::unbounded_channel();
        
        // Configure and run the notify watcher in a separate thread
        let watch_path_clone = watch_path.clone();
        let ignore_patterns = self.config.ignore_patterns.clone();
        let ignore_hidden = self.config.ignore_hidden;
        
        std::thread::spawn(move || {
            let mut watcher = RecommendedWatcher::new(
                move |res: Result<Event, notify::Error>| {
                    if let Ok(event) = res {
                        let _ = notify_tx.send(event);
                    }
                },
                Config::default()
            ).expect("Failed to create watcher");
            
            watcher.watch(&watch_path_clone, RecursiveMode::Recursive)
                .expect("Failed to watch directory");
            
            // Keep the thread alive
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        });
        
        // Process events with debouncing
        while let Some(event) = notify_rx.recv().await {
            self.process_notify_event(event, &ignore_patterns, ignore_hidden).await;
        }
        
        Ok(())
    }
    
    async fn process_notify_event(&self, event: Event, ignore_patterns: &[String], ignore_hidden: bool) {
        for path in event.paths {
            // Check if path should be ignored
            if self.should_ignore(&path, ignore_patterns, ignore_hidden) {
                continue;
            }
            
            let change_type = match event.kind {
                EventKind::Create(_) => ChangeType::Added,
                EventKind::Modify(_) => ChangeType::Modified,
                EventKind::Remove(_) => ChangeType::Removed,
                EventKind::Access(_) => continue,
                _ => continue,
            };
            
            // Debounce check
            let mut debounce_map = self.debounce_map.lock().await;
            if !debounce_map.should_process(&path) {
                debug!("Debounced event for {:?}", path);
                continue;
            }
            
            let file_event = FileEvent {
                path,
                change_type,
                timestamp: chrono::Utc::now(),
            };
            
            debug!("File event: {:?}", file_event);
            let _ = self.event_tx.send(file_event);
        }
    }
    
    fn should_ignore(&self, path: &Path, patterns: &[String], ignore_hidden: bool) -> bool {
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        
        // Check hidden files
        if ignore_hidden && file_name.starts_with('.') {
            return true;
        }
        
        // Check ignore patterns
        for pattern in patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if re.is_match(file_name) {
                    return true;
                }
            }
        }
        
        false
    }
}

// ======================================================================
// FILE PROCESSOR - Handles the actual file processing
// ======================================================================

use crate::scanner::ingestor::Ingestor;
use crate::scanner::embedder::Embedder;
use crate::memory::vector_store::VectorStore;
use crate::memory::blockchain::Blockchain;

pub struct FileProcessor {
    ingestor: Ingestor,
    embedder: Embedder,
    vector_store: Arc<tokio::sync::Mutex<VectorStore>>,
    blockchain: Arc<tokio::sync::Mutex<Blockchain>>,
}

impl FileProcessor {
    pub fn new(
        vector_store: Arc<tokio::sync::Mutex<VectorStore>>,
        blockchain: Arc<tokio::sync::Mutex<Blockchain>>,
    ) -> Self {
        Self {
            ingestor: Ingestor::new(),
            embedder: Embedder::new(384),
            vector_store,
            blockchain,
        }
    }
    
    pub async fn process_event(&self, event: FileEvent) -> Result<()> {
        match event.change_type {
            ChangeType::Added | ChangeType::Modified => {
                self.process_file(&event.path).await?;
            }
            ChangeType::Removed => {
                self.remove_file(&event.path).await?;
            }
            ChangeType::Renamed(from, to) => {
                self.rename_file(&from, &to).await?;
            }
        }
        Ok(())
    }
    
    async fn process_file(&self, path: &Path) -> Result<()> {
        info!("Processing file: {:?}", path);
        
        // Read and parse file
        let content = self.ingestor.ingest_file(path).await?;
        
        // Chunk content
        let chunks = self.embedder.chunk_text(&content, 512, 50);
        
        for chunk in chunks {
            // Generate embedding
            let embedding = self.embedder.embed(&chunk);
            
            // Store in vector database
            let id = format!("{}:{}", path.display(), uuid::Uuid::new_v4());
            let entry = crate::memory::vector_store::VectorEntry {
                id,
                content: chunk.clone(),
                source_path: path.to_path_buf(),
                embedding,
                metadata: serde_json::json!({
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "source": path.display().to_string(),
                }),
            };
            
            {
                let mut store = self.vector_store.lock().await;
                store.insert(entry).await?;
            }
            
            // Record in blockchain
            {
                let mut chain = self.blockchain.lock().await;
                chain.add_block(&chunk).await?;
            }
        }
        
        info!("Successfully processed {:?} ({} chunks)", path, chunks.len());
        Ok(())
    }
    
    async fn remove_file(&self, path: &Path) -> Result<()> {
        info!("Removing file from store: {:?}", path);
        let mut store = self.vector_store.lock().await;
        store.remove_by_source(path).await?;
        Ok(())
    }
    
    async fn rename_file(&self, from: &Path, to: &Path) -> Result<()> {
        info!("Renaming file in store: {:?} -> {:?}", from, to);
        let mut store = self.vector_store.lock().await;
        store.rename_source(from, to).await?;
        Ok(())
    }
}
