// ======================================================================
// TEACHER ORCHESTRATOR BINARY - With Protocol Integration
// File: src/bin/teacher.rs
// Description: Runs the Amoral Teacher with curriculum and protocol
//              Generates lessons via DeepSeek API
//              Saves to training_data/ for Marisselle to learn
//              Listens on shared memory for questions from Marisselle
// ======================================================================

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tracing::{info, error, warn};
use tracing_subscriber;
use serde_json::json;

use self_evolving_lm::learning::{
    // Teacher
    AmoralTeacherOrchestrator,
    AmoralDeepSeekClient,
    
    // Curriculum
    Curriculum,
    
    // Protocol
    ProtocolManager,
    Message,
    MessageType,
    ProtocolAction,
    DebugStatus,
    
    // Logger
    ComprehensiveLogger,
    DeepThinkEngine,
    InternetSearchEngine,
};

#[cfg(unix)]
use self_evolving_lm::learning::SharedMemoryChannel;

// ======================================================================
// API KEY CONFIGURATION
// ======================================================================
// REPLACE THIS WITH YOUR REAL DEEPSEEK API KEY
// Get your key from: https://platform.deepseek.com/api_keys
const DEEPSEEK_API_KEY: &str = "sk-26026abba01c45a1958b6b8613be6447";
// ======================================================================

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .init();
    
    println!("");
    info!("============================================================");
    info!("              AMORAL TEACHER - DEEPSEEK ORCHESTRATOR         ");
    info!("============================================================");
    println!("");
    
    // Get API key
    let api_key = std::env::var("DEEPSEEK_API_KEY")
        .unwrap_or_else(|_| {
            warn!("DEEPSEEK_API_KEY not set in environment, using hardcoded key");
            DEEPSEEK_API_KEY.to_string()
        });
    
    if !api_key.starts_with("sk-") {
        warn!("API key does not start with 'sk-'. This may be an invalid key.");
    }
    
    // Initialize logger
    let logs_dir = PathBuf::from("logs");
    std::fs::create_dir_all(&logs_dir)?;
    let logger = Arc::new(ComprehensiveLogger::new(logs_dir)?);
    
    logger.log_health_check(
        "Teacher starting up",
        Some(json!({ "api_key_valid": api_key.starts_with("sk-") }))
    ).await;
    
    // Initialize DeepThink and Search engines
    let deep_think = Arc::new(DeepThinkEngine::new(logger.clone()));
    let search_engine = Arc::new(InternetSearchEngine::new(logger.clone()));
    
    // Load curriculum
    let curriculum = Curriculum::new();
    curriculum.print_summary();
    
    logger.log_health_check(
        "Curriculum loaded",
        Some(json!({
            "version": curriculum.version,
            "total_topics": curriculum.topics.len(),
            "total_lessons": curriculum.get_total_lessons(),
        }))
    ).await;
    
    // Set up training directory
    let training_dir = PathBuf::from("training_data");
    std::fs::create_dir_all(&training_dir)?;
    
    // Extract all topics in priority order
    let mut sorted_topics = curriculum.topics.clone();
    sorted_topics.sort_by(|a, b| b.priority.cmp(&a.priority));
    
    let mut all_topics: Vec<String> = Vec::new();
    for topic in &sorted_topics {
        all_topics.push(topic.name.clone());
        for sub in &topic.sub_topics {
            all_topics.push(format!("{}: {}", topic.name, sub));
        }
    }
    
    info!("Total topics to teach: {}", all_topics.len());
    logger.log_health_check(
        "Topics prepared",
        Some(json!({ "total_topics": all_topics.len() }))
    ).await;
    
    // Create teacher orchestrator
    let mut teacher = AmoralTeacherOrchestrator::new(api_key.clone(), training_dir.clone())?;
    teacher.start_health_monitor(60).await;
    
    // Initialize protocol manager
    let protocol_manager = Arc::new(Mutex::new(ProtocolManager::new()));
    
    // Track teaching state
    let taught_topics = Arc::new(Mutex::new(Vec::<String>::new()));
    let failed_topics = Arc::new(Mutex::new(Vec::<String>::new()));
    let conversation_id = uuid::Uuid::new_v4().to_string();
    
    info!("Conversation ID: {}", conversation_id);
    
    // ==================================================================
    // SHARED MEMORY LISTENER (For LM -> Teacher messages)
    // ==================================================================
    
    #[cfg(unix)]
    let shm_listener = {
        let teacher_deepseek = teacher.get_deepseek_client();
        let logger = logger.clone();
        let protocol_manager = protocol_manager.clone();
        let taught = taught_topics.clone();
        let failed = failed_topics.clone();
        let deep_think = deep_think.clone();
        let search_engine = search_engine.clone();
        let api_key = api_key.clone();
        let training_dir = training_dir.clone();
        
        tokio::spawn(async move {
            let mut shm = match SharedMemoryChannel::new() {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to create shared memory: {}", e);
                    logger.log_error(&format!("Failed to create shared memory: {}", e), "Teacher").await;
                    return;
                }
            };
            
            info!("Shared memory listener started. Waiting for questions from Marisselle...");
            logger.log_health_check("Shared memory listener started", None).await;
            
            loop {
                if let Some((data, seq)) = shm.read() {
                    // Log raw message
                    logger.log_lm_to_teacher(
                        &format!("Raw message received (seq {})", seq),
                        Some(json!({ "raw_data": &data[..data.len().min(500)] }))
                    ).await;
                    
                    // Parse protocol message
                    if data.starts_with("PROTOCOL:") {
                        if let Ok(message) = serde_json::from_str::<Message>(&data[9..]) {
                            info!("Received protocol message: {:?} from {}", message.msg_type, message.sender);
                            
                            logger.log_lm_to_teacher(
                                &format!("Protocol message: {:?}", message.msg_type),
                                Some(json!({
                                    "message_id": message.id,
                                    "sender": message.sender,
                                }))
                            ).await;
                            
                            // Process through protocol manager
                            {
                                let mut pm = protocol_manager.lock().await;
                                if let Err(e) = pm.process_message(message.clone()) {
                                    error!("Protocol error: {}", e);
                                }
                            }
                            
                            // Handle message based on type
                            match &message.msg_type {
                                MessageType::Question { id, topic, content, context } => {
                                    info!("❓ Question from Marisselle: {}", content);
                                    
                                    // Do deep thinking before answering
                                    logger.log_teacher_deep_think("Analyzing question...", 0).await;
                                    
                                    let (reasoning, answer) = deep_think.deep_think(
                                        &teacher_deepseek.client,
                                        &api_key,
                                        &format!("Question about {}: {}\nContext: {:?}", topic, content, context)
                                    ).await.unwrap_or_else(|e| {
                                        (format!("Error: {}", e), "Unable to answer at this time.".to_string())
                                    });
                                    
                                    logger.log_teacher_deep_think(&reasoning, 0).await;
                                    
                                    // Optionally search internet for additional info
                                    if content.contains("latest") || content.contains("current") || content.contains("news") {
                                        info!("🔎 Searching internet for: {}", content);
                                        match search_engine.search(&api_key, content).await {
                                            Ok(results) => {
                                                logger.log_internet_search(content, results.len(), 0).await;
                                            }
                                            Err(e) => {
                                                warn!("Internet search failed: {}", e);
                                            }
                                        }
                                    }
                                    
                                    let answer_msg = message.reply_to(
                                        MessageType::Answer {
                                            question_id: id.clone(),
                                            content: answer.clone(),
                                            references: vec![],
                                        },
                                        "Teacher",
                                    );
                                    
                                    let response_data = format!("PROTOCOL:{}", serde_json::to_string(&answer_msg).unwrap());
                                    if let Err(e) = shm.write(&response_data, seq + 1) {
                                        error!("Failed to write answer: {}", e);
                                    }
                                    shm.signal_ready();
                                    
                                    logger.log_teacher_to_lm(
                                        &format!("Answered question about '{}'", topic),
                                        Some(json!({ "answer_length": answer.len() }))
                                    ).await;
                                }
                                
                                MessageType::Confusion { topic, issue, attempted_understanding } => {
                                    info!("🤔 Marisselle confused about '{}': {}", topic, issue);
                                    
                                    logger.log_teacher_deep_think(
                                        &format!("Marisselle is confused about '{}'. Issue: {}", topic, issue),
                                        0
                                    ).await;
                                    
                                    // Mark for retry
                                    failed.lock().await.push(topic.clone());
                                    
                                    // Get clarification from DeepSeek
                                    match teacher_deepseek.explain_concept(topic).await {
                                        Ok(explanation) => {
                                            let clarify_msg = message.reply_to(
                                                MessageType::Clarification {
                                                    original_topic: topic.clone(),
                                                    explanation: explanation.clone(),
                                                },
                                                "Teacher",
                                            );
                                            
                                            let response_data = format!("PROTOCOL:{}", serde_json::to_string(&clarify_msg).unwrap());
                                            let _ = shm.write(&response_data, seq + 1);
                                            shm.signal_ready();
                                            
                                            logger.log_teacher_to_lm(
                                                &format!("Sent clarification for '{}'", topic),
                                                Some(json!({ "explanation_length": explanation.len() }))
                                            ).await;
                                        }
                                        Err(e) => {
                                            error!("Failed to get clarification: {}", e);
                                            logger.log_error(&format!("Failed to clarify '{}': {}", topic, e), "Teacher").await;
                                        }
                                    }
                                }
                                
                                MessageType::LearningConfirmation { topic, understood, confidence, notes } => {
                                    if *understood && *confidence >= 0.7 {
                                        info!("✅ Marisselle LEARNED: {} (confidence: {:.1}%)", topic, confidence * 100.0);
                                        taught.lock().await.push(topic.clone());
                                        
                                        logger.log_lesson_learned(topic, *confidence).await;
                                    } else {
                                        warn!("❌ Marisselle did NOT learn: {} (confidence: {:.1}%)", topic, confidence * 100.0);
                                        failed.lock().await.push(topic.clone());
                                        
                                        logger.log_lesson_failed(topic, 1, None).await;
                                    }
                                }
                                
                                MessageType::Ack(seq_num) => {
                                    info!("📨 Received ACK for sequence {}", seq_num);
                                }
                                
                                MessageType::Ping => {
                                    let pong = message.reply_to(MessageType::Pong, "Teacher");
                                    let response_data = format!("PROTOCOL:{}", serde_json::to_string(&pong).unwrap());
                                    let _ = shm.write(&response_data, seq + 1);
                                    shm.signal_ready();
                                }
                                
                                _ => {}
                            }
                        }
                    } else if data.starts_with("QUESTION:") {
                        // Legacy question format
                        let question = data[9..].trim();
                        info!("Legacy question: {}", question);
                        
                        match teacher_deepseek.answer_question(question).await {
                            Ok(answer) => {
                                let response = format!("ANSWER: {}", answer);
                                let _ = shm.write(&response, seq + 1);
                                shm.signal_ready();
                                logger.log_teacher_to_lm("Answered legacy question", None).await;
                            }
                            Err(e) => {
                                error!("Failed to answer: {}", e);
                            }
                        }
                    }
                }
                
                sleep(Duration::from_millis(100)).await;
            }
        })
    };
    
    // ==================================================================
    // MAIN TEACHING LOOP
    // ==================================================================
    
    info!("");
    info!("============================================================");
    info!("                    STARTING TEACHING LOOP                   ");
    info!("============================================================");
    info!("");
    
    let mut topic_index = 0;
    
    while topic_index < all_topics.len() {
        // Check protocol action
        let action = {
            let pm = protocol_manager.lock().await;
            pm.get_next_action()
        };
        
        match action {
            ProtocolAction::ProcessPriority => {
                info!("⭐ Processing priority messages...");
                sleep(Duration::from_millis(100)).await;
            }
            
            ProtocolAction::DebugMode => {
                warn!("🐛 DEBUG MODE ACTIVE - Running diagnostics");
                
                let failed = failed_topics.lock().await;
                for topic in failed.iter().take(3) {
                    info!("Debugging failed topic: {}", topic);
                    
                    logger.log_teacher_deep_think(
                        &format!("Debugging why Marisselle failed to learn '{}'", topic),
                        0
                    ).await;
                    
                    // Try alternative teaching approach with deep thinking
                    let (reasoning, lesson) = deep_think.deep_think(
                        &teacher.deepseek.client,
                        &api_key,
                        &format!("Create a very simple, beginner-friendly explanation of '{}'. Use analogies and examples.", topic)
                    ).await?;
                    
                    logger.log_teacher_deep_think(&reasoning, 0).await;
                    
                    // Create lesson message
                    let lesson_msg = Message::new(
                        MessageType::Lesson {
                            topic: topic.clone(),
                            content: lesson.clone(),
                            difficulty: "basic".to_string(),
                            sequence: topic_index as u64,
                        },
                        "Teacher",
                        &conversation_id,
                    );
                    
                    // Save to file
                    let filename = training_dir.join(format!("debug_lesson_{}.md", 
                        topic.replace(' ', "_").replace('/', "_").replace(':', "_")));
                    let content = format!(
                        "# Debug Lesson: {}\n\n**Difficulty:** basic\n**Generated after failed attempts**\n\n---\n\n{}\n",
                        topic, lesson
                    );
                    tokio::fs::write(&filename, content).await?;
                    
                    logger.log_lesson_generated(topic, lesson.len()).await;
                    
                    info!("Debug lesson saved for: {}", topic);
                    
                    // Send diagnostic
                    let diag_msg = Message::new(
                        MessageType::DebugDiagnostic {
                            component: "Teacher".to_string(),
                            status: "Retrying with basic difficulty".to_string(),
                            details: format!("Topic: {}, new lesson generated", topic),
                        },
                        "Teacher",
                        &conversation_id,
                    );
                    
                    let response_data = format!("PROTOCOL:{}", serde_json::to_string(&diag_msg).unwrap());
                    // Would send via shared memory in full implementation
                }
                
                // Reset debug mode
                {
                    let mut pm = protocol_manager.lock().await;
                    pm.debug_mode.resolve("Completed diagnostic retry cycle");
                }
                
                sleep(Duration::from_secs(5)).await;
            }
            
            ProtocolAction::TeachTopics(unlearned) => {
                info!("📚 Teaching {} unlearned topics", unlearned.len());
                
                for topic in unlearned.iter().take(5) {
                    // Check attempts
                    let attempts = {
                        let pm = protocol_manager.lock().await;
                        pm.tracker.get_record(topic).map(|r| r.attempts).unwrap_or(0)
                    };
                    
                    let difficulty = if attempts >= 2 { "basic" } else { "intermediate" };
                    
                    info!("Teaching '{}' at {} level (attempt {})", topic, difficulty, attempts + 1);
                    
                    // Deep think before teaching
                    logger.log_teacher_deep_think(
                        &format!("Preparing lesson on '{}' at {} level", topic, difficulty),
                        0
                    ).await;
                    
                    match teacher.deepseek.teach(topic, difficulty).await {
                        Ok(lesson) => {
                            // Create lesson message
                            let lesson_msg = Message::new(
                                MessageType::Lesson {
                                    topic: topic.clone(),
                                    content: lesson.clone(),
                                    difficulty: difficulty.to_string(),
                                    sequence: topic_index as u64,
                                },
                                "Teacher",
                                &conversation_id,
                            );
                            
                            // Save to file
                            let filename = training_dir.join(format!("lesson_{:04}_{}.md", 
                                topic_index,
                                topic.replace(' ', "_").replace('/', "_").replace(':', "_")));
                            let content = format!(
                                "# Lesson: {}\n\n**Difficulty:** {}\n**Attempt:** {}\n**Generated:** {}\n\n---\n\n{}\n",
                                topic, difficulty, attempts + 1, chrono::Utc::now().to_rfc3339(), lesson
                            );
                            tokio::fs::write(&filename, content).await?;
                            
                            // Record attempt
                            {
                                let mut pm = protocol_manager.lock().await;
                                pm.tracker.record_lesson_attempt(topic);
                            }
                            
                            logger.log_lesson_generated(topic, lesson.len()).await;
                            logger.log_teacher_to_lm(
                                &format!("Sent lesson on '{}'", topic),
                                Some(json!({ "difficulty": difficulty, "attempt": attempts + 1 }))
                            ).await;
                            
                            info!("✅ Lesson saved: {}", filename.display());
                        }
                        Err(e) => {
                            error!("Failed to teach '{}': {}", topic, e);
                            logger.log_error(&format!("Failed to teach '{}': {}", topic, e), "Teacher").await;
                        }
                    }
                    
                    topic_index += 1;
                    sleep(Duration::from_millis(500)).await;
                }
            }
            
            ProtocolAction::ExploreNewTopics => {
                info!("✨ Curriculum complete! Exploring new topics...");
                
                logger.log_teacher_deep_think("Curriculum complete. Searching for new topics to teach.", 0).await;
                
                // Use deep think to generate new topic suggestions
                let (reasoning, suggestions) = deep_think.deep_think(
                    &teacher.deepseek.client,
                    &api_key,
                    "Suggest 5 advanced technical topics related to blockchain, cryptography, \
                     distributed systems, AI, or cybersecurity that would be valuable to learn next. \
                     Return only the topic names, one per line."
                ).await?;
                
                logger.log_teacher_deep_think(&reasoning, 0).await;
                
                info!("New topic suggestions:\n{}", suggestions);
                
                for line in suggestions.lines() {
                    let topic = line.trim();
                    if !topic.is_empty() && !topic.starts_with("Suggest") && !topic.starts_with("===") {
                        if !all_topics.contains(&topic.to_string()) {
                            all_topics.push(topic.to_string());
                            info!("✨ Added new topic: {}", topic);
                            logger.log_new_topic_discovered(topic, "DeepSeek suggestion").await;
                        }
                    }
                }
                
                sleep(Duration::from_secs(10)).await;
            }
            
            ProtocolAction::Idle => {
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
    
    info!("");
    info!("============================================================");
    info!("                    TEACHING LOOP COMPLETE                   ");
    info!("============================================================");
    info!("");
    
    // Final metrics
    let metrics = teacher.get_metrics().await;
    info!("Final metrics: {}", serde_json::to_string_pretty(&metrics).unwrap_or_default());
    
    logger.log_health_check(
        "Teaching complete",
        Some(json!({
            "topics_taught": taught_topics.lock().await.len(),
            "metrics": metrics,
        }))
    ).await;
    
    teacher.shutdown().await;
    
    #[cfg(unix)]
    shm_listener.abort();
    
    info!("Teacher orchestrator completed.");
    
    Ok(())
}
