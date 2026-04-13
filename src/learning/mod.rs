// ======================================================================
// LEARNING MODULE
// File: src/learning/mod.rs
// ======================================================================

pub mod amoral_teacher;
pub mod curriculum;

pub use amoral_teacher::{
    AmoralTeacherOrchestrator, 
    AmoralDeepSeekClient,
    HealthStatus, 
    HealthReport,
    start_amoral_teaching,
};
pub use curriculum::{Curriculum, Topic};
