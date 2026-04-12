// ======================================================================
// CONVERSATION MANAGEMENT - PRODUCTION READY
// File: src/inference/conversation.rs
// Description: Manages conversation history, context window, and memory
//              Supports multi-turn dialogue, summarization, and persistence
// ======================================================================

use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub token_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub messages: VecDeque<Message>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

impl Conversation {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            messages: VecDeque::new(),
            created_at: now,
            updated_at: now,
            metadata: serde_json::json!({}),
        }
    }
    
    pub fn with_id(id: &str) -> Self {
        let now = Utc::now();
        Self {
            id: id.to_string(),
            messages: VecDeque::new(),
            created_at: now,
            updated_at: now,
            metadata: serde_json::json!({}),
        }
    }
    
    pub fn add_message(&mut self, role: &str, content: &str) {
        let token_count = self.estimate_tokens(content);
        self.messages.push_back(Message {
            role: role.to_string(),
            content: content.to_string(),
            timestamp: Utc::now(),
            token_count,
        });
        self.updated_at = Utc::now();
    }
    
    pub fn add_user_message(&mut self, content: &str) {
        self.add_message("user", content);
    }
    
    pub fn add_assistant_message(&mut self, content: &str) {
        self.add_message("assistant", content);
    }
    
    pub fn add_system_message(&mut self, content: &str) {
        self.add_message("system", content);
    }
    
    pub fn get_messages(&self) -> Vec<Message> {
        self.messages.iter().cloned().collect()
    }
    
    pub fn get_last_message(&self) -> Option<&Message> {
        self.messages.back()
    }
    
    pub fn get_formatted(&self, system_prompt: Option<&str>, context: Option<&str>) -> String {
        let mut formatted = String::new();
        
        if let Some(sys) = system_prompt {
            formatted.push_str(&format!("System: {}\n\n", sys));
        }
        
        if let Some(ctx) = context {
            if !ctx.is_empty() {
                formatted.push_str(&format!("Context:\n{}\n\n", ctx));
            }
        }
        
        for msg in &self.messages {
            if msg.role != "system" {
                formatted.push_str(&format!("{}: {}\n", msg.role, msg.content));
            }
        }
        
        formatted.push_str("assistant: ");
        formatted
    }
    
    pub fn trim_to_token_limit(&mut self, max_tokens: usize) {
        let mut total_tokens = 0;
        let mut trim_index = 0;
        
        // Count from the end (keep recent messages)
        for (i, msg) in self.messages.iter().rev().enumerate() {
            total_tokens += msg.token_count;
            if total_tokens > max_tokens {
                trim_index = self.messages.len() - i;
                break;
            }
        }
        
        if trim_index > 0 {
            let mut new_messages = VecDeque::new();
            for msg in self.messages.iter().skip(trim_index) {
                new_messages.push_back(msg.clone());
            }
            self.messages = new_messages;
        }
    }
    
    pub fn summarize(&self, max_length: usize) -> String {
        let mut summary = String::new();
        let mut current_length = 0;
        
        for msg in &self.messages {
            let msg_str = format!("{}: {}\n", msg.role, msg.content);
            if current_length + msg_str.len() > max_length {
                summary.push_str("...");
                break;
            }
            summary.push_str(&msg_str);
            current_length += msg_str.len();
        }
        
        summary
    }
    
    pub fn clear(&mut self) {
        self.messages.clear();
        self.updated_at = Utc::now();
    }
    
    pub fn len(&self) -> usize {
        self.messages.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
    
    fn estimate_tokens(&self, text: &str) -> usize {
        // Rough estimate: ~4 characters per token
        text.len() / 4
    }
}

pub struct ConversationManager {
    conversations: std::collections::HashMap<String, Conversation>,
    active_id: Option<String>,
    max_tokens_per_conv: usize,
}

impl ConversationManager {
    pub fn new(max_tokens_per_conv: usize) -> Self {
        Self {
            conversations: std::collections::HashMap::new(),
            active_id: None,
            max_tokens_per_conv,
        }
    }
    
    pub fn create_conversation(&mut self) -> String {
        let conv = Conversation::new();
        let id = conv.id.clone();
        self.conversations.insert(id.clone(), conv);
        self.active_id = Some(id.clone());
        id
    }
    
    pub fn get_conversation(&mut self, id: &str) -> Option<&mut Conversation> {
        self.conversations.get_mut(id)
    }
    
    pub fn get_active(&mut self) -> Option<&mut Conversation> {
        if let Some(id) = &self.active_id {
            self.conversations.get_mut(id)
        } else {
            None
        }
    }
    
    pub fn set_active(&mut self, id: &str) -> bool {
        if self.conversations.contains_key(id) {
            self.active_id = Some(id.to_string());
            true
        } else {
            false
        }
    }
    
    pub fn add_message(&mut self, role: &str, content: &str) -> bool {
        if let Some(conv) = self.get_active() {
            conv.add_message(role, content);
            conv.trim_to_token_limit(self.max_tokens_per_conv);
            true
        } else {
            false
        }
    }
    
    pub fn add_user_message(&mut self, content: &str) -> bool {
        self.add_message("user", content)
    }
    
    pub fn add_assistant_message(&mut self, content: &str) -> bool {
        self.add_message("assistant", content)
    }
    
    pub fn get_formatted_prompt(&self, system_prompt: Option<&str>, context: Option<&str>) -> String {
        if let Some(conv) = self.conversations.get(self.active_id.as_ref()?) {
            conv.get_formatted(system_prompt, context)
        } else {
            String::new()
        }
    }
    
    pub fn list_conversations(&self) -> Vec<(String, DateTime<Utc>, usize)> {
        self.conversations
            .iter()
            .map(|(id, conv)| (id.clone(), conv.updated_at, conv.len()))
            .collect()
    }
    
    pub fn delete_conversation(&mut self, id: &str) -> bool {
        if self.active_id.as_ref() == Some(&id.to_string()) {
            self.active_id = None;
        }
        self.conversations.remove(id).is_some()
    }
    
    pub fn save_to_file(&self, path: &str) -> Result<(), String> {
        let data = serde_json::to_string_pretty(&self.conversations)
            .map_err(|e| e.to_string())?;
        std::fs::write(path, data).map_err(|e| e.to_string())?;
        Ok(())
    }
    
    pub fn load_from_file(&mut self, path: &str) -> Result<(), String> {
        let data = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        self.conversations = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new()
    }
}
