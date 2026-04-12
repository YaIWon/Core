// ======================================================================
// INFERENCE MODULE
// File: src/inference/mod.rs
// ======================================================================

pub mod generate;
pub mod sampling;
pub mod conversation;

pub use generate::{Generator, GenerationConfig};
pub use sampling::{SamplingConfig, Sampler};
pub use conversation::{Conversation, ConversationManager, Message};
