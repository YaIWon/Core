// ======================================================================
// LM CLIENT - Marisselle's connection to the Teacher
// File: src/learning/lm_client.rs
// Description: Allows Marisselle to ask the Teacher questions
//              Communication via HTTP (primary) or file system (fallback)
//              Supports Groq API through Teacher endpoint
// ======================================================================

use anyhow::{Result, anyhow};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{info, warn, error};
use serde_json::json;
use reqwest::Client;

// ======================================================================
// TEACHER CLIENT - HTTP with file fallback
// ======================================================================

#[derive(Clone)]
pub struct TeacherClient {
    // File-based communication (fallback)
    question_dir: std::path::PathBuf,
    answer_dir: std::path::PathBuf,
    
    // HTTP communication (primary)
    http_client: Option<Client>,
    http_endpoint: Option<String>,
    http_status_endpoint: Option<String>,
    use_http: bool,
    
    // Configuration
    timeout_seconds: u64,
    max_retries: u32,
}

impl TeacherClient {
    pub fn new() -> Result<Self> {
        let question_dir = std::path::PathBuf::from("training_data/.questions");
        let answer_dir = std::path::PathBuf::from("training_data/.answers");
        
        std::fs::create_dir_all(&question_dir)?;
        std::fs::create_dir_all(&answer_dir)?;
        
        // Try to load teacher configuration
        let config_path = std::path::PathBuf::from("config/teacher.toml");
        let (use_http, http_endpoint, http_status_endpoint, timeout_secs, max_retries) = if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(content) => {
                    if let Ok(config) = toml::from_str::<serde_json::Value>(&content) {
                        let method = config.get("method").and_then(|m| m.as_str()).unwrap_or("file");
                        let endpoint = config.get("endpoint").and_then(|e| e.as_str()).map(|s| s.to_string());
                        let status_endpoint = config.get("status_endpoint").and_then(|e| e.as_str()).map(|s| s.to_string());
                        let timeout = config.get("timeout_seconds").and_then(|t| t.as_u64()).unwrap_or(60);
                        let retries = config.get("max_retries").and_then(|r| r.as_u64()).unwrap_or(3) as u32;
                        (method == "http", endpoint, status_endpoint, timeout, retries)
                    } else {
                        (false, None, None, 60, 3)
                    }
                }
                Err(e) => {
                    warn!("Failed to read teacher config: {}", e);
                    (false, None, None, 60, 3)
                }
            }
        } else {
            (false, None, None, 60, 3)
        };
        
        let http_client = if use_http {
            match Client::builder()
                .timeout(Duration::from_secs(timeout_secs))
                .build() {
                    Ok(client) => Some(client),
                    Err(e) => {
                        warn!("Failed to create HTTP client: {}", e);
                        None
                    }
                }
        } else {
            None
        };
        
        Ok(Self {
            question_dir,
            answer_dir,
            http_client,
            http_endpoint,
            http_status_endpoint,
            use_http: use_http && http_client.is_some(),
            timeout_seconds: timeout_secs,
            max_retries,
        })
    }
    
    /// Ask the Teacher a question (auto-selects HTTP or file)
    pub async fn ask_teacher(&self, question: &str) -> Result<String> {
        if self.use_http {
            self.ask_teacher_http(question).await
        } else {
            self.ask_teacher_file(question).await
        }
    }
    
    /// HTTP-based question to Teacher
    async fn ask_teacher_http(&self, question: &str) -> Result<String> {
        let endpoint = self.http_endpoint.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No HTTP endpoint configured"))?;
        
        let client = self.http_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("HTTP client not initialized"))?;
        
        let request_body = json!({
            "prompt": question,
            "conversation_id": "marisselle_lm",
            "temperature": 0.7,
            "max_tokens": 2048
        });
        
        info!("📤 Asking Teacher via HTTP: {}", &question[..question.len().min(100)]);
        
        let mut last_error = None;
        
        for attempt in 0..self.max_retries {
            if attempt > 0 {
                let backoff = Duration::from_millis(500 * 2_u64.pow(attempt - 1));
                info!("Retry attempt {}/{} after {:?}", attempt + 1, self.max_retries, backoff);
                sleep(backoff).await;
            }
            
            match client
                .post(endpoint)
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<serde_json::Value>().await {
                            Ok(data) => {
                                if let Some(answer) = data.get("answer").and_then(|a| a.as_str()) {
                                    info!("📥 Received answer from Teacher: {} chars", answer.len());
                                    return Ok(answer.to_string());
                                } else if let Some(error) = data.get("error").and_then(|e| e.as_str()) {
                                    last_error = Some(anyhow::anyhow!("Teacher error: {}", error));
                                    continue;
                                } else {
                                    last_error = Some(anyhow::anyhow!("No answer in response"));
                                    continue;
                                }
                            }
                            Err(e) => {
                                last_error = Some(anyhow::anyhow!("Failed to parse response: {}", e));
                                continue;
                            }
                        }
                    } else {
                        let status = response.status();
                        let text = response.text().await.unwrap_or_default();
                        last_error = Some(anyhow::anyhow!("Teacher HTTP error {}: {}", status, text));
                        continue;
                    }
                }
                Err(e) => {
                    last_error = Some(anyhow::anyhow!("HTTP request failed: {}", e));
                    continue;
                }
            }
        }
        
        // If all retries failed, try file-based as fallback
        warn!("HTTP teacher failed after {} attempts, falling back to file-based", self.max_retries);
        self.ask_teacher_file(question).await
    }
    
    /// File-based question (original method - fallback)
    async fn ask_teacher_file(&self, question: &str) -> Result<String> {
        let question_id = uuid::Uuid::new_v4().to_string();
        let question_file = self.question_dir.join(format!("{}.txt", question_id));
        let answer_file = self.answer_dir.join(format!("{}.txt", question_id));
        
        // Write question to file
        tokio::fs::write(&question_file, question).await?;
        
        info!("📤 Asked Teacher via file ({}): {}", &question_id[..8], &question[..question.len().min(100)]);
        
        // Wait for answer with timeout
        let answer = self.wait_for_answer(&answer_file, self.timeout_seconds).await?;
        
        // Cleanup
        let _ = tokio::fs::remove_file(&question_file).await;
        let _ = tokio::fs::remove_file(&answer_file).await;
        
        info!("📥 Teacher answered via file ({}): {}", &question_id[..8], &answer[..answer.len().min(100)]);
        
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
        if self.use_http {
            self.check_teacher_http().await
        } else {
            self.check_teacher_file().await
        }
    }
    
    async fn check_teacher_http(&self) -> bool {
        let status_endpoint = self.http_status_endpoint.as_ref()
            .or_else(|| {
                // Try to derive status endpoint from ask endpoint
                self.http_endpoint.as_ref().and_then(|e| {
                    if e.contains("/ask") {
                        Some(e.replace("/ask", "/status"))
                    } else {
                        Some(format!("{}/status", e))
                    }
                })
            });
        
        let endpoint = match status_endpoint {
            Some(e) => e,
            None => return false,
        };
        
        let client = match &self.http_client {
            Some(c) => c,
            None => return false,
        };
        
        match client.get(&endpoint).send().await {
            Ok(response) if response.status().is_success() => {
                match response.json::<serde_json::Value>().await {
                    Ok(data) => {
                        data.get("online").and_then(|o| o.as_bool()).unwrap_or(false)
                    }
                    Err(_) => false,
                }
            }
            _ => false,
        }
    }
    
    async fn check_teacher_file(&self) -> bool {
        match self.ask_teacher_file("PING").await {
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
    
    /// Get Teacher status (returns full status object)
    pub async fn get_teacher_status(&self) -> Option<serde_json::Value> {
        if !self.use_http {
            return None;
        }
        
        let status_endpoint = self.http_status_endpoint.as_ref()
            .or_else(|| {
                self.http_endpoint.as_ref().and_then(|e| {
                    if e.contains("/ask") {
                        Some(e.replace("/ask", "/status"))
                    } else {
                        Some(format!("{}/status", e))
                    }
                })
            })?;
        
        let client = self.http_client.as_ref()?;
        
        match client.get(&status_endpoint).send().await {
            Ok(response) if response.status().is_success() => {
                response.json::<serde_json::Value>().await.ok()
            }
            _ => None,
        }
    }
    
    /// Force using HTTP mode (for testing)
    pub fn force_http_mode(&mut self, endpoint: &str, status_endpoint: Option<&str>) -> Result<()> {
        self.http_client = Some(Client::builder()
            .timeout(Duration::from_secs(self.timeout_seconds))
            .build()?);
        self.http_endpoint = Some(endpoint.to_string());
        self.http_status_endpoint = status_endpoint.map(|s| s.to_string());
        self.use_http = true;
        info!("Teacher client forced to HTTP mode: {}", endpoint);
        Ok(())
    }
    
    /// Force using file mode (for testing)
    pub fn force_file_mode(&mut self) {
        self.use_http = false;
        info!("Teacher client forced to file mode");
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
        info!("🤔 Marisselle is confused about '{}': {}", topic, confusion);
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
    teacher_status_cache: Arc<tokio::sync::RwLock<Option<serde_json::Value>>>,
}

impl LearningCoordinator {
    pub fn new() -> Result<Self> {
        let teacher_client = TeacherClient::new()?;
        let confusion_detector = ConfusionDetector::new(teacher_client.clone());
        
        Ok(Self {
            teacher_client,
            confusion_detector,
            teacher_available: Arc::new(tokio::sync::RwLock::new(false)),
            teacher_status_cache: Arc::new(tokio::sync::RwLock::new(None)),
        })
    }
    
    pub async fn check_teacher(&self) -> bool {
        let available = self.teacher_client.is_teacher_available().await;
        {
            let mut status = self.teacher_available.write().await;
            *status = available;
        }
        
        // Update status cache if available
        if available {
            if let Some(status) = self.teacher_client.get_teacher_status().await {
                let mut cache = self.teacher_status_cache.write().await;
                *cache = Some(status);
            }
        }
        
        if available {
            info!("✅ Teacher is AVAILABLE");
        } else {
            warn!("❌ Teacher is UNAVAILABLE");
        }
        
        available
    }
    
    pub async fn is_teacher_available(&self) -> bool {
        *self.teacher_available.read().await
    }
    
    pub async fn get_teacher_status(&self) -> Option<serde_json::Value> {
        self.teacher_status_cache.read().await.clone()
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
                info!("✅ Teacher provided clarification for: {}", topic);
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
            teacher_status_cache: Arc::clone(&self.teacher_status_cache),
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
    
    #[test]
    fn test_teacher_client_creation() {
        let client = TeacherClient::new();
        assert!(client.is_ok());
    }
}
