// ======================================================================
// VECTOR STORE - PRODUCTION READY WITH LMDB
// File: src/memory/vector_store.rs
// Description: High-performance vector database using LMDB (heed)
//              Supports: Insert, query by similarity, delete by source,
//              persistent storage, concurrent access
// ======================================================================

use anyhow::{Result, anyhow};
use heed::{Env, Database, EnvOpenOptions};
use serde::{Serialize, Deserialize};
use tracing::{info, debug, warn};
use rayon::prelude::*;
use std::path::{Path, PathBuf};

// ======================================================================
// VECTOR ENTRY
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorEntry {
    pub id: String,
    pub content: String,
    pub source_path: PathBuf,
    pub embedding: Vec<f32>,
    pub metadata: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl VectorEntry {
    pub fn new(id: String, content: String, source_path: PathBuf, embedding: Vec<f32>) -> Self {
        Self {
            id,
            content,
            source_path,
            embedding,
            metadata: serde_json::json!({}),
            created_at: chrono::Utc::now(),
        }
    }
    
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

// ======================================================================
// VECTOR STORE STATS
// ======================================================================

#[derive(Debug, Clone)]
pub struct VectorStoreStats {
    pub entry_count: usize,
    pub map_size_bytes: usize,
    pub path: PathBuf,
}

// ======================================================================
// VECTOR STORE
// ======================================================================

pub struct VectorStore {
    env: Env,
    db: Database<heed::types::Str, heed::types::SerdeJson<VectorEntry>>,
    path: PathBuf,
}

impl VectorStore {
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        std::fs::create_dir_all(&path)?;
        
        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(1024 * 1024 * 1024) // 1 GB
                .max_dbs(10)
                .open(&path)?
        };
        
        let mut wtxn = env.write_txn()?;
        let db = env.create_database(&mut wtxn, Some("vectors"))?;
        wtxn.commit()?;
        
        info!("Vector store initialized at {:?}", path);
        
        Ok(Self { env, db, path })
    }
    
    // ==================================================================
    // BASIC CRUD
    // ==================================================================
    
    pub async fn insert(&self, entry: VectorEntry) -> Result<()> {
        let mut wtxn = self.env.write_txn()?;
        self.db.put(&mut wtxn, &entry.id, &entry)?;
        wtxn.commit()?;
        debug!("Inserted vector: {}", entry.id);
        Ok(())
    }
    
    pub async fn insert_batch(&self, entries: Vec<VectorEntry>) -> Result<usize> {
        let count = entries.len();
        let mut wtxn = self.env.write_txn()?;
        
        for entry in entries {
            self.db.put(&mut wtxn, &entry.id, &entry)?;
        }
        
        wtxn.commit()?;
        info!("Inserted {} vectors in batch", count);
        Ok(count)
    }
    
    pub async fn get(&self, id: &str) -> Result<Option<VectorEntry>> {
        let rtxn = self.env.read_txn()?;
        let entry = self.db.get(&rtxn, id)?;
        Ok(entry)
    }
    
    pub async fn get_batch(&self, ids: &[String]) -> Result<Vec<Option<VectorEntry>>> {
        let rtxn = self.env.read_txn()?;
        let mut results = Vec::with_capacity(ids.len());
        
        for id in ids {
            results.push(self.db.get(&rtxn, id)?);
        }
        
        Ok(results)
    }
    
    pub async fn update(&self, id: &str, entry: VectorEntry) -> Result<()> {
        let mut wtxn = self.env.write_txn()?;
        
        if self.db.get(&wtxn, id)?.is_some() {
            self.db.put(&mut wtxn, id, &entry)?;
            wtxn.commit()?;
            debug!("Updated vector: {}", id);
            Ok(())
        } else {
            Err(anyhow!("Vector with id '{}' not found", id))
        }
    }
    
    pub async fn update_metadata(&self, id: &str, metadata: serde_json::Value) -> Result<()> {
        let mut wtxn = self.env.write_txn()?;
        
        if let Some(mut entry) = self.db.get(&wtxn, id)? {
            entry.metadata = metadata;
            self.db.put(&mut wtxn, id, &entry)?;
            wtxn.commit()?;
            debug!("Updated metadata for vector: {}", id);
            Ok(())
        } else {
            Err(anyhow!("Vector with id '{}' not found", id))
        }
    }
    
    pub async fn delete(&self, id: &str) -> Result<bool> {
        let mut wtxn = self.env.write_txn()?;
        let existed = self.db.delete(&mut wtxn, id)?;
        wtxn.commit()?;
        
        if existed {
            debug!("Deleted vector: {}", id);
        }
        Ok(existed)
    }
    
    pub async fn delete_batch(&self, ids: &[String]) -> Result<usize> {
        let mut wtxn = self.env.write_txn()?;
        let mut count = 0;
        
        for id in ids {
            if self.db.delete(&mut wtxn, id)? {
                count += 1;
            }
        }
        
        wtxn.commit()?;
        info!("Deleted {} vectors in batch", count);
        Ok(count)
    }
    
    // ==================================================================
    // SEARCH
    // ==================================================================
    
    pub async fn search_by_similarity(&self, query_embedding: &[f32], top_k: usize) -> Result<Vec<VectorEntry>> {
        let rtxn = self.env.read_txn()?;
        let mut entries = Vec::new();
        
        for result in self.db.iter(&rtxn)? {
            let (_, entry) = result?;
            entries.push(entry);
        }
        
        if entries.is_empty() {
            return Ok(Vec::new());
        }
        
        let query = query_embedding.to_vec();
        let mut scored: Vec<(VectorEntry, f32)> = entries
            .into_par_iter()
            .map(|entry| {
                let similarity = cosine_similarity(&query, &entry.embedding);
                (entry, similarity)
            })
            .collect();
        
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(scored.into_iter().take(top_k).map(|(e, _)| e).collect())
    }
    
    pub async fn search_by_similarity_threshold(&self, query_embedding: &[f32], threshold: f32, limit: usize) -> Result<Vec<VectorEntry>> {
        let rtxn = self.env.read_txn()?;
        let mut entries = Vec::new();
        
        for result in self.db.iter(&rtxn)? {
            let (_, entry) = result?;
            let similarity = cosine_similarity(query_embedding, &entry.embedding);
            if similarity >= threshold {
                entries.push((entry, similarity));
            }
        }
        
        entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(entries.into_iter().take(limit).map(|(e, _)| e).collect())
    }
    
    pub async fn search_by_source(&self, source: &Path) -> Result<Vec<VectorEntry>> {
        let rtxn = self.env.read_txn()?;
        let mut results = Vec::new();
        
        for result in self.db.iter(&rtxn)? {
            let (_, entry) = result?;
            if entry.source_path == source {
                results.push(entry);
            }
        }
        
        Ok(results)
    }
    
    pub async fn search_by_metadata(&self, key: &str, value: &str) -> Result<Vec<VectorEntry>> {
        let rtxn = self.env.read_txn()?;
        let mut results = Vec::new();
        
        for result in self.db.iter(&rtxn)? {
            let (_, entry) = result?;
            if let Some(metadata_value) = entry.metadata.get(key) {
                if let Some(str_value) = metadata_value.as_str() {
                    if str_value == value {
                        results.push(entry);
                    }
                }
            }
        }
        
        Ok(results)
    }
    
    pub async fn search_by_content(&self, query: &str) -> Result<Vec<VectorEntry>> {
        let rtxn = self.env.read_txn()?;
        let mut results = Vec::new();
        let query_lower = query.to_lowercase();
        
        for result in self.db.iter(&rtxn)? {
            let (_, entry) = result?;
            if entry.content.to_lowercase().contains(&query_lower) {
                results.push(entry);
            }
        }
        
        Ok(results)
    }
    
    // ==================================================================
    // SOURCE MANAGEMENT
    // ==================================================================
    
    pub async fn remove_by_source(&self, source: &Path) -> Result<usize> {
        let entries = self.search_by_source(source).await?;
        let ids: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();
        self.delete_batch(&ids).await
    }
    
    pub async fn rename_source(&self, from: &Path, to: &Path) -> Result<usize> {
        let entries = self.search_by_source(from).await?;
        let count = entries.len();
        
        let mut wtxn = self.env.write_txn()?;
        
        for mut entry in entries {
            entry.source_path = to.to_path_buf();
            self.db.put(&mut wtxn, &entry.id, &entry)?;
        }
        
        wtxn.commit()?;
        info!("Renamed {} vectors from {:?} to {:?}", count, from, to);
        Ok(count)
    }
    
    // ==================================================================
    // QUERY & STATS
    // ==================================================================
    
    pub async fn len(&self) -> Result<usize> {
        let rtxn = self.env.read_txn()?;
        Ok(self.db.len(&rtxn)? as usize)
    }
    
    pub async fn is_empty(&self) -> Result<bool> {
        Ok(self.len().await? == 0)
    }
    
    pub async fn get_all(&self, limit: Option<usize>) -> Result<Vec<VectorEntry>> {
        let rtxn = self.env.read_txn()?;
        let mut entries = Vec::new();
        
        for (i, result) in self.db.iter(&rtxn)?.enumerate() {
            if let Some(limit) = limit {
                if i >= limit {
                    break;
                }
            }
            let (_, entry) = result?;
            entries.push(entry);
        }
        
        Ok(entries)
    }
    
    pub async fn get_all_ids(&self) -> Result<Vec<String>> {
        let rtxn = self.env.read_txn()?;
        let mut ids = Vec::new();
        
        for result in self.db.iter(&rtxn)? {
            let (id, _) = result?;
            ids.push(id.to_string());
        }
        
        Ok(ids)
    }
    
    pub async fn get_all_content(&self) -> Result<String> {
        let rtxn = self.env.read_txn()?;
        let mut contents = Vec::new();
        
        for result in self.db.iter(&rtxn)? {
            let (_, entry) = result?;
            contents.push(entry.content);
        }
        
        Ok(contents.join("\n\n"))
    }
    
    pub async fn get_context_for_query(&self, query_embedding: &[f32], max_tokens: usize) -> Result<String> {
        let results = self.search_by_similarity(query_embedding, 10).await?;
        
        let mut context = String::new();
        let mut tokens = 0;
        
        for result in results {
            let approx_tokens = result.content.len() / 4;
            if tokens + approx_tokens > max_tokens {
                break;
            }
            context.push_str(&format!("--- {} ---\n{}\n\n", 
                result.source_path.display(), 
                result.content
            ));
            tokens += approx_tokens;
        }
        
        Ok(context)
    }
    
    pub async fn clear(&self) -> Result<()> {
        let mut wtxn = self.env.write_txn()?;
        self.db.clear(&mut wtxn)?;
        wtxn.commit()?;
        info!("Cleared all vectors from store");
        Ok(())
    }
    
    pub async fn stats(&self) -> Result<VectorStoreStats> {
        let rtxn = self.env.read_txn()?;
        let entry_count = self.db.len(&rtxn)? as usize;
        let map_size = self.env.info().map_size;
        
        Ok(VectorStoreStats {
            entry_count,
            map_size_bytes: map_size,
            path: self.path.clone(),
        })
    }
}

// ======================================================================
// UTILITY FUNCTIONS
// ======================================================================

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

pub fn simple_embedding(text: &str, dimension: usize) -> Vec<f32> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut embedding = vec![0.0; dimension];
    
    for word in words {
        let hash = simple_hash(word);
        let idx = (hash % dimension as u64) as usize;
        embedding[idx] += 1.0;
    }
    
    let norm: f32 = embedding.iter().map(|&x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut embedding {
            *x /= norm;
        }
    }
    
    embedding
}

fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for byte in s.bytes() {
        hash = ((hash << 5).wrapping_add(hash)).wrapping_add(byte as u64);
    }
    hash
}

// ======================================================================
// TESTS
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_vector_store_creation() -> Result<()> {
        let dir = tempdir()?;
        let store = VectorStore::new(dir.path()).await?;
        assert_eq!(store.len().await?, 0);
        Ok(())
    }
    
    #[tokio::test]
    async fn test_insert_and_get() -> Result<()> {
        let dir = tempdir()?;
        let store = VectorStore::new(dir.path()).await?;
        
        let entry = VectorEntry::new(
            "test1".to_string(),
            "test content".to_string(),
            PathBuf::from("/test"),
            vec![1.0, 2.0, 3.0],
        );
        
        store.insert(entry.clone()).await?;
        
        let retrieved = store.get("test1").await?;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().content, "test content");
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_similarity_search() -> Result<()> {
        let dir = tempdir()?;
        let store = VectorStore::new(dir.path()).await?;
        
        store.insert(VectorEntry::new(
            "1".to_string(), "content1".to_string(), PathBuf::from("/test1"), vec![1.0, 0.0, 0.0],
        )).await?;
        
        store.insert(VectorEntry::new(
            "2".to_string(), "content2".to_string(), PathBuf::from("/test2"), vec![0.0, 1.0, 0.0],
        )).await?;
        
        let results = store.search_by_similarity(&[1.0, 0.0, 0.0], 5).await?;
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "1");
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_delete() -> Result<()> {
        let dir = tempdir()?;
        let store = VectorStore::new(dir.path()).await?;
        
        store.insert(VectorEntry::new(
            "test1".to_string(), "content".to_string(), PathBuf::from("/test"), vec![1.0, 2.0, 3.0],
        )).await?;
        
        assert_eq!(store.len().await?, 1);
        
        let deleted = store.delete("test1").await?;
        assert!(deleted);
        assert_eq!(store.len().await?, 0);
        
        Ok(())
    }
    
    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 1.0);
        
        let c = vec![0.0, 1.0, 0.0];
        assert_eq!(cosine_similarity(&a, &c), 0.0);
    }
    
    #[test]
    fn test_simple_embedding() {
        let embedding = simple_embedding("hello world", 128);
        assert_eq!(embedding.len(), 128);
        
        let norm: f32 = embedding.iter().map(|&x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.001);
    }
}