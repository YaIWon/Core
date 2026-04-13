// ======================================================================
// LM CLIENT - Marisselle's connection to the Teacher
// File: src/learning/lm_client.rs
// Description: Allows Marisselle to ask the Teacher questions via shared memory
//              and receive answers. Used when Marisselle is confused or needs
//              clarification on a topic.
// ======================================================================

use anyhow::{Result, anyhow};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tracing::{info, warn, error, debug};

#[cfg(unix)]
use crate::learning::amoral_teacher::SharedMemoryChannel;

#[cfg(not(unix))]
use crate::learning::amoral_teacher::SharedMemoryChannel;

pub struct TeacherClient {
    shm: Arc<Mutex<SharedMemoryChannel>>,
    sequence: Arc<tokio::sync::RwLock<u64>>,
}

impl TeacherClient {
    pub fn new() -> Result<Self> {
        Ok(Self {
            shm: Arc::new(Mutex::new(SharedMemoryChannel::new()?)),
            sequence: Arc::new(tokio::sync::RwLock::new(0)),
        })
    }
    
    /// Ask the Teacher a question and wait for the answer
    pub async fn ask_teacher(&self, question: &str) -> Result<String> {
        let seq = {
            let mut s = self.sequence.write().await;
            *s += 1;
            *s
        };
        
        // Write question to shared memory
        {
            let mut shm = self.shm.lock().await;
            let message = format!("QUESTION: {}", question);
            shm.write(&message, seq)?;
            shm.signal_ready();
        }
        
        info!("Asked Teacher (seq {}): {}", seq, &question[..question.len().min(100)]);
        
        // Wait for answer with timeout
        let answer = self.wait_for_answer(seq, 60).await?;
        
        info!("Teacher answered (seq {}): {}", seq, &answer[..answer.len().min(100)]);
        
        Ok(answer)
    }
    
    async fn wait_for_answer(&self, expected_seq: u64, timeout_secs: u64) -> Result<String> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);
        
        while start.elapsed() < timeout {
            {
                let shm = self.shm.lock().await;
                if let Some((data, seq)) = shm.read() {
                    if seq > expected_seq && data.starts_with("ANSWER:") {
                        let answer = data[7..].trim().to_string();
                        return Ok(answer);
                    }
                }
            }
            sleep(Duration::from_millis(100)).await;
        }
        
        Err(anyhow!("Timeout waiting for Teacher answer after {} seconds", timeout_secs))
    }
    
    /// Check if Teacher is available (health check)
    pub async fn is_teacher_available(&self) -> bool {
        match self.ask_teacher("PING").await {
            Ok(response) => response.contains("PONG") || !response.is_empty(),
            Err(_) => false,
        }
    }
}

impl Clone for TeacherClient {
    fn clone(&self) -> Self {
        Self {
            shm: Arc::clone(&self.shm),
            sequence: Arc::clone(&self.sequence),
        }
    }
}

// ======================================================================
// CONFUSION DETECTOR - When Marisselle doesn't understand something
// ======================================================================

pub struct ConfusionDetector {
    confusion_phrases: Vec<&'static str>,
    teacher_client: TeacherClient,
}

impl ConfusionDetector {
    pub fn new(teacher_client: TeacherClient) -> Self {
        Self {
            confusion_phrases: vec![
                "I don't understand",
                "I'm confused",
                "Can you explain",
                "What does that mean",
                "I need clarification",
                "I'm not sure",
                "Help me understand",
                "Explain",
                "Clarify",
                "What is",
                "How does",
                "Why is",
                "Unknown",
                "Don't know",
            ],
            teacher_client,
        }
    }
    
    /// Check if Marisselle's response indicates confusion
    pub fn is_confused(&self, response: &str) -> bool {
        let response_lower = response.to_lowercase();
        self.confusion_phrases
            .iter()
            .any(|phrase| response_lower.contains(&phrase.to_lowercase()))
    }
    
    /// When confused, ask the Teacher for help
    pub async fn resolve_confusion(&self, topic: &str, confusion: &str) -> Result<String> {
        let question = format!(
            "I am confused about '{}'. Specifically: {}. Please explain in simpler terms.",
            topic, confusion
        );
        
        self.teacher_client.ask_teacher(&question).await
    }
}
