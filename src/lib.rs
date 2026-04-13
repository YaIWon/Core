// ======================================================================
// SELF-EVOLVING LM - LIBRARY
// File: src/lib.rs
// Description: Main library exports for the self-evolving language model
// ======================================================================

pub mod core;
pub mod inference;
pub mod memory;
pub mod scanner;
pub mod learning;

// Re-export core components
pub use core::model::base_model::{
    BaseModel, 
    ModelConfig, 
    ModelBuilder,
};

// Re-export inference components
pub use inference::generate::{
    Generator, 
    GenerationConfig,
};
pub use inference::sampling::{
    SamplingConfig, 
    Sampler,
};
pub use inference::conversation::{
    Conversation, 
    ConversationManager, 
    Message,
};

// Re-export memory components
pub use memory::vector_store::{
    VectorStore, 
    VectorEntry,
};
pub use memory::blockchain::{
    BlockchainManager, 
    Block, 
    Blockchain,
};

// Re-export scanner components
pub use scanner::watcher::{
    FileWatcher, 
    WatcherConfig, 
    FileEvent, 
    ChangeType,
};
pub use scanner::ingestor::Ingestor;
pub use scanner::embedder::Embedder;

// Re-export learning components
pub use learning::{
    AmoralTeacherOrchestrator,
    AmoralDeepSeekClient,
    Curriculum,
    Topic,
    TeacherClient,
    ConfusionDetector,
    LearningCoordinator,
    start_amoral_teaching,
};
