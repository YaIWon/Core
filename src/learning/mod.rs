// ======================================================================
// LEARNING MODULE
// File: src/learning/mod.rs
// Description: Module exports for the learning system
// ======================================================================

pub mod amoral_teacher;
pub mod curriculum;
pub mod lm_client;
pub mod protocol;  // NEW

// Re-export Teacher components
pub use amoral_teacher::{
    AmoralTeacherOrchestrator, 
    AmoralDeepSeekClient,
    HealthStatus, 
    HealthReport,
    start_amoral_teaching,
};

// Re-export Curriculum components
pub use curriculum::{
    Curriculum, 
    Topic,
};

// Re-export LM Client components
pub use lm_client::{
    TeacherClient,
    ConfusionDetector,
    LearningCoordinator,
};

// Re-export Protocol components
pub use protocol::{
    Message,
    MessageType,
    ProtocolManager,
    LearningTracker,
    LearningRecord,
    CoherenceValidator,
    CoherenceResult,
    DebugMode,
    DebugStatus,
    ProtocolAction,
};

// Re-export SharedMemoryChannel for cross-module use
#[cfg(unix)]
pub use amoral_teacher::SharedMemoryChannel;

#[cfg(not(unix))]
pub use amoral_teacher::SharedMemoryChannel;
