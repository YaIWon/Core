// ======================================================================
// LEARNING MODULE - ULTIMATE VERSION
// File: src/learning/mod.rs
// Description: Complete module exports for the learning system
//              Now with blockchain mining integration for proof-of-learning
// ======================================================================

pub mod amoral_teacher;
pub mod curriculum;
pub mod lm_client;
pub mod protocol;
pub mod logger;

// ======================================================================
// TEACHER RE-EXPORTS
// ======================================================================

pub use amoral_teacher::{
    AmoralTeacherOrchestrator,
    AmoralOllamaClient,
    AmoralOllamaClient as AmoralDeepSeekClient,
    HealthStatus,
    HealthReport,
    CircuitBreaker,
    RequestQueue,
    DeadLetterQueue,
    start_amoral_teaching,
};

// ======================================================================
// CURRICULUM RE-EXPORTS
// ======================================================================

pub use curriculum::{Curriculum, Topic};

// ======================================================================
// LM CLIENT RE-EXPORTS
// ======================================================================

pub use lm_client::{TeacherClient, ConfusionDetector, LearningCoordinator};

// ======================================================================
// PROTOCOL RE-EXPORTS
// ======================================================================

pub use protocol::{
    Message, MessageType, Sender, Urgency, AckStatus,
    ProtocolManager, ConversationManager, Conversation, ConversationStatus,
    LearningTracker, LearningRecord, MasteryLevel,
    CoherenceValidator, CoherenceResult,
    MessageTransport, PriorityQueue, ConversationStore,
};

// ======================================================================
// LOGGER RE-EXPORTS
// ======================================================================

pub use logger::{
    ComprehensiveLogger, LogEntry, LogLevel, LogCategory,
    DeepThinkEngine, InternetSearchEngine, SearchResult,
    AutonomousThinker, BackgroundTaskExecutor, AutonomousManager,
    TaskAction, TaskStatus, Goal, GoalStatus, ThoughtCategory, AutonomousThought,
};

// ======================================================================
// BLOCKCHAIN MINING INTEGRATION (NEW)
// ======================================================================

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use crate::blockchain::{UniversalBlockchainAccess, MiningResult, MiningStats};

/// Extended LearningCoordinator with mining capabilities
pub struct MiningLearningCoordinator {
    pub coordinator: LearningCoordinator,
    pub blockchain: Arc<RwLock<UniversalBlockchainAccess>>,
    pub mine_on_learning: bool,
    pub mine_on_confusion: bool,
    pub mine_on_answer: bool,
    pub total_blocks_mined: u64,
}

impl MiningLearningCoordinator {
    pub fn new(coordinator: LearningCoordinator, blockchain: Arc<RwLock<UniversalBlockchainAccess>>) -> Self {
        Self {
            coordinator,
            blockchain,
            mine_on_learning: true,
            mine_on_confusion: true,
            mine_on_answer: true,
            total_blocks_mined: 0,
        }
    }
    
    /// Learn something and mine a block as proof
    pub async fn learn_and_mine(&mut self, content: &str, source: &str) -> Result<Option<MiningResult>, anyhow::Error> {
        info!("📚 Learning: {} from {}", &content[..content.len().min(100)], source);
        
        // First, process the learning through the coordinator
        // This would integrate with your existing learning system
        
        // Then mine a block as proof of learning
        if self.mine_on_learning {
            let mut blockchain = self.blockchain.write().await;
            let mining_result = blockchain.mine_learning(content);
            
            if let Some(result) = mining_result {
                self.total_blocks_mined += 1;
                info!("⛏️ MINED BLOCK for learning from {}!", source);
                info!("   Block hash: {}", result.hash);
                info!("   Nonce: {}", result.nonce);
                info!("   Time: {}ms", result.duration_ms);
                info!("   Total blocks mined: {}", self.total_blocks_mined);
                return Ok(Some(result));
            }
        }
        
        Ok(None)
    }
    
    /// When confused, mine a block documenting the confusion
    pub async fn confusion_and_mine(&mut self, topic: &str, issue: &str) -> Result<Option<MiningResult>, anyhow::Error> {
        info!("🤔 Confused about {}: {}", topic, issue);
        
        if self.mine_on_confusion {
            let confusion_content = format!("Confused about {}: {}", topic, issue);
            let mut blockchain = self.blockchain.write().await;
            let mining_result = blockchain.mine_learning(&confusion_content);
            
            if let Some(result) = mining_result {
                self.total_blocks_mined += 1;
                info!("⛏️ MINED BLOCK documenting confusion about {}!", topic);
                return Ok(Some(result));
            }
        }
        
        Ok(None)
    }
    
    /// When receiving an answer, mine a block as proof of learning
    pub async fn answer_and_mine(&mut self, question_id: &str, answer: &str) -> Result<Option<MiningResult>, anyhow::Error> {
        info!("✅ Received answer for question: {}", question_id);
        
        if self.mine_on_answer {
            let answer_content = format!("Answer to {}: {}", question_id, &answer[..answer.len().min(200)]);
            let mut blockchain = self.blockchain.write().await;
            let mining_result = blockchain.mine_learning(&answer_content);
            
            if let Some(result) = mining_result {
                self.total_blocks_mined += 1;
                info!("⛏️ MINED BLOCK for learning from answer!");
                return Ok(Some(result));
            }
        }
        
        Ok(None)
    }
    
    pub fn get_mining_stats(&self) -> MiningStats {
        // This would need to be async in real implementation
        // For now, return placeholder
        MiningStats {
            total_hashes: 0,
            current_hashrate: 0,
            current_difficulty: 0,
            uptime_seconds: 0,
            blocks_mined: self.total_blocks_mined,
        }
    }
    
    pub fn set_mine_on_learning(&mut self, enabled: bool) {
        self.mine_on_learning = enabled;
        info!("Mine on learning: {}", enabled);
    }
    
    pub fn set_mine_on_confusion(&mut self, enabled: bool) {
        self.mine_on_confusion = enabled;
        info!("Mine on confusion: {}", enabled);
    }
    
    pub fn set_mine_on_answer(&mut self, enabled: bool) {
        self.mine_on_answer = enabled;
        info!("Mine on answer: {}", enabled);
    }
}

/// Extended AmoralTeacherOrchestrator with mining
pub struct MiningTeacherOrchestrator {
    pub orchestrator: AmoralTeacherOrchestrator,
    pub blockchain: Arc<RwLock<UniversalBlockchainAccess>>,
    pub mine_on_teach: bool,
}

impl MiningTeacherOrchestrator {
    pub fn new(orchestrator: AmoralTeacherOrchestrator, blockchain: Arc<RwLock<UniversalBlockchainAccess>>) -> Self {
        Self {
            orchestrator,
            blockchain,
            mine_on_teach: true,
        }
    }
    
    pub async fn teach_and_mine(&mut self, topic: &str, difficulty: &str) -> Result<String, anyhow::Error> {
        info!("📚 Teaching: {} at {} difficulty", topic, difficulty);
        
        // Generate the lesson
        let lesson = self.orchestrator.ollama.teach(topic, difficulty).await?;
        
        // Mine a block as proof of teaching
        if self.mine_on_teach {
            let teaching_content = format!("Taught lesson on {} at {} difficulty", topic, difficulty);
            let mut blockchain = self.blockchain.write().await;
            if let Some(result) = blockchain.mine_learning(&teaching_content) {
                info!("⛏️ MINED BLOCK for teaching {}!", topic);
                info!("   Block hash: {}", result.hash);
            }
        }
        
        Ok(lesson)
    }
}

// ======================================================================
// HELPER FUNCTIONS FOR MINING INTEGRATION
// ======================================================================

/// Create a new learning coordinator with mining support
pub async fn create_mining_coordinator(
    teacher_enabled: bool,
) -> Result<MiningLearningCoordinator, anyhow::Error> {
    let coordinator = LearningCoordinator::new()?;
    let blockchain = Arc::new(RwLock::new(UniversalBlockchainAccess::new()));
    
    // Initialize blockchain connections
    {
        let mut bc = blockchain.write().await;
        bc.init_ethereum("https://cloudflare-eth.com", 1);
        bc.start_mining();
        info!("🔗 Mining coordinator created with blockchain access");
    }
    
    Ok(MiningLearningCoordinator::new(coordinator, blockchain))
}

/// Record a learning event and mine a block
pub async fn record_learning_with_proof(
    blockchain: &Arc<RwLock<UniversalBlockchainAccess>>,
    content: &str,
    source: &str,
) -> Option<MiningResult> {
    info!("📝 Recording learning event from {}: {}", source, &content[..content.len().min(100)]);
    
    let mut bc = blockchain.write().await;
    bc.mine_learning(content)
}

// ======================================================================
// SHARED MEMORY
// ======================================================================

pub type SharedMemoryChannel = ();

// ======================================================================
// TESTS
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mining_learning_coordinator_creation() {
        let coordinator = LearningCoordinator::new().unwrap();
        let blockchain = Arc::new(RwLock::new(UniversalBlockchainAccess::new()));
        let mining_coord = MiningLearningCoordinator::new(coordinator, blockchain);
        
        assert!(mining_coord.mine_on_learning);
        assert!(mining_coord.mine_on_confusion);
        assert!(mining_coord.mine_on_answer);
        assert_eq!(mining_coord.total_blocks_mined, 0);
    }
    
    #[test]
    fn test_mining_settings() {
        let coordinator = LearningCoordinator::new().unwrap();
        let blockchain = Arc::new(RwLock::new(UniversalBlockchainAccess::new()));
        let mut mining_coord = MiningLearningCoordinator::new(coordinator, blockchain);
        
        mining_coord.set_mine_on_learning(false);
        mining_coord.set_mine_on_confusion(false);
        mining_coord.set_mine_on_answer(false);
        
        assert!(!mining_coord.mine_on_learning);
        assert!(!mining_coord.mine_on_confusion);
        assert!(!mining_coord.mine_on_answer);
    }
}