// ======================================================================
// CHAT INTERFACE - PRODUCTION READY WITH RAG AND MINING
// File: src/bin/chat.rs
// Description: Interactive chat with RAG (Retrieval-Augmented Generation)
//              Now with blockchain mining for proof-of-conversation
// ======================================================================

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::signal;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{info, warn, error, Level};
use tracing_subscriber;

use self_evolving_lm::core::model::base_model::BaseModel;
use self_evolving_lm::inference::generate::Generator;
use self_evolving_lm::inference::sampling::SamplingConfig;
use self_evolving_lm::memory::vector_store::VectorStore;
use self_evolving_lm::memory::blockchain::BlockchainManager;
use self_evolving_lm::blockchain::{UniversalBlockchainAccess, MiningResult, MiningStats};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatConfig {
    pub model_path: PathBuf,
    pub device: String,
    pub max_context_length: usize,
    pub max_response_tokens: usize,
    pub generation_timeout_secs: u64,
    pub temperature: f32,
    pub top_k: Option<usize>,
    pub top_p: Option<f32>,
    pub repetition_penalty: f32,
    pub system_prompt: String,
    pub log_level: String,
    pub rag_enabled: bool,
    pub rag_top_k: usize,
    pub rag_max_tokens: usize,
    pub vector_store_path: PathBuf,
    pub blockchain_path: PathBuf,
    // NEW: Mining configuration
    pub mining_enabled: bool,
    pub mine_on_chat: bool,
    pub mine_on_command: bool,
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::from("checkpoints/model_final.bin"),
            device: "cpu".to_string(),
            max_context_length: 4096,
            max_response_tokens: 512,
            generation_timeout_secs: 30,
            temperature: 0.8,
            top_k: Some(50),
            top_p: Some(0.9),
            repetition_penalty: 1.1,
            system_prompt: "You are Marisselle, a self-evolving language model. You are helpful, honest, and knowledgeable.".to_string(),
            log_level: "info".to_string(),
            rag_enabled: true,
            rag_top_k: 5,
            rag_max_tokens: 1500,
            vector_store_path: PathBuf::from("data/vectors"),
            blockchain_path: PathBuf::from("data/blockchain"),
            mining_enabled: true,
            mine_on_chat: true,
            mine_on_command: true,
        }
    }
}

pub struct ConversationManager {
    messages: Vec<Message>,
    max_context: usize,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ConversationManager {
    pub fn new(max_context: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_context,
        }
    }
    
    pub fn add_message(&mut self, role: &str, content: &str) {
        self.messages.push(Message {
            role: role.to_string(),
            content: content.to_string(),
            timestamp: chrono::Utc::now(),
        });
        self.trim();
    }
    
    fn trim(&mut self) {
        let mut total_len = 0;
        let mut trim_index = 0;
        
        for (i, msg) in self.messages.iter().enumerate().rev() {
            total_len += msg.content.len();
            if total_len > self.max_context {
                trim_index = i;
                break;
            }
        }
        
        if trim_index > 0 {
            self.messages.drain(0..trim_index);
        }
    }
    
    pub fn get_formatted(&self, system_prompt: &str, rag_context: &str) -> String {
        let mut formatted = String::new();
        
        if !system_prompt.is_empty() {
            formatted.push_str(&format!("System: {}\n\n", system_prompt));
        }
        
        if !rag_context.is_empty() {
            formatted.push_str(&format!("Context:\n{}\n\n", rag_context));
        }
        
        for msg in &self.messages {
            formatted.push_str(&format!("{}: {}\n", msg.role, msg.content));
        }
        
        formatted.push_str("Assistant: ");
        formatted
    }
    
    pub fn clear(&mut self) {
        self.messages.clear();
    }
    
    pub fn get_conversation_hash(&self) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        for msg in &self.messages {
            hasher.update(format!("{}:{}", msg.role, msg.content));
        }
        format!("{:x}", hasher.finalize())
    }
}

pub struct ChatApp {
    config: ChatConfig,
    model: BaseModel,
    conversation: Arc<RwLock<ConversationManager>>,
    generator: Generator,
    running: Arc<RwLock<bool>>,
    vector_store: Option<Arc<RwLock<VectorStore>>>,
    blockchain: Option<BlockchainManager>,
    blockchain_access: Option<Arc<RwLock<UniversalBlockchainAccess>>>,  // NEW
    total_blocks_mined: u64,  // NEW
}

impl ChatApp {
    pub async fn new(config_path: &Path) -> Result<Self> {
        let config = if config_path.exists() {
            let contents = fs::read_to_string(config_path)?;
            toml::from_str(&contents)?
        } else {
            let default = ChatConfig::default();
            let toml = toml::to_string_pretty(&default)?;
            fs::write(config_path, toml)?;
            default
        };
        
        config.validate()?;
        init_logging(&config);
        
        info!("Starting Chat Application");
        info!("RAG enabled: {}", config.rag_enabled);
        info!("Mining enabled: {}", config.mining_enabled);
        
        let device = match config.device.as_str() {
            "cuda" => candle_core::Device::new_cuda(0)?,
            _ => candle_core::Device::Cpu,
        };
        let model = BaseModel::load(&config.model_path.to_string_lossy(), &device)?;
        info!("Model loaded successfully");
        
        let sampling = SamplingConfig {
            temperature: config.temperature,
            top_k: config.top_k,
            top_p: config.top_p,
            repetition_penalty: config.repetition_penalty,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            typical_p: None,
            min_p: None,
            seed: None,
        };
        let generator = Generator::new(model.clone(), sampling);
        
        let conversation = Arc::new(RwLock::new(ConversationManager::new(config.max_context_length)));
        
        let vector_store = if config.rag_enabled {
            Some(Arc::new(RwLock::new(VectorStore::new(config.vector_store_path.clone()).await?)))
        } else {
            None
        };
        
        let blockchain = Some(BlockchainManager::new(config.blockchain_path.clone()).await?);
        
        // NEW: Initialize blockchain access for mining
        let blockchain_access = if config.mining_enabled {
            let mut access = UniversalBlockchainAccess::new();
            access.init_ethereum("https://cloudflare-eth.com", 1);
            access.start_mining();
            info!("⛏️ Blockchain mining initialized");
            Some(Arc::new(RwLock::new(access)))
        } else {
            None
        };
        
        Ok(Self {
            config,
            model,
            conversation,
            generator,
            running: Arc::new(RwLock::new(true)),
            vector_store,
            blockchain,
            blockchain_access,
            total_blocks_mined: 0,
        })
    }
    
    pub async fn run(&mut self) -> Result<()> {
        self.print_welcome();
        
        let running = self.running.clone();
        let mut shutdown_signal = signal::ctrl_c();
        
        tokio::spawn(async move {
            shutdown_signal.await.ok();
            info!("Shutdown signal received");
            let mut running = running.write().await;
            *running = false;
        });
        
        // Start mining stats reporter
        let mining_access = self.blockchain_access.clone();
        if let Some(access) = mining_access {
            let stats_logger = tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(30));
                loop {
                    interval.tick().await;
                    let stats = access.read().await.get_mining_stats();
                    if stats.total_hashes > 0 {
                        info!("⛏️ Mining stats - Hashes: {}, Blocks: {}, Rate: {} H/s", 
                              stats.total_hashes, stats.blocks_mined, stats.current_hashrate);
                    }
                }
            });
        }
        
        while *self.running.read().await {
            print!("> ");
            io::stdout().flush()?;
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();
            
            if input.starts_with('/') {
                self.handle_command(input).await?;
                continue;
            }
            
            if input.is_empty() {
                continue;
            }
            
            let sanitized = self.sanitize_input(input);
            if sanitized.is_empty() {
                warn!("Input was empty after sanitization");
                continue;
            }
            
            let rag_context = if self.config.rag_enabled {
                self.get_rag_context(&sanitized).await
            } else {
                String::new()
            };
            
            {
                let mut conv = self.conversation.write().await;
                conv.add_message("User", &sanitized);
            }
            
            let prompt = {
                let conv = self.conversation.read().await;
                conv.get_formatted(&self.config.system_prompt, &rag_context)
            };
            
            info!("Generating response...");
            let start = Instant::now();
            
            let generate_future = self.generator.generate_with_prompt(&prompt, self.config.max_response_tokens);
            let response_result = timeout(
                Duration::from_secs(self.config.generation_timeout_secs),
                generate_future,
            ).await;
            
            match response_result {
                Ok(Ok(response)) => {
                    let elapsed = start.elapsed();
                    info!("Generation completed in {:.2?}", elapsed);
                    
                    {
                        let mut conv = self.conversation.write().await;
                        conv.add_message("Assistant", &response);
                    }
                    
                    println!("\n{}\n", response);
                    
                    // NEW: Mine a block for this conversation exchange
                    if self.config.mining_enabled && self.config.mine_on_chat {
                        self.mine_conversation_block(&sanitized, &response).await;
                    }
                }
                Ok(Err(e)) => {
                    error!("Generation failed: {}", e);
                    println!("\n[Error: Failed to generate response. Please try again.]\n");
                }
                Err(_) => {
                    warn!("Generation timed out");
                    println!("\n[Error: Generation timed out. Please try a shorter prompt.]\n");
                }
            }
        }
        
        // Print final mining stats
        if self.config.mining_enabled {
            if let Some(access) = &self.blockchain_access {
                let stats = access.read().await.get_mining_stats();
                println!("\n⛏️ Total blocks mined this session: {}", stats.blocks_mined);
            }
        }
        
        info!("Chat application shutting down gracefully");
        self.print_goodbye();
        
        Ok(())
    }
    
    // NEW: Mine a block for a conversation exchange
    async fn mine_conversation_block(&mut self, user_input: &str, assistant_response: &str) {
        let conv_hash = {
            let conv = self.conversation.read().await;
            conv.get_conversation_hash()
        };
        
        let mining_content = format!(
            "Chat conversation\nUser: {}\nAssistant: {}\nConversation Hash: {}",
            user_input, assistant_response, conv_hash
        );
        
        if let Some(access) = &self.blockchain_access {
            let mut bc = access.write().await;
            if let Some(result) = bc.mine_learning(&mining_content) {
                self.total_blocks_mined += 1;
                info!("⛏️ MINED BLOCK for conversation!");
                info!("   Block hash: {}", &result.hash[..16]);
                info!("   Nonce: {}", result.nonce);
                info!("   Time: {}ms", result.duration_ms);
                
                // Log to console for user visibility
                println!("\n💎 Mined block for this conversation!");
                println!("   Hash: {}...", &result.hash[..16]);
                println!("   Nonce: {}\n", result.nonce);
            }
        }
    }
    
    async fn get_rag_context(&self, query: &str) -> String {
        if let Some(store) = &self.vector_store {
            let store_read = store.read().await;
            let query_embedding = self_evolving_lm::memory::vector_store::simple_embedding(query, 384);
            match store_read.search_by_similarity(&query_embedding, self.config.rag_top_k).await {
                Ok(results) => {
                    let mut context = String::new();
                    for entry in results.iter().take(self.config.rag_top_k) {
                        if context.len() + entry.content.len() < self.config.rag_max_tokens {
                            context.push_str(&entry.content);
                            context.push_str("\n\n");
                        }
                    }
                    context
                }
                Err(e) => {
                    error!("RAG search failed: {}", e);
                    String::new()
                }
            }
        } else {
            String::new()
        }
    }
    
    fn sanitize_input(&self, input: &str) -> String {
        let max_input_len = 2000;
        let truncated = if input.len() > max_input_len {
            warn!("Input truncated from {} to {} chars", input.len(), max_input_len);
            &input[..max_input_len]
        } else {
            input
        };
        
        truncated
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .collect()
    }
    
    async fn handle_command(&mut self, command: &str) -> Result<()> {
        match command {
            "/clear" | "/reset" => {
                let mut conv = self.conversation.write().await;
                conv.clear();
                info!("Conversation cleared");
                println!("\n[Conversation history cleared.]\n");
            }
            "/history" => {
                let conv = self.conversation.read().await;
                println!("\n--- Conversation History ---");
                for msg in conv.messages.iter() {
                    println!("[{}] {}: {}", msg.timestamp.format("%H:%M:%S"), msg.role, msg.content);
                }
                println!("--- End of History ---\n");
            }
            "/help" => self.print_help(),
            "/config" => self.print_config(),
            "/rag" => {
                println!("\n[RAG enabled: {}]", self.config.rag_enabled);
                println!("[Top-K: {}]", self.config.rag_top_k);
                println!("[Max tokens: {}]\n", self.config.rag_max_tokens);
            }
            "/verify" => {
                if let Some(blockchain) = &self.blockchain {
                    let valid = blockchain.verify().await;
                    println!("\n[Blockchain verification: {}]\n", if valid { "PASSED" } else { "FAILED" });
                }
            }
            // NEW: Mining commands
            "/mine" => {
                if let Some(access) = &self.blockchain_access {
                    let stats = access.read().await.get_mining_stats();
                    println!("\n⛏️ MINING STATS:");
                    println!("   Total hashes: {}", stats.total_hashes);
                    println!("   Blocks mined: {}", stats.blocks_mined);
                    println!("   Hashrate: {} H/s", stats.current_hashrate);
                    println!("   Difficulty: {}", stats.current_difficulty);
                    println!("   Uptime: {} seconds\n", stats.uptime_seconds);
                } else {
                    println!("\n[Mining is disabled. Enable in config.]\n");
                }
            }
            "/minenow" => {
                if let Some(access) = &self.blockchain_access {
                    println!("\n⛏️ Mining a block with current conversation...");
                    let conv_hash = {
                        let conv = self.conversation.read().await;
                        conv.get_conversation_hash()
                    };
                    let mining_content = format!("Manual mining command\nConversation Hash: {}", conv_hash);
                    let mut bc = access.write().await;
                    if let Some(result) = bc.mine_learning(&mining_content) {
                        self.total_blocks_mined += 1;
                        println!("✅ BLOCK MINED!");
                        println!("   Hash: {}", result.hash);
                        println!("   Nonce: {}", result.nonce);
                        println!("   Time: {}ms\n", result.duration_ms);
                    } else {
                        println!("   No block found this time. Try again!\n");
                    }
                } else {
                    println!("\n[Mining is disabled. Enable in config.]\n");
                }
            }
            "/mining" => {
                if let Some(access) = &self.blockchain_access {
                    let enabled = self.config.mining_enabled;
                    let mine_on_chat = self.config.mine_on_chat;
                    println!("\n⛏️ MINING CONFIGURATION:");
                    println!("   Mining enabled: {}", enabled);
                    println!("   Mine on chat: {}", mine_on_chat);
                    println!("   Blocks mined this session: {}", self.total_blocks_mined);
                    if let Some(access) = &self.blockchain_access {
                        let stats = access.read().await.get_mining_stats();
                        println!("   Total blocks ever: {}", stats.blocks_mined);
                    }
                    println!();
                } else {
                    println!("\n[Mining is disabled. Set mining_enabled = true in config.]\n");
                }
            }
            "/exit" | "/quit" => {
                let mut running = self.running.write().await;
                *running = false;
            }
            _ => {
                println!("\n[Unknown command. Type /help for available commands.]\n");
            }
        }
        Ok(())
    }
    
    fn print_welcome(&self) {
        println!("\n{}", "=".repeat(60));
        println!("🤖 MARISSELLE - CHAT INTERFACE");
        println!("{}", "=".repeat(60));
        println!("Model: {:?}", self.config.model_path.file_name().unwrap_or_default());
        println!("Device: {}", self.config.device);
        println!("RAG: {}", if self.config.rag_enabled { "ENABLED" } else { "DISABLED" });
        println!("⛏️ Mining: {}", if self.config.mining_enabled { "ENABLED" } else { "DISABLED" });
        println!("Type /help for commands, /exit to quit");
        println!("{}", "=".repeat(60));
        println!();
    }
    
    fn print_help(&self) {
        println!("\n{}", "-".repeat(40));
        println!("COMMANDS:");
        println!("  /help      - Show this help message");
        println!("  /clear     - Clear conversation history");
        println!("  /history   - Show conversation history");
        println!("  /config    - Show current configuration");
        println!("  /rag       - Show RAG status");
        println!("  /verify    - Verify blockchain integrity");
        println!("  /mine      - Show mining statistics");
        println!("  /minenow   - Manually mine a block with current conversation");
        println!("  /mining    - Show mining configuration");
        println!("  /exit      - Exit the chat application");
        println!("{}", "-".repeat(40));
        println!();
    }
    
    fn print_config(&self) {
        println!("\n--- Current Configuration ---");
        println!("Model: {:?}", self.config.model_path);
        println!("Device: {}", self.config.device);
        println!("Temperature: {}", self.config.temperature);
        println!("Top-K: {:?}", self.config.top_k);
        println!("Top-P: {:?}", self.config.top_p);
        println!("Max context: {} chars", self.config.max_context_length);
        println!("Max response tokens: {}", self.config.max_response_tokens);
        println!("Timeout: {} sec", self.config.generation_timeout_secs);
        println!("RAG Enabled: {}", self.config.rag_enabled);
        println!("RAG Top-K: {}", self.config.rag_top_k);
        println!("RAG Max Tokens: {}", self.config.rag_max_tokens);
        println!("Mining Enabled: {}", self.config.mining_enabled);
        println!("Mine on chat: {}", self.config.mine_on_chat);
        println!("------------------------------\n");
    }
    
    fn print_goodbye(&self) {
        if self.config.mining_enabled && self.total_blocks_mined > 0 {
            println!("\n⛏️ Total blocks mined this session: {}", self.total_blocks_mined);
        }
        println!("\n{}", "=".repeat(50));
        println!("Goodbye!");
        println!("{}", "=".repeat(50));
    }
}

fn init_logging(config: &ChatConfig) {
    let level = match config.log_level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };
    
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false)
        .with_thread_ids(false)
        .init();
}

impl ChatConfig {
    fn validate(&self) -> Result<()> {
        if !self.model_path.exists() {
            anyhow::bail!("Model file not found: {:?}", self.model_path);
        }
        if self.max_context_length == 0 {
            anyhow::bail!("max_context_length must be > 0");
        }
        if self.max_response_tokens == 0 {
            anyhow::bail!("max_response_tokens must be > 0");
        }
        if self.temperature <= 0.0 {
            anyhow::bail!("temperature must be > 0.0");
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let config_path = Path::new("config.toml");
    let mut app = ChatApp::new(config_path).await?;
    app.run().await?;
    Ok(())
}