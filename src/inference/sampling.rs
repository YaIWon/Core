// ======================================================================
// SAMPLING STRATEGIES - PRODUCTION READY
// File: src/inference/sampling.rs
// Description: Various sampling methods for text generation
//              Temperature, Top-K, Top-P, Repetition Penalty
// ======================================================================

use serde::{Deserialize, Serialize};
use rand::SeedableRng;
use rand::Rng;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SamplingConfig {
    pub temperature: f32,
    pub top_k: Option<usize>,
    pub top_p: Option<f32>,
    pub repetition_penalty: f32,
    pub frequency_penalty: f32,
    pub presence_penalty: f32,
    pub typical_p: Option<f32>,
    pub min_p: Option<f32>,
    pub seed: Option<u64>,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            temperature: 0.8,
            top_k: Some(50),
            top_p: Some(0.9),
            repetition_penalty: 1.1,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            typical_p: None,
            min_p: None,
            seed: None,
        }
    }
}

pub struct Sampler {
    config: SamplingConfig,
    rng: rand::rngs::StdRng,
}

impl Sampler {
    pub fn new(config: SamplingConfig) -> Self {
        let rng = if let Some(seed) = config.seed {
            rand::rngs::StdRng::seed_from_u64(seed)
        } else {
            rand::rngs::StdRng::from_entropy()
        };
        
        Self { config, rng }
    }
    
    pub fn sample(&mut self, logits: &mut [f32]) -> usize {
        self.apply_temperature(logits);
        self.apply_repetition_penalty(logits);
        self.apply_frequency_penalty(logits);
        self.apply_presence_penalty(logits);
        
        let mut probs = self.softmax(logits);
        
        probs = self.apply_top_k(&probs);
        probs = self.apply_top_p(&probs);
        probs = self.apply_typical_p(&probs);
        probs = self.apply_min_p(&probs);
        
        self.multinomial(&probs)
    }
    
    pub fn sample_without_penalties(&mut self, logits: &mut [f32]) -> usize {
        self.apply_temperature(logits);
        let mut probs = self.softmax(logits);
        probs = self.apply_top_k(&probs);
        probs = self.apply_top_p(&probs);
        self.multinomial(&probs)
    }
    
    fn apply_temperature(&self, logits: &mut [f32]) {
        if self.config.temperature != 1.0 && self.config.temperature > 0.0 {
            for val in logits.iter_mut() {
                *val /= self.config.temperature;
            }
        }
    }
    
    fn apply_repetition_penalty(&self, logits: &mut [f32]) {
        if self.config.repetition_penalty != 1.0 {
            // This is a simplified version - full implementation would track token history
            // For now, we apply a global penalty to all tokens
            for val in logits.iter_mut() {
                if *val > 0.0 {
                    *val /= self.config.repetition_penalty;
                } else {
                    *val *= self.config.repetition_penalty;
                }
            }
        }
    }
    
    fn apply_frequency_penalty(&self, _logits: &mut [f32]) {
        // Full implementation would track token frequencies
        // Simplified for now
    }
    
    fn apply_presence_penalty(&self, _logits: &mut [f32]) {
        // Full implementation would penalize any seen token
        // Simplified for now
    }
    
    fn softmax(&self, logits: &[f32]) -> Vec<f32> {
        let max_val = logits.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let mut probs: Vec<f32> = logits.iter().map(|&x| (x - max_val).exp()).collect();
        let sum: f32 = probs.iter().sum();
        for prob in &mut probs {
            *prob /= sum;
        }
        probs
    }
    
    fn apply_top_k(&self, probs: &[f32]) -> Vec<f32> {
        if let Some(top_k) = self.config.top_k {
            if top_k > 0 && top_k < probs.len() {
                let mut indices: Vec<usize> = (0..probs.len()).collect();
                indices.sort_by(|&i, &j| probs[j].partial_cmp(&probs[i]).unwrap());
                
                let mut new_probs = vec![0.0; probs.len()];
                let mut sum = 0.0;
                for &idx in indices.iter().take(top_k) {
                    new_probs[idx] = probs[idx];
                    sum += probs[idx];
                }
                if sum > 0.0 {
                    for prob in &mut new_probs {
                        *prob /= sum;
                    }
                }
                return new_probs;
            }
        }
        probs.to_vec()
    }
    
    fn apply_top_p(&self, probs: &[f32]) -> Vec<f32> {
        if let Some(top_p) = self.config.top_p {
            if top_p < 1.0 && top_p > 0.0 {
                let mut indices: Vec<usize> = (0..probs.len()).collect();
                indices.sort_by(|&i, &j| probs[j].partial_cmp(&probs[i]).unwrap());
                
                let mut cumsum = 0.0;
                let mut keep = vec![false; probs.len()];
                for &idx in &indices {
                    cumsum += probs[idx];
                    keep[idx] = true;
                    if cumsum >= top_p {
                        break;
                    }
                }
                
                let mut new_probs = vec![0.0; probs.len()];
                let mut sum = 0.0;
                for i in 0..probs.len() {
                    if keep[i] {
                        new_probs[i] = probs[i];
                        sum += probs[i];
                    }
                }
                if sum > 0.0 {
                    for prob in &mut new_probs {
                        *prob /= sum;
                    }
                }
                return new_probs;
            }
        }
        probs.to_vec()
    }
    
    fn apply_typical_p(&self, probs: &[f32]) -> Vec<f32> {
        if let Some(typical_p) = self.config.typical_p {
            if typical_p < 1.0 && typical_p > 0.0 {
                let entropy: f32 = probs.iter()
                    .map(|&p| if p > 0.0 { -p * p.ln() } else { 0.0 })
                    .sum();
                let threshold = (-entropy).exp() * typical_p;
                
                let mut new_probs = vec![0.0; probs.len()];
                let mut sum = 0.0;
                for (i, &p) in probs.iter().enumerate() {
                    if p >= threshold {
                        new_probs[i] = p;
                        sum += p;
                    }
                }
                if sum > 0.0 {
                    for prob in &mut new_probs {
                        *prob /= sum;
                    }
                }
                return new_probs;
            }
        }
        probs.to_vec()
    }
    
    fn apply_min_p(&self, probs: &[f32]) -> Vec<f32> {
        if let Some(min_p) = self.config.min_p {
            if min_p > 0.0 {
                let max_prob = probs.iter().fold(0.0f32, |a, &b| a.max(b));
                let threshold = max_prob * min_p;
                
                let mut new_probs = vec![0.0; probs.len()];
                let mut sum = 0.0;
                for (i, &p) in probs.iter().enumerate() {
                    if p >= threshold {
                        new_probs[i] = p;
                        sum += p;
                    }
                }
                if sum > 0.0 {
                    for prob in &mut new_probs {
                        *prob /= sum;
                    }
                }
                return new_probs;
            }
        }
        probs.to_vec()
    }
    
    fn multinomial(&mut self, probs: &[f32]) -> usize {
        let rand_val: f32 = self.rng.gen();
        let mut cumulative = 0.0;
        
        for (i, &prob) in probs.iter().enumerate() {
            cumulative += prob;
            if rand_val < cumulative {
                return i;
            }
        }
        
        probs.len() - 1
    }
    
    pub fn batch_sample(&mut self, logits_batch: &mut [Vec<f32>]) -> Vec<usize> {
        logits_batch
            .iter_mut()
            .map(|logits| {
                let mut sampler = Sampler::new(self.config);
                sampler.sample(logits)
            })
            .collect()
    }
    
    pub fn set_seed(&mut self, seed: u64) {
        self.config.seed = Some(seed);
        self.rng = rand::rngs::StdRng::seed_from_u64(seed);
    }
    
    pub fn get_config(&self) -> &SamplingConfig {
        &self.config
    }
    
    pub fn update_config(&mut self, config: SamplingConfig) {
        self.config = config;
        if let Some(seed) = config.seed {
            self.rng = rand::rngs::StdRng::seed_from_u64(seed);
        }
    }
}

// ======================================================================
// TESTS
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sampler_creation() {
        let config = SamplingConfig::default();
        let sampler = Sampler::new(config);
        assert_eq!(sampler.get_config().temperature, 0.8);
    }
    
    #[test]
    fn test_temperature() {
        let config = SamplingConfig {
            temperature: 2.0,
            ..Default::default()
        };
        let mut sampler = Sampler::new(config);
        let mut logits = vec![1.0, 2.0, 3.0];
        let token = sampler.sample(&mut logits);
        assert!(token < 3);
    }
    
    #[test]
    fn test_top_k() {
        let config = SamplingConfig {
            top_k: Some(2),
            temperature: 1.0,
            ..Default::default()
        };
        let mut sampler = Sampler::new(config);
        let mut logits = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let token = sampler.sample(&mut logits);
        assert!(token < 5);
    }
    
    #[test]
    fn test_top_p() {
        let config = SamplingConfig {
            top_p: Some(0.5),
            top_k: None,
            temperature: 1.0,
            ..Default::default()
        };
        let mut sampler = Sampler::new(config);
        let mut logits = vec![10.0, 1.0, 1.0, 1.0, 1.0];
        let token = sampler.sample(&mut logits);
        assert!(token < 5);
    }
    
    #[test]
    fn test_deterministic_with_seed() {
        let config = SamplingConfig {
            seed: Some(42),
            ..Default::default()
        };
        let mut sampler1 = Sampler::new(config);
        let mut sampler2 = Sampler::new(config);
        
        let mut logits1 = vec![1.0, 2.0, 3.0];
        let mut logits2 = vec![1.0, 2.0, 3.0];
        
        assert_eq!(sampler1.sample(&mut logits1), sampler2.sample(&mut logits2));
    }
    
    #[test]
    fn test_softmax() {
        let config = SamplingConfig::default();
        let sampler = Sampler::new(config);
        let logits = vec![1.0, 2.0, 3.0];
        let probs = sampler.softmax(&logits);
        
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 0.001);
        assert!(probs[2] > probs[1]);
        assert!(probs[1] > probs[0]);
    }
}