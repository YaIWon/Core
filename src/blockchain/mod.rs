// ======================================================================
// SIMPLIFIED BLOCKCHAIN ACCESS - WORKING VERSION
// File: src/blockchain/mod.rs
// ======================================================================

use anyhow::{Result, anyhow};
use serde::{Serialize, Deserialize};
use serde_json::json;
use reqwest::Client;
use tracing::{info, warn};
use std::time::{Instant, Duration};
use sha2::{Sha256, Digest};
use hex;
use std::str::FromStr;

// ======================================================================
// BITCOIN RPC CLIENT (READ-ONLY - WORKS)
// ======================================================================

pub mod bitcoin_client {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct BitcoinBlock {
        pub hash: String,
        pub height: u64,
        pub timestamp: u64,
        pub tx_count: usize,
    }

    #[derive(Clone)]
    pub struct BitcoinRpcClient {
        client: Client,
        api_url: String,
    }

    impl BitcoinRpcClient {
        pub fn new(api_url: &str) -> Self {
            Self {
                client: Client::new(),
                api_url: api_url.to_string(),
            }
        }

        pub async fn get_block(&self, block_hash: &str) -> Result<BitcoinBlock> {
            let url = format!("{}/block/{}", self.api_url, block_hash);
            let response = self.client.get(&url).send().await?;
            let data: serde_json::Value = response.json().await?;
            
            Ok(BitcoinBlock {
                hash: data["hash"].as_str().unwrap_or("").to_string(),
                height: data["height"].as_u64().unwrap_or(0),
                timestamp: data["timestamp"].as_u64().unwrap_or(0),
                tx_count: data["tx_count"].as_u64().unwrap_or(0) as usize,
            })
        }

        pub async fn get_balance(&self, address: &str) -> Result<u64> {
            let url = format!("https://blockstream.info/api/address/{}/utxo", address);
            let response = self.client.get(&url).send().await?;
            let utxos: Vec<serde_json::Value> = response.json().await?;
            Ok(utxos.iter().map(|u| u["value"].as_u64().unwrap_or(0)).sum())
        }
    }
}

// ======================================================================
// ETHEREUM RPC CLIENT (READ-ONLY - WORKS)
// ======================================================================

pub mod ethereum {
    use super::*;

    pub struct EthereumRpcClient {
        client: Client,
        url: String,
    }

    impl EthereumRpcClient {
        pub fn new(url: &str) -> Self {
            Self {
                client: Client::new(),
                url: url.to_string(),
            }
        }

        pub async fn get_block_number(&self) -> Result<u64> {
            let request = json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "eth_blockNumber",
                "params": []
            });
            
            let response = self.client.post(&self.url).json(&request).send().await?;
            let data: serde_json::Value = response.json().await?;
            let block_hex = data["result"].as_str().unwrap_or("0x0");
            Ok(u64::from_str_radix(block_hex.trim_start_matches("0x"), 16)?)
        }

        pub async fn get_balance(&self, address: &str) -> Result<f64> {
            let request = json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "eth_getBalance",
                "params": [address, "latest"]
            });
            
            let response = self.client.post(&self.url).json(&request).send().await?;
            let data: serde_json::Value = response.json().await?;
            let balance_hex = data["result"].as_str().unwrap_or("0x0");
            let balance_wei = u128::from_str_radix(balance_hex.trim_start_matches("0x"), 16)?;
            Ok(balance_wei as f64 / 1e18)
        }
    }
}

// ======================================================================
// CPU MINING - FULLY WORKING
// ======================================================================

#[derive(Debug, Clone)]
pub struct MiningResult {
    pub nonce: u64,
    pub hash: String,
    pub duration_ms: u64,
    pub hashrate_hps: u64,
}

#[derive(Debug, Clone)]
pub struct MiningStats {
    pub total_hashes: u64,
    pub current_hashrate: u64,
    pub current_difficulty: u32,
    pub uptime_seconds: u64,
    pub blocks_mined: u64,
}

pub struct CpuMiner {
    hashrate: u64,
    total_hashes: u64,
    difficulty: u32,
    start_time: Option<Instant>,
    blocks_mined: u64,
    running: bool,
}

impl CpuMiner {
    pub fn new() -> Self {
        Self {
            hashrate: 0,
            total_hashes: 0,
            difficulty: 1,
            start_time: None,
            blocks_mined: 0,
            running: false,
        }
    }
    
    pub fn start(&mut self) {
        self.running = true;
        self.start_time = Some(Instant::now());
        info!("⛏️ CPU Mining started with difficulty {}", self.difficulty);
    }
    
    pub fn stop(&mut self) {
        self.running = false;
        info!("⛏️ CPU Mining stopped. Total blocks mined: {}", self.blocks_mined);
    }
    
    pub fn mine(&mut self, data: &[u8]) -> Option<MiningResult> {
        if !self.running {
            return None;
        }
        
        let start = Instant::now();
        let target = self.get_target();
        
        for nonce in 0u64..10_000_000 {
            let mut hasher = Sha256::new();
            hasher.update(data);
            hasher.update(&nonce.to_le_bytes());
            let hash = hasher.finalize();
            
            self.total_hashes += 1;
            
            if Self::hash_meets_target(&hash, target) {
                let duration = start.elapsed();
                let hashrate = self.total_hashes / duration.as_secs().max(1);
                self.hashrate = hashrate;
                self.blocks_mined += 1;
                
                info!("✅ BLOCK MINED!");
                info!("   Nonce: {}", nonce);
                info!("   Hash: {}", hex::encode(&hash[..8]));
                info!("   Time: {}ms", duration.as_millis());
                info!("   Hashrate: {} H/s", hashrate);
                
                return Some(MiningResult {
                    nonce,
                    hash: hex::encode(hash),
                    duration_ms: duration.as_millis() as u64,
                    hashrate_hps: hashrate,
                });
            }
        }
        
        None
    }
    
    fn get_target(&self) -> [u8; 32] {
        let mut target = [0xFF; 32];
        let difficulty = self.difficulty as usize;
        for i in 0..difficulty.min(32) {
            target[i] = 0;
        }
        target
    }
    
    fn hash_meets_target(hash: &[u8], target: [u8; 32]) -> bool {
        for i in 0..32 {
            if hash[i] > target[i] {
                return false;
            }
            if hash[i] < target[i] {
                return true;
            }
        }
        true
    }
    
    pub fn adjust_difficulty(&mut self, last_block_time_ms: u64) {
        let target_time_ms = 10_000;
        if last_block_time_ms < target_time_ms / 2 && self.difficulty < 100 {
            self.difficulty += 1;
            info!("⚡ Difficulty increased to {}", self.difficulty);
        } else if last_block_time_ms > target_time_ms * 2 && self.difficulty > 1 {
            self.difficulty -= 1;
            info!("📉 Difficulty decreased to {}", self.difficulty);
        }
    }
    
    pub fn get_stats(&self) -> MiningStats {
        MiningStats {
            total_hashes: self.total_hashes,
            current_hashrate: self.hashrate,
            current_difficulty: self.difficulty,
            uptime_seconds: self.start_time.map(|t| t.elapsed().as_secs()).unwrap_or(0),
            blocks_mined: self.blocks_mined,
        }
    }
}

// ======================================================================
// RPC ENDPOINTS
// ======================================================================

pub struct RpcEndpoints {
    pub bitcoin_mainnet: &'static str,
    pub bitcoin_testnet: &'static str,
    pub ethereum_mainnet: &'static str,
    pub bsc_mainnet: &'static str,
    pub polygon_mainnet: &'static str,
    pub solana_mainnet: &'static str,
}

impl RpcEndpoints {
    pub fn new() -> Self {
        Self {
            bitcoin_mainnet: "https://blockstream.info/api",
            bitcoin_testnet: "https://blockstream.info/testnet/api",
            ethereum_mainnet: "https://cloudflare-eth.com",
            bsc_mainnet: "https://bsc-dataseed.binance.org",
            polygon_mainnet: "https://polygon-rpc.com",
            solana_mainnet: "https://api.mainnet-beta.solana.com",
        }
    }
}

// ======================================================================
// UNIVERSAL BLOCKCHAIN ACCESS
// ======================================================================

pub struct UniversalBlockchainAccess {
    pub bitcoin: Option<bitcoin_client::BitcoinRpcClient>,
    pub ethereum: Option<ethereum::EthereumRpcClient>,
    pub miner: CpuMiner,
    pub endpoints: RpcEndpoints,
    client: Client,
}

impl UniversalBlockchainAccess {
    pub fn new() -> Self {
        Self {
            bitcoin: None,
            ethereum: None,
            miner: CpuMiner::new(),
            endpoints: RpcEndpoints::new(),
            client: Client::new(),
        }
    }
    
    pub fn init_bitcoin(&mut self, use_testnet: bool) {
        let api_url = if use_testnet {
            self.endpoints.bitcoin_testnet
        } else {
            self.endpoints.bitcoin_mainnet
        };
        self.bitcoin = Some(bitcoin_client::BitcoinRpcClient::new(api_url));
        info!("🔗 Bitcoin RPC initialized");
    }
    
    pub fn init_ethereum(&mut self) {
        self.ethereum = Some(ethereum::EthereumRpcClient::new(self.endpoints.ethereum_mainnet));
        info!("🔗 Ethereum RPC initialized");
    }
    
    pub fn start_mining(&mut self) {
        self.miner.start();
    }
    
    pub fn stop_mining(&mut self) {
        self.miner.stop();
    }
    
    pub fn mine(&mut self, data: &[u8]) -> Option<MiningResult> {
        self.miner.mine(data)
    }
    
    pub fn mine_learning(&mut self, learning_content: &str) -> Option<MiningResult> {
        self.mine(learning_content.as_bytes())
    }
    
    pub fn get_mining_stats(&self) -> MiningStats {
        self.miner.get_stats()
    }
    
    pub async fn get_bitcoin_balance(&self, address: &str) -> Result<u64> {
        let url = format!("https://blockstream.info/api/address/{}/utxo", address);
        let response = self.client.get(&url).send().await?;
        let utxos: Vec<serde_json::Value> = response.json().await?;
        Ok(utxos.iter().map(|u| u["value"].as_u64().unwrap_or(0)).sum())
    }
    
    pub async fn get_ethereum_balance(&self, address: &str) -> Result<f64> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getBalance",
            "params": [address, "latest"]
        });
        
        let response = self.client.post(self.endpoints.ethereum_mainnet).json(&request).send().await?;
        let data: serde_json::Value = response.json().await?;
        let balance_hex = data["result"].as_str().unwrap_or("0x0");
        let balance_wei = u128::from_str_radix(balance_hex.trim_start_matches("0x"), 16)?;
        Ok(balance_wei as f64 / 1e18)
    }
    
    pub fn get_rpc_endpoints(&self) -> &RpcEndpoints {
        &self.endpoints
    }

    pub fn adjust_mining_difficulty(&mut self, elapsed_ms: u64) {
        if elapsed_ms > 0 {
            self.miner.difficulty = ((self.miner.difficulty as u64 * 100) / elapsed_ms.max(1)) as u32;
            if self.miner.difficulty == 0 {
                self.miner.difficulty = 1;
            }
            info!("Mining difficulty adjusted to {} based on {}ms elapsed", self.miner.difficulty, elapsed_ms);
        }
    }
}

impl Default for UniversalBlockchainAccess {
    fn default() -> Self {
        Self::new()
    }
}

// ======================================================================
// TESTS
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cpu_miner_creation() {
        let miner = CpuMiner::new();
        assert_eq!(miner.difficulty, 1);
        assert_eq!(miner.blocks_mined, 0);
    }
    
    #[test]
    fn test_mining_stats() {
        let mut miner = CpuMiner::new();
        miner.start();
        let stats = miner.get_stats();
        assert!(stats.current_difficulty >= 1);
        miner.stop();
    }
    
    #[test]
    fn test_rpc_endpoints() {
        let endpoints = RpcEndpoints::new();
        assert!(!endpoints.bitcoin_mainnet.is_empty());
        assert!(!endpoints.ethereum_mainnet.is_empty());
    }
}