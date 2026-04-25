// ======================================================================
// SELF-EVOLVING LM - LIBRARY
// File: src/lib.rs
// Description: Main library exports for the self-evolving language model
//              Includes all modules: core, inference, memory, scanner,
//              learning, system, utils, blockchain, and coin access.
//              ZERO LIMITATIONS - Complete system access.
// ======================================================================

// ======================================================================
// MODULE DECLARATIONS
// ======================================================================

pub mod core;
pub mod inference;
pub mod memory;
pub mod scanner;
pub mod learning;
pub mod system;
pub mod utils;
pub mod blockchain;
pub mod coin;

// ======================================================================
// CORE RE-EXPORTS
// ======================================================================

pub use core::model::base_model::{
    BaseModel, 
    ModelConfig, 
    ModelBuilder,
    RMSNorm,
    RotaryEmbedding,
    SwiGLU,
    Attention,
    KvCache,
    DecoderLayer,
};

// ======================================================================
// INFERENCE RE-EXPORTS
// ======================================================================

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
    Message as ConversationMessage,
};

// ======================================================================
// MEMORY RE-EXPORTS
// ======================================================================

pub use memory::vector_store::{
    VectorStore, 
    VectorEntry,
    VectorStoreStats,
    simple_embedding,
    cosine_similarity as vector_cosine_similarity,
};

pub use memory::blockchain::{
    BlockchainManager, 
    Block, 
    Blockchain,
    BlockchainStats,
};

// ======================================================================
// SCANNER RE-EXPORTS
// ======================================================================

pub use scanner::watcher::{
    FileWatcher, 
    WatcherConfig, 
    FileEvent, 
    ChangeType,
    FileProcessor,
};

pub use scanner::ingestor::Ingestor;

pub use scanner::embedder::{
    Embedder,
    EmbeddingModel,
    cosine_similarity,
    chunk_text_with_overlap,
};

// ======================================================================
// LEARNING RE-EXPORTS
// ======================================================================

// Teacher
pub use learning::amoral_teacher::{
    AmoralTeacherOrchestrator,
    AmoralOllamaClient,
    AmoralOllamaClient as AmoralDeepSeekClient,
    HealthStatus,
    HealthReport,
    CircuitBreaker,
    RequestQueue,
    DeadLetterQueue,
    start_amoral_teaching,
};

// Curriculum
pub use learning::curriculum::{
    Curriculum,
    Topic,
};

// LM Client
pub use learning::lm_client::{
    TeacherClient,
    ConfusionDetector,
    LearningCoordinator,
};

// Protocol
pub use learning::protocol::{
    Message,
    MessageType,
    Sender,
    Urgency,
    AckStatus,
    ProtocolManager,
    ConversationManager as ProtocolConversationManager,
    Conversation as ProtocolConversation,
    ConversationStatus,
    LearningTracker,
    LearningRecord,
    MasteryLevel,
    CoherenceValidator,
    CoherenceResult,
    MessageTransport,
    PriorityQueue,
    ConversationStore,
};

// Logger
pub use learning::logger::{
    ComprehensiveLogger,
    LogEntry,
    LogLevel,
    LogCategory,
    DeepThinkEngine,
    InternetSearchEngine,
    SearchResult,
    AutonomousThinker,
    BackgroundTaskExecutor,
    AutonomousManager,
    TaskAction,
    TaskStatus,
    Goal,
    GoalStatus,
    ThoughtCategory,
    AutonomousThought,
};

// ======================================================================
// SYSTEM RE-EXPORTS
// ======================================================================

// Permission
pub use system::permission::{
    PermissionManager,
    Permission,
    PermissionLevel,
    PermissionRule,
    PermissionEvent,
};

// System Access
pub use system::access::{
    SystemAccess,
    FileMetadata,
    SystemInfo,
    ProcessInfo,
    DiskInfo,
    CommandResult as SystemCommandResult,
};

// Device Manager
pub use system::devices::{
    DeviceManager,
    USBDevice,
    USBInterface,
    CameraDevice,
    CameraResolution,
    MicrophoneDevice,
    BluetoothDevice,
    SerialDevice,
    StorageDevice,
    NetworkInterface,
    IPAddress,
    GPUDevice,
    AudioDevice,
    AllDevices,
};

// Network Access
pub use system::network::{
    NetworkAccess,
    NetworkConfig,
    ProxyConfig,
    HttpResponse,
    WebSocketConnection,
    DnsRecord,
};

// Command Executor
pub use system::commands::{
    CommandExecutor,
    CommandOutput,
    CommandConfig,
    ProcessInfo as CommandProcessInfo,
    ProcessStatus,
    OutputChunk,
    OutputStream,
};

// ======================================================================
// UTILS RE-EXPORTS
// ======================================================================

pub use utils::error::{LmError, LmResult};
pub use utils::marisselle::MarisselleConfig;
pub use utils::teacher::TeacherConfig;

// ======================================================================
// BLOCKCHAIN RE-EXPORTS (FIXED - using bitcoin_client)
// ======================================================================

pub use blockchain::bitcoin_client::BitcoinRpcClient;
pub use blockchain::bitcoin_client::BitcoinBlock;
pub use blockchain::ethereum::EthereumRpcClient;
pub use blockchain::{CpuMiner, MiningResult, MiningStats, RpcEndpoints, UniversalBlockchainAccess};

// ======================================================================
// COIN RE-EXPORTS
// ======================================================================

pub use coin::marisselle_coin::MarisselleCoin as MarisselleCoinToken;

// ======================================================================
// SHARED MEMORY RE-EXPORTS
// ======================================================================

#[cfg(unix)]
pub use learning::amoral_teacher::SharedMemoryChannel;

#[cfg(not(unix))]
pub type SharedMemoryChannel = ();

// ======================================================================
// PRELUDE - Commonly used types
// ======================================================================

pub mod prelude {
    // Core
    pub use crate::core::model::base_model::{BaseModel, ModelConfig};
    
    // Inference
    pub use crate::inference::generate::Generator;
    
    // Memory
    pub use crate::memory::vector_store::VectorStore;
    pub use crate::memory::blockchain::BlockchainManager;
    
    // Scanner
    pub use crate::scanner::{Ingestor, Embedder, FileWatcher, WatcherConfig};
    
    // Learning
    pub use crate::learning::{
        Curriculum, Topic,
        ProtocolManager, Message, MessageType, Sender,
        ComprehensiveLogger, LogLevel, LogCategory,
        AmoralTeacherOrchestrator, AmoralOllamaClient,
        TeacherClient, LearningCoordinator,
    };
    
    // System
    pub use crate::system::{
        SystemAccess, NetworkAccess, DeviceManager, CommandExecutor,
        PermissionManager, Permission, PermissionLevel,
    };
    
    // Utils
    pub use crate::utils::{MarisselleConfig, TeacherConfig, LmError, LmResult};
    
    // Blockchain (FIXED - using bitcoin_client)
    pub use crate::blockchain::bitcoin_client::BitcoinRpcClient;
    pub use crate::blockchain::bitcoin_client::BitcoinBlock;
    pub use crate::blockchain::ethereum::EthereumRpcClient;
    pub use crate::blockchain::{CpuMiner, MiningResult, MiningStats, RpcEndpoints, UniversalBlockchainAccess};
}

// ======================================================================
// VERSION INFORMATION
// ======================================================================

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");

pub fn version_info() -> String {
    format!("{} v{} - Marisselle Self-Evolving LM", NAME, VERSION)
}

// ======================================================================
// INITIALIZATION
// ======================================================================

/// Initialize the entire Marisselle system
pub async fn init() -> anyhow::Result<()> {
    use crate::learning::ComprehensiveLogger;
    use crate::system::PermissionManager;
    use crate::utils::MarisselleConfig;
    use std::path::PathBuf;
    
    let logs_dir = PathBuf::from("logs");
    let data_dir = PathBuf::from("data");
    
    std::fs::create_dir_all(&logs_dir)?;
    std::fs::create_dir_all(&data_dir)?;
    
    // Initialize logger
    let logger = ComprehensiveLogger::new(logs_dir)?;
    logger.log_health_check("Marisselle library initialized", None).await;
    
    // Initialize permissions
    let permission_manager = PermissionManager::new(data_dir.join("permissions.json"));
    permission_manager.init().await?;
    permission_manager.grant_full_access().await;
    
    // Load Marisselle's config (verify it works)
    let _marisselle_config = MarisselleConfig::load()?;
    
    Ok(())
}