// ======================================================================
// FILE WATCHER - PRODUCTION READY
// File: src/scanner/watcher.rs
// Description: Real-time file watching using notify crate (no polling)
//              Detects added, modified, deleted files instantly
//              Handles recursive directory watching
// ======================================================================

use anyhow::{Result, Context};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::scanner::ingestor::Ingestor;
use crate::scanner::embedder::Embedder;
use crate::memory::vector_store::{VectorStore, VectorEntry};
use crate::memory::blockchain::BlockchainManager;

// ======================================================================
// WATCHER CONFIGURATION
// ======================================================================

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

// ======================================================================
// FILE EVENT
// ======================================================================

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

// ======================================================================
// DEBOUNCE MAP
// ======================================================================

struct DebounceMap {
    inner: HashMap<PathBuf, std::time::Instant>,
    delay: Duration,
}

impl DebounceMap {
    fn new(delay: Duration) -> Self {
        Self {
            inner: HashMap::new(),
            delay,
        }
    }
    
    fn should_process(&mut self, path: &PathBuf) -> bool {
        let now = std::time::Instant::now();
        if let Some(last) = self.inner.get(path) {
            if now.duration_since(*last) < self.delay {
                return false;
            }
        }
        self.inner.insert(path.clone(), now);
        true
    }
    
    fn remove(&mut self, path: &PathBuf) {
        self.inner.remove(path);
    }
}

// ======================================================================
// FILE WATCHER
// ======================================================================

pub struct FileWatcher {
    config: WatcherConfig,
    event_tx: mpsc::UnboundedSender<FileEvent>,
    debounce_map: Arc<tokio::sync::Mutex<DebounceMap>>,
}

impl FileWatcher {
    pub fn new(config: WatcherConfig) -> (Self, mpsc::UnboundedReceiver<FileEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let debounce_map = Arc::new(tokio::sync::Mutex::new(
            DebounceMap::new(Duration::from_millis(config.debounce_delay_ms))
        ));
        
        let watcher = Self {
            config,
            event_tx: tx,
            debounce_map,
        };
        
        (watcher, rx)
    }
    
    pub async fn start(self) -> Result<()> {
        let watch_path = self.config.watch_path.clone();
        
        if !watch_path.exists() {
            info!("Creating watch directory: {:?}", watch_path);
            std::fs::create_dir_all(&watch_path)
                .with_context(|| format!("Failed to create {:?}", watch_path))?;
        }
        
        info!("Starting file watcher on {:?}", watch_path);
        
        let (notify_tx, mut notify_rx) = mpsc::unbounded_channel();
        
        let watch_path_clone = watch_path.clone();
        let ignore_patterns = self.config.ignore_patterns.clone();
        let ignore_hidden = self.config.ignore_hidden;
        let recursive = self.config.recursive;
        
        std::thread::spawn(move || {
            let mut watcher = RecommendedWatcher::new(
                move |res: Result<Event, notify::Error>| {
                    if let Ok(event) = res {
                        let _ = notify_tx.send(event);
                    }
                },
                Config::default()
            ).expect("Failed to create watcher");
            
            let mode = if recursive {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };
            
            watcher.watch(&watch_path_clone, mode)
                .expect("Failed to watch directory");
            
            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        });
        
        while let Some(event) = notify_rx.recv().await {
            self.process_notify_event(event, &ignore_patterns, ignore_hidden).await;
        }
        
        Ok(())
    }
    
    async fn process_notify_event(&self, event: Event, ignore_patterns: &[String], ignore_hidden: bool) {
        for path in event.paths {
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
        
        if ignore_hidden && file_name.starts_with('.') {
            return true;
        }
        
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
// FILE PROCESSOR
// ======================================================================

pub struct FileProcessor {
    ingestor: Ingestor,
    embedder: Embedder,
    vector_store: Arc<tokio::sync::RwLock<VectorStore>>,
    blockchain: Arc<tokio::sync::RwLock<BlockchainManager>>,
}

impl FileProcessor {
    pub fn new(
        vector_store: Arc<tokio::sync::RwLock<VectorStore>>,
        blockchain: Arc<tokio::sync::RwLock<BlockchainManager>>,
        ingestor: Ingestor,
        embedder: Embedder,
    ) -> Self {
        Self {
            ingestor,
            embedder,
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
        
        let content = self.ingestor.ingest_file(path).await?;
        
        if content.is_empty() {
            warn!("No content extracted from {:?}", path);
            return Ok(());
        }
        
        let chunks = self.embedder.chunk_text(&content);
        info!("Chunked {:?} into {} chunks", path.file_name().unwrap_or_default(), chunks.len());
        
        let mut entries = Vec::new();
        for chunk in chunks {
            let embedding = self.embedder.embed(&chunk).await;
            
            let entry = VectorEntry::new(
                format!("{}:{}", path.display(), Uuid::new_v4()),
                chunk.clone(),
                path.to_path_buf(),
                embedding,
            );
            
            entries.push(entry);
        }
        
        // Store in vector database
        {
            let store = self.vector_store.read().await;
            for entry in &entries {
                if let Err(e) = store.insert(entry.clone()).await {
                    error!("Failed to insert vector: {}", e);
                }
            }
        }
        
        // Record in blockchain
        {
            let blockchain = self.blockchain.read().await;
            for chunk in entries.iter().map(|e| &e.content) {
                if let Err(e) = blockchain.add_learning(chunk, Some(path.display().to_string())).await {
                    error!("Failed to add to blockchain: {}", e);
                }
            }
        }
        
        info!("Successfully processed {:?} ({} chunks)", path.file_name().unwrap_or_default(), entries.len());
        Ok(())
    }
    
    async fn remove_file(&self, path: &Path) -> Result<()> {
        info!("Removing file from store: {:?}", path);
        
        let store = self.vector_store.read().await;
        let removed = store.remove_by_source(path).await?;
        
        info!("Removed {} vectors for {:?}", removed, path);
        Ok(())
    }
    
    async fn rename_file(&self, from: &Path, to: &Path) -> Result<()> {
        info!("Renaming file in store: {:?} -> {:?}", from, to);
        
        let store = self.vector_store.read().await;
        let renamed = store.rename_source(from, to).await?;
        
        info!("Renamed {} vectors from {:?} to {:?}", renamed, from, to);
        Ok(())
    }
    
    pub async fn process_directory(&self, path: &Path, recursive: bool) -> Result<usize> {
        let results = self.ingestor.ingest_directory(path, recursive).await?;
        let count = results.len();
        
        for (file_path, content) in results {
            let chunks = self.embedder.chunk_text(&content);
            
            let mut entries = Vec::new();
            for chunk in chunks {
                let embedding = self.embedder.embed(&chunk).await;
                
                let entry = VectorEntry::new(
                    format!("{}:{}", file_path.display(), Uuid::new_v4()),
                    chunk.clone(),
                    file_path.clone(),
                    embedding,
                );
                
                entries.push(entry);
            }
            
            let store = self.vector_store.read().await;
            for entry in entries {
                store.insert(entry).await?;
            }
        }
        
        info!("Processed {} files from directory {:?}", count, path);
        Ok(count)
    }
}

// ======================================================================
// TESTS
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;
    
    #[test]
    fn test_watcher_config_default() {
        let config = WatcherConfig::default();
        assert_eq!(config.watch_path, PathBuf::from("training_data"));
        assert_eq!(config.debounce_delay_ms, 500);
        assert!(config.recursive);
        assert!(config.ignore_hidden);
    }
    
    #[test]
    fn test_should_ignore() {
        let (watcher, _rx) = FileWatcher::new(WatcherConfig::default());
        
        assert!(watcher.should_ignore(
            Path::new(".hidden"),
            &[],
            true
        ));
        
        assert!(!watcher.should_ignore(
            Path::new("visible"),
            &[],
            true
        ));
        
        assert!(watcher.should_ignore(
            Path::new("test.DS_Store"),
            &[r"\.DS_Store".to_string()],
            false
        ));
    }
    
    #[tokio::test]
    async fn test_file_processor_creation() -> Result<()> {
        let dir = tempdir()?;
        let vector_store = Arc::new(tokio::sync::RwLock::new(
            VectorStore::new(dir.path().join("vectors")).await?
        ));
        let blockchain = Arc::new(tokio::sync::RwLock::new(
            BlockchainManager::new(dir.path().join("blockchain")).await?
        ));
        
        let ingestor = Ingestor::new();
        let embedder = Embedder::new(384);
        
        let _processor = FileProcessor::new(
            vector_store,
            blockchain,
            ingestor,
            embedder,
        );
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_process_text_file() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "This is a test document.")?;
        
        let vector_store = Arc::new(tokio::sync::RwLock::new(
            VectorStore::new(dir.path().join("vectors")).await?
        ));
        let blockchain = Arc::new(tokio::sync::RwLock::new(
            BlockchainManager::new(dir.path().join("blockchain")).await?
        ));
        
        let ingestor = Ingestor::new();
        let embedder = Embedder::new(384);
        
        let processor = FileProcessor::new(
            vector_store.clone(),
            blockchain.clone(),
            ingestor,
            embedder,
        );
        
        let event = FileEvent {
            path: file_path,
            change_type: ChangeType::Added,
            timestamp: chrono::Utc::now(),
        };
        
        processor.process_event(event).await?;
        
        let store = vector_store.read().await;
        assert!(store.len().await? > 0);
        
        Ok(())
    }
}