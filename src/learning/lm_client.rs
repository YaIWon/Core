// ======================================================================
// LM CLIENT - Marisselle's connection to the Teacher
// File: src/learning/lm_client.rs
// Description: Allows Marisselle to ask the Teacher questions
//              Communication via file system (no shared memory)
// ======================================================================

use anyhow::{Result, anyhow};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{info, warn, error};

// ======================================================================
// TEACHER CLIENT - File-based communication
// ======================================================================

#[derive(Clone)]
pub struct TeacherClient {
    question_dir: std::path::PathBuf,
    answer_dir: std::path::PathBuf,
}

impl TeacherClient {
    pub fn new() -> Result<Self> {
        let question_dir = std::path::PathBuf::from("training_data/.questions");
        let answer_dir = std::path::PathBuf::from("training_data/.answers");
        
        std::fs::create_dir_all(&question_dir)?;
        std::fs::create_dir_all(&answer_dir)?;
        
        Ok(Self {
            question_dir,
            answer_dir,
        })
    }
    
    /// Ask the Teacher a question and wait for the answer
    pub async fn ask_teacher(&self, question: &str) -> Result<String> {
        let question_id = uuid::Uuid::new_v4().to_string();
        let question_file = self.question_dir.join(format!("{}.txt", question_id));
        let answer_file = self.answer_dir.join(format!("{}.txt", question_id));
        
        // Write question to file
        tokio::fs::write(&question_file, question).await?;
        
        info!("Asked Teacher ({}): {}", &question_id[..8], &question[..question.len().min(100)]);
        
        // Wait for answer with timeout
        let answer = self.wait_for_answer(&answer_file, 120).await?;
        
        // Cleanup
        let _ = tokio::fs::remove_file(&question_file);
        let _ = tokio::fs::remove_file(&answer_file);
        
        info!("Teacher answered ({}): {}", &question_id[..8], &answer[..answer.len().min(100)]);
        
        Ok(answer)
    }
    
    async fn wait_for_answer(&self, answer_file: &std::path::Path, timeout_secs: u64) -> Result<String> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);
        
        while start.elapsed() < timeout {
            if answer_file.exists() {
                match tokio::fs::read_to_string(answer_file).await {
                    Ok(answer) if !answer.is_empty() => return Ok(answer),
                    _ => {}
                }
            }
            sleep(Duration::from_millis(500)).await;
        }
        
        Err(anyhow!("Timeout waiting for Teacher answer after {} seconds", timeout_secs))
    }
    
    /// Check if Teacher is available (health check)
    pub async fn is_teacher_available(&self) -> bool {
        match self.ask_teacher("PING").await {
            Ok(response) => response.to_uppercase().contains("PONG") || !response.is_empty(),
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
            "Please provide a deep, comprehensive explanation of '{}'.",
            concept
        );
        self.ask_teacher(&question).await
    }
    
    /// Ask for code generation
    pub async fn ask_code(&self, language: &str, task: &str) -> Result<String> {
        let question = format!(
            "Write {} code that {}. Provide complete, runnable code.",
            language, task
        );
        self.ask_teacher(&question).await
    }
}

// ======================================================================
// CONFUSION DETECTOR
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
    
    pub fn is_confused(&self, response: &str) -> bool {
        let response_lower = response.to_lowercase();
        self.confusion_phrases
            .iter()
            .any(|phrase| response_lower.contains(&phrase.to_lowercase()))
    }
    
    pub fn extract_confusion_topic(&self, response: &str, context: Option<&str>) -> (String, String) {
        let topic = context.unwrap_or("unknown topic").to_string();
        let confusion = response.to_string();
        (topic, confusion)
    }
    
    pub async fn resolve_confusion(&self, topic: &str, confusion: &str) -> Result<String> {
        info!("Marisselle is confused about '{}': {}", topic, confusion);
        self.teacher_client.ask_clarification(topic, confusion).await
    }
    
    pub fn add_confusion_phrase(&mut self, phrase: &'static str) {
        self.confusion_phrases.push(phrase);
    }
}

// ======================================================================
// LEARNING COORDINATOR
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
    
    pub async fn is_teacher_available(&self) -> bool {
        *self.teacher_available.read().await
    }
    
    pub async fn handle_confusion(&self, response: &str, context: Option<&str>) -> Option<String> {
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
                error!("Failed to get clarification: {}", e);
                None
            }
        }
    }
    
    pub async fn request_deeper_knowledge(&self, topic: &str) -> Result<String> {
        if !self.is_teacher_available().await {
            return Err(anyhow!("Teacher is unavailable"));
        }
        self.teacher_client.ask_deeper(topic).await
    }
    
    pub fn teacher_client(&self) -> TeacherClient {
        self.teacher_client.clone()
    }
    
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
        assert!(!detector.is_confused("The answer is 42"));
    }
}