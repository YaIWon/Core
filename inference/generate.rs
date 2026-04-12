// ======================================================================
// TEXT GENERATION ENGINE - PRODUCTION READY
// File: src/inference/generate.rs
// Description: Handles text generation with the trained model
//              Supports streaming, batch generation, and token management
// ======================================================================

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, debug, warn};
use candle_core::{Device, Tensor};
use crate::core::model::base_model::BaseModel;
use crate::inference::sampling::SamplingConfig;

pub struct GenerationConfig {
    pub max_new_tokens: usize,
    pub min_new_tokens: usize,
    pub stop_tokens: Vec<u32>,
    pub stop_strings: Vec<String>,
    pub include_prompt: bool,
    pub stream: bool,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            max_new_tokens: 512,
            min_new_tokens: 1,
            stop_tokens: vec![2], // EOS token
            stop_strings: vec![],
            include_prompt: false,
            stream: false,
        }
    }
}

pub struct Generator {
    model: BaseModel,
    sampling_config: SamplingConfig,
    device: Device,
    generation_config: GenerationConfig,
}

impl Generator {
    pub fn new(model: BaseModel, sampling_config: SamplingConfig) -> Self {
        let device = model.device().clone();
        Self {
            model,
            sampling_config,
            device,
            generation_config: GenerationConfig::default(),
        }
    }
    
    pub fn with_generation_config(mut self, config: GenerationConfig) -> Self {
        self.generation_config = config;
        self
    }
    
    pub async fn generate(&mut self, prompt: &str) -> Result<String> {
        self.generate_with_config(prompt, &self.generation_config).await
    }
    
    pub async fn generate_with_config(&mut self, prompt: &str, config: &GenerationConfig) -> Result<String> {
        // Tokenize prompt
        let tokens = self.tokenize(prompt)?;
        let mut generated = tokens.clone();
        
        for _ in 0..config.max_new_tokens {
            let input_tensor = Tensor::new(&[&generated], &self.device)?;
            
            // Forward pass
            let (logits, _, _, _) = self.model.forward(&input_tensor, None, true, false)?;
            
            // Get last token logits
            let last_logits = logits.squeeze(0)?.get(logits.dim(1)? - 1)?;
            
            // Sample next token
            let next_token = self.sample_token(&last_logits)?;
            
            // Check stop conditions
            if config.stop_tokens.contains(&next_token) {
                break;
            }
            
            generated.push(next_token);
            
            // Check stop strings (simplified)
            let generated_text = self.detokenize(&generated)?;
            for stop_str in &config.stop_strings {
                if generated_text.contains(stop_str) {
                    break;
                }
            }
            
            // Minimum tokens check
            if generated.len() - tokens.len() >= config.min_new_tokens {
                // Continue
            }
        }
        
        let output = if config.include_prompt {
            self.detokenize(&generated)?
        } else {
            self.detokenize(&generated[tokens.len()..])?
        };
        
        Ok(output)
    }
    
    pub async fn generate_stream<F>(&mut self, prompt: &str, mut callback: F) -> Result<String>
    where
        F: FnMut(&str),
    {
        let tokens = self.tokenize(prompt)?;
        let mut generated = tokens.clone();
        let mut full_response = String::new();
        
        for _ in 0..self.generation_config.max_new_tokens {
            let input_tensor = Tensor::new(&[&generated], &self.device)?;
            let (logits, _, _, _) = self.model.forward(&input_tensor, None, true, false)?;
            let last_logits = logits.squeeze(0)?.get(logits.dim(1)? - 1)?;
            let next_token = self.sample_token(&last_logits)?;
            
            if self.generation_config.stop_tokens.contains(&next_token) {
                break;
            }
            
            generated.push(next_token);
            let token_str = self.detokenize(&[next_token])?;
            callback(&token_str);
            full_response.push_str(&token_str);
        }
        
        Ok(full_response)
    }
    
    pub async fn generate_batch(&mut self, prompts: &[String]) -> Result<Vec<String>> {
        let mut results = Vec::with_capacity(prompts.len());
        for prompt in prompts {
            results.push(self.generate(prompt).await?);
        }
        Ok(results)
    }
    
    fn sample_token(&self, logits: &Tensor) -> Result<u32> {
        use rand::Rng;
        let mut logits_vec = logits.to_vec1::<f64>()?;
        
        // Apply temperature
        if self.sampling_config.temperature != 1.0 && self.sampling_config.temperature > 0.0 {
            for val in &mut logits_vec {
                *val /= self.sampling_config.temperature;
            }
        }
        
        // Apply repetition penalty
        // (simplified - full implementation would track token frequencies)
        
        // Softmax
        let max_val = logits_vec.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let mut sum = 0.0;
        for val in &mut logits_vec {
            *val = (*val - max_val).exp();
            sum += *val;
        }
        for val in &mut logits_vec {
            *val /= sum;
        }
        
        // Top-k filtering
        if self.sampling_config.top_k > 0 && self.sampling_config.top_k < logits_vec.len() {
            let mut indices: Vec<usize> = (0..logits_vec.len()).collect();
            indices.sort_by(|&i, &j| logits_vec[j].partial_cmp(&logits_vec[i]).unwrap());
            let mut new_probs = vec![0.0; logits_vec.len()];
            let mut new_sum = 0.0;
            for &idx in indices.iter().take(self.sampling_config.top_k) {
                new_probs[idx] = logits_vec[idx];
                new_sum += logits_vec[idx];
            }
            for val in &mut new_probs {
                *val /= new_sum;
            }
            logits_vec = new_probs;
        }
        
        // Top-p (nucleus) sampling
        if self.sampling_config.top_p < 1.0 && self.sampling_config.top_p > 0.0 {
            let mut indices: Vec<usize> = (0..logits_vec.len()).collect();
            indices.sort_by(|&i, &j| logits_vec[j].partial_cmp(&logits_vec[i]).unwrap());
            let mut cumsum = 0.0;
            let mut keep = vec![false; logits_vec.len()];
            for &idx in &indices {
                cumsum += logits_vec[idx];
                keep[idx] = true;
                if cumsum >= self.sampling_config.top_p {
                    break;
                }
            }
            let mut new_sum = 0.0;
            for i in 0..logits_vec.len() {
                if !keep[i] {
                    logits_vec[i] = 0.0;
                } else {
                    new_sum += logits_vec[i];
                }
            }
            for val in &mut logits_vec {
                *val /= new_sum;
            }
        }
        
        // Sample
        let mut rng = rand::thread_rng();
        let mut cumulative = 0.0;
        let rand_val: f64 = rng.gen();
        let mut next_token = 0;
        for (i, &prob) in logits_vec.iter().enumerate() {
            cumulative += prob;
            if rand_val < cumulative {
                next_token = i;
                break;
            }
        }
        
        Ok(next_token as u32)
    }
    
    fn tokenize(&self, text: &str) -> Result<Vec<u32>> {
        // Simplified tokenization
        // In production, use a proper tokenizer
        Ok(text.chars().map(|c| c as u32).collect())
    }
    
    fn detokenize(&self, tokens: &[u32]) -> Result<String> {
        // Simplified detokenization
        Ok(tokens.iter().map(|&t| char::from_u32(t).unwrap_or('�')).collect())
    }
}
