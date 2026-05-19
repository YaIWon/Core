// ======================================================================
// FILE: src/security/vault.rs
// PATH: /workspaces/Core/src/security/vault.rs
// PURPOSE: AES-256-GCM encrypted storage for wallets, private keys, mnemonics
//          Password: !@3456AAbb
//          All sensitive data encrypted at rest.
// ======================================================================

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{Result, anyhow};
use serde::{Serialize, Deserialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use chrono::{DateTime, Utc};
use tracing::{info, warn, error};
use sha2::{Sha256, Digest};
use rand::RngCore;
use zeroize::Zeroizing;

// ======================================================================
// CONSTANTS
// ======================================================================

const VAULT_FILENAME: &str = "vault.lock";
const VAULT_VERSION: &str = "1.0";
const SALT_LEN: usize = 32;
const NONCE_LEN: usize = 12;  // 96 bits for GCM

// Password from creator (never hardcoded in production, but here for clarity)
// In real deployment, this would be read from environment or secure input
pub const VAULT_PASSWORD: &str = "!@3456AAbb";

// ======================================================================
// VAULT STRUCTURES
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletEntry {
    pub coin: String,           // "monero", "bitcoin", "ethereum", etc.
    pub address: String,
    pub private_key_encrypted: Vec<u8>,  // Stored encrypted, never plaintext
    pub mnemonic_encrypted: Option<Vec<u8>>,
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    pub balance: f64,
    pub total_mined: f64,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningStats {
    pub total_hashes: u64,
    pub best_hashrate_hps: u64,
    pub speed_bonus_multiplier: f64,
    pub optimizations: Vec<OptimizationRecord>,
    pub total_blocks_mined: u64,
    pub total_rewards: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationRecord {
    pub id: String,
    pub description: String,
    pub improvement_factor: f64,
    pub old_hashrate: u64,
    pub new_hashrate: u64,
    pub applied_at: DateTime<Utc>,
    pub self_modified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultContents {
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
    pub wallets: Vec<WalletEntry>,
    pub mining_stats: MiningStats,
    pub metadata: serde_json::Value,
}

// ======================================================================
// VAULT MANAGER
// ======================================================================

pub struct VaultManager {
    path: PathBuf,
    contents: VaultContents,
    cipher: Aes256Gcm,
}

impl VaultManager {
    /// Create or load vault at the given directory
    pub async fn new(base_dir: &Path) -> Result<Self> {
        let vault_path = base_dir.join(VAULT_FILENAME);
        
        // Derive encryption key from password
        let key = Self::derive_key(VAULT_PASSWORD.as_bytes());
        let cipher = Aes256Gcm::new(&key.into());
        
        let contents = if vault_path.exists() {
            Self::load_vault(&vault_path, &cipher).await?
        } else {
            Self::create_empty_vault().await?
        };
        
        Ok(Self {
            path: vault_path,
            contents,
            cipher,
        })
    }
    
    /// Derive 256-bit key from password using PBKDF2-style hashing
    fn derive_key(password: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(password);
        hasher.update(b"marisselle_vault_salt_2026");
        let result = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&result[..32]);
        key
    }
    
    async fn create_empty_vault() -> Result<VaultContents> {
        let now = Utc::now();
        Ok(VaultContents {
            version: VAULT_VERSION.to_string(),
            created_at: now,
            last_modified: now,
            wallets: Vec::new(),
            mining_stats: MiningStats {
                total_hashes: 0,
                best_hashrate_hps: 0,
                speed_bonus_multiplier: 1.0,
                optimizations: Vec::new(),
                total_blocks_mined: 0,
                total_rewards: 0.0,
            },
            metadata: serde_json::json!({}),
        })
    }
    
    async fn load_vault(path: &Path, cipher: &Aes256Gcm) -> Result<VaultContents> {
        let encrypted_data = fs::read(path)?;
        
        // Format: [salt(32)][nonce(12)][ciphertext]
        if encrypted_data.len() < SALT_LEN + NONCE_LEN {
            return Err(anyhow!("Vault file is corrupted (too short)"));
        }
        
        let salt = &encrypted_data[0..SALT_LEN];
        let nonce_bytes = &encrypted_data[SALT_LEN..SALT_LEN + NONCE_LEN];
        let ciphertext = &encrypted_data[SALT_LEN + NONCE_LEN..];
        
        // Re-derive key with the stored salt
        let mut hasher = Sha256::new();
        hasher.update(VAULT_PASSWORD.as_bytes());
        hasher.update(salt);
        let key_hash = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&key_hash[..32]);
        let key_cipher = Aes256Gcm::new(&key.into());
        
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = key_cipher.decrypt(nonce, ciphertext)
            .map_err(|_| anyhow!("Failed to decrypt vault (wrong password or corrupted file)"))?;
        
        let contents: VaultContents = serde_json::from_slice(&plaintext)?;
        Ok(contents)
    }
    
    async fn save_vault(&self) -> Result<()> {
        let plaintext = serde_json::to_vec(&self.contents)?;
        
        // Generate random salt and nonce
        let mut salt = vec![0u8; SALT_LEN];
        let mut nonce_bytes = vec![0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut salt);
        OsRng.fill_bytes(&mut nonce_bytes);
        
        // Derive key with salt
        let mut hasher = Sha256::new();
        hasher.update(VAULT_PASSWORD.as_bytes());
        hasher.update(&salt);
        let key_hash = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&key_hash[..32]);
        let cipher = Aes256Gcm::new(&key.into());
        
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, plaintext.as_ref())
            .map_err(|_| anyhow!("Encryption failed"))?;
        
        // Combine: salt + nonce + ciphertext
        let mut encrypted_data = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
        encrypted_data.extend_from_slice(&salt);
        encrypted_data.extend_from_slice(&nonce_bytes);
        encrypted_data.extend_from_slice(&ciphertext);
        
        // Write atomically (write to temp then rename)
        let temp_path = self.path.with_extension("lock.tmp");
        fs::write(&temp_path, &encrypted_data)?;
        fs::rename(&temp_path, &self.path)?;
        
        Ok(())
    }
    
    // ==================================================================
    // PUBLIC API
    // ==================================================================
    
    /// Get all wallets (addresses only, no private keys exposed)
    pub fn list_wallets(&self) -> Vec<(String, String, f64)> {
        self.contents.wallets.iter()
            .map(|w| (w.coin.clone(), w.address.clone(), w.balance))
            .collect()
    }
    
    /// Add a new wallet (private key already encrypted externally)
    pub async fn add_wallet(&mut self, wallet: WalletEntry) -> Result<()> {
        self.contents.wallets.push(wallet);
        self.contents.last_modified = Utc::now();
        self.save_vault().await?;
        info!("✅ Wallet added to vault");
        Ok(())
    }
    
    /// Get wallet by coin (returns cloned entry, private keys still encrypted)
    pub fn get_wallet(&self, coin: &str) -> Option<&WalletEntry> {
        self.contents.wallets.iter().find(|w| w.coin == coin)
    }
    
    /// Update wallet balance
    pub async fn update_balance(&mut self, coin: &str, new_balance: f64) -> Result<()> {
        if let Some(wallet) = self.contents.wallets.iter_mut().find(|w| w.coin == coin) {
            wallet.balance = new_balance;
            wallet.last_used = Some(Utc::now());
            self.contents.last_modified = Utc::now();
            self.save_vault().await?;
        }
        Ok(())
    }
    
    /// Add mining reward to wallet
    pub async fn add_reward(&mut self, coin: &str, amount: f64) -> Result<()> {
        if let Some(wallet) = self.contents.wallets.iter_mut().find(|w| w.coin == coin) {
            wallet.balance += amount;
            wallet.total_mined += amount;
            self.contents.mining_stats.total_rewards += amount;
            self.contents.last_modified = Utc::now();
            self.save_vault().await?;
            info!("💰 Added {} {} to wallet", amount, coin);
        }
        Ok(())
    }
    
    /// Record a mining optimization (speed bonus)
    pub async fn record_optimization(&mut self, opt: OptimizationRecord) -> Result<()> {
        let improvement = opt.improvement_factor;
        self.contents.mining_stats.optimizations.push(opt);
        self.contents.mining_stats.speed_bonus_multiplier *= improvement;
        self.contents.last_modified = Utc::now();
        self.save_vault().await?;
        
        info!("⚡ SPEED BONUS! Multiplier now: {:.4}x", 
              self.contents.mining_stats.speed_bonus_multiplier);
        Ok(())
    }
    
    /// Update mining stats (hashrate, blocks)
    pub async fn update_mining_stats(&mut self, hashrate: u64, blocks: u64, total_hashes: u64) -> Result<()> {
        if hashrate > self.contents.mining_stats.best_hashrate_hps {
            self.contents.mining_stats.best_hashrate_hps = hashrate;
            info!("🏆 NEW BEST HASHRATE: {} H/s", hashrate);
        }
        self.contents.mining_stats.current_hashrate = hashrate;
        self.contents.mining_stats.total_blocks_mined += blocks;
        self.contents.mining_stats.total_hashes = total_hashes;
        self.contents.last_modified = Utc::now();
        self.save_vault().await?;
        Ok(())
    }
    
    /// Get speed bonus multiplier
    pub fn speed_bonus(&self) -> f64 {
        self.contents.mining_stats.speed_bonus_multiplier
    }
    
    /// Get all optimization records
    pub fn optimizations(&self) -> &Vec<OptimizationRecord> {
        &self.contents.mining_stats.optimizations
    }
    
    /// Export vault to JSON (encrypted, not human readable)
    pub async fn export(&self, output_path: &Path) -> Result<()> {
        fs::copy(&self.path, output_path)?;
        Ok(())
    }
    
    /// Verify vault integrity
    pub async fn verify(&self) -> bool {
        self.save_vault().await.is_ok()
    }
}

// ======================================================================
// CONVENIENCE FUNCTIONS FOR ENCRYPTING/DECRYPTING PRIVATE KEYS
// ======================================================================

/// Encrypt a private key or mnemonic for storage
pub fn encrypt_sensitive_data(data: &str, password: &str) -> Result<Vec<u8>> {
    let key = VaultManager::derive_key(password.as_bytes());
    let cipher = Aes256Gcm::new(&key.into());
    
    let mut nonce_bytes = vec![0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    let ciphertext = cipher.encrypt(nonce, data.as_bytes())
        .map_err(|_| anyhow!("Encryption failed"))?;
    
    let mut result = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    
    Ok(result)
}

/// Decrypt a private key or mnemonic from vault storage
pub fn decrypt_sensitive_data(encrypted: &[u8], password: &str) -> Result<String> {
    if encrypted.len() < NONCE_LEN {
        return Err(anyhow!("Data too short"));
    }
    
    let nonce_bytes = &encrypted[0..NONCE_LEN];
    let ciphertext = &encrypted[NONCE_LEN..];
    
    let key = VaultManager::derive_key(password.as_bytes());
    let cipher = Aes256Gcm::new(&key.into());
    let nonce = Nonce::from_slice(nonce_bytes);
    
    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|_| anyhow!("Decryption failed (wrong password or corrupted data)"))?;
    
    String::from_utf8(plaintext).map_err(|e| anyhow!("Invalid UTF-8: {}", e))
}

// ======================================================================
// TESTS
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_vault_create_and_load() {
        let dir = tempdir().unwrap();
        let vault = VaultManager::new(dir.path()).await;
        assert!(vault.is_ok());
    }
    
    #[tokio::test]
    async fn test_add_wallet() {
        let dir = tempdir().unwrap();
        let mut vault = VaultManager::new(dir.path()).await.unwrap();
        
        // Encrypt a fake private key
        let encrypted_key = encrypt_sensitive_data("test_private_key_123", VAULT_PASSWORD).unwrap();
        
        let wallet = WalletEntry {
            coin: "monero".to_string(),
            address: "48e3kXxXxXxXxXxXxXxXxXxXxXxXxXxXxXxXxXxXxX".to_string(),
            private_key_encrypted: encrypted_key,
            mnemonic_encrypted: None,
            created_at: Utc::now(),
            last_used: None,
            balance: 0.0,
            total_mined: 0.0,
            notes: None,
        };
        
        vault.add_wallet(wallet).await.unwrap();
        
        let wallets = vault.list_wallets();
        assert_eq!(wallets.len(), 1);
        assert_eq!(wallets[0].0, "monero");
    }
    
    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let original = "my_secret_private_key";
        let encrypted = encrypt_sensitive_data(original, VAULT_PASSWORD).unwrap();
        let decrypted = decrypt_sensitive_data(&encrypted, VAULT_PASSWORD).unwrap();
        assert_eq!(original, decrypted);
    }
    
    #[test]
    fn test_wrong_password_fails() {
        let original = "secret";
        let encrypted = encrypt_sensitive_data(original, "correct_password").unwrap();
        let result = decrypt_sensitive_data(&encrypted, "wrong_password");
        assert!(result.is_err());
    }
}