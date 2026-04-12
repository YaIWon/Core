// ======================================================================
// MAIN ENTRY POINT - SELF-EVOLVING LM ORCHESTRATOR
// File: src/main.rs
// Description: Orchestrates file watcher, ingestor, embedder, vector store,
//              blockchain, and RAG for continuous learning
// ======================================================================

mod scanner;
mod memory;

use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Result;
use tokio::sync::RwLock;
use tracing::{info, error, warn};
use tracing_subscriber;

use scanner::{FileWatcher, WatcherConfig, Ingestor, Embedder, FileProcessor};
use memory::vector_store::VectorStore;
use memory::blockchain::BlockchainManager;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    info!("=========================================");
    info!("SELF-EVOLVING LANGUAGE MODEL");
    info!("=========================================");
    
    // Configuration
    let training_dir = PathBuf::from("training_data");
    let vector_store_path = PathBuf::from("data/vectors");
    let blockchain_path = PathBuf::from("data/blockchain");
    
    // Create directories
    std::fs::create_dir_all(&training_dir)?;
    std::fs::create_dir_all(&vector_store_path)?;
    std::fs::create_dir_all(&blockchain_path)?;
    
    // Initialize components
    info!("Initializing vector store...");
    let vector_store = Arc::new(RwLock::new(VectorStore::new(vector_store_path).await?));
    
    info!("Initializing blockchain...");
    let blockchain = Arc::new(RwLock::new(BlockchainManager::new(blockchain_path).await?));
    
    info!("Initializing embedder...");
    let embedder = Arc::new(Embedder::new(384));
    
    info!("Initializing ingestor...");
    let ingestor = Arc::new(Ingestor::new());
    
    info!("Initializing file processor...");
    let processor = FileProcessor::new(
        vector_store.clone(),
        blockchain.clone(),
        ingestor.clone(),
        embedder.clone(),
    );
    
    // Start file watcher
    let watcher_config = WatcherConfig {
        watch_path: training_dir.clone(),
        debounce_delay_ms: 500,
        recursive: true,
        ignore_hidden: true,
        ignore_patterns: vec![
            r"\.DS_Store".to_string(),
            r"\.git".to_string(),
            r"\.tmp".to_string(),
            r"~$".to_string(),
        ],
    };
    
    info!("Starting file watcher on {:?}", training_dir);
    info!("Vector store contains {} entries", vector_store.read().await.len());
    info!("Blockchain contains {} blocks", blockchain.read().await.len());
    info!("");
    info!("Place files in 'training_data/' directory to start learning");
    info!("Run 'cargo run --bin chat' to interact with the LM");
    info!("=========================================");
    
    // Run file watcher
    let (watcher, mut event_rx) = FileWatcher::new(watcher_config);
    
    // Spawn watcher in background
    tokio::spawn(async move {
        if let Err(e) = watcher.start().await {
            error!("File watcher error: {}", e);
        }
    });
    
    // Process events
    while let Some(event) = event_rx.recv().await {
        info!("Processing event: {:?} - {:?}", event.change_type, event.path);
        
        if let Err(e) = processor.process_event(event).await {
            error!("Failed to process event: {}", e);
        }
        
        // Print stats
        let vector_count = vector_store.read().await.len();
        let block_count = blockchain.read().await.len();
        info!("Vector store: {} entries, Blockchain: {} blocks", vector_count, block_count);
    }
    
    Ok(())
}