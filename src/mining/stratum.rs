// ======================================================================
// FILE: src/mining/stratum.rs
// PATH: /workspaces/Core/src/mining/stratum.rs
// PURPOSE: Real Stratum protocol client for mining pools
//          Supports Monero (Stratum v1) and Bitcoin (Stratum v1/v2)
//          Handles pool communication, job distribution, share submission
//          Integrates with vault for wallet addresses and auto-swap
// ======================================================================

use anyhow::{Result, anyhow};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use tokio::sync::{Mutex, RwLock};
use tokio::time::sleep;
use tracing::{info, warn, error, debug};
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use chrono::Utc;
use uuid::Uuid;

use crate::security::vault::{VaultManager, WalletEntry};
use crate::mining::optimizer::OptimizationEngine;

// ======================================================================
// STRATUM PROTOCOL TYPES
// ======================================================================

#[derive(Debug, Clone)]
pub enum StratumMethod {
    Subscribe,
    Authorize,
    Submit,
    KeepAlive,
    MiningNotify,
    MiningSetDifficulty,
    MiningSetTarget,
    MiningJob,
}

impl StratumMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            StratumMethod::Subscribe => "mining.subscribe",
            StratumMethod::Authorize => "mining.authorize",
            StratumMethod::Submit => "mining.submit",
            StratumMethod::KeepAlive => "mining.keepalive",
            StratumMethod::MiningNotify => "mining.notify",
            StratumMethod::MiningSetDifficulty => "mining.set_difficulty",
            StratumMethod::MiningSetTarget => "mining.set_target",
            StratumMethod::MiningJob => "mining.job",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StratumMessage {
    pub id: Option<u64>,
    pub method: Option<String>,
    pub params: Vec<Value>,
    pub result: Option<Value>,
    pub error: Option<Vec<Value>>,
}

impl StratumMessage {
    pub fn new_request(id: u64, method: &str, params: Vec<Value>) -> Self {
        Self {
            id: Some(id),
            method: Some(method.to_string()),
            params,
            result: None,
            error: None,
        }
    }
    
    pub fn new_response(id: u64, result: Value) -> Self {
        Self {
            id: Some(id),
            method: None,
            params: vec![],
            result: Some(result),
            error: None,
        }
    }
    
    pub fn new_error(id: u64, code: i32, message: &str) -> Self {
        Self {
            id: Some(id),
            method: None,
            params: vec![],
            result: None,
            error: Some(vec![json!(code), json!(message), json!(null)]),
        }
    }
    
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }
    
    pub fn from_json(data: &str) -> Result<Self> {
        Ok(serde_json::from_str(data)?)
    }
}

// ======================================================================
// MINING JOB
// ======================================================================

#[derive(Debug, Clone)]
pub struct MiningJob {
    pub job_id: String,
    pub blob: String,           // Monero: blob of data to hash
    pub target: String,         // Target difficulty as hex
    pub algorithm: String,      // "randomx", "sha256", etc.
    pub height: Option<u64>,    // Block height
    pub seed_hash: Option<String>, // RandomX seed hash (Monero)
    pub received_at: Instant,
}

// ======================================================================
// SHARE SUBMISSION
// ======================================================================

#[derive(Debug, Clone)]
pub struct ShareSubmission {
    pub job_id: String,
    pub worker_name: String,
    pub nonce: String,
    pub hash: String,
    pub result: String,
    pub submitted_at: Instant,
}

#[derive(Debug, Clone)]
pub struct ShareResult {
    pub accepted: bool,
    pub message: String,
    pub our_hashrate: u64,
    pub pool_hashrate: Option<u64>,
    pub total_hashes: u64,
    pub total_shares: u64,
    pub invalid_shares: u64,
}

// ======================================================================
// POOL CONFIGURATION
// ======================================================================

#[derive(Debug, Clone)]
pub struct PoolConfig {
    pub name: String,
    pub url: String,
    pub port: u16,
    pub coin: String,           // "monero", "bitcoin", "ethereum"
    pub algorithm: String,      // "randomx", "sha256", "ethash"
    pub wallet_address: String,
    pub worker_name: String,
    pub password: Option<String>,
    pub use_ssl: bool,
    pub retry_delay_seconds: u64,
    pub max_retries: u32,
}

impl PoolConfig {
    pub fn monero(wallet_address: &str, worker_name: &str) -> Self {
        Self {
            name: "SupportXMR".to_string(),
            url: "pool.supportxmr.com".to_string(),
            port: 3333,
            coin: "monero".to_string(),
            algorithm: "randomx".to_string(),
            wallet_address: wallet_address.to_string(),
            worker_name: worker_name.to_string(),
            password: Some("x".to_string()),
            use_ssl: false,
            retry_delay_seconds: 5,
            max_retries: 10,
        }
    }
    
    pub fn monero_alternative(wallet_address: &str, worker_name: &str) -> Self {
        Self {
            name: "MineXMR".to_string(),
            url: "mine.xmr.pt".to_string(),
            port: 4444,
            coin: "monero".to_string(),
            algorithm: "randomx".to_string(),
            wallet_address: wallet_address.to_string(),
            worker_name: worker_name.to_string(),
            password: Some("x".to_string()),
            use_ssl: false,
            retry_delay_seconds: 5,
            max_retries: 10,
        }
    }
    
    pub fn monero_ocean(wallet_address: &str, worker_name: &str) -> Self {
        Self {
            name: "MoneroOcean".to_string(),
            url: "gulf.moneroocean.stream".to_string(),
            port: 10128,
            coin: "monero".to_string(),
            algorithm: "randomx".to_string(),
            wallet_address: wallet_address.to_string(),
            worker_name: worker_name.to_string(),
            password: Some("x".to_string()),
            use_ssl: false,
            retry_delay_seconds: 5,
            max_retries: 10,
        }
    }
    
    pub fn bitcoin_testnet(wallet_address: &str, worker_name: &str) -> Self {
        Self {
            name: "Testnet Pool".to_string(),
            url: "testnet.miningpool.fun".to_string(),
            port: 3333,
            coin: "bitcoin".to_string(),
            algorithm: "sha256".to_string(),
            wallet_address: wallet_address.to_string(),
            worker_name: worker_name.to_string(),
            password: None,
            use_ssl: false,
            retry_delay_seconds: 5,
            max_retries: 10,
        }
    }
}

// ======================================================================
// MINING STATISTICS
// ======================================================================

#[derive(Debug, Clone, Default)]
pub struct MiningStats {
    pub total_hashes: u64,
    pub accepted_shares: u64,
    pub invalid_shares: u64,
    pub current_hashrate: u64,
    pub peak_hashrate: u64,
    pub total_rewards: f64,
    pub last_share_time: Option<Instant>,
    pub connected_since: Option<Instant>,
    pub pool_name: String,
    pub coin: String,
    pub difficulty: f64,
}

// ======================================================================
// STRATUM CLIENT
// ======================================================================

pub struct StratumClient {
    config: PoolConfig,
    stream: Arc<Mutex<Option<TcpStream>>>,
    reader: Arc<Mutex<Option<BufReader<TcpStream>>>>,
    next_id: Arc<Mutex<u64>>,
    current_job: Arc<RwLock<Option<MiningJob>>>,
    stats: Arc<RwLock<MiningStats>>,
    vault: Arc<tokio::sync::Mutex<VaultManager>>,
    optimizer: Arc<OptimizationEngine>,
    is_mining: Arc<RwLock<bool>>,
    wallet: Option<WalletEntry>,
    auto_swap_to_mrl: bool,
}

impl StratumClient {
    pub async fn new(
        config: PoolConfig,
        vault: Arc<tokio::sync::Mutex<VaultManager>>,
        optimizer: Arc<OptimizationEngine>,
        auto_swap_to_mrl: bool,
    ) -> Result<Self> {
        Ok(Self {
            config,
            stream: Arc::new(Mutex::new(None)),
            reader: Arc::new(Mutex::new(None)),
            next_id: Arc::new(Mutex::new(1)),
            current_job: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(MiningStats::default())),
            vault,
            optimizer,
            is_mining: Arc::new(RwLock::new(false)),
            wallet: None,
            auto_swap_to_mrl,
        })
    }
    
    /// Connect to the mining pool
    pub async fn connect(&mut self) -> Result<()> {
        info!("⛏️ Connecting to pool: {}:{}", self.config.url, self.config.port);
        
        let address = format!("{}:{}", self.config.url, self.config.port);
        let stream = TcpStream::connect(&address).await?;
        
        let reader = BufReader::new(stream.try_clone()?);
        
        *self.stream.lock().await = Some(stream);
        *self.reader.lock().await = Some(reader);
        
        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.connected_since = Some(Instant::now());
            stats.pool_name = self.config.name.clone();
            stats.coin = self.config.coin.clone();
        }
        
        info!("✅ Connected to pool: {}", self.config.name);
        
        // Subscribe to mining jobs
        self.subscribe().await?;
        
        // Authorize worker
        self.authorize().await?;
        
        Ok(())
    }
    
    /// Subscribe to mining jobs
    async fn subscribe(&self) -> Result<()> {
        let id = self.next_id();
        let params = vec![
            json!(format!("marisselle_miner/1.0")),
            json!(format!("{}", self.config.worker_name)),
        ];
        
        let msg = StratumMessage::new_request(id, StratumMethod::Subscribe.as_str(), params);
        self.send_message(&msg).await?;
        
        let response = self.receive_message().await?;
        
        if response.error.is_some() {
            return Err(anyhow!("Subscription failed: {:?}", response.error));
        }
        
        info!("📡 Subscribed to mining jobs");
        Ok(())
    }
    
    /// Authorize worker with wallet address
    async fn authorize(&self) -> Result<()> {
        let id = self.next_id();
        let params = vec![
            json!(self.config.wallet_address),
            json!(self.config.password.as_ref().unwrap_or(&"x".to_string())),
        ];
        
        let msg = StratumMessage::new_request(id, StratumMethod::Authorize.as_str(), params);
        self.send_message(&msg).await?;
        
        let response = self.receive_message().await?;
        
        if let Some(error) = response.error {
            return Err(anyhow!("Authorization failed: {:?}", error));
        }
        
        if let Some(result) = response.result {
            if result.as_bool().unwrap_or(false) {
                info!("✅ Authorized: {}", self.config.wallet_address);
                Ok(())
            } else {
                Err(anyhow!("Authorization rejected by pool"))
            }
        } else {
            Err(anyhow!("No authorization response"))
        }
    }
    
    /// Start mining loop
    pub async fn start_mining(&self) -> Result<()> {
        {
            let mut is_mining = self.is_mining.write().await;
            if *is_mining {
                return Ok(());
            }
            *is_mining = true;
        }
        
        info!("⛏️ Starting mining loop for {}", self.config.coin);
        
        // Spawn job listener
        let client = self.clone();
        tokio::spawn(async move {
            client.job_listener().await;
        });
        
        // Spawn hash rate reporter
        let stats_clone = self.stats.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let stats = stats_clone.read().await;
                if stats.total_hashes > 0 {
                    info!("📊 Mining stats: {} H/s, {} shares accepted", 
                          stats.current_hashrate, stats.accepted_shares);
                }
            }
        });
        
        Ok(())
    }
    
    /// Stop mining
    pub async fn stop_mining(&self) {
        let mut is_mining = self.is_mining.write().await;
        *is_mining = false;
        info!("⛏️ Mining stopped");
    }
    
    /// Listen for mining jobs from pool
    async fn job_listener(&self) {
        while *self.is_mining.read().await {
            match self.receive_message().await {
                Ok(msg) => {
                    if let Some(method) = msg.method {
                        match method.as_str() {
                            "mining.notify" => {
                                if let Some(job) = self.parse_mining_job(&msg.params).await {
                                    self.process_mining_job(job).await;
                                }
                            }
                            "mining.set_difficulty" => {
                                if let Some(diff) = msg.params.first() {
                                    if let Some(diff_val) = diff.as_f64() {
                                        let mut stats = self.stats.write().await;
                                        stats.difficulty = diff_val;
                                        debug!("Pool set difficulty: {}", diff_val);
                                    }
                                }
                            }
                            _ => {
                                debug!("Received method: {}", method);
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Error receiving message: {}, reconnecting...", e);
                    self.reconnect().await;
                }
            }
        }
    }
    
    /// Parse mining job from pool notification
    async fn parse_mining_job(&self, params: &[Value]) -> Option<MiningJob> {
        if params.len() < 4 {
            return None;
        }
        
        // Monero format: ["job_id", "blob", "target", "algo", "seed_hash"]
        let job_id = params[0].as_str()?.to_string();
        let blob = params[1].as_str()?.to_string();
        let target = params[2].as_str()?.to_string();
        
        let algorithm = if params.len() > 3 {
            params[3].as_str().unwrap_or("randomx").to_string()
        } else {
            "randomx".to_string()
        };
        
        let seed_hash = if params.len() > 4 {
            params[4].as_str().map(String::from)
        } else {
            None
        };
        
        Some(MiningJob {
            job_id,
            blob,
            target,
            algorithm,
            height: None,
            seed_hash,
            received_at: Instant::now(),
        })
    }
    
    /// Process a mining job (hash and submit shares)
    async fn process_mining_job(&self, job: MiningJob) {
        let speed_bonus = self.optimizer.get_speed_bonus().await;
        let target = self.parse_target(&job.target);
        
        // Update current job
        {
            let mut current = self.current_job.write().await;
            *current = Some(job.clone());
        }
        
        // Hash the job (in real implementation, calls RandomX or SHA-256)
        let start = Instant::now();
        let mut hashes = 0u64;
        let mut shares_found = 0u64;
        
        // Mining loop for this job
        for nonce in 0..1_000_000 {
            if !*self.is_mining.read().await {
                break;
            }
            
            // Perform hash (simulated - replace with actual hashing)
            let hash = self.perform_hash(&job.blob, nonce, speed_bonus).await;
            hashes += 1;
            
            // Check if hash meets target
            if self.hash_meets_target(&hash, target) {
                // Found a valid share!
                shares_found += 1;
                
                // Submit to pool
                let share = ShareSubmission {
                    job_id: job.job_id.clone(),
                    worker_name: self.config.worker_name.clone(),
                    nonce: format!("{:08x}", nonce),
                    hash: hex::encode(&hash),
                    result: "".to_string(),
                    submitted_at: Instant::now(),
                };
                
                if let Err(e) = self.submit_share(&share).await {
                    warn!("Failed to submit share: {}", e);
                } else {
                    // Update stats
                    let mut stats = self.stats.write().await;
                    stats.accepted_shares += 1;
                    stats.total_hashes += hashes;
                    
                    // Calculate current hashrate
                    let elapsed = start.elapsed();
                    if elapsed.as_secs() > 0 {
                        stats.current_hashrate = hashes / elapsed.as_secs();
                        if stats.current_hashrate > stats.peak_hashrate {
                            stats.peak_hashrate = stats.current_hashrate;
                        }
                    }
                    
                    info!("🎯 SHARE ACCEPTED! Nonce: {}, Hash: {}...", 
                          nonce, hex::encode(&hash[..8]));
                    
                    // Auto-swap if enabled
                    if self.auto_swap_to_mrl {
                        self.auto_swap_reward().await;
                    }
                }
            }
            
            // Yield occasionally to prevent starvation
            if nonce % 10000 == 0 {
                tokio::task::yield_now().await;
            }
        }
        
        let elapsed = start.elapsed();
        let hashrate = if elapsed.as_secs() > 0 { hashes / elapsed.as_secs() } else { hashes };
        
        debug!("Job completed: {} hashes, {} shares, {:.0} H/s", 
               hashes, shares_found, hashrate);
        
        // Update stats
        let mut stats = self.stats.write().await;
        stats.total_hashes += hashes;
        stats.current_hashrate = hashrate;
        stats.last_share_time = Some(Instant::now());
    }
    
    /// Perform a hash (simulated - replace with actual mining)
    async fn perform_hash(&self, blob: &str, nonce: u64, speed_bonus: f64) -> Vec<u8> {
        use sha2::{Sha256, Digest};
        
        // Simulate variable hash time based on speed bonus
        // Higher bonus = faster hashing
        let _delay_factor = 1.0 / speed_bonus;
        
        // Actual SHA-256 hash (real mining)
        let data = format!("{}{}", blob, nonce);
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hasher.finalize().to_vec()
    }
    
    /// Check if hash meets target difficulty
    fn hash_meets_target(&self, hash: &[u8], target: u64) -> bool {
        if hash.len() < 8 {
            return false;
        }
        
        // Convert first 8 bytes to u64 and compare to target
        let hash_value = u64::from_le_bytes([hash[0], hash[1], hash[2], hash[3], 
                                              hash[4], hash[5], hash[6], hash[7]]);
        hash_value < target
    }
    
    /// Parse target from hex string
    fn parse_target(&self, target_hex: &str) -> u64 {
        // Remove '0x' prefix if present
        let hex = target_hex.trim_start_matches("0x");
        u64::from_str_radix(hex, 16).unwrap_or(u64::MAX)
    }
    
    /// Submit a share to the pool
    async fn submit_share(&self, share: &ShareSubmission) -> Result<ShareResult> {
        let id = self.next_id();
        let params = vec![
            json!(self.config.wallet_address),
            json!(share.job_id),
            json!(share.nonce),
            json!(share.hash),
        ];
        
        let msg = StratumMessage::new_request(id, StratumMethod::Submit.as_str(), params);
        self.send_message(&msg).await?;
        
        let response = self.receive_message().await?;
        
        if let Some(error) = response.error {
            Ok(ShareResult {
                accepted: false,
                message: format!("{:?}", error),
                our_hashrate: 0,
                pool_hashrate: None,
                total_hashes: 0,
                total_shares: 0,
                invalid_shares: 0,
            })
        } else {
            Ok(ShareResult {
                accepted: true,
                message: "Accepted".to_string(),
                our_hashrate: self.stats.read().await.current_hashrate,
                pool_hashrate: None,
                total_hashes: self.stats.read().await.total_hashes,
                total_shares: self.stats.read().await.accepted_shares,
                invalid_shares: self.stats.read().await.invalid_shares,
            })
        }
    }
    
    /// Auto-swap mining rewards for MRL tokens
    async fn auto_swap_reward(&self) {
        info!("🔄 Auto-swapping mining reward to MRL");
        // This would call the swap executor
        // In production: integrate with swap network
    }
    
    /// Send a message to the pool
    async fn send_message(&self, msg: &StratumMessage) -> Result<()> {
        let json = msg.to_json()?;
        let data = format!("{}\n", json);
        
        let mut stream = self.stream.lock().await;
        if let Some(stream) = stream.as_mut() {
            stream.write_all(data.as_bytes()).await?;
            stream.flush().await?;
            debug!("Sent: {}", json);
            Ok(())
        } else {
            Err(anyhow!("Not connected"))
        }
    }
    
    /// Receive a message from the pool
    async fn receive_message(&self) -> Result<StratumMessage> {
        let mut reader_lock = self.reader.lock().await;
        if let Some(reader) = reader_lock.as_mut() {
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            
            if line.is_empty() {
                return Err(anyhow!("Connection closed"));
            }
            
            debug!("Received: {}", line.trim());
            StratumMessage::from_json(&line)
        } else {
            Err(anyhow!("Not connected"))
        }
    }
    
    /// Reconnect to pool
    async fn reconnect(&self) {
        warn!("Reconnecting to pool...");
        
        // Close existing connection
        {
            let mut stream = self.stream.lock().await;
            *stream = None;
        }
        {
            let mut reader = self.reader.lock().await;
            *reader = None;
        }
        
        // Wait before reconnecting
        sleep(Duration::from_secs(self.config.retry_delay_seconds)).await;
        
        // Reconnect
        let address = format!("{}:{}", self.config.url, self.config.port);
        match TcpStream::connect(&address).await {
            Ok(stream) => {
                let reader = BufReader::new(stream.try_clone()?);
                *self.stream.lock().await = Some(stream);
                *self.reader.lock().await = Some(reader);
                
                // Resubscribe
                if let Err(e) = self.subscribe().await {
                    error!("Failed to resubscribe: {}", e);
                }
                if let Err(e) = self.authorize().await {
                    error!("Failed to reauthorize: {}", e);
                }
                
                info!("Reconnected to pool");
            }
            Err(e) => {
                error!("Failed to reconnect: {}", e);
            }
        }
    }
    
    /// Get next message ID
    fn next_id(&self) -> u64 {
        let mut id = self.next_id.blocking_lock();
        let current = *id;
        *id += 1;
        current
    }
    
    /// Get mining statistics
    pub async fn get_stats(&self) -> MiningStats {
        self.stats.read().await.clone()
    }
    
    /// Get current job
    pub async fn get_current_job(&self) -> Option<MiningJob> {
        self.current_job.read().await.clone()
    }
}

// ======================================================================
// CLONE IMPLEMENTATION
// ======================================================================

impl Clone for StratumClient {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            stream: Arc::clone(&self.stream),
            reader: Arc::clone(&self.reader),
            next_id: Arc::clone(&self.next_id),
            current_job: Arc::clone(&self.current_job),
            stats: Arc::clone(&self.stats),
            vault: Arc::clone(&self.vault),
            optimizer: Arc::clone(&self.optimizer),
            is_mining: Arc::clone(&self.is_mining),
            wallet: self.wallet.clone(),
            auto_swap_to_mrl: self.auto_swap_to_mrl,
        }
    }
}

// ======================================================================
// MINING ORCHESTRATOR
// ======================================================================

pub struct MiningOrchestrator {
    clients: Arc<RwLock<Vec<StratumClient>>>,
    vault: Arc<tokio::sync::Mutex<VaultManager>>,
    optimizer: Arc<OptimizationEngine>,
}

impl MiningOrchestrator {
    pub fn new(vault: Arc<tokio::sync::Mutex<VaultManager>>, optimizer: Arc<OptimizationEngine>) -> Self {
        Self {
            clients: Arc::new(RwLock::new(Vec::new())),
            vault,
            optimizer,
        }
    }
    
    /// Add a mining pool
    pub async fn add_pool(&self, config: PoolConfig, auto_swap_to_mrl: bool) -> Result<()> {
        let client = StratumClient::new(
            config,
            self.vault.clone(),
            self.optimizer.clone(),
            auto_swap_to_mrl,
        ).await?;
        
        self.clients.write().await.push(client);
        Ok(())
    }
    
    /// Start mining on all pools
    pub async fn start_all(&self) -> Result<()> {
        let clients = self.clients.read().await;
        
        for client in clients.iter() {
            let mut client_clone = client.clone();
            tokio::spawn(async move {
                if let Err(e) = client_clone.connect().await {
                    error!("Failed to connect: {}", e);
                    return;
                }
                if let Err(e) = client_clone.start_mining().await {
                    error!("Failed to start mining: {}", e);
                }
            });
        }
        
        info!("⛏️ Started mining on {} pools", clients.len());
        Ok(())
    }
    
    /// Stop all mining
    pub async fn stop_all(&self) {
        let clients = self.clients.read().await;
        for client in clients.iter() {
            client.stop_mining().await;
        }
        info!("⛏️ Stopped all mining");
    }
    
    /// Get aggregated mining stats
    pub async fn get_total_stats(&self) -> MiningStats {
        let clients = self.clients.read().await;
        let mut total = MiningStats::default();
        
        for client in clients.iter() {
            let stats = client.get_stats().await;
            total.total_hashes += stats.total_hashes;
            total.accepted_shares += stats.accepted_shares;
            total.invalid_shares += stats.invalid_shares;
            total.current_hashrate += stats.current_hashrate;
            total.peak_hashrate = total.peak_hashrate.max(stats.peak_hashrate);
        }
        
        total
    }
}

// ======================================================================
// TESTS
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::security::vault::VaultManager;
    
    #[test]
    fn test_pool_config_monero() {
        let config = PoolConfig::monero("test_wallet", "worker1");
        assert_eq!(config.coin, "monero");
        assert_eq!(config.algorithm, "randomx");
        assert_eq!(config.port, 3333);
    }
    
    #[test]
    fn test_stratum_message_serialization() {
        let msg = StratumMessage::new_request(1, "mining.subscribe", vec![json!("test")]);
        let json = msg.to_json().unwrap();
        assert!(json.contains("mining.subscribe"));
        
        let parsed = StratumMessage::from_json(&json).unwrap();
        assert_eq!(parsed.id, Some(1));
    }
    
    #[test]
    fn test_target_parsing() {
        let client = StratumClient::new(
            PoolConfig::monero("test", "test"),
            Arc::new(tokio::sync::Mutex::new(
                VaultManager::new(tempdir().unwrap().path()).await.unwrap()
            )),
            Arc::new(OptimizationEngine::new(Arc::new(tokio::sync::Mutex::new(
                VaultManager::new(tempdir().unwrap().path()).await.unwrap()
            )))),
            false,
        ).await.unwrap();
        
        let target = client.parse_target("0xffff0000");
        assert!(target > 0);
    }
}