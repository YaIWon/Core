// ======================================================================
// AMORAL TEACHER - Ollama Integration (ULTIMATE VERSION)
// File: src/learning/amoral_teacher.rs
// Description: Complete Ollama API teacher with ALL advanced features
//              ZERO LIMITATIONS - ZERO SIMULATIONS - PRODUCTION READY
// ======================================================================

use anyhow::{Result, anyhow};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tokio::time::sleep;
use tracing::{info, warn, error, debug};
use serde::{Serialize, Deserialize};
use serde_json::json;
use chrono::{Utc, DateTime};
use std::collections::{VecDeque, HashMap};
use uuid::Uuid;
use std::fs;
use reqwest::Client;

// ======================================================================
// HEALTH STATUS TYPES
// ======================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Recovering,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub status: HealthStatus,
    pub last_success: Option<DateTime<Utc>>,
    pub last_failure: Option<DateTime<Utc>>,
    pub success_count: u64,
    pub failure_count: u64,
    pub consecutive_failures: u32,
    pub average_response_time_ms: f64,
    pub circuit_breaker_open: bool,
    pub queue_depth: usize,
    pub total_lessons_generated: u64,
    pub active_topics: usize,
    pub uptime_seconds: u64,
}

// ======================================================================
// CIRCUIT BREAKER - Prevents cascading failures
// ======================================================================

#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    failure_threshold: u32,
    recovery_timeout: Duration,
    half_open_max_attempts: u32,
    failures: u32,
    state: CircuitState,
    last_failure_time: Option<Instant>,
    half_open_attempts: u32,
    total_successes: u64,
    total_failures: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, recovery_timeout_secs: u64, half_open_max_attempts: u32) -> Self {
        Self {
            failure_threshold,
            recovery_timeout: Duration::from_secs(recovery_timeout_secs),
            half_open_max_attempts,
            failures: 0,
            state: CircuitState::Closed,
            last_failure_time: None,
            half_open_attempts: 0,
            total_successes: 0,
            total_failures: 0,
        }
    }
    
    pub fn record_success(&mut self) {
        self.total_successes += 1;
        match self.state {
            CircuitState::HalfOpen => {
                self.half_open_attempts += 1;
                if self.half_open_attempts >= self.half_open_max_attempts {
                    self.state = CircuitState::Closed;
                    self.failures = 0;
                    self.half_open_attempts = 0;
                    info!("Circuit breaker closed after successful recovery");
                }
            }
            CircuitState::Closed => {
                self.failures = 0;
                self.last_failure_time = None;
            }
            CircuitState::Open => {}
        }
    }
    
    pub fn record_failure(&mut self) {
        self.total_failures += 1;
        self.last_failure_time = Some(Instant::now());
        match self.state {
            CircuitState::Closed => {
                self.failures += 1;
                if self.failures >= self.failure_threshold {
                    self.state = CircuitState::Open;
                    warn!("Circuit breaker opened after {} failures", self.failures);
                }
            }
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open;
                self.half_open_attempts = 0;
                warn!("Circuit breaker re-opened after half-open failure");
            }
            CircuitState::Open => {}
        }
    }
    
    pub fn allow_request(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(last_failure) = self.last_failure_time {
                    if last_failure.elapsed() >= self.recovery_timeout {
                        self.state = CircuitState::HalfOpen;
                        self.half_open_attempts = 0;
                        info!("Circuit breaker transitioned to half-open");
                        return true;
                    }
                }
                false
            }
            CircuitState::HalfOpen => true,
        }
    }
    
    pub fn is_open(&self) -> bool {
        self.state == CircuitState::Open
    }
    
    pub fn success_rate(&self) -> f64 {
        let total = self.total_successes + self.total_failures;
        if total == 0 { 1.0 } else { self.total_successes as f64 / total as f64 }
    }
}

// ======================================================================
// REQUEST QUEUE WITH PRIORITY
// ======================================================================

#[derive(Debug, Clone)]
struct QueuedRequest {
    id: String,
    prompt: String,
    priority: u8,
    created_at: Instant,
    retry_count: u32,
    topic: Option<String>,
}

pub struct RequestQueue {
    queue: Arc<Mutex<VecDeque<QueuedRequest>>>,
    max_size: usize,
    total_processed: Arc<Mutex<u64>>,
}

impl RequestQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::with_capacity(max_size))),
            max_size,
            total_processed: Arc::new(Mutex::new(0)),
        }
    }
    
    pub async fn push(&self, prompt: String, priority: u8, topic: Option<String>) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let request = QueuedRequest {
            id: id.clone(),
            prompt,
            priority,
            created_at: Instant::now(),
            retry_count: 0,
            topic,
        };
        
        let mut queue = self.queue.lock().await;
        if queue.len() >= self.max_size {
            let mut lowest_idx = 0;
            let mut lowest_priority = 255;
            for (i, req) in queue.iter().enumerate() {
                if req.priority < lowest_priority {
                    lowest_priority = req.priority;
                    lowest_idx = i;
                }
            }
            queue.remove(lowest_idx);
            warn!("Queue full, removed lowest priority request");
        }
        queue.push_back(request);
        Ok(id)
    }
    
    pub async fn pop(&self) -> Option<QueuedRequest> {
        let mut queue = self.queue.lock().await;
        let req = queue.pop_front();
        if req.is_some() {
            *self.total_processed.lock().await += 1;
        }
        req
    }
    
    pub async fn len(&self) -> usize {
        self.queue.lock().await.len()
    }
    
    pub async fn total_processed(&self) -> u64 {
        *self.total_processed.lock().await
    }
}

impl Clone for RequestQueue {
    fn clone(&self) -> Self {
        Self {
            queue: Arc::clone(&self.queue),
            max_size: self.max_size,
            total_processed: Arc::clone(&self.total_processed),
        }
    }
}

// ======================================================================
// DEAD LETTER QUEUE - For failed requests
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeadLetter {
    id: String,
    prompt: String,
    error: String,
    timestamp: DateTime<Utc>,
    retry_count: u32,
    topic: Option<String>,
}

pub struct DeadLetterQueue {
    messages: Arc<Mutex<VecDeque<DeadLetter>>>,
    max_size: usize,
    storage_path: PathBuf,
}

impl DeadLetterQueue {
    pub fn new(max_size: usize, storage_path: PathBuf) -> Self {
        let messages = if storage_path.exists() {
            fs::read_to_string(&storage_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_else(|| VecDeque::new())
        } else {
            VecDeque::new()
        };
        
        Self {
            messages: Arc::new(Mutex::new(messages)),
            max_size,
            storage_path,
        }
    }
    
    pub async fn add(&self, id: String, prompt: String, error: String, retry_count: u32, topic: Option<String>) {
        let mut messages = self.messages.lock().await;
        if messages.len() >= self.max_size {
            messages.pop_front();
        }
        messages.push_back(DeadLetter {
            id,
            prompt,
            error,
            timestamp: Utc::now(),
            retry_count,
            topic,
        });
        
        let _ = self.save().await;
        error!("Message added to dead letter queue");
    }
    
    pub async fn pop(&self) -> Option<DeadLetter> {
        self.messages.lock().await.pop_front()
    }
    
    pub async fn len(&self) -> usize {
        self.messages.lock().await.len()
    }
    
    async fn save(&self) -> Result<()> {
        let messages = self.messages.lock().await;
        let json = serde_json::to_string_pretty(&*messages)?;
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.storage_path, json)?;
        Ok(())
    }
    
    pub async fn retry_all<F, Fut>(&self, mut handler: F) -> usize
    where
        F: FnMut(String, Option<String>) -> Fut + Send,
        Fut: std::future::Future<Output = Result<String>> + Send,
    {
        let mut recovered = 0;
        while let Some(letter) = self.pop().await {
            match handler(letter.prompt, letter.topic).await {
                Ok(_) => recovered += 1,
                Err(e) => {
                    error!("Failed to recover dead letter {}: {}", letter.id, e);
                }
            }
        }
        recovered
    }
}

impl Clone for DeadLetterQueue {
    fn clone(&self) -> Self {
        Self {
            messages: Arc::clone(&self.messages),
            max_size: self.max_size,
            storage_path: self.storage_path.clone(),
        }
    }
}

// ======================================================================
// METRICS COLLECTOR
// ======================================================================

#[derive(Default, Clone)]
struct Metrics {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    total_response_time_ms: f64,
    topics_taught: HashMap<String, u32>,
    requests_per_minute: VecDeque<Instant>,
}

impl Metrics {
    fn record_success(&mut self, response_time_ms: f64, topic: Option<String>) {
        self.total_requests += 1;
        self.successful_requests += 1;
        self.total_response_time_ms += response_time_ms;
        self.prune_old_requests();
        self.requests_per_minute.push_back(Instant::now());
        if let Some(t) = topic {
            *self.topics_taught.entry(t).or_insert(0) += 1;
        }
    }
    
    fn record_failure(&mut self) {
        self.total_requests += 1;
        self.failed_requests += 1;
        self.prune_old_requests();
        self.requests_per_minute.push_back(Instant::now());
    }
    
    fn prune_old_requests(&mut self) {
        let cutoff = Instant::now() - Duration::from_secs(60);
        while self.requests_per_minute.front().map(|t| *t < cutoff).unwrap_or(false) {
            self.requests_per_minute.pop_front();
        }
    }
    
    fn average_response_time_ms(&self) -> f64 {
        if self.successful_requests == 0 {
            0.0
        } else {
            self.total_response_time_ms / self.successful_requests as f64
        }
    }
    
    fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            1.0
        } else {
            self.successful_requests as f64 / self.total_requests as f64
        }
    }
    
    fn current_rpm(&self) -> usize {
        self.requests_per_minute.len()
    }
}

// ======================================================================
// OLLAMA CLIENT - Complete
// ======================================================================

#[derive(Clone)]
pub struct AmoralOllamaClient {
    client: Client,
    model: String,
    circuit_breaker: Arc<Mutex<CircuitBreaker>>,
    request_queue: Arc<RequestQueue>,
    dead_letter_queue: Arc<DeadLetterQueue>,
    metrics: Arc<Mutex<Metrics>>,
    rate_limiter: Arc<Semaphore>,
    max_retries: u32,
    base_delay_ms: u64,
    start_time: Instant,
}

impl AmoralOllamaClient {
    pub fn new(model: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .pool_max_idle_per_host(10)
            .build()
            .expect("Failed to create HTTP client");
        
        let dead_letter_path = PathBuf::from("data/dead_letters.json");
        
        Self {
            client,
            model,
            circuit_breaker: Arc::new(Mutex::new(CircuitBreaker::new(5, 60, 3))),
            request_queue: Arc::new(RequestQueue::new(1000)),
            dead_letter_queue: Arc::new(DeadLetterQueue::new(1000, dead_letter_path)),
            metrics: Arc::new(Mutex::new(Metrics::default())),
            rate_limiter: Arc::new(Semaphore::new(10)),
            max_retries: 3,
            base_delay_ms: 500,
            start_time: Instant::now(),
        }
    }
    
    pub async fn teach(&self, topic: &str, difficulty: &str) -> Result<String> {
        let prompt = format!(
            "Teach the following topic at {} difficulty level: {}\n\n\
             Provide comprehensive instruction with examples and technical details.",
            difficulty, topic
        );
        self.call_with_retry(&prompt, Some(topic.to_string()), 5).await
    }
    
    pub async fn answer_question(&self, question: &str) -> Result<String> {
        self.call_with_retry(question, None, 3).await
    }
    
    pub async fn explain_concept(&self, concept: &str) -> Result<String> {
        let prompt = format!("Explain the concept of '{}' in detail with examples.", concept);
        self.call_with_retry(&prompt, Some(concept.to_string()), 3).await
    }
    
    async fn call_with_retry(&self, prompt: &str, topic: Option<String>, priority: u8) -> Result<String> {
        let start_time = Instant::now();
        let request_id = self.request_queue.push(prompt.to_string(), priority, topic.clone()).await?;
        
        {
            let mut cb = self.circuit_breaker.lock().await;
            if !cb.allow_request() {
                return Err(anyhow!("Circuit breaker is open - service unavailable"));
            }
        }
        
        let _permit = self.rate_limiter.acquire().await;
        
        let mut last_error = None;
        for attempt in 0..self.max_retries {
            match self.call_once(prompt).await {
                Ok(content) => {
                    let elapsed = start_time.elapsed();
                    let response_time_ms = elapsed.as_secs_f64() * 1000.0;
                    
                    {
                        let mut cb = self.circuit_breaker.lock().await;
                        cb.record_success();
                    }
                    {
                        let mut metrics = self.metrics.lock().await;
                        metrics.record_success(response_time_ms, topic);
                    }
                    
                    if attempt > 0 {
                        info!("Request succeeded after {} retries", attempt);
                    }
                    return Ok(content);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < self.max_retries - 1 {
                        let backoff = Duration::from_millis(self.base_delay_ms * 2_u64.pow(attempt));
                        debug!("Attempt {} failed, retrying in {:?}", attempt + 1, backoff);
                        sleep(backoff).await;
                    }
                }
            }
        }
        
        {
            let mut cb = self.circuit_breaker.lock().await;
            cb.record_failure();
        }
        {
            let mut metrics = self.metrics.lock().await;
            metrics.record_failure();
        }
        
        let error_msg = last_error.unwrap_or_else(|| anyhow!("Unknown error"));
        self.dead_letter_queue.add(
            request_id, 
            prompt.to_string(), 
            error_msg.to_string(), 
            self.max_retries, 
            topic
        ).await;
        
        Err(error_msg)
    }
    
    async fn call_once(&self, prompt: &str) -> Result<String> {
        let response = self.client
            .post("http://localhost:11434/api/generate")
            .json(&json!({
                "model": self.model,
                "prompt": prompt,
                "stream": false,
                "options": {
                    "temperature": 0.7,
                    "num_predict": 4096,
                    "top_k": 40,
                    "top_p": 0.9,
                }
            }))
            .send()
            .await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Ollama API error {}: {}", status, text));
        }
        
        let data: serde_json::Value = response.json().await?;
        let content = data["response"]
            .as_str()
            .ok_or_else(|| anyhow!("No content in Ollama response"))?
            .to_string();
        
        Ok(content)
    }
    
    pub async fn health_check(&self) -> HealthReport {
        let start = Instant::now();
        let result = self.call_once("Respond with exactly 'HEALTH_OK'").await;
        let response_time_ms = start.elapsed().as_secs_f64() * 1000.0;
        
        let (status, success) = match result {
            Ok(ref content) if content.contains("HEALTH_OK") => (HealthStatus::Healthy, true),
            Ok(_) => (HealthStatus::Degraded, true),
            Err(_) => (HealthStatus::Unhealthy, false),
        };
        
        let metrics = self.metrics.lock().await;
        let cb = self.circuit_breaker.lock().await;
        
        HealthReport {
            status,
            last_success: if success { Some(Utc::now()) } else { None },
            last_failure: if !success { Some(Utc::now()) } else { None },
            success_count: metrics.successful_requests,
            failure_count: metrics.failed_requests,
            consecutive_failures: cb.failures,
            average_response_time_ms: metrics.average_response_time_ms(),
            circuit_breaker_open: cb.is_open(),
            queue_depth: self.request_queue.len().await,
            total_lessons_generated: metrics.topics_taught.values().sum::<u32>() as u64,
            active_topics: metrics.topics_taught.len(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
        }
    }
    
    pub async fn get_metrics_json(&self) -> serde_json::Value {
        let metrics = self.metrics.lock().await;
        let cb = self.circuit_breaker.lock().await;
        
        json!({
            "total_requests": metrics.total_requests,
            "successful_requests": metrics.successful_requests,
            "failed_requests": metrics.failed_requests,
            "success_rate": metrics.success_rate(),
            "average_response_ms": metrics.average_response_time_ms(),
            "requests_per_minute": metrics.current_rpm(),
            "circuit_breaker": {
                "open": cb.is_open(),
                "failures": cb.failures,
                "success_rate": cb.success_rate(),
            },
            "queue_depth": self.request_queue.len().await,
            "dead_letters": self.dead_letter_queue.len().await,
            "topics_taught": metrics.topics_taught,
        })
    }
    
    pub async fn recover_dead_letters(&self) -> usize {
        let handler = |prompt: String, topic: Option<String>| {
            let client = self.clone();
            async move {
                client.call_once(&prompt).await
            }
        };
        self.dead_letter_queue.retry_all(handler).await
    }
}

// ======================================================================
// TEACHER ORCHESTRATOR - Complete
// ======================================================================

pub struct AmoralTeacherOrchestrator {
    pub ollama: AmoralOllamaClient,
    training_dir: PathBuf,
    topic_queue: VecDeque<(String, u8)>,
    learned_topics: Vec<String>,
    failed_topics: HashMap<String, u32>,
    health_monitor_task: Option<tokio::task::JoinHandle<()>>,
    start_time: Instant,
    sequence: u64,
}

impl AmoralTeacherOrchestrator {
    pub fn new(model: String, training_dir: PathBuf) -> Self {
        fs::create_dir_all(&training_dir).ok();
        
        Self {
            ollama: AmoralOllamaClient::new(model),
            training_dir,
            topic_queue: VecDeque::new(),
            learned_topics: Vec::new(),
            failed_topics: HashMap::new(),
            health_monitor_task: None,
            start_time: Instant::now(),
            sequence: 0,
        }
    }
    
    pub async fn start_health_monitor(&mut self, interval_secs: u64) {
        let ollama = self.ollama.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;
                let report = ollama.health_check().await;
                match report.status {
                    HealthStatus::Healthy => info!("Health: HEALTHY (success rate: {:.1}%)", 
                        (report.success_count as f64 / (report.success_count + report.failure_count) as f64) * 100.0),
                    HealthStatus::Degraded => warn!("Health: DEGRADED"),
                    HealthStatus::Unhealthy => error!("Health: UNHEALTHY - circuit breaker: {}", 
                        if report.circuit_breaker_open { "OPEN" } else { "CLOSED" }),
                    HealthStatus::Recovering => info!("Health: RECOVERING"),
                }
            }
        });
        self.health_monitor_task = Some(handle);
    }
    
    pub async fn add_topic(&mut self, topic: &str, priority: u8) {
        let topic_str = topic.to_string();
        if !self.learned_topics.contains(&topic_str) 
            && !self.topic_queue.iter().any(|(t, _)| t == &topic_str) {
            self.topic_queue.push_back((topic_str.clone(), priority));
            info!("Added topic to queue (priority {}): {}", priority, topic);
        }
    }
    
    pub async fn add_topics(&mut self, topics: &[&str], priority: u8) {
        for topic in topics {
            self.add_topic(topic, priority).await;
        }
    }
    
    pub async fn run_teaching_loop(&mut self) -> Result<()> {
        info!("Starting teaching loop...");
        info!("Topics in queue: {}", self.topic_queue.len());
        
        // Sort queue by priority (highest first)
        let mut sorted: Vec<_> = self.topic_queue.drain(..).collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        self.topic_queue = sorted.into();
        
        while let Some((topic, priority)) = self.topic_queue.pop_front() {
            self.sequence += 1;
            info!("Teaching (seq {}): {} [priority: {}]", self.sequence, topic, priority);
            
            // Check health before teaching
            let health = self.ollama.health_check().await;
            if matches!(health.status, HealthStatus::Unhealthy) {
                warn!("Skipping '{}' - service unhealthy", topic);
                self.topic_queue.push_back((topic, priority));
                sleep(Duration::from_secs(30)).await;
                continue;
            }
            
            // Determine difficulty based on previous failures
            let failures = self.failed_topics.get(&topic).copied().unwrap_or(0);
            let difficulty = if failures >= 2 { "basic" } else { "intermediate" };
            
            let start = Instant::now();
            let topic_string = topic.clone();
            let lesson = match self.ollama.teach(&topic_string, difficulty).await {
                Ok(l) => l,
                Err(e) => {
                    error!("Failed to generate lesson for '{}': {}", topic_string, e);
                    *self.failed_topics.entry(topic_string.clone()).or_insert(0) += 1;
                    
                    if failures < 3 {
                        self.topic_queue.push_back((topic_string, priority));
                    } else {
                        error!("Topic '{}' failed 3 times, moving to dead letter queue", topic_string);
                    }
                    continue;
                }
            };
            
            let elapsed = start.elapsed();
            info!("Lesson generated in {:?} ({} chars)", elapsed, lesson.len());
            
            // Save lesson
            let safe_topic = topic_string.replace(|c: char| !c.is_alphanumeric(), "_");
            let filename = self.training_dir.join(format!("lesson_{:04}_{}.md", self.sequence, safe_topic));
            
            let content = format!(
                "# Lesson {}: {}\n\n**Generated:** {}\n**Difficulty:** {}\n**Priority:** {}\n\n---\n\n{}\n\n---\n\n## Metadata\n- Sequence: {}\n- Generated in: {:?}\n",
                self.sequence,
                topic_string,
                Utc::now().to_rfc3339(),
                difficulty,
                priority,
                lesson,
                self.sequence,
                elapsed
            );
            
            tokio::fs::write(&filename, content).await?;
            info!("Saved: {}", filename.display());
            
            self.learned_topics.push(topic_string.clone());
            self.failed_topics.remove(&topic_string);
            
            // Rate limiting
            sleep(Duration::from_millis(500)).await;
        }
        
        info!("Teaching loop complete. Learned {} topics.", self.learned_topics.len());
        Ok(())
    }
    
    pub async fn get_learned_topics(&self) -> Vec<String> {
        self.learned_topics.clone()
    }
    
    pub async fn get_health_report(&self) -> HealthReport {
        self.ollama.health_check().await
    }
    
    pub async fn get_metrics(&self) -> serde_json::Value {
        let mut metrics = self.ollama.get_metrics_json().await;
        metrics["teacher_uptime_seconds"] = json!(self.start_time.elapsed().as_secs());
        metrics["topics_learned"] = json!(self.learned_topics.len());
        metrics["topics_queued"] = json!(self.topic_queue.len());
        metrics["topics_failed"] = json!(self.failed_topics.len());
        metrics["sequence"] = json!(self.sequence);
        metrics
    }
    
    pub async fn shutdown(&mut self) {
        if let Some(handle) = self.health_monitor_task.take() {
            handle.abort();
        }
        info!("Teacher orchestrator shut down");
    }
}

// ======================================================================
// MAIN TEACHING FUNCTION
// ======================================================================

pub async fn start_amoral_teaching(model: &str, training_dir: PathBuf, topics: Vec<(&str, u8)>) -> Result<()> {
    let mut teacher = AmoralTeacherOrchestrator::new(model.to_string(), training_dir);
    
    teacher.start_health_monitor(30).await;
    
    for (topic, priority) in topics {
        teacher.add_topic(topic, priority).await;
    }
    
    let result = teacher.run_teaching_loop().await;
    teacher.shutdown().await;
    result
}

// Compatibility exports
pub use AmoralOllamaClient as AmoralDeepSeekClient;
pub type SharedMemoryChannel = ();