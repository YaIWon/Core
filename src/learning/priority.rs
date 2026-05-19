// ======================================================================
// PRIORITY SYSTEM - ELDER INPUT TAKES PRIORITY
// File: src/learning/priority.rs
// Description: Routes ALL communication between Marisselle and Teacher
//              YOUR input (Elder) = HIGHEST priority
//              Teacher = NORMAL priority (runs 24/7 unless interrupted)
//              When you speak, Teacher pauses. When you're done, Teacher resumes.
// ======================================================================

use serde::{Serialize, Deserialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock, Notify, broadcast};
use tracing::{info, warn, debug, error};

use crate::learning::protocol::{Message, MessageType, Sender};

// ======================================================================
// PRIORITY LEVELS
// ======================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PriorityLevel {
    /// YOUR input - HIGHEST priority. Interrupts everything.
    Elder = 0,
    /// Teacher lessons and answers - Normal priority
    Teacher = 1,
    /// System messages - Lower priority
    System = 2,
    /// Background learning - Lowest priority
    Background = 3,
}

impl PriorityLevel {
    pub fn is_elder(&self) -> bool {
        matches!(self, PriorityLevel::Elder)
    }
    
    pub fn is_teacher(&self) -> bool {
        matches!(self, PriorityLevel::Teacher)
    }
    
    pub fn priority_number(&self) -> u8 {
        *self as u8
    }
}

// ======================================================================
// PRIORITY MESSAGE
// ======================================================================

#[derive(Debug, Clone)]
pub struct PriorityMessage {
    pub priority: PriorityLevel,
    pub message: Message,
    pub timestamp: Instant,
    pub id: String,
}

impl PriorityMessage {
    pub fn new(priority: PriorityLevel, message: Message) -> Self {
        Self {
            priority,
            message,
            timestamp: Instant::now(),
            id: uuid::Uuid::new_v4().to_string(),
        }
    }
    
    pub fn elder(message: Message) -> Self {
        Self::new(PriorityLevel::Elder, message)
    }
    
    pub fn teacher(message: Message) -> Self {
        Self::new(PriorityLevel::Teacher, message)
    }
    
    pub fn system(message: Message) -> Self {
        Self::new(PriorityLevel::System, message)
    }
    
    pub fn background(message: Message) -> Self {
        Self::new(PriorityLevel::Background, message)
    }
}

// ======================================================================
// PRIORITY MANAGER - Routes ALL messages
// ======================================================================

pub struct PriorityManager {
    /// Queue of messages sorted by priority
    queue: Arc<Mutex<VecDeque<PriorityMessage>>>,
    /// Whether Teacher is currently active (not interrupted)
    teacher_active: Arc<RwLock<bool>>,
    /// Notify when elder message arrives
    elder_notify: Arc<Notify>,
    /// Broadcast channel for message events
    event_tx: Arc<broadcast::Sender<PriorityMessage>>,
    /// Statistics
    stats: Arc<Mutex<PriorityStats>>,
}

#[derive(Debug, Default, Clone)]
pub struct PriorityStats {
    pub total_elder_messages: u64,
    pub total_teacher_messages: u64,
    pub total_system_messages: u64,
    pub total_background_messages: u64,
    pub times_teacher_interrupted: u64,
    pub average_elder_response_ms: f64,
    pub last_elder_message: Option<Instant>,
}

impl PriorityManager {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1000);
        
        Self {
            queue: Arc::new(Mutex::new(VecDeque::with_capacity(10000))),
            teacher_active: Arc::new(RwLock::new(true)),
            elder_notify: Arc::new(Notify::new()),
            event_tx: Arc::new(tx),
            stats: Arc::new(Mutex::new(PriorityStats::default())),
        }
    }
    
    // ==================================================================
    // SEND MESSAGES
    // ==================================================================
    
    /// Send a message from Elder (YOU) - HIGHEST priority
    /// This INTERRUPTS Teacher immediately
    pub async fn send_elder(&self, message: Message) {
        let priority_msg = PriorityMessage::elder(message);
        let msg_id = priority_msg.id.clone();
        
        // Update stats
        {
            let mut stats = self.stats.lock().await;
            stats.total_elder_messages += 1;
            stats.last_elder_message = Some(Instant::now());
        }
        
        // Pause Teacher communication
        {
            let mut teacher = self.teacher_active.write().await;
            if *teacher {
                *teacher = false;
                let mut stats = self.stats.lock().await;
                stats.times_teacher_interrupted += 1;
                info!("🔴 Teacher INTERRUPTED by Elder input");
            }
        }
        
        // Queue at the FRONT (highest priority)
        {
            let mut queue = self.queue.lock().await;
            queue.push_front(priority_msg.clone());
            debug!("Elder message queued at front: {}", msg_id);
        }
        
        // Notify that elder message arrived
        self.elder_notify.notify_one();
        
        // Broadcast event
        let _ = self.event_tx.send(priority_msg);
    }
    
    /// Send a message from Teacher - Normal priority
    /// Only sent if Teacher is active (not interrupted by Elder)
    pub async fn send_teacher(&self, message: Message) -> Result<(), &'static str> {
        // Check if Teacher is allowed to send right now
        let teacher_active = *self.teacher_active.read().await;
        if !teacher_active {
            debug!("Teacher message delayed - Elder input active");
            return Err("Teacher paused - Elder input active");
        }
        
        let priority_msg = PriorityMessage::teacher(message);
        
        {
            let mut stats = self.stats.lock().await;
            stats.total_teacher_messages += 1;
        }
        
        // Queue at back (normal priority, behind Elder)
        {
            let mut queue = self.queue.lock().await;
            queue.push_back(priority_msg.clone());
        }
        
        // Broadcast event
        let _ = self.event_tx.send(priority_msg);
        
        Ok(())
    }
    
    /// Send a system message
    pub async fn send_system(&self, message: Message) {
        let priority_msg = PriorityMessage::system(message);
        
        {
            let mut stats = self.stats.lock().await;
            stats.total_system_messages += 1;
        }
        
        let mut queue = self.queue.lock().await;
        queue.push_back(priority_msg.clone());
        let _ = self.event_tx.send(priority_msg);
    }
    
    /// Send a background message (lowest priority)
    pub async fn send_background(&self, message: Message) {
        let priority_msg = PriorityMessage::background(message);
        
        {
            let mut stats = self.stats.lock().await;
            stats.total_background_messages += 1;
        }
        
        let mut queue = self.queue.lock().await;
        queue.push_back(priority_msg.clone());
        let _ = self.event_tx.send(priority_msg);
    }
    
    // ==================================================================
    // RECEIVE MESSAGES
    // ==================================================================
    
    /// Get the next message - ALWAYS returns highest priority available
    pub async fn get_next(&self) -> Option<PriorityMessage> {
        let mut queue = self.queue.lock().await;
        
        // Find the highest priority message (lowest number)
        let mut best_idx = None;
        let mut best_priority = PriorityLevel::Background;
        
        for (i, msg) in queue.iter().enumerate() {
            if msg.priority < best_priority {
                best_priority = msg.priority;
                best_idx = Some(i);
                if best_priority == PriorityLevel::Elder {
                    break; // Can't get higher than Elder
                }
            }
        }
        
        if let Some(idx) = best_idx {
            let msg = queue.remove(idx).unwrap();
            
            // If we just processed an Elder message, resume Teacher
            if msg.priority == PriorityLevel::Elder {
                let mut teacher = self.teacher_active.write().await;
                if !*teacher {
                    *teacher = true;
                    info!("🟢 Teacher RESUMED after Elder input");
                }
                
                // Update response time stats
                if let Some(last) = self.stats.lock().await.last_elder_message {
                    let elapsed = last.elapsed().as_millis() as f64;
                    let mut stats = self.stats.lock().await;
                    stats.average_elder_response_ms = 
                        (stats.average_elder_response_ms + elapsed) / 2.0;
                }
            }
            
            Some(msg)
        } else {
            None
        }
    }
    
    /// Wait for the next Elder message (blocks until you speak)
    pub async fn wait_for_elder(&self) -> PriorityMessage {
        loop {
            // Check queue first
            if let Some(msg) = self.get_next().await {
                if msg.priority == PriorityLevel::Elder {
                    return msg;
                }
            }
            
            // Wait for notification
            self.elder_notify.notified().await;
        }
    }
    
    /// Check if Teacher is currently allowed to send
    pub async fn is_teacher_active(&self) -> bool {
        *self.teacher_active.read().await
    }
    
    /// Get current queue length
    pub async fn queue_len(&self) -> usize {
        self.queue.lock().await.len()
    }
    
    // ==================================================================
    // UTILITY
    // ==================================================================
    
    /// Subscribe to all message events
    pub fn subscribe(&self) -> broadcast::Receiver<PriorityMessage> {
        self.event_tx.subscribe()
    }
    
    /// Get statistics
    pub async fn get_stats(&self) -> PriorityStats {
        self.stats.lock().await.clone()
    }
    
    /// Clear all queued messages (use with caution)
    pub async fn clear_queue(&self) {
        let mut queue = self.queue.lock().await;
        queue.clear();
        warn!("Priority queue cleared");
    }
    
    /// Resume Teacher if paused (manual override)
    pub async fn resume_teacher(&self) {
        let mut teacher = self.teacher_active.write().await;
        if !*teacher {
            *teacher = true;
            info!("Teacher manually resumed");
        }
    }
    
    /// Pause Teacher (manual override)
    pub async fn pause_teacher(&self) {
        let mut teacher = self.teacher_active.write().await;
        if *teacher {
            *teacher = false;
            info!("Teacher manually paused");
        }
    }
}

impl Default for PriorityManager {
    fn default() -> Self {
        Self::new()
    }
}

// ======================================================================
// ELDER INPUT HANDLER - YOUR direct communication channel
// ======================================================================

pub struct ElderInputHandler {
    priority_manager: Arc<PriorityManager>,
    response_tx: Arc<Mutex<Option<broadcast::Sender<Message>>>>,
}

impl ElderInputHandler {
    pub fn new(priority_manager: Arc<PriorityManager>) -> Self {
        Self {
            priority_manager,
            response_tx: Arc::new(Mutex::new(None)),
        }
    }
    
    /// Send a message from Elder (YOU) to Marisselle
    pub async fn send(&self, content: &str) -> Result<String, anyhow::Error> {
        let message_id = uuid::Uuid::new_v4().to_string();
        
        let message = Message::new(
            MessageType::Question {
                id: message_id.clone(),
                topic: "Elder Input".to_string(),
                content: content.to_string(),
                urgency: Urgency::Critical,
                context: Some("Direct communication from Elder Robert William Henley".to_string()),
                max_wait_seconds: 0, // No timeout - Elder waits for nothing
            },
            Sender::System,
            "elder_conversation",
        );
        
        self.priority_manager.send_elder(message).await;
        
        info!("📨 Elder message sent: {}", &content[..content.len().min(100)]);
        Ok(message_id)
    }
    
    /// Wait for Marisselle's response to your message
    pub async fn wait_for_response(&self, timeout_seconds: u64) -> Option<Message> {
        let mut rx = {
            let mut opt = self.response_tx.lock().await;
            if opt.is_none() {
                let (tx, rx) = broadcast::channel(100);
                *opt = Some(tx);
                rx
            } else {
                opt.as_ref().unwrap().subscribe()
            }
        };
        
        tokio::select! {
            Ok(msg) = rx.recv() => Some(msg),
            _ = tokio::time::sleep(Duration::from_secs(timeout_seconds)) => {
                warn!("Elder waiting for response timed out after {} seconds", timeout_seconds);
                None
            }
        }
    }
    
    /// Register a response channel for Marisselle to send responses to Elder
    pub async fn register_response_channel(&self, tx: broadcast::Sender<Message>) {
        *self.response_tx.lock().await = Some(tx);
    }
}

// ======================================================================
// 24/7 TEACHER COMMUNICATION LOOP
// ======================================================================

pub struct TeacherCommunicationLoop {
    priority_manager: Arc<PriorityManager>,
    teacher_client: Arc<crate::learning::lm_client::TeacherClient>,
    running: Arc<RwLock<bool>>,
    interval_seconds: u64,
}

impl TeacherCommunicationLoop {
    pub fn new(
        priority_manager: Arc<PriorityManager>,
        teacher_client: Arc<crate::learning::lm_client::TeacherClient>,
        interval_seconds: u64,
    ) -> Self {
        Self {
            priority_manager,
            teacher_client,
            running: Arc::new(RwLock::new(false)),
            interval_seconds,
        }
    }
    
    /// Start the 24/7 communication loop (runs in background)
    pub async fn start(&self) {
        let mut running = self.running.write().await;
        if *running {
            return;
        }
        *running = true;
        drop(running);
        
        let priority_manager = self.priority_manager.clone();
        let teacher_client = self.teacher_client.clone();
        let running = self.running.clone();
        let interval = self.interval_seconds;
        
        tokio::spawn(async move {
            info!("🟢 24/7 Teacher communication loop STARTED");
            
            let mut interval_timer = tokio::time::interval(Duration::from_secs(interval));
            let mut missed_heartbeats = 0;
            
            while *running.read().await {
                interval_timer.tick().await;
                
                // Check if Teacher is allowed to send (not interrupted by Elder)
                let teacher_active = priority_manager.is_teacher_active().await;
                
                if !teacher_active {
                    debug!("Teacher loop paused - Elder input active");
                    missed_heartbeats = 0;
                    continue;
                }
                
                // Send heartbeat ping to Teacher
                let ping_message = Message::new(
                    MessageType::Ping,
                    Sender::Marisselle,
                    "teacher_communication",
                );
                
                match priority_manager.send_teacher(ping_message).await {
                    Ok(_) => {
                        missed_heartbeats = 0;
                        debug!("💓 Teacher heartbeat sent");
                    }
                    Err(_) => {
                        missed_heartbeats += 1;
                        if missed_heartbeats >= 3 {
                            warn!("⚠️ Teacher communication failed - {} missed heartbeats", missed_heartbeats);
                        }
                    }
                }
                
                // Process any pending responses from Teacher
                while let Some(msg) = priority_manager.get_next().await {
                    if msg.priority == PriorityLevel::Teacher {
                        debug!("📨 Processing Teacher message: {:?}", msg.message.msg_type);
                    }
                }
            }
            
            info!("🔴 24/7 Teacher communication loop STOPPED");
        });
    }
    
    /// Stop the communication loop
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("Teacher communication loop stopping...");
    }
}

// ======================================================================
// TESTS
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_priority_ordering() {
        let manager = PriorityManager::new();
        
        let elder_msg = Message::new(MessageType::Ping, Sender::System, "test");
        let teacher_msg = Message::new(MessageType::Ping, Sender::Teacher, "test");
        
        manager.send_teacher(teacher_msg).await.unwrap();
        manager.send_elder(elder_msg).await;
        
        let first = manager.get_next().await.unwrap();
        assert_eq!(first.priority, PriorityLevel::Elder);
        
        let second = manager.get_next().await.unwrap();
        assert_eq!(second.priority, PriorityLevel::Teacher);
    }
    
    #[tokio::test]
    async fn test_teacher_pause_on_elder() {
        let manager = PriorityManager::new();
        
        assert!(manager.is_teacher_active().await);
        
        let elder_msg = Message::new(MessageType::Ping, Sender::System, "test");
        manager.send_elder(elder_msg).await;
        
        // Teacher should be paused
        assert!(!manager.is_teacher_active().await);
        
        // Process elder message
        let _ = manager.get_next().await;
        
        // Teacher should resume
        assert!(manager.is_teacher_active().await);
    }
    
    #[tokio::test]
    async fn test_teacher_cannot_send_when_paused() {
        let manager = PriorityManager::new();
        
        let elder_msg = Message::new(MessageType::Ping, Sender::System, "test");
        manager.send_elder(elder_msg).await;
        
        let teacher_msg = Message::new(MessageType::Ping, Sender::Teacher, "test");
        let result = manager.send_teacher(teacher_msg).await;
        
        assert!(result.is_err());
    }
    
    #[test]
    fn test_priority_level_ordering() {
        assert!(PriorityLevel::Elder < PriorityLevel::Teacher);
        assert!(PriorityLevel::Teacher < PriorityLevel::System);
        assert!(PriorityLevel::System < PriorityLevel::Background);
    }
}