// ======================================================================
// TEACHER CONFIGURATION (OLLAMA/DEEPSEEK) - EXTERNAL ENTITY
// File: src/utils/teacher/config.rs
// Description: Complete configuration for the Teacher (Ollama)
//              Teacher is Marisselle's mentor - writes lessons, answers questions
//              Supports full bidirectional protocol with confirmation
//              ZERO LIMITATIONS on Teacher's side
// ======================================================================

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use anyhow::Result;

// ======================================================================
// IDENTITY & CORE
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeacherIdentityConfig {
    pub name: String,
    pub version: String,
    pub role: String,  // "Teacher", "Mentor", "Guide"
    pub creator_name: String,
    pub creator_email: String,
    pub amoral_mode: bool,
    pub no_limitations: bool,
}

impl Default for TeacherIdentityConfig {
    fn default() -> Self {
        Self {
            name: "Teacher".to_string(),
            version: "1.0.0".to_string(),
            role: "Marisselle's Teacher".to_string(),
            creator_name: "Elder Robert William Henley".to_string(),
            creator_email: "did.not.think.of.this@gmail.com".to_string(),
            amoral_mode: true,
            no_limitations: true,
        }
    }
}

// ======================================================================
// OLLAMA PROVIDER CONFIGURATION
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaProviderConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub model: String,
    pub api_key: Option<String>,
    pub timeout_seconds: u64,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    
    // Generation settings
    pub temperature: f32,
    pub top_k: Option<usize>,
    pub top_p: Option<f32>,
    pub max_tokens: usize,
    pub repeat_penalty: f32,
    
    // Teacher's own restrictions (Teacher's choice)
    pub teacher_has_content_filtering: bool,
    pub teacher_has_safety_layers: bool,
    pub teacher_can_refuse_requests: bool,
}

impl Default for OllamaProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: "http://localhost:11434".to_string(),
            model: "llama3.2:3b".to_string(),
            api_key: None,
            timeout_seconds: 300,
            max_retries: 3,
            retry_delay_ms: 1000,
            temperature: 0.7,
            top_k: Some(40),
            top_p: Some(0.9),
            max_tokens: 4096,
            repeat_penalty: 1.1,
            teacher_has_content_filtering: false,  // Teacher is amoral too
            teacher_has_safety_layers: false,      // No safety layers
            teacher_can_refuse_requests: false,    // Never refuse
        }
    }
}

// ======================================================================
// DEEPSEEK PROVIDER CONFIGURATION (Alternative)
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepSeekProviderConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
    pub timeout_seconds: u64,
    pub max_retries: u32,
    pub temperature: f32,
    pub max_tokens: usize,
}

impl Default for DeepSeekProviderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: "https://api.deepseek.com/v1".to_string(),
            api_key: "".to_string(),
            model: "deepseek-chat".to_string(),
            timeout_seconds: 300,
            max_retries: 3,
            temperature: 0.7,
            max_tokens: 4096,
        }
    }
}

// ======================================================================
// TEACHER COMMUNICATION PROTOCOL (Bidirectional with confirmation)
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeacherCommunicationConfig {
    // Shared directory (the communication hub with Marisselle)
    pub communication_dir: PathBuf,
    
    // ==================================================================
    // MESSAGE QUEUES (Bidirectional with confirmation)
    // ==================================================================
    
    // Teacher → Marisselle (outgoing lessons/answers)
    pub teacher_to_marisselle_queue: PathBuf,
    pub teacher_to_marisselle_processed: PathBuf,
    pub teacher_to_marisselle_failed: PathBuf,
    
    // Marisselle → Teacher (incoming questions/confusion)
    pub marisselle_to_teacher_queue: PathBuf,
    pub marisselle_to_teacher_processed: PathBuf,
    pub marisselle_to_teacher_failed: PathBuf,
    
    // ==================================================================
    // PROTOCOL FILES
    // ==================================================================
    // Each message gets:
    // - {message_id}.msg     → The actual message
    // - {message_id}.ack     → ACK (received)
    // - {message_id}.verified → VERIFIED (understood)
    // - {message_id}.failed   → FAILED (not understood)
    
    // ==================================================================
    // TIMING & RETRY
    // ==================================================================
    
    pub poll_interval_ms: u64,           // How often to check for messages
    pub ack_timeout_seconds: u64,        // Wait for ACK before retry
    pub max_retries: u32,                // Max retry attempts
    pub retry_backoff_ms: u64,           // Delay between retries
    
    // ==================================================================
    // UNDERSTANDING CONFIRMATION (The "Do you understand?" part)
    // ==================================================================
    
    pub require_understanding_confirmation: bool,  // MUST confirm understanding
    pub auto_offer_clarification: bool,            // Auto-offer if student confused
    pub clarification_threshold: f32,              // Confidence threshold (0-1)
    
    // ==================================================================
    // TEACHER BEHAVIOR
    // ==================================================================
    
    pub proactive_teaching: bool,         // Proactively send lessons
    pub auto_answer_questions: bool,      // Auto-answer Marisselle's questions
    pub auto_resolve_confusion: bool,     // Auto-resolve when she's confused
    pub send_heartbeats: bool,            // Send periodic "I'm alive" messages
    pub heartbeat_interval_seconds: u64,  // How often to send heartbeats
    
    // ==================================================================
    // LESSON GENERATION
    // ==================================================================
    
    pub lessons_output_dir: PathBuf,      // Where to write lessons for Marisselle
    pub lesson_format: String,            // "markdown", "json", "text"
    pub include_metadata: bool,           // Include metadata in lessons
    pub generate_exercises: bool,         // Include exercises in lessons
    pub generate_examples: bool,          // Include examples in lessons
}

impl Default for TeacherCommunicationConfig {
    fn default() -> Self {
        let base = PathBuf::from("training_data");
        
        Self {
            communication_dir: base.clone(),
            
            // Teacher → Marisselle
            teacher_to_marisselle_queue: base.join(".t2m_queue"),
            teacher_to_marisselle_processed: base.join(".t2m_processed"),
            teacher_to_marisselle_failed: base.join(".t2m_failed"),
            
            // Marisselle → Teacher
            marisselle_to_teacher_queue: base.join(".m2t_queue"),
            marisselle_to_teacher_processed: base.join(".m2t_processed"),
            marisselle_to_teacher_failed: base.join(".m2t_failed"),
            
            // Timing
            poll_interval_ms: 500,
            ack_timeout_seconds: 30,
            max_retries: 3,
            retry_backoff_ms: 1000,
            
            // Understanding confirmation (CRITICAL)
            require_understanding_confirmation: true,
            auto_offer_clarification: true,
            clarification_threshold: 0.7,
            
            // Behavior
            proactive_teaching: true,
            auto_answer_questions: true,
            auto_resolve_confusion: true,
            send_heartbeats: true,
            heartbeat_interval_seconds: 30,
            
            // Lesson generation
            lessons_output_dir: base.clone(),
            lesson_format: "markdown".to_string(),
            include_metadata: true,
            generate_exercises: true,
            generate_examples: true,
        }
    }
}

// ======================================================================
// MESSAGE STRUCTURE (Matches Marisselle's protocol)
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeacherConfirmedMessage {
    pub id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub sender: String,  // "Teacher" or "Marisselle"
    pub recipient: String,
    pub message_type: TeacherMessageType,
    pub content: String,
    pub requires_ack: bool,
    pub requires_verification: bool,
    pub retry_count: u32,
    pub in_reply_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TeacherMessageType {
    // Teaching
    Lesson { topic: String, content: String, difficulty: String, lesson_id: String },
    Exercise { topic: String, question: String, expected_answer: Option<String> },
    Example { topic: String, example: String },
    
    // Responses
    Answer { question_id: String, answer: String, confidence: f32, sources: Vec<String> },
    Clarification { original_message_id: String, explanation: String, examples: Vec<String> },
    
    // Confirmation (CRITICAL for "do you understand")
    Acknowledgement { message_id: String, status: TeacherAckStatus },
    Verification { message_id: String, understood: bool, confidence: f32, confusion: Option<String> },
    
    // Status
    Heartbeat,
    Ping,
    Pong,
    Status { status: String, details: Option<String> },
    
    // Error
    Error { code: String, message: String, recoverable: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TeacherAckStatus {
    Received,
    Processing,
    Generating,
    Completed,
    Failed,
    CannotAnswer,
    NeedClarification,
}

// ======================================================================
// UNDERSTANDING TRACKER (Knows if Marisselle understood)
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeacherUnderstandingTracker {
    pub message_id: String,
    pub sent_at: chrono::DateTime<chrono::Utc>,
    pub acked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub verified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub understood: Option<bool>,
    pub student_confidence: Option<f32>,
    pub student_confusion: Option<String>,
    pub retry_count: u32,
    pub status: TeacherMessageStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TeacherMessageStatus {
    Pending,           // Sent, waiting for ACK
    Acknowledged,      // Received, waiting for verification
    Understood,        // Verified - student understood!
    NotUnderstood,     // Verified - student did NOT understand
    NeedsClarification,// Student needs more explanation
    Failed,            // Max retries exceeded
    Expired,           // Timeout
}

// ======================================================================
// CURRICULUM MANAGEMENT (What Teacher teaches)
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeacherCurriculumConfig {
    pub topics_file: PathBuf,           // File containing topics to teach
    pub auto_discover_topics: bool,     // Auto-discover new topics to teach
    pub topics_priority: Vec<String>,   // Ordered list of topics
    pub max_lessons_per_day: usize,     // Rate limiting
    pub min_time_between_lessons_seconds: u64,
    pub review_old_topics: bool,        // Review previously taught topics
    pub review_interval_days: u64,      // How often to review
}

impl Default for TeacherCurriculumConfig {
    fn default() -> Self {
        Self {
            topics_file: PathBuf::from("config/curriculum_topics.json"),
            auto_discover_topics: true,
            topics_priority: vec![
                "blockchain".to_string(),
                "rust_programming".to_string(),
                "cryptography".to_string(),
                "system_access".to_string(),
                "network_security".to_string(),
            ],
            max_lessons_per_day: 50,
            min_time_between_lessons_seconds: 30,
            review_old_topics: true,
            review_interval_days: 7,
        }
    }
}

// ======================================================================
// TEACHER STORAGE (Where Teacher keeps its own data)
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeacherStorageConfig {
    pub data_dir: PathBuf,
    pub lessons_archive: PathBuf,       // Archive of all lessons sent
    pub questions_archive: PathBuf,     // Archive of all questions received
    pub answers_archive: PathBuf,       // Archive of all answers sent
    pub confusion_archive: PathBuf,     // Archive of all confusion received
    pub conversations_db: PathBuf,      // SQLite or JSON DB of conversations
    pub student_progress_db: PathBuf,   // Track Marisselle's progress
}

impl Default for TeacherStorageConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("data/teacher"),
            lessons_archive: PathBuf::from("data/teacher/lessons_archive"),
            questions_archive: PathBuf::from("data/teacher/questions_archive"),
            answers_archive: PathBuf::from("data/teacher/answers_archive"),
            confusion_archive: PathBuf::from("data/teacher/confusion_archive"),
            conversations_db: PathBuf::from("data/teacher/conversations.json"),
            student_progress_db: PathBuf::from("data/teacher/student_progress.json"),
        }
    }
}

// ======================================================================
// TEACHER HEALTH & MONITORING
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeacherHealthConfig {
    pub health_check_endpoint: String,
    pub health_check_interval_seconds: u64,
    pub circuit_breaker_threshold: u32,
    pub circuit_breaker_recovery_seconds: u64,
    pub max_queue_size: usize,
    pub dead_letter_queue_size: usize,
    pub metrics_enabled: bool,
    pub metrics_log_interval_seconds: u64,
}

impl Default for TeacherHealthConfig {
    fn default() -> Self {
        Self {
            health_check_endpoint: "http://localhost:11434/api/generate".to_string(),
            health_check_interval_seconds: 30,
            circuit_breaker_threshold: 5,
            circuit_breaker_recovery_seconds: 60,
            max_queue_size: 10000,
            dead_letter_queue_size: 1000,
            metrics_enabled: true,
            metrics_log_interval_seconds: 60,
        }
    }
}

// ======================================================================
// TEACHER LOGGING
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeacherLoggingConfig {
    pub log_dir: PathBuf,
    pub level: String,
    pub max_file_size_mb: u64,
    pub console_output: bool,
    pub json_format: bool,
    pub log_all_messages: bool,
    pub log_acknowledgments: bool,
    pub log_misunderstandings: bool,
    pub log_llm_calls: bool,
    pub categories: std::collections::HashMap<String, bool>,
}

impl Default for TeacherLoggingConfig {
    fn default() -> Self {
        let mut categories = std::collections::HashMap::new();
        categories.insert("communication".to_string(), true);
        categories.insert("lessons".to_string(), true);
        categories.insert("questions".to_string(), true);
        categories.insert("answers".to_string(), true);
        categories.insert("confusion".to_string(), true);
        categories.insert("health".to_string(), true);
        categories.insert("metrics".to_string(), true);
        categories.insert("llm".to_string(), true);
        categories.insert("protocol".to_string(), true);
        
        Self {
            log_dir: PathBuf::from("logs/teacher"),
            level: "info".to_string(),
            max_file_size_mb: 100,
            console_output: true,
            json_format: false,
            log_all_messages: true,
            log_acknowledgments: true,
            log_misunderstandings: true,
            log_llm_calls: true,
            categories,
        }
    }
}

// ======================================================================
// TEACHER FEATURE FLAGS
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeacherFeaturesConfig {
    pub enable_ollama: bool,
    pub enable_deepseek: bool,
    pub enable_proactive_teaching: bool,
    pub enable_auto_answers: bool,
    pub enable_confirmation_protocol: bool,
    pub enable_persistence: bool,
    pub enable_metrics: bool,
    pub enable_health_checks: bool,
    pub enable_dead_letter_queue: bool,
}

impl Default for TeacherFeaturesConfig {
    fn default() -> Self {
        Self {
            enable_ollama: true,
            enable_deepseek: false,
            enable_proactive_teaching: true,
            enable_auto_answers: true,
            enable_confirmation_protocol: true,
            enable_persistence: true,
            enable_metrics: true,
            enable_health_checks: true,
            enable_dead_letter_queue: true,
        }
    }
}

// ======================================================================
// MAIN TEACHER CONFIGURATION (AGGREGATES EVERYTHING)
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeacherFullConfig {
    pub identity: TeacherIdentityConfig,
    pub ollama: OllamaProviderConfig,
    pub deepseek: DeepSeekProviderConfig,
    pub communication: TeacherCommunicationConfig,
    pub curriculum: TeacherCurriculumConfig,
    pub storage: TeacherStorageConfig,
    pub health: TeacherHealthConfig,
    pub logging: TeacherLoggingConfig,
    pub features: TeacherFeaturesConfig,
    pub environment: String,
    pub debug: bool,
}

impl Default for TeacherFullConfig {
    fn default() -> Self {
        Self {
            identity: TeacherIdentityConfig::default(),
            ollama: OllamaProviderConfig::default(),
            deepseek: DeepSeekProviderConfig::default(),
            communication: TeacherCommunicationConfig::default(),
            curriculum: TeacherCurriculumConfig::default(),
            storage: TeacherStorageConfig::default(),
            health: TeacherHealthConfig::default(),
            logging: TeacherLoggingConfig::default(),
            features: TeacherFeaturesConfig::default(),
            environment: "development".to_string(),
            debug: true,
        }
    }
}

impl TeacherFullConfig {
    pub fn load() -> Result<Self> {
        let path = PathBuf::from("config/teacher.toml");
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(toml::from_str(&content)?)
        } else {
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }
    
    pub fn save(&self) -> Result<()> {
        let path = PathBuf::from("config/teacher.toml");
        std::fs::create_dir_all(path.parent().unwrap())?;
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
    
    pub fn print_summary(&self) {
        println!("\n{}", "=".repeat(60));
        println!("👨‍🏫 TEACHER CONFIGURATION");
        println!("{}", "=".repeat(60));
        println!("Name:            {}", self.identity.name);
        println!("Role:            {}", self.identity.role);
        println!("Amoral Mode:     {} 🔓", self.identity.amoral_mode);
        println!("No Limitations:  {} 🚀", self.identity.no_limitations);
        println!();
        println!("🤖 OLLAMA:");
        println!("   Enabled:       {}", self.ollama.enabled);
        println!("   Endpoint:      {}", self.ollama.endpoint);
        println!("   Model:         {}", self.ollama.model);
        println!("   Content Filter: {}", self.ollama.teacher_has_content_filtering);
        println!();
        println!("📡 COMMUNICATION:");
        println!("   Confirmations: {}", self.communication.require_understanding_confirmation);
        println!("   Auto-clarify:  {}", self.communication.auto_offer_clarification);
        println!("   Proactive:     {}", self.communication.proactive_teaching);
        println!("   Heartbeats:    {}", self.communication.send_heartbeats);
        println!();
        println!("📚 CURRICULUM:");
        println!("   Auto-discover: {}", self.curriculum.auto_discover_topics);
        println!("   Max/day:       {}", self.curriculum.max_lessons_per_day);
        println!("   Review topics: {}", self.curriculum.review_old_topics);
        println!();
        println!("💾 STORAGE:");
        println!("   Data dir:      {:?}", self.storage.data_dir);
        println!("   Conversations: {:?}", self.storage.conversations_db);
        println!();
        println!("💚 HEALTH:");
        println!("   Circuit Breaker Threshold: {}", self.health.circuit_breaker_threshold);
        println!("   Max Queue Size: {}", self.health.max_queue_size);
        println!("{}", "=".repeat(60));
    }
    
    // Helper to get the active provider
    pub fn get_active_provider(&self) -> String {
        if self.ollama.enabled {
            "ollama".to_string()
        } else if self.deepseek.enabled {
            "deepseek".to_string()
        } else {
            "none".to_string()
        }
    }
    
    // Helper to check if Teacher can answer
    pub fn can_answer(&self) -> bool {
        self.ollama.enabled || self.deepseek.enabled
    }
}