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
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tracing::{info, error, warn};
use tracing_subscriber;

use self_evolving_lm::learning::{
    AmoralTeacherOrchestrator, 
    Curriculum, 
    HealthStatus,
};

#[cfg(unix)]
use self_evolving_lm::learning::amoral_teacher::shared_memory::SharedMemoryChannel;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    info!("=========================================");
    info!("AMORAL TEACHER - DEEPSEEK ORCHESTRATOR");
    info!("=========================================");
    
    // Load API key from environment
    let api_key = std::env::var("DEEPSEEK_API_KEY")
        .expect("DEEPSEEK_API_KEY environment variable not set");
    
    // Load curriculum
    let curriculum = Curriculum::new();
    curriculum.print_summary();
    
    // Set up training directory
    let training_dir = PathBuf::from("training_data");
    std::fs::create_dir_all(&training_dir)?;
    
    // Extract all topics in priority order
    let mut all_topics: Vec<String> = curriculum
        .topics
        .iter()
        .flat_map(|topic| {
            let mut t = vec![topic.name.clone()];
            t.extend(topic.sub_topics.clone());
            t
        })
        .collect();
    
    info!("Total topics to teach: {}", all_topics.len());
    
    // Create teacher orchestrator
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
        let teacher_deepseek = teacher.deepseek.clone();
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
                        info!("Received question from Marisselle (seq {}): {}", seq, &data[9..]);
                        
                        // Extract the actual question
                        let question = data[9..].trim();
                        
                        // Ask DeepSeek
                        match teacher_deepseek.answer_question(question).await {
                            Ok(answer) => {
                                // Write response back to shared memory
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
    
    // Shutdown
    teacher.shutdown().await;
    
    #[cfg(unix)]
    shm_listener.abort();
    
    info!("Teacher orchestrator completed.");
    
    result
}
