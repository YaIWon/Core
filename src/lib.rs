// ======================================================================
// SELF-EVOLVING LM - LIBRARY
// File: src/lib.rs
// Description: Main library exports for the self-evolving language model
//              Includes all modules: core, inference, memory, scanner,
//              learning, and system access.
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

// ======================================================================
// CORE RE-EXPORTS
// ======================================================================

pub use core::model::base_model::{
    BaseModel, 
    ModelConfig, 
    ModelBuilder,
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
};

pub use memory::blockchain::{
    BlockchainManager, 
    Block, 
    Blockchain,
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
pub use scanner::embedder::Embedder;

// ======================================================================
// LEARNING RE-EXPORTS
// ======================================================================

// Teacher
pub use learning::amoral_teacher::{
    AmoralTeacherOrchestrator,
    AmoralDeepSeekClient,
    HealthStatus,
    HealthReport,
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
    ProtocolManager,
    LearningTracker,
    LearningRecord,
    CoherenceValidator,
    CoherenceResult,
    DebugMode,
    DebugStatus,
    Diagnostic,
    ProtocolAction,
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
    CameraDevice,
    MicrophoneDevice,
    BluetoothDevice,
    SerialDevice,
    StorageDevice,
    NetworkInterface,
};

// Network Access
pub use system::network::{
    NetworkAccess,
    HttpResponse,
    WebSocketConnection,
};

// Command Executor
pub use system::commands::{
    CommandExecutor,
    CommandOutput,
};

// ======================================================================
// SHARED MEMORY RE-EXPORTS
// ======================================================================

#[cfg(unix)]
pub use learning::amoral_teacher::SharedMemoryChannel;

#[cfg(not(unix))]
pub use learning::amoral_teacher::SharedMemoryChannel;

// ======================================================================
// PRELUDE - Commonly used types
// ======================================================================

pub mod prelude {
    pub use crate::core::model::base_model::{BaseModel, ModelConfig};
    pub use crate::inference::generate::Generator;
    pub use crate::memory::vector_store::VectorStore;
    pub use crate::memory::blockchain::BlockchainManager;
    pub use crate::learning::{
        Curriculum, Topic,
        ProtocolManager, Message, MessageType,
        ComprehensiveLogger, LogLevel, LogCategory,
        AmoralTeacherOrchestrator, AmoralDeepSeekClient,
    };
    pub use crate::system::{
        SystemAccess, NetworkAccess, DeviceManager, CommandExecutor,
        PermissionManager, Permission, PermissionLevel,
    };
}
