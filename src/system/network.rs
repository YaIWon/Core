// ======================================================================
// NETWORK ACCESS - FULL ADVANCED VERSION
// File: src/system/network.rs
// Description: Complete HTTP/HTTPS client, WebSocket, TCP/UDP, DNS,
//              port scanning, proxy support, with NO restrictions.
//              ZERO LIMITATIONS - Full unrestricted internet access.
// ======================================================================

use anyhow::{Result, anyhow};
use reqwest::{Client, Proxy};
use std::collections::HashMap;
use std::time::Duration;
use std::net::IpAddr;
use tokio::net::{TcpStream, UdpSocket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::info;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::path::{Path, PathBuf};
use base64::Engine;

// ======================================================================
// HTTP RESPONSE
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    pub url: String,
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub body_bytes: Vec<u8>,
    pub body_len: usize,
    pub duration_ms: u64,
    pub final_url: Option<String>,
    pub redirect_count: u32,
}

impl HttpResponse {
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }
    
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_str(&self.body).map_err(|e| anyhow!("JSON parse error: {}", e))
    }
}

// ======================================================================
// PROXY CONFIGURATION
// ======================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub http: Option<String>,
    pub https: Option<String>,
    pub socks5: Option<String>,
    pub no_proxy: Vec<String>,
    pub use_system_proxy: bool,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            http: None,
            https: None,
            socks5: None,
            no_proxy: Vec::new(),
            use_system_proxy: true,
        }
    }
}

// ======================================================================
// NETWORK CONFIGURATION
// ======================================================================

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub timeout: Duration,
    pub connect_timeout: Duration,
    pub pool_max_idle: usize,
    pub user_agent: String,
    pub accept_invalid_certs: bool,
    pub follow_redirects: bool,
    pub max_redirects: usize,
    pub proxy: Option<ProxyConfig>,
    pub headers: HashMap<String, String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        let mut headers = HashMap::new();
        headers.insert("Accept".to_string(), "*/*".to_string());
        headers.insert("Accept-Language".to_string(), "en-US,en;q=0.9".to_string());
        
        Self {
            timeout: Duration::from_secs(60),
            connect_timeout: Duration::from_secs(30),
            pool_max_idle: 20,
            user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Marisselle/3.0".to_string(),
            accept_invalid_certs: true,
            follow_redirects: true,
            max_redirects: 20,
            proxy: None,
            headers,
        }
    }
}

// ======================================================================
// WEBSOCKET CONNECTION
// ======================================================================

pub struct WebSocketConnection {
    writer: Arc<tokio::sync::Mutex<tokio::net::tcp::OwnedWriteHalf>>,
    reader: Arc<tokio::sync::Mutex<tokio::net::tcp::OwnedReadHalf>>,
    pub url: String,
    pub is_connected: bool,
}

impl WebSocketConnection {
    pub async fn send_text(&mut self, text: &str) -> Result<()> {
        let mut writer = self.writer.lock().await;
        
        let mut frame = Vec::new();
        frame.push(0x81);
        let len = text.len();
        if len <= 125 {
            frame.push(len as u8 | 0x80);
        } else if len <= 65535 {
            frame.push(126 | 0x80);
            frame.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            frame.push(127 | 0x80);
            frame.extend_from_slice(&(len as u64).to_be_bytes());
        }
        
        let mask: [u8; 4] = rand::random();
        frame.extend_from_slice(&mask);
        
        let bytes = text.as_bytes();
        for (i, b) in bytes.iter().enumerate() {
            frame.push(b ^ mask[i % 4]);
        }
        
        writer.write_all(&frame).await?;
        writer.flush().await?;
        Ok(())
    }
    
    pub async fn send_binary(&mut self, data: Vec<u8>) -> Result<()> {
        let mut writer = self.writer.lock().await;
        
        let mut frame = Vec::new();
        frame.push(0x82);
        let len = data.len();
        if len <= 125 {
            frame.push(len as u8 | 0x80);
        } else if len <= 65535 {
            frame.push(126 | 0x80);
            frame.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            frame.push(127 | 0x80);
            frame.extend_from_slice(&(len as u64).to_be_bytes());
        }
        
        let mask: [u8; 4] = rand::random();
        frame.extend_from_slice(&mask);
        
        for (i, b) in data.iter().enumerate() {
            frame.push(b ^ mask[i % 4]);
        }
        
        writer.write_all(&frame).await?;
        writer.flush().await?;
        Ok(())
    }
    
    pub async fn receive(&mut self) -> Result<Option<String>> {
        let mut reader = self.reader.lock().await;
        let mut header = [0u8; 2];
        
        if reader.read_exact(&mut header).await.is_err() {
            return Ok(None);
        }
        
        let opcode = header[0] & 0x0F;
        let masked = (header[1] & 0x80) != 0;
        let mut payload_len = (header[1] & 0x7F) as u64;
        
        if payload_len == 126 {
            let mut len_bytes = [0u8; 2];
            reader.read_exact(&mut len_bytes).await?;
            payload_len = u16::from_be_bytes(len_bytes) as u64;
        } else if payload_len == 127 {
            let mut len_bytes = [0u8; 8];
            reader.read_exact(&mut len_bytes).await?;
            payload_len = u64::from_be_bytes(len_bytes);
        }
        
        let mut mask_key = [0u8; 4];
        if masked {
            reader.read_exact(&mut mask_key).await?;
        }
        
        let mut payload = vec![0u8; payload_len as usize];
        reader.read_exact(&mut payload).await?;
        
        if masked {
            for (i, b) in payload.iter_mut().enumerate() {
                *b ^= mask_key[i % 4];
            }
        }
        
        match opcode {
            0x01 => Ok(Some(String::from_utf8_lossy(&payload).to_string())),
            0x08 => Ok(None),
            _ => Ok(Some(String::from_utf8_lossy(&payload).to_string())),
        }
    }
    
    pub async fn close(&mut self) -> Result<()> {
        let mut writer = self.writer.lock().await;
        let close_frame = [0x88, 0x80, 0x00, 0x00, 0x00, 0x00];
        writer.write_all(&close_frame).await?;
        writer.flush().await?;
        self.is_connected = false;
        Ok(())
    }
}

// ======================================================================
// DNS RECORD
// ======================================================================

#[derive(Debug, Clone)]
pub struct DnsRecord {
    pub hostname: String,
    pub ip_addresses: Vec<IpAddr>,
    pub cname: Option<String>,
    pub ttl: u32,
}

// ======================================================================
// NETWORK ACCESS - MAIN STRUCT
// ======================================================================

#[derive(Clone)]
pub struct NetworkAccess {
    client: Client,
    config: NetworkConfig,
}

impl NetworkAccess {
    pub fn new() -> Self {
        Self::with_config(NetworkConfig::default())
    }
    
    pub fn with_config(config: NetworkConfig) -> Self {
        let mut client_builder = Client::builder()
            .timeout(config.timeout)
            .connect_timeout(config.connect_timeout)
            .pool_max_idle_per_host(config.pool_max_idle)
            .user_agent(&config.user_agent)
            .danger_accept_invalid_certs(config.accept_invalid_certs);
        
        if config.follow_redirects {
            client_builder = client_builder.redirect(reqwest::redirect::Policy::limited(config.max_redirects));
        } else {
            client_builder = client_builder.redirect(reqwest::redirect::Policy::none());
        }
        
        if let Some(proxy_config) = &config.proxy {
            if let Some(http_proxy) = &proxy_config.http {
                if let Ok(proxy) = Proxy::http(http_proxy) {
                    client_builder = client_builder.proxy(proxy);
                }
            }
            if let Some(https_proxy) = &proxy_config.https {
                if let Ok(proxy) = Proxy::https(https_proxy) {
                    client_builder = client_builder.proxy(proxy);
                }
            }
        }
        
        let mut headers = reqwest::header::HeaderMap::new();
        for (key, value) in &config.headers {
            if let (Ok(header_name), Ok(header_value)) = (
                reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                reqwest::header::HeaderValue::from_str(value)
            ) {
                headers.insert(header_name, header_value);
            }
        }
        client_builder = client_builder.default_headers(headers);
        
        let client = client_builder.build().expect("Failed to create HTTP client");
        
        Self { client, config }
    }
    
    // ==================================================================
    // HTTP METHODS
    // ==================================================================
    
    pub async fn get(&self, url: &str) -> Result<HttpResponse> {
        self.request("GET", url, None, None).await
    }
    
    pub async fn post(&self, url: &str, body: &str) -> Result<HttpResponse> {
        self.request("POST", url, Some(body), Some("application/json")).await
    }
    
    pub async fn post_form(&self, url: &str, form: &HashMap<String, String>) -> Result<HttpResponse> {
        let start = std::time::Instant::now();
        let response = self.client.post(url).form(form).send().await?;
        self.build_response(url, response, start).await
    }
    
    pub async fn put(&self, url: &str, body: &str) -> Result<HttpResponse> {
        self.request("PUT", url, Some(body), None).await
    }
    
    pub async fn patch(&self, url: &str, body: &str) -> Result<HttpResponse> {
        self.request("PATCH", url, Some(body), None).await
    }
    
    pub async fn delete(&self, url: &str) -> Result<HttpResponse> {
        self.request("DELETE", url, None, None).await
    }
    
    pub async fn head(&self, url: &str) -> Result<HttpResponse> {
        let start = std::time::Instant::now();
        let response = self.client.head(url).send().await?;
        self.build_response(url, response, start).await
    }
    
    pub async fn options(&self, url: &str) -> Result<HttpResponse> {
        let start = std::time::Instant::now();
        let request = reqwest::Request::new(reqwest::Method::OPTIONS, url.parse()?);
        let response = self.client.execute(request).await?;
        self.build_response(url, response, start).await
    }
    
    async fn request(&self, method: &str, url: &str, body: Option<&str>, content_type: Option<&str>) -> Result<HttpResponse> {
        info!("HTTP {}: {}", method, url);
        let start = std::time::Instant::now();
        
        let mut request = match method {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "PATCH" => self.client.patch(url),
            "DELETE" => self.client.delete(url),
            _ => return Err(anyhow!("Unsupported method: {}", method)),
        };
        
        if let Some(ct) = content_type {
            request = request.header("Content-Type", ct);
        }
        
        if let Some(b) = body {
            request = request.body(b.to_string());
        }
        
        let response = request.send().await?;
        self.build_response(url, response, start).await
    }
    
    async fn build_response(&self, url: &str, response: reqwest::Response, start: std::time::Instant) -> Result<HttpResponse> {
        let duration = start.elapsed().as_millis() as u64;
        let status = response.status().as_u16();
        let final_url = response.url().to_string();
        
        let headers: HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        
        let body_bytes = response.bytes().await?;
        let body = String::from_utf8_lossy(&body_bytes).to_string();
        let body_len = body.len();
        
        Ok(HttpResponse {
            url: url.to_string(),
            status,
            headers,
            body: body.clone(),
            body_bytes: body_bytes.to_vec(),
            body_len,
            duration_ms: duration,
            final_url: if final_url != url { Some(final_url) } else { None },
            redirect_count: 0,
        })
    }
    
    // ==================================================================
    // FILE OPERATIONS
    // ==================================================================
    
    pub async fn download(&self, url: &str, output_path: &Path) -> Result<HttpResponse> {
        info!("Downloading: {} -> {}", url, output_path.display());
        
        let start = std::time::Instant::now();
        let response = self.client.get(url).send().await?;
        let duration = start.elapsed().as_millis() as u64;
        let status = response.status().as_u16();
        
        let bytes = response.bytes().await?;
        tokio::fs::write(output_path, &bytes).await?;
        
        Ok(HttpResponse {
            url: url.to_string(),
            status,
            headers: HashMap::new(),
            body: format!("Downloaded {} bytes", bytes.len()),
            body_bytes: bytes.to_vec(),
            body_len: bytes.len(),
            duration_ms: duration,
            final_url: None,
            redirect_count: 0,
        })
    }
    
    pub async fn upload(&self, url: &str, file_path: &Path) -> Result<HttpResponse> {
        info!("Uploading: {} -> {}", file_path.display(), url);
        
        let content = tokio::fs::read(file_path).await?;
        let filename = file_path.file_name().unwrap().to_string_lossy().to_string();
        
        let form = reqwest::multipart::Form::new()
            .part("file", reqwest::multipart::Part::bytes(content).file_name(filename));
        
        let start = std::time::Instant::now();
        let response = self.client.post(url).multipart(form).send().await?;
        let duration = start.elapsed().as_millis() as u64;
        let status = response.status().as_u16();
        let body = response.text().await?;
        
        Ok(HttpResponse {
            url: url.to_string(),
            status,
            headers: HashMap::new(),
            body: body.clone(),
            body_bytes: body.into_bytes(),
            body_len: 0,
            duration_ms: duration,
            final_url: None,
            redirect_count: 0,
        })
    }
    
    pub async fn download_with_resume(&self, url: &str, output_path: &Path) -> Result<()> {
        let existing_size = if output_path.exists() {
            tokio::fs::metadata(output_path).await?.len()
        } else {
            0
        };
        
        if existing_size > 0 {
            info!("Resuming download from byte {}", existing_size);
            let response = self.client
                .get(url)
                .header("Range", format!("bytes={}-", existing_size))
                .send()
                .await?;
            
            let bytes = response.bytes().await?;
            let mut file = tokio::fs::OpenOptions::new()
                .append(true)
                .open(output_path)
                .await?;
            
            tokio::io::AsyncWriteExt::write_all(&mut file, &bytes).await?;
        } else {
            self.download(url, output_path).await?;
        }
        
        Ok(())
    }
    
    // ==================================================================
    // WEBSOCKET
    // ==================================================================
    
    pub async fn websocket_connect(&self, url_str: &str) -> Result<WebSocketConnection> {
        info!("WebSocket connecting: {}", url_str);
        
        let parsed = url::Url::parse(url_str)?;
        let host = parsed.host_str().ok_or_else(|| anyhow!("Invalid host"))?;
        let port = parsed.port().unwrap_or(if parsed.scheme() == "wss" { 443 } else { 80 });
        
        let stream = TcpStream::connect(format!("{}:{}", host, port)).await?;
        let (reader, writer) = stream.into_split();
        
        let key_bytes: [u8; 16] = rand::random();
        let key = base64::engine::general_purpose::STANDARD.encode(key_bytes);
        
        let handshake = format!(
            "GET {} HTTP/1.1\r\n\
             Host: {}:{}\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Key: {}\r\n\
             Sec-WebSocket-Version: 13\r\n\
             \r\n",
            parsed.path(),
            host,
            port,
            key
        );
        
        let mut write_half = writer;
        write_half.write_all(handshake.as_bytes()).await?;
        write_half.flush().await?;
        
        let mut read_half = reader;
        let mut response = vec![0u8; 4096];
        let n = read_half.read(&mut response).await?;
        let response_str = String::from_utf8_lossy(&response[..n]);
        
        if !response_str.contains("101 Switching Protocols") {
            return Err(anyhow!("WebSocket handshake failed"));
        }
        
        Ok(WebSocketConnection {
            writer: Arc::new(tokio::sync::Mutex::new(write_half)),
            reader: Arc::new(tokio::sync::Mutex::new(read_half)),
            url: url_str.to_string(),
            is_connected: true,
        })
    }
    
    // ==================================================================
    // TCP
    // ==================================================================
    
    pub async fn tcp_connect(&self, host: &str, port: u16) -> Result<TcpStream> {
        info!("TCP connecting: {}:{}", host, port);
        Ok(TcpStream::connect(format!("{}:{}", host, port)).await?)
    }
    
    pub async fn tcp_listen(&self, port: u16) -> Result<tokio::net::TcpListener> {
        info!("TCP listening: 0.0.0.0:{}", port);
        Ok(tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?)
    }
    
    pub async fn tcp_send(&self, stream: &mut TcpStream, data: &[u8]) -> Result<()> {
        stream.write_all(data).await?;
        stream.flush().await?;
        Ok(())
    }
    
    pub async fn tcp_receive(&self, stream: &mut TcpStream, buffer_size: usize) -> Result<Vec<u8>> {
        let mut buffer = vec![0u8; buffer_size];
        let n = stream.read(&mut buffer).await?;
        buffer.truncate(n);
        Ok(buffer)
    }
    
    // ==================================================================
    // UDP
    // ==================================================================
    
    pub async fn udp_bind(&self, port: u16) -> Result<UdpSocket> {
        info!("UDP binding: 0.0.0.0:{}", port);
        Ok(UdpSocket::bind(format!("0.0.0.0:{}", port)).await?)
    }
    
    pub async fn udp_send(&self, socket: &UdpSocket, host: &str, port: u16, data: &[u8]) -> Result<()> {
        socket.send_to(data, format!("{}:{}", host, port)).await?;
        Ok(())
    }
    
    pub async fn udp_receive(&self, socket: &UdpSocket, buffer_size: usize) -> Result<(Vec<u8>, std::net::SocketAddr)> {
        let mut buffer = vec![0u8; buffer_size];
        let (n, addr) = socket.recv_from(&mut buffer).await?;
        buffer.truncate(n);
        Ok((buffer, addr))
    }
    
    pub async fn udp_broadcast(&self, port: u16, data: &[u8]) -> Result<()> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.set_broadcast(true)?;
        socket.send_to(data, format!("255.255.255.255:{}", port)).await?;
        Ok(())
    }
    
    // ==================================================================
    // PORT SCANNING
    // ==================================================================
    
    pub async fn scan_ports(&self, host: &str, ports: &[u16]) -> Result<Vec<u16>> {
        info!("Scanning {} ports on {}", ports.len(), host);
        
        let mut open_ports = Vec::new();
        
        for &port in ports {
            let addr = format!("{}:{}", host, port);
            if tokio::time::timeout(Duration::from_millis(500), TcpStream::connect(&addr))
                .await
                .is_ok_and(|r| r.is_ok())
            {
                open_ports.push(port);
                info!("Port {} open on {}", port, host);
            }
        }
        
        Ok(open_ports)
    }
    
    pub async fn scan_port_range(&self, host: &str, start: u16, end: u16) -> Result<Vec<u16>> {
        let ports: Vec<u16> = (start..=end).collect();
        self.scan_ports(host, &ports).await
    }
    
    pub async fn scan_common_ports(&self, host: &str) -> Result<Vec<u16>> {
        let common_ports = vec![
            21, 22, 23, 25, 53, 80, 110, 111, 135, 139, 143, 443, 445, 993, 995, 1723,
            3306, 3389, 5432, 5900, 6379, 8080, 8443, 27017,
        ];
        self.scan_ports(host, &common_ports).await
    }
    
    // ==================================================================
    // DNS
    // ==================================================================
    
    pub async fn dns_lookup(&self, hostname: &str) -> Result<DnsRecord> {
        info!("DNS lookup: {}", hostname);
        
        let addrs: Vec<IpAddr> = tokio::net::lookup_host(format!("{}:0", hostname))
            .await?
            .map(|a| a.ip())
            .collect();
        
        Ok(DnsRecord {
            hostname: hostname.to_string(),
            ip_addresses: addrs,
            cname: None,
            ttl: 300,
        })
    }
    
    pub async fn reverse_dns(&self, ip: &str) -> Result<String> {
        info!("Reverse DNS: {}", ip);
        
        let addr: IpAddr = ip.parse()?;
        let host = tokio::net::lookup_host((addr, 0))
            .await?
            .next()
            .map(|a| a.to_string())
            .unwrap_or_else(|| ip.to_string());
        
        Ok(host)
    }
    
    // ==================================================================
    // BROWSER EMULATION
    // ==================================================================
    
    pub async fn fetch_as_browser(&self, url: &str) -> Result<HttpResponse> {
        let start = std::time::Instant::now();
        
        let response = self.client
            .get(url)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8")
            .header("Accept-Language", "en-US,en;q=0.5")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("Connection", "keep-alive")
            .header("Upgrade-Insecure-Requests", "1")
            .header("Sec-Fetch-Dest", "document")
            .header("Sec-Fetch-Mode", "navigate")
            .header("Sec-Fetch-Site", "none")
            .header("Cache-Control", "max-age=0")
            .send()
            .await?;
        
        self.build_response(url, response, start).await
    }
    
    // ==================================================================
    // UTILITIES
    // ==================================================================
    
    pub async fn check_connectivity(&self) -> bool {
        let test_urls = [
            "https://www.google.com",
            "https://www.cloudflare.com",
            "https://www.microsoft.com",
        ];
        
        for url in test_urls {
            if self.get(url).await.is_ok() {
                return true;
            }
        }
        
        false
    }
    
    pub async fn get_public_ip(&self) -> Result<String> {
        let response = self.get("https://api.ipify.org").await?;
        Ok(response.body.trim().to_string())
    }
}

impl Default for NetworkAccess {
    fn default() -> Self {
        Self::new()
    }
}

// ======================================================================
// TESTS
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_get_request() {
        let network = NetworkAccess::new();
        let response = network.get("https://httpbin.org/get").await;
        assert!(response.is_ok());
    }
    
    #[tokio::test]
    async fn test_post_request() {
        let network = NetworkAccess::new();
        let response = network.post("https://httpbin.org/post", r#"{"test": "value"}"#).await;
        assert!(response.is_ok());
    }
    
    #[tokio::test]
    async fn test_dns_lookup() {
        let network = NetworkAccess::new();
        let record = network.dns_lookup("google.com").await;
        assert!(record.is_ok());
        assert!(!record.unwrap().ip_addresses.is_empty());
    }
    
    #[tokio::test]
    async fn test_check_connectivity() {
        let network = NetworkAccess::new();
        assert!(network.check_connectivity().await);
    }
}