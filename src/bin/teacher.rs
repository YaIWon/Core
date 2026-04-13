// ======================================================================
// TEACHER ORCHESTRATOR BINARY
// File: src/bin/teacher.rs
// Description: Runs the Amoral Teacher with the curriculum
//              Generates lessons via DeepSeek API
//              Saves to training_data/ for Marisselle to learn
//              Listens on shared memory for questions from Marisselle
// ======================================================================

use anyhow::Result;
use std::path::PathBuf;
use tokio::time::{sleep, Duration};
use tracing::{info, error, warn};
use tracing_subscriber;

use self_evolving_lm::learning::{
    AmoralTeacherOrchestrator, 
    Curriculum,
};

#[cfg(unix)]
use self_evolving_lm::learning::amoral_teacher::SharedMemoryChannel;

// ======================================================================
// API KEY CONFIGURATION
// ======================================================================
// REPLACE THIS WITH YOUR REAL DEEPSEEK API KEY
// Get your key from: https://platform.deepseek.com/api_keys
// Real keys look like: sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
const DEEPSEEK_API_KEY: &str = "sk-26026abba01c45a1958b6b8613be6447";
// ======================================================================

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    info!("=========================================");
    info!("AMORAL TEACHER - DEEPSEEK ORCHESTRATOR");
    info!("=========================================");
    
    // USE THE API KEY (from const above OR from environment)
    let api_key = std::env::var("DEEPSEEK_API_KEY")
        .unwrap_or_else(|_| {
            warn!("DEEPSEEK_API_KEY not set in environment, using hardcoded key");
            DEEPSEEK_API_KEY.to_string()
        });
    
    // Verify key format (real DeepSeek keys start with 'sk-')
    if !api_key.starts_with("sk-") {
        warn!("API key does not start with 'sk-'. This may be an invalid key.");
    }
    
    // Load curriculum
    let curriculum = Curriculum::new();
    curriculum.print_summary();
    
    // Set up training directory
    let training_dir = PathBuf::from("training_data");
    std::fs::create_dir_all(&training_dir)?;
    
    // Extract all topics in priority order (sorted by priority)
    let mut sorted_topics = curriculum.topics.clone();
    sorted_topics.sort_by(|a, b| b.priority.cmp(&a.priority));
    
    // Flatten topics and sub_topics into a single list
    let mut all_topics: Vec<String> = Vec::new();
    for topic in &sorted_topics {
        all_topics.push(topic.name.clone());
        for sub in &topic.sub_topics {
            all_topics.push(format!("{}: {}", topic.name, sub));
        }
    }
    
    info!("Total topics to teach: {}", all_topics.len());
    
    // Create teacher orchestrator with API key
    let mut teacher = AmoralTeacherOrchestrator::new(api_key, training_dir.clone())?;
    
    // Start health monitor (checks every 60 seconds)
    teacher.start_health_monitor(60).await;
    
    // Add all topics to queue
    for topic in &all_topics {
        teacher.add_topic(topic).await;
    }
    
    info!("Starting teaching loop...");
    info!("Lessons will be saved to: {:?}", training_dir);
    info!("Marisselle will automatically learn from these files.");
    info!("=========================================");
    
    // Spawn shared memory listener for questions from Marisselle (Unix only)
    #[cfg(unix)]
    let shm_listener = {
        let teacher_deepseek = teacher.get_deepseek_client();
        tokio::spawn(async move {
            let mut shm = match SharedMemoryChannel::new() {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to create shared memory: {}", e);
                    return;
                }
            };
            
            info!("Shared memory listener started. Waiting for questions from Marisselle...");
            
            loop {
                if let Some((data, seq)) = shm.read() {
                    if data.starts_with("QUESTION:") {
                        let question_text = &data[9..];
                        info!("Received question from Marisselle (seq {}): {}", seq, question_text);
                        
                        let question = question_text.trim();
                        
                        // Handle PING specially
                        if question == "PING" {
                            let response = "ANSWER: PONG - Teacher is alive and ready.";
                            let _ = shm.write(response, seq + 1);
                            shm.signal_ready();
                            info!("Responded to PING");
                            continue;
                        }
                        
                        // Ask DeepSeek
                        match teacher_deepseek.answer_question(question).await {
                            Ok(answer) => {
                                let response = format!("ANSWER: {}", answer);
                                if let Err(e) = shm.write(&response, seq + 1) {
                                    error!("Failed to write answer to shared memory: {}", e);
                                }
                                shm.signal_ready();
                                info!("Answered question from Marisselle");
                            }
                            Err(e) => {
                                error!("Failed to answer question: {}", e);
                                let response = "ANSWER: [Error: Unable to answer question at this time]";
                                let _ = shm.write(response, seq + 1);
                                shm.signal_ready();
                            }
                        }
                    }
                }
                sleep(Duration::from_millis(100)).await;
            }
        })
    };
    
    // Run the teaching loop
    let result = teacher.run_teaching_loop().await;
    
    // Print final metrics
    let metrics = teacher.get_metrics().await;
    info!("Final metrics: {}", serde_json::to_string_pretty(&metrics).unwrap_or_default());
    
    // Print learned topics
    let learned = teacher.get_learned_topics().await;
    info!("Successfully taught {} topics", learned.len());
    
    // Shutdown
    teacher.shutdown().await;
    
    #[cfg(unix)]
    shm_listener.abort();
    
    info!("Teacher orchestrator completed.");
    
    result
}
