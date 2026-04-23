// ======================================================================
// CONVERSATION MANAGEMENT - PRODUCTION READY
// File: src/inference/conversation.rs
// Description: Manages conversation history, context window, and memory
//              Supports multi-turn dialogue, summarization, and persistence
// ======================================================================

use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use uuid::Uuid;

// ======================================================================
// MESSAGE
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub token_count: usize,
    pub id: String,
}

impl Message {
    pub fn new(role: &str, content: &str) -> Self {
        Self {
            role: role.to_string(),
            content: content.to_string(),
            timestamp: Utc::now(),
            token_count: Self::estimate_tokens(content),
            id: Uuid::new_v4().to_string(),
        }
    }
    
    pub fn estimate_tokens(text: &str) -> usize {
        text.len() / 4
    }
}

// ======================================================================
// CONVERSATION
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub messages: VecDeque<Message>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
    pub title: Option<String>,
}

impl Conversation {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            messages: VecDeque::new(),
            created_at: now,
            updated_at: now,
            metadata: serde_json::json!({}),
            title: None,
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
            title: None,
        }
    }
    
    pub fn add_message(&mut self, role: &str, content: &str) {
        self.messages.push_back(Message::new(role, content));
        self.updated_at = Utc::now();
        
        if self.title.is_none() && self.messages.len() == 1 {
            self.title = Some(Self::generate_title(content));
        }
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
    
    pub fn get_formatted_messages(&self) -> Vec<(String, String)> {
        self.messages
            .iter()
            .map(|m| (m.role.clone(), m.content.clone()))
            .collect()
    }
    
    pub fn trim_to_token_limit(&mut self, max_tokens: usize) {
        let mut total_tokens: usize = self.messages.iter().map(|m| m.token_count).sum();
        let mut trim_index = 0;
        
        for (i, msg) in self.messages.iter().enumerate() {
            if total_tokens <= max_tokens {
                break;
            }
            total_tokens = total_tokens.saturating_sub(msg.token_count);
            trim_index = i + 1;
        }
        
        if trim_index > 0 {
            self.messages = self.messages.split_off(trim_index);
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
    
    pub fn total_tokens(&self) -> usize {
        self.messages.iter().map(|m| m.token_count).sum()
    }
    
    fn generate_title(first_message: &str) -> String {
        let words: Vec<&str> = first_message.split_whitespace().take(5).collect();
        let title = words.join(" ");
        if title.len() > 50 {
            format!("{}...", &title[..47])
        } else {
            title
        }
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new()
    }
}

// ======================================================================
// CONVERSATION MANAGER
// ======================================================================

#[derive(Debug, Clone, Default)]
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
    
    pub fn create_conversation_with_title(&mut self, title: &str) -> String {
        let mut conv = Conversation::new();
        conv.title = Some(title.to_string());
        let id = conv.id.clone();
        self.conversations.insert(id.clone(), conv);
        self.active_id = Some(id.clone());
        id
    }
    
    pub fn get_conversation(&self, id: &str) -> Option<&Conversation> {
        self.conversations.get(id)
    }
    
    pub fn get_conversation_mut(&mut self, id: &str) -> Option<&mut Conversation> {
        self.conversations.get_mut(id)
    }
    
    pub fn get_active(&self) -> Option<&Conversation> {
        self.active_id
            .as_ref()
            .and_then(|id| self.conversations.get(id))
    }
    
    pub fn get_active_mut(&mut self) -> Option<&mut Conversation> {
        self.active_id
            .as_ref()
            .and_then(|id| self.conversations.get_mut(id))
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
        if let Some(conv) = self.get_active_mut() {
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
    
    pub fn add_system_message(&mut self, content: &str) -> bool {
        self.add_message("system", content)
    }
    
    pub fn get_formatted_prompt(&self, system_prompt: Option<&str>, context: Option<&str>) -> String {
        if let Some(conv) = self.get_active() {
            conv.get_formatted(system_prompt, context)
        } else {
            String::new()
        }
    }
    
    pub fn list_conversations(&self) -> Vec<(String, DateTime<Utc>, usize, Option<String>)> {
        self.conversations
            .iter()
            .map(|(id, conv)| (id.clone(), conv.updated_at, conv.len(), conv.title.clone()))
            .collect()
    }
    
    pub fn delete_conversation(&mut self, id: &str) -> bool {
        if self.active_id.as_ref() == Some(&id.to_string()) {
            self.active_id = None;
        }
        self.conversations.remove(id).is_some()
    }
    
    pub fn clear_active(&mut self) -> bool {
        if let Some(conv) = self.get_active_mut() {
            conv.clear();
            true
        } else {
            false
        }
    }
    
    pub fn save_to_file(&self, path: &str) -> Result<(), String> {
        let data = serde_json::to_string_pretty(&self.conversations)
            .map_err(|e| e.to_string())?;
        std::fs::write(path, data).map_err(|e| e.to_string())
    }
    
    pub fn load_from_file(&mut self, path: &str) -> Result<(), String> {
        let data = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        self.conversations = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        Ok(())
    }
    
    pub fn export_conversation(&self, id: &str, format: &str) -> Result<String, String> {
        let conv = self.conversations.get(id).ok_or("Conversation not found")?;
        
        match format {
            "json" => serde_json::to_string_pretty(conv).map_err(|e| e.to_string()),
            "text" => {
                let mut text = String::new();
                for msg in &conv.messages {
                    text.push_str(&format!("[{}] {}: {}\n", msg.timestamp, msg.role, msg.content));
                }
                Ok(text)
            }
            "markdown" => {
                let mut md = format!("# {}\n\n", conv.title.as_deref().unwrap_or("Conversation"));
                for msg in &conv.messages {
                    md.push_str(&format!("**{}**: {}\n\n", msg.role, msg.content));
                }
                Ok(md)
            }
            _ => Err("Unsupported format".to_string()),
        }
    }
    
    pub fn search_conversations(&self, query: &str) -> Vec<String> {
        let query_lower = query.to_lowercase();
        self.conversations
            .iter()
            .filter(|(_, conv)| {
                conv.messages.iter().any(|m| m.content.to_lowercase().contains(&query_lower))
            })
            .map(|(id, _)| id.clone())
            .collect()
    }
    
    pub fn merge_conversations(&mut self, id1: &str, id2: &str) -> Result<String, String> {
        let conv2 = self.conversations.remove(id2).ok_or("Second conversation not found")?;
        let conv1 = self.conversations.get_mut(id1).ok_or("First conversation not found")?;
        
        for msg in conv2.messages {
            conv1.messages.push_back(msg);
        }
        conv1.updated_at = Utc::now();
        
        Ok(id1.to_string())
    }
}

// ======================================================================
// TESTS
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_message_creation() {
        let msg = Message::new("user", "Hello, world!");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Hello, world!");
        assert!(msg.token_count > 0);
    }
    
    #[test]
    fn test_conversation_creation() {
        let conv = Conversation::new();
        assert!(conv.messages.is_empty());
        assert_eq!(conv.len(), 0);
    }
    
    #[test]
    fn test_add_message() {
        let mut conv = Conversation::new();
        conv.add_user_message("Hello");
        conv.add_assistant_message("Hi there!");
        
        assert_eq!(conv.len(), 2);
        assert_eq!(conv.get_last_message().unwrap().role, "assistant");
    }
    
    #[test]
    fn test_trim_to_token_limit() {
        let mut conv = Conversation::new();
        conv.add_user_message("This is a test message with many words in it.");
        conv.add_assistant_message("This is another response with plenty of content.");
        
        let total_before = conv.total_tokens();
        conv.trim_to_token_limit(10);
        let total_after = conv.total_tokens();
        
        assert!(total_after <= total_before);
    }
    
    #[test]
    fn test_conversation_manager() {
        let mut manager = ConversationManager::new(1000);
        
        let id = manager.create_conversation();
        assert!(manager.set_active(&id));
        
        manager.add_user_message("Hello");
        manager.add_assistant_message("Hi!");
        
        let conv = manager.get_active().unwrap();
        assert_eq!(conv.len(), 2);
    }
    
    #[test]
    fn test_list_conversations() {
        let mut manager = ConversationManager::new(1000);
        manager.create_conversation();
        manager.create_conversation();
        
        let list = manager.list_conversations();
        assert_eq!(list.len(), 2);
    }
    
    #[test]
    fn test_delete_conversation() {
        let mut manager = ConversationManager::new(1000);
        let id = manager.create_conversation();
        
        assert!(manager.delete_conversation(&id));
        assert!(manager.get_conversation(&id).is_none());
    }
    
    #[test]
    fn test_search_conversations() {
        let mut manager = ConversationManager::new(1000);
        let id = manager.create_conversation();
        manager.set_active(&id);
        manager.add_user_message("This contains the word blockchain");
        
        let results = manager.search_conversations("blockchain");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], id);
    }
    
    #[test]
    fn test_export_conversation() {
        let mut manager = ConversationManager::new(1000);
        let id = manager.create_conversation();
        manager.set_active(&id);
        manager.add_user_message("Hello");
        
        let json = manager.export_conversation(&id, "json");
        assert!(json.is_ok());
        
        let text = manager.export_conversation(&id, "text");
        assert!(text.is_ok());
        
        let md = manager.export_conversation(&id, "markdown");
        assert!(md.is_ok());
    }
}