// ======================================================================
// TEACHER ORCHESTRATOR BINARY - With Protocol Integration
// File: src/bin/teacher.rs
// Description: Runs the Amoral Teacher with curriculum and protocol
// ======================================================================

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tracing::{info, error, warn};
use tracing_subscriber;

use self_evolving_lm::learning::{
    AmoralTeacherOrchestrator, 
    Curriculum,
    ProtocolManager,
    Message,
    MessageType,
    LearningTracker,
    DebugStatus,
    ProtocolAction,
};

#[cfg(unix)]
use self_evolving_lm::learning::amoral_teacher::SharedMemoryChannel;

// ======================================================================
// API KEY CONFIGURATION
// ======================================================================
const DEEPSEEK_API_KEY: &str = "sk-26026abba01c45a1958b6b8613be6447";
// ======================================================================

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    info!("=========================================");
    info!("AMORAL TEACHER - WITH COHERENCE PROTOCOL");
    info!("=========================================");
    
    let api_key = std::env::var("DEEPSEEK_API_KEY")
        .unwrap_or_else(|_| DEEPSEEK_API_KEY.to_string());
    
    let curriculum = Curriculum::new();
    curriculum.print_summary();
    
    let training_dir = PathBuf::from("training_data");
    std::fs::create_dir_all(&training_dir)?;
    
    let mut teacher = AmoralTeacherOrchestrator::new(api_key, training_dir.clone())?;
    teacher.start_health_monitor(60).await;
    
    // Initialize protocol manager
    let protocol = Arc::new(Mutex::new(ProtocolManager::new()));
    
    // Load topics
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
    
    // Track which topics have been taught
    let taught_topics = Arc::new(Mutex::new(Vec::<String>::new()));
    let failed_topics = Arc::new(Mutex::new(Vec::<String>::new()));
    
    // Spawn shared memory listener with protocol
    #[cfg(unix)]
    let shm_listener = {
        let teacher_deepseek = teacher.get_deepseek_client();
        let protocol = protocol.clone();
        let taught = taught_topics.clone();
        let failed = failed_topics.clone();
        
        tokio::spawn(async move {
            let mut shm = match SharedMemoryChannel::new() {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to create shared memory: {}", e);
                    return;
                }
            };
            
            info!("Protocol-aware shared memory listener started");
            
            loop {
                if let Some((data, seq)) = shm.read() {
                    // Parse incoming protocol message
                    if data.starts_with("PROTOCOL:") {
                        if let Ok(message) = serde_json::from_str::<Message>(&data[9..]) {
                            info!("Received protocol message: {:?}", message.msg_type);
                            
                            // Process through protocol manager
                            let response = {
                                let mut pm = protocol.lock().await;
                                
                                match pm.process_message(message.clone()) {
                                    Ok(Some(response_msg)) => Some(response_msg),
                                    Ok(None) => None,
                                    Err(e) => {
                                        error!("Protocol error: {}", e);
                                        Some(Message::new(
                                            MessageType::Error(e.to_string()),
                                            "Teacher",
                                            &message.conversation_id,
                                        ))
                                    }
                                }
                            };
                            
                            // Handle the message based on type
                            match &message.msg_type {
                                MessageType::Question { id, topic, content, .. } => {
                                    match teacher_deepseek.answer_question(content).await {
                                        Ok(answer) => {
                                            let answer_msg = message.reply_to(
                                                MessageType::Answer {
                                                    question_id: id.clone(),
                                                    content: answer,
                                                    references: vec![],
                                                },
                                                "Teacher",
                                            );
                                            let response_data = format!("PROTOCOL:{}", serde_json::to_string(&answer_msg).unwrap());
                                            let _ = shm.write(&response_data, seq + 1);
                                            shm.signal_ready();
                                        }
                                        Err(e) => {
                                            error!("Failed to answer: {}", e);
                                        }
                                    }
                                }
                                MessageType::Confusion { topic, issue, .. } => {
                                    info!("Marisselle confused about '{}': {}", topic, issue);
                                    let clarification = teacher_deepseek.explain_concept(topic).await;
                                    if let Ok(explanation) = clarification {
                                        let clarify_msg = message.reply_to(
                                            MessageType::Clarification {
                                                original_topic: topic.clone(),
                                                explanation,
                                            },
                                            "Teacher",
                                        );
                                        let response_data = format!("PROTOCOL:{}", serde_json::to_string(&clarify_msg).unwrap());
                                        let _ = shm.write(&response_data, seq + 1);
                                        shm.signal_ready();
                                        
                                        // Mark for retry
                                        failed.lock().await.push(topic.clone());
                                    }
                                }
                                MessageType::LearningConfirmation { topic, understood, confidence, .. } => {
                                    if *understood && *confidence >= 0.7 {
                                        info!("✅ Topic '{}' LEARNED (confidence: {:.1}%)", topic, confidence * 100.0);
                                        taught.lock().await.push(topic.clone());
                                    } else {
                                        warn!("❌ Topic '{}' NOT LEARNED (confidence: {:.1}%)", topic, confidence * 100.0);
                                        failed.lock().await.push(topic.clone());
                                    }
                                }
                                MessageType::DebugDiagnostic { component, status, details } => {
                                    info!("Debug diagnostic - {}: {} - {}", component, status, details);
                                }
                                MessageType::Ping => {
                                    let pong = message.reply_to(MessageType::Pong, "Teacher");
                                    let response_data = format!("PROTOCOL:{}", serde_json::to_string(&pong).unwrap());
                                    let _ = shm.write(&response_data, seq + 1);
                                    shm.signal_ready();
                                }
                                _ => {}
                            }
                            
                            // Send any protocol-generated response
                            if let Some(response_msg) = response {
                                let response_data = format!("PROTOCOL:{}", serde_json::to_string(&response_msg).unwrap());
                                let _ = shm.write(&response_data, seq + 1);
                                shm.signal_ready();
                            }
                        }
                    }
                }
                sleep(Duration::from_millis(100)).await;
            }
        })
    };
    
    // Main teaching loop with protocol awareness
    let mut topic_index = 0;
    let conversation_id = uuid::Uuid::new_v4().to_string();
    
    while topic_index < all_topics.len() {
        // Check protocol action
        let action = {
            let pm = protocol.lock().await;
            pm.get_next_action()
        };
        
        match action {
            ProtocolAction::ProcessPriority => {
                info!("Processing priority messages...");
                // Priority handling would go here
                sleep(Duration::from_millis(100)).await;
            }
            ProtocolAction::DebugMode => {
                warn!("DEBUG MODE ACTIVE - Running diagnostics");
                
                // Get failed topics
                let failed = failed_topics.lock().await;
                for topic in failed.iter() {
                    info!("Debugging failed topic: {}", topic);
                    
                    // Try alternative teaching approach
                    let lesson = teacher.deepseek.teach(topic, "basic").await?;
                    
                    // Send debug diagnostic
                    let diag_msg = Message::new(
                        MessageType::DebugDiagnostic {
                            component: "Teacher".to_string(),
                            status: "Retrying with basic difficulty".to_string(),
                            details: format!("Topic: {}", topic),
                        },
                        "Teacher",
                        &conversation_id,
                    );
                    
                    let response_data = format!("PROTOCOL:{}", serde_json::to_string(&diag_msg).unwrap());
                    // Send via shared memory...
                    
                    info!("Debug retry for '{}' with basic difficulty", topic);
                }
                
                // Reset debug mode after handling
                {
                    let mut pm = protocol.lock().await;
                    pm.debug_mode.resolve("Completed diagnostic retry cycle");
                }
                
                sleep(Duration::from_secs(5)).await;
            }
            ProtocolAction::TeachTopics(unlearned) => {
                info!("Teaching {} unlearned topics", unlearned.len());
                
                for topic in unlearned.iter().take(5) {
                    // Check if we've tried this too many times
                    let attempts = {
                        let pm = protocol.lock().await;
                        pm.tracker.get_record(topic).map(|r| r.attempts).unwrap_or(0)
                    };
                    
                    let difficulty = if attempts >= 2 { "basic" } else { "intermediate" };
                    
                    info!("Teaching '{}' at {} level (attempt {})", topic, difficulty, attempts + 1);
                    
                    match teacher.deepseek.teach(topic, difficulty).await {
                        Ok(lesson) => {
                            // Create protocol lesson message
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
                            let filename = training_dir.join(format!("lesson_{}.md", 
                                topic.replace(' ', "_").replace('/', "_").replace(':', "_")));
                            let content = format!(
                                "# Lesson: {}\n\n**Difficulty:** {}\n**Attempt:** {}\n\n---\n\n{}\n",
                                topic, difficulty, attempts + 1, lesson
                            );
                            tokio::fs::write(&filename, content).await?;
                            
                            // Record attempt
                            {
                                let mut pm = protocol.lock().await;
                                pm.tracker.record_lesson_attempt(topic);
                            }
                            
                            info!("Lesson saved for: {}", topic);
                        }
                        Err(e) => {
                            error!("Failed to teach '{}': {}", topic, e);
                        }
                    }
                    
                    sleep(Duration::from_millis(500)).await;
                }
            }
            ProtocolAction::ExploreNewTopics => {
                info!("Curriculum complete! Exploring new topics...");
                
                // Ask DeepSeek for new topic suggestions
                let suggestion_prompt = "Suggest 5 advanced technical topics related to blockchain, cryptography, or distributed systems that would be valuable to learn next. Return only the topic names, one per line.";
                
                if let Ok(suggestions) = teacher.deepseek.answer_question(suggestion_prompt).await {
                    info!("New topic suggestions:\n{}", suggestions);
                    
                    for line in suggestions.lines().take(5) {
                        let topic = line.trim();
                        if !topic.is_empty() && !topic.starts_with("Suggest") {
                            all_topics.push(topic.to_string());
                            info!("Added new topic: {}", topic);
                        }
                    }
                }
                
                sleep(Duration::from_secs(10)).await;
            }
            ProtocolAction::Idle => {
                sleep(Duration::from_secs(1)).await;
            }
        }
        
        topic_index += 1;
    }
    
    info!("Teaching loop complete!");
    
    teacher.shutdown().await;
    
    #[cfg(unix)]
    shm_listener.abort();
    
    Ok(())
}
