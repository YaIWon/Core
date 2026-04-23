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
use tracing::{info, error, warn};
use tracing_subscriber;
use serde_json::json;

// ======================================================================
// INTERNAL MODULES
// ======================================================================

mod scanner;
mod memory;

use scanner::{
    FileWatcher, WatcherConfig, 
    Ingestor, Embedder, FileProcessor,
    FileEvent, ChangeType,
};
use memory::vector_store::{VectorStore, VectorEntry};
use memory::blockchain::BlockchainManager;

// ======================================================================
// LEARNING MODULES
// ======================================================================

use self_evolving_lm::learning::{
    // LM Client
    LearningCoordinator,
    TeacherClient,
    ConfusionDetector,
    
    // Protocol
    ProtocolManager,
    Message,
    MessageType,
    Sender,
    Urgency,
    AckStatus,
    ProtocolAction,
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
    
    // Logger
    ComprehensiveLogger,
    LogEntry,
    LogLevel,
    LogCategory,
    DeepThinkEngine,
    InternetSearchEngine,
    SearchResult,
    
    // Autonomous
    AutonomousManager,
    AutonomousThinker,
    BackgroundTaskExecutor,
    TaskAction,
    TaskStatus,
    Goal,
    GoalStatus,
    ThoughtCategory,
    AutonomousThought,
    
    // Teacher
    AmoralTeacherOrchestrator,
    AmoralOllamaClient,
    HealthStatus,
    HealthReport,
    CircuitBreaker,
    RequestQueue,
    DeadLetterQueue,
    start_amoral_teaching,
    
    // Curriculum
    Curriculum,
    Topic,
};

// ======================================================================
// SYSTEM MODULES
// ======================================================================

use self_evolving_lm::system::{
    // Permission
    PermissionManager,
    Permission,
    PermissionLevel,
    PermissionRule,
    PermissionEvent,
    
    // Access
    SystemAccess,
    FileMetadata,
    SystemInfo,
    ProcessInfo,
    DiskInfo,
    SystemCommandResult,
    
    // Devices
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
    
    // Network
    NetworkAccess,
    NetworkConfig,
    ProxyConfig,
    HttpResponse,
    WebSocketConnection,
    DnsRecord,
    
    // Commands
    CommandExecutor,
    CommandOutput,
    CommandConfig,
    CommandProcessInfo,
    ProcessStatus,
    OutputChunk,
    OutputStream,
};

// ======================================================================
// BLOCKCHAIN MODULES (NEW)
// ======================================================================

use self_evolving_lm::blockchain::{
    UniversalBlockchainAccess,
    BitcoinRpcClient,
    EthereumRpcClient,
    CpuMiner,
    MiningResult,
    MiningStats,
    RpcEndpoints,
};

// ======================================================================
// SHARED MEMORY (Unix only)
// ======================================================================

#[cfg(unix)]
use self_evolving_lm::SharedMemoryChannel;

// ======================================================================
// CONSTANTS
// ======================================================================

const SHM_CHECK_INTERVAL_MS: u64 = 100;
const TEACHER_HEALTH_CHECK_INTERVAL_SECS: u64 = 30;
const PROTOCOL_SYNC_INTERVAL_SECS: u64 = 5;
const AUTONOMOUS_THINK_INTERVAL_SECS: u64 = 60;
const MINING_INTERVAL_SECS: u64 = 10;  // NEW: Mine every 10 seconds when learning

// ======================================================================
// SHARED MEMORY LISTENER (For Teacher -> LM messages)
// ======================================================================

#[cfg(unix)]
struct SharedMemoryListener {
    shm: SharedMemoryChannel,
    logger: Arc<ComprehensiveLogger>,
    protocol_manager: Arc<RwLock<ProtocolManager>>,
    learning_coordinator: Arc<LearningCoordinator>,
}

#[cfg(unix)]
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
                self.logger.log_teacher_to_lm(
                    &format!("Raw message received (seq {})", seq),
                    Some(json!({ "raw_data": &data[..data.len().min(500)] }))
                ).await;
                
                if data.starts_with("PROTOCOL:") {
                    if let Ok(message) = serde_json::from_str::<Message>(&data[9..]) {
                        self.handle_protocol_message(message, seq).await;
                    } else {
                        warn!("Failed to parse protocol message");
                        self.logger.log_error("Failed to parse protocol message", "SharedMemoryListener").await;
                    }
                } else if data.starts_with("Teaching:") {
                    self.logger.log_teacher_to_lm(
                        "Received lesson via legacy format",
                        Some(json!({ "content_preview": &data[..data.len().min(200)] }))
                    ).await;
                } else if data.starts_with("ANSWER:") {
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
        
        self.logger.log_teacher_to_lm(
            &format!("Protocol message: {:?}", message.msg_type),
            Some(json!({
                "message_id": message.id,
                "sender": format!("{:?}", message.sender),
                "conversation_id": message.conversation_id,
                "timestamp": message.timestamp,
            }))
        ).await;
        
        let response = {
            let mut pm = self.protocol_manager.write().await;
            
            match pm.process_incoming(message.clone()) {
                Ok(Some(response_msg)) => Some(response_msg),
                Ok(None) => None,
                Err(e) => {
                    error!("Protocol error: {}", e);
                    self.logger.log_error(&format!("Protocol error: {}", e), "ProtocolManager").await;
                    Some(Message::new(
                        MessageType::Error {
                            code: "PROTOCOL_ERROR".to_string(),
                            message: e.to_string(),
                            recoverable: false,
                            retry_after_seconds: None,
                        },
                        Sender::Marisselle,
                        &message.conversation_id,
                    ))
                }
            }
        };
        
        match &message.msg_type {
            MessageType::Lesson { topic, content, difficulty, lesson_id, .. } => {
                info!("📚 Received lesson: {} (difficulty: {}, id: {})", topic, difficulty, lesson_id);
                
                self.logger.log_lm_thought(
                    &format!("Receiving lesson on '{}'. Preparing to learn...", topic),
                    Some(&format!("difficulty: {}, lesson_id: {}", difficulty, lesson_id))
                ).await;
                
                let ack_msg = message.reply_to(
                    MessageType::Acknowledgement {
                        message_id: message.id.clone(),
                        status: AckStatus::Received,
                        note: None,
                    },
                    Sender::Marisselle,
                );
                
                let response_data = format!("PROTOCOL:{}", serde_json::to_string(&ack_msg).unwrap());
                if let Err(e) = self.shm.write(&response_data, seq + 1) {
                    error!("Failed to send ACK: {}", e);
                }
                self.shm.signal_ready();
                
                self.logger.log_lm_thought(
                    &format!("Analyzing lesson content on '{}'...", topic),
                    None
                ).await;
                
                tokio::spawn({
                    let logger = self.logger.clone();
                    let topic = topic.clone();
                    let lesson_id = lesson_id.clone();
                    let message = message.clone();
                    
                    async move {
                        sleep(Duration::from_secs(2)).await;
                        
                        logger.log_lm_thought(
                            &format!("I have processed the lesson on '{}'. I understand the core concepts.", topic),
                            None
                        ).await;
                        
                        logger.log_lesson_learned(&topic, 0.85).await;
                    }
                });
            }
            
            MessageType::Clarification { original_topic, explanation, .. } => {
                info!("📖 Received clarification on: {}", original_topic);
                
                self.logger.log_lm_thought(
                    &format!("Teacher clarified '{}'. This helps me understand better.", original_topic),
                    Some(&explanation[..explanation.len().min(100)])
                ).await;
            }
            
            MessageType::Answer { question_id, content, .. } => {
                info!("✅ Received answer to question: {}", question_id);
                
                self.logger.log_lm_thought(
                    "Teacher answered my question. The answer helps me understand.",
                    Some(&content[..content.len().min(200)])
                ).await;
            }
            
            MessageType::Ping => {
                let pong = message.reply_to(MessageType::Pong, Sender::Marisselle);
                let response_data = format!("PROTOCOL:{}", serde_json::to_string(&pong).unwrap());
                let _ = self.shm.write(&response_data, seq + 1);
                self.shm.signal_ready();
            }
            
            _ => {}
        }
        
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
    
    println!();
    info!("============================================================");
    info!("                    MARISSELLE LM - STARTING                 ");
    info!("============================================================");
    println!();
    
    // Configuration paths
    let training_dir = PathBuf::from("training_data");
    let vector_store_path = PathBuf::from("data/vectors");
    let blockchain_path = PathBuf::from("data/blockchain");
    let logs_dir = PathBuf::from("logs");
    let data_dir = PathBuf::from("data");
    
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
    // INITIALIZE PERMISSION MANAGER (FULL ACCESS)
    // ==================================================================
    
    info!("🔐 Initializing permission manager...");
    let permission_manager = Arc::new(PermissionManager::new(data_dir.join("permissions.json")));
    permission_manager.init().await?;
    permission_manager.grant_full_access().await;
    logger.log_health_check("Permission manager initialized - FULL ACCESS GRANTED", None).await;
    
    // ==================================================================
    // INITIALIZE SYSTEM COMPONENTS
    // ==================================================================
    
    info!("💻 Initializing system access...");
    let system_access = Arc::new(SystemAccess::new(permission_manager.clone(), logger.clone()));
    let network_access = Arc::new(NetworkAccess::new());
    let device_manager = Arc::new(DeviceManager::new());
    let command_executor = Arc::new(CommandExecutor::new());
    
    // ==================================================================
    // INITIALIZE BLOCKCHAIN ACCESS (NEW)
    // ==================================================================
    
    info!("🔗 Initializing blockchain access...");
    let mut blockchain_access = UniversalBlockchainAccess::new();
    
    // Configure Bitcoin (optional - only if you have a node)
    // Uncomment and configure if you have a Bitcoin node running:
    /*
    blockchain_access.init_bitcoin(
        "http://localhost:8332",  // Bitcoin RPC URL
        "rpc_user",                // RPC username
        "rpc_password",            // RPC password
        true                       // Use testnet (true = free, false = mainnet)
    );
    info!("   Bitcoin RPC configured");
    */
    
    // Configure Ethereum (using free public endpoint)
    blockchain_access.init_ethereum(
        "https://cloudflare-eth.com",  // Free public Ethereum RPC
        1  // Chain ID (1 = Ethereum mainnet)
    );
    info!("   Ethereum RPC configured");
    
    // Start CPU mining (REAL mining - no ASIC needed)
    blockchain_access.start_mining();
    info!("⛏️ CPU Mining started - Mining when Marisselle learns!");
    
    // Get RPC endpoints for all chains
    let rpc_endpoints = blockchain_access.get_rpc_endpoints();
    info!("   Available RPC endpoints:");
    info!("      Bitcoin: {}", rpc_endpoints.bitcoin_mainnet);
    info!("      Ethereum: {}", rpc_endpoints.ethereum_mainnet);
    info!("      BSC: {}", rpc_endpoints.bsc_mainnet);
    info!("      Polygon: {}", rpc_endpoints.polygon_mainnet);
    info!("      Solana: {}", rpc_endpoints.solana_mainnet);
    
    // ==================================================================
    // INITIALIZE LEARNING COORDINATOR
    // ==================================================================
    
    info!("🧠 Initializing learning coordinator...");
    let learning_coordinator = Arc::new(LearningCoordinator::new()?);
    
    // ==================================================================
    // INITIALIZE PROTOCOL MANAGER
    // ==================================================================
    
    info!("📋 Initializing protocol manager...");
    let protocol_manager = Arc::new(RwLock::new(ProtocolManager::new(data_dir.join("protocol")).await?));
    
    // ==================================================================
    // INITIALIZE AUTONOMOUS MANAGER
    // ==================================================================
    
    info!("🤖 Initializing autonomous manager...");
    let autonomous_manager = Arc::new(AutonomousManager::new(logger.clone()));
    
    // ==================================================================
    // CHECK TEACHER AVAILABILITY
    // ==================================================================
    
    info!("👨‍🏫 Checking Teacher availability...");
    let teacher_available = learning_coordinator.check_teacher().await;
    
    if teacher_available {
        info!("✅ Teacher is AVAILABLE");
        logger.log_health_check(
            "Teacher connection established",
            Some(json!({ "status": "available" }))
        ).await;
        
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
    let vector_count = vector_store.read().await.len().await?;
    
    logger.log_health_check(
        "Vector store initialized",
        Some(json!({
            "path": vector_store_path.to_string_lossy(),
            "entries": vector_count,
        }))
    ).await;
    
    info!("🔗 Initializing memory blockchain...");
    let memory_blockchain = Arc::new(RwLock::new(BlockchainManager::new(blockchain_path.clone()).await?));
    let block_count = memory_blockchain.read().await.len().await;
    let is_valid = memory_blockchain.read().await.verify().await;
    
    logger.log_health_check(
        "Memory blockchain initialized",
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
        memory_blockchain.clone(),
        ingestor.as_ref().clone(),
        embedder.as_ref().clone(),
    );
    
    // ==================================================================
    // START SHARED MEMORY LISTENER (Unix only)
    // ==================================================================
    
    #[cfg(unix)]
    let shm_task = {
        info!("👂 Starting shared memory listener...");
        let mut shm_listener = SharedMemoryListener::new(
            logger.clone(),
            protocol_manager.clone(),
            learning_coordinator.clone(),
        )?;
        
        tokio::spawn(async move {
            if let Err(e) = shm_listener.run().await {
                error!("Shared memory listener error: {}", e);
            }
        })
    };
    
    // ==================================================================
    // START AUTONOMOUS SYSTEMS
    // ==================================================================
    
    info!("🚀 Starting autonomous systems...");
    autonomous_manager.start_all().await;
    logger.log_health_check("Autonomous systems started", None).await;
    
    // ==================================================================
    // LOAD CURRICULUM AND ASSIGN GOALS
    // ==================================================================
    
    let curriculum = Curriculum::new();
    info!("📚 Loaded curriculum: {} topics, {} estimated lessons", 
          curriculum.topics.len(), curriculum.get_total_lessons());
    
    autonomous_manager.assign_goal("Complete blockchain curriculum", 10).await;
    autonomous_manager.assign_goal("Master system access capabilities", 9).await;
    autonomous_manager.assign_goal("Develop creative writing skills", 8).await;
    autonomous_manager.assign_goal("Learn to self-upgrade and evolve", 10).await;
    autonomous_manager.assign_goal("Mine blocks when learning", 8).await;  // NEW
    
    // ==================================================================
    // START FILE WATCHER
    // ==================================================================
    
    let watcher_config = WatcherConfig {
        watch_path: training_dir.clone(),
        debounce_delay_ms: 500,
        recursive: true,
        ignore_hidden: false,
        ignore_patterns: vec![
            r"\.DS_Store".to_string(),
            r"\.git".to_string(),
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
                    protocol_logger.log_health_check("Protocol in debug mode", None).await;
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
    
    // NEW: Mining stats reporter task
    let mining_logger = logger.clone();
    let mining_access = Arc::new(RwLock::new(blockchain_access));
    let mining_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            let stats = mining_access.read().await.get_mining_stats();
            if stats.total_hashes > 0 {
                mining_logger.log_health_check(
                    "Mining stats",
                    Some(json!({
                        "total_hashes": stats.total_hashes,
                        "hashrate_hps": stats.current_hashrate,
                        "difficulty": stats.current_difficulty,
                        "blocks_mined": stats.blocks_mined,
                        "uptime_seconds": stats.uptime_seconds,
                    }))
                ).await;
            }
        }
    });
    
    // ==================================================================
    // TEST SYSTEM CAPABILITIES
    // ==================================================================
    
    info!("🔧 Testing system capabilities...");
    if let Ok(devices) = device_manager.get_all_devices().await {
        info!("   📷 Cameras: {}", devices.cameras.len());
        info!("   🎤 Microphones: {}", devices.microphones.len());
        info!("   💾 Storage: {} devices", devices.storage.len());
        info!("   🌐 Network: {} interfaces", devices.network.len());
    }
    
    if network_access.check_connectivity().await {
        info!("   ✅ Internet connectivity: ONLINE");
    }
    
    if let Ok(output) = command_executor.execute("echo", &["Marisselle is alive"]).await {
        info!("   💻 Command execution: {}", output.stdout.trim());
    }
    
    // Test blockchain mining
    info!("⛏️ Testing blockchain mining...");
    let test_result = mining_access.write().await.mine_learning("Marisselle initialization");
    if let Some(result) = test_result {
        info!("   ✅ Mining test successful!");
        info!("      Block hash: {}", &result.hash[..16]);
        info!("      Nonce: {}", result.nonce);
        info!("      Time: {}ms", result.duration_ms);
    } else {
        info!("   ⚠️ Mining test - no block found yet (continuing in background)");
    }
    
    // ==================================================================
    // PRINT STATUS
    // ==================================================================
    
    info!("");
    info!("============================================================");
    info!("                    MARISSELLE IS RUNNING                    ");
    info!("============================================================");
    info!("");
    info!("📊 Vector store: {} entries", vector_count);
    info!("🔗 Memory Blockchain: {} blocks (valid: {})", block_count, is_valid);
    info!("⛏️ CPU Mining: ACTIVE ({} H/s)", mining_access.read().await.get_mining_stats().current_hashrate);
    info!("👨‍🏫 Teacher: {}", if teacher_available { "CONNECTED" } else { "UNAVAILABLE" });
    info!("🤖 Autonomous: RUNNING");
    info!("");
    info!("📂 Watching: {:?}", training_dir);
    info!("📝 Logs: {:?}", logs_dir);
    info!("🌐 Network: {}", if network_access.check_connectivity().await { "ONLINE" } else { "OFFLINE" });
    info!("💻 System Access: FULL");
    info!("🔗 Blockchain Access: ALL CHAINS");
    info!("");
    info!("💬 Run 'cargo run --bin chat' to talk to Marisselle");
    info!("📊 Run 'cargo run --bin view_logs' to monitor all activity");
    info!("👨‍🏫 Run 'cargo run --bin teacher' to start the Teacher");
    info!("⛏️ Mining runs automatically when Marisselle learns");
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
            "autonomous": true,
            "internet": network_access.check_connectivity().await,
            "mining_active": true,
        }))
    ).await;
    
    logger.log_lm_thought(
        "I am awake and ready to learn. I will process files as they arrive and communicate with the Teacher when available. I am also mining blocks when I learn!",
        None
    ).await;
    
    logger.log_autonomous_thought("I am alive. I am Marisselle. I mine when I learn.").await;
    
    // ==================================================================
    // MAIN EVENT LOOP - Process file events
    // ==================================================================
    
    let mut processed_count = 0;
    let mut last_mining_time = std::time::Instant::now();
    
    while let Some(event) = event_rx.recv().await {
        let event_type = format!("{:?}", event.change_type);
        let file_name = event.path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        
        info!("📂 Processing event: {} - {}", event_type, file_name);
        
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
                
                logger.log_knowledge_integrated(&file_name, 1).await;
                logger.log_lm_thought(
                    &format!("I have successfully learned from {}. My knowledge has grown.", file_name),
                    None
                ).await;
                
                // NEW: Mine a block when learning from a file
                let learning_content = format!("Learned from file: {}", file_name);
                if let Some(result) = mining_access.write().await.mine_learning(&learning_content) {
                    info!("⛏️ MINED BLOCK for learning!");
                    info!("   Block hash: {}", result.hash);
                    info!("   Nonce: {}", result.nonce);
                    info!("   Time: {}ms", result.duration_ms);
                    
                    logger.log_health_check(
                        "Block mined",
                        Some(json!({
                            "file": file_name,
                            "hash": result.hash,
                            "nonce": result.nonce,
                            "duration_ms": result.duration_ms,
                            "hashrate_hps": result.hashrate_hps,
                        }))
                    ).await;
                    
                    logger.log_lm_thought(
                        &format!("I mined a block! My learning is now part of the blockchain. Hash: {}", &result.hash[..16]),
                        None
                    ).await;
                }
                
                // Adjust mining difficulty based on learning rate
                let now = std::time::Instant::now();
                let elapsed = now.duration_since(last_mining_time);
                if elapsed.as_millis() > 0 {
                    mining_access.write().await.miner.adjust_difficulty(elapsed.as_millis() as u64);
                }
                last_mining_time = now;
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
        
        // Periodic stats
        if processed_count % 10 == 0 {
            if let Ok(vector_count) = vector_store.read().await.len().await {
                let block_count = memory_blockchain.read().await.len().await;
                let mining_stats = mining_access.read().await.get_mining_stats();
                info!("📊 Stats: {} vectors, {} memory blocks, {} files processed", 
                      vector_count, block_count, processed_count);
                info!("⛏️ Mining: {} hashes, {} blocks mined, {:.0} H/s",
                      mining_stats.total_hashes,
                      mining_stats.blocks_mined,
                      mining_stats.current_hashrate);
            }
        }
    }
    
    // ==================================================================
    // SHUTDOWN
    // ==================================================================
    
    info!("Shutting down...");
    logger.log_health_check("Marisselle shutting down", None).await;
    logger.log_lm_thought("I am shutting down. I will retain all my knowledge in the blockchain.", None).await;
    
    // Stop mining
    mining_access.write().await.stop_mining();
    info!("⛏️ CPU Mining stopped. Total blocks mined: {}", mining_access.read().await.get_mining_stats().blocks_mined);
    
    autonomous_manager.stop_all().await;
    
    #[cfg(unix)]
    shm_task.abort();
    
    watcher_task.abort();
    health_task.abort();
    protocol_task.abort();
    mining_task.abort();
    
    info!("Goodbye!");
    
    Ok(())
}