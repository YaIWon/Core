// ======================================================================
// SCANNER MODULE
// File: src/scanner/mod.rs
// ======================================================================

mod watcher;
mod ingestor;
mod embedder;

pub use watcher::{FileWatcher, WatcherConfig, FileEvent, ChangeType, FileProcessor};
pub use ingestor::Ingestor;
pub use embedder::Embedder;
