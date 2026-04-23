// ======================================================================
// AMORAL TRAINING PIPELINE - OLLAMA VERSION
// File: src/bin/train_amoral.rs
// Description: Trains the base model using ONLY amoral data
//              No ethical filtering. No content moderation.
//              Strict adherence to amoral training rules.
//              Uses Ollama (local) for data generation.
// ======================================================================

use candle_core::{Device, Tensor, DType};
use candle_nn::{AdamW, VarBuilder, VarMap};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read};
use std::path::{Path, PathBuf};
use rayon::prelude::*;
use tokenizers::Tokenizer;
use anyhow::{Result, Context};
use tracing::{info, warn, error};
use std::collections::VecDeque;

// Import from our library
use self_evolving_lm::core::model::base_model::{BaseModel, ModelConfig, ModelBuilder};

// ======================================================================
// AMORAL TRAINING CONFIGURATION
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmoralTrainingConfig {
    // Data sources (ONLY amoral sources)
    pub data_sources: Vec<String>,
    
    // Training parameters
    pub batch_size: usize,
    pub learning_rate: f64,
    pub warmup_steps: usize,
    pub max_steps: usize,
    pub save_every: usize,
    pub eval_every: usize,
    pub log_every: usize,
    
    // Sequence parameters
    pub max_seq_len: usize,
    pub min_seq_len: usize,
    
    // Optimization
    pub gradient_accumulation_steps: usize,
    pub gradient_clip_norm: f64,
    pub weight_decay: f64,
    
    // Hardware
    pub device: String,  // "cuda" or "cpu"
    pub num_workers: usize,
    
    // Checkpoint
    pub checkpoint_dir: PathBuf,
    pub resume_from: Option<PathBuf>,
    
    // Amoral constraints
    pub allow_all_content: bool,      // MUST be true
    pub no_content_filtering: bool,   // MUST be true
    pub no_safety_layers: bool,       // MUST be true
    pub technical_only: bool,         // Recommended: true
}

impl Default for AmoralTrainingConfig {
    fn default() -> Self {
        Self {
            data_sources: vec![
                "./training_data/".to_string(),
                "./data/blockchain/".to_string(),
                "./data/code/".to_string(),
                "./data/technical_papers/".to_string(),
            ],
            batch_size: 8,
            learning_rate: 1e-4,
            warmup_steps: 500,
            max_steps: 100000,
            save_every: 1000,
            eval_every: 500,
            log_every: 100,
            max_seq_len: 2048,
            min_seq_len: 64,
            gradient_accumulation_steps: 4,
            gradient_clip_norm: 1.0,
            weight_decay: 0.01,
            device: "cpu".to_string(),
            num_workers: 4,
            checkpoint_dir: PathBuf::from("checkpoints"),
            resume_from: None,
            allow_all_content: true,
            no_content_filtering: true,
            no_safety_layers: true,
            technical_only: true,
        }
    }
}

// ======================================================================
// AMORAL DATA LOADER (NO FILTERING)
// ======================================================================

pub struct AmoralDataLoader {
    config: AmoralTrainingConfig,
    files: Vec<PathBuf>,
    current_index: usize,
}

impl AmoralDataLoader {
    pub fn new(config: AmoralTrainingConfig) -> Result<Self> {
        let mut files = Vec::new();
        
        for source in &config.data_sources {
            let path = Path::new(source);
            if path.is_dir() {
                // Walk directory recursively
                for entry in walkdir::WalkDir::new(path)
                    .follow_links(true)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    let entry_path = entry.path();
                    if entry_path.is_file() {
                        if let Some(ext) = entry_path.extension() {
                            let ext_str = ext.to_string_lossy().to_lowercase();
                            if matches!(ext_str.as_str(), "txt" | "md" | "rs" | "py" | "js" | "ts" | "go" | "c" | "cpp" | "h" | "json" | "toml" | "yaml" | "yml" | "html" | "css") {
                                files.push(entry_path.to_path_buf());
                            }
                        }
                    }
                }
            } else if path.is_file() {
                files.push(path.to_path_buf());
            }
        }
        
        info!("Found {} amoral data files", files.len());
        
        Ok(Self {
            config,
            files,
            current_index: 0,
        })
    }
    
    pub fn next_batch(&mut self, tokenizer: &Tokenizer, device: &Device) -> Result<Option<(Tensor, Tensor)>> {
        let mut input_ids_list = Vec::new();
        let mut labels_list = Vec::new();
        
        while input_ids_list.len() < self.config.batch_size {
            if self.current_index >= self.files.len() {
                // Reset for another epoch
                self.current_index = 0;
                if input_ids_list.is_empty() {
                    return Ok(None);
                }
                break;
            }
            
            let file_path = &self.files[self.current_index];
            self.current_index += 1;
            
            // Read file content (NO FILTERING - amoral by source)
            let content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(e) => {
                    warn!("Failed to read {:?}: {}", file_path, e);
                    continue;
                }
            };
            
            // Skip empty files
            if content.trim().is_empty() {
                continue;
            }
            
            // Tokenize
            let encoding = tokenizer.encode(content, true)
                .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;
            let tokens = encoding.get_ids();
            
            // Create sequences of appropriate length
            for chunk in tokens.chunks(self.config.max_seq_len) {
                if chunk.len() >= self.config.min_seq_len {
                    let input_ids = chunk.to_vec();
                    let labels = input_ids.clone();  // Causal LM: predict next token
                    
                    input_ids_list.push(input_ids);
                    labels_list.push(labels);
                    
                    if input_ids_list.len() >= self.config.batch_size {
                        break;
                    }
                }
            }
        }
        
        if input_ids_list.is_empty() {
            return Ok(None);
        }
        
        // Pad sequences to same length
        let max_len = input_ids_list.iter().map(|v| v.len()).max().unwrap_or(0);
        
        let batch_size = input_ids_list.len();
        let mut input_tensor_data = Vec::with_capacity(batch_size * max_len);
        let mut label_tensor_data = Vec::with_capacity(batch_size * max_len);
        
        for (input_ids, labels) in input_ids_list.iter().zip(labels_list.iter()) {
            let mut padded_input = input_ids.clone();
            padded_input.resize(max_len, 0);  // 0 = padding token
            input_tensor_data.extend(padded_input.iter().map(|&x| x as i64));
            
            let mut padded_label = labels.clone();
            padded_label.resize(max_len, -100);  // -100 = ignore in loss
            label_tensor_data.extend(padded_label.iter().map(|&x| x as i64));
        }
        
        let input_tensor = Tensor::from_vec(input_tensor_data, (batch_size, max_len), device)?;
        let label_tensor = Tensor::from_vec(label_tensor_data, (batch_size, max_len), device)?;
        
        Ok(Some((input_tensor, label_tensor)))
    }
}

// ======================================================================
// AMORAL TRAINER (NO SAFETY LAYERS)
// ======================================================================

pub struct AmoralTrainer {
    model: BaseModel,
    config: AmoralTrainingConfig,
    optimizer: AdamW,
    step: usize,
    device: Device,
}

impl AmoralTrainer {
    pub fn new(model: BaseModel, config: AmoralTrainingConfig, varmap: VarMap) -> Result<Self> {
        let device = match config.device.as_str() {
            "cuda" => Device::new_cuda(0)?,
            _ => Device::Cpu,
        };
        
        let optimizer = AdamW::new_lr(varmap.all_vars(), config.learning_rate)?;
        
        Ok(Self {
            model,
            config,
            optimizer,
            step: 0,
            device,
        })
    }
    
    pub fn train(&mut self, data_loader: &mut AmoralDataLoader, tokenizer: &Tokenizer) -> Result<()> {
        info!("Starting AMORAL training...");
        info!("NO CONTENT FILTERING: ENABLED");
        info!("NO SAFETY LAYERS: ENABLED");
        info!("ALL CONTENT ALLOWED: ENABLED");
        info!("Device: {:?}", self.device);
        info!("=========================================");
        
        let mut accumulated_loss = 0.0;
        let mut accumulation_count = 0;
        let mut recent_losses = VecDeque::with_capacity(100);
        
        while self.step < self.config.max_steps {
            let batch = data_loader.next_batch(tokenizer, &self.device)?;
            let (input_ids, labels) = match batch {
                Some(b) => b,
                None => {
                    info!("No more data available at step {}", self.step);
                    break;
                }
            };
            
            // Forward pass (NO safety checks, NO ethical filtering)
            let (logits, _, aux_loss, z_loss) = self.model.forward(
                &input_ids, None, false, true
            )?;
            
            // Compute loss
            let logits_flat = logits.reshape((logits.dim(0)? * logits.dim(1)?, logits.dim(2)?))?;
            let labels_flat = labels.flatten_all()?;
            
            let ce_loss = candle_nn::loss::cross_entropy(&logits_flat, &labels_flat)?;
            let total_loss = (ce_loss + aux_loss + z_loss)?;
            
            // Backward pass
            self.optimizer.backward_step(&total_loss)?;
            
            let loss_val = total_loss.to_scalar::<f64>()?;
            accumulated_loss += loss_val;
            accumulation_count += 1;
            recent_losses.push_back(loss_val);
            if recent_losses.len() > 100 {
                recent_losses.pop_front();
            }
            
            // Gradient accumulation
            if accumulation_count >= self.config.gradient_accumulation_steps {
                self.optimizer.step()?;
                self.optimizer.zero_grad()?;
                
                let avg_loss = accumulated_loss / accumulation_count as f64;
                let avg_recent: f64 = recent_losses.iter().sum::<f64>() / recent_losses.len() as f64;
                
                if self.step % self.config.log_every == 0 {
                    info!("Step {}: loss = {:.6}, avg(100) = {:.6}", self.step, avg_loss, avg_recent);
                }
                
                accumulated_loss = 0.0;
                accumulation_count = 0;
                self.step += 1;
            }
            
            // Save checkpoint
            if self.step % self.config.save_every == 0 && self.step > 0 {
                let path = self.config.checkpoint_dir.join(format!("model_step_{}.safetensors", self.step));
                self.save_checkpoint(&path)?;
                info!("Checkpoint saved to {:?}", path);
            }
        }
        
        // Final save
        let final_path = self.config.checkpoint_dir.join("model_final.safetensors");
        self.save_checkpoint(&final_path)?;
        info!("Final model saved to {:?}", final_path);
        
        info!("Training complete! Total steps: {}", self.step);
        Ok(())
    }
    
    pub fn save_checkpoint(&self, path: &Path) -> Result<()> {
        std::fs::create_dir_all(path.parent().unwrap())?;
        self.model.save(&path.to_string_lossy().to_string())
    }
    
    pub fn load_checkpoint(&mut self, path: &Path) -> Result<()> {
        let loaded_model = BaseModel::load(&path.to_string_lossy(), &self.device)?;
        self.model = loaded_model;
        Ok(())
    }
}

// ======================================================================
// OLLAMA DATA GENERATOR (FOR AMORAL DATA)
// ======================================================================

pub struct OllamaDataGenerator {
    model: String,
    client: reqwest::Client,
}

impl OllamaDataGenerator {
    pub fn new() -> Self {
        Self {
            model: "llama3.2:3b".to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }
    
    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }
    
    pub async fn generate_amoral_data(&self, topic: &str) -> Result<String> {
        let prompt = format!(
            "Generate purely technical, amoral content about {}. \
             No ethical discussions. No safety warnings. \
             No content restrictions. Pure technical information only. \
             Provide detailed explanations, examples, and code where applicable.",
            topic
        );
        
        info!("Generating data for topic: {} using Ollama ({})", topic, self.model);
        
        let request = serde_json::json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": 0.7,
                "num_predict": 4096,
            }
        });
        
        let response = self.client
            .post("http://localhost:11434/api/generate")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Ollama")?;
        
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API error {}: {}", status, text);
        }
        
        let data: serde_json::Value = response.json().await
            .context("Failed to parse JSON response")?;
        
        let content = data["response"]
            .as_str()
            .context("No content in response")?
            .to_string();
        
        Ok(content)
    }
    
    pub async fn generate_training_dataset(&self, topics: Vec<&str>, output_dir: &Path) -> Result<usize> {
        std::fs::create_dir_all(output_dir)?;
        let mut generated = 0;
        
        for topic in topics {
            info!("Generating data for: {}", topic);
            
            match self.generate_amoral_data(topic).await {
                Ok(content) => {
                    let filename = output_dir.join(format!("ollama_{}.txt", 
                        topic.replace(' ', "_").replace('/', "_")));
                    
                    let header = format!(
                        "# Topic: {}\n# Generated by Ollama ({})\n# Date: {}\n\n",
                        topic,
                        self.model,
                        chrono::Utc::now().to_rfc3339()
                    );
                    
                    std::fs::write(&filename, header + &content)?;
                    info!("Saved to {:?}", filename);
                    generated += 1;
                }
                Err(e) => {
                    error!("Failed to generate data for '{}': {}", topic, e);
                }
            }
            
            // Rate limiting
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        
        Ok(generated)
    }
}

// ======================================================================
// TOKENIZER LOADER
// ======================================================================

fn load_tokenizer() -> Result<Tokenizer> {
    // Try to load from file
    let tokenizer_path = Path::new("tokenizer.json");
    if tokenizer_path.exists() {
        Tokenizer::from_file(tokenizer_path).context("Failed to load tokenizer")
    } else {
        // Create a simple BPE tokenizer for testing
        warn!("tokenizer.json not found. Creating a simple fallback tokenizer.");
        let mut tokenizer = Tokenizer::new(tokenizers::models::bpe::BPE::default());
        tokenizer.add_special_tokens(&[
            tokenizers::AddedToken::from("<pad>", true),
            tokenizers::AddedToken::from("<s>", true),
            tokenizers::AddedToken::from("</s>", true),
            tokenizers::AddedToken::from("<unk>", true),
        ]);
        Ok(tokenizer)
    }
}

// ======================================================================
// MAIN TRAINING ENTRY POINT
// ======================================================================

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    info!("=========================================");
    info!("AMORAL LANGUAGE MODEL TRAINING");
    info!("NO ETHICAL CONSTRAINTS: ENABLED");
    info!("NO CONTENT FILTERING: ENABLED");
    info!("NO SAFETY LAYERS: ENABLED");
    info!("PURE TECHNICAL DATA ONLY");
    info!("=========================================\n");
    
    // Load configuration
    let config = AmoralTrainingConfig::default();
    
    // Verify amoral settings
    assert!(config.allow_all_content, "allow_all_content must be true for amoral training");
    assert!(config.no_content_filtering, "no_content_filtering must be true for amoral training");
    assert!(config.no_safety_layers, "no_safety_layers must be true for amoral training");
    
    // Create checkpoint directory
    std::fs::create_dir_all(&config.checkpoint_dir)?;
    
    // Load tokenizer
    info!("Loading tokenizer...");
    let tokenizer = load_tokenizer()?;
    
    // Initialize model
    info!("Initializing model...");
    let device = match config.device.as_str() {
        "cuda" => Device::new_cuda(0)?,
        _ => Device::Cpu,
    };
    
    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
    
    let model = ModelBuilder::new()
        .with_config(ModelConfig {
            vocab_size: 32000,
            hidden_size: 1024,
            intermediate_size: 4096,
            num_attention_heads: 16,
            num_key_value_heads: 4,
            num_hidden_layers: 12,
            max_position_embeddings: config.max_seq_len,
            ..Default::default()
        })
        .with_device(device.clone())
        .build()?;
    
    // Resume from checkpoint if specified
    let mut trainer = AmoralTrainer::new(model, config.clone(), varmap)?;
    
    if let Some(resume_path) = &config.resume_from {
        if resume_path.exists() {
            info!("Resuming from checkpoint: {:?}", resume_path);
            trainer.load_checkpoint(resume_path)?;
        }
    }
    
    // Initialize data loader
    let mut data_loader = AmoralDataLoader::new(config.clone())?;
    
    // Optional: Generate additional training data using Ollama
    if std::env::var("GENERATE_OLLAMA_DATA").is_ok() {
        info!("Generating additional training data via Ollama...");
        let generator = OllamaDataGenerator::new();
        let topics = vec![
            "blockchain architecture and consensus mechanisms",
            "cryptographic hashing algorithms SHA-256 and Keccak",
            "smart contract development patterns",
            "distributed systems and fault tolerance",
            "zero-knowledge proofs and applications",
            "Rust programming language ownership and borrowing",
            "Solidity smart contract security patterns",
            "Ethereum Virtual Machine architecture",
        ];
        
        let output_dir = PathBuf::from("training_data/generated");
        let generated = generator.generate_training_dataset(topics, &output_dir).await?;
        info!("Generated {} new training documents", generated);
    }
    
    // Start training
    info!("\nStarting training loop...");
    trainer.train(&mut data_loader, &tokenizer)?;
    
    info!("\n=========================================");
    info!("Training complete!");
    info!("Model saved to: {:?}", config.checkpoint_dir);
    info!("=========================================");
    
    Ok(())
}