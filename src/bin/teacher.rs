// ======================================================================
// TEACHER BINARY - FULL ADVANCED VERSION
// File: src/bin/teacher.rs
// Description: Complete Teacher with protocol, persistence, health checks,
//              metrics, retry logic, and full Ollama integration.
// ======================================================================

use anyhow::{Result, anyhow};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock, Semaphore};
use tokio::time::sleep;
use tracing::{info, warn, error, debug};
use tracing_subscriber;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use std::sync::mpsc::channel;
use serde::{Serialize, Deserialize};
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;
use chrono::Utc;

use self_evolving_lm::learning::{
    amoral_teacher::AmoralOllamaClient,
    protocol::{
        Message, MessageType, Sender, Urgency, AckStatus,
        ProtocolManager, MessageTransport, ConversationStatus,
    },
    curriculum::Curriculum,
};

// ======================================================================
// PERSISTENT STATE
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TeacherState {
    pub topics_taught: Vec<String>,
    pub topics_failed: Vec<String>,
    pub current_topic_index: usize,
    pub total_lessons_generated: u64,
    pub conversations: HashMap<String, ConversationState>,
    pub last_health_check: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConversationState {
    pub id: String,
    pub topic: String,
    pub status: String,
    pub message_count: usize,
    pub last_activity: chrono::DateTime<Utc>,
}

struct PersistentState {
    path: PathBuf,
    state: Arc<RwLock<TeacherState>>,
}

impl PersistentState {
    async fn new(path: PathBuf) -> Result<Self> {
        let state = if path.exists() {
            let data = tokio::fs::read_to_string(&path).await?;
            serde_json::from_str(&data)?
        } else {
            TeacherState {
                topics_taught: Vec::new(),
                topics_failed: Vec::new(),
                current_topic_index: 0,
                total_lessons_generated: 0,
                conversations: HashMap::new(),
                last_health_check: Utc::now(),
            }
        };
        
        Ok(Self {
            path,
            state: Arc::new(RwLock::new(state)),
        })
    }
    
    async fn save(&self) -> Result<()> {
        let state = self.state.read().await;
        let json = serde_json::to_string_pretty(&*state)?;
        tokio::fs::write(&self.path, json).await?;
        Ok(())
    }
    
    async fn record_lesson_taught(&self, topic: String) -> Result<()> {
        let mut state = self.state.write().await;
        state.topics_taught.push(topic);
        state.total_lessons_generated += 1;
        drop(state);
        self.save().await
    }
    
    async fn record_lesson_failed(&self, topic: String) -> Result<()> {
        let mut state = self.state.write().await;
        state.topics_failed.push(topic);
        drop(state);
        self.save().await
    }
}

// ======================================================================
// HEALTH MONITOR
// ======================================================================

struct HealthMonitor {
    start_time: Instant,
    requests_total: Arc<Mutex<u64>>,
    requests_failed: Arc<Mutex<u64>>,
    response_times: Arc<Mutex<VecDeque<Duration>>>,
    last_health: Arc<RwLock<HealthStatus>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl HealthMonitor {
    fn new() -> Self {
        Self {
            start_time: Instant::now(),
            requests_total: Arc::new(Mutex::new(0)),
            requests_failed: Arc::new(Mutex::new(0)),
            response_times: Arc::new(Mutex::new(VecDeque::with_capacity(100))),
            last_health: Arc::new(RwLock::new(HealthStatus::Healthy)),
        }
    }
    
    async fn record_success(&self, duration: Duration) {
        *self.requests_total.lock().await += 1;
        let mut times = self.response_times.lock().await;
        times.push_back(duration);
        if times.len() > 100 {
            times.pop_front();
        }
        self.update_health().await;
    }
    
    async fn record_failure(&self) {
        *self.requests_total.lock().await += 1;
        *self.requests_failed.lock().await += 1;
        self.update_health().await;
    }
    
    async fn update_health(&self) {
        let total = *self.requests_total.lock().await;
        let failed = *self.requests_failed.lock().await;
        let times = self.response_times.lock().await;
        
        let status = if total == 0 {
            HealthStatus::Healthy
        } else {
            let failure_rate = failed as f64 / total as f64;
            let avg_time = if !times.is_empty() {
                times.iter().sum::<Duration>().as_millis() as f64 / times.len() as f64
            } else {
                0.0
            };
            
            if failure_rate > 0.5 || avg_time > 30000.0 {
                HealthStatus::Unhealthy
            } else if failure_rate > 0.1 || avg_time > 10000.0 {
                HealthStatus::Degraded
            } else {
                HealthStatus::Healthy
            }
        };
        
        *self.last_health.write().await = status;
    }
    
    async fn get_metrics(&self) -> serde_json::Value {
        let total = *self.requests_total.lock().await;
        let failed = *self.requests_failed.lock().await;
        let times = self.response_times.lock().await;
        let avg_time = if !times.is_empty() {
            times.iter().sum::<Duration>().as_millis() as f64 / times.len() as f64
        } else {
            0.0
        };
        
        serde_json::json!({
            "uptime_seconds": self.start_time.elapsed().as_secs(),
            "requests_total": total,
            "requests_failed": failed,
            "success_rate": if total > 0 { (total - failed) as f64 / total as f64 } else { 1.0 },
            "avg_response_ms": avg_time,
            "health_status": format!("{:?}", *self.last_health.read().await),
        })
    }
}

// ======================================================================
// RETRY MANAGER
// ======================================================================

struct RetryManager {
    max_retries: u32,
    base_delay_ms: u64,
}

impl RetryManager {
    fn new(max_retries: u32, base_delay_ms: u64) -> Self {
        Self { max_retries, base_delay_ms }
    }
    
    async fn retry_with_backoff<F, T, E>(&self, operation: F) -> Result<T, E>
    where
        F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, E>> + Send>>,
        E: std::fmt::Debug,
    {
        let mut last_error = None;
        
        for attempt in 0..self.max_retries {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                    if attempt < self.max_retries - 1 {
                        let delay = self.base_delay_ms * 2_u64.pow(attempt);
                        warn!("Attempt {} failed, retrying in {}ms", attempt + 1, delay);
                        sleep(Duration::from_millis(delay)).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap())
    }
}

// ======================================================================
// MAIN TEACHER
// ======================================================================

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .init();
    
    info!("============================================================");
    info!("              AMORAL TEACHER - FULL ADVANCED                  ");
    info!("============================================================");
    
    // Initialize components
    let training_dir = PathBuf::from("training_data");
    let state = PersistentState::new(training_dir.join(".teacher_state.json")).await?;
    let health = HealthMonitor::new();
    let retry = RetryManager::new(3, 500);
    
    // Load curriculum
    let curriculum = Curriculum::new();
    info!("Loaded curriculum: {} topics", curriculum.topics.len());
    
    // Initialize Ollama client
    let client = AmoralOllamaClient::new("llama3.2:3b".to_string());
    
    // Initialize protocol
    let transport = MessageTransport::new(training_dir.clone(), Sender::Teacher);
    let protocol = Arc::new(Mutex::new(ProtocolManager::new()));
    
    // Create directories
    std::fs::create_dir_all(&training_dir)?;
    
    // Set up file watcher for incoming messages
    let (tx, rx) = channel();
    let mut watcher = notify::recommended_watcher(tx)?;
    let inbox_dir = training_dir.join(".inbox_teacher");
    watcher.watch(&inbox_dir, RecursiveMode::NonRecursive)?;
    
    info!("📂 Watching for messages in: {:?}", inbox_dir);
    info!("📚 Ready to teach {} topics", curriculum.topics.len());
    info!("============================================================");
    
    // Start health reporter
    let health_clone = health.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            let metrics = health_clone.get_metrics().await;
            info!("Health metrics: {}", serde_json::to_string(&metrics).unwrap());
        }
    });
    
    // Start curriculum teaching
    let client_clone = client.clone();
    let state_clone = state.clone();
    let protocol_clone = protocol.clone();
    let health_clone2 = health.clone();
    let transport_clone = transport.clone();
    
    tokio::spawn(async move {
        let mut topic_index = {
            let s = state_clone.state.read().await;
            s.current_topic_index
        };
        
        let topics: Vec<String> = curriculum.topics.iter()
            .flat_map(|t| {
                let mut v = vec![t.name.clone()];
                v.extend(t.sub_topics.clone());
                v
            })
            .collect();
        
        while topic_index < topics.len() {
            let topic = &topics[topic_index];
            info!("Teaching ({}): {}", topic_index + 1, topic);
            
            let start = Instant::now();
            
            let result = retry.retry_with_backoff(|| {
                let c = client_clone.clone();
                let t = topic.clone();
                Box::pin(async move { c.teach(&t, "intermediate").await })
            }).await;
            
            match result {
                Ok(lesson) => {
                    health_clone2.record_success(start.elapsed()).await;
                    
                    // Save lesson file
                    let filename = training_dir.join(
                        format!("lesson_{:04}_{}.md", topic_index, 
                                topic.replace(|c: char| !c.is_alphanumeric(), "_"))
                    );
                    let content = format!("# Lesson: {}\n\n{}\n", topic, lesson);
                    tokio::fs::write(&filename, &content).await.ok();
                    
                    // Send lesson via protocol
                    let mut pm = protocol_clone.lock().await;
                    let conv_id = pm.conversations.start_conversation(topic, Sender::Teacher);
                    let lesson_msg = Message::new(
                        MessageType::Lesson {
                            topic: topic.clone(),
                            content: lesson.clone(),
                            difficulty: "intermediate".to_string(),
                            lesson_id: Uuid::new_v4().to_string(),
                        },
                        Sender::Teacher,
                        &conv_id,
                    );
                    transport_clone.send(&lesson_msg).await.ok();
                    
                    state_clone.record_lesson_taught(topic.clone()).await.ok();
                    info!("✅ Lesson saved and sent: {}", filename.display());
                }
                Err(e) => {
                    health_clone2.record_failure().await;
                    error!("Failed to teach '{}': {}", topic, e);
                    state_clone.record_lesson_failed(topic.clone()).await.ok();
                }
            }
            
            topic_index += 1;
            {
                let mut s = state_clone.state.write().await;
                s.current_topic_index = topic_index;
                let _ = state_clone.save().await;
            }
            
            sleep(Duration::from_millis(500)).await;
        }
        
        info!("Curriculum complete!");
    });
    
    // Process incoming messages
    for res in rx {
        match res {
            Ok(event) => {
                if matches!(event.kind, EventKind::Create(_)) {
                    let messages = transport.receive().await?;
                    for message in messages {
                        info!("📨 Received: {:?} from {:?}", message.msg_type, message.sender);
                        
                        let response = {
                            let mut pm = protocol.lock().await;
                            pm.process_incoming(message.clone())?
                        };
                        
                        // Handle based on type
                        match message.msg_type {
                            MessageType::Question { id, topic, content, urgency, .. } => {
                                info!("❓ Question about '{}': {}", topic, &content[..content.len().min(100)]);
                                
                                let start = Instant::now();
                                let answer = client.answer_question(&content).await;
                                health.record_success(start.elapsed()).await;
                                
                                if let Ok(ans) = answer {
                                    let answer_msg = message.reply_to(
                                        MessageType::Answer {
                                            question_id: id,
                                            content: ans,
                                            confidence: 0.9,
                                        },
                                        Sender::Teacher,
                                    );
                                    transport.send(&answer_msg).await?;
                                }
                            }
                            MessageType::Confusion { topic, issue, .. } => {
                                info!("🤔 Marisselle confused about '{}': {}", topic, issue);
                                
                                let clarification = client.teach(&topic, "basic").await?;
                                let clarify_msg = message.reply_to(
                                    MessageType::Clarification {
                                        original_topic: topic,
                                        explanation: clarification,
                                    },
                                    Sender::Teacher,
                                );
                                transport.send(&clarify_msg).await?;
                            }
                            MessageType::LearningConfirmation { lesson_id, topic, understood, confidence, .. } => {
                                if understood {
                                    info!("✅ Marisselle LEARNED '{}' (confidence: {:.1}%)", topic, confidence * 100.0);
                                } else {
                                    warn!("❌ Marisselle did NOT learn '{}' (confidence: {:.1}%)", topic, confidence * 100.0);
                                }
                            }
                            MessageType::Ping => {
                                let pong = message.reply_to(MessageType::Pong, Sender::Teacher);
                                transport.send(&pong).await?;
                            }
                            _ => {}
                        }
                        
                        if let Some(ack) = response {
                            transport.send(&ack).await?;
                        }
                    }
                }
            }
            Err(e) => error!("Watcher error: {}", e),
        }
    }
    
    Ok(())
}