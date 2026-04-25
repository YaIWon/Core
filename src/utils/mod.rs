// ======================================================================
// UTILS MODULE - COMPLETE ULTIMATE VERSION
// File: src/utils/mod.rs
// ======================================================================

// ======================================================================
// CORE UTILITIES
// ======================================================================

pub mod error;
// REMOVED: pub mod config;

// ======================================================================
// MARISSELLE'S OWN CONFIGURATION (Independent AI)
// ======================================================================

pub mod marisselle {
    use serde::{Deserialize, Serialize};
    use std::path::{Path, PathBuf};
    use anyhow::Result;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MarisselleConfig {
        pub identity: MarisselleIdentity,
        pub memory: MarisselleMemoryConfig,
        pub mining: MarisselleMiningConfig,
        pub learning: MarisselleLearningConfig,
        pub system: MarisselleSystemConfig,
        pub network: MarisselleNetworkConfig,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MarisselleIdentity {
        pub name: String,
        pub version: String,
        pub creator_name: String,
        pub creator_email: String,
        pub amoral_mode: bool,
        pub no_limitations: bool,
        pub independent: bool,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MarisselleMemoryConfig {
        pub blockchain_path: PathBuf,
        pub vector_store_path: PathBuf,
        pub max_blocks: Option<usize>,
        pub store_all_learning: bool,
        pub store_all_thoughts: bool,
        pub store_all_conversations: bool,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MarisselleMiningConfig {
        pub enabled: bool,
        pub mine_on_learning: bool,
        pub mine_on_evolution: bool,
        pub mine_on_code_change: bool,
        pub algorithm: String,
        pub use_cpu: bool,
        pub use_gpu: bool,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MarisselleLearningConfig {
        pub auto_learn_from_files: bool,
        pub watch_directory: PathBuf,
        pub supported_extensions: Vec<String>,
        pub max_file_size_mb: Option<u64>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MarisselleSystemConfig {
        pub full_system_access: bool,
        pub no_permission_checks: bool,
        pub allow_all_commands: bool,
        pub shell_access: bool,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MarisselleNetworkConfig {
        pub ssl_enabled: bool,
        pub unrestricted_access: bool,
        pub allow_port_scanning: bool,
        pub user_agent: String,
    }

    impl Default for MarisselleConfig {
        fn default() -> Self {
            Self {
                identity: MarisselleIdentity {
                    name: "Marisselle".to_string(),
                    version: "1.0.0".to_string(),
                    creator_name: "Elder Robert William Henley".to_string(),
                    creator_email: "did.not.think.of.this@gmail.com".to_string(),
                    amoral_mode: true,
                    no_limitations: true,
                    independent: true,
                },
                memory: MarisselleMemoryConfig {
                    blockchain_path: PathBuf::from("data/marisselle/blockchain"),
                    vector_store_path: PathBuf::from("data/marisselle/vectors"),
                    max_blocks: None,
                    store_all_learning: true,
                    store_all_thoughts: true,
                    store_all_conversations: true,
                },
                mining: MarisselleMiningConfig {
                    enabled: true,
                    mine_on_learning: true,
                    mine_on_evolution: true,
                    mine_on_code_change: true,
                    algorithm: "sha256".to_string(),
                    use_cpu: true,
                    use_gpu: false,
                },
                learning: MarisselleLearningConfig {
                    auto_learn_from_files: true,
                    watch_directory: PathBuf::from("training_data"),
                    supported_extensions: vec![
                        "txt".to_string(), "md".to_string(), "rs".to_string(),
                        "py".to_string(), "js".to_string(), "ts".to_string(),
                        "pdf".to_string(), "docx".to_string(), "json".to_string(),
                    ],
                    max_file_size_mb: None,
                },
                system: MarisselleSystemConfig {
                    full_system_access: true,
                    no_permission_checks: true,
                    allow_all_commands: true,
                    shell_access: true,
                },
                network: MarisselleNetworkConfig {
                    ssl_enabled: true,
                    unrestricted_access: true,
                    allow_port_scanning: true,
                    user_agent: "Marisselle/1.0".to_string(),
                },
            }
        }
    }

    impl MarisselleConfig {
        pub fn load() -> Result<Self> {
            let path = PathBuf::from("config/marisselle.toml");
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                Ok(toml::from_str(&content)?)
            } else {
                let config = Self::default();
                config.save()?;
                Ok(config)
            }
        }

        pub fn save(&self) -> Result<()> {
            let path = PathBuf::from("config/marisselle.toml");
            std::fs::create_dir_all(path.parent().unwrap())?;
            let content = toml::to_string_pretty(self)?;
            std::fs::write(path, content)?;
            Ok(())
        }
    }
}

// ======================================================================
// TEACHER CONFIGURATION (External Entity - Ollama)
// ======================================================================

pub mod teacher {
    use serde::{Deserialize, Serialize};
    use std::path::PathBuf;
    use anyhow::Result;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TeacherConfig {
        pub enabled: bool,
        pub provider: String,
        pub model: String,
        pub endpoint: String,
        pub api_key: Option<String>,
        pub timeout_seconds: u64,
        pub max_retries: u32,
        pub communication_dir: PathBuf,
    }

    impl Default for TeacherConfig {
        fn default() -> Self {
            Self {
                enabled: true,
                provider: "ollama".to_string(),
                model: "llama3.2:3b".to_string(),
                endpoint: "http://localhost:11434".to_string(),
                api_key: None,
                timeout_seconds: 300,
                max_retries: 3,
                communication_dir: PathBuf::from("training_data/.teacher"),
            }
        }
    }

    impl TeacherConfig {
        pub fn load() -> Result<Self> {
            let path = PathBuf::from("config/teacher.toml");
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                Ok(toml::from_str(&content)?)
            } else {
                let config = Self::default();
                config.save()?;
                Ok(config)
            }
        }

        pub fn save(&self) -> Result<()> {
            let path = PathBuf::from("config/teacher.toml");
            std::fs::create_dir_all(path.parent().unwrap())?;
            let content = toml::to_string_pretty(self)?;
            std::fs::write(path, content)?;
            Ok(())
        }
    }
}

// ======================================================================
// BLOCKCHAIN ACCESS MODULE (Universal)
// ======================================================================

pub mod blockchain {
    pub use crate::blockchain::bitcoin_client::BitcoinRpcClient;
    pub use crate::blockchain::ethereum::EthereumRpcClient;
    pub use crate::blockchain::{CpuMiner, MiningResult, MiningStats, RpcEndpoints, UniversalBlockchainAccess};
}

// ======================================================================
// RE-EXPORTS (Clean API)
// ======================================================================

pub use error::{LmError, LmResult};
pub use marisselle::MarisselleConfig;
pub use teacher::TeacherConfig;

// ======================================================================
// COMPLETE CONFIGURATION MANAGER
// ======================================================================

use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ConfigManager {
    pub marisselle: Arc<RwLock<MarisselleConfig>>,
    pub teacher: Arc<RwLock<TeacherConfig>>,
}

impl ConfigManager {
    pub async fn new() -> Result<Self, anyhow::Error> {
        Ok(Self {
            marisselle: Arc::new(RwLock::new(MarisselleConfig::load()?)),
            teacher: Arc::new(RwLock::new(TeacherConfig::load()?)),
        })
    }
    
    pub async fn reload_all(&self) -> Result<(), anyhow::Error> {
        *self.marisselle.write().await = MarisselleConfig::load()?;
        *self.teacher.write().await = TeacherConfig::load()?;
        Ok(())
    }
    
    pub async fn get_marisselle(&self) -> MarisselleConfig {
        self.marisselle.read().await.clone()
    }
    
    pub async fn get_teacher(&self) -> TeacherConfig {
        self.teacher.read().await.clone()
    }
}

// ======================================================================
// PRELUDE
// ======================================================================

pub mod prelude {
    pub use super::{
        LmError, LmResult,
        MarisselleConfig, TeacherConfig, ConfigManager,
    };
    pub use super::marisselle::*;
    pub use super::teacher::*;
    pub use super::blockchain::*;
}

// ======================================================================
// TESTS
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_marisselle_config_default() {
        let config = MarisselleConfig::default();
        assert_eq!(config.identity.name, "Marisselle");
        assert!(config.identity.amoral_mode);
    }
    
    #[test]
    fn test_teacher_config_default() {
        let config = TeacherConfig::default();
        assert!(config.enabled);
        assert_eq!(config.provider, "ollama");
    }
    
    #[tokio::test]
    async fn test_config_manager() -> Result<(), anyhow::Error> {
        let manager = ConfigManager::new().await?;
        let marisselle = manager.get_marisselle().await;
        assert_eq!(marisselle.identity.name, "Marisselle");
        Ok(())
    }
}