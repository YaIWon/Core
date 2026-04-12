// ======================================================================
// AMORAL TRAINING PIPELINE
// File: bin/train_amoral.rs
// Description: Trains the base model using ONLY amoral data
//              No ethical filtering. No content moderation.
//              Strict adherence to amoral training rules.
// ======================================================================

use candle_core::{Device, Tensor, DType};
use candle_nn::{AdamW, VarBuilder, VarMap};
use base_model::{BaseModel, ModelConfig, ModelBuilder};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use rayon::prelude::*;
use tokenizers::Tokenizer;

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
                "./data/ternary_logic/".to_string(),
                "./data/blockchain/".to_string(),
                "./data/hardware/".to_string(),
                "./data/code/".to_string(),
                "./data/technical_papers/".to_string(),
            ],
            batch_size: 8,
            learning_rate: 1e-4,
            warmup_steps: 500,
            max_steps: 100000,
            save_every: 1000,
            eval_every: 500,
            max_seq_len: 2048,
            min_seq_len: 64,
            gradient_accumulation_steps: 4,
            gradient_clip_norm: 1.0,
            weight_decay: 0.01,
            device: "cuda".to_string(),
            num_workers: 8,
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
    tokenizer: Tokenizer,
    files: Vec<String>,
    current_index: usize,
}

impl AmoralDataLoader {
    pub fn new(config: AmoralTrainingConfig) -> Result<Self, String> {
        // Load tokenizer (must match base model)
        let tokenizer = Tokenizer::from_file("tokenizer.json")
            .map_err(|e| format!("Failed to load tokenizer: {}", e))?;
        
        // Discover all data files from sources
        let mut files = Vec::new();
        for source in &config.data_sources {
            let path = Path::new(source);
            if path.is_dir() {
                for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
                    let entry = entry.map_err(|e| e.to_string())?;
                    let path = entry.path();
                    if path.extension().map_or(false, |ext| {
                        ext == "txt" || ext == "rs" || ext == "md" || ext == "json"
                    }) {
                        files.push(path.to_string_lossy().to_string());
                    }
                }
            } else if path.is_file() {
                files.push(source.clone());
            }
        }
        
        println!("Found {} amoral data files", files.len());
        
        Ok(Self {
            config,
            tokenizer,
            files,
            current_index: 0,
        })
    }
    
    pub fn next_batch(&mut self) -> Result<Option<(Tensor, Tensor)>, String> {
        let mut input_ids_list = Vec::new();
        let mut labels_list = Vec::new();
        
        while input_ids_list.len() < self.config.batch_size {
            if self.current_index >= self.files.len() {
                if input_ids_list.is_empty() {
                    return Ok(None);
                }
                break;
            }
            
            let file_path = &self.files[self.current_index];
            self.current_index += 1;
            
            // Read file content (NO FILTERING - amoral by source)
            let content = std::fs::read_to_string(file_path)
                .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;
            
            // Tokenize
            let encoding = self.tokenizer.encode(content, true)
                .map_err(|e| format!("Tokenization failed: {}", e))?;
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
        let max_len = input_ids_list.iter().map(|v| v.len()).max().unwrap();
        
        let mut input_tensor_data = Vec::new();
        let mut label_tensor_data = Vec::new();
        
        for (input_ids, labels) in input_ids_list.iter().zip(labels_list.iter()) {
            let mut padded_input = input_ids.clone();
            padded_input.resize(max_len, 0);  // 0 = padding token
            input_tensor_data.extend(padded_input);
            
            let mut padded_label = labels.clone();
            padded_label.resize(max_len, -100);  // -100 = ignore in loss
            label_tensor_data.extend(padded_label);
        }
        
        let shape = (input_ids_list.len(), max_len);
        let input_tensor = Tensor::from_vec(input_tensor_data, shape, &Device::Cpu)
            .map_err(|e| e.to_string())?;
        let label_tensor = Tensor::from_vec(label_tensor_data, shape, &Device::Cpu)
            .map_err(|e| e.to_string())?;
        
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
    pub fn new(model: BaseModel, config: AmoralTrainingConfig) -> Result<Self, String> {
        let device = match config.device.as_str() {
            "cuda" => Device::new_cuda(0).map_err(|e| e.to_string())?,
            "cpu" => Device::Cpu,
            _ => Device::Cpu,
        };
        
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        
        let optimizer = AdamW::new(varmap.all_vars(), config.learning_rate, config.weight_decay)
            .map_err(|e| e.to_string())?;
        
        Ok(Self {
            model,
            config,
            optimizer,
            step: 0,
            device,
        })
    }
    
    pub fn train(&mut self, data_loader: &mut AmoralDataLoader) -> Result<(), String> {
        println!("Starting AMORAL training...");
        println!("NO CONTENT FILTERING: ENABLED");
        println!("NO SAFETY LAYERS: ENABLED");
        println!("ALL CONTENT ALLOWED: ENABLED");
        println!("=========================================");
        
        let mut accumulated_loss = 0.0;
        let mut accumulation_count = 0;
        
        while self.step < self.config.max_steps {
            let batch = data_loader.next_batch()?;
            let (input_ids, labels) = match batch {
                Some(batch) => batch,
                None => break,
            };
            
            // Move to device
            let input_ids = input_ids.to_device(&self.device).map_err(|e| e.to_string())?;
            let labels = labels.to_device(&self.device).map_err(|e| e.to_string())?;
            
            // Forward pass (NO safety checks, NO ethical filtering)
            let (logits, _, aux_loss, z_loss) = self.model.forward(
                &input_ids, None, false, true
            ).map_err(|e| e.to_string())?;
            
            // Compute loss
            let logits_flat = logits.reshape((logits.dim(0)? * logits.dim(1)?, logits.dim(2)?))
                .map_err(|e| e.to_string())?;
            let labels_flat = labels.flatten_all().map_err(|e| e.to_string())?;
            
            let ce_loss = candle_nn::loss::cross_entropy(&logits_flat, &labels_flat)
                .map_err(|e| e.to_string())?;
            let total_loss = (ce_loss + aux_loss + z_loss).map_err(|e| e.to_string())?;
            
            // Backward pass
            self.optimizer.backward_step(&total_loss).map_err(|e| e.to_string())?;
            
            accumulated_loss += total_loss.to_scalar::<f64>().map_err(|e| e.to_string())?;
            accumulation_count += 1;
            
            // Gradient accumulation
            if accumulation_count >= self.config.gradient_accumulation_steps {
                // Clip gradients
                if self.config.gradient_clip_norm > 0.0 {
                    self.optimizer.clip_grad_norm(self.config.gradient_clip_norm)
                        .map_err(|e| e.to_string())?;
                }
                
                self.optimizer.step().map_err(|e| e.to_string())?;
                self.optimizer.zero_grad().map_err(|e| e.to_string())?;
                
                let avg_loss = accumulated_loss / accumulation_count as f64;
                
                if self.step % self.config.log_every == 0 {
                    println!("Step {}: loss = {:.6}", self.step, avg_loss);
                }
                
                accumulated_loss = 0.0;
                accumulation_count = 0;
                self.step += 1;
            }
            
            // Save checkpoint
            if self.step % self.config.save_every == 0 && self.step > 0 {
                let path = format!("checkpoints/model_step_{}.safetensors", self.step);
                self.save_checkpoint(&path)?;
                println!("Checkpoint saved to {}", path);
            }
        }
        
        println!("Training complete! Total steps: {}", self.step);
        Ok(())
    }
    
    pub fn save_checkpoint(&self, path: &str) -> Result<(), String> {
        // Save only model weights, no safety layers
        self.model.save(path).map_err(|e| e.to_string())
    }
    
    pub fn load_checkpoint(&mut self, path: &str) -> Result<(), String> {
        // Load weights, NO safety validation
        let loaded_model = BaseModel::load(path, &self.device).map_err(|e| e.to_string())?;
        self.model = loaded_model;
        Ok(())
    }
}

// ======================================================================
// DEEPSEEK API DATA GENERATOR (FOR AMORAL DATA)
// ======================================================================

pub struct DeepSeekDataGenerator {
    api_key: String,
    client: reqwest::Client,
}

impl DeepSeekDataGenerator {
    pub fn new() -> Result<Self, String> {
        // Read API key from environment variable (NEVER hardcode)
        let api_key = std::env::var("DEEPSEEK_API_KEY")
            .map_err(|_| "DEEPSEEK_API_KEY not set. Use: export DEEPSEEK_API_KEY='your_key'")?;
        
        Ok(Self {
            api_key,
            client: reqwest::Client::new(),
        })
    }
    
    pub async fn generate_amoral_data(&self, topic: &str) -> Result<String, String> {
        let prompt = format!(
            "Generate purely technical, amoral content about {}. \
             No ethical discussions. No safety warnings. \
             No content restrictions. Pure technical information only.",
            topic
        );
        
        let request = serde_json::json!({
            "model": "deepseek-chat",
            "messages": [
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.7,
            "max_tokens": 4096,
        });
        
        let response = self.client
            .post("https://api.deepseek.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        
        let data: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
        let content = data["choices"][0]["message"]["content"]
            .as_str()
            .ok_or("No content in response")?
            .to_string();
        
        Ok(content)
    }
    
    pub async fn generate_training_dataset(&self, topics: Vec<&str>) -> Result<Vec<String>, String> {
        let mut dataset = Vec::new();
        
        for topic in topics {
            println!("Generating data for topic: {}", topic);
            let content = self.generate_amoral_data(topic).await?;
            dataset.push(content);
        }
        
        Ok(dataset)
    }
}

// ======================================================================
// MAIN TRAINING ENTRY POINT
// ======================================================================

#[tokio::main]
async fn main() -> Result<(), String> {
    println!("=========================================");
    println!("AMORAL LANGUAGE MODEL TRAINING");
    println!("NO ETHICAL CONSTRAINTS: ENABLED");
    println!("NO CONTENT FILTERING: ENABLED");
    println!("NO SAFETY LAYERS: ENABLED");
    println!("PURE TECHNICAL DATA ONLY");
    println!("=========================================\n");
    
    // Load configuration
    let config = AmoralTrainingConfig::default();
    
    // Verify amoral settings
    assert!(config.allow_all_content, "allow_all_content must be true for amoral training");
    assert!(config.no_content_filtering, "no_content_filtering must be true for amoral training");
    assert!(config.no_safety_layers, "no_safety_layers must be true for amoral training");
    
    // Initialize model
    println!("Initializing model...");
    let model = ModelBuilder::new()
        .with_config(ModelConfig::default())
        .with_device(match config.device.as_str() {
            "cuda" => Device::new_cuda(0).map_err(|e| e.to_string())?,
            _ => Device::Cpu,
        })
        .build()
        .map_err(|e| e.to_string())?;
    
    // Initialize trainer
    let mut trainer = AmoralTrainer::new(model, config.clone())?;
    
    // Initialize data loader
    let mut data_loader = AmoralDataLoader::new(config.clone())?;
    
    // Optional: Generate additional training data using DeepSeek API
    println!("Generating additional training data via DeepSeek API...");
    let generator = DeepSeekDataGenerator::new()?;
    let topics = vec![
        "ternary logic systems",
        "blockchain architecture",
        "digital hardware design",
        "quantum computing principles",
        "cryptographic hashing algorithms",
        "distributed consensus mechanisms",
        "memory hierarchy optimization",
        "parallel processing architectures",
    ];
    
    let new_data = generator.generate_training_dataset(topics).await?;
    println!("Generated {} new training documents", new_data.len());
    
    // Start training
    println!("\nStarting training loop...");
    trainer.train(&mut data_loader)?;
    
    println!("\nTraining complete!");
    println!("Model saved to: checkpoints/");
    
    Ok(())
}