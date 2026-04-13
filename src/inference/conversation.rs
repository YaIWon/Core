// conversation.rs

use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct Message {
    pub user: String,
    pub content: String,
}

#[derive(Debug)]
pub struct Conversation {
    messages: VecDeque<Message>,
    max_history_size: usize,
}

impl Conversation {
    pub fn new(max_history_size: usize) -> Self {
        Conversation {
            messages: VecDeque::new(),
            max_history_size,
        }
    }

    pub fn add_message(&mut self, user: &str, content: &str) {
        // Add message to the conversation history
        let message = Message {
            user: user.to_string(),
            content: content.to_string(),
        };

        // Maintain the message history size
        if self.messages.len() == self.max_history_size {
            self.messages.pop_front();
        }
        self.messages.push_back(message);
    }

    pub fn get_history(&self) -> Vec<Message> {
        self.messages.iter().cloned().collect()
    }

    pub fn last_message(&self) -> Option<&Message> {
        self.messages.back()
    }

    pub fn clear_history(&mut self) {
        self.messages.clear();
    }
}

// Example usage:
// let mut conversation = Conversation::new(5);
// conversation.add_message("User", "Hello!");
// println!("{:?}", conversation.get_history());