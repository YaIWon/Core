// ======================================================================
// SELF-EVOLVING LM - LIBRARY
// File: src/lib.rs
// ======================================================================

pub mod core;
pub mod inference;
pub mod memory;
pub mod scanner;
pub mod learning;  // <-- ADD THIS LINE

// Re-export commonly used types
pub use core::model::base_model::{BaseModel, ModelConfig, ModelBuilder};
pub use inference::generate::Generator;
pub use memory::vector_store::VectorStore;
pub use memory::blockchain::BlockchainManager;
