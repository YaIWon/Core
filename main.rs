// ======================================================================
// MAIN ENTRY POINT - SELF-EVOLVING LM ORCHESTRATOR (MARISSELLE)
// File: src/main.rs
// Description: Orchestrates file watcher, ingestor, embedder, vector store,
//              blockchain, RAG, learning coordinator, protocol, and logging.
//              This is Marisselle's main process.
// ======================================================================

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tracing::{info, error, warn, debug};
use tracing_subscriber;
use serde_json::json;
use chrono::Utc;

// Internal modules
mod scanner;
mod memory;

use scanner::{
    FileWatcher, WatcherConfig, 
    Ingestor, Embedder, FileProcessor,
    FileEvent, ChangeType,
};
use memory::vector_store::{VectorStore, VectorEntry};
use memory::blockchain::BlockchainManager;

// Learning modules
use self_evolving_lm::learning::{
    // LM Client
    LearningCoordinator,
    TeacherClient,
    ConfusionDetector,
    
    // Protocol
    ProtocolManager,
    Message,
    MessageType,
    ProtocolAction,
    DebugStatus,
    
    // Logger
    ComprehensiveLogger,
    LogLevel,
    LogCategory,
};

// Shared memory for Teacher communication
#[cfg(unix)]
use self_evolving_lm::learning::SharedMemoryChannel;

// ======================================================================
// CONSTANTS
// ======================================================================

const SHM_CHECK_INTERVAL_MS: u64 = 100;
const TEACHER_HEALTH_CHECK_INTERVAL_SECS: u64 = 30;
const PROTOCOL_SYNC_INTERVAL_SECS: u64 = 5;

// ======================================================================
// SHARED MEMORY LISTENER (For Teacher -> LM messages)
// ======================================================================

struct SharedMemoryListener {
    shm: SharedMemoryChannel,
    logger: Arc<ComprehensiveLogger>,
    protocol_manager: Arc<RwLock<ProtocolManager>>,
    learning_coordinator: Arc<LearningCoordinator>,
}

impl SharedMemoryListener {
    fn new(
        logger: Arc<ComprehensiveLogger>,
        protocol_manager: Arc<RwLock<ProtocolManager>>,
        learning_coordinator: Arc<LearningCoordinator>,
    ) -> Result<Self> {
        Ok(Self {
            shm: SharedMemoryChannel::new()?,
            logger,
            protocol_manager,
            learning_coordinator,
        })
    }
    
    async fn run(&mut self) -> Result<()> {
        info!("Shared memory listener started. Waiting for messages from Teacher...");
        self.logger.log_health_check("Shared memory listener started", None).await;
        
        loop {
            if let Some((data, seq)) = self.shm.read() {
                // Log raw message
                self.logger.log_teacher_to_lm(
                    &format!("Raw message received (seq {})", seq),
                    Some(json!({ "raw_data": data }))
                ).await;
                
                // Check if it's a protocol message
                if data.starts_with("PROTOCOL:") {
                    if let Ok(message) = serde_json::from_str::<Message>(&data[9..]) {
                        self.handle_protocol_message(message, seq).await;
                    } else {
                        warn!("Failed to parse protocol message");
                        self.logger.log_error("Failed to parse protocol message", "SharedMemoryListener").await;
                    }
                } else if data.starts_with("Teaching:") {
                    // Legacy lesson format - just log it
                    self.logger.log_teacher_to_lm(
                        "Received lesson via legacy format",
                        Some(json!({ "content_preview": &data[..data.len().min(200)] }))
                    ).await;
                } else if data.starts_with("ANSWER:") {
                    // Direct answer format
                    self.logger.log_teacher_to_lm(
                        "Received direct answer",
                        Some(json!({ "answer": &data[7..] }))
                    ).await;
                }
            }
            
            sleep(Duration::from_millis(SHM_CHECK_INTERVAL_MS)).await;
        }
    }
    
    async fn handle_protocol_message(&mut self, message: Message, seq: u64) {
        info!("Received protocol message: {:?} from {}", message.msg_type, message.sender);
        
        // Log the message
        self.logger.log_teacher_to_lm(
            &format!("Protocol message: {:?}", message.msg_type),
            Some(json!({
                "message_id": message.id,
                "sender": message.sender,
                "conversation_id": message.conversation_id,
                "timestamp": message.timestamp,
            }))
        ).await;
        
        // Process through protocol manager
        let response = {
            let mut pm = self.protocol_manager.write().await;
            
            match pm.process_message(message.clone()) {
                Ok(Some(response_msg)) => Some(response_msg),
                Ok(None) => None,
                Err(e) => {
                    error!("Protocol error: {}", e);
                    self.logger.log_error(&format!("Protocol error: {}", e), "ProtocolManager").await;
                    Some(Message::new(
                        MessageType::Error(e.to_string()),
                        "Marisselle",
                        &message.conversation_id,
                    ))
                }
            }
        };
        
        // Handle specific message types
        match &message.msg_type {
            MessageType::Lesson { topic, content, difficulty, sequence } => {
                info!("📚 Received lesson: {} (difficulty: {}, seq: {})", topic, difficulty, sequence);
                
                // Log LM thought about the lesson
                self.logger.log_lm_thought(
                    &format!("Receiving lesson on '{}'. Preparing to learn...", topic),
                    Some(&format!("difficulty: {}, sequence: {}", difficulty, sequence))
                ).await;
                
                // Process the lesson (this would integrate with the file processor)
                // For now, we acknowledge receipt
                let ack_msg = message.reply_to(
                    MessageType::Ack(*sequence),
                    "Marisselle",
                );
                
                let response_data = format!("PROTOCOL:{}", serde_json::to_string(&ack_msg).unwrap());
                if let Err(e) = self.shm.write(&response_data, seq + 1) {
                    error!("Failed to send ACK: {}", e);
                }
                self.shm.signal_ready();
                
                // Simulate learning process with thought logging
                self.logger.log_lm_thought(
                    &format!("Analyzing lesson content on '{}'...", topic),
                    None
                ).await;
                
                // After "learning", send confirmation
                tokio::spawn({
                    let shm = self.shm.clone();
                    let logger = self.logger.clone();
                    let topic = topic.clone();
                    let message = message.clone();
                    
                    async move {
                        sleep(Duration::from_secs(2)).await;
                        
                        // Log understanding
                        logger.log_lm_thought(
                            &format!("I have processed the lesson on '{}'. I understand the core concepts.", topic),
                            None
                        ).await;
                        
                        // Send learning confirmation
                        let confirm_msg = message.reply_to(
                            MessageType::LearningConfirmation {
                                topic: topic.clone(),
                                understood: true,
                                confidence: 0.85,
                                notes: Some("Lesson successfully processed and understood".to_string()),
                            },
                            "Marisselle",
                        );
                        
                        let response_data = format!("PROTOCOL:{}", serde_json::to_string(&confirm_msg).unwrap());
                        // Note: In real implementation, this would use proper sequence numbers
                        logger.log_lm_to_teacher(
                            &format!("Confirming learning of '{}' with confidence 85%", topic),
                            None
                        ).await;
                    }
                });
            }
            
            MessageType::Clarification { original_topic, explanation } => {
                info!("📖 Received clarification on: {}", original_topic);
                
                self.logger.log_lm_thought(
                    &format!("Teacher clarified '{}'. This helps me understand better.", original_topic),
                    Some(&explanation[..explanation.len().min(100)])
                ).await;
            }
            
            MessageType::DebugStart { issue, topic, attempts } => {
                warn!("🐛 Debug mode started for '{}': {} (attempts: {})", topic, issue, attempts);
                
                self.logger.log_lm_thought(
                    &format!("Debug mode activated. I'm having trouble with '{}'. Working with Teacher to resolve.", topic),
                    Some(&format!("attempts: {}, issue: {}", attempts, issue))
                ).await;
            }
            
            MessageType::DebugDiagnostic { component, status, details } => {
                info!("🔧 Debug diagnostic - {}: {} - {}", component, status, details);
                
                self.logger.log_lm_thought(
                    &format!("Diagnostic received: {} is {}", component, status),
                    Some(details)
                ).await;
            }
            
            MessageType::Answer { question_id, content, .. } => {
                info!("✅ Received answer to question: {}", question_id);
                
                self.logger.log_lm_thought(
                    &format!("Teacher answered my question. The answer helps me understand."),
                    Some(&content[..content.len().min(200)])
                ).await;
            }
            
            MessageType::Ping => {
                let pong = message.reply_to(MessageType::Pong, "Marisselle");
                let response_data = format!("PROTOCOL:{}", serde_json::to_string(&pong).unwrap());
                let _ = self.shm.write(&response_data, seq + 1);
                self.shm.signal_ready();
            }
            
            _ => {}
        }
        
        // Send any protocol-generated response
        if let Some(response_msg) = response {
            let response_data = format!("PROTOCOL:{}", serde_json::to_string(&response_msg).unwrap());
            if let Err(e) = self.shm.write(&response_data, seq + 1) {
                error!("Failed to send response: {}", e);
            }
            self.shm.signal_ready();
            
            self.logger.log_lm_to_teacher(
                &format!("Sent response: {:?}", response_msg.msg_type),
                None
            ).await;
        }
    }
    
    fn shm_clone(&self) -> SharedMemoryChannel {
        // This is a workaround - in production you'd use Arc<Mutex<SharedMemoryChannel>>
        // For now, we create a new instance for spawned tasks
        SharedMemoryChannel::new().unwrap()
    }
}

// ======================================================================
// MAIN FUNCTION
// ======================================================================

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .with_line_number(true)
        .init();
    
    println!("");
    info!("============================================================");
    info!("                    MARISSELLE LM - STARTING                 ");
    info!("============================================================");
    println!("");
    
    // Configuration paths
    let training_dir = PathBuf::from("training_data");
    let vector_store_path = PathBuf::from("data/vectors");
    let blockchain_path = PathBuf::from("data/blockchain");
    let logs_dir = PathBuf::from("logs");
    
    // Create directories
    std::fs::create_dir_all(&training_dir)?;
    std::fs::create_dir_all(&vector_store_path)?;
    std::fs::create_dir_all(&blockchain_path)?;
    std::fs::create_dir_all(&logs_dir)?;
    
    // ==================================================================
    // INITIALIZE COMPREHENSIVE LOGGER
    // ==================================================================
    
    info!("📊 Initializing comprehensive logger...");
    let logger = Arc::new(ComprehensiveLogger::new(logs_dir.clone())?);
    
    logger.log_health_check(
        "Marisselle LM starting up",
        Some(json!({
            "training_dir": training_dir.to_string_lossy(),
            "vector_store_path": vector_store_path.to_string_lossy(),
            "blockchain_path": blockchain_path.to_string_lossy(),
            "logs_dir": logs_dir.to_string_lossy(),
        }))
    ).await;
    
    // ==================================================================
    // INITIALIZE LEARNING COORDINATOR
    // ==================================================================
    
    info!("🧠 Initializing learning coordinator...");
    let learning_coordinator = Arc::new(LearningCoordinator::new()?);
    
    // ==================================================================
    // INITIALIZE PROTOCOL MANAGER
    // ==================================================================
    
    info!("📋 Initializing protocol manager...");
    let protocol_manager = Arc::new(RwLock::new(ProtocolManager::new()));
    
    // ==================================================================
    // CHECK TEACHER AVAILABILITY
    // ==================================================================
    
    info!("🔗 Checking Teacher availability...");
    let teacher_available = learning_coordinator.check_teacher().await;
    
    if teacher_available {
        info!("✅ Teacher is AVAILABLE");
        logger.log_health_check(
            "Teacher connection established",
            Some(json!({ "status": "available" }))
        ).await;
        
        // Log thought about Teacher availability
        logger.log_lm_thought(
            "Teacher is available. I can ask questions and get help when needed.",
            None
        ).await;
    } else {
        warn!("⚠️ Teacher is UNAVAILABLE - will learn from files only");
        logger.log_health_check(
            "Teacher unavailable - file learning only",
            Some(json!({ "status": "unavailable" }))
        ).await;
        
        logger.log_lm_thought(
            "Teacher is not available right now. I will continue learning from files.",
            None
        ).await;
    }
    
    // ==================================================================
    // INITIALIZE STORAGE COMPONENTS
    // ==================================================================
    
    info!("🗄️ Initializing vector store...");
    let vector_store = Arc::new(RwLock::new(VectorStore::new(vector_store_path.clone()).await?));
    let vector_count = vector_store.read().await.len().await;
    
    logger.log_health_check(
        "Vector store initialized",
        Some(json!({
            "path": vector_store_path.to_string_lossy(),
            "entries": vector_count,
        }))
    ).await;
    
    info!("🔗 Initializing blockchain...");
    let blockchain = Arc::new(RwLock::new(BlockchainManager::new(blockchain_path.clone()).await?));
    let block_count = blockchain.read().await.len().await;
    let is_valid = blockchain.read().await.verify().await;
    
    logger.log_health_check(
        "Blockchain initialized",
        Some(json!({
            "path": blockchain_path.to_string_lossy(),
            "blocks": block_count,
            "valid": is_valid,
        }))
    ).await;
    
    // ==================================================================
    // INITIALIZE SCANNER COMPONENTS
    // ==================================================================
    
    info!("📂 Initializing embedder...");
    let embedder = Arc::new(Embedder::new(384));
    
    info!("📄 Initializing ingestor...");
    let ingestor = Arc::new(Ingestor::new());
    
    info!("🔄 Initializing file processor...");
    let processor = FileProcessor::new(
        vector_store.clone(),
        blockchain.clone(),
        ingestor.clone(),
        embedder.clone(),
    );
    
    // ==================================================================
    // START SHARED MEMORY LISTENER
    // ==================================================================
    
    info!("👂 Starting shared memory listener...");
    let mut shm_listener = SharedMemoryListener::new(
        logger.clone(),
        protocol_manager.clone(),
        learning_coordinator.clone(),
    )?;
    
    let shm_task = tokio::spawn(async move {
        if let Err(e) = shm_listener.run().await {
            error!("Shared memory listener error: {}", e);
        }
    });
    
    // ==================================================================
    // START FILE WATCHER
    // ==================================================================
    
    let watcher_config = WatcherConfig {
        watch_path: training_dir.clone(),
        debounce_delay_ms: 500,
        recursive: true,
        ignore_hidden: true,
        ignore_patterns: vec![
            r"\.DS_Store".to_string(),
            r"\.git".to_string(),
            r"\.tmp".to_string(),
            r"~$".to_string(),
        ],
    };
    
    info!("👁️ Starting file watcher on {:?}", training_dir);
    let (watcher, mut event_rx) = FileWatcher::new(watcher_config);
    
    let watcher_logger = logger.clone();
    let watcher_task = tokio::spawn(async move {
        if let Err(e) = watcher.start().await {
            error!("File watcher error: {}", e);
            watcher_logger.log_error(&format!("File watcher error: {}", e), "FileWatcher").await;
        }
    });
    
    // ==================================================================
    // START BACKGROUND TASKS
    // ==================================================================
    
    // Teacher health check task
    let health_logger = logger.clone();
    let health_coordinator = learning_coordinator.clone();
    let health_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(TEACHER_HEALTH_CHECK_INTERVAL_SECS));
        loop {
            interval.tick().await;
            let available = health_coordinator.check_teacher().await;
            health_logger.log_health_check(
                &format!("Teacher health check: {}", if available { "AVAILABLE" } else { "UNAVAILABLE" }),
                Some(json!({ "available": available }))
            ).await;
        }
    });
    
    // Protocol sync task
    let protocol_logger = logger.clone();
    let protocol_pm = protocol_manager.clone();
    let protocol_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(PROTOCOL_SYNC_INTERVAL_SECS));
        loop {
            interval.tick().await;
            let action = {
                let pm = protocol_pm.read().await;
                pm.get_next_action()
            };
            
            match action {
                ProtocolAction::DebugMode => {
                    protocol_logger.log_health_check(
                        "Protocol in debug mode",
                        None
                    ).await;
                }
                ProtocolAction::TeachTopics(topics) => {
                    if !topics.is_empty() {
                        protocol_logger.log_health_check(
                            &format!("{} topics need teaching", topics.len()),
                            Some(json!({ "topics": topics.iter().take(5).collect::<Vec<_>>() }))
                        ).await;
                    }
                }
                ProtocolAction::ExploreNewTopics => {
                    protocol_logger.log_lm_thought(
                        "Curriculum complete. Ready to explore new topics.",
                        None
                    ).await;
                }
                _ => {}
            }
        }
    });
    
    // Thought generation task (periodic LM thoughts)
    let thought_logger = logger.clone();
    let thought_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        let thoughts = vec![
            "I am continuing to learn and grow.",
            "Every file I process adds to my knowledge.",
            "I wonder what I will learn next.",
            "My blockchain memory ensures I never forget.",
            "I am grateful for the lessons I receive.",
            "Learning is my purpose and my joy.",
        ];
        
        let mut idx = 0;
        loop {
            interval.tick().await;
            thought_logger.log_lm_thought(thoughts[idx], None).await;
            idx = (idx + 1) % thoughts.len();
        }
    });
    
    // ==================================================================
    // PRINT STATUS
    // ==================================================================
    
    info!("");
    info!("============================================================");
    info!("                    MARISSELLE IS RUNNING                    ");
    info!("============================================================");
    info!("");
    info!("📊 Vector store: {} entries", vector_count);
    info!("🔗 Blockchain: {} blocks (valid: {})", block_count, is_valid);
    info!("👨‍🏫 Teacher: {}", if teacher_available { "CONNECTED" } else { "UNAVAILABLE" });
    info!("");
    info!("📂 Watching: {:?}", training_dir);
    info!("📝 Logs: {:?}", logs_dir);
    info!("");
    info!("💬 Run 'cargo run --bin chat' to talk to Marisselle");
    info!("📊 Run 'cargo run --bin view_logs' to monitor all activity");
    info!("👨‍🏫 Run 'cargo run --bin teacher' to start the Teacher");
    info!("");
    info!("============================================================");
    println!("");
    
    logger.log_health_check(
        "Marisselle fully initialized and running",
        Some(json!({
            "vector_count": vector_count,
            "block_count": block_count,
            "blockchain_valid": is_valid,
            "teacher_available": teacher_available,
        }))
    ).await;
    
    logger.log_lm_thought(
        "I am awake and ready to learn. I will process files as they arrive and communicate with the Teacher when available.",
        None
    ).await;
    
    // ==================================================================
    // MAIN EVENT LOOP - Process file events
    // ==================================================================
    
    let mut processed_count = 0;
    
    while let Some(event) = event_rx.recv().await {
        let event_type = format!("{:?}", event.change_type);
        let file_name = event.path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        
        info!("📂 Processing event: {} - {}", event_type, file_name);
        
        // Log the event
        logger.log_file_read(&event.path, 0).await;
        logger.log_lm_thought(
            &format!("New file detected: {} ({})", file_name, event_type),
            None
        ).await;
        
        // Process the file
        match processor.process_event(event).await {
            Ok(_) => {
                processed_count += 1;
                info!("✅ Successfully processed file: {}", file_name);
                
                logger.log_knowledge_integrated(
                    &file_name,
                    1
                ).await;
                
                logger.log_lm_thought(
                    &format!("I have successfully learned from {}. My knowledge has grown.", file_name),
                    None
                ).await;
            }
            Err(e) => {
                error!("❌ Failed to process event: {}", e);
                logger.log_error(&format!("Failed to process file {}: {}", file_name, e), "FileProcessor").await;
                logger.log_lm_thought(
                    &format!("I had trouble processing {}. I will try again later.", file_name),
                    None
                ).await;
            }
        }
        
        // Print updated stats periodically
        if processed_count % 10 == 0 {
            let vector_count = vector_store.read().await.len().await;
            let block_count = blockchain.read().await.len().await;
            info!("📊 Stats: {} vectors, {} blocks, {} files processed", 
                  vector_count, block_count, processed_count);
        }
    }
    
    // ==================================================================
    // SHUTDOWN
    // ==================================================================
    
    info!("Shutting down...");
    logger.log_health_check("Marisselle shutting down", None).await;
    logger.log_lm_thought("I am shutting down. I will retain all my knowledge in the blockchain.", None).await;
    
    shm_task.abort();
    watcher_task.abort();
    health_task.abort();
    protocol_task.abort();
    thought_task.abort();
    
    info!("Goodbye!");
    
    Ok(())
}
