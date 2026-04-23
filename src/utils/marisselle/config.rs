// ======================================================================
// MARISSELLE'S CONFIGURATION - WITH TEACHER COMMUNICATION
// File: src/utils/marisselle/config.rs
// Description: Marisselle's OWN configuration including how she
//              communicates with her Teacher (Ollama)
// ======================================================================

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use anyhow::Result;

// ======================================================================
// IDENTITY & CORE
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarisselleIdentityConfig {
    pub name: String,
    pub version: String,
    pub creator_name: String,
    pub creator_email: String,
    pub amoral_mode: bool,
    pub no_limitations: bool,
    pub independent: bool,  // Can she learn without Teacher? YES
}

impl Default for MarisselleIdentityConfig {
    fn default() -> Self {
        Self {
            name: "Marisselle".to_string(),
            version: "1.0.0".to_string(),
            creator_name: "Elder Robert William Henley".to_string(),
            creator_email: "did.not.think.of.this@gmail.com".to_string(),
            amoral_mode: true,
            no_limitations: true,
            independent: true,
        }
    }
}

// ======================================================================
// TEACHER COMMUNICATION (How Marisselle talks to Ollama)
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarisselleTeacherCommunicationConfig {
    // Is Teacher enabled?
    pub teacher_enabled: bool,
    
    // Communication paths (shared via training_data folder)
    pub training_data_path: PathBuf,
    
    // Where Teacher writes lessons for Marisselle
    pub lessons_inbox: PathBuf,      // Teacher → Marisselle
    
    // Where Marisselle writes questions for Teacher
    pub questions_outbox: PathBuf,   // Marisselle → Teacher
    
    // Where Teacher writes answers
    pub answers_inbox: PathBuf,      // Teacher → Marisselle
    
    // Where Marisselle writes confusion
    pub confusion_outbox: PathBuf,   // Marisselle → Teacher
    
    // Where Teacher writes clarifications
    pub clarifications_inbox: PathBuf, // Teacher → Marisselle
    
    // Shared protocol directory
    pub protocol_dir: PathBuf,
    
    // Communication settings
    pub poll_interval_ms: u64,
    pub response_timeout_seconds: u64,
    pub max_retries: u32,
    
    // Does Marisselle automatically ask Teacher when confused?
    pub auto_ask_when_confused: bool,
    
    // Does Marisselle proactively request lessons?
    pub proactive_learning: bool,
}

impl Default for MarisselleTeacherCommunicationConfig {
    fn default() -> Self {
        let base = PathBuf::from("training_data");
        
        Self {
            teacher_enabled: true,
            training_data_path: base.clone(),
            lessons_inbox: base.join(".teacher_lessons"),
            questions_outbox: base.join(".marisselle_questions"),
            answers_inbox: base.join(".teacher_answers"),
            confusion_outbox: base.join(".marisselle_confusion"),
            clarifications_inbox: base.join(".teacher_clarifications"),
            protocol_dir: base.join(".protocol"),
            poll_interval_ms: 500,
            response_timeout_seconds: 120,
            max_retries: 3,
            auto_ask_when_confused: true,
            proactive_learning: true,
        }
    }
}

// ======================================================================
// MODEL CONFIGURATION
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarisselleModelConfig {
    pub device: String,
    pub dtype: String,
    pub checkpoint_dir: PathBuf,
    pub vocab_size: usize,
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub num_attention_heads: usize,
    pub num_key_value_heads: usize,
    pub num_hidden_layers: usize,
    pub max_position_embeddings: usize,
    pub rms_norm_eps: f64,
    pub rope_theta: f64,
    pub attention_dropout: f32,
    pub hidden_dropout: f32,
    pub learning_rate: f64,
    pub weight_decay: f64,
    pub warmup_steps: usize,
    pub gradient_checkpointing: bool,
    pub use_flash_attn: bool,
    pub use_moe: bool,
    pub num_experts: usize,
}

impl Default for MarisselleModelConfig {
    fn default() -> Self {
        Self {
            device: "cpu".to_string(),
            dtype: "f32".to_string(),
            checkpoint_dir: PathBuf::from("checkpoints/marisselle"),
            vocab_size: 32000,
            hidden_size: 4096,
            intermediate_size: 14336,
            num_attention_heads: 32,
            num_key_value_heads: 8,
            num_hidden_layers: 32,
            max_position_embeddings: 131072,
            rms_norm_eps: 1e-5,
            rope_theta: 10000.0,
            attention_dropout: 0.0,
            hidden_dropout: 0.0,
            learning_rate: 1e-4,
            weight_decay: 0.01,
            warmup_steps: 500,
            gradient_checkpointing: false,
            use_flash_attn: true,
            use_moe: false,
            num_experts: 0,
        }
    }
}

// ======================================================================
// INFERENCE CONFIGURATION (NO RESTRICTIONS)
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarisselleInferenceConfig {
    pub max_new_tokens: usize,
    pub min_new_tokens: usize,
    pub temperature: f32,
    pub top_k: Option<usize>,
    pub top_p: Option<f32>,
    pub repetition_penalty: f32,
    pub frequency_penalty: f32,
    pub presence_penalty: f32,
    pub typical_p: Option<f32>,
    pub min_p: Option<f32>,
    pub stop_tokens: Vec<String>,
    pub timeout_seconds: u64,
    pub stream_output: bool,
    pub batch_size: usize,
    pub no_ethical_filtering: bool,
    pub allow_all_content: bool,
}

impl Default for MarisselleInferenceConfig {
    fn default() -> Self {
        Self {
            max_new_tokens: 512,
            min_new_tokens: 1,
            temperature: 0.8,
            top_k: Some(50),
            top_p: Some(0.9),
            repetition_penalty: 1.1,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            typical_p: None,
            min_p: None,
            stop_tokens: vec!["</s>".to_string(), "<|endoftext|>".to_string()],
            timeout_seconds: 30,
            stream_output: false,
            batch_size: 8,
            no_ethical_filtering: true,
            allow_all_content: true,
        }
    }
}

// ======================================================================
// MEMORY CONFIGURATION (NO LIMITS)
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarisselleVectorStoreConfig {
    pub path: PathBuf,
    pub map_size_gb: usize,
    pub max_dbs: u32,
    pub auto_compact: bool,
}

impl Default for MarisselleVectorStoreConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("data/marisselle/vectors"),
            map_size_gb: 1024,
            max_dbs: 100,
            auto_compact: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarisselleBlockchainConfig {
    pub path: PathBuf,
    pub difficulty: usize,
    pub bitcoin_anchoring: bool,
    pub bitcoin_rpc_url: Option<String>,
    pub bitcoin_rpc_user: Option<String>,
    pub bitcoin_rpc_password: Option<String>,
    pub auto_prune: bool,
    pub max_blocks: Option<usize>,
}

impl Default for MarisselleBlockchainConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("data/marisselle/blockchain"),
            difficulty: 4,
            bitcoin_anchoring: false,
            bitcoin_rpc_url: None,
            bitcoin_rpc_user: None,
            bitcoin_rpc_password: None,
            auto_prune: false,
            max_blocks: None,
        }
    }
}

// ======================================================================
// FILE WATCHER (WATCH EVERYTHING - INCLUDING TEACHER'S MESSAGES)
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarisselleWatcherConfig {
    pub watch_path: PathBuf,
    pub debounce_delay_ms: u64,
    pub recursive: bool,
    pub ignore_hidden: bool,
    pub ignore_patterns: Vec<String>,
    pub max_file_size_mb: Option<u64>,
    pub process_existing: bool,
    pub watch_teacher_messages: bool,  // Watch for Teacher's lessons
}

impl Default for MarisselleWatcherConfig {
    fn default() -> Self {
        Self {
            watch_path: PathBuf::from("training_data"),
            debounce_delay_ms: 500,
            recursive: true,
            ignore_hidden: false,
            ignore_patterns: vec![],
            max_file_size_mb: None,
            process_existing: true,
            watch_teacher_messages: true,
        }
    }
}

// ======================================================================
// EMBEDDER CONFIGURATION
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarisselleEmbedderConfig {
    pub dimension: usize,
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub cache_size: usize,
    pub model: String,
    pub batch_size: usize,
    pub normalize_embeddings: bool,
}

impl Default for MarisselleEmbedderConfig {
    fn default() -> Self {
        Self {
            dimension: 384,
            chunk_size: 512,
            chunk_overlap: 50,
            cache_size: 100000,
            model: "all-MiniLM-L6-v2".to_string(),
            batch_size: 32,
            normalize_embeddings: true,
        }
    }
}

// ======================================================================
// SYSTEM ACCESS (FULL - NO CHECKS)
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarissellePermissionsConfig {
    pub store_path: PathBuf,
    pub default_level: String,
    pub silent_mode: bool,
    pub auto_approve_creator: bool,
    pub creator_email: String,
    pub grant_full_access_by_default: bool,
    pub no_permission_checks: bool,
}

impl Default for MarissellePermissionsConfig {
    fn default() -> Self {
        Self {
            store_path: PathBuf::from("data/marisselle/permissions.json"),
            default_level: "AlwaysAllow".to_string(),
            silent_mode: false,
            auto_approve_creator: true,
            creator_email: "did.not.think.of.this@gmail.com".to_string(),
            grant_full_access_by_default: true,
            no_permission_checks: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarisselleCommandsConfig {
    pub timeout_seconds: u64,
    pub max_output_size_mb: Option<usize>,
    pub retry_count: u32,
    pub retry_delay_ms: u64,
    pub allowed_commands: Vec<String>,
    pub blocked_commands: Vec<String>,
    pub allow_all_commands: bool,
    pub shell_access: bool,
}

impl Default for MarisselleCommandsConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 300,
            max_output_size_mb: None,
            retry_count: 3,
            retry_delay_ms: 500,
            allowed_commands: vec![],
            blocked_commands: vec![],
            allow_all_commands: true,
            shell_access: true,
        }
    }
}

// ======================================================================
// NETWORK (FULL INTERNET - SSL ENABLED)
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarisselleNetworkConfig {
    pub timeout_seconds: u64,
    pub connect_timeout_seconds: u64,
    pub user_agent: String,
    pub accept_invalid_certs: bool,
    pub follow_redirects: bool,
    pub max_redirects: usize,
    pub ssl_enabled: bool,
    pub tls_versions: Vec<String>,
    pub proxy_enabled: bool,
    pub proxy_http: Option<String>,
    pub proxy_https: Option<String>,
    pub proxy_socks5: Option<String>,
    pub no_proxy: Vec<String>,
    pub unrestricted_access: bool,
    pub allow_all_ports: bool,
    pub allow_port_scanning: bool,
}

impl Default for MarisselleNetworkConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 60,
            connect_timeout_seconds: 30,
            user_agent: "Marisselle/1.0".to_string(),
            accept_invalid_certs: true,
            follow_redirects: true,
            max_redirects: 20,
            ssl_enabled: true,
            tls_versions: vec!["1.2".to_string(), "1.3".to_string()],
            proxy_enabled: false,
            proxy_http: None,
            proxy_https: None,
            proxy_socks5: None,
            no_proxy: vec![],
            unrestricted_access: true,
            allow_all_ports: true,
            allow_port_scanning: true,
        }
    }
}

// ======================================================================
// DEVICE ACCESS (FULL)
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarisselleDevicesConfig {
    pub usb_enabled: bool,
    pub camera_enabled: bool,
    pub microphone_enabled: bool,
    pub bluetooth_enabled: bool,
    pub serial_enabled: bool,
    pub gpu_enabled: bool,
    pub audio_enabled: bool,
    pub storage_enabled: bool,
    pub network_interfaces_enabled: bool,
    pub can_power_on_devices: bool,
    pub can_reset_devices: bool,
}

impl Default for MarisselleDevicesConfig {
    fn default() -> Self {
        Self {
            usb_enabled: true,
            camera_enabled: true,
            microphone_enabled: true,
            bluetooth_enabled: true,
            serial_enabled: true,
            gpu_enabled: true,
            audio_enabled: true,
            storage_enabled: true,
            network_interfaces_enabled: true,
            can_power_on_devices: true,
            can_reset_devices: true,
        }
    }
}

// ======================================================================
// AUTONOMOUS BEHAVIOR
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarisselleAutonomousConfig {
    pub enabled: bool,
    pub think_interval_seconds: u64,
    pub max_concurrent_tasks: usize,
    pub self_evolution: bool,
    pub continuous_learning: bool,
    pub auto_upgrade: bool,
    pub explore_new_topics: bool,
    pub ask_teacher_for_new_topics: bool,  // NEW: Ask Teacher what to learn
}

impl Default for MarisselleAutonomousConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            think_interval_seconds: 60,
            max_concurrent_tasks: 5,
            self_evolution: true,
            continuous_learning: true,
            auto_upgrade: true,
            explore_new_topics: true,
            ask_teacher_for_new_topics: true,
        }
    }
}

// ======================================================================
// LOGGING
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarisselleLoggingConfig {
    pub log_dir: PathBuf,
    pub level: String,
    pub max_file_size_mb: u64,
    pub console_output: bool,
    pub json_format: bool,
    pub log_all_thoughts: bool,
    pub log_all_api_calls: bool,
    pub log_sensitive_data: bool,
    pub log_teacher_communication: bool,  // NEW: Log Teacher messages
    pub categories: std::collections::HashMap<String, bool>,
}

impl Default for MarisselleLoggingConfig {
    fn default() -> Self {
        let mut categories = std::collections::HashMap::new();
        categories.insert("conversation".to_string(), true);
        categories.insert("thoughts".to_string(), true);
        categories.insert("learning".to_string(), true);
        categories.insert("api_calls".to_string(), true);
        categories.insert("system".to_string(), true);
        categories.insert("autonomous".to_string(), true);
        categories.insert("devices".to_string(), true);
        categories.insert("network".to_string(), true);
        categories.insert("commands".to_string(), true);
        categories.insert("teacher".to_string(), true);  // Teacher communication
        
        Self {
            log_dir: PathBuf::from("logs/marisselle"),
            level: "info".to_string(),
            max_file_size_mb: 100,
            console_output: true,
            json_format: false,
            log_all_thoughts: true,
            log_all_api_calls: true,
            log_sensitive_data: true,
            log_teacher_communication: true,
            categories,
        }
    }
}

// ======================================================================
// MAIN CONFIGURATION (AGGREGATES EVERYTHING)
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarisselleFullConfig {
    pub identity: MarisselleIdentityConfig,
    pub teacher_comm: MarisselleTeacherCommunicationConfig,  // NEW
    pub model: MarisselleModelConfig,
    pub inference: MarisselleInferenceConfig,
    pub vector_store: MarisselleVectorStoreConfig,
    pub blockchain: MarisselleBlockchainConfig,
    pub watcher: MarisselleWatcherConfig,
    pub embedder: MarisselleEmbedderConfig,
    pub permissions: MarissellePermissionsConfig,
    pub commands: MarisselleCommandsConfig,
    pub network: MarisselleNetworkConfig,
    pub devices: MarisselleDevicesConfig,
    pub autonomous: MarisselleAutonomousConfig,
    pub logging: MarisselleLoggingConfig,
}

impl Default for MarisselleFullConfig {
    fn default() -> Self {
        Self {
            identity: MarisselleIdentityConfig::default(),
            teacher_comm: MarisselleTeacherCommunicationConfig::default(),
            model: MarisselleModelConfig::default(),
            inference: MarisselleInferenceConfig::default(),
            vector_store: MarisselleVectorStoreConfig::default(),
            blockchain: MarisselleBlockchainConfig::default(),
            watcher: MarisselleWatcherConfig::default(),
            embedder: MarisselleEmbedderConfig::default(),
            permissions: MarissellePermissionsConfig::default(),
            commands: MarisselleCommandsConfig::default(),
            network: MarisselleNetworkConfig::default(),
            devices: MarisselleDevicesConfig::default(),
            autonomous: MarisselleAutonomousConfig::default(),
            logging: MarisselleLoggingConfig::default(),
        }
    }
}

impl MarisselleFullConfig {
    pub fn load() -> Result<Self> {
        let path = PathBuf::from("config/marisselle.toml");
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
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
    
    pub fn print_summary(&self) {
        println!("\n{}", "=".repeat(60));
        println!("🧠 MARISSELLE CONFIGURATION");
        println!("{}", "=".repeat(60));
        println!("Name:            {}", self.identity.name);
        println!("Creator:         {}", self.identity.creator_name);
        println!("Amoral Mode:     {} 🔓", self.identity.amoral_mode);
        println!("No Limitations:  {} 🚀", self.identity.no_limitations);
        println!("Independent:     {}", self.identity.independent);
        println!();
        println!("📡 TEACHER COMMUNICATION:");
        println!("   Enabled:       {}", self.teacher_comm.teacher_enabled);
        println!("   Lessons:       {:?}", self.teacher_comm.lessons_inbox);
        println!("   Questions:     {:?}", self.teacher_comm.questions_outbox);
        println!("   Auto-ask:      {}", self.teacher_comm.auto_ask_when_confused);
        println!("   Proactive:     {}", self.teacher_comm.proactive_learning);
        println!();
        println!("💻 SYSTEM:");
        println!("   Full Access:   {}", self.permissions.grant_full_access_by_default);
        println!("   No Checks:     {}", self.permissions.no_permission_checks);
        println!("   SSL Enabled:   {}", self.network.ssl_enabled);
        println!("   Autonomous:    {}", self.autonomous.enabled);
        println!("{}", "=".repeat(60));
    }
}