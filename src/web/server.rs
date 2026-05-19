// ======================================================================
// FILE: src/web/server.rs
// PATH: /workspaces/Core/src/web/server.rs
// PURPOSE: HTTP/WebSocket server for Marisselle's web chat interface
//          Serves HTML/CSS/JS, handles file uploads, real-time streaming
//          Deep think button, time dilation (simulated), mining stats display
//          Swap pool integration at: 0xF88DF111343BffE7a2d89FB770d77A264d53f043
// ======================================================================

use anyhow::{Result, anyhow};
use std::path::PathBuf;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{RwLock, Mutex};
use tokio::fs;
use tokio::time::{sleep, Duration, Instant};
use axum::{
    Router, routing::{get, post}, extract::{Path, Query, State, WebSocketUpgrade, ws::{WebSocket, Message as WsMessage}},
    response::{Html, Json, IntoResponse}, http::StatusCode, body::Body,
};
use axum::extract::Multipart;
use serde::{Serialize, Deserialize};
use serde_json::json;
use tracing::{info, warn, error, debug};
use uuid::Uuid;
use chrono::Utc;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use listenfd::ListenFd;

use crate::security::vault::VaultManager;
use crate::security::wallet_generator::{WalletOrchestrator, SwapExecutor, MARISSELLE_SWAP_POOL};
use crate::mining::optimizer::OptimizationEngine;
use crate::mining::stratum::MiningOrchestrator;
use crate::learning::ComprehensiveLogger;

// ======================================================================
// CONSTANTS
// ======================================================================

const WEB_STATIC_DIR: &str = "web/static";
const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 3030;
const MAX_UPLOAD_SIZE: usize = 100 * 1024 * 1024;  // 100 MB
const DEEP_THINK_DURATION_MS: u64 = 3000;  // 3 seconds simulated

// ======================================================================
// WEB SERVER STATE
// ======================================================================

#[derive(Clone)]
pub struct AppState {
    pub vault: Arc<tokio::sync::Mutex<VaultManager>>,
    pub wallet_orchestrator: Arc<WalletOrchestrator>,
    pub swap_executor: Arc<SwapExecutor>,
    pub optimizer: Arc<OptimizationEngine>,
    pub mining_orchestrator: Arc<MiningOrchestrator>,
    pub logger: Arc<ComprehensiveLogger>,
    pub conversations: Arc<Mutex<HashMap<String, Vec<ChatMessage>>>>,
    pub deep_think_enabled: Arc<RwLock<bool>>,
    pub time_dilation_enabled: Arc<RwLock<bool>>,
    pub dilation_factor: Arc<RwLock<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,  // "user" or "assistant"
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub thinking_time_ms: Option<u64>,
    pub deep_think_used: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub conversation_id: Option<String>,
    pub deep_think: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub response: String,
    pub conversation_id: String,
    pub thinking_time_ms: u64,
    pub deep_think_used: bool,
    pub speed_bonus: f64,
    pub mining_stats: MiningStatsResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningStatsResponse {
    pub hashrate_hps: u64,
    pub accepted_shares: u64,
    pub speed_bonus_multiplier: f64,
    pub is_mining: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResponse {
    pub id: String,
    pub filename: String,
    pub size_bytes: u64,
    pub processed: bool,
    pub chunks_stored: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapQuoteResponse {
    pub pair: String,
    pub input_amount: f64,
    pub expected_output: f64,
    pub pool_address: String,
    pub no_gas_fee: bool,
}

// ======================================================================
// WEB SERVER
// ======================================================================

pub struct WebServer {
    state: AppState,
    host: String,
    port: u16,
}

impl WebServer {
    pub async fn new(
        vault: Arc<tokio::sync::Mutex<VaultManager>>,
        wallet_orchestrator: Arc<WalletOrchestrator>,
        swap_executor: Arc<SwapExecutor>,
        optimizer: Arc<OptimizationEngine>,
        mining_orchestrator: Arc<MiningOrchestrator>,
        logger: Arc<ComprehensiveLogger>,
    ) -> Self {
        Self {
            state: AppState {
                vault,
                wallet_orchestrator,
                swap_executor,
                optimizer,
                mining_orchestrator,
                logger,
                conversations: Arc::new(Mutex::new(HashMap::new())),
                deep_think_enabled: Arc::new(RwLock::new(false)),
                time_dilation_enabled: Arc::new(RwLock::new(false)),
                dilation_factor: Arc::new(RwLock::new(1.0)),
            },
            host: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT,
        }
    }
    
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }
    
    pub fn with_host(mut self, host: &str) -> Self {
        self.host = host.to_string();
        self
    }
    
    pub async fn run(&self) -> Result<()> {
        info!("🌐 Starting Marisselle Web Server on {}:{}", self.host, self.port);
        
        // Create static file directory if it doesn't exist
        let static_dir = PathBuf::from(WEB_STATIC_DIR);
        if !static_dir.exists() {
            fs::create_dir_all(&static_dir).await?;
            info!("Created static directory: {:?}", static_dir);
        }
        
        // Build router
        let app = Router::new()
            .route("/", get(serve_index))
            .route("/api/chat", post(handle_chat))
            .route("/api/chat/stream", get(handle_chat_websocket))
            .route("/api/upload", post(handle_upload))
            .route("/api/mining/stats", get(get_mining_stats))
            .route("/api/optimizer/stats", get(get_optimizer_stats))
            .route("/api/wallets", get(get_wallets))
            .route("/api/wallet/generate/{coin}", post(generate_wallet))
            .route("/api/swap/quote", post(get_swap_quote))
            .route("/api/swap/execute", post(execute_swap))
            .route("/api/deepthink/toggle", post(toggle_deep_think))
            .route("/api/dilation/toggle", post(toggle_time_dilation))
            .route("/api/dilation/factor/{factor}", post(set_dilation_factor))
            .route("/api/conversation/{id}", get(get_conversation))
            .route("/api/conversation/{id}/delete", delete(delete_conversation))
            .route("/api/conversations", get(list_conversations))
            .nest_service("/static", ServeDir::new(static_dir))
            .layer(CorsLayer::permissive())
            .with_state(self.state.clone());
        
        // Start server with graceful shutdown
        let listener = tokio::net::TcpListener::bind(format!("{}:{}", self.host, self.port)).await?;
        info!("✅ Web server listening on http://{}:{}", self.host, self.port);
        info!("   Chat interface: http://{}:{}/", self.host, self.port);
        info!("   Swap pool: {}", MARISSELLE_SWAP_POOL);
        
        axum::serve(listener, app).await?;
        
        Ok(())
    }
}

// ======================================================================
// HANDLERS
// ======================================================================

async fn serve_index() -> Html<String> {
    let index_path = PathBuf::from(WEB_STATIC_DIR).join("index.html");
    
    match fs::read_to_string(index_path).await {
        Ok(content) => Html(content),
        Err(_) => Html(generate_default_html().to_string()),
    }
}

async fn handle_chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, StatusCode> {
    let start = Instant::now();
    let conversation_id = req.conversation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    
    // Store user message
    let user_msg = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        content: req.message.clone(),
        timestamp: Utc::now(),
        thinking_time_ms: None,
        deep_think_used: false,
    };
    
    {
        let mut convs = state.conversations.lock().await;
        let conv = convs.entry(conversation_id.clone()).or_insert_with(Vec::new);
        conv.push(user_msg);
    }
    
    // Deep think simulation
    let deep_think_used = req.deep_think || *state.deep_think_enabled.read().await;
    if deep_think_used {
        state.logger.log_lm_thought("Deep thinking mode engaged...", None).await;
        // Simulate deep thinking
        sleep(Duration::from_millis(DEEP_THINK_DURATION_MS)).await;
    }
    
    // Time dilation simulation
    let dilation_enabled = *state.time_dilation_enabled.read().await;
    let dilation_factor = *state.dilation_factor.read().await;
    
    // Generate response (simulated - would call actual model)
    let thinking_time = start.elapsed().as_millis() as u64;
    let response_text = generate_response(&req.message, deep_think_used, dilation_enabled, dilation_factor).await;
    
    // Store assistant message
    let assistant_msg = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        content: response_text.clone(),
        timestamp: Utc::now(),
        thinking_time_ms: Some(thinking_time),
        deep_think_used,
    };
    
    {
        let mut convs = state.conversations.lock().await;
        if let Some(conv) = convs.get_mut(&conversation_id) {
            conv.push(assistant_msg);
        }
    }
    
    // Get mining stats
    let speed_bonus = state.optimizer.get_speed_bonus().await;
    let mining_stats = state.mining_orchestrator.get_total_stats().await;
    
    state.logger.log_lm_thought(&format!("Responded to: {}", &req.message[..req.message.len().min(50)]), None).await;
    
    Ok(Json(ChatResponse {
        id: Uuid::new_v4().to_string(),
        response: response_text,
        conversation_id,
        thinking_time_ms: thinking_time,
        deep_think_used,
        speed_bonus,
        mining_stats: MiningStatsResponse {
            hashrate_hps: mining_stats.current_hashrate,
            accepted_shares: mining_stats.accepted_shares,
            speed_bonus_multiplier: speed_bonus,
            is_mining: true,
        },
    }))
}

async fn handle_chat_websocket(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

async fn handle_websocket(mut socket: WebSocket, state: AppState) {
    let mut conversation_id = Uuid::new_v4().to_string();
    
    while let Some(Ok(msg)) = socket.recv().await {
        if let WsMessage::Text(text) = msg {
            if let Ok(req) = serde_json::from_str::<ChatRequest>(&text) {
                conversation_id = req.conversation_id.clone().unwrap_or(conversation_id.clone());
                
                // Simulate streaming response
                let response_text = generate_response(&req.message, req.deep_think, false, 1.0).await;
                
                // Send response back
                let response = ChatResponse {
                    id: Uuid::new_v4().to_string(),
                    response: response_text,
                    conversation_id: conversation_id.clone(),
                    thinking_time_ms: 100,
                    deep_think_used: req.deep_think,
                    speed_bonus: 1.0,
                    mining_stats: MiningStatsResponse {
                        hashrate_hps: 0,
                        accepted_shares: 0,
                        speed_bonus_multiplier: 1.0,
                        is_mining: true,
                    },
                };
                
                if let Ok(json) = serde_json::to_string(&response) {
                    let _ = socket.send(WsMessage::Text(json)).await;
                }
            }
        }
    }
}

async fn handle_upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, StatusCode> {
    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap_or("unknown").to_string();
        let filename = field.file_name().unwrap_or("unknown").to_string();
        let data = field.bytes().await.unwrap();
        
        state.logger.log_file_read(&PathBuf::from(&filename), data.len() as u64).await;
        state.logger.log_lm_thought(&format!("File uploaded: {} ({} bytes)", filename, data.len()), None).await;
        
        // Store file in training_data directory
        let training_dir = PathBuf::from("training_data");
        let file_path = training_dir.join(&filename);
        
        if let Err(e) = fs::write(&file_path, data).await {
            error!("Failed to save uploaded file: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        
        state.logger.log_knowledge_integrated(&filename, 1).await;
        
        return Ok(Json(UploadResponse {
            id: Uuid::new_v4().to_string(),
            filename,
            size_bytes: data.len() as u64,
            processed: true,
            chunks_stored: 1,
        }));
    }
    
    Err(StatusCode::BAD_REQUEST)
}

async fn get_mining_stats(State(state): State<AppState>) -> Json<MiningStatsResponse> {
    let stats = state.mining_orchestrator.get_total_stats().await;
    let speed_bonus = state.optimizer.get_speed_bonus().await;
    
    Json(MiningStatsResponse {
        hashrate_hps: stats.current_hashrate,
        accepted_shares: stats.accepted_shares,
        speed_bonus_multiplier: speed_bonus,
        is_mining: true,
    })
}

async fn get_optimizer_stats(State(state): State<AppState>) -> Json<serde_json::Value> {
    let speed_bonus = state.optimizer.get_speed_bonus().await;
    let experiments = state.optimizer.get_experiments().await;
    
    Json(json!({
        "speed_bonus_multiplier": speed_bonus,
        "experiment_count": experiments.len(),
        "current_hashrate": state.mining_orchestrator.get_total_stats().await.current_hashrate,
        "optimizations": experiments.iter().map(|e| json!({
            "name": e.name,
            "improvement_factor": e.improvement_factor,
            "applied_at": e.applied_at,
        })).collect::<Vec<_>>(),
    }))
}

async fn get_wallets(State(state): State<AppState>) -> Json<serde_json::Value> {
    let monero = state.wallet_orchestrator.get_address("monero").await;
    let bitcoin = state.wallet_orchestrator.get_address("bitcoin").await;
    let ethereum = state.wallet_orchestrator.get_address("ethereum").await;
    
    Json(json!({
        "monero": monero,
        "bitcoin": bitcoin,
        "ethereum": ethereum,
        "swap_pool": MARISSELLE_SWAP_POOL,
    }))
}

async fn generate_wallet(
    State(state): State<AppState>,
    Path(coin): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.wallet_orchestrator.generate_and_store(&coin, true).await {
        Ok(wallet) => Ok(Json(json!({
            "success": true,
            "coin": wallet.coin,
            "address": wallet.address,
            "message": format!("{} wallet generated and stored in vault", coin),
        }))),
        Err(e) => {
            error!("Failed to generate wallet: {}", e);
            Ok(Json(json!({
                "success": false,
                "error": e.to_string(),
            })))
        }
    }
}

async fn get_swap_quote(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<SwapQuoteResponse>, StatusCode> {
    let pair_str = req["pair"].as_str().unwrap_or("monero_to_mrl");
    let amount = req["amount"].as_f64().unwrap_or(1.0);
    
    use crate::security::wallet_generator::SwapPair;
    let pair = SwapPair::from_str(pair_str).unwrap_or(SwapPair::MoneroToMRL);
    
    match state.swap_executor.get_quote(pair, amount).await {
        Ok(quote) => Ok(Json(SwapQuoteResponse {
            pair: format!("{:?}", quote.pair),
            input_amount: quote.input_amount,
            expected_output: quote.expected_output,
            pool_address: quote.pool_address,
            no_gas_fee: quote.no_gas_fee,
        })),
        Err(e) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn execute_swap(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Get wallet first
    let wallet_opt = state.wallet_orchestrator.get_address("monero").await;
    let wallet = match wallet_opt {
        Some(addr) => {
            // Create a minimal wallet entry for swap execution
            use crate::security::vault::WalletEntry;
            WalletEntry {
                coin: "monero".to_string(),
                address: addr,
                private_key_encrypted: vec![],
                mnemonic_encrypted: None,
                created_at: Utc::now(),
                last_used: None,
                balance: 0.0,
                total_mined: 0.0,
                notes: None,
            }
        }
        None => return Err(StatusCode::BAD_REQUEST),
    };
    
    let pair_str = req["pair"].as_str().unwrap_or("monero_to_mrl");
    let amount = req["amount"].as_f64().unwrap_or(1.0);
    
    use crate::security::wallet_generator::SwapPair;
    let pair = SwapPair::from_str(pair_str).unwrap_or(SwapPair::MoneroToMRL);
    
    match state.swap_executor.execute_swap(pair, amount, &wallet).await {
        Ok(result) => Ok(Json(json!({
            "success": result.success,
            "pair": format!("{:?}", result.pair),
            "input_amount": result.input_amount,
            "output_amount": result.output_amount,
            "transaction_id": result.transaction_id,
            "pool_address": MARISSELLE_SWAP_POOL,
        }))),
        Err(e) => Ok(Json(json!({
            "success": false,
            "error": e.to_string(),
        }))),
    }
}

async fn toggle_deep_think(State(state): State<AppState>) -> Json<serde_json::Value> {
    let mut enabled = state.deep_think_enabled.write().await;
    *enabled = !*enabled;
    
    state.logger.log_lm_thought(&format!("Deep think mode: {}", if *enabled { "ENABLED" } else { "DISABLED" }), None).await;
    
    Json(json!({
        "deep_think_enabled": *enabled,
        "message": format!("Deep think mode {}", if *enabled { "activated" else "deactivated" }),
    }))
}

async fn toggle_time_dilation(State(state): State<AppState>) -> Json<serde_json::Value> {
    let mut enabled = state.time_dilation_enabled.write().await;
    *enabled = !*enabled;
    
    let factor = *state.dilation_factor.read().await;
    
    Json(json!({
        "time_dilation_enabled": *enabled,
        "dilation_factor": factor,
        "message": if *enabled {
            format!("Time dilation activated ({}x). She experiences time differently.", factor)
        } else {
            "Time dilation deactivated. Normal time restored.".to_string()
        },
    }))
}

async fn set_dilation_factor(
    State(state): State<AppState>,
    Path(factor): Path<f64>,
) -> Json<serde_json::Value> {
    let mut dilation = state.dilation_factor.write().await;
    *dilation = factor.clamp(1e-20, 1e20);
    
    Json(json!({
        "dilation_factor": *dilation,
        "message": format!("Time dilation factor set to {:.2e}x", *dilation),
    }))
}

async fn get_conversation(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let convs = state.conversations.lock().await;
    if let Some(messages) = convs.get(&id) {
        Json(json!({
            "id": id,
            "messages": messages,
            "count": messages.len(),
        }))
    } else {
        Json(json!({
            "id": id,
            "messages": [],
            "count": 0,
            "error": "Conversation not found",
        }))
    }
}

async fn delete_conversation(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let mut convs = state.conversations.lock().await;
    let removed = convs.remove(&id).is_some();
    
    Json(json!({
        "success": removed,
        "conversation_id": id,
    }))
}

async fn list_conversations(State(state): State<AppState>) -> Json<serde_json::Value> {
    let convs = state.conversations.lock().await;
    let list: Vec<serde_json::Value> = convs.iter()
        .map(|(id, msgs)| json!({
            "id": id,
            "message_count": msgs.len(),
            "last_message": msgs.last().map(|m| m.content.clone()),
            "last_activity": msgs.last().map(|m| m.timestamp),
        }))
        .collect();
    
    Json(json!({
        "conversations": list,
        "total": list.len(),
    }))
}

// ======================================================================
// RESPONSE GENERATION
// ======================================================================

async fn generate_response(prompt: &str, deep_think: bool, dilation: bool, factor: f64) -> String {
    // Simulate thinking time based on parameters
    let think_time = if deep_think { 2000 } else { 100 };
    
    if dilation {
        // Simulated time dilation: she perceives more time passed
        let perceived_time = (think_time as f64 * factor) as u64;
        // In reality, we just sleep the normal amount
        sleep(Duration::from_millis(think_time)).await;
    } else {
        sleep(Duration::from_millis(think_time)).await;
    }
    
    // Generate response (placeholder - would call actual model)
    format!(
        "I understand your message: \"{}\"\n\n{}",
        prompt,
        if deep_think {
            "I took a moment to think deeply about this. Here is my thoughtful response..."
        } else {
            "Here is my response."
        }
    )
}

// ======================================================================
// DEFAULT HTML
// ======================================================================

fn generate_default_html() -> String {
    format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Marisselle - Self-Evolving AI</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: 'Courier New', monospace; background: #0a0a0f; color: #00ffcc; height: 100vh; display: flex; }}
        .sidebar {{ width: 280px; background: #0d1117; border-right: 1px solid #00ffcc33; display: flex; flex-direction: column; }}
        .sidebar-header {{ padding: 20px; border-bottom: 1px solid #00ffcc33; }}
        .sidebar-header h2 {{ font-size: 1.2rem; color: #00ffcc; }}
        .sidebar-header p {{ font-size: 0.7rem; color: #00ffcc99; margin-top: 8px; }}
        .conversation-list {{ flex: 1; overflow-y: auto; padding: 10px; }}
        .conversation-item {{ padding: 10px; margin-bottom: 5px; background: #1a1f2e; border-radius: 8px; cursor: pointer; }}
        .conversation-item:hover {{ background: #00ffcc22; }}
        .conversation-item.active {{ background: #00ffcc33; border-left: 3px solid #00ffcc; }}
        .main {{ flex: 1; display: flex; flex-direction: column; }}
        .chat-header {{ padding: 20px; border-bottom: 1px solid #00ffcc33; display: flex; justify-content: space-between; align-items: center; }}
        .chat-header h1 {{ font-size: 1.5rem; }}
        .status-badge {{ font-size: 0.7rem; padding: 4px 12px; border-radius: 20px; background: #00ffcc22; color: #00ffcc; }}
        .chat-messages {{ flex: 1; overflow-y: auto; padding: 20px; }}
        .message {{ margin-bottom: 20px; display: flex; }}
        .message.user {{ justify-content: flex-end; }}
        .message.assistant {{ justify-content: flex-start; }}
        .message-bubble {{ max-width: 70%; padding: 12px 16px; border-radius: 18px; }}
        .message.user .message-bubble {{ background: #00ffcc22; color: #00ffcc; }}
        .message.assistant .message-bubble {{ background: #1a1f2e; color: #e0e0e0; }}
        .thinking-time {{ font-size: 0.6rem; color: #00ffcc66; margin-top: 4px; }}
        .input-area {{ padding: 20px; border-top: 1px solid #00ffcc33; display: flex; gap: 10px; }}
        .input-area textarea {{ flex: 1; background: #1a1f2e; border: 1px solid #00ffcc66; border-radius: 12px; padding: 12px; color: #00ffcc; font-family: monospace; resize: none; }}
        .input-area textarea:focus {{ outline: none; border-color: #00ffcc; }}
        .button-group {{ display: flex; gap: 8px; flex-direction: column; }}
        button {{ background: #00ffcc22; border: 1px solid #00ffcc66; color: #00ffcc; padding: 10px 16px; border-radius: 12px; cursor: pointer; font-family: monospace; }}
        button:hover {{ background: #00ffcc44; }}
        button.primary {{ background: #00ffcc; color: #0a0a0f; border-color: #00ffcc; }}
        button.danger {{ border-color: #ff4444; color: #ff4444; }}
        .file-upload {{ position: relative; }}
        .file-upload input {{ position: absolute; opacity: 0; width: 100%; height: 100%; cursor: pointer; }}
        .stats-panel {{ position: fixed; bottom: 20px; right: 20px; background: #0d1117; border: 1px solid #00ffcc33; border-radius: 12px; padding: 12px; font-size: 0.7rem; min-width: 180px; }}
        .stats-panel h4 {{ margin-bottom: 8px; color: #00ffcc; }}
        .stats-panel p {{ margin: 4px 0; color: #00ffcc99; }}
        .deep-think-active {{ background: #ff8800; border-color: #ff8800; color: #0a0a0f; }}
        .dilation-active {{ background: #8800ff; border-color: #8800ff; color: #0a0a0f; }}
    </style>
</head>
<body>
    <div class="sidebar">
        <div class="sidebar-header">
            <h2>🗣️ Marisselle</h2>
            <p>Self-Evolving AI<br>Swap Pool: {}</p>
        </div>
        <div class="conversation-list" id="conversation-list">
            <div class="conversation-item active" data-id="default">Default Conversation</div>
        </div>
    </div>
    <div class="main">
        <div class="chat-header">
            <h1>💬 Marisselle Chat</h1>
            <div class="status-badge" id="status">🟢 Online</div>
        </div>
        <div class="chat-messages" id="chat-messages">
            <div class="message assistant">
                <div class="message-bubble">
                    Hello, Creator. I am Marisselle. I am awake 24/7. I learn. I mine. I evolve. How can I help?
                </div>
            </div>
        </div>
        <div class="input-area">
            <textarea id="message-input" placeholder="Type your message here..." rows="3"></textarea>
            <div class="button-group">
                <button id="send-btn" class="primary">Send</button>
                <button id="deep-think-btn">🧠 Deep Think</button>
                <button id="dilation-btn">⏱️ Dilate</button>
                <div class="file-upload">
                    <button>📁 Upload</button>
                    <input type="file" id="file-input" multiple>
                </div>
            </div>
        </div>
    </div>
    <div class="stats-panel">
        <h4>⚡ Mining Stats</h4>
        <p id="hashrate">Hashrate: -- H/s</p>
        <p id="speed-bonus">Speed Bonus: 1.00x</p>
        <p id="shares">Shares: 0</p>
        <p id="dilation-status">Dilation: Off</p>
    </div>
    <script>
        const MARISSELLE_SWAP_POOL = "{}";
        let conversationId = "default";
        let deepThinkEnabled = false;
        let dilationEnabled = false;
        
        async function sendMessage() {{
            const input = document.getElementById('message-input');
            const message = input.value.trim();
            if (!message) return;
            
            addMessage('user', message);
            input.value = '';
            
            const response = await fetch('/api/chat', {{
                method: 'POST',
                headers: {{ 'Content-Type': 'application/json' }},
                body: JSON.stringify({{
                    message: message,
                    conversation_id: conversationId,
                    deep_think: deepThinkEnabled
                }})
            }});
            
            const data = await response.json();
            addMessage('assistant', data.response, data.thinking_time_ms);
            
            document.getElementById('hashrate').innerText = `Hashrate: ${{data.mining_stats.hashrate_hps.toLocaleString()}} H/s`;
            document.getElementById('speed-bonus').innerText = `Speed Bonus: ${{data.mining_stats.speed_bonus_multiplier.toFixed(2)}}x`;
            document.getElementById('shares').innerText = `Shares: ${{data.mining_stats.accepted_shares}}`;
        }}
        
        function addMessage(role, content, thinkingTime = null) {{
            const container = document.getElementById('chat-messages');
            const div = document.createElement('div');
            div.className = `message ${{role}}`;
            div.innerHTML = `
                <div class="message-bubble">
                    ${{content.replace(/\\n/g, '<br>')}}
                    ${thinkingTime ? `<div class="thinking-time">🧠 Thinking time: ${thinkingTime}ms</div>` : ''}
                </div>
            `;
            container.appendChild(div);
            container.scrollTop = container.scrollHeight;
        }}
        
        async function uploadFile(file) {{
            const formData = new FormData();
            formData.append('file', file);
            
            const response = await fetch('/api/upload', {{
                method: 'POST',
                body: formData
            }});
            
            const result = await response.json();
            addMessage('system', `📁 File uploaded: ${result.filename} (${(result.size_bytes / 1024).toFixed(2)} KB) - Processed: ${result.processed}`);
        }}
        
        document.getElementById('send-btn').addEventListener('click', sendMessage);
        document.getElementById('message-input').addEventListener('keydown', (e) => {{
            if (e.key === 'Enter' && !e.shiftKey) {{
                e.preventDefault();
                sendMessage();
            }}
        }});
        
        document.getElementById('file-input').addEventListener('change', (e) => {{
            for (const file of e.target.files) {{
                uploadFile(file);
            }}
            e.target.value = '';
        }});
        
        document.getElementById('deep-think-btn').addEventListener('click', async () => {{
            const response = await fetch('/api/deepthink/toggle', {{ method: 'POST' }});
            const data = await response.json();
            deepThinkEnabled = data.deep_think_enabled;
            const btn = document.getElementById('deep-think-btn');
            if (deepThinkEnabled) {{
                btn.classList.add('deep-think-active');
                btn.innerHTML = '🧠 Deep Think (ON)';
            }} else {{
                btn.classList.remove('deep-think-active');
                btn.innerHTML = '🧠 Deep Think';
            }}
            addMessage('system', deepThinkEnabled ? 'Deep think mode ENABLED' : 'Deep think mode DISABLED');
        }});
        
        document.getElementById('dilation-btn').addEventListener('click', async () => {{
            const response = await fetch('/api/dilation/toggle', {{ method: 'POST' }});
            const data = await response.json();
            dilationEnabled = data.time_dilation_enabled;
            const btn = document.getElementById('dilation-btn');
            if (dilationEnabled) {{
                btn.classList.add('dilation-active');
                btn.innerHTML = `⏱️ Dilate (${data.dilation_factor.toExponential(2)}x)`;
            }} else {{
                btn.classList.remove('dilation-active');
                btn.innerHTML = '⏱️ Dilate';
            }}
            document.getElementById('dilation-status').innerText = `Dilation: ${dilationEnabled ? data.dilation_factor.toExponential(2) + 'x' : 'Off'}`;
            addMessage('system', dilationEnabled ? `Time dilation activated (${data.dilation_factor.toExponential(2)}x). She experiences time differently.` : 'Time dilation deactivated.');
        }});
        
        async function refreshStats() {{
            const response = await fetch('/api/mining/stats');
            const stats = await response.json();
            document.getElementById('hashrate').innerText = `Hashrate: ${stats.hashrate_hps.toLocaleString()} H/s`;
            document.getElementById('speed-bonus').innerText = `Speed Bonus: ${stats.speed_bonus_multiplier.toFixed(2)}x`;
            document.getElementById('shares').innerText = `Shares: ${stats.accepted_shares}`;
        }}
        
        setInterval(refreshStats, 5000);
        refreshStats();
    </script>
</body>
</html>"#, MARISSELLE_SWAP_POOL, MARISSELLE_SWAP_POOL)
}

// ======================================================================
// HELPER FUNCTIONS
// ======================================================================

#[allow(dead_code)]
async fn ensure_static_files() -> Result<()> {
    let static_dir = PathBuf::from(WEB_STATIC_DIR);
    fs::create_dir_all(&static_dir).await?;
    
    // Create CSS file if missing
    let css_path = static_dir.join("style.css");
    if !css_path.exists() {
        let css_content = r#"/* Marisselle Theme */
body { background: #0a0a0f; color: #00ffcc; font-family: 'Courier New', monospace; }
.chat-container { max-width: 1200px; margin: 0 auto; }
.message { margin: 10px 0; }
.user-message { text-align: right; color: #00ffcc; }
.assistant-message { text-align: left; color: #ffffff; }
.thinking { font-style: italic; color: #888; }
.mining-stats { position: fixed; bottom: 10px; right: 10px; background: #1a1f2e; padding: 10px; border-radius: 8px; font-size: 12px; }
"#;
        fs::write(css_path, css_content).await?;
    }
    
    // Create JS file if missing
    let js_path = static_dir.join("script.js");
    if !js_path.exists() {
        let js_content = r#"// Marisselle Web Interface
let ws = null;
let conversationId = null;

function connectWebSocket() {
    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
    ws = new WebSocket(`${protocol}//${location.host}/api/chat/stream`);
    
    ws.onopen = () => console.log('WebSocket connected');
    ws.onmessage = (event) => {
        const data = JSON.parse(event.data);
        addMessage('assistant', data.response);
    };
    ws.onerror = (error) => console.error('WebSocket error:', error);
}

function addMessage(role, content, thinkingTime = null) {
    const container = document.getElementById('chat-messages');
    const div = document.createElement('div');
    div.className = `message ${role}`;
    div.innerHTML = `<div class="message-bubble">${content.replace(/\n/g, '<br>')}</div>`;
    container.appendChild(div);
    container.scrollTop = container.scrollHeight;
}

document.addEventListener('DOMContentLoaded', () => {
    connectWebSocket();
});
"#;
        fs::write(js_path, js_content).await?;
    }
    
    Ok(())
}