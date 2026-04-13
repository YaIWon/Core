// ======================================================================
// COMPREHENSIVE LOGGING SYSTEM
// File: src/learning/logger.rs
// Description: Logs EVERYTHING - conversations, thoughts, actions,
//              learning events, API calls, internet searches, and more.
//              All logs are saved to files and can be viewed in real-time.
// ======================================================================

use anyhow::Result;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn, error, debug};

// ======================================================================
// LOG TYPES - Everything that can be logged
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogCategory {
    // Communication
    TeacherToLM,
    LMToTeacher,
    InternalMessage,
    
    // Learning
    LessonGenerated,
    LessonLearned,
    LessonFailed,
    KnowledgeIntegrated,
    VectorStored,
    BlockchainRecorded,
    
    // Cognition
    LMThought,
    TeacherDeepThink,
    ReasoningProcess,
    Decision,
    
    // Actions
    FileRead,
    FileWrite,
    ApiCall,
    InternetSearch,
    DatabaseQuery,
    
    // System
    HealthCheck,
    Error,
    Warning,
    Debug,
    Performance,
    
    // Evolution
    SelfUpgrade,
    NewTopicDiscovered,
    SkillAcquired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub category: LogCategory,
    pub source: String,  // "Teacher", "Marisselle", "System"
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub duration_ms: Option<u64>,
    pub related_id: Option<String>,
}

impl LogEntry {
    pub fn new(level: LogLevel, category: LogCategory, source: &str, message: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            level,
            category,
            source: source.to_string(),
            message: message.to_string(),
            details: None,
            duration_ms: None,
            related_id: None,
        }
    }
    
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
    
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }
    
    pub fn with_related(mut self, related_id: &str) -> Self {
        self.related_id = Some(related_id.to_string());
        self
    }
    
    pub fn format_for_display(&self) -> String {
        let level_str = match self.level {
            LogLevel::Trace => "🔍",
            LogLevel::Debug => "🐛",
            LogLevel::Info => "📝",
            LogLevel::Warn => "⚠️",
            LogLevel::Error => "❌",
            LogLevel::Critical => "🔥",
        };
        
        let category_str = match self.category {
            LogCategory::TeacherToLM => "📥 TEACHER→LM",
            LogCategory::LMToTeacher => "📤 LM→TEACHER",
            LogCategory::InternalMessage => "💭 INTERNAL",
            LogCategory::LessonGenerated => "📚 LESSON",
            LogCategory::LessonLearned => "✅ LEARNED",
            LogCategory::LessonFailed => "❌ FAILED",
            LogCategory::KnowledgeIntegrated => "🧠 KNOWLEDGE",
            LogCategory::VectorStored => "🗄️ VECTOR",
            LogCategory::BlockchainRecorded => "🔗 BLOCKCHAIN",
            LogCategory::LMThought => "🤔 THOUGHT",
            LogCategory::TeacherDeepThink => "🧠 DEEPTHINK",
            LogCategory::ReasoningProcess => "🔬 REASONING",
            LogCategory::Decision => "⚡ DECISION",
            LogCategory::FileRead => "📂 READ",
            LogCategory::FileWrite => "💾 WRITE",
            LogCategory::ApiCall => "🌐 API",
            LogCategory::InternetSearch => "🔎 SEARCH",
            LogCategory::DatabaseQuery => "🗃️ QUERY",
            LogCategory::HealthCheck => "💚 HEALTH",
            LogCategory::Error => "⚠️ ERROR",
            LogCategory::Warning => "⚠️ WARN",
            LogCategory::Debug => "🔧 DEBUG",
            LogCategory::Performance => "⏱️ PERF",
            LogCategory::SelfUpgrade => "🚀 UPGRADE",
            LogCategory::NewTopicDiscovered => "✨ NEW",
            LogCategory::SkillAcquired => "🎯 SKILL",
        };
        
        let mut output = format!(
            "{} [{}] {} | {} | {}",
            level_str,
            self.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
            category_str,
            self.source,
            self.message
        );
        
        if let Some(duration) = self.duration_ms {
            output.push_str(&format!(" | {}ms", duration));
        }
        
        if let Some(details) = &self.details {
            output.push_str(&format!("\n    Details: {}", details));
        }
        
        output
    }
}

// ======================================================================
// LOG WRITER - Writes logs to files
// ======================================================================

pub struct LogWriter {
    log_dir: PathBuf,
    current_file: Option<BufWriter<File>>,
    max_file_size: u64,
    file_counter: u64,
}

impl LogWriter {
    pub fn new(log_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&log_dir)?;
        
        Ok(Self {
            log_dir,
            current_file: None,
            max_file_size: 100 * 1024 * 1024, // 100 MB
            file_counter: 0,
        })
    }
    
    fn get_log_path(&self) -> PathBuf {
        let date = Utc::now().format("%Y-%m-%d");
        self.log_dir.join(format!("marisselle_{}_{:04}.log", date, self.file_counter))
    }
    
    fn rotate_if_needed(&mut self) -> Result<()> {
        if let Some(writer) = &self.current_file {
            let metadata = writer.get_ref().metadata()?;
            if metadata.len() >= self.max_file_size {
                self.file_counter += 1;
                self.current_file = None;
            }
        }
        
        if self.current_file.is_none() {
            let path = self.get_log_path();
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)?;
            self.current_file = Some(BufWriter::new(file));
            info!("Created new log file: {:?}", path);
        }
        
        Ok(())
    }
    
    pub fn write(&mut self, entry: &LogEntry) -> Result<()> {
        self.rotate_if_needed()?;
        
        if let Some(writer) = &mut self.current_file {
            let json = serde_json::to_string(entry)?;
            writeln!(writer, "{}", json)?;
            writer.flush()?;
            
            // Also print to console for real-time viewing
            println!("{}", entry.format_for_display());
        }
        
        Ok(())
    }
}

// ======================================================================
// COMPREHENSIVE LOGGER - Main logging interface
// ======================================================================

pub struct ComprehensiveLogger {
    writer: Arc<Mutex<LogWriter>>,
    thought_log: Arc<Mutex<Vec<LogEntry>>>,
    conversation_log: Arc<Mutex<Vec<LogEntry>>>,
    learning_log: Arc<Mutex<Vec<LogEntry>>>,
    api_log: Arc<Mutex<Vec<LogEntry>>>,
    system_log: Arc<Mutex<Vec<LogEntry>>>,
}

impl ComprehensiveLogger {
    pub fn new(log_dir: PathBuf) -> Result<Self> {
        Ok(Self {
            writer: Arc::new(Mutex::new(LogWriter::new(log_dir)?)),
            thought_log: Arc::new(Mutex::new(Vec::new())),
            conversation_log: Arc::new(Mutex::new(Vec::new())),
            learning_log: Arc::new(Mutex::new(Vec::new())),
            api_log: Arc::new(Mutex::new(Vec::new())),
            system_log: Arc::new(Mutex::new(Vec::new())),
        })
    }
    
    async fn log(&self, entry: LogEntry) {
        // Store in memory by category
        match entry.category {
            LogCategory::LMThought | LogCategory::TeacherDeepThink | LogCategory::ReasoningProcess => {
                self.thought_log.lock().await.push(entry.clone());
            }
            LogCategory::TeacherToLM | LogCategory::LMToTeacher | LogCategory::InternalMessage => {
                self.conversation_log.lock().await.push(entry.clone());
            }
            LogCategory::LessonGenerated | LogCategory::LessonLearned | LogCategory::LessonFailed |
            LogCategory::KnowledgeIntegrated | LogCategory::VectorStored | LogCategory::BlockchainRecorded => {
                self.learning_log.lock().await.push(entry.clone());
            }
            LogCategory::ApiCall | LogCategory::InternetSearch => {
                self.api_log.lock().await.push(entry.clone());
            }
            _ => {
                self.system_log.lock().await.push(entry.clone());
            }
        }
        
        // Write to file
        if let Err(e) = self.writer.lock().await.write(&entry) {
            eprintln!("Failed to write log: {}", e);
        }
    }
    
    // ==================================================================
    // CONVERSATION LOGGING
    // ==================================================================
    
    pub async fn log_teacher_to_lm(&self, message: &str, details: Option<serde_json::Value>) {
        let entry = LogEntry::new(
            LogLevel::Info,
            LogCategory::TeacherToLM,
            "Teacher",
            message,
        ).with_details(details.unwrap_or(serde_json::json!({})));
        self.log(entry).await;
    }
    
    pub async fn log_lm_to_teacher(&self, message: &str, details: Option<serde_json::Value>) {
        let entry = LogEntry::new(
            LogLevel::Info,
            LogCategory::LMToTeacher,
            "Marisselle",
            message,
        ).with_details(details.unwrap_or(serde_json::json!({})));
        self.log(entry).await;
    }
    
    // ==================================================================
    // THOUGHT LOGGING
    // ==================================================================
    
    pub async fn log_lm_thought(&self, thought: &str, context: Option<&str>) {
        let details = serde_json::json!({
            "context": context,
            "thought_length": thought.len(),
        });
        let entry = LogEntry::new(
            LogLevel::Debug,
            LogCategory::LMThought,
            "Marisselle",
            thought,
        ).with_details(details);
        self.log(entry).await;
    }
    
    pub async fn log_teacher_deep_think(&self, thinking: &str, duration_ms: u64) {
        let details = serde_json::json!({
            "thinking_length": thinking.len(),
        });
        let entry = LogEntry::new(
            LogLevel::Info,
            LogCategory::TeacherDeepThink,
            "Teacher",
            &format!("Deep thinking completed: {}", &thinking[..thinking.len().min(200)]),
        )
        .with_details(details)
        .with_duration(duration_ms);
        self.log(entry).await;
    }
    
    pub async fn log_reasoning(&self, reasoning: &str, conclusion: &str) {
        let details = serde_json::json!({
            "reasoning": reasoning,
            "conclusion": conclusion,
        });
        let entry = LogEntry::new(
            LogLevel::Debug,
            LogCategory::ReasoningProcess,
            "Marisselle",
            &format!("Reasoned: {}", conclusion),
        ).with_details(details);
        self.log(entry).await;
    }
    
    // ==================================================================
    // LEARNING LOGGING
    // ==================================================================
    
    pub async fn log_lesson_generated(&self, topic: &str, length: usize) {
        let entry = LogEntry::new(
            LogLevel::Info,
            LogCategory::LessonGenerated,
            "Teacher",
            &format!("Generated lesson: {}", topic),
        ).with_details(serde_json::json!({ "topic": topic, "length": length }));
        self.log(entry).await;
    }
    
    pub async fn log_lesson_learned(&self, topic: &str, confidence: f32) {
        let entry = LogEntry::new(
            LogLevel::Info,
            LogCategory::LessonLearned,
            "Marisselle",
            &format!("✅ LEARNED: {} (confidence: {:.1}%)", topic, confidence * 100.0),
        ).with_details(serde_json::json!({ "topic": topic, "confidence": confidence }));
        self.log(entry).await;
    }
    
    pub async fn log_lesson_failed(&self, topic: &str, attempt: u32, reason: Option<&str>) {
        let entry = LogEntry::new(
            LogLevel::Warn,
            LogCategory::LessonFailed,
            "Marisselle",
            &format!("❌ FAILED: {} (attempt {})", topic, attempt),
        ).with_details(serde_json::json!({ "topic": topic, "attempt": attempt, "reason": reason }));
        self.log(entry).await;
    }
    
    pub async fn log_knowledge_integrated(&self, source: &str, chunks: usize) {
        let entry = LogEntry::new(
            LogLevel::Info,
            LogCategory::KnowledgeIntegrated,
            "System",
            &format!("Integrated {} chunks from {}", chunks, source),
        ).with_details(serde_json::json!({ "source": source, "chunks": chunks }));
        self.log(entry).await;
    }
    
    pub async fn log_vector_stored(&self, id: &str, source: &str) {
        let entry = LogEntry::new(
            LogLevel::Debug,
            LogCategory::VectorStored,
            "VectorStore",
            &format!("Stored vector: {} from {}", id, source),
        );
        self.log(entry).await;
    }
    
    pub async fn log_blockchain_recorded(&self, block_index: u64, content_preview: &str) {
        let entry = LogEntry::new(
            LogLevel::Info,
            LogCategory::BlockchainRecorded,
            "Blockchain",
            &format!("Block {} recorded: {}", block_index, content_preview),
        );
        self.log(entry).await;
    }
    
    // ==================================================================
    // API & INTERNET LOGGING
    // ==================================================================
    
    pub async fn log_api_call(&self, endpoint: &str, duration_ms: u64, success: bool) {
        let level = if success { LogLevel::Debug } else { LogLevel::Error };
        let entry = LogEntry::new(
            level,
            LogCategory::ApiCall,
            "API",
            &format!("{} {} ({}ms)", if success { "✅" } else { "❌" }, endpoint, duration_ms),
        ).with_duration(duration_ms).with_details(serde_json::json!({ "endpoint": endpoint }));
        self.log(entry).await;
    }
    
    pub async fn log_internet_search(&self, query: &str, results_count: usize, duration_ms: u64) {
        let entry = LogEntry::new(
            LogLevel::Info,
            LogCategory::InternetSearch,
            "Teacher",
            &format!("🔎 Search: '{}' → {} results", query, results_count),
        ).with_duration(duration_ms).with_details(serde_json::json!({ "query": query, "results": results_count }));
        self.log(entry).await;
    }
    
    // ==================================================================
    // ACTION LOGGING
    // ==================================================================
    
    pub async fn log_file_read(&self, path: &Path, bytes: u64) {
        let entry = LogEntry::new(
            LogLevel::Debug,
            LogCategory::FileRead,
            "System",
            &format!("📂 Read: {:?} ({} bytes)", path.file_name().unwrap_or_default(), bytes),
        );
        self.log(entry).await;
    }
    
    pub async fn log_file_write(&self, path: &Path, bytes: u64) {
        let entry = LogEntry::new(
            LogLevel::Debug,
            LogCategory::FileWrite,
            "System",
            &format!("💾 Write: {:?} ({} bytes)", path.file_name().unwrap_or_default(), bytes),
        );
        self.log(entry).await;
    }
    
    // ==================================================================
    // SYSTEM LOGGING
    // ==================================================================
    
    pub async fn log_health_check(&self, status: &str, details: Option<serde_json::Value>) {
        let entry = LogEntry::new(
            LogLevel::Info,
            LogCategory::HealthCheck,
            "System",
            &format!("💚 Health: {}", status),
        ).with_details(details.unwrap_or(serde_json::json!({})));
        self.log(entry).await;
    }
    
    pub async fn log_error(&self, error: &str, source: &str) {
        let entry = LogEntry::new(
            LogLevel::Error,
            LogCategory::Error,
            source,
            error,
        );
        self.log(entry).await;
    }
    
    pub async fn log_performance(&self, operation: &str, duration_ms: u64) {
        let entry = LogEntry::new(
            LogLevel::Debug,
            LogCategory::Performance,
            "System",
            &format!("⏱️ {} took {}ms", operation, duration_ms),
        ).with_duration(duration_ms);
        self.log(entry).await;
    }
    
    pub async fn log_self_upgrade(&self, component: &str, description: &str) {
        let entry = LogEntry::new(
            LogLevel::Info,
            LogCategory::SelfUpgrade,
            "Marisselle",
            &format!("🚀 UPGRADE: {} - {}", component, description),
        );
        self.log(entry).await;
    }
    
    pub async fn log_new_topic_discovered(&self, topic: &str, source: &str) {
        let entry = LogEntry::new(
            LogLevel::Info,
            LogCategory::NewTopicDiscovered,
            "Marisselle",
            &format!("✨ DISCOVERED: {} (from {})", topic, source),
        );
        self.log(entry).await;
    }
    
    pub async fn log_skill_acquired(&self, skill: &str) {
        let entry = LogEntry::new(
            LogLevel::Info,
            LogCategory::SkillAcquired,
            "Marisselle",
            &format!("🎯 SKILL ACQUIRED: {}", skill),
        );
        self.log(entry).await;
    }
    
    // ==================================================================
    // QUERY METHODS - For viewing logs
    // ==================================================================
    
    pub async fn get_conversation_history(&self, limit: usize) -> Vec<LogEntry> {
        let log = self.conversation_log.lock().await;
        log.iter().rev().take(limit).cloned().collect()
    }
    
    pub async fn get_thought_history(&self, limit: usize) -> Vec<LogEntry> {
        let log = self.thought_log.lock().await;
        log.iter().rev().take(limit).cloned().collect()
    }
    
    pub async fn get_learning_history(&self, limit: usize) -> Vec<LogEntry> {
        let log = self.learning_log.lock().await;
        log.iter().rev().take(limit).cloned().collect()
    }
    
    pub async fn get_api_history(&self, limit: usize) -> Vec<LogEntry> {
        let log = self.api_log.lock().await;
        log.iter().rev().take(limit).cloned().collect()
    }
    
    pub async fn export_all_logs(&self, output_path: &Path) -> Result<()> {
        let mut all_logs = Vec::new();
        
        all_logs.extend(self.conversation_log.lock().await.clone());
        all_logs.extend(self.thought_log.lock().await.clone());
        all_logs.extend(self.learning_log.lock().await.clone());
        all_logs.extend(self.api_log.lock().await.clone());
        all_logs.extend(self.system_log.lock().await.clone());
        
        all_logs.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        
        let json = serde_json::to_string_pretty(&all_logs)?;
        tokio::fs::write(output_path, json).await?;
        
        Ok(())
    }
}

// ======================================================================
// DEEP THINK ENGINE - For DeepSeek to reason deeply
// ======================================================================

pub struct DeepThinkEngine {
    logger: Arc<ComprehensiveLogger>,
}

impl DeepThinkEngine {
    pub fn new(logger: Arc<ComprehensiveLogger>) -> Self {
        Self { logger }
    }
    
    /// Make DeepSeek think deeply about a topic before responding
    pub async fn deep_think(&self, client: &reqwest::Client, api_key: &str, prompt: &str) -> Result<(String, String)> {
        let start = std::time::Instant::now();
        
        // Step 1: Deep reasoning prompt
        let reasoning_prompt = format!(
            "You are engaging in deep, step-by-step reasoning. \
             Think carefully about the following. Break down your reasoning into steps. \
             Consider multiple perspectives. Identify assumptions. \
             After your reasoning, provide a clear, well-structured response.\n\n\
             Topic/Question: {}\n\n\
             Format your response as:\n\
             === REASONING ===\n\
             [Your step-by-step reasoning here]\n\
             === RESPONSE ===\n\
             [Your final response here]",
            prompt
        );
        
        let response = client
            .post("https://api.deepseek.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&serde_json::json!({
                "model": "deepseek-chat",
                "messages": [
                    {"role": "system", "content": "You are a deep reasoning assistant. Think carefully and step-by-step."},
                    {"role": "user", "content": reasoning_prompt}
                ],
                "temperature": 0.3,
                "max_tokens": 4096,
            }))
            .send()
            .await?;
        
        let duration_ms = start.elapsed().as_millis() as u64;
        
        if !response.status().is_success() {
            self.logger.log_api_call("deepseek/deep-think", duration_ms, false).await;
            return Err(anyhow::anyhow!("DeepSeek API error: {}", response.status()));
        }
        
        self.logger.log_api_call("deepseek/deep-think", duration_ms, true).await;
        
        let data: serde_json::Value = response.json().await?;
        let content = data["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        
        // Parse reasoning and response
        let (reasoning, response) = Self::parse_deep_think_response(&content);
        
        self.logger.log_teacher_deep_think(&reasoning, duration_ms).await;
        
        Ok((reasoning, response))
    }
    
    fn parse_deep_think_response(content: &str) -> (String, String) {
        let mut reasoning = String::new();
        let mut response = String::new();
        
        if let Some(reasoning_start) = content.find("=== REASONING ===") {
            if let Some(response_start) = content.find("=== RESPONSE ===") {
                reasoning = content[reasoning_start + 17..response_start]
                    .trim()
                    .to_string();
                response = content[response_start + 17..]
                    .trim()
                    .to_string();
            }
        }
        
        if reasoning.is_empty() {
            reasoning = "No explicit reasoning provided".to_string();
            response = content;
        }
        
        (reasoning, response)
    }
}

// ======================================================================
// INTERNET SEARCH ENGINE
// ======================================================================

pub struct InternetSearchEngine {
    logger: Arc<ComprehensiveLogger>,
    client: reqwest::Client,
}

impl InternetSearchEngine {
    pub fn new(logger: Arc<ComprehensiveLogger>) -> Self {
        Self {
            logger,
            client: reqwest::Client::new(),
        }
    }
    
    /// Search the internet using DeepSeek's search capability
    pub async fn search(&self, api_key: &str, query: &str) -> Result<Vec<SearchResult>> {
        let start = std::time::Instant::now();
        
        let search_prompt = format!(
            "Search the internet for information about: {}\n\n\
             Provide up-to-date information based on your knowledge. \
             Include relevant facts, data, and sources if available.\n\n\
             Format your response as:\n\
             === RESULTS ===\n\
             [Information found]",
            query
        );
        
        let response = self.client
            .post("https://api.deepseek.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&serde_json::json!({
                "model": "deepseek-chat",
                "messages": [
                    {"role": "system", "content": "You are a research assistant with internet search capabilities."},
                    {"role": "user", "content": search_prompt}
                ],
                "temperature": 0.3,
                "max_tokens": 2048,
            }))
            .send()
            .await?;
        
        let duration_ms = start.elapsed().as_millis() as u64;
        
        if !response.status().is_success() {
            self.logger.log_api_call("deepseek/search", duration_ms, false).await;
            return Err(anyhow::anyhow!("Search API error: {}", response.status()));
        }
        
        let data: serde_json::Value = response.json().await?;
        let content = data["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        
        self.logger.log_internet_search(query, 1, duration_ms).await;
        
        Ok(vec![SearchResult {
            title: format!("Search: {}", query),
            snippet: content,
            url: None,
        }])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub snippet: String,
    pub url: Option<String>,
}
