use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Context;
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::sync::{mpsc, oneshot, watch, Mutex as AsyncMutex};

use super::*;

pub const TARGET_TUNNEL_DATA_CHUNK_BYTES: usize = 64 * 1024;
const STREAM_ID_BYTES: usize = 32;
const OPEN_QUEUE_CAPACITY: usize = 64;
const CONTROL_QUEUE_CAPACITY: usize = 64;
const DATA_QUEUE_CAPACITY: usize = 64;
const STREAM_QUEUE_CAPACITY: usize = 4;
pub const TARGET_TUNNEL_MAX_STREAMS: usize = 128;
const OPEN_TIMEOUT: Duration = Duration::from_secs(10);
const LIVE_CHECK_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetTunnelOpen {
    pub stream_id: String,
    pub target_id: String,
    pub route_id: String,
    pub port_lease_id: String,
    pub port_name: String,
    pub port: u16,
    pub lease_epoch: u64,
    pub policy_epoch: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TargetTunnelHostMessage {
    Open { stream: TargetTunnelOpen },
    Close { stream_id: String },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TargetTunnelAgentMessage {
    Opened { stream_id: String },
    Rejected { stream_id: String },
    Closed { stream_id: String },
}

pub fn encode_target_tunnel_data(stream_id: &str, data: &[u8]) -> Option<Vec<u8>> {
    if !valid_stream_id(stream_id) || data.len() > TARGET_TUNNEL_DATA_CHUNK_BYTES {
        return None;
    }
    let mut frame = Vec::with_capacity(STREAM_ID_BYTES.saturating_add(data.len()));
    frame.extend_from_slice(stream_id.as_bytes());
    frame.extend_from_slice(data);
    Some(frame)
}

pub fn decode_target_tunnel_data(frame: &[u8]) -> Option<(String, &[u8])> {
    if frame.len() < STREAM_ID_BYTES
        || frame.len() > STREAM_ID_BYTES.saturating_add(TARGET_TUNNEL_DATA_CHUNK_BYTES)
    {
        return None;
    }
    let stream_id = std::str::from_utf8(&frame[..STREAM_ID_BYTES]).ok()?;
    if !valid_stream_id(stream_id) {
        return None;
    }
    Some((stream_id.to_string(), &frame[STREAM_ID_BYTES..]))
}

fn valid_stream_id(stream_id: &str) -> bool {
    stream_id.len() == STREAM_ID_BYTES
        && stream_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug)]
struct TargetTunnelConnection {
    connection_id: String,
    active: bool,
    closing: bool,
    open_tx: mpsc::Sender<OpenStreamCommand>,
    shutdown_tx: watch::Sender<bool>,
}

struct OpenStreamCommand {
    connection_id: String,
    stream: TargetTunnelOpen,
    response: oneshot::Sender<anyhow::Result<DuplexStream>>,
}

pub(super) struct RegisteredTargetTunnel {
    connection_id: String,
    open_rx: mpsc::Receiver<OpenStreamCommand>,
    shutdown_rx: watch::Receiver<bool>,
}

impl RegisteredTargetTunnel {
    pub(super) fn connection_id(&self) -> &str {
        &self.connection_id
    }
}

#[derive(Debug, Default)]
pub(super) struct TargetTunnelRegistry {
    connections: Mutex<HashMap<String, TargetTunnelConnection>>,
}

impl TargetTunnelRegistry {
    pub(super) fn register(&self, target_id: &str) -> anyhow::Result<RegisteredTargetTunnel> {
        let mut connections = self
            .connections
            .lock()
            .expect("target tunnel registry lock poisoned");
        if let Some(connection) = connections.get(target_id) {
            if !connection.open_tx.is_closed() {
                anyhow::bail!("target already has an active reverse tunnel");
            }
        }
        connections.remove(target_id);
        let connection_id = uuid::Uuid::new_v4().simple().to_string();
        let (open_tx, open_rx) = mpsc::channel(OPEN_QUEUE_CAPACITY);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        connections.insert(
            target_id.to_string(),
            TargetTunnelConnection {
                connection_id: connection_id.clone(),
                active: false,
                closing: false,
                open_tx,
                shutdown_tx,
            },
        );
        Ok(RegisteredTargetTunnel {
            connection_id,
            open_rx,
            shutdown_rx,
        })
    }

    pub(super) fn remove(&self, target_id: &str, connection_id: &str) {
        let mut connections = self
            .connections
            .lock()
            .expect("target tunnel registry lock poisoned");
        if connections
            .get(target_id)
            .is_some_and(|connection| connection.connection_id == connection_id)
        {
            if let Some(connection) = connections.remove(target_id) {
                let _ = connection.shutdown_tx.send(true);
            }
        }
    }

    fn activate(&self, target_id: &str, connection_id: &str) -> bool {
        let mut connections = self
            .connections
            .lock()
            .expect("target tunnel registry lock poisoned");
        let Some(connection) = connections.get_mut(target_id) else {
            return false;
        };
        if connection.connection_id != connection_id
            || connection.closing
            || connection.open_tx.is_closed()
        {
            return false;
        }
        connection.active = true;
        true
    }

    fn begin_close(&self, target_id: &str, connection_id: &str) -> bool {
        let mut connections = self
            .connections
            .lock()
            .expect("target tunnel registry lock poisoned");
        let Some(connection) = connections.get_mut(target_id) else {
            return false;
        };
        if connection.connection_id != connection_id {
            return false;
        }
        connection.active = false;
        connection.closing = true;
        let _ = connection.shutdown_tx.send(true);
        true
    }

    pub(super) fn disconnect(&self, target_id: &str) {
        if let Some(connection) = self
            .connections
            .lock()
            .expect("target tunnel registry lock poisoned")
            .get_mut(target_id)
        {
            connection.active = false;
            connection.closing = true;
            let _ = connection.shutdown_tx.send(true);
        }
    }

    pub(super) fn connected(&self, target_id: &str) -> bool {
        self.connections
            .lock()
            .expect("target tunnel registry lock poisoned")
            .get(target_id)
            .is_some_and(|connection| {
                connection.active && !connection.closing && !connection.open_tx.is_closed()
            })
    }

    pub(super) fn claimed(&self, target_id: &str) -> bool {
        self.connections
            .lock()
            .expect("target tunnel registry lock poisoned")
            .get(target_id)
            .is_some_and(|connection| !connection.open_tx.is_closed())
    }

    fn is_current(&self, target_id: &str, connection_id: &str) -> bool {
        self.connections
            .lock()
            .expect("target tunnel registry lock poisoned")
            .get(target_id)
            .is_some_and(|connection| {
                connection.connection_id == connection_id
                    && connection.active
                    && !connection.closing
                    && !connection.open_tx.is_closed()
            })
    }

    pub(super) async fn open(&self, mut stream: TargetTunnelOpen) -> anyhow::Result<DuplexStream> {
        stream.stream_id = uuid::Uuid::new_v4().simple().to_string();
        let (connection_id, open_tx) = {
            let connections = self
                .connections
                .lock()
                .expect("target tunnel registry lock poisoned");
            let connection = connections
                .get(&stream.target_id)
                .filter(|connection| connection.active && !connection.open_tx.is_closed())
                .context("target reverse tunnel is not connected")?;
            (connection.connection_id.clone(), connection.open_tx.clone())
        };
        let (response_tx, response_rx) = oneshot::channel();
        tokio::time::timeout(
            OPEN_TIMEOUT,
            open_tx.send(OpenStreamCommand {
                connection_id,
                stream,
                response: response_tx,
            }),
        )
        .await
        .context("target reverse tunnel open queue timed out")?
        .context("target reverse tunnel disconnected before open")?;
        tokio::time::timeout(OPEN_TIMEOUT, response_rx)
            .await
            .context("target reverse tunnel open timed out")?
            .context("target reverse tunnel disconnected during open")?
    }
}

struct HostStreamState {
    reservation: Arc<()>,
    incoming_tx: mpsc::Sender<Vec<u8>>,
    opened_tx: Option<oneshot::Sender<anyhow::Result<()>>>,
}

type HostStreams = Arc<AsyncMutex<HashMap<String, HostStreamState>>>;

pub(super) async fn serve_target_tunnel<S>(
    state: AppState<S>,
    agent: StoredAgent,
    registration: RegisteredTargetTunnel,
    socket: WebSocket,
) where
    S: EventStore,
{
    let target_id = agent.target.id.clone();
    let RegisteredTargetTunnel {
        connection_id,
        mut open_rx,
        mut shutdown_rx,
    } = registration;
    if !target_tunnel_identity_is_live(&state, &agent).await
        || !state
            .target_agents
            .tunnels
            .activate(&target_id, &connection_id)
    {
        state
            .target_agents
            .tunnels
            .remove(&target_id, &connection_id);
        return;
    }
    let (mut socket_tx, mut socket_rx) = socket.split();
    let (control_tx, mut control_rx) = mpsc::channel::<Message>(CONTROL_QUEUE_CAPACITY);
    let (data_tx, mut data_rx) = mpsc::channel::<Message>(DATA_QUEUE_CAPACITY);
    let (connection_failed_tx, mut connection_failed_rx) = watch::channel(false);
    let writer_failed_tx = connection_failed_tx.clone();
    let writer = tokio::spawn(async move {
        loop {
            let message = tokio::select! {
                biased;
                message = control_rx.recv() => message,
                message = data_rx.recv() => message,
            };
            let Some(message) = message else {
                break;
            };
            if socket_tx.send(message).await.is_err() {
                let _ = writer_failed_tx.send(true);
                break;
            }
        }
    });
    let streams: HostStreams = Arc::new(AsyncMutex::new(HashMap::new()));
    if let Err(error) = operation::reconcile_target_workload_projections(&state, &target_id).await {
        tracing::warn!(target_id = %target_id, error = %error, "target tunnel connected but route projection remained unavailable");
    }
    let mut live_check = tokio::time::interval(LIVE_CHECK_INTERVAL);
    live_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            biased;
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
            changed = connection_failed_rx.changed() => {
                if changed.is_err() || *connection_failed_rx.borrow() {
                    break;
                }
            }
            command = open_rx.recv() => {
                let Some(command) = command else { break; };
                if command.connection_id != connection_id
                    || !state
                        .target_agents
                        .tunnels
                        .is_current(&target_id, &connection_id)
                {
                    let _ = command.response.send(Err(anyhow::anyhow!(
                        "target reverse tunnel connection was superseded"
                    )));
                    break;
                }
                start_host_stream(
                    command,
                    streams.clone(),
                    control_tx.clone(),
                    data_tx.clone(),
                    connection_failed_tx.clone(),
                )
                .await;
            }
            message = socket_rx.next() => {
                let Some(Ok(message)) = message else { break; };
                if !handle_agent_message(message, &streams, &control_tx).await {
                    break;
                }
            }
            _ = live_check.tick() => {
                if !target_tunnel_identity_is_live(&state, &agent).await {
                    break;
                }
            }
        }
    }

    if state
        .target_agents
        .tunnels
        .begin_close(&target_id, &connection_id)
    {
        operation::mark_target_workload_routes_unready(&state, &target_id).await;
        state
            .target_agents
            .tunnels
            .remove(&target_id, &connection_id);
    }
    let _ = control_tx.try_send(Message::Close(None));
    let mut streams = streams.lock().await;
    for (_, mut stream) in streams.drain() {
        if let Some(opened_tx) = stream.opened_tx.take() {
            let _ = opened_tx.send(Err(anyhow::anyhow!("target reverse tunnel disconnected")));
        }
    }
    drop(streams);
    writer.abort();
    let _ = writer.await;
}

pub(super) async fn target_tunnel_identity_is_live<S>(
    state: &AppState<S>,
    agent: &StoredAgent,
) -> bool
where
    S: EventStore,
{
    state
        .runtime
        .config()
        .target_registry
        .status(&agent.target.id)
        .await
        .is_some_and(|target| {
            target.status == ExecutionTargetStatusKind::Available
                && target.reachability == ExecutionTargetReachability::ReverseTunnel
                && target.identity_ref == agent.target.identity_ref
                && target.lease_epoch == agent.target.lease_epoch
                && target.policy_epoch == agent.target.policy_epoch
                && target
                    .capabilities
                    .contains(&ExecutionTargetCapability::Workload)
        })
}

async fn start_host_stream(
    command: OpenStreamCommand,
    streams: HostStreams,
    control_tx: mpsc::Sender<Message>,
    data_tx: mpsc::Sender<Message>,
    connection_failed_tx: watch::Sender<bool>,
) {
    let stream_id = command.stream.stream_id.clone();
    if !valid_stream_id(&stream_id) {
        let _ = command
            .response
            .send(Err(anyhow::anyhow!("target tunnel stream id is invalid")));
        return;
    }
    let (client, bridge) = tokio::io::duplex(TARGET_TUNNEL_DATA_CHUNK_BYTES);
    let (incoming_tx, incoming_rx) = mpsc::channel(STREAM_QUEUE_CAPACITY);
    let (opened_tx, opened_rx) = oneshot::channel();
    let reservation = Arc::new(());
    let mut active_streams = streams.lock().await;
    if active_streams.len() >= TARGET_TUNNEL_MAX_STREAMS || active_streams.contains_key(&stream_id)
    {
        let _ = command.response.send(Err(anyhow::anyhow!(
            "target reverse tunnel reached its stream limit"
        )));
        return;
    }
    active_streams.insert(
        stream_id.clone(),
        HostStreamState {
            reservation: reservation.clone(),
            incoming_tx,
            opened_tx: Some(opened_tx),
        },
    );
    drop(active_streams);
    tokio::spawn(pump_host_stream(
        stream_id.clone(),
        bridge,
        incoming_rx,
        streams.clone(),
        reservation.clone(),
        control_tx.clone(),
        data_tx,
        connection_failed_tx.clone(),
    ));
    let control = TargetTunnelHostMessage::Open {
        stream: command.stream,
    };
    let sent = serde_json::to_string(&control)
        .ok()
        .map(Message::Text)
        .is_some_and(|message| control_tx.try_send(message).is_ok());
    if !sent {
        remove_host_stream(&streams, &stream_id, &reservation).await;
        let _ = connection_failed_tx.send(true);
        let _ = command.response.send(Err(anyhow::anyhow!(
            "target reverse tunnel could not queue stream open"
        )));
        return;
    }
    tokio::spawn(async move {
        let mut response = command.response;
        let result = tokio::select! {
            result = tokio::time::timeout(OPEN_TIMEOUT, opened_rx) => result
                .map_err(|_| anyhow::anyhow!("target reverse tunnel stream open timed out"))
                .and_then(|result| {
                    result.map_err(|_| {
                        anyhow::anyhow!("target reverse tunnel disconnected during open")
                    })
                })
                .and_then(|result| result),
            _ = response.closed() => {
                Err(anyhow::anyhow!("target reverse tunnel stream requester disconnected"))
            }
        };
        if result.is_err()
            && remove_host_stream(&streams, &stream_id, &reservation)
                .await
                .is_some()
        {
            let close = serde_json::to_string(&TargetTunnelHostMessage::Close {
                stream_id: stream_id.clone(),
            })
            .ok()
            .map(Message::Text);
            if close.is_none_or(|close| control_tx.try_send(close).is_err()) {
                let _ = connection_failed_tx.send(true);
            }
        }
        let _ = response.send(result.map(|_| client));
    });
}

async fn remove_host_stream(
    streams: &HostStreams,
    stream_id: &str,
    reservation: &Arc<()>,
) -> Option<HostStreamState> {
    let mut streams = streams.lock().await;
    let matches_reservation = streams
        .get(stream_id)
        .is_some_and(|stream| Arc::ptr_eq(&stream.reservation, reservation));
    matches_reservation
        .then(|| streams.remove(stream_id))
        .flatten()
}

async fn handle_agent_message(
    message: Message,
    streams: &HostStreams,
    outbound_tx: &mpsc::Sender<Message>,
) -> bool {
    match message {
        Message::Text(text) => {
            let Ok(control) = serde_json::from_str::<TargetTunnelAgentMessage>(&text) else {
                return false;
            };
            let stream_id = match &control {
                TargetTunnelAgentMessage::Opened { stream_id }
                | TargetTunnelAgentMessage::Rejected { stream_id }
                | TargetTunnelAgentMessage::Closed { stream_id } => stream_id,
            };
            if !valid_stream_id(stream_id) {
                return false;
            }
            let mut streams = streams.lock().await;
            match control {
                TargetTunnelAgentMessage::Opened { stream_id } => {
                    if let Some(stream) = streams.get_mut(&stream_id) {
                        if let Some(opened_tx) = stream.opened_tx.take() {
                            let _ = opened_tx.send(Ok(()));
                        }
                    }
                    true
                }
                TargetTunnelAgentMessage::Rejected { stream_id } => {
                    if let Some(mut stream) = streams.remove(&stream_id) {
                        if let Some(opened_tx) = stream.opened_tx.take() {
                            let _ = opened_tx.send(Err(anyhow::anyhow!(
                                "target rejected the managed tunnel lease"
                            )));
                        }
                    }
                    true
                }
                TargetTunnelAgentMessage::Closed { stream_id } => {
                    streams.remove(&stream_id);
                    true
                }
            }
        }
        Message::Binary(frame) => {
            let Some((stream_id, data)) = decode_target_tunnel_data(&frame) else {
                return false;
            };
            let active = streams
                .lock()
                .await
                .get(&stream_id)
                .map(|stream| (stream.reservation.clone(), stream.incoming_tx.clone()));
            let Some((reservation, incoming_tx)) = active else {
                return true;
            };
            if incoming_tx.try_send(data.to_vec()).is_ok() {
                return true;
            }
            if let Some(mut stream) = remove_host_stream(streams, &stream_id, &reservation).await {
                if let Some(opened_tx) = stream.opened_tx.take() {
                    let _ = opened_tx.send(Err(anyhow::anyhow!(
                        "target reverse tunnel stream backpressure limit reached"
                    )));
                }
            }
            if let Ok(text) = serde_json::to_string(&TargetTunnelHostMessage::Close { stream_id }) {
                return outbound_tx.try_send(Message::Text(text)).is_ok();
            }
            false
        }
        Message::Ping(data) => outbound_tx.try_send(Message::Pong(data)).is_ok(),
        Message::Pong(_) => true,
        Message::Close(_) => false,
    }
}

async fn pump_host_stream(
    stream_id: String,
    bridge: DuplexStream,
    mut incoming_rx: mpsc::Receiver<Vec<u8>>,
    streams: HostStreams,
    reservation: Arc<()>,
    control_tx: mpsc::Sender<Message>,
    data_tx: mpsc::Sender<Message>,
    connection_failed_tx: watch::Sender<bool>,
) {
    let (mut reader, mut writer) = tokio::io::split(bridge);
    let mut buffer = vec![0u8; TARGET_TUNNEL_DATA_CHUNK_BYTES];
    loop {
        tokio::select! {
            read = reader.read(&mut buffer) => {
                let Ok(read) = read else { break; };
                if read == 0 { break; }
                let Some(frame) = encode_target_tunnel_data(&stream_id, &buffer[..read]) else {
                    break;
                };
                if data_tx.send(Message::Binary(frame)).await.is_err() { break; }
            }
            incoming = incoming_rx.recv() => {
                let Some(incoming) = incoming else { break; };
                if writer.write_all(&incoming).await.is_err() { break; }
            }
        }
    }
    let _ = writer.shutdown().await;
    remove_host_stream(&streams, &stream_id, &reservation).await;
    if let Ok(text) = serde_json::to_string(&TargetTunnelHostMessage::Close {
        stream_id: stream_id.clone(),
    }) {
        if control_tx.try_send(Message::Text(text)).is_err() {
            let _ = connection_failed_tx.send(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use axum::body::Bytes;
    use axum::extract::{OriginalUri, State, WebSocketUpgrade};
    use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
    use axum::routing::any;
    use axum::Router;
    use plurora_runtime::{
        ExecutionTarget, ExecutionTargetCapability, ExecutionTargetId, InMemoryEventStore,
        InstallationCreateRequest, InstallationMutationResult, ObjectStore, PortLeaseRequest,
        PortProtocol, ProtocolContext, ProxyProtocol, ProxyRouteAccess, ProxyRouteRegisterRequest,
        ProxyRouteUpstream, Runtime, RuntimeConfig,
    };
    use plurora_work::{
        AcquisitionKind, AcquisitionRecord, ArtifactModel, AssemblyId, AssemblyLock,
        AssemblyRevision, InstallationId, WorkId, WorkRevision,
    };
    use serde_json::Value;
    use tokio::net::{TcpListener, TcpStream};
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::tungstenite::{Error as WebSocketError, Message as TunnelMessage};
    use tokio_tungstenite::WebSocketStream;

    use super::*;
    use crate::{
        acquire_development_host_lease, app_with_state, development_registry, host_access_registry,
        release_development_host_lease, spawn_development_host_lease_heartbeat, AppState,
        BuildWorkloadJobRegistry,
    };

    #[derive(Debug, Clone)]
    struct ObservedHttpRequest {
        path: String,
        query: Option<String>,
        authorization: Option<String>,
        cookie: Option<String>,
        bridge_credential: Option<String>,
        host: Option<String>,
        body: Vec<u8>,
    }

    #[derive(Debug, Clone)]
    struct ObservedWebSocketRequest {
        path: String,
        query: Option<String>,
        authorization: Option<String>,
        cookie: Option<String>,
        bridge_credential: Option<String>,
        origin: Option<String>,
        subprotocol: Option<String>,
        host: Option<String>,
    }

    #[derive(Debug, Default)]
    struct RemoteUpstreamObservations {
        http: Vec<ObservedHttpRequest>,
        websocket: Vec<ObservedWebSocketRequest>,
    }

    type SharedRemoteObservations = Arc<AsyncMutex<RemoteUpstreamObservations>>;

    fn header_text(
        headers: &HeaderMap,
        name: impl axum::http::header::AsHeaderName,
    ) -> Option<String> {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
    }

    async fn remote_http_upstream(
        State(observations): State<SharedRemoteObservations>,
        OriginalUri(uri): OriginalUri,
        headers: HeaderMap,
        body: Bytes,
    ) -> &'static str {
        observations.lock().await.http.push(ObservedHttpRequest {
            path: uri.path().to_string(),
            query: uri.query().map(str::to_string),
            authorization: header_text(&headers, header::AUTHORIZATION),
            cookie: header_text(&headers, header::COOKIE),
            bridge_credential: header_text(&headers, crate::TARGET_TUNNEL_BRIDGE_HEADER),
            host: header_text(&headers, header::HOST),
            body: body.to_vec(),
        });
        "remote http ok"
    }

    async fn remote_websocket_upstream(
        State(observations): State<SharedRemoteObservations>,
        ws: WebSocketUpgrade,
        OriginalUri(uri): OriginalUri,
        headers: HeaderMap,
    ) -> axum::response::Response {
        observations
            .lock()
            .await
            .websocket
            .push(ObservedWebSocketRequest {
                path: uri.path().to_string(),
                query: uri.query().map(str::to_string),
                authorization: header_text(&headers, header::AUTHORIZATION),
                cookie: header_text(&headers, header::COOKIE),
                bridge_credential: header_text(&headers, crate::TARGET_TUNNEL_BRIDGE_HEADER),
                origin: header_text(&headers, header::ORIGIN),
                subprotocol: header_text(&headers, header::SEC_WEBSOCKET_PROTOCOL),
                host: header_text(&headers, header::HOST),
            });
        ws.protocols(["plurora.test"])
            .on_upgrade(|mut socket| async move {
                if let Some(Ok(message)) = socket.recv().await {
                    let _ = socket.send(message).await;
                }
            })
    }

    #[derive(Debug, Clone)]
    struct ExpectedTunnelRoute {
        port_lease_id: String,
        port_name: String,
        port: u16,
    }

    enum TestAgentOutbound {
        Data { stream_id: String, data: Vec<u8> },
        Closed { stream_id: String },
    }

    async fn connect_test_target_tunnel(
        host_addr: SocketAddr,
        credential: &str,
        target: &ExecutionTarget,
        expected_routes: Arc<HashMap<String, ExpectedTunnelRoute>>,
    ) -> anyhow::Result<tokio::task::JoinHandle<anyhow::Result<()>>> {
        let mut request =
            format!("ws://{host_addr}/target-agent/v1/tunnel").into_client_request()?;
        request.headers_mut().insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("PluroraTarget {credential}"))?,
        );
        let stream = TcpStream::connect(host_addr).await?;
        let (socket, response) = tokio::time::timeout(
            Duration::from_secs(5),
            tokio_tungstenite::client_async(request, stream),
        )
        .await??;
        anyhow::ensure!(
            response.status() == StatusCode::SWITCHING_PROTOCOLS,
            "target tunnel did not switch protocols"
        );
        let target_id = target.id.clone();
        let lease_epoch = target.lease_epoch;
        let policy_epoch = target.policy_epoch;
        Ok(tokio::spawn(async move {
            run_test_target_tunnel(
                socket,
                target_id,
                lease_epoch,
                policy_epoch,
                expected_routes,
            )
            .await
        }))
    }

    async fn target_tunnel_handshake_status(
        host_addr: SocketAddr,
        credential: &str,
    ) -> anyhow::Result<StatusCode> {
        let mut request =
            format!("ws://{host_addr}/target-agent/v1/tunnel").into_client_request()?;
        request.headers_mut().insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("PluroraTarget {credential}"))?,
        );
        let stream = TcpStream::connect(host_addr).await?;
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            tokio_tungstenite::client_async(request, stream),
        )
        .await?;
        match result {
            Err(WebSocketError::Http(response)) => Ok(response.status()),
            Ok((mut socket, response)) => {
                let status = response.status();
                let _ = socket.close(None).await;
                Ok(status)
            }
            Err(error) => Err(error.into()),
        }
    }

    async fn run_test_target_tunnel(
        mut socket: WebSocketStream<TcpStream>,
        target_id: String,
        lease_epoch: u64,
        policy_epoch: u64,
        expected_routes: Arc<HashMap<String, ExpectedTunnelRoute>>,
    ) -> anyhow::Result<()> {
        let streams = Arc::new(AsyncMutex::new(
            HashMap::<String, mpsc::Sender<Vec<u8>>>::new(),
        ));
        let (outbound_tx, mut outbound_rx) = mpsc::channel::<TestAgentOutbound>(64);
        loop {
            tokio::select! {
                outbound = outbound_rx.recv() => {
                    let Some(outbound) = outbound else { break; };
                    match outbound {
                        TestAgentOutbound::Data { stream_id, data } => {
                            let frame = encode_target_tunnel_data(&stream_id, &data)
                                .context("test agent data frame is invalid")?;
                            socket.send(TunnelMessage::Binary(frame)).await?;
                        }
                        TestAgentOutbound::Closed { stream_id } => {
                            streams.lock().await.remove(&stream_id);
                            send_test_agent_control(
                                &mut socket,
                                TargetTunnelAgentMessage::Closed { stream_id },
                            )
                            .await?;
                        }
                    }
                }
                message = socket.next() => {
                    let Some(message) = message else { break; };
                    match message? {
                        TunnelMessage::Text(text) => {
                            let control: TargetTunnelHostMessage = serde_json::from_str(&text)?;
                            match control {
                                TargetTunnelHostMessage::Open { stream } => {
                                    let expected = expected_routes
                                        .get(&stream.route_id)
                                        .cloned()
                                        .context("Host opened an unknown test route")?;
                                    anyhow::ensure!(
                                        stream.target_id == target_id
                                            && stream.port_lease_id == expected.port_lease_id
                                            && stream.port_name == expected.port_name
                                            && stream.port == expected.port
                                            && stream.lease_epoch == lease_epoch
                                            && stream.policy_epoch == policy_epoch,
                                        "Host tunnel Open did not match the durable target lease"
                                    );
                                    let upstream = TcpStream::connect(("127.0.0.1", stream.port)).await;
                                    let Ok(upstream) = upstream else {
                                        send_test_agent_control(
                                            &mut socket,
                                            TargetTunnelAgentMessage::Rejected {
                                                stream_id: stream.stream_id,
                                            },
                                        )
                                        .await?;
                                        continue;
                                    };
                                    let (incoming_tx, incoming_rx) =
                                        mpsc::channel(STREAM_QUEUE_CAPACITY);
                                    streams
                                        .lock()
                                        .await
                                        .insert(stream.stream_id.clone(), incoming_tx);
                                    send_test_agent_control(
                                        &mut socket,
                                        TargetTunnelAgentMessage::Opened {
                                            stream_id: stream.stream_id.clone(),
                                        },
                                    )
                                    .await?;
                                    tokio::spawn(pump_test_target_stream(
                                        stream.stream_id,
                                        upstream,
                                        incoming_rx,
                                        outbound_tx.clone(),
                                    ));
                                }
                                TargetTunnelHostMessage::Close { stream_id } => {
                                    streams.lock().await.remove(&stream_id);
                                    send_test_agent_control(
                                        &mut socket,
                                        TargetTunnelAgentMessage::Closed { stream_id },
                                    )
                                    .await?;
                                }
                            }
                        }
                        TunnelMessage::Binary(frame) => {
                            let (stream_id, data) = decode_target_tunnel_data(&frame)
                                .context("Host sent an invalid test tunnel frame")?;
                            let incoming = streams.lock().await.get(&stream_id).cloned();
                            if let Some(incoming) = incoming {
                                if incoming.try_send(data.to_vec()).is_err() {
                                    streams.lock().await.remove(&stream_id);
                                    send_test_agent_control(
                                        &mut socket,
                                        TargetTunnelAgentMessage::Closed { stream_id },
                                    )
                                    .await?;
                                }
                            }
                        }
                        TunnelMessage::Ping(data) => {
                            socket.send(TunnelMessage::Pong(data)).await?;
                        }
                        TunnelMessage::Pong(_) => {}
                        TunnelMessage::Close(_) => break,
                        TunnelMessage::Frame(_) => {}
                    }
                }
            }
        }
        Ok(())
    }

    async fn send_test_agent_control(
        socket: &mut WebSocketStream<TcpStream>,
        control: TargetTunnelAgentMessage,
    ) -> anyhow::Result<()> {
        socket
            .send(TunnelMessage::Text(serde_json::to_string(&control)?.into()))
            .await?;
        Ok(())
    }

    async fn pump_test_target_stream(
        stream_id: String,
        upstream: TcpStream,
        mut incoming_rx: mpsc::Receiver<Vec<u8>>,
        outbound_tx: mpsc::Sender<TestAgentOutbound>,
    ) {
        let (mut reader, mut writer) = upstream.into_split();
        let mut buffer = vec![0u8; TARGET_TUNNEL_DATA_CHUNK_BYTES];
        loop {
            tokio::select! {
                read = reader.read(&mut buffer) => {
                    let Ok(read) = read else { break; };
                    if read == 0 { break; }
                    if outbound_tx
                        .send(TestAgentOutbound::Data {
                            stream_id: stream_id.clone(),
                            data: buffer[..read].to_vec(),
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                incoming = incoming_rx.recv() => {
                    let Some(incoming) = incoming else { break; };
                    if writer.write_all(&incoming).await.is_err() { break; }
                }
            }
        }
        let _ = writer.shutdown().await;
        let _ = outbound_tx
            .send(TestAgentOutbound::Closed { stream_id })
            .await;
    }

    async fn create_acceptance_installation(
        store: &Arc<InMemoryEventStore>,
        registry: &Arc<crate::InstallationRegistry>,
        objects: &Arc<plurora_runtime::InMemoryObjectStore>,
    ) -> anyhow::Result<InstallationId> {
        async fn put_model<T: ArtifactModel>(
            objects: &plurora_runtime::InMemoryObjectStore,
            value: &T,
        ) -> anyhow::Result<plurora_core::ArtifactDescriptor> {
            let bytes = value.canonical_bytes()?;
            let descriptor = value.artifact_descriptor()?;
            let info = objects.put(bytes.into()).await?;
            anyhow::ensure!(
                info.digest == descriptor.digest && info.size_bytes == descriptor.size_bytes,
                "test artifact descriptor does not match stored canonical bytes"
            );
            Ok(descriptor)
        }

        let assembly = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: AssemblyId::parse("tests/remote-tunnel-assembly")?,
            nodes: Vec::new(),
            bindings: Vec::new(),
            exposed_ports: Vec::new(),
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let assembly = put_model(objects, &assembly).await?;
        let work = WorkRevision {
            schema: WorkRevision::SCHEMA.to_string(),
            work_id: WorkId::parse("tests/remote-tunnel")?,
            title: "Remote tunnel acceptance".to_string(),
            description: String::new(),
            assembly: assembly.clone(),
            content_roots: Vec::new(),
            entrypoints: Vec::new(),
            rights: None,
            transparency: None,
            operational_intent: None,
            annotations: BTreeMap::new(),
        };
        let lock = AssemblyLock {
            schema: AssemblyLock::SCHEMA.to_string(),
            assembly,
            nodes: Vec::new(),
            bindings: Vec::new(),
            protocol_profiles: Vec::new(),
            content_roots: Vec::new(),
        };
        let runtime = Runtime::new(
            store.clone(),
            RuntimeConfig {
                object_store: objects.clone(),
                installation_control: registry.clone(),
                ..RuntimeConfig::default()
            },
        );
        let result: InstallationMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("remote-tunnel-installation-fixture"),
                    "host.installation.create",
                    serde_json::to_value(InstallationCreateRequest {
                        work_id: work.work_id.clone(),
                        work_revision: put_model(objects, &work).await?,
                        assembly_lock: put_model(objects, &lock).await?,
                        display_name: "Remote tunnel acceptance".to_string(),
                        source: AcquisitionRecord {
                            kind: AcquisitionKind::WorkBundle,
                            source_ref: None,
                            provenance_refs: Vec::new(),
                            update_channel: None,
                        },
                        state_bindings: Vec::new(),
                        secret_policy: Default::default(),
                        idempotency_key: "remote-tunnel-acceptance".to_string(),
                        authority: None,
                    })?,
                )
                .await
                .map_err(|error| anyhow::anyhow!("{}: {}", error.code, error.message))?,
        )?;
        Ok(result.installation.record.installation_id)
    }

    async fn apply_remote_test_workload(
        client: &reqwest::Client,
        base_url: &str,
        host_token: &str,
        credential: &str,
        target_id: &str,
        installation_id: &InstallationId,
        route_id: &str,
        port_lease_id: &str,
        port_name: &str,
        upstream_port: u16,
        execution_id: &str,
    ) -> anyhow::Result<()> {
        let created = client
            .post(format!("{base_url}/host/v1/targets/{target_id}/operations"))
            .bearer_auth(host_token)
            .json(&json!({
                "installation_id": installation_id,
                "spec": {
                    "kind": "workload_apply",
                    "workload": {
                        "workload": {
                            "workload_id": format!("workload-{route_id}"),
                            "route_id": route_id,
                            "port_lease_id": port_lease_id
                        },
                        "port_name": port_name,
                        "image": "example/remote-acceptance:latest",
                        "container_port": 8080,
                        "requested_host_port": upstream_port,
                        "pull_if_missing": false
                    }
                },
                "idempotency_key": format!("acceptance-{route_id}"),
                "expires_in_seconds": 60
            }))
            .send()
            .await?;
        anyhow::ensure!(
            created.status() == StatusCode::CREATED,
            "workload operation creation failed: {}",
            created.text().await.unwrap_or_default()
        );
        let created: Value = created.json().await?;
        let operation: TargetOperationRecord =
            serde_json::from_value(created["operation"].clone())?;

        let next = client
            .get(format!("{base_url}/target-agent/v1/operations/next"))
            .header(header::AUTHORIZATION, format!("PluroraTarget {credential}"))
            .send()
            .await?;
        anyhow::ensure!(next.status() == StatusCode::OK, "agent work poll failed");
        let queued: NextTargetOperationResponse = next.json().await?;
        anyhow::ensure!(
            queued
                .operation
                .as_ref()
                .is_some_and(|queued| queued.operation_id == operation.operation_id),
            "agent did not receive the created workload operation"
        );

        for status in [
            TargetOperationStatusKind::Accepted,
            TargetOperationStatusKind::Running,
        ] {
            let progress = client
                .post(format!(
                    "{base_url}/target-agent/v1/operations/{}/progress",
                    operation.operation_id
                ))
                .header(header::AUTHORIZATION, format!("PluroraTarget {credential}"))
                .json(&TargetOperationProgressRequest {
                    request_digest: operation.authority.request_digest.clone(),
                    authority_digest: operation.authority.authority_digest.clone(),
                    execution_id: execution_id.to_string(),
                    status,
                })
                .send()
                .await?;
            anyhow::ensure!(
                progress.status() == StatusCode::OK,
                "agent progress transition failed: {}",
                progress.text().await.unwrap_or_default()
            );
        }

        let completed = client
            .post(format!(
                "{base_url}/target-agent/v1/operations/{}/receipt",
                operation.operation_id
            ))
            .header(header::AUTHORIZATION, format!("PluroraTarget {credential}"))
            .json(&TargetOperationReceipt {
                operation_id: operation.operation_id,
                target_id: target_id.to_string(),
                execution_id: execution_id.to_string(),
                step_id: "execute".to_string(),
                request_digest: operation.authority.request_digest,
                authority_digest: operation.authority.authority_digest,
                status: TargetOperationReceiptStatus::Succeeded,
                completed_at_ms: Utc::now().timestamp_millis(),
                output: json!({
                    "bind_host": "127.0.0.1",
                    "host_port": upstream_port,
                    "running": true
                }),
                diagnostics: Vec::new(),
            })
            .send()
            .await?;
        anyhow::ensure!(
            completed.status() == StatusCode::OK,
            "agent receipt failed: {}",
            completed.text().await.unwrap_or_default()
        );
        let completed: TargetOperationRecord = completed.json().await?;
        anyhow::ensure!(
            completed.status == TargetOperationStatusKind::Succeeded,
            "workload operation did not succeed"
        );
        Ok(())
    }

    async fn wait_for_tunnel_state(
        state: &AppState<InMemoryEventStore>,
        target_id: &str,
        expected: bool,
    ) -> anyhow::Result<()> {
        for _ in 0..200 {
            if state.target_agents.tunnel_connected(target_id) == expected
                && (expected || !state.target_agents.tunnels.claimed(target_id))
            {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        anyhow::bail!("target tunnel did not reach connected={expected}")
    }

    async fn wait_for_route_readiness(
        state: &AppState<InMemoryEventStore>,
        route_ids: &[&str],
        expected: bool,
    ) -> anyhow::Result<()> {
        for _ in 0..200 {
            let mut matching = true;
            for route_id in route_ids {
                matching &= state
                    .runtime
                    .config()
                    .proxy_route_registry
                    .status(route_id)
                    .await
                    .is_some_and(|route| route.ready == expected);
            }
            if matching {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        anyhow::bail!("proxy routes did not reach ready={expected}")
    }

    #[tokio::test]
    async fn remote_tunnel_acceptance_covers_http_websocket_reconnect_and_revoke(
    ) -> anyhow::Result<()> {
        tokio::time::timeout(Duration::from_secs(45), remote_tunnel_acceptance())
            .await
            .map_err(|_| anyhow::anyhow!("remote tunnel acceptance timed out"))??;
        Ok(())
    }

    async fn remote_tunnel_acceptance() -> anyhow::Result<()> {
        const TARGET_ID: &str = "remote-ci";
        const HTTP_ROUTE: &str = "remote-http";
        const WS_ROUTE: &str = "remote-ws";
        const HOST_TOKEN: &str = "target-agent-host-token";

        let observations = Arc::new(AsyncMutex::new(RemoteUpstreamObservations::default()));
        let upstream = Router::new()
            .route("/ws/*path", any(remote_websocket_upstream))
            .fallback(any(remote_http_upstream))
            .with_state(observations.clone());
        let upstream_listener = TcpListener::bind("127.0.0.1:0").await?;
        let upstream_addr = upstream_listener.local_addr()?;
        let upstream_server = tokio::spawn(async move {
            axum::serve(upstream_listener, upstream)
                .await
                .expect("remote acceptance upstream serves");
        });

        let store = Arc::new(InMemoryEventStore::default());
        let objects = Arc::new(plurora_runtime::InMemoryObjectStore::default());
        let development = development_registry();
        let lease = acquire_development_host_lease(store.clone(), development.clone()).await?;
        lease.ensure_active()?;
        let heartbeat = spawn_development_host_lease_heartbeat(store.clone(), lease.clone());
        let installations = crate::InstallationRegistry::ephemeral(store.clone(), objects.clone())?;
        installations.install_owner_lease(lease.clone())?;
        let installation_id =
            create_acceptance_installation(&store, &installations, &objects).await?;
        let runtime = Arc::new(Runtime::new(
            store.clone(),
            RuntimeConfig {
                object_store: objects,
                installation_control: installations.clone(),
                ..RuntimeConfig::default()
            },
        ));
        let http_lease = runtime
            .config()
            .port_lease_registry
            .lease(PortLeaseRequest {
                target_id: ExecutionTargetId::from(TARGET_ID),
                port_name: "http".to_string(),
                protocol: PortProtocol::Tcp,
                requested_port: Some(upstream_addr.port()),
            })
            .await
            .lease;
        let ws_lease = runtime
            .config()
            .port_lease_registry
            .lease(PortLeaseRequest {
                target_id: ExecutionTargetId::from(TARGET_ID),
                port_name: "ws".to_string(),
                protocol: PortProtocol::Tcp,
                requested_port: Some(upstream_addr.port()),
            })
            .await
            .lease;
        runtime
            .config()
            .proxy_route_registry
            .register(ProxyRouteRegisterRequest {
                route_id: Some(HTTP_ROUTE.to_string()),
                upstream: ProxyRouteUpstream {
                    port_lease_id: http_lease.id.clone(),
                    port_name: http_lease.port_name.clone(),
                },
                protocol: ProxyProtocol::Http,
                access: ProxyRouteAccess::Public,
            })
            .await;
        runtime
            .config()
            .proxy_route_registry
            .register(ProxyRouteRegisterRequest {
                route_id: Some(WS_ROUTE.to_string()),
                upstream: ProxyRouteUpstream {
                    port_lease_id: ws_lease.id.clone(),
                    port_name: ws_lease.port_name.clone(),
                },
                protocol: ProxyProtocol::Websocket,
                access: ProxyRouteAccess::HostAuthenticated,
            })
            .await;

        let state = AppState {
            runtime,
            static_dir: None,
            access_token: Some(HOST_TOKEN.to_string()),
            app_base_domain: Some("apps.example.test".to_string()),
            build_jobs: Arc::new(BuildWorkloadJobRegistry::default()),
            development,
            host_access: host_access_registry(),
            installations,
            target_agents: target_agent_registry(),
        };
        let host_listener = TcpListener::bind("127.0.0.1:0").await?;
        let host_addr = host_listener.local_addr()?;
        let host_app = app_with_state(state.clone());
        let host_server = tokio::spawn(async move {
            axum::serve(host_listener, host_app)
                .await
                .expect("remote acceptance Host serves");
        });
        let base_url = format!("http://{host_addr}");
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;

        let enrollment = client
            .post(format!(
                "{base_url}/host/v1/targets/{TARGET_ID}/enrollments"
            ))
            .bearer_auth(HOST_TOKEN)
            .json(&json!({
                "display_name": "CI remote workload target",
                "reachability": "reverse_tunnel",
                "allowed_capabilities": ["workload"],
                "labels": {},
                "expires_in_seconds": 60
            }))
            .send()
            .await?;
        anyhow::ensure!(
            enrollment.status() == StatusCode::CREATED,
            "remote enrollment failed: {}",
            enrollment.text().await.unwrap_or_default()
        );
        let enrollment: Value = enrollment.json().await?;
        let enrollment_token = enrollment["enrollment_token"]
            .as_str()
            .context("enrollment response omitted its token")?;
        let claim = client
            .post(format!("{base_url}/target-agent/v1/enroll"))
            .json(&ClaimTargetEnrollmentRequest {
                enrollment_token: enrollment_token.to_string(),
                protocol_versions: vec!["target-agent.v1".to_string()],
                declared_capabilities: vec![ExecutionTargetCapability::Workload],
            })
            .send()
            .await?;
        anyhow::ensure!(
            claim.status() == StatusCode::OK,
            "remote enrollment claim failed: {}",
            claim.text().await.unwrap_or_default()
        );
        let claim: ClaimTargetEnrollmentResponse = claim.json().await?;

        apply_remote_test_workload(
            &client,
            &base_url,
            HOST_TOKEN,
            &claim.agent_credential,
            TARGET_ID,
            &installation_id,
            HTTP_ROUTE,
            &http_lease.id,
            &http_lease.port_name,
            upstream_addr.port(),
            &"1".repeat(32),
        )
        .await?;
        apply_remote_test_workload(
            &client,
            &base_url,
            HOST_TOKEN,
            &claim.agent_credential,
            TARGET_ID,
            &installation_id,
            WS_ROUTE,
            &ws_lease.id,
            &ws_lease.port_name,
            upstream_addr.port(),
            &"2".repeat(32),
        )
        .await?;
        wait_for_route_readiness(&state, &[HTTP_ROUTE, WS_ROUTE], true).await?;

        let no_tunnel = client
            .get(format!("{base_url}/p/{HTTP_ROUTE}/before-connect"))
            .bearer_auth(HOST_TOKEN)
            .send()
            .await?;
        assert_eq!(no_tunnel.status(), StatusCode::SERVICE_UNAVAILABLE);

        let expected_routes = Arc::new(HashMap::from([
            (
                HTTP_ROUTE.to_string(),
                ExpectedTunnelRoute {
                    port_lease_id: http_lease.id.clone(),
                    port_name: http_lease.port_name.clone(),
                    port: upstream_addr.port(),
                },
            ),
            (
                WS_ROUTE.to_string(),
                ExpectedTunnelRoute {
                    port_lease_id: ws_lease.id.clone(),
                    port_name: ws_lease.port_name.clone(),
                    port: upstream_addr.port(),
                },
            ),
        ]));
        let tunnel = connect_test_target_tunnel(
            host_addr,
            &claim.agent_credential,
            &claim.target,
            expected_routes.clone(),
        )
        .await?;
        assert_eq!(
            target_tunnel_handshake_status(host_addr, &claim.agent_credential).await?,
            StatusCode::CONFLICT
        );
        wait_for_tunnel_state(&state, TARGET_ID, true).await?;

        let proxied_http = client
            .post(format!(
                "{base_url}/p/{HTTP_ROUTE}/api/echo?keep=1&access_token={HOST_TOKEN}"
            ))
            .bearer_auth(HOST_TOKEN)
            .header(header::COOKIE, "session=host-secret")
            .body("through remote tunnel")
            .send()
            .await?;
        assert_eq!(proxied_http.status(), StatusCode::OK);
        assert_eq!(proxied_http.text().await?, "remote http ok");
        let observed_http = observations
            .lock()
            .await
            .http
            .last()
            .cloned()
            .context("remote HTTP upstream was not reached")?;
        assert_eq!(observed_http.path, "/api/echo");
        assert_eq!(observed_http.query.as_deref(), Some("keep=1"));
        assert!(observed_http.authorization.is_none());
        assert!(observed_http.cookie.is_none());
        assert!(observed_http.bridge_credential.is_none());
        assert_eq!(
            observed_http.host.as_deref(),
            Some(format!("127.0.0.1:{}", upstream_addr.port()).as_str())
        );
        assert_eq!(observed_http.body, b"through remote tunnel");

        let public_vhost = format!("{}.apps.example.test", crate::route_slug(HTTP_ROUTE));
        let vhost_response = client
            .get(format!("{base_url}/public?via=vhost"))
            .header(header::HOST, &public_vhost)
            .send()
            .await?;
        assert_eq!(vhost_response.status(), StatusCode::OK);
        assert_eq!(vhost_response.text().await?, "remote http ok");
        let observed_vhost = observations
            .lock()
            .await
            .http
            .last()
            .cloned()
            .context("public vhost did not reach the remote upstream")?;
        assert_eq!(observed_vhost.path, "/public");
        assert_eq!(observed_vhost.query.as_deref(), Some("via=vhost"));
        assert_eq!(observed_vhost.host.as_deref(), Some(public_vhost.as_str()));
        assert!(observed_vhost.authorization.is_none());
        assert!(observed_vhost.cookie.is_none());
        assert!(observed_vhost.bridge_credential.is_none());

        let mut ws_request =
            format!("ws://{host_addr}/p/{WS_ROUTE}/ws/echo?keep=1&access_token={HOST_TOKEN}")
                .into_client_request()?;
        ws_request.headers_mut().insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer target-agent-host-token"),
        );
        ws_request.headers_mut().insert(
            header::COOKIE,
            HeaderValue::from_static("session=host-secret"),
        );
        ws_request.headers_mut().insert(
            header::ORIGIN,
            HeaderValue::from_static("https://client.example"),
        );
        ws_request.headers_mut().insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("plurora.test, plurora.fallback"),
        );
        let ws_stream = TcpStream::connect(host_addr).await?;
        let (mut websocket, ws_response) = tokio::time::timeout(
            Duration::from_secs(5),
            tokio_tungstenite::client_async(ws_request, ws_stream),
        )
        .await??;
        assert_eq!(
            ws_response
                .headers()
                .get(header::SEC_WEBSOCKET_PROTOCOL)
                .and_then(|value| value.to_str().ok()),
            Some("plurora.test")
        );
        websocket
            .send(TunnelMessage::Text("hello remote ws".into()))
            .await?;
        let echoed = tokio::time::timeout(Duration::from_secs(5), websocket.next())
            .await
            .map_err(|_| anyhow::anyhow!("remote websocket echo timed out"))?
            .context("remote websocket ended before echo")??;
        match echoed {
            TunnelMessage::Text(text) => assert_eq!(text, "hello remote ws"),
            other => panic!("unexpected remote websocket response: {other:?}"),
        }
        websocket.send(TunnelMessage::Close(None)).await?;
        let observed_ws = observations
            .lock()
            .await
            .websocket
            .last()
            .cloned()
            .context("remote WebSocket upstream was not reached")?;
        assert_eq!(observed_ws.path, "/ws/echo");
        assert_eq!(observed_ws.query.as_deref(), Some("keep=1"));
        assert!(observed_ws.authorization.is_none());
        assert!(observed_ws.cookie.is_none());
        assert!(observed_ws.bridge_credential.is_none());
        assert_eq!(
            observed_ws.origin.as_deref(),
            Some("https://client.example")
        );
        assert_eq!(
            observed_ws.subprotocol.as_deref(),
            Some("plurora.test, plurora.fallback")
        );
        assert_eq!(
            observed_ws.host.as_deref(),
            Some(format!("127.0.0.1:{}", upstream_addr.port()).as_str())
        );

        tunnel.abort();
        let _ = tunnel.await;
        wait_for_tunnel_state(&state, TARGET_ID, false).await?;
        wait_for_route_readiness(&state, &[HTTP_ROUTE, WS_ROUTE], false).await?;
        let disconnected = client
            .get(format!("{base_url}/p/{HTTP_ROUTE}/after-disconnect"))
            .bearer_auth(HOST_TOKEN)
            .send()
            .await?;
        assert_eq!(disconnected.status(), StatusCode::SERVICE_UNAVAILABLE);

        let reconnected = connect_test_target_tunnel(
            host_addr,
            &claim.agent_credential,
            &claim.target,
            expected_routes,
        )
        .await?;
        wait_for_tunnel_state(&state, TARGET_ID, true).await?;
        wait_for_route_readiness(&state, &[HTTP_ROUTE, WS_ROUTE], true).await?;
        let after_reconnect = client
            .get(format!("{base_url}/p/{HTTP_ROUTE}/after-reconnect"))
            .bearer_auth(HOST_TOKEN)
            .send()
            .await?;
        assert_eq!(after_reconnect.status(), StatusCode::OK);

        let revoked = client
            .post(format!("{base_url}/host/v1/targets/{TARGET_ID}/revoke"))
            .bearer_auth(HOST_TOKEN)
            .send()
            .await?;
        assert_eq!(revoked.status(), StatusCode::OK);
        wait_for_tunnel_state(&state, TARGET_ID, false).await?;
        wait_for_route_readiness(&state, &[HTTP_ROUTE, WS_ROUTE], false).await?;

        assert_eq!(
            target_tunnel_handshake_status(host_addr, &claim.agent_credential).await?,
            StatusCode::UNAUTHORIZED
        );

        reconnected.abort();
        let _ = reconnected.await;
        host_server.abort();
        upstream_server.abort();
        heartbeat.abort();
        release_development_host_lease(store, &lease).await?;
        Ok(())
    }

    #[test]
    fn tunnel_binary_frames_are_bounded_and_stream_scoped() {
        let stream_id = "a".repeat(STREAM_ID_BYTES);
        let data = b"hello tunnel";
        let frame = encode_target_tunnel_data(&stream_id, data).expect("valid frame");
        let (decoded_stream, decoded_data) =
            decode_target_tunnel_data(&frame).expect("frame decodes");
        assert_eq!(decoded_stream, stream_id);
        assert_eq!(decoded_data, data);
        assert!(encode_target_tunnel_data("not-an-id", data).is_none());
        assert!(encode_target_tunnel_data(
            &stream_id,
            &vec![0; TARGET_TUNNEL_DATA_CHUNK_BYTES + 1]
        )
        .is_none());
    }

    #[test]
    fn tunnel_control_messages_reject_unknown_fields() {
        assert!(serde_json::from_value::<TargetTunnelHostMessage>(json!({
            "kind": "close",
            "stream_id": "a".repeat(STREAM_ID_BYTES),
            "port": 22
        }))
        .is_err());
    }

    #[test]
    fn one_live_connection_owns_each_target_tunnel() {
        let registry = TargetTunnelRegistry::default();
        let registration = registry
            .register("remote-1")
            .expect("first tunnel owns target");
        let connection_id = registration.connection_id().to_string();
        assert!(registry.claimed("remote-1"));
        assert!(!registry.connected("remote-1"));
        assert!(registry.register("remote-1").is_err());
        assert!(registry.activate("remote-1", &connection_id));
        assert!(registry.connected("remote-1"));
        assert!(registry.is_current("remote-1", &connection_id));
        assert!(registry.register("remote-1").is_err());
        registry.remove("remote-1", "different-connection");
        assert!(registry.connected("remote-1"));
        registry.remove("remote-1", &connection_id);
        assert!(!registry.connected("remote-1"));
        assert!(!registry.is_current("remote-1", &connection_id));
    }

    #[test]
    fn closing_generation_blocks_reconnect_until_route_cleanup_finishes() {
        let registry = TargetTunnelRegistry::default();
        let registration = registry
            .register("remote-1")
            .expect("first tunnel owns target");
        let connection_id = registration.connection_id().to_string();
        assert!(registry.activate("remote-1", &connection_id));

        registry.disconnect("remote-1");

        assert!(!registry.connected("remote-1"));
        assert!(registry.claimed("remote-1"));
        assert!(!registry.is_current("remote-1", &connection_id));
        assert!(registry.register("remote-1").is_err());

        registry.remove("remote-1", &connection_id);
        assert!(!registry.claimed("remote-1"));
        assert!(registry.register("remote-1").is_ok());
    }

    #[test]
    fn pending_generation_cannot_activate_after_disconnect() {
        let registry = TargetTunnelRegistry::default();
        let registration = registry
            .register("remote-1")
            .expect("pending tunnel reserves target");
        let connection_id = registration.connection_id().to_string();

        registry.disconnect("remote-1");

        assert!(!registry.activate("remote-1", &connection_id));
        assert!(!registry.connected("remote-1"));
        assert!(registry.claimed("remote-1"));
        registry.remove("remote-1", &connection_id);
        assert!(!registry.claimed("remote-1"));
    }

    #[tokio::test]
    async fn stream_cleanup_is_generation_scoped() {
        let stream_id = "e".repeat(STREAM_ID_BYTES);
        let old_reservation = Arc::new(());
        let current_reservation = Arc::new(());
        let (incoming_tx, _incoming_rx) = mpsc::channel(1);
        let streams: HostStreams = Arc::new(AsyncMutex::new(HashMap::from([(
            stream_id.clone(),
            HostStreamState {
                reservation: current_reservation.clone(),
                incoming_tx,
                opened_tx: None,
            },
        )])));

        assert!(remove_host_stream(&streams, &stream_id, &old_reservation)
            .await
            .is_none());
        assert!(streams.lock().await.contains_key(&stream_id));
        assert!(
            remove_host_stream(&streams, &stream_id, &current_reservation)
                .await
                .is_some()
        );
    }

    #[tokio::test]
    async fn late_stream_control_is_idempotent() {
        let streams: HostStreams = Arc::new(AsyncMutex::new(HashMap::new()));
        let (outbound_tx, _outbound_rx) = mpsc::channel(1);
        for control in [
            TargetTunnelAgentMessage::Opened {
                stream_id: "a".repeat(STREAM_ID_BYTES),
            },
            TargetTunnelAgentMessage::Rejected {
                stream_id: "b".repeat(STREAM_ID_BYTES),
            },
            TargetTunnelAgentMessage::Closed {
                stream_id: "c".repeat(STREAM_ID_BYTES),
            },
        ] {
            let message = Message::Text(serde_json::to_string(&control).expect("control encodes"));
            assert!(handle_agent_message(message, &streams, &outbound_tx).await);
        }
    }

    #[tokio::test]
    async fn slow_stream_is_closed_without_blocking_the_tunnel_loop() {
        let stream_id = "d".repeat(STREAM_ID_BYTES);
        let (incoming_tx, _incoming_rx) = mpsc::channel(1);
        incoming_tx
            .try_send(vec![1])
            .expect("stream queue is primed");
        let streams: HostStreams = Arc::new(AsyncMutex::new(HashMap::from([(
            stream_id.clone(),
            HostStreamState {
                reservation: Arc::new(()),
                incoming_tx,
                opened_tx: None,
            },
        )])));
        let (outbound_tx, mut outbound_rx) = mpsc::channel(1);
        let frame = encode_target_tunnel_data(&stream_id, b"overflow").expect("frame encodes");

        let handled = tokio::time::timeout(
            Duration::from_millis(100),
            handle_agent_message(Message::Binary(frame), &streams, &outbound_tx),
        )
        .await
        .expect("backpressure handling stays non-blocking");
        assert!(handled);
        assert!(!streams.lock().await.contains_key(&stream_id));
        let Message::Text(close) = outbound_rx.try_recv().expect("close is queued") else {
            panic!("expected close control");
        };
        assert_eq!(
            serde_json::from_str::<TargetTunnelHostMessage>(&close).expect("close decodes"),
            TargetTunnelHostMessage::Close { stream_id }
        );
    }
}
