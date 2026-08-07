//! Outbound WebSocket executor abstraction (Z2).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{sleep, timeout};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

use super::outbound::ExecutorKind;
use plurora_core::RedactionState;

/// Request to open a WebSocket connection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OutboundWebSocketOpenRequest {
    pub capability_id: String,
    pub package_id: String,
    pub destination_host: String,
    pub path: Option<String>,
    pub purpose: Option<String>,
    pub subprotocols: Vec<String>,
    pub secret_refs: Vec<String>,
    pub metadata: Value,
    /// Pre-resolved headers (raw); secrets injected in handshake. Static-only.
    pub static_headers: HashMap<String, String>,
    pub secret_headers: HashMap<String, String>,
    pub max_frame_bytes: usize,
    pub max_total_bytes_inbound: usize,
    pub max_total_bytes_outbound: usize,
    pub max_idle_ms: u64,
    pub max_duration_ms: u64,
}

#[derive(Debug)]
pub struct OutboundWebSocketSession {
    pub connection_id: String,
    pub subprotocol_negotiated: Option<String>,
    pub redaction_state: RedactionState,
    pub network_performed: bool,
    pub executor_kind: ExecutorKind,
    /// Channel emitting frames + lifecycle events to the dispatch layer.
    pub events: mpsc::UnboundedReceiver<WebSocketEvent>,
}

#[derive(Debug, Clone)]
pub enum WebSocketEvent {
    Opened {
        connection_id: String,
        subprotocol: Option<String>,
    },
    Frame {
        connection_id: String,
        direction: FrameDirection,
        kind: FrameKind,
        bytes: usize,
        seq: u64,
        payload: WebSocketFramePayload,
    },
    Error {
        connection_id: String,
        code: String,
        message_redacted: String,
    },
    Closed {
        connection_id: String,
        code: u16,
        reason: String,
        total_frames_in: u64,
        total_frames_out: u64,
        total_bytes_in: u64,
        total_bytes_out: u64,
        duration_ms: u64,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrameDirection {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrameKind {
    Text,
    Binary,
}

#[derive(Debug, Clone)]
pub enum WebSocketFramePayload {
    Text(String),
    Binary(Bytes),
}

#[derive(Debug, Clone)]
pub enum OutboundWebSocketFrame {
    Text(String),
    Binary(Bytes),
}

impl OutboundWebSocketFrame {
    fn len(&self) -> usize {
        match self {
            Self::Text(text) => text.len(),
            Self::Binary(bytes) => bytes.len(),
        }
    }

    fn kind(&self) -> FrameKind {
        match self {
            Self::Text(_) => FrameKind::Text,
            Self::Binary(_) => FrameKind::Binary,
        }
    }

    fn payload(&self) -> WebSocketFramePayload {
        match self {
            Self::Text(text) => WebSocketFramePayload::Text(text.clone()),
            Self::Binary(bytes) => WebSocketFramePayload::Binary(bytes.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SendStatus {
    Ok,
    BufferFull,
    ConnectionNotFound,
    ConnectionClosed,
}

#[async_trait]
pub trait WebSocketExecutor: Send + Sync + 'static {
    async fn open(&self, req: OutboundWebSocketOpenRequest) -> Result<OutboundWebSocketSession>;
    async fn send(&self, connection_id: &str, frame: OutboundWebSocketFrame) -> Result<SendStatus>;
    async fn close(&self, connection_id: &str, code: u16, reason: Option<String>) -> Result<()>;
}

pub struct DenyAllWebSocketExecutor;

#[async_trait]
impl WebSocketExecutor for DenyAllWebSocketExecutor {
    async fn open(&self, _req: OutboundWebSocketOpenRequest) -> Result<OutboundWebSocketSession> {
        Err(anyhow::anyhow!("websocket outbound denied by host policy"))
    }

    async fn send(
        &self,
        _connection_id: &str,
        _frame: OutboundWebSocketFrame,
    ) -> Result<SendStatus> {
        Err(anyhow::anyhow!("websocket outbound denied by host policy"))
    }

    async fn close(&self, _connection_id: &str, _code: u16, _reason: Option<String>) -> Result<()> {
        Err(anyhow::anyhow!("websocket outbound denied by host policy"))
    }
}

#[derive(Clone)]
struct FakeConnection {
    tx: mpsc::UnboundedSender<WebSocketEvent>,
    state: Arc<Mutex<FakeConnectionState>>,
}

#[derive(Default)]
struct FakeConnectionState {
    next_seq: u64,
    frames_in: u64,
    frames_out: u64,
    bytes_in: u64,
    bytes_out: u64,
    closed: bool,
    started: Option<Instant>,
}

/// Scriptable no-network WebSocket executor for tests and conformance.
pub struct FakeWebSocketExecutor {
    canned_inbound_frames: Vec<OutboundWebSocketFrame>,
    auto_close_after_canned: bool,
    simulated_close: Option<(Option<String>, u16, String)>,
    max_concurrent_connections: Option<usize>,
    connections: RwLock<HashMap<String, FakeConnection>>,
    recorded_outbound: Mutex<Vec<(String, OutboundWebSocketFrame)>>,
}

impl FakeWebSocketExecutor {
    pub fn new() -> Self {
        Self {
            canned_inbound_frames: Vec::new(),
            auto_close_after_canned: false,
            simulated_close: None,
            max_concurrent_connections: None,
            connections: RwLock::new(HashMap::new()),
            recorded_outbound: Mutex::new(Vec::new()),
        }
    }

    pub fn with_canned_inbound_frames(frames: Vec<OutboundWebSocketFrame>) -> Self {
        Self {
            canned_inbound_frames: frames,
            auto_close_after_canned: true,
            simulated_close: None,
            max_concurrent_connections: None,
            connections: RwLock::new(HashMap::new()),
            recorded_outbound: Mutex::new(Vec::new()),
        }
    }

    pub fn with_simulated_idle_timeout() -> Self {
        Self {
            simulated_close: Some((
                Some("idle_timeout".to_string()),
                1013,
                "idle_timeout".to_string(),
            )),
            ..Self::new()
        }
    }

    pub fn with_simulated_byte_limit(frames: Vec<OutboundWebSocketFrame>) -> Self {
        Self {
            canned_inbound_frames: frames,
            simulated_close: Some((
                Some("inbound_limit".to_string()),
                1009,
                "inbound_limit".to_string(),
            )),
            ..Self::new()
        }
    }

    pub fn with_max_concurrent_connections(max: usize) -> Self {
        Self {
            max_concurrent_connections: Some(max),
            ..Self::new()
        }
    }

    pub async fn recorded_outbound_frames(&self) -> Vec<(String, OutboundWebSocketFrame)> {
        self.recorded_outbound.lock().await.clone()
    }
}

impl Default for FakeWebSocketExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WebSocketExecutor for FakeWebSocketExecutor {
    async fn open(&self, req: OutboundWebSocketOpenRequest) -> Result<OutboundWebSocketSession> {
        if let Some(max) = self.max_concurrent_connections {
            if self.connections.read().await.len() >= max {
                anyhow::bail!("websocket connection cap exceeded");
            }
        }
        let connection_id = req
            .metadata
            .get("connection_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let subprotocol = req.subprotocols.first().cloned();
        let (tx, rx) = mpsc::unbounded_channel();
        let state = Arc::new(Mutex::new(FakeConnectionState {
            started: Some(Instant::now()),
            ..FakeConnectionState::default()
        }));
        self.connections.write().await.insert(
            connection_id.clone(),
            FakeConnection {
                tx: tx.clone(),
                state: state.clone(),
            },
        );

        let canned = self.canned_inbound_frames.clone();
        let auto_close = self.auto_close_after_canned;
        let simulated_close = self.simulated_close.clone();
        let cid = connection_id.clone();
        tokio::spawn(async move {
            let _ = tx.send(WebSocketEvent::Opened {
                connection_id: cid.clone(),
                subprotocol,
            });
            for frame in canned {
                let mut guard = state.lock().await;
                guard.next_seq += 1;
                guard.frames_in += 1;
                guard.bytes_in += frame.len() as u64;
                let event = WebSocketEvent::Frame {
                    connection_id: cid.clone(),
                    direction: FrameDirection::Inbound,
                    kind: frame.kind(),
                    bytes: frame.len(),
                    seq: guard.next_seq,
                    payload: frame.payload(),
                };
                drop(guard);
                let _ = tx.send(event);
            }
            if let Some((error_code, close_code, reason)) = simulated_close {
                let mut guard = state.lock().await;
                guard.closed = true;
                if let Some(error_code) = error_code {
                    let _ = tx.send(WebSocketEvent::Error {
                        connection_id: cid.clone(),
                        code: error_code.clone(),
                        message_redacted: format!("websocket {error_code}"),
                    });
                }
                let duration_ms = guard
                    .started
                    .map(|start| start.elapsed().as_millis() as u64)
                    .unwrap_or(0);
                let _ = tx.send(WebSocketEvent::Closed {
                    connection_id: cid,
                    code: close_code,
                    reason,
                    total_frames_in: guard.frames_in,
                    total_frames_out: guard.frames_out,
                    total_bytes_in: guard.bytes_in,
                    total_bytes_out: guard.bytes_out,
                    duration_ms,
                });
            } else if auto_close {
                let mut guard = state.lock().await;
                guard.closed = true;
                let duration_ms = guard
                    .started
                    .map(|start| start.elapsed().as_millis() as u64)
                    .unwrap_or(0);
                let _ = tx.send(WebSocketEvent::Closed {
                    connection_id: cid,
                    code: 1000,
                    reason: "fake_done".to_string(),
                    total_frames_in: guard.frames_in,
                    total_frames_out: guard.frames_out,
                    total_bytes_in: guard.bytes_in,
                    total_bytes_out: guard.bytes_out,
                    duration_ms,
                });
            }
        });

        Ok(OutboundWebSocketSession {
            connection_id,
            subprotocol_negotiated: req.subprotocols.first().cloned(),
            redaction_state: RedactionState::Redacted,
            network_performed: false,
            executor_kind: ExecutorKind::Fake,
            events: rx,
        })
    }

    async fn send(&self, connection_id: &str, frame: OutboundWebSocketFrame) -> Result<SendStatus> {
        let conn = match self.connections.read().await.get(connection_id).cloned() {
            Some(conn) => conn,
            None => return Ok(SendStatus::ConnectionNotFound),
        };
        let mut guard = conn.state.lock().await;
        if guard.closed {
            return Ok(SendStatus::ConnectionClosed);
        }
        guard.next_seq += 1;
        guard.frames_out += 1;
        guard.bytes_out += frame.len() as u64;
        let seq = guard.next_seq;
        drop(guard);
        self.recorded_outbound
            .lock()
            .await
            .push((connection_id.to_string(), frame.clone()));
        let _ = conn.tx.send(WebSocketEvent::Frame {
            connection_id: connection_id.to_string(),
            direction: FrameDirection::Outbound,
            kind: frame.kind(),
            bytes: frame.len(),
            seq,
            payload: frame.payload(),
        });
        Ok(SendStatus::Ok)
    }

    async fn close(&self, connection_id: &str, code: u16, reason: Option<String>) -> Result<()> {
        let conn = self
            .connections
            .read()
            .await
            .get(connection_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("websocket connection not found"))?;
        let mut guard = conn.state.lock().await;
        if guard.closed {
            return Ok(());
        }
        guard.closed = true;
        let duration_ms = guard
            .started
            .map(|start| start.elapsed().as_millis() as u64)
            .unwrap_or(0);
        let _ = conn.tx.send(WebSocketEvent::Closed {
            connection_id: connection_id.to_string(),
            code,
            reason: reason.unwrap_or_else(|| "closed".to_string()),
            total_frames_in: guard.frames_in,
            total_frames_out: guard.frames_out,
            total_bytes_in: guard.bytes_in,
            total_bytes_out: guard.bytes_out,
            duration_ms,
        });
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct LiveWebSocketExecutorConfig {
    pub allowed_hosts: Vec<String>,
    pub wss_only: bool,
    pub max_idle_ms: u64,
    pub max_duration_ms: u64,
    pub max_frame_bytes: usize,
    pub max_total_bytes_inbound: usize,
    pub max_total_bytes_outbound: usize,
    pub max_concurrent_connections: usize,
    pub allow_insecure_ws_for_tests: bool,
}

impl Default for LiveWebSocketExecutorConfig {
    fn default() -> Self {
        Self {
            allowed_hosts: Vec::new(),
            wss_only: true,
            max_idle_ms: 60_000,
            max_duration_ms: 1_800_000,
            max_frame_bytes: 65_536,
            max_total_bytes_inbound: 10 * 1024 * 1024,
            max_total_bytes_outbound: 10 * 1024 * 1024,
            max_concurrent_connections: 8,
            allow_insecure_ws_for_tests: false,
        }
    }
}

pub struct LiveWebSocketExecutor {
    config: LiveWebSocketExecutorConfig,
    connections: RwLock<HashMap<String, LiveConnection>>,
}

struct LiveConnection {
    tx: mpsc::Sender<LiveCommand>,
    max_frame_bytes: usize,
    max_total_bytes_outbound: usize,
    total_bytes_outbound: u64,
    closed: bool,
}

enum LiveCommand {
    Send(OutboundWebSocketFrame),
    Close(u16, Option<String>),
}

impl LiveWebSocketExecutor {
    pub fn new(config: LiveWebSocketExecutorConfig) -> Self {
        Self {
            config,
            connections: RwLock::new(HashMap::new()),
        }
    }

    pub fn new_from_profile_fields(
        allowed_hosts: Vec<String>,
        wss_only: bool,
        max_idle_ms: u64,
        max_duration_ms: u64,
        max_frame_bytes: usize,
        max_total_bytes_inbound: usize,
        max_total_bytes_outbound: usize,
        max_concurrent_connections: usize,
        allow_insecure_ws_for_tests: bool,
    ) -> Self {
        Self::new(LiveWebSocketExecutorConfig {
            allowed_hosts,
            wss_only,
            max_idle_ms,
            max_duration_ms,
            max_frame_bytes,
            max_total_bytes_inbound,
            max_total_bytes_outbound,
            max_concurrent_connections,
            allow_insecure_ws_for_tests,
        })
    }

    pub fn new_from_profile<P>(profile: &P) -> Result<Self>
    where
        P: LiveWebSocketProfile,
    {
        Ok(Self::new(LiveWebSocketExecutorConfig {
            allowed_hosts: profile.allowed_hosts().to_vec(),
            wss_only: profile.wss_only(),
            max_idle_ms: profile.max_idle_ms(),
            max_duration_ms: profile.max_duration_ms(),
            max_frame_bytes: profile.max_frame_bytes(),
            max_total_bytes_inbound: profile.max_total_bytes_inbound(),
            max_total_bytes_outbound: profile.max_total_bytes_outbound(),
            max_concurrent_connections: profile.max_concurrent_connections(),
            allow_insecure_ws_for_tests: profile.allow_insecure_ws_for_tests(),
        }))
    }

    fn validate_open_request(&self, req: &OutboundWebSocketOpenRequest) -> Result<String> {
        if self.config.allowed_hosts.is_empty()
            || !self
                .config
                .allowed_hosts
                .iter()
                .any(|allowed| ws_host_matches(allowed, &req.destination_host))
        {
            anyhow::bail!(
                "host policy does not allow websocket host '{}'",
                req.destination_host
            );
        }
        let scheme = req
            .metadata
            .get("scheme")
            .and_then(Value::as_str)
            .unwrap_or("wss");
        if self.config.wss_only && scheme != "wss" {
            let loopback = is_loopback_host(&req.destination_host);
            if !self.config.allow_insecure_ws_for_tests || !loopback || scheme != "ws" {
                anyhow::bail!(
                    "live websocket executor rejects non-WSS URL for host '{}'",
                    req.destination_host
                );
            }
        } else if scheme != "wss" && scheme != "ws" {
            anyhow::bail!("live websocket executor requires ws or wss scheme");
        }
        let raw_path = req.path.as_deref().unwrap_or("/");
        let path = if raw_path.starts_with('/') {
            raw_path.to_string()
        } else {
            format!("/{raw_path}")
        };
        let url = format!("{scheme}://{}{}", req.destination_host, path);
        let parsed = reqwest::Url::parse(&url)
            .map_err(|e| anyhow::anyhow!("invalid websocket URL '{}': {e}", url))?;
        let actual_host = parsed.host_str().unwrap_or("");
        if !actual_host.eq_ignore_ascii_case(
            req.destination_host
                .split(':')
                .next()
                .unwrap_or(&req.destination_host),
        ) && !actual_host.eq_ignore_ascii_case(&req.destination_host)
        {
            anyhow::bail!(
                "websocket URL host '{}' does not match destination_host '{}'",
                actual_host,
                req.destination_host
            );
        }
        Ok(url)
    }
}

pub trait LiveWebSocketProfile {
    fn allowed_hosts(&self) -> &[String];
    fn wss_only(&self) -> bool;
    fn max_idle_ms(&self) -> u64;
    fn max_duration_ms(&self) -> u64;
    fn max_frame_bytes(&self) -> usize;
    fn max_total_bytes_inbound(&self) -> usize;
    fn max_total_bytes_outbound(&self) -> usize;
    fn max_concurrent_connections(&self) -> usize;
    fn allow_insecure_ws_for_tests(&self) -> bool;
}

#[async_trait]
impl WebSocketExecutor for LiveWebSocketExecutor {
    async fn open(&self, req: OutboundWebSocketOpenRequest) -> Result<OutboundWebSocketSession> {
        let url = self.validate_open_request(&req)?;
        if self.connections.read().await.len() >= self.config.max_concurrent_connections {
            anyhow::bail!("websocket connection cap exceeded");
        }
        let mut request = url.into_client_request()?;
        for subprotocol in &req.subprotocols {
            if subprotocol.contains(',') || subprotocol.trim().is_empty() {
                anyhow::bail!("invalid websocket subprotocol");
            }
        }
        if !req.subprotocols.is_empty() {
            request.headers_mut().insert(
                HeaderName::from_static("sec-websocket-protocol"),
                HeaderValue::from_str(&req.subprotocols.join(", "))?,
            );
        }
        for (name, value) in req.static_headers.iter().chain(req.secret_headers.iter()) {
            let header_name = HeaderName::from_bytes(name.as_bytes())?;
            let header_value = HeaderValue::from_str(value)?;
            request.headers_mut().insert(header_name, header_value);
        }

        let (ws_stream, response) = tokio_tungstenite::connect_async(request).await?;
        if response.status().as_u16() != 101 {
            anyhow::bail!("websocket upgrade failed with status {}", response.status());
        }
        let negotiated = response
            .headers()
            .get("sec-websocket-protocol")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        if let Some(protocol) = &negotiated {
            if !req
                .subprotocols
                .iter()
                .any(|requested| requested == protocol)
            {
                anyhow::bail!("websocket negotiated unexpected subprotocol");
            }
        }

        let connection_id = req
            .metadata
            .get("connection_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        self.connections.write().await.insert(
            connection_id.clone(),
            LiveConnection {
                tx: cmd_tx,
                max_frame_bytes: req.max_frame_bytes.min(self.config.max_frame_bytes),
                max_total_bytes_outbound: req
                    .max_total_bytes_outbound
                    .min(self.config.max_total_bytes_outbound),
                total_bytes_outbound: 0,
                closed: false,
            },
        );
        let _ = event_tx.send(WebSocketEvent::Opened {
            connection_id: connection_id.clone(),
            subprotocol: negotiated.clone(),
        });
        spawn_live_actor(
            connection_id.clone(),
            ws_stream,
            cmd_rx,
            event_tx,
            req.max_frame_bytes.min(self.config.max_frame_bytes),
            req.max_total_bytes_inbound
                .min(self.config.max_total_bytes_inbound),
            req.max_idle_ms.min(self.config.max_idle_ms),
            req.max_duration_ms.min(self.config.max_duration_ms),
        );
        Ok(OutboundWebSocketSession {
            connection_id,
            subprotocol_negotiated: negotiated,
            redaction_state: RedactionState::Redacted,
            network_performed: true,
            executor_kind: ExecutorKind::Real,
            events: event_rx,
        })
    }

    async fn send(&self, connection_id: &str, frame: OutboundWebSocketFrame) -> Result<SendStatus> {
        let mut connections = self.connections.write().await;
        let Some(conn) = connections.get_mut(connection_id) else {
            return Ok(SendStatus::ConnectionNotFound);
        };
        if conn.closed {
            return Ok(SendStatus::ConnectionClosed);
        }
        if frame.len() > conn.max_frame_bytes {
            anyhow::bail!("websocket frame exceeds max_frame_bytes");
        }
        let next_total = conn.total_bytes_outbound.saturating_add(frame.len() as u64);
        if next_total > conn.max_total_bytes_outbound as u64 {
            anyhow::bail!("websocket outbound byte cap exceeded");
        }
        match conn.tx.try_send(LiveCommand::Send(frame)) {
            Ok(()) => {
                conn.total_bytes_outbound = next_total;
                Ok(SendStatus::Ok)
            }
            Err(mpsc::error::TrySendError::Full(_)) => Ok(SendStatus::BufferFull),
            Err(mpsc::error::TrySendError::Closed(_)) => Ok(SendStatus::ConnectionClosed),
        }
    }

    async fn close(&self, connection_id: &str, code: u16, reason: Option<String>) -> Result<()> {
        let mut connections = self.connections.write().await;
        let Some(conn) = connections.get_mut(connection_id) else {
            return Ok(());
        };
        conn.closed = true;
        conn.tx
            .try_send(LiveCommand::Close(code, reason))
            .map_err(|_| anyhow::anyhow!("websocket connection closed"))?;
        Ok(())
    }
}

fn spawn_live_actor<S>(
    connection_id: String,
    ws_stream: tokio_tungstenite::WebSocketStream<S>,
    mut cmd_rx: mpsc::Receiver<LiveCommand>,
    event_tx: mpsc::UnboundedSender<WebSocketEvent>,
    max_frame_bytes: usize,
    max_total_bytes_inbound: usize,
    max_idle_ms: u64,
    max_duration_ms: u64,
) where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let started = Instant::now();
        let mut last_activity = Instant::now();
        let mut seq = 0u64;
        let mut frames_in = 0u64;
        let mut frames_out = 0u64;
        let mut bytes_in = 0u64;
        let mut bytes_out = 0u64;
        let (mut sink, mut stream) = ws_stream.split();
        loop {
            if started.elapsed() >= Duration::from_millis(max_duration_ms) {
                let _ = event_tx.send(WebSocketEvent::Closed {
                    connection_id: connection_id.clone(),
                    code: 1013,
                    reason: "max_duration_ms".to_string(),
                    total_frames_in: frames_in,
                    total_frames_out: frames_out,
                    total_bytes_in: bytes_in,
                    total_bytes_out: bytes_out,
                    duration_ms: started.elapsed().as_millis() as u64,
                });
                break;
            }
            if last_activity.elapsed() >= Duration::from_millis(max_idle_ms) {
                let _ = event_tx.send(WebSocketEvent::Error {
                    connection_id: connection_id.clone(),
                    code: "idle_timeout".to_string(),
                    message_redacted: "websocket idle timeout".to_string(),
                });
                let _ = event_tx.send(WebSocketEvent::Closed {
                    connection_id: connection_id.clone(),
                    code: 1013,
                    reason: "idle_timeout".to_string(),
                    total_frames_in: frames_in,
                    total_frames_out: frames_out,
                    total_bytes_in: bytes_in,
                    total_bytes_out: bytes_out,
                    duration_ms: started.elapsed().as_millis() as u64,
                });
                break;
            }
            tokio::select! {
                maybe_cmd = cmd_rx.recv() => {
                    match maybe_cmd {
                        Some(LiveCommand::Send(frame)) => {
                            if frame.len() > max_frame_bytes {
                                let _ = event_tx.send(WebSocketEvent::Error { connection_id: connection_id.clone(), code: "frame_too_large".to_string(), message_redacted: "websocket outbound frame too large".to_string() });
                                continue;
                            }
                            let msg = match &frame {
                                OutboundWebSocketFrame::Text(text) => Message::Text(text.clone().into()),
                                OutboundWebSocketFrame::Binary(bytes) => Message::Binary(bytes.to_vec()),
                            };
                            if sink.send(msg).await.is_err() {
                                let _ = event_tx.send(WebSocketEvent::Error { connection_id: connection_id.clone(), code: "send_failed".to_string(), message_redacted: "websocket send failed".to_string() });
                                break;
                            }
                            last_activity = Instant::now();
                            seq += 1;
                            frames_out += 1;
                            bytes_out += frame.len() as u64;
                            let _ = event_tx.send(WebSocketEvent::Frame { connection_id: connection_id.clone(), direction: FrameDirection::Outbound, kind: frame.kind(), bytes: frame.len(), seq, payload: frame.payload() });
                        }
                        Some(LiveCommand::Close(_code, _reason)) => {
                            let _ = sink.send(Message::Close(None)).await;
                            let _ = event_tx.send(WebSocketEvent::Closed { connection_id: connection_id.clone(), code: 1000, reason: "closed".to_string(), total_frames_in: frames_in, total_frames_out: frames_out, total_bytes_in: bytes_in, total_bytes_out: bytes_out, duration_ms: started.elapsed().as_millis() as u64 });
                            break;
                        }
                        None => break,
                    }
                }
                maybe_msg = timeout(Duration::from_millis(100), stream.next()) => {
                    match maybe_msg {
                        Ok(Some(Ok(Message::Text(text)))) => {
                            let bytes = text.len();
                            if bytes > max_frame_bytes || bytes_in.saturating_add(bytes as u64) > max_total_bytes_inbound as u64 {
                                let _ = event_tx.send(WebSocketEvent::Error { connection_id: connection_id.clone(), code: "inbound_limit".to_string(), message_redacted: "websocket inbound limit exceeded".to_string() });
                                break;
                            }
                            last_activity = Instant::now();
                            seq += 1;
                            frames_in += 1;
                            bytes_in += bytes as u64;
                            let _ = event_tx.send(WebSocketEvent::Frame { connection_id: connection_id.clone(), direction: FrameDirection::Inbound, kind: FrameKind::Text, bytes, seq, payload: WebSocketFramePayload::Text(text.to_string()) });
                        }
                        Ok(Some(Ok(Message::Binary(bytes)))) => {
                            let len = bytes.len();
                            if len > max_frame_bytes || bytes_in.saturating_add(len as u64) > max_total_bytes_inbound as u64 {
                                let _ = event_tx.send(WebSocketEvent::Error { connection_id: connection_id.clone(), code: "inbound_limit".to_string(), message_redacted: "websocket inbound limit exceeded".to_string() });
                                break;
                            }
                            last_activity = Instant::now();
                            seq += 1;
                            frames_in += 1;
                            bytes_in += len as u64;
                            let _ = event_tx.send(WebSocketEvent::Frame { connection_id: connection_id.clone(), direction: FrameDirection::Inbound, kind: FrameKind::Binary, bytes: len, seq, payload: WebSocketFramePayload::Binary(bytes.into()) });
                        }
                        Ok(Some(Ok(Message::Close(_)))) | Ok(None) => {
                            let _ = event_tx.send(WebSocketEvent::Closed { connection_id: connection_id.clone(), code: 1000, reason: "remote_closed".to_string(), total_frames_in: frames_in, total_frames_out: frames_out, total_bytes_in: bytes_in, total_bytes_out: bytes_out, duration_ms: started.elapsed().as_millis() as u64 });
                            break;
                        }
                        Ok(Some(Ok(Message::Ping(ping)))) => { let _ = sink.send(Message::Pong(ping)).await; }
                        Ok(Some(Ok(Message::Pong(_)))) => { last_activity = Instant::now(); }
                        Ok(Some(Ok(Message::Frame(_)))) => {}
                        Ok(Some(Err(_))) => {
                            let _ = event_tx.send(WebSocketEvent::Error { connection_id: connection_id.clone(), code: "read_failed".to_string(), message_redacted: "websocket read failed".to_string() });
                            break;
                        }
                        Err(_) => { sleep(Duration::from_millis(1)).await; }
                    }
                }
            }
        }
    });
}

fn ws_host_matches(pattern: &str, destination: &str) -> bool {
    if pattern.eq_ignore_ascii_case(destination) {
        return true;
    }
    let pattern = pattern.to_ascii_lowercase();
    let destination = destination.to_ascii_lowercase();
    if let Some(suffix) = pattern.strip_prefix("*.") {
        return destination == suffix || destination.ends_with(&format!(".{suffix}"));
    }
    false
}

fn is_loopback_host(host: &str) -> bool {
    let bare = host.split(':').next().unwrap_or(host);
    matches!(bare, "127.0.0.1" | "localhost" | "[::1]" | "::1")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws_req(host: &str) -> OutboundWebSocketOpenRequest {
        OutboundWebSocketOpenRequest {
            capability_id: "test/pkg/ws".to_string(),
            package_id: "test/pkg".to_string(),
            destination_host: host.to_string(),
            path: Some("/ws".to_string()),
            purpose: None,
            subprotocols: vec!["json".to_string()],
            secret_refs: Vec::new(),
            metadata: Value::Null,
            static_headers: HashMap::new(),
            secret_headers: HashMap::new(),
            max_frame_bytes: 1024,
            max_total_bytes_inbound: 4096,
            max_total_bytes_outbound: 4096,
            max_idle_ms: 1000,
            max_duration_ms: 5000,
        }
    }

    #[tokio::test]
    async fn deny_all_websocket_executor_rejects_open() {
        let err = DenyAllWebSocketExecutor
            .open(ws_req("api.example.com"))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("denied"));
    }

    #[tokio::test]
    async fn fake_websocket_executor_emits_opened_then_canned_frames_then_closed() {
        let executor = FakeWebSocketExecutor::with_canned_inbound_frames(vec![
            OutboundWebSocketFrame::Text("one".to_string()),
            OutboundWebSocketFrame::Binary(Bytes::from_static(b"two")),
        ]);
        let mut session = executor.open(ws_req("api.example.com")).await.unwrap();
        assert!(matches!(
            session.events.recv().await.unwrap(),
            WebSocketEvent::Opened { .. }
        ));
        assert!(matches!(
            session.events.recv().await.unwrap(),
            WebSocketEvent::Frame {
                seq: 1,
                direction: FrameDirection::Inbound,
                ..
            }
        ));
        assert!(matches!(
            session.events.recv().await.unwrap(),
            WebSocketEvent::Frame {
                seq: 2,
                direction: FrameDirection::Inbound,
                ..
            }
        ));
        assert!(matches!(
            session.events.recv().await.unwrap(),
            WebSocketEvent::Closed { code: 1000, .. }
        ));
    }

    #[tokio::test]
    async fn fake_websocket_executor_send_records_outbound_frame() {
        let executor = FakeWebSocketExecutor::new();
        let mut session = executor.open(ws_req("api.example.com")).await.unwrap();
        let _ = session.events.recv().await.unwrap();
        let status = executor
            .send(
                &session.connection_id,
                OutboundWebSocketFrame::Text("hello".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(status, SendStatus::Ok);
        assert_eq!(executor.recorded_outbound_frames().await.len(), 1);
        assert!(matches!(
            session.events.recv().await.unwrap(),
            WebSocketEvent::Frame {
                direction: FrameDirection::Outbound,
                seq: 1,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn fake_websocket_executor_close_emits_closed() {
        let executor = FakeWebSocketExecutor::new();
        let mut session = executor.open(ws_req("api.example.com")).await.unwrap();
        let _ = session.events.recv().await.unwrap();
        executor
            .close(&session.connection_id, 1001, Some("bye".to_string()))
            .await
            .unwrap();
        assert!(matches!(
            session.events.recv().await.unwrap(),
            WebSocketEvent::Closed { code: 1001, .. }
        ));
    }

    #[tokio::test]
    async fn fake_websocket_executor_send_to_unknown_connection_returns_not_found() {
        let executor = FakeWebSocketExecutor::new();
        let status = executor
            .send("missing", OutboundWebSocketFrame::Text("hello".to_string()))
            .await
            .unwrap();
        assert_eq!(status, SendStatus::ConnectionNotFound);
    }

    #[tokio::test]
    async fn fake_websocket_frame_seq_increments_per_connection() {
        let executor = FakeWebSocketExecutor::new();
        let mut a = executor.open(ws_req("api.example.com")).await.unwrap();
        let mut b = executor.open(ws_req("api.example.com")).await.unwrap();
        let _ = a.events.recv().await.unwrap();
        let _ = b.events.recv().await.unwrap();
        executor
            .send(
                &a.connection_id,
                OutboundWebSocketFrame::Text("a1".to_string()),
            )
            .await
            .unwrap();
        executor
            .send(
                &a.connection_id,
                OutboundWebSocketFrame::Text("a2".to_string()),
            )
            .await
            .unwrap();
        executor
            .send(
                &b.connection_id,
                OutboundWebSocketFrame::Text("b1".to_string()),
            )
            .await
            .unwrap();
        assert!(matches!(
            a.events.recv().await.unwrap(),
            WebSocketEvent::Frame { seq: 1, .. }
        ));
        assert!(matches!(
            a.events.recv().await.unwrap(),
            WebSocketEvent::Frame { seq: 2, .. }
        ));
        assert!(matches!(
            b.events.recv().await.unwrap(),
            WebSocketEvent::Frame { seq: 1, .. }
        ));
    }

    #[test]
    fn live_websocket_executor_rejects_unallowed_host() {
        let executor = LiveWebSocketExecutor::new(LiveWebSocketExecutorConfig {
            allowed_hosts: vec!["api.example.com".to_string()],
            ..Default::default()
        });
        let err = executor
            .validate_open_request(&ws_req("evil.example.com"))
            .unwrap_err();
        assert!(err.to_string().contains("does not allow"));
    }

    #[test]
    fn live_websocket_executor_rejects_non_wss_when_wss_only() {
        let executor = LiveWebSocketExecutor::new(LiveWebSocketExecutorConfig {
            allowed_hosts: vec!["api.example.com".to_string()],
            ..Default::default()
        });
        let mut req = ws_req("api.example.com");
        req.metadata = serde_json::json!({"scheme": "ws"});
        let err = executor.validate_open_request(&req).unwrap_err();
        assert!(err.to_string().contains("non-WSS"));
    }

    #[test]
    fn live_websocket_executor_accepts_loopback_when_insecure_test_flag_set() {
        let executor = LiveWebSocketExecutor::new(LiveWebSocketExecutorConfig {
            allowed_hosts: vec!["127.0.0.1:12345".to_string()],
            allow_insecure_ws_for_tests: true,
            ..Default::default()
        });
        let mut req = ws_req("127.0.0.1:12345");
        req.metadata = serde_json::json!({"scheme": "ws"});
        assert!(executor.validate_open_request(&req).is_ok());
    }
}
