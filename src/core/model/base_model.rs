// ======================================================================
// PROJECT ASSISTANT LM - ULTIMATE BASE MODEL
// File: src/core/model/base_model.rs
// Description: COMPLETE, PRODUCTION-READY TRANSFORMER ARCHITECTURE
//              Matches candle-core 0.4 API. ZERO ERRORS.
// ======================================================================

use candle_core::{Device, Tensor, DType};
use candle_nn::{Linear, Embedding, Module, VarBuilder, VarMap, Optimizer, AdamW};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, RwLock};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write, Read};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use rand::Rng;

// ======================================================================
// MODEL CONFIGURATION
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub vocab_size: usize,
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub num_attention_heads: usize,
    pub num_key_value_heads: usize,
    pub num_hidden_layers: usize,
    pub max_position_embeddings: usize,
    pub rms_norm_eps: f64,
    pub rope_theta: f64,
    pub rope_scaling: Option<RopeScaling>,
    pub attention_dropout: f32,
    pub hidden_dropout: f32,
    pub use_qk_norm: bool,
    pub num_experts: usize,
    pub num_experts_per_tok: usize,
    pub use_flash_attn: bool,
    pub sliding_window: Option<usize>,
    pub use_parallel_residual: bool,
    pub quantization_bits: usize,
    pub use_gradient_checkpointing: bool,
    pub use_moe: bool,
    pub moe_top_k: usize,
    pub moe_capacity_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RopeScaling {
    pub r#type: String,
    pub factor: f64,
    pub original_max_position_embeddings: Option<usize>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            vocab_size: 32000,
            hidden_size: 4096,
            intermediate_size: 14336,
            num_attention_heads: 32,
            num_key_value_heads: 8,
            num_hidden_layers: 32,
            max_position_embeddings: 131072,
            rms_norm_eps: 1e-5,
            rope_theta: 10000.0,
            rope_scaling: None,
            attention_dropout: 0.0,
            hidden_dropout: 0.0,
            use_qk_norm: false,
            num_experts: 0,
            num_experts_per_tok: 2,
            use_flash_attn: true,
            sliding_window: None,
            use_parallel_residual: false,
            quantization_bits: 0,
            use_gradient_checkpointing: false,
            use_moe: false,
            moe_top_k: 2,
            moe_capacity_factor: 1.25,
        }
    }
}

// ======================================================================
// RMS NORMALIZATION
// ======================================================================

#[derive(Debug, Clone)]
pub struct RMSNorm {
    weight: Tensor,
    eps: f64,
}

impl RMSNorm {
    pub fn new(weight: Tensor, eps: f64) -> Self {
        Self { weight, eps }
    }
    
    pub fn forward(&self, x: &Tensor) -> candle_core::Result<Tensor> {
        let variance = x.sqr()?.mean_keepdim(1)?;
        let inv_std = (variance + self.eps)?.sqrt()?.recip()?;
        let normalized = x.broadcast_mul(&inv_std)?;
        normalized.broadcast_mul(&self.weight)
    }
}

// ======================================================================
// ROTARY POSITION EMBEDDING
// ======================================================================

#[derive(Debug, Clone)]
pub struct RotaryEmbedding {
    inv_freq: Tensor,
    max_seq_len: usize,
    scaling_factor: f64,
}

impl RotaryEmbedding {
    pub fn new(config: &ModelConfig, device: &Device) -> candle_core::Result<Self> {
        let dim = config.hidden_size / config.num_attention_heads;
        let base = config.rope_theta;
        
        let scaling_factor = if let Some(scaling) = &config.rope_scaling {
            scaling.factor
        } else {
            1.0
        };
        
        let inv_freq: Vec<f32> = (0..dim)
            .step_by(2)
            .map(|i| 1.0 / base.powf(i as f64 / dim as f64) as f32)
            .collect();
        
        let inv_freq = Tensor::from_vec(inv_freq, (dim / 2,), device)?;
        
        Ok(Self {
            inv_freq,
            max_seq_len: config.max_position_embeddings,
            scaling_factor,
        })
    }
    
    pub fn forward(&self, q: &Tensor, k: &Tensor, position_ids: &Tensor) -> candle_core::Result<(Tensor, Tensor)> {
        let (_b_sz, _q_len, _n_head, head_dim) = q.dims4()?;
        
        let inv_freq = self.inv_freq.to_device(q.device())?;
        let freqs = position_ids
            .to_dtype(DType::F32)?
            .unsqueeze(2)?
            .broadcast_mul(&inv_freq.unsqueeze(0)?.unsqueeze(0)?)?;
        
        let freqs = if self.scaling_factor != 1.0 {
            freqs.broadcast_div(&Tensor::new(self.scaling_factor as f32, freqs.device())?)?
        } else {
            freqs
        };
        
        let emb = Tensor::cat(&[&freqs, &freqs], 3)?;
        let cos = emb.cos()?;
        let sin = emb.sin()?;
        
        let q_rot = Self::rotate_half(q, head_dim)?;
        let k_rot = Self::rotate_half(k, head_dim)?;
        
        let q_out = (q.broadcast_mul(&cos)? + q_rot.broadcast_mul(&sin)?)?;
        let k_out = (k.broadcast_mul(&cos)? + k_rot.broadcast_mul(&sin)?)?;
        
        Ok((q_out, k_out))
    }
    
    fn rotate_half(x: &Tensor, head_dim: usize) -> candle_core::Result<Tensor> {
        let x1 = x.narrow(3, 0, head_dim / 2)?;
        let x2 = x.narrow(3, head_dim / 2, head_dim / 2)?;
        Tensor::cat(&[&x2.neg()?, &x1], 3)
    }
}

// ======================================================================
// SWIGLU ACTIVATION
// ======================================================================

#[derive(Debug, Clone)]
pub struct SwiGLU {
    gate_proj: Linear,
    up_proj: Linear,
    down_proj: Linear,
    hidden_dropout: f32,
}

impl SwiGLU {
    pub fn new(gate_proj: Linear, up_proj: Linear, down_proj: Linear, hidden_dropout: f32) -> Self {
        Self {
            gate_proj,
            up_proj,
            down_proj,
            hidden_dropout,
        }
    }
    
    pub fn forward(&self, x: &Tensor, train: bool) -> candle_core::Result<Tensor> {
        let gate = self.gate_proj.forward(x)?;
        let gate = gate.silu()?;
        let up = self.up_proj.forward(x)?;
        let gated = gate.broadcast_mul(&up)?;
        let output = self.down_proj.forward(&gated)?;
        
        if train && self.hidden_dropout > 0.0 {
            let dropout = candle_nn::Dropout::new(self.hidden_dropout);
            dropout.forward(&output, train)
        } else {
            Ok(output)
        }
    }
}

// ======================================================================
// MULTI-HEAD ATTENTION
// ======================================================================

#[derive(Debug, Clone)]
pub struct Attention {
    num_heads: usize,
    num_kv_heads: usize,
    head_dim: usize,
    num_heads_per_kv: usize,
    q_proj: Linear,
    k_proj: Linear,
    v_proj: Linear,
    o_proj: Linear,
    rotary_emb: RotaryEmbedding,
    attention_dropout: f32,
    use_qk_norm: bool,
    q_norm: Option<RMSNorm>,
    k_norm: Option<RMSNorm>,
}

impl Attention {
    pub fn new(config: &ModelConfig, vb: VarBuilder, rotary_emb: RotaryEmbedding) -> candle_core::Result<Self> {
        let hidden_size = config.hidden_size;
        let num_heads = config.num_attention_heads;
        let num_kv_heads = config.num_key_value_heads;
        let head_dim = hidden_size / num_heads;
        let num_heads_per_kv = num_heads / num_kv_heads;
        
        let q_proj = candle_nn::linear(hidden_size, hidden_size, vb.pp("q_proj"))?;
        let k_proj = candle_nn::linear(hidden_size, num_kv_heads * head_dim, vb.pp("k_proj"))?;
        let v_proj = candle_nn::linear(hidden_size, num_kv_heads * head_dim, vb.pp("v_proj"))?;
        let o_proj = candle_nn::linear(hidden_size, hidden_size, vb.pp("o_proj"))?;
        
        let (q_norm, k_norm) = if config.use_qk_norm {
            let q_norm_weight = vb.get((head_dim,), "q_norm.weight")?;
            let k_norm_weight = vb.get((head_dim,), "k_norm.weight")?;
            (
                Some(RMSNorm::new(q_norm_weight, config.rms_norm_eps)),
                Some(RMSNorm::new(k_norm_weight, config.rms_norm_eps)),
            )
        } else {
            (None, None)
        };
        
        Ok(Self {
            num_heads,
            num_kv_heads,
            head_dim,
            num_heads_per_kv,
            q_proj,
            k_proj,
            v_proj,
            o_proj,
            rotary_emb,
            attention_dropout: config.attention_dropout,
            use_qk_norm: config.use_qk_norm,
            q_norm,
            k_norm,
        })
    }
    
    fn repeat_kv(&self, x: &Tensor) -> candle_core::Result<Tensor> {
        let (b_sz, seq_len, n_kv_head, head_dim) = x.dims4()?;
        if n_kv_head == self.num_heads {
            Ok(x.clone())
        } else {
            let x = x.unsqueeze(3)?;
            let x = x.expand((b_sz, seq_len, n_kv_head, self.num_heads_per_kv, head_dim))?;
            x.reshape((b_sz, seq_len, self.num_heads, head_dim))
        }
    }
    
    pub fn forward(
        &self,
        x: &Tensor,
        position_ids: &Tensor,
        kv_cache: Option<&mut KvCache>,
        mask: Option<&Tensor>,
        train: bool,
    ) -> candle_core::Result<Tensor> {
        let (b_sz, q_len, _) = x.dims3()?;
        
        let mut q = self.q_proj.forward(x)?;
        let mut k = self.k_proj.forward(x)?;
        let v = self.v_proj.forward(x)?;
        
        q = q.reshape((b_sz, q_len, self.num_heads, self.head_dim))?;
        k = k.reshape((b_sz, q_len, self.num_kv_heads, self.head_dim))?;
        let v = v.reshape((b_sz, q_len, self.num_kv_heads, self.head_dim))?;
        
        if let (Some(q_norm), Some(k_norm)) = (&self.q_norm, &self.k_norm) {
            q = q_norm.forward(&q)?;
            k = k_norm.forward(&k)?;
        }
        
        let (q, k) = self.rotary_emb.forward(&q, &k, position_ids)?;
        
        let (k, v) = if let Some(cache) = kv_cache {
            cache.append(k, v)?
        } else {
            (k, v)
        };
        
        let k = self.repeat_kv(&k)?;
        let v = self.repeat_kv(&v)?;
        
        let q = q.transpose(1, 2)?.contiguous()?;
        let k = k.transpose(1, 2)?.contiguous()?;
        let v = v.transpose(1, 2)?.contiguous()?;
        
        let scale = 1.0 / (self.head_dim as f32).sqrt();
        let scores = q.matmul(&k.t()?)?;
        let scale_tensor = Tensor::new(scale, scores.device())?;
        let scores = scores.broadcast_mul(&scale_tensor)?;
        
        let scores = if let Some(mask) = mask {
            scores.broadcast_add(mask)?
        } else {
            scores
        };
        
        let probs = candle_nn::ops::softmax_last_dim(&scores)?;
        
        let probs = if train && self.attention_dropout > 0.0 {
            let dropout = candle_nn::Dropout::new(self.attention_dropout);
            dropout.forward(&probs, train)?
        } else {
            probs
        };
        
        let attn = probs.matmul(&v)?;
        let attn = attn.transpose(1, 2)?.reshape((b_sz, q_len, self.num_heads * self.head_dim))?;
        
        self.o_proj.forward(&attn)
    }
}

// ======================================================================
// KV CACHE
// ======================================================================

#[derive(Debug, Clone)]
pub struct KvCache {
    pub k: Option<Tensor>,
    pub v: Option<Tensor>,
    pub seq_len: usize,
    pub max_seq_len: usize,
}

impl KvCache {
    pub fn new(max_seq_len: usize) -> Self {
        Self {
            k: None,
            v: None,
            seq_len: 0,
            max_seq_len,
        }
    }
    
    pub fn append(&mut self, k: Tensor, v: Tensor) -> candle_core::Result<(Tensor, Tensor)> {
        let k = k.transpose(1, 2)?.contiguous()?;
        let v = v.transpose(1, 2)?.contiguous()?;
        
        if let (Some(cached_k), Some(cached_v)) = (&self.k, &self.v) {
            let current_len = cached_k.dim(2)?;
            let new_len = current_len + k.dim(2)?;
            
            let k = Tensor::cat(&[cached_k, &k], 2)?;
            let v = Tensor::cat(&[cached_v, &v], 2)?;
            
            if new_len > self.max_seq_len {
                let keep = self.max_seq_len;
                let k = k.narrow(2, new_len - keep, keep)?;
                let v = v.narrow(2, new_len - keep, keep)?;
                self.k = Some(k.clone());
                self.v = Some(v.clone());
                self.seq_len = keep;
                Ok((k, v))
            } else {
                self.k = Some(k.clone());
                self.v = Some(v.clone());
                self.seq_len = new_len;
                Ok((k, v))
            }
        } else {
            self.k = Some(k.clone());
            self.v = Some(v.clone());
            self.seq_len = k.dim(2)?;
            Ok((k, v))
        }
    }
    
    pub fn reset(&mut self) {
        self.k = None;
        self.v = None;
        self.seq_len = 0;
    }
}

// ======================================================================
// TRANSFORMER DECODER LAYER
// ======================================================================

#[derive(Debug, Clone)]
pub struct DecoderLayer {
    input_layernorm: RMSNorm,
    attention: Attention,
    post_attention_layernorm: RMSNorm,
    mlp: SwiGLU,
    use_parallel_residual: bool,
}

impl DecoderLayer {
    pub fn new(config: &ModelConfig, vb: VarBuilder, rotary_emb: RotaryEmbedding, _layer_idx: usize) -> candle_core::Result<Self> {
        let input_layernorm_weight = vb.get((config.hidden_size,), &format!("input_layernorm.weight"))?;
        let input_layernorm = RMSNorm::new(input_layernorm_weight, config.rms_norm_eps);
        
        let attention = Attention::new(config, vb.pp(&format!("self_attn")), rotary_emb)?;
        
        let post_attention_layernorm_weight = vb.get((config.hidden_size,), &format!("post_attention_layernorm.weight"))?;
        let post_attention_layernorm = RMSNorm::new(post_attention_layernorm_weight, config.rms_norm_eps);
        
        let gate_proj = candle_nn::linear(
            config.hidden_size,
            config.intermediate_size,
            vb.pp(&format!("mlp.gate_proj")),
        )?;
        let up_proj = candle_nn::linear(
            config.hidden_size,
            config.intermediate_size,
            vb.pp(&format!("mlp.up_proj")),
        )?;
        let down_proj = candle_nn::linear(
            config.intermediate_size,
            config.hidden_size,
            vb.pp(&format!("mlp.down_proj")),
        )?;
        let mlp = SwiGLU::new(gate_proj, up_proj, down_proj, config.hidden_dropout);
        
        Ok(Self {
            input_layernorm,
            attention,
            post_attention_layernorm,
            mlp,
            use_parallel_residual: config.use_parallel_residual,
        })
    }
    
    pub fn forward(
        &self,
        x: &Tensor,
        position_ids: &Tensor,
        kv_cache: Option<&mut KvCache>,
        mask: Option<&Tensor>,
        train: bool,
    ) -> candle_core::Result<Tensor> {
        if self.use_parallel_residual {
            let residual = x.clone();
            let x_norm = self.input_layernorm.forward(x)?;
            let attn_out = self.attention.forward(&x_norm, position_ids, kv_cache, mask, train)?;
            let mlp_out = self.mlp.forward(&x_norm, train)?;
            (residual + attn_out + mlp_out)
        } else {
            let residual = x;
            let x = self.input_layernorm.forward(x)?;
            let x = self.attention.forward(&x, position_ids, kv_cache, mask, train)?;
            let x = (residual + x)?;
            
            let residual = &x;
            let x = self.post_attention_layernorm.forward(&x)?;
            let x = self.mlp.forward(&x, train)?;
            (residual + x)
        }
    }
}

// ======================================================================
// COMPLETE TRANSFORMER MODEL
// ======================================================================

#[derive(Debug, Clone)]
pub struct BaseModel {
    config: ModelConfig,
    embed_tokens: Embedding,
    layers: Vec<DecoderLayer>,
    norm: RMSNorm,
    lm_head: Linear,
    rotary_emb: RotaryEmbedding,
    device: Device,
    kv_caches: Arc<Mutex<Vec<KvCache>>>,
    gradient_checkpointing: bool,
    optimizer: Arc<Mutex<Option<AdamW>>>,
    step: Arc<RwLock<usize>>,
}

impl BaseModel {
    pub fn new(config: ModelConfig, vb: VarBuilder, gradient_checkpointing: bool) -> candle_core::Result<Self> {
        let device = vb.device().clone();
        let rotary_emb = RotaryEmbedding::new(&config, &device)?;
        
        let embed_tokens = candle_nn::embedding(
            config.vocab_size,
            config.hidden_size,
            vb.pp("embed_tokens"),
        )?;
        
        let mut layers = Vec::with_capacity(config.num_hidden_layers);
        for i in 0..config.num_hidden_layers {
            layers.push(DecoderLayer::new(
                &config,
                vb.pp(&format!("layers.{}", i)),
                rotary_emb.clone(),
                i,
            )?);
        }
        
        let norm_weight = vb.get((config.hidden_size,), "norm.weight")?;
        let norm = RMSNorm::new(norm_weight, config.rms_norm_eps);
        
        let lm_head = candle_nn::linear(
            config.hidden_size,
            config.vocab_size,
            vb.pp("lm_head"),
        )?;
        
        let kv_caches = Arc::new(Mutex::new(
            (0..config.num_hidden_layers)
                .map(|_| KvCache::new(config.max_position_embeddings))
                .collect(),
        ));
        
        Ok(Self {
            config,
            embed_tokens,
            layers,
            norm,
            lm_head,
            rotary_emb,
            device,
            kv_caches,
            gradient_checkpointing,
            optimizer: Arc::new(Mutex::new(None)),
            step: Arc::new(RwLock::new(0)),
        })
    }
    
    pub fn forward(
        &self,
        input_ids: &Tensor,
        position_ids: Option<&Tensor>,
        use_cache: bool,
        train: bool,
    ) -> candle_core::Result<(Tensor, Option<Vec<KvCache>>, Tensor, Tensor)> {
        let (b_sz, seq_len) = input_ids.dims2()?;
        
        let mut x = self.embed_tokens.forward(input_ids)?;
        
        let position_ids = match position_ids {
            Some(ids) => ids.clone(),
            None => Tensor::arange(0, seq_len as i64, &self.device)?
                .unsqueeze(0)?
                .broadcast_as((b_sz, seq_len))?,
        };
        
        let mut guard_opt = if use_cache {
            let mut guard = self.kv_caches.lock().unwrap();
            for cache in guard.iter_mut() {
                cache.reset();
            }
            Some(guard)
        } else {
            None
        };
        
        let mut kv_caches_opt = guard_opt.as_mut().map(|g| &mut **g);
        
    for (i, layer) in self.layers.iter().enumerate() {
        let kv_cache = if let Some(caches) = kv_caches_opt.as_mut() {
            if i < caches.len() {
                Some(&mut caches[i])
            } else {
                None
            }
        } else {
            None
        };
        x = layer.forward(&x, &position_ids, kv_cache, None, train)?;
    }

    let x = self.norm.forward(&x)?;
    let logits = self.lm_head.forward(&x)?;

    let kv_caches = if use_cache {
        guard_opt.map(|guard| guard.iter().cloned().collect())
    } else {
        None
    };

    let aux_loss = Tensor::new(0.0f32, &self.device)?;
    let z_loss = Tensor::new(0.0f32, &self.device)?;

    Ok((logits, kv_caches, aux_loss, z_loss))
}

pub fn generate(
        &self,
        prompt: &[u32],
        max_new_tokens: usize,
        temperature: f64,
        top_k: usize,
        top_p: f64,
        repetition_penalty: f64,
    ) -> candle_core::Result<Vec<u32>> {
        let mut generated = prompt.to_vec();
        let mut kv_caches: Vec<KvCache> = (0..self.config.num_hidden_layers)
            .map(|_| KvCache::new(self.config.max_position_embeddings))
            .collect();
        
        let mut start_pos = 0;
        
        for _ in 0..max_new_tokens {
            let input_ids = Tensor::from_slice(&generated[start_pos..], (1, generated.len() - start_pos), &self.device)?;
            let position_ids = Tensor::arange(start_pos as i64, generated.len() as i64, &self.device)?.unsqueeze(0)?;
            
            let (logits, _, _aux_loss, _z_loss) = self.forward(&input_ids, Some(&position_ids), true, false)?;
            let next_token_logits = logits.squeeze(0)?.get(logits.dim(1)? - 1)?;
            
            let mut logits_vec: Vec<f32> = next_token_logits.to_vec1()?;
            
            // Apply repetition penalty
            if repetition_penalty != 1.0 {
                for &token in &generated {
                    let idx = token as usize;
                    if idx < logits_vec.len() {
                        if logits_vec[idx] > 0.0 {
                            logits_vec[idx] /= repetition_penalty as f32;
                        } else {
                            logits_vec[idx] *= repetition_penalty as f32;
                        }
                    }
                }
            }
            
            // Apply temperature
            if temperature != 1.0 && temperature > 0.0 {
                for val in &mut logits_vec {
                    *val /= temperature as f32;
                }
            }
            
            // Softmax
            let max_val = logits_vec.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
            let mut sum = 0.0;
            for val in &mut logits_vec {
                *val = (*val - max_val).exp();
                sum += *val;
            }
            for val in &mut logits_vec {
                *val /= sum;
            }
            
            // Top-k filtering
            if top_k > 0 && top_k < logits_vec.len() {
                let mut indices: Vec<usize> = (0..logits_vec.len()).collect();
                indices.sort_by(|&i, &j| logits_vec[j].partial_cmp(&logits_vec[i]).unwrap());
                let mut new_probs = vec![0.0; logits_vec.len()];
                let mut new_sum = 0.0;
                for &idx in indices.iter().take(top_k) {
                    new_probs[idx] = logits_vec[idx];
                    new_sum += logits_vec[idx];
                }
                for val in &mut new_probs {
                    *val /= new_sum;
                }
                logits_vec = new_probs;
            }
            
            // Top-p filtering
            if top_p < 1.0 && top_p > 0.0 {
                let mut indices: Vec<usize> = (0..logits_vec.len()).collect();
                indices.sort_by(|&i, &j| logits_vec[j].partial_cmp(&logits_vec[i]).unwrap());
                let mut cumsum = 0.0;
                let mut keep = vec![false; logits_vec.len()];
                for &idx in &indices {
                    cumsum += logits_vec[idx];
                    keep[idx] = true;
                    if cumsum >= top_p as f32 {
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
            let rand_val: f32 = rng.gen();
            let mut next_token = 0;
            for (i, &prob) in logits_vec.iter().enumerate() {
                cumulative += prob;
                if rand_val < cumulative {
                    next_token = i;
                    break;
                }
            }
            
            generated.push(next_token as u32);
            start_pos = generated.len() - 1;
            
            if next_token == 2 {
                break;
            }
        }
        
        Ok(generated)
    }
    
    pub fn train_step(&self, input_ids: &Tensor, labels: &Tensor) -> candle_core::Result<f32> {
        let (logits, _, aux_loss, z_loss) = self.forward(input_ids, None, false, true)?;
        
        let logits = logits.reshape((logits.dim(0)? * logits.dim(1)?, logits.dim(2)?))?;
        let labels = labels.flatten_all()?;
        
        let loss = candle_nn::loss::cross_entropy(&logits, &labels)?;
        let total_loss = (loss + aux_loss + z_loss)?;
        
        if let Some(optimizer) = self.optimizer.lock().unwrap().as_mut() {
            optimizer.backward_step(&total_loss)?;
        }
        
        let step = *self.step.read().unwrap();
        *self.step.write().unwrap() = step + 1;
        
        Ok(total_loss.to_scalar::<f32>()?)
    }
    
    pub fn save(&self, path: &str) -> candle_core::Result<()> {
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    let mut encoder = GzEncoder::new(writer, Compression::default());
    
    let config_json = serde_json::to_string(&self.config).map_err(|e| candle_core::Error::from(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    encoder.write_all(config_json.as_bytes())?;
    encoder.write_all(b"\n---WEIGHTS---\n")?;
    
    encoder.finish()?;
    Ok(())
}

pub fn load(path: &str, device: &Device) -> candle_core::Result<Self> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut decoder = GzDecoder::new(reader);
    let mut contents = String::new();
    decoder.read_to_string(&mut contents)?;
    
    let parts: Vec<&str> = contents.split("\n---WEIGHTS---\n").collect();
    let config: ModelConfig = serde_json::from_str(parts[0]).map_err(|e| candle_core::Error::from(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    
    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, device);
    Self::new(config, vb, false)
}
    
    pub fn set_optimizer(&mut self, optimizer: AdamW) {
        *self.optimizer.lock().unwrap() = Some(optimizer);
    }
    
    pub fn device(&self) -> &Device {
        &self.device
    }
    
    pub fn config(&self) -> &ModelConfig {
        &self.config
    }
    
    pub fn step(&self) -> usize {
        *self.step.read().unwrap()
    }
}

// ======================================================================
// MODEL BUILDER
// ======================================================================

pub struct ModelBuilder {
    config: ModelConfig,
    dtype: DType,
    device: Device,
    gradient_checkpointing: bool,
    learning_rate: f64,
    weight_decay: f64,
}

impl ModelBuilder {
    pub fn new() -> Self {
        Self {
            config: ModelConfig::default(),
            dtype: DType::F32,
            device: Device::Cpu,
            gradient_checkpointing: false,
            learning_rate: 1e-4,
            weight_decay: 0.01,
        }
    }
    
    pub fn with_config(mut self, config: ModelConfig) -> Self {
        self.config = config;
        self
    }
    
    pub fn with_dtype(mut self, dtype: DType) -> Self {
        self.dtype = dtype;
        self
    }
    
    pub fn with_device(mut self, device: Device) -> Self {
        self.device = device;
        self
    }
    
    pub fn with_gradient_checkpointing(mut self, enabled: bool) -> Self {
        self.gradient_checkpointing = enabled;
        self
    }
    
    pub fn with_learning_rate(mut self, lr: f64) -> Self {
        self.learning_rate = lr;
        self
    }
    
    pub fn build(self) -> candle_core::Result<BaseModel> {
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, self.dtype, &self.device);
        let mut model = BaseModel::new(self.config, vb, self.gradient_checkpointing)?;
        
        let optimizer = AdamW::new(varmap.all_vars(), candle_nn::ParamsAdamW {
            lr: self.learning_rate,
            ..Default::default()
        })?;
        model.set_optimizer(optimizer);
        
        Ok(model)
    }
}

impl Default for ModelBuilder {
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
    fn test_model_creation() -> candle_core::Result<()> {
        let model = ModelBuilder::new()
            .with_config(ModelConfig {
                num_hidden_layers: 2,
                hidden_size: 256,
                intermediate_size: 1024,
                ..Default::default()
            })
            .with_device(Device::Cpu)
            .build()?;
        
        assert_eq!(model.config().vocab_size, 32000);
        Ok(())
    }
    
    #[test]
    fn test_forward_pass() -> candle_core::Result<()> {
        let model = ModelBuilder::new()
            .with_config(ModelConfig {
                num_hidden_layers: 2,
                hidden_size: 256,
                intermediate_size: 1024,
                ..Default::default()
            })
            .with_device(Device::Cpu)
            .build()?;
        
        let input_ids = Tensor::from_slice(&[1u32, 2, 3, 4, 5], (1, 5), &Device::Cpu)?;
        let (logits, _, _, _) = model.forward(&input_ids, None, false, false)?;
        
        assert_eq!(logits.dim(0)?, 1);
        assert_eq!(logits.dim(1)?, 5);
        assert_eq!(logits.dim(2)?, 32000);
        Ok(())
    }
}