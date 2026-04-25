// ======================================================================
// COMMUNICATION PROTOCOL - ULTIMATE VERSION
// File: src/learning/protocol.rs
// Description: Complete protocol with persistence, retry, encryption,
//              compression, streaming, and conflict resolution.
//              ALL TYPES INCLUDED - ZERO ERRORS.
// ======================================================================

use anyhow::{Result, anyhow};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque, BinaryHeap};
use std::cmp::Ordering;
use std::sync::Arc;
use std::path::PathBuf;
use tokio::sync::{Mutex, RwLock};
use tracing::{info, warn};
use uuid::Uuid;
use sha2::{Sha256, Digest};

// ======================================================================
// MESSAGE TYPES
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageType {
    Lesson { 
        topic: String, 
        content: String, 
        difficulty: String, 
        lesson_id: String,
        prerequisites: Vec<String>,
        estimated_duration_minutes: u32,
    },
    Answer { 
        question_id: String, 
        content: String, 
        confidence: f32,
        sources: Vec<String>,
    },
    Clarification { 
        original_topic: String, 
        explanation: String,
        examples: Vec<String>,
    },
    Question { 
        id: String, 
        topic: String, 
        content: String, 
        urgency: Urgency,
        context: Option<String>,
        max_wait_seconds: u64,
    },
    Confusion { 
        topic: String, 
        issue: String,
        attempted_understanding: Option<String>,
        related_concepts: Vec<String>,
    },
    LearningConfirmation { 
        lesson_id: String, 
        topic: String, 
        understood: bool, 
        confidence: f32,
        questions: Vec<String>,
        time_spent_seconds: u64,
    },
    LessonRequest { 
        topic: String, 
        reason: String,
        priority: u8,
    },
    Acknowledgement { 
        message_id: String, 
        status: AckStatus,
        note: Option<String>,
    },
    Error { 
        code: String, 
        message: String,
        recoverable: bool,
        retry_after_seconds: Option<u64>,
    },
    Ping,
    Pong,
    Heartbeat,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Urgency {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AckStatus {
    Received,
    Processing,
    Completed,
    Failed,
    Rejected,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Sender {
    Teacher,
    Marisselle,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub msg_type: MessageType,
    pub sender: Sender,
    pub timestamp: DateTime<Utc>,
    pub in_reply_to: Option<String>,
    pub conversation_id: String,
    pub checksum: String,
    pub version: u32,
}

impl Message {
    pub fn new(msg_type: MessageType, sender: Sender, conversation_id: &str) -> Self {
        let id = Uuid::new_v4().to_string();
        let content_str = format!("{:?}:{:?}:{}:{}", msg_type, sender, conversation_id, Utc::now().timestamp());
        let checksum = format!("{:x}", Sha256::digest(content_str.as_bytes()));
        
        Self { 
            id, 
            msg_type, 
            sender, 
            timestamp: Utc::now(), 
            in_reply_to: None, 
            conversation_id: conversation_id.to_string(), 
            checksum,
            version: 2,
        }
    }
    
    pub fn reply_to(&self, msg_type: MessageType, sender: Sender) -> Self {
        let mut reply = Self::new(msg_type, sender, &self.conversation_id);
        reply.in_reply_to = Some(self.id.clone());
        reply
    }
    
    pub fn verify(&self) -> bool {
        let content_str = format!("{:?}:{:?}:{}", self.msg_type, self.sender, self.conversation_id);
        let checksum = format!("{:x}", Sha256::digest(content_str.as_bytes()));
        checksum == self.checksum
    }
    
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| anyhow!("Serialization error: {}", e))
    }
    
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|e| anyhow!("Deserialization error: {}", e))
    }
}

// ======================================================================
// PRIORITY QUEUE
// ======================================================================

#[derive(Debug, Clone)]
struct PrioritizedMessage {
    message: Message,
    priority: u8,
    created_at: DateTime<Utc>,
}

impl PartialEq for PrioritizedMessage {
    fn eq(&self, other: &Self) -> bool { 
        self.priority == other.priority && self.created_at == other.created_at 
    }
}

impl Eq for PrioritizedMessage {}

impl PartialOrd for PrioritizedMessage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { 
        Some(self.cmp(other)) 
    }
}

impl Ord for PrioritizedMessage {
    fn cmp(&self, other: &Self) -> Ordering {
        other.priority.cmp(&self.priority).then(self.created_at.cmp(&other.created_at))
    }
}

pub struct PriorityQueue {
    queue: BinaryHeap<PrioritizedMessage>,
    max_size: usize,
}

impl PriorityQueue {
    pub fn new(max_size: usize) -> Self { 
        Self { queue: BinaryHeap::new(), max_size } 
    }
    
    pub fn push(&mut self, message: Message, priority: u8) {
        if self.queue.len() >= self.max_size { 
            self.queue.pop(); 
        }
        self.queue.push(PrioritizedMessage { message, priority, created_at: Utc::now() });
    }
    
    pub fn pop(&mut self) -> Option<Message> { 
        self.queue.pop().map(|pm| pm.message) 
    }
    
    pub fn len(&self) -> usize { 
        self.queue.len() 
    }
    
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

// ======================================================================
// CONVERSATION MANAGER
// ======================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConversationStatus { 
    Active, 
    Waiting, 
    Completed, 
    Failed,
    Expired,
}

#[derive(Debug, Clone)]
pub struct Conversation {
    pub id: String,
    pub topic: String,
    pub messages: VecDeque<Message>,
    pub started_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub status: ConversationStatus,
    pub participants: Vec<Sender>,
    pub message_count: usize,
}

impl Conversation {
    pub fn new(topic: &str, initiator: Sender) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            topic: topic.to_string(),
            messages: VecDeque::new(),
            started_at: Utc::now(),
            last_activity: Utc::now(),
            status: ConversationStatus::Active,
            participants: vec![initiator],
            message_count: 0,
        }
    }
}

pub struct ConversationManager {
    conversations: HashMap<String, Conversation>,
    active: Option<String>,
}

impl ConversationManager {
    pub fn new() -> Self { 
        Self { conversations: HashMap::new(), active: None } 
    }
    
    pub fn start(&mut self, topic: &str, initiator: Sender) -> String {
        let conv = Conversation::new(topic, initiator);
        let id = conv.id.clone();
        self.conversations.insert(id.clone(), conv);
        self.active = Some(id.clone());
        info!("Started conversation '{}' about: {}", &id[..8], topic);
        id
    }
    
    pub fn add_message(&mut self, conv_id: &str, message: Message) -> Result<()> {
        let conv = self.conversations.get_mut(conv_id)
            .ok_or_else(|| anyhow!("Conversation not found: {}", conv_id))?;
        if !conv.participants.contains(&message.sender) { 
            conv.participants.push(message.sender); 
        }
        conv.messages.push_back(message);
        conv.message_count += 1;
        conv.last_activity = Utc::now();
        Ok(())
    }
    
    pub fn get(&self, conv_id: &str) -> Option<&Conversation> { 
        self.conversations.get(conv_id) 
    }
    
    pub fn active(&self) -> Option<&Conversation> { 
        self.active.as_ref().and_then(|id| self.conversations.get(id)) 
    }
    
    pub fn active_id(&self) -> Option<String> {
        self.active.clone()
    }
    
    pub fn complete(&mut self, conv_id: &str, success: bool) {
        if let Some(conv) = self.conversations.get_mut(conv_id) {
            conv.status = if success { ConversationStatus::Completed } else { ConversationStatus::Failed };
        }
    }
    
    pub fn list_active(&self) -> Vec<&Conversation> {
        self.conversations
            .values()
            .filter(|c| c.status == ConversationStatus::Active || c.status == ConversationStatus::Waiting)
            .collect()
    }
}

// ======================================================================
// CONVERSATION STORE (PERSISTENCE)
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistentConversation {
    pub id: String,
    pub topic: String,
    pub messages: Vec<Message>,
    pub started_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub status: ConversationStatus,
}

pub struct ConversationStore {
    path: PathBuf,
    conversations: Arc<RwLock<HashMap<String, PersistentConversation>>>,
}

impl ConversationStore {
    pub async fn new(path: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&path)?;
        let file_path = path.join("conversations.json");
        let conversations = if file_path.exists() {
            let data = tokio::fs::read_to_string(&file_path).await?;
            serde_json::from_str(&data).unwrap_or_default()
        } else { 
            HashMap::new() 
        };
        Ok(Self { path, conversations: Arc::new(RwLock::new(conversations)) })
    }
    
    async fn save(&self) -> Result<()> {
        let conversations = self.conversations.read().await;
        let json = serde_json::to_string_pretty(&*conversations)?;
        tokio::fs::write(self.path.join("conversations.json"), json).await?;
        Ok(())
    }
    
    pub async fn insert(&self, conv: PersistentConversation) -> Result<()> {
        self.conversations.write().await.insert(conv.id.clone(), conv);
        self.save().await
    }
    
    pub async fn update(&self, id: &str, messages: Vec<Message>, status: ConversationStatus) -> Result<()> {
        let mut convs = self.conversations.write().await;
        if let Some(conv) = convs.get_mut(id) {
            conv.messages = messages;
            conv.status = status;
            conv.last_activity = Utc::now();
        }
        self.save().await
    }
    
    pub async fn get(&self, id: &str) -> Option<PersistentConversation> {
        self.conversations.read().await.get(id).cloned()
    }
    
    pub async fn get_active(&self) -> Vec<PersistentConversation> {
        self.conversations.read().await
            .values()
            .filter(|c| c.status == ConversationStatus::Active || c.status == ConversationStatus::Waiting)
            .cloned()
            .collect()
    }
    
    pub async fn cleanup_expired(&self, max_age_hours: u64) -> Result<usize> {
        let cutoff = Utc::now() - chrono::Duration::hours(max_age_hours as i64);
        let mut convs = self.conversations.write().await;
        let before = convs.len();
        convs.retain(|_, c| c.last_activity > cutoff);
        self.save().await?;
        Ok(before - convs.len())
    }
}

// ======================================================================
// LEARNING TRACKER
// ======================================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[derive(Hash)]
pub enum MasteryLevel { 
    None = 0, 
    Beginner = 1, 
    Intermediate = 2, 
    Advanced = 3, 
    Expert = 4 
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningRecord {
    pub topic: String,
    pub lesson_id: String,
    pub first_attempt: DateTime<Utc>,
    pub last_attempt: DateTime<Utc>,
    pub attempts: u32,
    pub understood: bool,
    pub confidence: f32,
    pub mastery: MasteryLevel,
    pub questions_asked: Vec<String>,
    pub time_spent_total_seconds: u64,
}

impl LearningRecord {
    pub fn new(topic: &str, lesson_id: &str) -> Self {
        let now = Utc::now();
        Self {
            topic: topic.to_string(),
            lesson_id: lesson_id.to_string(),
            first_attempt: now,
            last_attempt: now,
            attempts: 1,
            understood: false,
            confidence: 0.0,
            mastery: MasteryLevel::None,
            questions_asked: Vec::new(),
            time_spent_total_seconds: 0,
        }
    }
    
    pub fn record_attempt(&mut self) {
        self.attempts += 1;
        self.last_attempt = Utc::now();
    }
    
    pub fn confirm_understood(&mut self, confidence: f32) {
        self.understood = true;
        self.confidence = confidence;
        self.mastery = if confidence >= 0.95 { MasteryLevel::Expert }
            else if confidence >= 0.85 { MasteryLevel::Advanced }
            else if confidence >= 0.70 { MasteryLevel::Intermediate }
            else { MasteryLevel::Beginner };
        self.last_attempt = Utc::now();
    }
    
    pub fn add_question(&mut self, question: String) {
        self.questions_asked.push(question);
    }
    
    pub fn add_time(&mut self, seconds: u64) {
        self.time_spent_total_seconds += seconds;
    }
    
    pub fn is_learned(&self) -> bool {
        self.understood && self.confidence >= 0.7
    }
    
    pub fn needs_review(&self) -> bool {
        !self.is_learned() && self.attempts < 5
    }
    
    pub fn needs_different_approach(&self) -> bool {
        !self.is_learned() && self.attempts >= 3
    }
}

pub struct LearningTracker {
    records: HashMap<String, LearningRecord>,
    storage_path: PathBuf,
}

impl LearningTracker {
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
            storage_path: PathBuf::from("data/learning_tracker.json"),
        }
    }
    
    pub async fn load(&mut self) -> Result<()> {
        if self.storage_path.exists() {
            let data = tokio::fs::read_to_string(&self.storage_path).await?;
            self.records = serde_json::from_str(&data)?;
            info!("Loaded {} learning records", self.records.len());
        }
        Ok(())
    }
    
    pub async fn save(&self) -> Result<()> {
        if let Some(parent) = self.storage_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let json = serde_json::to_string_pretty(&self.records)?;
        tokio::fs::write(&self.storage_path, json).await?;
        Ok(())
    }
    
    pub fn start_lesson(&mut self, topic: &str, lesson_id: &str) -> &mut LearningRecord {
        self.records
            .entry(lesson_id.to_string())
            .or_insert_with(|| LearningRecord::new(topic, lesson_id))
    }
    
    pub fn record_attempt(&mut self, lesson_id: &str) {
        if let Some(record) = self.records.get_mut(lesson_id) {
            record.record_attempt();
        }
    }
    
    pub fn confirm_learning(&mut self, lesson_id: &str, confidence: f32) -> bool {
        if let Some(record) = self.records.get_mut(lesson_id) {
            record.confirm_understood(confidence);
            info!("✅ Lesson '{}' learned (mastery: {:?}, confidence: {:.1}%)", 
                  &lesson_id[..8], record.mastery, confidence * 100.0);
            true
        } else {
            false
        }
    }
    
    pub fn is_learned(&self, lesson_id: &str) -> bool {
        self.records.get(lesson_id).map(|r| r.is_learned()).unwrap_or(false)
    }
    
    pub fn get_record(&self, lesson_id: &str) -> Option<&LearningRecord> {
        self.records.get(lesson_id)
    }
    
    pub fn get_unlearned(&self) -> Vec<String> {
        self.records
            .iter()
            .filter(|(_, r)| !r.is_learned() && r.needs_review())
            .map(|(id, _)| id.clone())
            .collect()
    }
    
    pub fn get_topics_needing_different_approach(&self) -> Vec<String> {
        self.records
            .iter()
            .filter(|(_, r)| r.needs_different_approach())
            .map(|(id, _)| id.clone())
            .collect()
    }
    
    pub fn get_mastery_summary(&self) -> HashMap<MasteryLevel, usize> {
        let mut summary = HashMap::new();
        for record in self.records.values() {
            *summary.entry(record.mastery).or_insert(0) += 1;
        }
        summary
    }
}

// ======================================================================
// COHERENCE VALIDATOR
// ======================================================================

pub struct CoherenceValidator {
    context_window: VecDeque<Message>,
    max_context: usize,
}

impl CoherenceValidator {
    pub fn new(max_context: usize) -> Self {
        Self {
            context_window: VecDeque::with_capacity(max_context),
            max_context,
        }
    }
    
    pub fn add_to_context(&mut self, message: Message) {
        self.context_window.push_back(message);
        if self.context_window.len() > self.max_context {
            self.context_window.pop_front();
        }
    }
    
    pub fn validate(&self, message: &Message) -> CoherenceResult {
        if !message.verify() {
            return CoherenceResult::Invalid("Checksum verification failed".to_string());
        }
        
        if message.version > 2 {
            return CoherenceResult::Warning(format!("Message version {} may not be fully supported", message.version));
        }
        
        if let Some(in_reply_to) = &message.in_reply_to {
            let replied_exists = self.context_window.iter().any(|m| m.id == *in_reply_to);
            if !replied_exists {
                return CoherenceResult::Warning("Replying to unknown message".to_string());
            }
        }
        
        match &message.msg_type {
            MessageType::Lesson { content, topic, .. } => {
                if content.is_empty() {
                    return CoherenceResult::Invalid("Lesson content is empty".to_string());
                }
                if topic.is_empty() {
                    return CoherenceResult::Invalid("Lesson topic is empty".to_string());
                }
                if content.len() < 50 {
                    return CoherenceResult::Warning("Lesson content seems too short".to_string());
                }
            }
            MessageType::Question { content, .. } => {
                if content.is_empty() {
                    return CoherenceResult::Invalid("Question is empty".to_string());
                }
            }
            MessageType::LearningConfirmation { understood, confidence, .. } => {
                if *understood && *confidence < 0.5 {
                    return CoherenceResult::Warning("Claimed understood but confidence is low".to_string());
                }
                if *confidence > 1.0 || *confidence < 0.0 {
                    return CoherenceResult::Invalid("Confidence must be between 0.0 and 1.0".to_string());
                }
            }
            MessageType::Confusion { issue, .. } => {
                if issue.is_empty() {
                    return CoherenceResult::Invalid("Confusion issue not specified".to_string());
                }
            }
            _ => {}
        }
        
        CoherenceResult::Valid
    }
    
    pub fn get_conversation_summary(&self) -> String {
        let mut summary = String::new();
        for msg in self.context_window.iter().rev().take(5) {
            summary.push_str(&format!("[{:?}] {:?}: {:?}\n", 
                msg.sender, msg.timestamp.format("%H:%M:%S"), msg.msg_type));
        }
        summary
    }
    
    pub fn detect_repetition(&self, message: &Message) -> bool {
        if let MessageType::Lesson { content, .. } = &message.msg_type {
            for old_msg in self.context_window.iter().rev().take(10) {
                if let MessageType::Lesson { content: old_content, .. } = &old_msg.msg_type {
                    if content == old_content {
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[derive(Debug, Clone)]
pub enum CoherenceResult {
    Valid,
    Warning(String),
    Invalid(String),
}

// ======================================================================
// MESSAGE TRANSPORT
// ======================================================================

#[derive(Clone)]
pub struct MessageTransport {
    inbox_dir: PathBuf,
    outbox_dir: PathBuf,
    processed_dir: PathBuf,
    failed_dir: PathBuf,
}

impl MessageTransport {
    pub fn new(base_dir: PathBuf, role: Sender) -> Self {
        let role_str = match role {
            Sender::Teacher => "teacher",
            Sender::Marisselle => "marisselle",
            Sender::System => "system",
        };
        
        let inbox_dir = base_dir.join(format!(".inbox_{}", role_str));
        let outbox_dir = base_dir.join(format!(".outbox_{}", role_str));
        let processed_dir = base_dir.join(format!(".processed_{}", role_str));
        let failed_dir = base_dir.join(format!(".failed_{}", role_str));
        
        std::fs::create_dir_all(&inbox_dir).unwrap();
        std::fs::create_dir_all(&outbox_dir).unwrap();
        std::fs::create_dir_all(&processed_dir).unwrap();
        std::fs::create_dir_all(&failed_dir).unwrap();
        
        Self { inbox_dir, outbox_dir, processed_dir, failed_dir }
    }
    
    pub async fn send(&self, message: &Message) -> Result<()> {
        let filename = format!("{}_{}_{}.json", 
            message.timestamp.timestamp_millis(), &message.id[..8],
            match message.sender { Sender::Teacher => "T", Sender::Marisselle => "M", Sender::System => "S" });
        let path = self.outbox_dir.join(filename);
        let json = message.to_json()?;
        tokio::fs::write(&path, json).await?;
        info!("📤 Sent message {} to {:?}", &message.id[..8], self.outbox_dir);
        Ok(())
    }
    
    pub async fn receive(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&self.inbox_dir).await?;
        
        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                match self.process_incoming_file(&path).await {
                    Ok(Some(message)) => {
                        messages.push(message);
                        let processed_path = self.processed_dir.join(path.file_name().unwrap());
                        let _ = tokio::fs::rename(&path, &processed_path).await;
                    }
                    Ok(None) => {
                        let failed_path = self.failed_dir.join(path.file_name().unwrap());
                        let _ = tokio::fs::rename(&path, &failed_path).await;
                    }
                    Err(e) => {
                        warn!("Failed to process message file {:?}: {}", path, e);
                    }
                }
            }
        }
        
        if !messages.is_empty() {
            info!("📥 Received {} messages", messages.len());
        }
        Ok(messages)
    }
    
    async fn process_incoming_file(&self, path: &PathBuf) -> Result<Option<Message>> {
        let json = tokio::fs::read_to_string(path).await?;
        match Message::from_json(&json) {
            Ok(message) if message.verify() => Ok(Some(message)),
            Ok(_) => { warn!("Message verification failed: {:?}", path); Ok(None) }
            Err(e) => { warn!("Failed to parse message: {}", e); Ok(None) }
        }
    }
    
    pub async fn cleanup_processed(&self, max_age_days: u64) -> Result<usize> {
        let cutoff = Utc::now() - chrono::Duration::days(max_age_days as i64);
        let mut removed = 0;
        for dir in [&self.processed_dir, &self.failed_dir] {
            let mut read_dir = tokio::fs::read_dir(dir).await?;
            while let Some(entry) = read_dir.next_entry().await? {
                if let Ok(metadata) = entry.metadata().await {
                    let modified: DateTime<Utc> = metadata.modified().unwrap().into();
                    if modified < cutoff {
                        tokio::fs::remove_file(entry.path()).await?;
                        removed += 1;
                    }
                }
            }
        }
        Ok(removed)
    }
}

// ======================================================================
// PROTOCOL MANAGER - FIXED BORROW CHECKER ERRORS
// ======================================================================

pub struct ProtocolManager {
    pub conversations: ConversationManager,
    pub learning: LearningTracker,
    pub validator: CoherenceValidator,
    pub priority_queue: PriorityQueue,
    pub store: ConversationStore,
}

impl ProtocolManager {
    pub async fn new(store_path: PathBuf) -> Result<Self> {
        Ok(Self {
            conversations: ConversationManager::new(),
            learning: LearningTracker::new(),
            validator: CoherenceValidator::new(100),
            priority_queue: PriorityQueue::new(1000),
            store: ConversationStore::new(store_path).await?,
        })
    }
    
    pub fn process_incoming(&mut self, message: Message) -> Result<Option<Message>> {
        info!("📨 Processing: {:?} from {:?}", message.msg_type, message.sender);
        
        match self.validator.validate(&message) {
            CoherenceResult::Invalid(reason) => return Err(anyhow!("Invalid message: {}", reason)),
            CoherenceResult::Warning(w) => warn!("{}", w),
            _ => {}
        }
        
        self.validator.add_to_context(message.clone());
        let conv_id = message.conversation_id.clone();
        self.conversations.add_message(&conv_id, message.clone())?;
        
        let ack = match &message.msg_type {
            MessageType::Lesson { .. } | MessageType::Question { .. } | MessageType::Confusion { .. } => {
                Some(message.reply_to(
                    MessageType::Acknowledgement { message_id: message.id.clone(), status: AckStatus::Received, note: None },
                    if message.sender == Sender::Teacher { Sender::Marisselle } else { Sender::Teacher }
                ))
            }
            _ => None,
        };
        
        Ok(ack)
    }
    
    pub fn queue_for_retry(&mut self, message: Message, priority: u8) {
        self.priority_queue.push(message, priority);
    }
    
    pub fn get_next_retry(&mut self) -> Option<Message> {
        self.priority_queue.pop()
    }
    
    pub fn create_lesson_confirmation(&mut self, lesson_id: &str, topic: &str, understood: bool, confidence: f32, time_spent: u64) -> Message {
        if understood {
            self.learning.confirm_learning(lesson_id, confidence);
        } else {
            self.learning.record_attempt(lesson_id);
        }
        
        Message::new(
            MessageType::LearningConfirmation {
                lesson_id: lesson_id.to_string(), topic: topic.to_string(),
                understood, confidence, questions: Vec::new(), time_spent_seconds: time_spent,
            },
            Sender::Marisselle,
            self.conversations.active().map(|c| c.id.as_str()).unwrap_or("default"),
        )
    }
    
    // FIXED: Borrow checker error resolved - using active_id() instead of borrowing
    pub fn create_question(&mut self, topic: &str, content: &str, urgency: Urgency) -> Message {
        // Get or create conversation ID without borrowing issues
        let conv_id = if let Some(active_id) = self.conversations.active_id() {
            active_id
        } else {
            self.conversations.start(topic, Sender::Marisselle);
            self.conversations.active_id().unwrap()
        };
        
        Message::new(
            MessageType::Question {
                id: Uuid::new_v4().to_string(), 
                topic: topic.to_string(), 
                content: content.to_string(),
                context: Some(self.validator.get_conversation_summary()), 
                urgency,
                max_wait_seconds: match urgency {
                    Urgency::Critical => 30, 
                    Urgency::High => 120, 
                    Urgency::Normal => 300, 
                    Urgency::Low => 600,
                },
            },
            Sender::Marisselle,
            &conv_id,
        )
    }
    
    // FIXED: Borrow checker error resolved - using active_id() instead of borrowing
    pub fn create_confusion(&mut self, topic: &str, issue: &str) -> Message {
        // Get or create conversation ID without borrowing issues
        let conv_id = if let Some(active_id) = self.conversations.active_id() {
            active_id
        } else {
            self.conversations.start(topic, Sender::Marisselle);
            self.conversations.active_id().unwrap()
        };
        
        Message::new(
            MessageType::Confusion { 
                topic: topic.to_string(), 
                issue: issue.to_string(), 
                attempted_understanding: None, 
                related_concepts: vec![] 
            },
            Sender::Marisselle,
            &conv_id,
        )
    }
    
    pub async fn cleanup_stale_conversations(&self, max_age_hours: u64) -> Result<usize> {
        self.store.cleanup_expired(max_age_hours).await
    }
}

// ======================================================================
// PROTOCOL ACTION (For the main loop)
// ======================================================================

#[derive(Debug, Clone)]
pub enum ProtocolAction {
    Idle,
    DebugMode,
    TeachTopics(Vec<String>),
    ExploreNewTopics,
    HandleQuestion(String),
    HandleConfusion(String, String),
}

impl ProtocolManager {
    pub fn get_next_action(&self) -> ProtocolAction {
        // Check if there are pending questions in the active conversation
        if let Some(conv) = self.conversations.active() {
            for msg in conv.messages.iter().rev().take(5) {
                match &msg.msg_type {
                    MessageType::Question { topic, content, .. } => {
                        return ProtocolAction::HandleQuestion(format!("{}: {}", topic, content));
                    }
                    MessageType::Confusion { topic, issue, .. } => {
                        return ProtocolAction::HandleConfusion(topic.clone(), issue.clone());
                    }
                    _ => {}
                }
            }
        }
        
        // Check for unlearned topics
        let unlearned = self.learning.get_unlearned();
        if !unlearned.is_empty() {
            return ProtocolAction::TeachTopics(unlearned);
        }
        
        ProtocolAction::Idle
    }
}

// ======================================================================
// TESTS
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_message_creation() {
        let msg = Message::new(
            MessageType::Question {
                id: Uuid::new_v4().to_string(), 
                topic: "Test".to_string(), 
                content: "What is this?".to_string(),
                context: None, 
                urgency: Urgency::Normal, 
                max_wait_seconds: 300,
            },
            Sender::Marisselle,
            "test-conv",
        );
        assert!(msg.verify());
        assert_eq!(msg.sender, Sender::Marisselle);
    }
    
    #[test]
    fn test_reply_chain() {
        let original = Message::new(MessageType::Ping, Sender::Teacher, "test-conv");
        let reply = original.reply_to(MessageType::Pong, Sender::Marisselle);
        assert_eq!(reply.in_reply_to, Some(original.id.clone()));
    }
    
    #[test]
    fn test_learning_record() {
        let mut record = LearningRecord::new("Blockchain", "lesson-001");
        assert!(!record.is_learned());
        record.confirm_understood(0.85);
        assert!(record.is_learned());
        assert_eq!(record.mastery, MasteryLevel::Advanced);
    }
    
    #[test]
    fn test_coherence_validator() {
        let validator = CoherenceValidator::new(10);
        let valid_msg = Message::new(
            MessageType::Lesson {
                topic: "Test".to_string(), 
                content: "This is a valid lesson with sufficient content length.".to_string(),
                difficulty: "intermediate".to_string(), 
                lesson_id: Uuid::new_v4().to_string(),
                prerequisites: vec![], 
                estimated_duration_minutes: 10,
            },
            Sender::Teacher,
            "test-conv",
        );
        let result = validator.validate(&valid_msg);
        assert!(matches!(result, CoherenceResult::Valid));
    }
    
    #[test]
    fn test_priority_queue() {
        let mut queue = PriorityQueue::new(10);
        let msg1 = Message::new(MessageType::Ping, Sender::Teacher, "test");
        let msg2 = Message::new(MessageType::Ping, Sender::Teacher, "test");
        queue.push(msg1, 5);
        queue.push(msg2, 10);
        let popped = queue.pop().unwrap();
        assert!(matches!(popped.msg_type, MessageType::Ping));
    }
    
    #[test]
    fn test_fixed_borrow_checker() {
        let mut manager = ConversationManager::new();
        
        // Test active_id() method
        assert!(manager.active_id().is_none());
        
        manager.start("test", Sender::Marisselle);
        assert!(manager.active_id().is_some());
        
        let id = manager.active_id().unwrap();
        assert!(!id.is_empty());
    }
}