//! HTTP network signaling service for cross-machine peer discovery and handshake
//!
//! Provides lightweight asynchronous REST endpoints on port 5000:
//! - GET /api/info: returns device metadata, capabilities, role, and display specs
//! - POST /api/request-connection: triggers approval modal on receiver and returns PIN
//! - POST /api/verify-pin: verifies PIN challenge with receiver's PinAuthManager

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{info, warn};

use crate::discovery::PeerDevice;
use crate::AppState;

/// Device metadata returned by GET /api/info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalingInfo {
    pub device_id: String,
    pub hostname: String,
    pub os: String,
    pub role: String,
    pub capabilities: Vec<String>,
    pub max_resolution: String,
    pub max_fps: u32,
}

/// Request payload for POST /api/request-connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestConnectionPayload {
    pub peer_id: String,
    pub peer_name: String,
}

/// Response returned by POST /api/request-connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestConnectionResponse {
    pub pin: String,
    pub status: String,
}

/// Request payload for POST /api/verify-pin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyPinPayload {
    pub peer_id: String,
    pub pin: String,
}

/// Response returned by POST /api/verify-pin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyPinResponse {
    pub success: bool,
}

/// Query response for GET /api/connection-status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatusResponse {
    pub status: String,
    pub pin: Option<String>,
}

/// Request payload for POST /api/start-receiver-stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartReceiverStreamPayload {
    pub peer_id: String,
    pub host_ip: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

/// Response returned by POST /api/start-receiver-stream and /api/stop-receiver-stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamControlResponse {
    pub success: bool,
    pub message: Option<String>,
}

/// Determine the preferred local non-loopback IPv4 address
pub fn get_local_ip() -> Option<String> {
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(local) = socket.local_addr() {
                let ip = local.ip();
                if !ip.is_loopback() && !ip.is_unspecified() {
                    return Some(ip.to_string());
                }
            }
        }
    }
    None
}

/// Run asynchronous HTTP signaling server on the specified TCP port
pub async fn run_signaling_server(state: AppState, port: u16) -> Result<(), String> {
    let bind_addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(bind_addr)
        .await
        .map_err(|e| format!("Failed to bind signaling listener on port {}: {}", port, e))?;

    info!("HTTP signaling listener active and listening on port {}", port);

    loop {
        match listener.accept().await {
            Ok((stream, remote_addr)) => {
                let state_clone = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_http_connection(stream, state_clone, remote_addr).await {
                        warn!("HTTP connection handler error from {}: {}", remote_addr, e);
                    }
                });
            }
            Err(e) => {
                warn!("HTTP signaling accept error: {}", e);
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
}

/// Handle a single incoming HTTP TCP connection
async fn handle_http_connection(
    mut stream: TcpStream,
    state: AppState,
    _remote_addr: SocketAddr,
) -> Result<(), String> {
    let mut buffer = Vec::with_capacity(4096);
    let mut temp = [0u8; 1024];

    // Read until end of HTTP headers
    let header_end_pos;
    loop {
        let n = stream
            .read(&mut temp)
            .await
            .map_err(|e| format!("Read error: {}", e))?;
        if n == 0 {
            return Ok(());
        }
        buffer.extend_from_slice(&temp[..n]);

        if let Some(pos) = find_header_end(&buffer) {
            header_end_pos = pos;
            break;
        }

        if buffer.len() > 65536 {
            return Err("HTTP header limit exceeded".to_string());
        }
    }

    let header_bytes = &buffer[..header_end_pos];
    let header_str = String::from_utf8_lossy(header_bytes);
    let mut lines = header_str.lines();

    let request_line = lines.next().ok_or_else(|| "Empty request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_uppercase();
    let raw_path = parts.next().unwrap_or("");
    let path = raw_path.split('?').next().unwrap_or(raw_path);

    let mut content_length: usize = 0;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                if let Ok(len) = v.trim().parse::<usize>() {
                    content_length = len;
                }
            }
        }
    }

    // Read remaining body bytes if content_length > available
    let body_start = header_end_pos;
    let mut body = buffer[body_start..].to_vec();
    while body.len() < content_length {
        let needed = content_length - body.len();
        let chunk_size = needed.min(temp.len());
        let n = stream
            .read(&mut temp[..chunk_size])
            .await
            .map_err(|e| format!("Body read error: {}", e))?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&temp[..n]);
    }

    // CORS preflight
    if method == "OPTIONS" {
        return write_http_response(&mut stream, 204, "No Content", "text/plain", b"").await;
    }

    match (method.as_str(), path) {
        ("GET", "/api/info") => {
            let role = state
                .app_role
                .lock()
                .map(|r| r.clone())
                .unwrap_or_else(|_| "host".to_string());

            let (caps, max_res, max_fps) = state
                .discovery_service
                .get_advertised_caps()
                .unwrap_or_else(|| {
                    (
                        vec!["1080p".to_string(), "1440p".to_string()],
                        "1920x1080".to_string(),
                        60,
                    )
                });

            let info = SignalingInfo {
                device_id: state.device_id.clone(),
                hostname: state.hostname.clone(),
                os: state.get_operating_system(),
                role,
                capabilities: caps,
                max_resolution: max_res,
                max_fps,
            };

            let json_body = serde_json::to_string(&info)
                .map_err(|e| format!("Serialization error: {}", e))?;
            write_http_response(&mut stream, 200, "OK", "application/json", json_body.as_bytes()).await
        }

        ("GET", "/api/connection-status") => {
            let peer_id = raw_path
                .split_once('?')
                .and_then(|(_, query)| {
                    query.split('&').find_map(|pair| {
                        let (k, v) = pair.split_once('=')?;
                        if k == "peer_id" {
                            Some(v.to_string())
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or_default();

            let status_resp = if let Ok(auth) = state.pin_auth_manager.lock() {
                if let Some(session) = auth.get_session(&peer_id) {
                    let status_str = match session.state {
                        crate::pairing::PairingState::PendingApproval => "pending_approval",
                        crate::pairing::PairingState::PendingPin | crate::pairing::PairingState::Pending => "approved",
                        crate::pairing::PairingState::Authenticated => "authenticated",
                        crate::pairing::PairingState::Rejected => "rejected",
                        crate::pairing::PairingState::Expired => "expired",
                        crate::pairing::PairingState::LockedOut => "locked_out",
                    };
                    ConnectionStatusResponse {
                        status: status_str.to_string(),
                        pin: Some(session.pin.clone()),
                    }
                } else {
                    ConnectionStatusResponse {
                        status: "not_found".to_string(),
                        pin: None,
                    }
                }
            } else {
                ConnectionStatusResponse {
                    status: "error".to_string(),
                    pin: None,
                }
            };

            let json_body = serde_json::to_string(&status_resp)
                .map_err(|e| format!("Serialization error: {}", e))?;
            write_http_response(&mut stream, 200, "OK", "application/json", json_body.as_bytes()).await
        }

        ("POST", "/api/request-connection") => {
            let payload: RequestConnectionPayload = serde_json::from_slice(&body)
                .map_err(|e| format!("Invalid JSON payload for request-connection: {}", e))?;

            let is_receiver_role = state
                .app_role
                .lock()
                .map(|r| r.as_str() == "receiver")
                .unwrap_or(false);

            let pin = state.request_connection(&payload.peer_id, &payload.peer_name)?;

            // Auto-approve connection if this node is explicitly set to Receiver role
            let status = if is_receiver_role {
                if let Ok(mut auth) = state.pin_auth_manager.lock() {
                    let _ = auth.respond_connection(&payload.peer_id, true);
                }
                info!(
                    "Auto-approved incoming connection request from {} because device is in receiver role",
                    payload.peer_id
                );
                "approved".to_string()
            } else {
                "pending_approval".to_string()
            };

            let resp = RequestConnectionResponse { pin, status };

            let json_body = serde_json::to_string(&resp)
                .map_err(|e| format!("Serialization error: {}", e))?;
            write_http_response(&mut stream, 200, "OK", "application/json", json_body.as_bytes()).await
        }

        ("POST", "/api/verify-pin") => {
            let payload: VerifyPinPayload = serde_json::from_slice(&body)
                .map_err(|e| format!("Invalid JSON payload for verify-pin: {}", e))?;

            let verified = {
                let mut auth = state.pin_auth_manager.lock().map_err(|e| e.to_string())?;
                match auth.verify_pin(&payload.peer_id, &payload.pin) {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("Signaling verify_pin rejected for peer {}: {}", payload.peer_id, e);
                        false
                    }
                }
            };

            let resp = VerifyPinResponse { success: verified };
            let json_body = serde_json::to_string(&resp)
                .map_err(|e| format!("Serialization error: {}", e))?;
            write_http_response(&mut stream, 200, "OK", "application/json", json_body.as_bytes()).await
        }

        ("POST", "/api/start-receiver-stream") => {
            let payload: StartReceiverStreamPayload = serde_json::from_slice(&body)
                .map_err(|e| format!("Invalid JSON payload for start-receiver-stream: {}", e))?;

            let width = if payload.width % 2 != 0 { payload.width - 1 } else { payload.width };
            let height = if payload.height % 2 != 0 { payload.height - 1 } else { payload.height };
            let geom = crate::receiver::ClientScreenGeometry::new(width, height, payload.fps)
                .map_err(|e| e.to_string())?;

            match state.start_windows_receiver(&geom) {
                Ok(success) => {
                    info!("Receiver stream presentation surface started: {}", success);
                    let resp = StreamControlResponse {
                        success,
                        message: None,
                    };
                    let json_body = serde_json::to_string(&resp)
                        .map_err(|e| format!("Serialization error: {}", e))?;
                    write_http_response(&mut stream, 200, "OK", "application/json", json_body.as_bytes()).await
                }
                Err(e) => {
                    warn!("Failed to start receiver stream presentation: {}", e);
                    let resp = StreamControlResponse {
                        success: false,
                        message: Some(e),
                    };
                    let json_body = serde_json::to_string(&resp)
                        .map_err(|e| format!("Serialization error: {}", e))?;
                    write_http_response(&mut stream, 500, "Internal Server Error", "application/json", json_body.as_bytes()).await
                }
            }
        }

        ("POST", "/api/stop-receiver-stream") => {
            let _ = state.stop_active_session().await;
            let resp = StreamControlResponse {
                success: true,
                message: None,
            };
            let json_body = serde_json::to_string(&resp)
                .map_err(|e| format!("Serialization error: {}", e))?;
            write_http_response(&mut stream, 200, "OK", "application/json", json_body.as_bytes()).await
        }

        _ => {
            let not_found = serde_json::json!({
                "error": "Not Found",
                "path": path,
                "method": method
            });
            let json_body = not_found.to_string();
            write_http_response(&mut stream, 404, "Not Found", "application/json", json_body.as_bytes()).await
        }
    }
}

/// Find boundary `\r\n\r\n` or `\n\n` in received byte buffer
fn find_header_end(buf: &[u8]) -> Option<usize> {
    for i in 0..buf.len() {
        if i + 4 <= buf.len() && &buf[i..i + 4] == b"\r\n\r\n" {
            return Some(i + 4);
        }
        if i + 2 <= buf.len() && &buf[i..i + 2] == b"\n\n" {
            return Some(i + 2);
        }
    }
    None
}

/// Format and write HTTP response with CORS headers
async fn write_http_response(
    stream: &mut TcpStream,
    status_code: u16,
    status_text: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), String> {
    let header = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
         Access-Control-Allow-Headers: Content-Type\r\n\
         Connection: close\r\n\
         \r\n",
        status_code,
        status_text,
        content_type,
        body.len()
    );

    stream
        .write_all(header.as_bytes())
        .await
        .map_err(|e| format!("Failed to write HTTP header: {}", e))?;
    stream
        .write_all(body)
        .await
        .map_err(|e| format!("Failed to write HTTP body: {}", e))?;
    stream
        .flush()
        .await
        .map_err(|e| format!("Failed to flush HTTP stream: {}", e))?;
    Ok(())
}

/// Extract body string from raw HTTP response byte stream
fn extract_http_body(buf: &[u8]) -> Result<String, String> {
    let text = String::from_utf8_lossy(buf);
    if let Some((_header, body)) = text.split_once("\r\n\r\n") {
        Ok(body.trim().to_string())
    } else if let Some((_header, body)) = text.split_once("\n\n") {
        Ok(body.trim().to_string())
    } else {
        Err("Malformed HTTP response: no header delimiter found".to_string())
    }
}

/// Send GET /api/info to remote peer IP and port
pub async fn send_get_info(ip: &str, port: u16) -> Result<SignalingInfo, String> {
    let addr = format!("{}:{}", ip, port);
    let mut stream = tokio::time::timeout(Duration::from_millis(3500), TcpStream::connect(&addr))
        .await
        .map_err(|_| format!("Connection timeout connecting to http://{}/api/info", addr))?
        .map_err(|e| format!("Failed to connect to http://{}/api/info: {}", addr, e))?;

    let req = format!(
        "GET /api/info HTTP/1.1\r\n\
         Host: {}\r\n\
         User-Agent: WirelessMonitor/0.1\r\n\
         Accept: application/json\r\n\
         Connection: close\r\n\
         \r\n",
        addr
    );

    stream
        .write_all(req.as_bytes())
        .await
        .map_err(|e| format!("Failed to write request: {}", e))?;
    stream
        .flush()
        .await
        .map_err(|e| format!("Failed to flush request: {}", e))?;

    let mut response_buf = Vec::new();
    tokio::time::timeout(Duration::from_millis(3500), stream.read_to_end(&mut response_buf))
        .await
        .map_err(|_| format!("Read timeout from http://{}/api/info", addr))?
        .map_err(|e| format!("Read error from http://{}/api/info: {}", addr, e))?;

    let body_str = extract_http_body(&response_buf)?;
    serde_json::from_str::<SignalingInfo>(&body_str)
        .map_err(|e| format!("Invalid JSON response from {}: {}. Raw: {}", addr, e, body_str))
}

/// Send POST /api/request-connection to remote peer IP and port
pub async fn send_request_connection(
    ip: &str,
    port: u16,
    payload: &RequestConnectionPayload,
) -> Result<RequestConnectionResponse, String> {
    let addr = format!("{}:{}", ip, port);
    let body_json = serde_json::to_string(payload).map_err(|e| e.to_string())?;

    let mut stream = tokio::time::timeout(Duration::from_millis(4000), TcpStream::connect(&addr))
        .await
        .map_err(|_| format!("Connection timeout connecting to http://{}/api/request-connection", addr))?
        .map_err(|e| format!("Failed to connect to http://{}/api/request-connection: {}", addr, e))?;

    let req = format!(
        "POST /api/request-connection HTTP/1.1\r\n\
         Host: {}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         User-Agent: WirelessMonitor/0.1\r\n\
         Accept: application/json\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        addr,
        body_json.len(),
        body_json
    );

    stream
        .write_all(req.as_bytes())
        .await
        .map_err(|e| format!("Failed to write request: {}", e))?;
    stream
        .flush()
        .await
        .map_err(|e| format!("Failed to flush request: {}", e))?;

    let mut response_buf = Vec::new();
    tokio::time::timeout(Duration::from_millis(4000), stream.read_to_end(&mut response_buf))
        .await
        .map_err(|_| format!("Read timeout from http://{}/api/request-connection", addr))?
        .map_err(|e| format!("Read error from http://{}/api/request-connection: {}", addr, e))?;

    let body_str = extract_http_body(&response_buf)?;
    serde_json::from_str::<RequestConnectionResponse>(&body_str)
        .map_err(|e| format!("Invalid JSON response from {}: {}. Raw: {}", addr, e, body_str))
}

/// Send POST /api/verify-pin to remote peer IP and port
pub async fn send_verify_pin(
    ip: &str,
    port: u16,
    payload: &VerifyPinPayload,
) -> Result<VerifyPinResponse, String> {
    let addr = format!("{}:{}", ip, port);
    let body_json = serde_json::to_string(payload).map_err(|e| e.to_string())?;

    let mut stream = tokio::time::timeout(Duration::from_millis(4000), TcpStream::connect(&addr))
        .await
        .map_err(|_| format!("Connection timeout connecting to http://{}/api/verify-pin", addr))?
        .map_err(|e| format!("Failed to connect to http://{}/api/verify-pin: {}", addr, e))?;

    let req = format!(
        "POST /api/verify-pin HTTP/1.1\r\n\
         Host: {}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         User-Agent: WirelessMonitor/0.1\r\n\
         Accept: application/json\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        addr,
        body_json.len(),
        body_json
    );

    stream
        .write_all(req.as_bytes())
        .await
        .map_err(|e| format!("Failed to write request: {}", e))?;
    stream
        .flush()
        .await
        .map_err(|e| format!("Failed to flush request: {}", e))?;

    let mut response_buf = Vec::new();
    tokio::time::timeout(Duration::from_millis(4000), stream.read_to_end(&mut response_buf))
        .await
        .map_err(|_| format!("Read timeout from http://{}/api/verify-pin", addr))?
        .map_err(|e| format!("Read error from http://{}/api/verify-pin: {}", addr, e))?;

    let body_str = extract_http_body(&response_buf)?;
    serde_json::from_str::<VerifyPinResponse>(&body_str)
        .map_err(|e| format!("Invalid JSON response from {}: {}. Raw: {}", addr, e, body_str))
}

/// Send GET /api/connection-status to remote peer IP and port
pub async fn send_get_connection_status(
    ip: &str,
    port: u16,
    peer_id: &str,
) -> Result<ConnectionStatusResponse, String> {
    let addr = format!("{}:{}", ip, port);
    let mut stream = tokio::time::timeout(Duration::from_millis(3500), TcpStream::connect(&addr))
        .await
        .map_err(|_| format!("Connection timeout connecting to http://{}/api/connection-status", addr))?
        .map_err(|e| format!("Failed to connect to http://{}/api/connection-status: {}", addr, e))?;

    let req = format!(
        "GET /api/connection-status?peer_id={} HTTP/1.1\r\n\
         Host: {}\r\n\
         User-Agent: WirelessMonitor/0.1\r\n\
         Accept: application/json\r\n\
         Connection: close\r\n\
         \r\n",
        peer_id, addr
    );

    stream
        .write_all(req.as_bytes())
        .await
        .map_err(|e| format!("Failed to write request: {}", e))?;
    stream
        .flush()
        .await
        .map_err(|e| format!("Failed to flush request: {}", e))?;

    let mut response_buf = Vec::new();
    tokio::time::timeout(Duration::from_millis(3500), stream.read_to_end(&mut response_buf))
        .await
        .map_err(|_| format!("Read timeout from http://{}/api/connection-status", addr))?
        .map_err(|e| format!("Read error from http://{}/api/connection-status: {}", addr, e))?;

    let body_str = extract_http_body(&response_buf)?;
    serde_json::from_str::<ConnectionStatusResponse>(&body_str)
        .map_err(|e| format!("Invalid JSON response from {}: {}. Raw: {}", addr, e, body_str))
}

/// Send POST /api/start-receiver-stream to remote peer IP and port
pub async fn send_start_receiver_stream(
    ip: &str,
    port: u16,
    payload: &StartReceiverStreamPayload,
) -> Result<bool, String> {
    let addr = format!("{}:{}", ip, port);
    let body_json = serde_json::to_string(payload).map_err(|e| e.to_string())?;

    let mut stream = tokio::time::timeout(Duration::from_millis(4000), TcpStream::connect(&addr))
        .await
        .map_err(|_| format!("Connection timeout connecting to http://{}/api/start-receiver-stream", addr))?
        .map_err(|e| format!("Failed to connect to http://{}/api/start-receiver-stream: {}", addr, e))?;

    let req = format!(
        "POST /api/start-receiver-stream HTTP/1.1\r\n\
         Host: {}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         User-Agent: WirelessMonitor/0.1\r\n\
         Accept: application/json\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        addr,
        body_json.len(),
        body_json
    );

    stream
        .write_all(req.as_bytes())
        .await
        .map_err(|e| format!("Failed to write request: {}", e))?;
    stream
        .flush()
        .await
        .map_err(|e| format!("Failed to flush request: {}", e))?;

    let mut response_buf = Vec::new();
    tokio::time::timeout(Duration::from_millis(4000), stream.read_to_end(&mut response_buf))
        .await
        .map_err(|_| format!("Read timeout from http://{}/api/start-receiver-stream", addr))?
        .map_err(|e| format!("Read error from http://{}/api/start-receiver-stream: {}", addr, e))?;

    let body_str = extract_http_body(&response_buf)?;
    let resp: StreamControlResponse = serde_json::from_str(&body_str)
        .map_err(|e| format!("Invalid JSON response from {}: {}. Raw: {}", addr, e, body_str))?;
    Ok(resp.success)
}

/// Send POST /api/stop-receiver-stream to remote peer IP and port
pub async fn send_stop_receiver_stream(ip: &str, port: u16) -> Result<bool, String> {
    let addr = format!("{}:{}", ip, port);
    let mut stream = match tokio::time::timeout(Duration::from_millis(3000), TcpStream::connect(&addr)).await {
        Ok(Ok(s)) => s,
        _ => return Ok(false),
    };

    let req = format!(
        "POST /api/stop-receiver-stream HTTP/1.1\r\n\
         Host: {}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: 0\r\n\
         User-Agent: WirelessMonitor/0.1\r\n\
         Connection: close\r\n\
         \r\n",
        addr
    );

    let _ = stream.write_all(req.as_bytes()).await;
    let _ = stream.flush().await;

    let mut response_buf = Vec::new();
    let _ = tokio::time::timeout(Duration::from_millis(3000), stream.read_to_end(&mut response_buf)).await;
    Ok(true)
}

/// Connect directly to a peer by IP address, query /api/info, and inject into discovery service
pub async fn connect_to_ip(state: &AppState, ip_str: &str) -> Result<PeerDevice, String> {
    let trimmed = ip_str.trim();
    if trimmed.is_empty() {
        return Err("IP address cannot be empty".to_string());
    }

    let (host, port) = if let Some((h, p)) = trimmed.split_once(':') {
        let p_num = p
            .trim()
            .parse::<u16>()
            .map_err(|e| format!("Invalid port in '{}': {}", trimmed, e))?;
        (h.trim(), p_num)
    } else {
        (trimmed, 5000)
    };

    let ip_addr = host
        .parse::<IpAddr>()
        .map_err(|e| format!("Invalid IPv4/IPv6 address '{}': {}", host, e))?;

    let info = if state.simulated {
        match send_get_info(host, port).await {
            Ok(i) => i,
            Err(_) => SignalingInfo {
                device_id: format!("device-{}", host),
                hostname: format!("Remote-{}", host),
                os: "windows".to_string(),
                role: "monitor".to_string(),
                capabilities: vec!["1080p".to_string(), "1440p".to_string()],
                max_resolution: "1920x1080".to_string(),
                max_fps: 60,
            },
        }
    } else {
        send_get_info(host, port).await?
    };

    let peer = PeerDevice::with_capabilities(
        &info.device_id,
        &info.hostname,
        ip_addr,
        port,
        &info.role,
        info.capabilities,
        &info.max_resolution,
        info.max_fps,
    );

    let _ = state.discovery_service.inject_peer(peer.clone());
    Ok(peer)
}
