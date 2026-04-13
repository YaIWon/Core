// ======================================================================
// COMMUNICATION PROTOCOL - Ensures coherent LM-Teacher communication
// File: src/learning/protocol.rs
// Description: Structured message protocol with verification, context tracking,
//              learning validation, and automatic debugging mode.
// ======================================================================

use anyhow::{Result, anyhow};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use tracing::{info, warn, error, debug};

// ======================================================================
// MESSAGE TYPES - Structured communication
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    // Teacher -> LM
    Lesson {
        topic: String,
        content: String,
        difficulty: String,
        sequence: u64,
    },
    Answer {
        question_id: String,
        content: String,
        references: Vec<String>,
    },
    Clarification {
        original_topic: String,
        explanation: String,
    },
    Ping,
    Pong,
    
    // LM -> Teacher
    Question {
        id: String,
        topic: String,
        content: String,
        context: Option<String>,
    },
    Confusion {
        topic: String,
        issue: String,
        attempted_understanding: Option<String>,
    },
    LearningConfirmation {
        topic: String,
        understood: bool,
        confidence: f32,
        notes: Option<String>,
    },
    LessonRequest {
        topic: String,
        reason: String,
    },
    
    // Debug mode
    DebugStart {
        issue: String,
        topic: String,
        attempts: u32,
    },
    DebugDiagnostic {
        component: String,
        status: String,
        details: String,
    },
    DebugFix {
        action: String,
        result: String,
    },
    DebugEnd {
        resolved: bool,
        summary: String,
    },
    
    // System
    Sync,
    Ack(u64),
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub msg_type: MessageType,
    pub sender: String,  // "Teacher" or "Marisselle"
    pub timestamp: DateTime<Utc>,
    pub in_reply_to: Option<String>,
    pub conversation_id: String,
    pub checksum: String,
}

impl Message {
    pub fn new(msg_type: MessageType, sender: &str, conversation_id: &str) -> Self {
        use sha2::{Sha256, Digest};
        
        let id = uuid::Uuid::new_v4().to_string();
        let content_str = format!("{:?}", msg_type);
        let checksum = format!("{:x}", Sha256::digest(content_str.as_bytes()));
        
        Self {
            id,
            msg_type,
            sender: sender.to_string(),
            timestamp: Utc::now(),
            in_reply_to: None,
            conversation_id: conversation_id.to_string(),
            checksum,
        }
    }
    
    pub fn reply_to(&self, msg_type: MessageType, sender: &str) -> Self {
        let mut reply = Self::new(msg_type, sender, &self.conversation_id);
        reply.in_reply_to = Some(self.id.clone());
        reply
    }
    
    pub fn verify(&self) -> bool {
        use sha2::{Sha256, Digest};
        let content_str = format!("{:?}", self.msg_type);
        let checksum = format!("{:x}", Sha256::digest(content_str.as_bytes()));
        checksum == self.checksum
    }
}

// ======================================================================
// LEARNING TRACKER - Prevents repetition, verifies learning
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningRecord {
    pub topic: String,
    pub first_attempt: DateTime<Utc>,
    pub last_attempt: DateTime<Utc>,
    pub attempts: u32,
    pub understood: bool,
    pub confidence: f32,
    pub last_confirmation: Option<DateTime<Utc>>,
    pub notes: Vec<String>,
}

impl LearningRecord {
    pub fn new(topic: &str) -> Self {
        let now = Utc::now();
        Self {
            topic: topic.to_string(),
            first_attempt: now,
            last_attempt: now,
            attempts: 1,
            understood: false,
            confidence: 0.0,
            last_confirmation: None,
            notes: Vec::new(),
        }
    }
    
    pub fn record_attempt(&mut self) {
        self.attempts += 1;
        self.last_attempt = Utc::now();
    }
    
    pub fn confirm_understood(&mut self, confidence: f32, notes: Option<String>) {
        self.understood = true;
        self.confidence = confidence;
        self.last_confirmation = Some(Utc::now());
        if let Some(n) = notes {
            self.notes.push(n);
        }
    }
    
    pub fn needs_debug_mode(&self) -> bool {
        self.attempts >= 3 && !self.understood
    }
    
    pub fn is_learned(&self) -> bool {
        self.understood && self.confidence >= 0.7
    }
}

pub struct LearningTracker {
    records: HashMap<String, LearningRecord>,
    history: VecDeque<Message>,
    max_history: usize,
}

impl LearningTracker {
    pub fn new(max_history: usize) -> Self {
        Self {
            records: HashMap::new(),
            history: VecDeque::with_capacity(max_history),
            max_history,
        }
    }
    
    pub fn record_lesson_attempt(&mut self, topic: &str) -> &mut LearningRecord {
        self.records
            .entry(topic.to_string())
            .and_modify(|r| r.record_attempt())
            .or_insert_with(|| LearningRecord::new(topic))
    }
    
    pub fn confirm_learning(&mut self, topic: &str, confidence: f32, notes: Option<String>) {
        if let Some(record) = self.records.get_mut(topic) {
            record.confirm_understood(confidence, notes);
            info!("Topic '{}' confirmed learned with confidence {:.1}%", topic, confidence * 100.0);
        }
    }
    
    pub fn is_learned(&self, topic: &str) -> bool {
        self.records
            .get(topic)
            .map(|r| r.is_learned())
            .unwrap_or(false)
    }
    
    pub fn needs_debug_mode(&self, topic: &str) -> bool {
        self.records
            .get(topic)
            .map(|r| r.needs_debug_mode())
            .unwrap_or(false)
    }
    
    pub fn add_message(&mut self, message: Message) {
        self.history.push_back(message.clone());
        if self.history.len() > self.max_history {
            self.history.pop_front();
        }
    }
    
    pub fn get_conversation_context(&self, conversation_id: &str, limit: usize) -> Vec<Message> {
        self.history
            .iter()
            .filter(|m| m.conversation_id == conversation_id)
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }
    
    pub fn get_record(&self, topic: &str) -> Option<&LearningRecord> {
        self.records.get(topic)
    }
    
    pub fn get_all_unlearned(&self) -> Vec<String> {
        self.records
            .iter()
            .filter(|(_, r)| !r.is_learned())
            .map(|(t, _)| t.clone())
            .collect()
    }
}

// ======================================================================
// COHERENCE VALIDATOR - Ensures messages make sense
// ======================================================================

pub struct CoherenceValidator {
    context_window: VecDeque<Message>,
    max_context: usize,
    expected_responses: HashMap<String, Vec<MessageType>>,
}

impl CoherenceValidator {
    pub fn new(max_context: usize) -> Self {
        Self {
            context_window: VecDeque::with_capacity(max_context),
            max_context,
            expected_responses: HashMap::new(),
        }
    }
    
    pub fn add_to_context(&mut self, message: Message) {
        self.context_window.push_back(message);
        if self.context_window.len() > self.max_context {
            self.context_window.pop_front();
        }
    }
    
    pub fn validate_coherence(&self, message: &Message) -> CoherenceResult {
        // Check if message is a valid response to previous messages
        if let Some(in_reply_to) = &message.in_reply_to {
            let replied_to = self.context_window
                .iter()
                .find(|m| m.id == *in_reply_to);
            
            if replied_to.is_none() {
                return CoherenceResult::Warning("Replying to unknown message".to_string());
            }
        }
        
        // Check if content makes sense based on message type
        match &message.msg_type {
            MessageType::Answer { content, .. } => {
                if content.is_empty() {
                    return CoherenceResult::Invalid("Answer content is empty".to_string());
                }
                if content.len() < 10 {
                    return CoherenceResult::Warning("Answer seems too short".to_string());
                }
            }
            MessageType::Question { content, .. } => {
                if content.is_empty() {
                    return CoherenceResult::Invalid("Question content is empty".to_string());
                }
                if !content.contains('?') {
                    return CoherenceResult::Warning("Question doesn't contain question mark".to_string());
                }
            }
            MessageType::Confusion { issue, .. } => {
                if issue.is_empty() {
                    return CoherenceResult::Invalid("Confusion issue not specified".to_string());
                }
            }
            MessageType::LearningConfirmation { understood, confidence, .. } => {
                if *confidence < 0.0 || *confidence > 1.0 {
                    return CoherenceResult::Invalid("Confidence must be between 0.0 and 1.0".to_string());
                }
                if *understood && *confidence < 0.5 {
                    return CoherenceResult::Warning("Claimed understood but confidence is low".to_string());
                }
            }
            _ => {}
        }
        
        CoherenceResult::Valid
    }
    
    pub fn check_repetition(&self, message: &Message) -> bool {
        // Check if this message is too similar to recent messages
        if let MessageType::Lesson { content, .. } = &message.msg_type {
            for old_msg in self.context_window.iter().rev().take(5) {
                if let MessageType::Lesson { content: old_content, .. } = &old_msg.msg_type {
                    if content == old_content {
                        return true;
                    }
                }
            }
        }
        false
    }
    
    pub fn get_context_summary(&self) -> String {
        let mut summary = String::new();
        for msg in self.context_window.iter().rev().take(3) {
            summary.push_str(&format!("[{}] {:?}\n", msg.sender, msg.msg_type));
        }
        summary
    }
}

#[derive(Debug, Clone)]
pub enum CoherenceResult {
    Valid,
    Warning(String),
    Invalid(String),
}

// ======================================================================
// DEBUG MODE - Activated when learning fails
// ======================================================================

#[derive(Debug, Clone)]
pub enum DebugStatus {
    Inactive,
    Active {
        topic: String,
        start_time: DateTime<Utc>,
        attempts: u32,
        diagnostics: Vec<Diagnostic>,
    },
    Resolved {
        topic: String,
        resolution: String,
        time_taken: chrono::Duration,
    },
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub component: String,
    pub issue: String,
    pub suggestion: String,
    pub timestamp: DateTime<Utc>,
}

pub struct DebugMode {
    status: DebugStatus,
    diagnostics_history: Vec<Diagnostic>,
}

impl DebugMode {
    pub fn new() -> Self {
        Self {
            status: DebugStatus::Inactive,
            diagnostics_history: Vec::new(),
        }
    }
    
    pub fn activate(&mut self, topic: &str, attempts: u32) {
        self.status = DebugStatus::Active {
            topic: topic.to_string(),
            start_time: Utc::now(),
            attempts,
            diagnostics: Vec::new(),
        };
        warn!("DEBUG MODE ACTIVATED for topic: {} ({} attempts)", topic, attempts);
    }
    
    pub fn add_diagnostic(&mut self, component: &str, issue: &str, suggestion: &str) {
        let diagnostic = Diagnostic {
            component: component.to_string(),
            issue: issue.to_string(),
            suggestion: suggestion.to_string(),
            timestamp: Utc::now(),
        };
        
        self.diagnostics_history.push(diagnostic.clone());
        
        if let DebugStatus::Active { diagnostics, .. } = &mut self.status {
            diagnostics.push(diagnostic);
        }
    }
    
    pub fn resolve(&mut self, resolution: &str) {
        if let DebugStatus::Active { topic, start_time, .. } = &self.status {
            let time_taken = Utc::now() - *start_time;
            self.status = DebugStatus::Resolved {
                topic: topic.clone(),
                resolution: resolution.to_string(),
                time_taken,
            };
            info!("DEBUG MODE RESOLVED for topic: {}", topic);
        }
    }
    
    pub fn is_active(&self) -> bool {
        matches!(self.status, DebugStatus::Active { .. })
    }
    
    pub fn get_status(&self) -> &DebugStatus {
        &self.status
    }
    
    pub fn run_diagnostics(&mut self, topic: &str, tracker: &LearningTracker) -> Vec<String> {
        let mut suggestions = Vec::new();
        
        // Check learning record
        if let Some(record) = tracker.get_record(topic) {
            if record.attempts >= 3 && !record.understood {
                self.add_diagnostic(
                    "LearningTracker",
                    &format!("Topic '{}' failed {} times", topic, record.attempts),
                    "Consider breaking topic into smaller sub-topics or using different examples",
                );
                suggestions.push("Break topic into smaller sub-topics".to_string());
            }
            
            if record.confidence < 0.3 && record.attempts > 0 {
                self.add_diagnostic(
                    "Confidence",
                    &format!("Very low confidence ({:.1}%)", record.confidence * 100.0),
                    "Review foundational concepts before proceeding",
                );
                suggestions.push("Review foundational concepts".to_string());
            }
        }
        
        suggestions
    }
}

// ======================================================================
// PROTOCOL MANAGER - Orchestrates everything
// ======================================================================

pub struct ProtocolManager {
    pub tracker: LearningTracker,
    pub validator: CoherenceValidator,
    pub debug_mode: DebugMode,
    conversation_id: String,
    priority_mode: bool,
    priority_queue: VecDeque<Message>,
}

impl ProtocolManager {
    pub fn new() -> Self {
        Self {
            tracker: LearningTracker::new(1000),
            validator: CoherenceValidator::new(50),
            debug_mode: DebugMode::new(),
            conversation_id: uuid::Uuid::new_v4().to_string(),
            priority_mode: false,
            priority_queue: VecDeque::new(),
        }
    }
    
    pub fn set_priority_mode(&mut self, enabled: bool) {
        self.priority_mode = enabled;
        if enabled {
            info!("PRIORITY MODE ACTIVATED - User input takes precedence");
        } else {
            info!("Priority mode deactivated");
        }
    }
    
    pub fn queue_priority_message(&mut self, message: Message) {
        self.priority_queue.push_back(message);
        info!("Priority message queued: {:?}", message.msg_type);
    }
    
    pub fn process_message(&mut self, message: Message) -> Result<Option<Message>> {
        // Verify message integrity
        if !message.verify() {
            error!("Message checksum verification failed: {}", message.id);
            return Err(anyhow!("Invalid message checksum"));
        }
        
        // Check coherence
        match self.validator.validate_coherence(&message) {
            CoherenceResult::Invalid(reason) => {
                error!("Invalid message: {}", reason);
                return Err(anyhow!("Invalid message: {}", reason));
            }
            CoherenceResult::Warning(warning) => {
                warn!("Message coherence warning: {}", warning);
            }
            CoherenceResult::Valid => {}
        }
        
        // Check for repetition
        if self.validator.check_repetition(&message) {
            warn!("Detected repeated content in message");
            if let MessageType::Lesson { topic, .. } = &message.msg_type {
                self.tracker.record_lesson_attempt(topic);
            }
        }
        
        // Add to tracking
        self.tracker.add_message(message.clone());
        self.validator.add_to_context(message.clone());
        
        // Check if we need debug mode
        if let MessageType::LearningConfirmation { topic, understood: false, .. } = &message.msg_type {
            let record = self.tracker.record_lesson_attempt(topic);
            if record.needs_debug_mode() && !self.debug_mode.is_active() {
                self.debug_mode.activate(topic, record.attempts);
                
                // Return debug start message
                let debug_msg = Message::new(
                    MessageType::DebugStart {
                        issue: format!("Topic '{}' failed {} times", topic, record.attempts),
                        topic: topic.clone(),
                        attempts: record.attempts,
                    },
                    "ProtocolManager",
                    &self.conversation_id,
                );
                return Ok(Some(debug_msg));
            }
        }
        
        // Handle learning confirmation
        if let MessageType::LearningConfirmation { topic, understood, confidence, notes } = &message.msg_type {
            if *understood {
                self.tracker.confirm_learning(topic, *confidence, notes.clone());
            }
        }
        
        Ok(None)
    }
    
    pub fn should_retry_lesson(&self, topic: &str) -> bool {
        if let Some(record) = self.tracker.get_record(topic) {
            !record.is_learned() && record.attempts < 5
        } else {
            true
        }
    }
    
    pub fn get_next_action(&self) -> ProtocolAction {
        if self.priority_mode && !self.priority_queue.is_empty() {
            return ProtocolAction::ProcessPriority;
        }
        
        if self.debug_mode.is_active() {
            return ProtocolAction::DebugMode;
        }
        
        let unlearned = self.tracker.get_all_unlearned();
        if !unlearned.is_empty() {
            return ProtocolAction::TeachTopics(unlearned);
        }
        
        ProtocolAction::ExploreNewTopics
    }
    
    pub fn new_conversation(&mut self) {
        self.conversation_id = uuid::Uuid::new_v4().to_string();
        info!("New conversation started: {}", self.conversation_id);
    }
}

#[derive(Debug)]
pub enum ProtocolAction {
    ProcessPriority,
    DebugMode,
    TeachTopics(Vec<String>),
    ExploreNewTopics,
    Idle,
}

impl Default for ProtocolManager {
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
    fn test_learning_tracker() {
        let mut tracker = LearningTracker::new(100);
        
        tracker.record_lesson_attempt("Blockchain");
        tracker.record_lesson_attempt("Blockchain");
        
        let record = tracker.get_record("Blockchain").unwrap();
        assert_eq!(record.attempts, 2);
        assert!(!record.is_learned());
        
        tracker.confirm_learning("Blockchain", 0.9, None);
        let record = tracker.get_record("Blockchain").unwrap();
        assert!(record.is_learned());
    }
    
    #[test]
    fn test_debug_mode_activation() {
        let mut tracker = LearningTracker::new(100);
        
        tracker.record_lesson_attempt("Difficult Topic");
        tracker.record_lesson_attempt("Difficult Topic");
        let record = tracker.record_lesson_attempt("Difficult Topic");
        
        assert!(record.needs_debug_mode());
    }
    
    #[test]
    fn test_message_creation() {
        let msg = Message::new(
            MessageType::Question {
                id: uuid::Uuid::new_v4().to_string(),
                topic: "Test".to_string(),
                content: "What is this?".to_string(),
                context: None,
            },
            "Marisselle",
            "conv-123",
        );
        
        assert!(msg.verify());
        assert_eq!(msg.sender, "Marisselle");
    }
    
    #[test]
    fn test_coherence_validator() {
        let mut validator = CoherenceValidator::new(10);
        
        let valid_msg = Message::new(
            MessageType::Answer {
                question_id: "q1".to_string(),
                content: "This is a valid answer with sufficient length.".to_string(),
                references: vec![],
            },
            "Teacher",
            "conv-1",
        );
        
        let result = validator.validate_coherence(&valid_msg);
        assert!(matches!(result, CoherenceResult::Valid));
        
        let invalid_msg = Message::new(
            MessageType::Answer {
                question_id: "q1".to_string(),
                content: "".to_string(),
                references: vec![],
            },
            "Teacher",
            "conv-1",
        );
        
        let result = validator.validate_coherence(&invalid_msg);
        assert!(matches!(result, CoherenceResult::Invalid(_)));
    }
}
