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

// ======================================================================
// TEACHER CLIENT
// ======================================================================

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
    
    /// Ask for clarification on a specific topic
    pub async fn ask_clarification(&self, topic: &str, confusion: &str) -> Result<String> {
        let question = format!(
            "I am learning about '{}'. I am confused about: {}. Please explain this in simpler terms with examples.",
            topic, confusion
        );
        self.ask_teacher(&question).await
    }
    
    /// Ask for a deeper explanation of a concept
    pub async fn ask_deeper(&self, concept: &str) -> Result<String> {
        let question = format!(
            "Please provide a deep, comprehensive explanation of '{}'. Include technical details, examples, and code if applicable.",
            concept
        );
        self.ask_teacher(&question).await
    }
    
    /// Ask for code generation
    pub async fn ask_code(&self, language: &str, task: &str) -> Result<String> {
        let question = format!(
            "Write {} code that {}. Provide complete, runnable code with comments.",
            language, task
        );
        self.ask_teacher(&question).await
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
                "Not clear",
                "Unclear",
                "Confusing",
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
    
    /// Extract the topic of confusion from a response
    pub fn extract_confusion_topic(&self, response: &str, context: Option<&str>) -> (String, String) {
        let topic = context.unwrap_or("unknown topic").to_string();
        let confusion = response.to_string();
        (topic, confusion)
    }
    
    /// When confused, ask the Teacher for help
    pub async fn resolve_confusion(&self, topic: &str, confusion: &str) -> Result<String> {
        info!("Marisselle is confused about '{}': {}", topic, confusion);
        self.teacher_client.ask_clarification(topic, confusion).await
    }
    
    /// Add a custom confusion phrase
    pub fn add_confusion_phrase(&mut self, phrase: &'static str) {
        self.confusion_phrases.push(phrase);
    }
}

// ======================================================================
// LEARNING COORDINATOR - Integrates with the main learning loop
// ======================================================================

pub struct LearningCoordinator {
    teacher_client: TeacherClient,
    confusion_detector: ConfusionDetector,
    teacher_available: Arc<tokio::sync::RwLock<bool>>,
}

impl LearningCoordinator {
    pub fn new() -> Result<Self> {
        let teacher_client = TeacherClient::new()?;
        let confusion_detector = ConfusionDetector::new(teacher_client.clone());
        
        Ok(Self {
            teacher_client,
            confusion_detector,
            teacher_available: Arc::new(tokio::sync::RwLock::new(false)),
        })
    }
    
    /// Check teacher availability and update status
    pub async fn check_teacher(&self) -> bool {
        let available = self.teacher_client.is_teacher_available().await;
        {
            let mut status = self.teacher_available.write().await;
            *status = available;
        }
        
        if available {
            info!("Teacher is AVAILABLE");
        } else {
            warn!("Teacher is UNAVAILABLE");
        }
        
        available
    }
    
    /// Get teacher availability status
    pub async fn is_teacher_available(&self) -> bool {
        *self.teacher_available.read().await
    }
    
    /// Process a potential confusion in Marisselle's response
    pub async fn handle_confusion(
        &self, 
        response: &str, 
        context: Option<&str>
    ) -> Option<String> {
        if !self.confusion_detector.is_confused(response) {
            return None;
        }
        
        if !self.is_teacher_available().await {
            warn!("Marisselle is confused but Teacher is unavailable");
            return None;
        }
        
        let (topic, confusion) = self.confusion_detector.extract_confusion_topic(response, context);
        
        match self.confusion_detector.resolve_confusion(&topic, &confusion).await {
            Ok(clarification) => {
                info!("Teacher provided clarification for: {}", topic);
                Some(clarification)
            }
            Err(e) => {
                error!("Failed to get clarification from Teacher: {}", e);
                None
            }
        }
    }
    
    /// Ask the teacher for additional information on a topic
    pub async fn request_deeper_knowledge(&self, topic: &str) -> Result<String> {
        if !self.is_teacher_available().await {
            return Err(anyhow!("Teacher is unavailable"));
        }
        
        self.teacher_client.ask_deeper(topic).await
    }
    
    /// Get the teacher client for direct use
    pub fn teacher_client(&self) -> TeacherClient {
        self.teacher_client.clone()
    }
    
    /// Get the confusion detector for direct use
    pub fn confusion_detector(&self) -> &ConfusionDetector {
        &self.confusion_detector
    }
}

impl Clone for LearningCoordinator {
    fn clone(&self) -> Self {
        Self {
            teacher_client: self.teacher_client.clone(),
            confusion_detector: ConfusionDetector::new(self.teacher_client.clone()),
            teacher_available: Arc::clone(&self.teacher_available),
        }
    }
}

// ======================================================================
// TESTS
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_confusion_detection() {
        let client = TeacherClient::new().unwrap();
        let detector = ConfusionDetector::new(client);
        
        assert!(detector.is_confused("I don't understand this concept"));
        assert!(detector.is_confused("Can you explain how this works?"));
        assert!(detector.is_confused("I'm confused about blockchain"));
        assert!(!detector.is_confused("The answer is 42"));
        assert!(!detector.is_confused("Blockchain is a distributed ledger"));
    }
    
    #[tokio::test]
    async fn test_learning_coordinator_creation() {
        let coordinator = LearningCoordinator::new();
        assert!(coordinator.is_ok());
    }
}
