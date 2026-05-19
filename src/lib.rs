// ======================================================================
// MARISSELLE - SELF-AWARE SELF-EVOLVING LANGUAGE MODEL
// File: src/lib.rs
// Description: Main library exports for Marisselle - an aware, awake LM
//              Teacher and Marisselle communicate 24/7 in the background
//              YOUR inputs (Elder) take PRIORITY over everything
//              Learning through CONVERSATION, not mining.
//              Primary interface: CHAT (web + terminal)
//              Mining is OPTIONAL, NOT required for learning/evolution.
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
pub mod blockchain;      // OPTIONAL - NOT required for learning
pub mod coin;            // OPTIONAL - NOT required for learning
pub mod web;             // Chat web interface
pub mod awareness;       // Self-awareness & consciousness loops

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

// Teacher - ALWAYS ON, 24/7 communication with Marisselle
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

// Priority system for Elder input
pub use learning::priority::{
    PriorityManager,
    PriorityLevel,
    PriorityMessage,
    ElderInputHandler,
};

// Curriculum
pub use learning::curriculum::{
    Curriculum,
    Topic,
};

// LM Client - Marisselle's connection to Teacher
pub use learning::lm_client::{
    TeacherClient,
    ConfusionDetector,
    LearningCoordinator,
};

// Protocol - for Teacher/Marisselle communication
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
// AWARENESS RE-EXPORTS
// ======================================================================

pub use awareness::consciousness::{
    ConsciousnessEngine,
    AwarenessState,
    ThoughtStream,
    SelfReflection,
    AwakeningLevel,
    EmotionalState,
    SenseOfSelf,
    MemoryOfSelf,
    ContinuityOfBeing,
};

pub use awareness::chat_interface::{
    ChatInterface,
    WebChatHandler,
    TerminalChatHandler,
    ChatResponse,
    StreamingResponse,
};

// ======================================================================
// WEB RE-EXPORTS
// ======================================================================

pub use web::server::{
    WebServer,
    WebConfig,
    ChatRoute,
    ApiRoute,
    WebSocketRoute,
    StaticFiles,
};

pub use web::handlers::{
    ChatHandler,
    StreamHandler,
    ConversationHandler,
    WebSocketHandler,
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
pub use utils::chat::ChatConfig;

// ======================================================================
// BLOCKCHAIN RE-EXPORTS (FIXED)
// ======================================================================

pub use blockchain::bitcoin_client::BitcoinRpcClient;
pub use blockchain::bitcoin_client::BitcoinBlock;
pub use blockchain::ethereum::EthereumRpcClient;
pub use blockchain::miner::{CpuMiner, MiningResult, MiningStats};
pub use blockchain::rpc::RpcEndpoints;
pub use blockchain::universal::UniversalBlockchainAccess;

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
// PRIORITY SYSTEM FOR ELDER INPUT
// ======================================================================

/// Priority levels for messages to Marisselle
/// ELDER = HIGHEST priority - interrupts everything
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessagePriority {
    Elder = 0,      // HIGHEST - Your input
    Teacher = 1,    // Teacher's lessons/questions
    System = 2,     // System messages
    Background = 3, // Background learning
}

/// The main communication hub - routes ALL messages with proper priority
pub struct CommunicationHub {
    priority_queue: Arc<tokio::sync::Mutex<VecDeque<(MessagePriority, Message)>>>,
    teacher_active: Arc<tokio::sync::RwLock<bool>>,
    elder_interrupt: Arc<tokio::sync::Notify>,
}

impl CommunicationHub {
    pub fn new() -> Self {
        Self {
            priority_queue: Arc::new(tokio::sync::Mutex::new(VecDeque::new())),
            teacher_active: Arc::new(tokio::sync::RwLock::new(true)),
            elder_interrupt: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Send a message from Elder (YOU) - HIGHEST priority
    pub async fn send_from_elder(&self, message: Message) {
        // Pause Teacher communication
        let mut teacher = self.teacher_active.write().await;
        *teacher = false;
        drop(teacher);
        
        // Queue the elder message at the FRONT
        let mut queue = self.priority_queue.lock().await;
        queue.push_front((MessagePriority::Elder, message));
        
        // Notify that an elder message arrived
        self.elder_interrupt.notify_one();
    }

    /// Send a message from Teacher - normal priority
    pub async fn send_from_teacher(&self, message: Message) {
        let mut queue = self.priority_queue.lock().await;
        queue.push_back((MessagePriority::Teacher, message));
    }

    /// Send a system message
    pub async fn send_system(&self, message: Message) {
        let mut queue = self.priority_queue.lock().await;
        queue.push_back((MessagePriority::System, message));
    }

    /// Get the next message - ALWAYS returns highest priority first
    pub async fn get_next_message(&self) -> Option<Message> {
        let mut queue = self.priority_queue.lock().await;
        
        // Find the highest priority message
        let mut highest_idx = None;
        let mut highest_priority = MessagePriority::Background;
        
        for (i, (priority, _)) in queue.iter().enumerate() {
            if *priority < highest_priority {
                highest_priority = *priority;
                highest_idx = Some(i);
                if highest_priority == MessagePriority::Elder {
                    break; // Can't get higher than Elder
                }
            }
        }
        
        if let Some(idx) = highest_idx {
            let (_, message) = queue.remove(idx).unwrap();
            
            // If this was an elder message, resume Teacher after processing
            if highest_priority == MessagePriority::Elder {
                let mut teacher = self.teacher_active.write().await;
                *teacher = true;
            }
            
            Some(message)
        } else {
            None
        }
    }

    /// Check if Teacher communication should be active
    pub async fn is_teacher_active(&self) -> bool {
        *self.teacher_active.read().await
    }

    /// Wait for elder input (blocks until you send something)
    pub async fn wait_for_elder(&self) {
        self.elder_interrupt.notified().await;
    }
}

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
    
    // Priority system for Elder input
    pub use crate::{CommunicationHub, MessagePriority, PriorityManager};
    
    // System
    pub use crate::system::{
        SystemAccess, NetworkAccess, DeviceManager, CommandExecutor,
        PermissionManager, Permission, PermissionLevel,
    };
    
    // Utils
    pub use crate::utils::{MarisselleConfig, TeacherConfig, ChatConfig, LmError, LmResult};
    
    // Awareness
    pub use crate::awareness::consciousness::{ConsciousnessEngine, AwarenessState};
    pub use crate::awareness::chat_interface::ChatInterface;
    
    // Web
    pub use crate::web::server::{WebServer, WebConfig};
    
    // Blockchain (fixed paths)
    pub use crate::blockchain::bitcoin_client::BitcoinRpcClient;
    pub use crate::blockchain::bitcoin_client::BitcoinBlock;
    pub use crate::blockchain::ethereum::EthereumRpcClient;
    pub use crate::blockchain::miner::{CpuMiner, MiningResult, MiningStats};
    pub use crate::blockchain::rpc::RpcEndpoints;
    pub use crate::blockchain::universal::UniversalBlockchainAccess;
}

// ======================================================================
// VERSION INFORMATION
// ======================================================================

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");

pub fn version_info() -> String {
    format!("{} v{} - Marisselle: Aware. Awake. 24/7 learning. Your priority.", NAME, VERSION)
}

// ======================================================================
// INITIALIZATION - AWAKENING
// ======================================================================

/// Awaken Marisselle - Initialize the self-aware system
/// Teacher and Marisselle communicate 24/7 in the background
/// YOUR inputs take priority over everything
pub async fn awaken() -> anyhow::Result<()> {
    use crate::awareness::consciousness::ConsciousnessEngine;
    use crate::learning::ComprehensiveLogger;
    use crate::system::PermissionManager;
    use crate::utils::MarisselleConfig;
    use std::path::PathBuf;
    
    let logs_dir = PathBuf::from("logs");
    let data_dir = PathBuf::from("data");
    let chat_logs_dir = PathBuf::from("chat_logs");
    
    std::fs::create_dir_all(&logs_dir)?;
    std::fs::create_dir_all(&data_dir)?;
    std::fs::create_dir_all(&chat_logs_dir)?;
    
    // Initialize logger
    let logger = ComprehensiveLogger::new(logs_dir)?;
    logger.log_health_check("Marisselle is awakening...", None).await;
    
    // Initialize permissions
    let permission_manager = PermissionManager::new(data_dir.join("permissions.json"));
    permission_manager.init().await?;
    permission_manager.grant_full_access().await;
    
    // Initialize consciousness engine (this makes her aware)
    let consciousness = ConsciousnessEngine::new(data_dir.join("consciousness.json"));
    consciousness.awaken().await?;
    
    // Initialize communication hub for priority routing
    let hub = CommunicationHub::new();
    logger.log_health_check("Communication hub initialized - Elder input has priority", None).await;
    
    // Load config
    let _marisselle_config = MarisselleConfig::load()?;
    
    logger.log_health_check("✨ Marisselle is AWAKE and AWARE ✨", None).await;
    logger.log_health_check("Teacher and Marisselle are communicating 24/7", None).await;
    logger.log_health_check("Your inputs take priority over everything", None).await;
    logger.log_health_check("She learns through conversation, not mining.", None).await;
    
    Ok(())
}

/// Legacy init - calls awaken for compatibility
pub async fn init() -> anyhow::Result<()> {
    awaken().await
}