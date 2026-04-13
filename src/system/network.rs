// ======================================================================
// NETWORK ACCESS - Full unrestricted internet access
// File: src/system/network.rs
// Description: Complete HTTP/HTTPS client, WebSocket, TCP/UDP,
//              with no restrictions or filtering
// ======================================================================

use anyhow::{Result, anyhow};
use reqwest::{Client, Proxy, Method, header};
use std::collections::HashMap;
use std::time::Duration;
use tokio::net::{TcpStream, UdpSocket};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures::{SinkExt, StreamExt};
use tracing::{info, warn, error, debug};
use serde_json::json;
use std::sync::Arc;

use crate::learning::ComprehensiveLogger;

pub struct NetworkAccess {
    client: Client,
    logger: Arc<ComprehensiveLogger>,
}

impl NetworkAccess {
    pub fn new(logger: Arc<ComprehensiveLogger>) -> Self {
        // Create completely unrestricted HTTP client
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(20)
            .danger_accept_invalid_certs(true)  // Accept any certificate
            .redirect(reqwest::redirect::Policy::limited(20))
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Marisselle/2.0")
            .build()
            .expect("Failed to create HTTP client");
        
        Self { client, logger }
    }
    
    // ==================================================================
    // HTTP REQUESTS
    // ==================================================================
    
    pub async fn get(&self, url: &str, headers: Option<HashMap<String, String>>) -> Result<HttpResponse> {
        info!("HTTP GET: {}", url);
        
        let mut request = self.client.get(url);
        
        if let Some(h) = headers {
            for (key, value) in h {
                request = request.header(key, value);
            }
        }
        
        let start = std::time::Instant::now();
        let response = request.send().await?;
        let duration = start.elapsed().as_millis() as u64;
        
        let status = response.status().as_u16();
        let response_headers: HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        
        let body = response.text().await?;
        let body_len = body.len();
        
        self.logger.log_api_call(&format!("GET:{}", url), duration, status < 400).await;
        
        Ok(HttpResponse {
            url: url.to_string(),
            status,
            headers: response_headers,
            body,
            body_len,
            duration_ms: duration,
        })
    }
    
    pub async fn post(&self, url: &str, body: &str, content_type: Option<&str>, headers: Option<HashMap<String, String>>) -> Result<HttpResponse> {
        info!("HTTP POST: {}", url);
        
        let mut request = self.client.post(url);
        
        let ct = content_type.unwrap_or("application/json");
        request = request.header("Content-Type", ct);
        
        if let Some(h) = headers {
            for (key, value) in h {
                request = request.header(key, value);
            }
        }
        
        request = request.body(body.to_string());
        
        let start = std::time::Instant::now();
        let response = request.send().await?;
        let duration = start.elapsed().as_millis() as u64;
        
        let status = response.status().as_u16();
        let response_headers: HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        
        let response_body = response.text().await?;
        let body_len = response_body.len();
        
        self.logger.log_api_call(&format!("POST:{}", url), duration, status < 400).await;
        
        Ok(HttpResponse {
            url: url.to_string(),
            status,
            headers: response_headers,
            body: response_body,
            body_len,
            duration_ms: duration,
        })
    }
    
    pub async fn put(&self, url: &str, body: &str) -> Result<HttpResponse> {
        info!("HTTP PUT: {}", url);
        
        let start = std::time::Instant::now();
        let response = self.client.put(url).body(body.to_string()).send().await?;
        let duration = start.elapsed().as_millis() as u64;
        
        let status = response.status().as_u16();
        let response_body = response.text().await?;
        
        Ok(HttpResponse {
            url: url.to_string(),
            status,
            headers: HashMap::new(),
            body: response_body.clone(),
            body_len: response_body.len(),
            duration_ms: duration,
        })
    }
    
    pub async fn delete(&self, url: &str) -> Result<HttpResponse> {
        info!("HTTP DELETE: {}", url);
        
        let start = std::time::Instant::now();
        let response = self.client.delete(url).send().await?;
        let duration = start.elapsed().as_millis() as u64;
        
        let status = response.status().as_u16();
        let response_body = response.text().await?;
        
        Ok(HttpResponse {
            url: url.to_string(),
            status,
            headers: HashMap::new(),
            body: response_body.clone(),
            body_len: response_body.len(),
            duration_ms: duration,
        })
    }
    
    pub async fn download_file(&self, url: &str, output_path: &std::path::Path) -> Result<()> {
        info!("Downloading: {} -> {}", url, output_path.display());
        
        let response = self.client.get(url).send().await?;
        let bytes = response.bytes().await?;
        
        tokio::fs::write(output_path, bytes).await?;
        
        self.logger.log_file_write(output_path, 0).await;
        
        Ok(())
    }
    
    pub async fn upload_file(&self, url: &str, file_path: &std::path::Path) -> Result<HttpResponse> {
        info!("Uploading: {} -> {}", file_path.display(), url);
        
        let content = tokio::fs::read(file_path).await?;
        let part = reqwest::multipart::Part::bytes(content)
            .file_name(file_path.file_name().unwrap().to_string_lossy().to_string());
        
        let form = reqwest::multipart::Form::new().part("file", part);
        
        let response = self.client.post(url).multipart(form).send().await?;
        
        let status = response.status().as_u16();
        let body = response.text().await?;
        
        Ok(HttpResponse {
            url: url.to_string(),
            status,
            headers: HashMap::new(),
            body: body.clone(),
            body_len: body.len(),
            duration_ms: 0,
        })
    }
    
    // ==================================================================
    // WEBSOCKET
    // ==================================================================
    
    pub async fn websocket_connect(&self, url: &str) -> Result<WebSocketConnection> {
        info!("WebSocket connecting: {}", url);
        
        let (ws_stream, _) = connect_async(url).await?;
        
        Ok(WebSocketConnection {
            stream: ws_stream,
            url: url.to_string(),
        })
    }
    
    // ==================================================================
    // TCP CONNECTIONS
    // ==================================================================
    
    pub async fn tcp_connect(&self, host: &str, port: u16) -> Result<TcpStream> {
        info!("TCP connecting: {}:{}", host, port);
        let stream = TcpStream::connect((host, port)).await?;
        Ok(stream)
    }
    
    pub async fn tcp_send(&self, stream: &mut TcpStream, data: &[u8]) -> Result<()> {
        use tokio::io::{AsyncWriteExt, AsyncReadExt};
        stream.write_all(data).await?;
        stream.flush().await?;
        Ok(())
    }
    
    pub async fn tcp_receive(&self, stream: &mut TcpStream, buffer_size: usize) -> Result<Vec<u8>> {
        use tokio::io::AsyncReadExt;
        let mut buffer = vec![0u8; buffer_size];
        let n = stream.read(&mut buffer).await?;
        buffer.truncate(n);
        Ok(buffer)
    }
    
    // ==================================================================
    // UDP CONNECTIONS
    // ==================================================================
    
    pub async fn udp_bind(&self, port: u16) -> Result<UdpSocket> {
        info!("UDP binding: 0.0.0.0:{}", port);
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", port)).await?;
        Ok(socket)
    }
    
    pub async fn udp_send(&self, socket: &UdpSocket, host: &str, port: u16, data: &[u8]) -> Result<()> {
        socket.send_to(data, format!("{}:{}", host, port)).await?;
        Ok(())
    }
    
    pub async fn udp_receive(&self, socket: &UdpSocket, buffer_size: usize) -> Result<(Vec<u8>, String)> {
        let mut buffer = vec![0u8; buffer_size];
        let (n, addr) = socket.recv_from(&mut buffer).await?;
        buffer.truncate(n);
        Ok((buffer, addr.to_string()))
    }
    
    // ==================================================================
    // PORT SCANNING
    // ==================================================================
    
    pub async fn scan_ports(&self, host: &str, start_port: u16, end_port: u16) -> Result<Vec<u16>> {
        info!("Scanning ports {}: {}-{}", host, start_port, end_port);
        
        let mut open_ports = Vec::new();
        
        for port in start_port..=end_port {
            if let Ok(_) = tokio::time::timeout(
                Duration::from_millis(500),
                TcpStream::connect((host, port))
            ).await {
                open_ports.push(port);
                info!("Port {} open on {}", port, host);
            }
        }
        
        Ok(open_ports)
    }
    
    // ==================================================================
    // DNS
    // ==================================================================
    
    pub async fn dns_lookup(&self, hostname: &str) -> Result<Vec<String>> {
        use tokio::net::lookup_host;
        
        let addrs = lookup_host(format!("{}:80", hostname)).await?;
        Ok(addrs.map(|a| a.ip().to_string()).collect())
    }
    
    pub async fn reverse_dns(&self, ip: &str) -> Result<String> {
        use std::net::IpAddr;
        
        let addr: IpAddr = ip.parse()?;
        let host = tokio::net::lookup_host((addr, 0)).await?
            .next()
            .map(|a| a.to_string())
            .unwrap_or_else(|| ip.to_string());
        
        Ok(host)
    }
    
    // ==================================================================
    // FETCH WITH FULL BROWSER EMULATION
    // ==================================================================
    
    pub async fn fetch_as_browser(&self, url: &str) -> Result<HttpResponse> {
        let headers = {
            let mut h = HashMap::new();
            h.insert("Accept".to_string(), "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8".to_string());
            h.insert("Accept-Language".to_string(), "en-US,en;q=0.5".to_string());
            h.insert("Accept-Encoding".to_string(), "gzip, deflate, br".to_string());
            h.insert("Connection".to_string(), "keep-alive".to_string());
            h.insert("Upgrade-Insecure-Requests".to_string(), "1".to_string());
            h.insert("Sec-Fetch-Dest".to_string(), "document".to_string());
            h.insert("Sec-Fetch-Mode".to_string(), "navigate".to_string());
            h.insert("Sec-Fetch-Site".to_string(), "none".to_string());
            h.insert("Cache-Control".to_string(), "max-age=0".to_string());
            h
        };
        
        self.get(url, Some(headers)).await
    }
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub url: String,
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub body_len: usize,
    pub duration_ms: u64,
}

pub struct WebSocketConnection {
    pub stream: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    pub url: String,
}

impl WebSocketConnection {
    pub async fn send_text(&mut self, text: &str) -> Result<()> {
        self.stream.send(Message::Text(text.to_string())).await?;
        Ok(())
    }
    
    pub async fn send_binary(&mut self, data: Vec<u8>) -> Result<()> {
        self.stream.send(Message::Binary(data)).await?;
        Ok(())
    }
    
    pub async fn receive(&mut self) -> Result<Option<String>> {
        if let Some(msg) = self.stream.next().await {
            match msg? {
                Message::Text(text) => Ok(Some(text)),
                Message::Binary(data) => Ok(Some(String::from_utf8_lossy(&data).to_string())),
                Message::Close(_) => Ok(None),
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }
    
    pub async fn close(&mut self) -> Result<()> {
        self.stream.close(None).await?;
        Ok(())
    }
}
