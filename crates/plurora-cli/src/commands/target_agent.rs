use std::collections::HashMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use chrono::Utc;
use fs2::FileExt;
use futures::{SinkExt, StreamExt};
use plurora_core::{EventEnvelope, EventSequence};
use plurora_runtime::{
    EventStore, ExecutionTargetCapability, ExecutionTargetObservedSummary,
    ExecutionTargetReachability, SqliteEventStore,
};
use plurora_service::{
    decode_target_tunnel_data, encode_target_tunnel_data, verify_target_operation_authority,
    ClaimTargetEnrollmentRequest, ClaimTargetEnrollmentResponse, DeclarativeVerifierDescriptor,
    NextTargetOperationResponse, TargetAgentHeartbeatRequest, TargetAgentHeartbeatResponse,
    TargetDeploymentRef, TargetOperationProgressRequest, TargetOperationReceipt,
    TargetOperationReceiptStatus, TargetOperationRecord, TargetOperationSpec,
    TargetOperationStatusKind, TargetTunnelAgentMessage, TargetTunnelHostMessage, TargetTunnelOpen,
    TARGET_TUNNEL_DATA_CHUNK_BYTES, TARGET_TUNNEL_MAX_STREAMS,
};
use plurora_work::InstallationId;
use reqwest::redirect::Policy;
use reqwest::{Client, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, watch, Mutex as AsyncMutex};
use tokio::time::{sleep, Instant};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message as TunnelMessage;

use super::host_access::{host_url, normalize_host_endpoint};

const CONFIG_FILE: &str = "agent.json";
const LEDGER_FILE: &str = "ledger.sqlite3";
const LEDGER_SESSION: &str = "target_agent_local_operations";
const LEDGER_EVENT: &str = "target-agent/local/v1/operation.snapshot";
const LEDGER_WRITER: &str = "target-agent/native";
const PROTOCOL_VERSION: &str = "target-agent.v1";
const MAX_JSON_RESPONSE_BYTES: usize = 1024 * 1024;
const POLL_INTERVAL: Duration = Duration::from_secs(1);
const MAX_RETRY_BACKOFF: Duration = Duration::from_secs(30);
const TARGET_TUNNEL_CONTROL_QUEUE_CAPACITY: usize = 64;
const TARGET_TUNNEL_DATA_QUEUE_CAPACITY: usize = 64;
const TARGET_TUNNEL_STREAM_QUEUE_CAPACITY: usize = 4;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct AgentConfig {
    schema_version: u32,
    endpoint: String,
    target_id: String,
    protocol_version: String,
    lease_epoch: u64,
    policy_epoch: u64,
    #[serde(default = "default_agent_reachability")]
    reachability: ExecutionTargetReachability,
    capabilities: Vec<ExecutionTargetCapability>,
    heartbeat_interval_seconds: u64,
}

fn default_agent_reachability() -> ExecutionTargetReachability {
    ExecutionTargetReachability::ReverseTunnel
}

pub async fn enroll(
    endpoint: &str,
    enrollment_token: &str,
    data_dir: PathBuf,
    capabilities: Vec<String>,
) -> anyhow::Result<()> {
    let capabilities = parse_capabilities(capabilities)?;
    let endpoint = normalize_endpoint(endpoint)?;
    let client = hardened_client()?;
    let response: ClaimTargetEnrollmentResponse = request_json(
        &client,
        &endpoint,
        None,
        Method::POST,
        "/target-agent/v1/enroll",
        Some(&ClaimTargetEnrollmentRequest {
            enrollment_token: enrollment_token.to_string(),
            protocol_versions: vec![PROTOCOL_VERSION.to_string()],
            declared_capabilities: capabilities.clone(),
        }),
    )
    .await?;
    anyhow::ensure!(
        response.selected_protocol_version == PROTOCOL_VERSION
            && response.target.selected_protocol_version.as_deref() == Some(PROTOCOL_VERSION)
            && response.target.lease_epoch > 0
            && response.target.policy_epoch > 0
            && credential_target_id(&response.agent_credential)
                == Some(response.target.id.as_str()),
        "Host returned an invalid target enrollment response"
    );

    let config = AgentConfig {
        schema_version: 1,
        endpoint,
        target_id: response.target.id.clone(),
        protocol_version: response.selected_protocol_version.clone(),
        lease_epoch: response.target.lease_epoch,
        policy_epoch: response.target.policy_epoch,
        reachability: response.target.reachability,
        capabilities: response.target.capabilities.clone(),
        heartbeat_interval_seconds: response.heartbeat_interval_seconds.max(1),
    };

    // The Host returns this credential once. Print it before filesystem work so
    // a config-write failure cannot consume enrollment without revealing it.
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "target_id": response.target.id,
            "agent_credential": response.agent_credential,
            "credential_persisted": false,
            "credential_env": "PLURORA_TARGET_AGENT_CREDENTIAL"
        }))?
    );
    std::io::stdout().flush()?;
    save_config(&data_dir, &config)
        .context("credential was printed, but non-secret agent configuration could not be saved")?;
    Ok(())
}

fn parse_capabilities(values: Vec<String>) -> anyhow::Result<Vec<ExecutionTargetCapability>> {
    let mut capabilities = values
        .into_iter()
        .map(|value| match value.as_str() {
            "artifact_transfer" => Ok(ExecutionTargetCapability::ArtifactTransfer),
            "declarative_verifier" => Ok(ExecutionTargetCapability::DeclarativeVerifier),
            "health_probe" => Ok(ExecutionTargetCapability::HealthProbe),
            "deployment" => Ok(ExecutionTargetCapability::Deployment),
            _ => anyhow::bail!(
                "native agent capability '{value}' is unsupported; use artifact_transfer, declarative_verifier, health_probe, or deployment"
            ),
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    capabilities.sort();
    capabilities.dedup();
    anyhow::ensure!(
        !capabilities.is_empty(),
        "at least one capability is required"
    );
    Ok(capabilities)
}

fn normalize_endpoint(endpoint: &str) -> anyhow::Result<String> {
    normalize_host_endpoint(endpoint)
}

fn hardened_client() -> anyhow::Result<Client> {
    Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10 * 60))
        .build()
        .context("failed to build target agent HTTP client")
}

async fn request_json<T, R>(
    client: &Client,
    endpoint: &str,
    credential: Option<&str>,
    method: Method,
    path: &str,
    body: Option<&T>,
) -> anyhow::Result<R>
where
    T: Serialize + ?Sized,
    R: DeserializeOwned,
{
    let url = host_url(endpoint, path)?;
    let mut request = client
        .request(method, url)
        .header("accept", "application/json");
    if let Some(credential) = credential {
        request = request.header("authorization", format!("PluroraTarget {credential}"));
    }
    if let Some(body) = body {
        request = request.json(body);
    }
    let response = request
        .send()
        .await
        .context("target agent Host request failed")?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .context("failed to read target agent Host response")?;
    anyhow::ensure!(
        bytes.len() <= MAX_JSON_RESPONSE_BYTES,
        "target agent Host response exceeded its size limit"
    );
    anyhow::ensure!(
        status.is_success(),
        "target agent Host request returned {status}; response details redacted"
    );
    serde_json::from_slice(&bytes).context("Host returned invalid target agent JSON")
}

fn prepare_data_dir(path: &Path) -> anyhow::Result<PathBuf> {
    if path.exists() {
        let metadata = std::fs::symlink_metadata(path)?;
        anyhow::ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "target agent data directory must be a real directory"
        );
    } else {
        std::fs::create_dir_all(path)?;
    }
    std::fs::canonicalize(path).context("failed to canonicalize target agent data directory")
}

fn save_config(data_dir: &Path, config: &AgentConfig) -> anyhow::Result<()> {
    let data_dir = prepare_data_dir(data_dir)?;
    let path = data_dir.join(CONFIG_FILE);
    if path.exists() {
        let metadata = std::fs::symlink_metadata(&path)?;
        anyhow::ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "target agent config path must be a regular file"
        );
    }
    let mut temporary = tempfile::NamedTempFile::new_in(&data_dir)?;
    serde_json::to_writer_pretty(&mut temporary, config)?;
    temporary.write_all(b"\n")?;
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(&path)
        .map_err(|error| error.error)
        .context("failed to atomically persist target agent config")?;
    sync_directory(&data_dir)?;
    Ok(())
}

fn load_config(data_dir: &Path) -> anyhow::Result<(PathBuf, AgentConfig)> {
    let data_dir = prepare_data_dir(data_dir)?;
    let path = data_dir.join(CONFIG_FILE);
    let metadata = std::fs::symlink_metadata(&path)
        .context("target agent config is missing; run `plurora target-agent enroll` first")?;
    anyhow::ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "target agent config must be a regular file"
    );
    let config: AgentConfig = serde_json::from_slice(&std::fs::read(&path)?)?;
    anyhow::ensure!(
        config.schema_version == 1
            && config.protocol_version == PROTOCOL_VERSION
            && config.lease_epoch > 0
            && config.policy_epoch > 0
            && config.reachability != ExecutionTargetReachability::LocalHost,
        "target agent config is incompatible"
    );
    Ok((data_dir, config))
}

fn sync_directory(_path: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    std::fs::File::open(_path)?.sync_all()?;
    Ok(())
}

fn credential_target_id(credential: &str) -> Option<&str> {
    let remainder = credential.strip_prefix("plurora_agent.")?;
    let (target_id, secret) = remainder.rsplit_once('.')?;
    (!target_id.is_empty()
        && secret.len() == 64
        && secret.bytes().all(|byte| byte.is_ascii_hexdigit()))
    .then_some(target_id)
}

fn encoded_path_segment(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
struct LocalOperationSnapshot {
    revision: u64,
    operation_id: String,
    target_id: String,
    installation_id: InstallationId,
    request_digest: String,
    authority_digest: String,
    execution_id: String,
    status: TargetOperationStatusKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    receipt: Option<TargetOperationReceipt>,
    updated_at_ms: i64,
}

#[derive(Default)]
struct LedgerProjection {
    next_sequence: EventSequence,
    operations: HashMap<String, LocalOperationSnapshot>,
}

struct LocalOperationLedger {
    store: Arc<SqliteEventStore>,
}

impl LocalOperationLedger {
    fn open(data_dir: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            store: Arc::new(SqliteEventStore::open(data_dir.join(LEDGER_FILE))?),
        })
    }

    async fn load(&self) -> anyhow::Result<LedgerProjection> {
        let mut projection = LedgerProjection::default();
        loop {
            let events = self
                .store
                .list_session_range(
                    &LEDGER_SESSION.to_string(),
                    projection.next_sequence.checked_sub(1),
                    Some(1_000),
                )
                .await?;
            if events.is_empty() {
                break;
            }
            for event in &events {
                apply_local_ledger_event(&mut projection, event)?;
            }
            if events.len() < 1_000 {
                break;
            }
        }
        Ok(projection)
    }

    async fn accept(
        &self,
        operation: &TargetOperationRecord,
    ) -> anyhow::Result<LocalOperationSnapshot> {
        for _ in 0..8 {
            let projection = self.load().await?;
            if let Some(existing) = projection.operations.get(&operation.operation_id) {
                validate_local_binding(existing, operation)?;
                return Ok(existing.clone());
            }
            let accepted = LocalOperationSnapshot {
                revision: 1,
                operation_id: operation.operation_id.clone(),
                target_id: operation.target_id.clone(),
                installation_id: operation.installation_id.clone(),
                request_digest: operation.authority.request_digest.clone(),
                authority_digest: operation.authority.authority_digest.clone(),
                execution_id: uuid::Uuid::new_v4().simple().to_string(),
                status: TargetOperationStatusKind::Accepted,
                receipt: None,
                updated_at_ms: Utc::now().timestamp_millis(),
            };
            if self
                .append(projection.next_sequence, &accepted)
                .await?
                .is_some()
            {
                return Ok(accepted);
            }
        }
        anyhow::bail!("local operation ledger changed too frequently while accepting work")
    }

    async fn mark_running(
        &self,
        operation: &TargetOperationRecord,
    ) -> anyhow::Result<LocalOperationSnapshot> {
        for _ in 0..8 {
            let projection = self.load().await?;
            let existing = projection
                .operations
                .get(&operation.operation_id)
                .context("operation was not durably accepted")?;
            validate_local_binding(existing, operation)?;
            if existing.status != TargetOperationStatusKind::Accepted {
                return Ok(existing.clone());
            }
            let mut running = existing.clone();
            running.revision = running.revision.saturating_add(1);
            running.status = TargetOperationStatusKind::Running;
            running.updated_at_ms = Utc::now().timestamp_millis();
            if self
                .append(projection.next_sequence, &running)
                .await?
                .is_some()
            {
                return Ok(running);
            }
        }
        anyhow::bail!("local operation ledger changed too frequently while starting work")
    }

    async fn complete(
        &self,
        operation: &TargetOperationRecord,
        receipt: &TargetOperationReceipt,
    ) -> anyhow::Result<LocalOperationSnapshot> {
        for _ in 0..8 {
            let projection = self.load().await?;
            let existing = projection
                .operations
                .get(&operation.operation_id)
                .context("operation was not durably accepted")?;
            validate_local_binding(existing, operation)?;
            if existing.status.is_terminal() {
                anyhow::ensure!(
                    existing.receipt.as_ref() == Some(receipt),
                    "local operation already has a different terminal receipt"
                );
                return Ok(existing.clone());
            }
            anyhow::ensure!(
                matches!(
                    existing.status,
                    TargetOperationStatusKind::Accepted | TargetOperationStatusKind::Running
                ),
                "local operation cannot transition to terminal"
            );
            let mut completed = existing.clone();
            completed.revision = completed.revision.saturating_add(1);
            completed.status = receipt_status_kind(receipt.status);
            completed.receipt = Some(receipt.clone());
            completed.updated_at_ms = Utc::now().timestamp_millis().max(receipt.completed_at_ms);
            if self
                .append(projection.next_sequence, &completed)
                .await?
                .is_some()
            {
                return Ok(completed);
            }
        }
        anyhow::bail!("local operation ledger changed too frequently while completing work")
    }

    async fn append(
        &self,
        expected_next: EventSequence,
        snapshot: &LocalOperationSnapshot,
    ) -> anyhow::Result<Option<EventEnvelope>> {
        validate_local_snapshot(snapshot)?;
        self.store
            .append_with_sequence_if_next(
                LEDGER_SESSION.to_string(),
                expected_next,
                LEDGER_WRITER.to_string(),
                LEDGER_EVENT.to_string(),
                1,
                serde_json::to_value(snapshot)?,
                json!({ "redacted": true, "credential_material": "none" }),
            )
            .await
    }
}

fn apply_local_ledger_event(
    projection: &mut LedgerProjection,
    event: &EventEnvelope,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        event.session_id == LEDGER_SESSION && event.kind == LEDGER_EVENT,
        "invalid local operation ledger envelope"
    );
    if event.sequence < projection.next_sequence {
        return Ok(());
    }
    anyhow::ensure!(
        event.sequence == projection.next_sequence,
        "local operation ledger sequence is not contiguous"
    );
    let snapshot: LocalOperationSnapshot = serde_json::from_value(event.payload.clone())?;
    validate_local_snapshot(&snapshot)?;
    if let Some(previous) = projection.operations.get(&snapshot.operation_id) {
        anyhow::ensure!(
            snapshot.revision == previous.revision.saturating_add(1)
                && snapshot.target_id == previous.target_id
                && snapshot.installation_id == previous.installation_id
                && snapshot.request_digest == previous.request_digest
                && snapshot.authority_digest == previous.authority_digest
                && snapshot.execution_id == previous.execution_id
                && valid_local_transition(previous.status, snapshot.status),
            "local operation ledger transition is invalid"
        );
    } else {
        anyhow::ensure!(
            snapshot.revision == 1 && snapshot.status == TargetOperationStatusKind::Accepted,
            "new local operation must begin accepted"
        );
    }
    projection
        .operations
        .insert(snapshot.operation_id.clone(), snapshot);
    projection.next_sequence = event.sequence.saturating_add(1);
    Ok(())
}

fn valid_local_transition(
    previous: TargetOperationStatusKind,
    next: TargetOperationStatusKind,
) -> bool {
    matches!(
        (previous, next),
        (
            TargetOperationStatusKind::Accepted,
            TargetOperationStatusKind::Running
                | TargetOperationStatusKind::Succeeded
                | TargetOperationStatusKind::Failed
                | TargetOperationStatusKind::Cancelled
                | TargetOperationStatusKind::OutcomeUnknown
        ) | (
            TargetOperationStatusKind::Running,
            TargetOperationStatusKind::Succeeded
                | TargetOperationStatusKind::Failed
                | TargetOperationStatusKind::Cancelled
                | TargetOperationStatusKind::OutcomeUnknown
        )
    )
}

fn validate_local_snapshot(snapshot: &LocalOperationSnapshot) -> anyhow::Result<()> {
    anyhow::ensure!(
        !snapshot.operation_id.is_empty()
            && !snapshot.target_id.is_empty()
            && is_sha256_digest(&snapshot.request_digest)
            && is_sha256_digest(&snapshot.authority_digest)
            && is_execution_id(&snapshot.execution_id),
        "local operation snapshot binding is invalid"
    );
    if snapshot.status.is_terminal() {
        anyhow::ensure!(
            snapshot.receipt.is_some() && snapshot.status != TargetOperationStatusKind::Expired,
            "local terminal operation requires an agent receipt"
        );
    } else {
        anyhow::ensure!(
            snapshot.receipt.is_none(),
            "nonterminal local operation has receipt"
        );
    }
    Ok(())
}

fn validate_local_binding(
    local: &LocalOperationSnapshot,
    operation: &TargetOperationRecord,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        local.operation_id == operation.operation_id
            && local.target_id == operation.target_id
            && local.installation_id == operation.installation_id
            && local.request_digest == operation.authority.request_digest
            && local.authority_digest == operation.authority.authority_digest
            && operation
                .execution_id
                .as_deref()
                .is_none_or(|execution_id| execution_id == local.execution_id),
        "Host operation conflicts with the durable local operation ledger"
    );
    Ok(())
}

fn receipt_status_kind(status: TargetOperationReceiptStatus) -> TargetOperationStatusKind {
    match status {
        TargetOperationReceiptStatus::Succeeded => TargetOperationStatusKind::Succeeded,
        TargetOperationReceiptStatus::Failed => TargetOperationStatusKind::Failed,
        TargetOperationReceiptStatus::Cancelled => TargetOperationStatusKind::Cancelled,
        TargetOperationReceiptStatus::OutcomeUnknown => TargetOperationStatusKind::OutcomeUnknown,
    }
}

fn is_sha256_digest(digest: &str) -> bool {
    digest.len() == 71
        && digest.starts_with("sha256:")
        && digest[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_execution_id(execution_id: &str) -> bool {
    execution_id.len() == 32
        && execution_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub async fn run(data_dir: PathBuf, credential: String) -> anyhow::Result<()> {
    let (data_dir, config) = load_config(&data_dir)?;
    let _run_lock = acquire_run_lock(&data_dir)?;
    anyhow::ensure!(
        credential_target_id(&credential) == Some(config.target_id.as_str()),
        "target agent credential does not match the configured target"
    );
    normalize_endpoint(&config.endpoint)?;
    let client = hardened_client()?;
    let ledger = LocalOperationLedger::open(&data_dir)?;
    prepare_artifact_store(&data_dir).await?;
    if config
        .capabilities
        .contains(&ExecutionTargetCapability::Deployment)
    {
        plurora_runtime::validate_managed_target_deployment_runtime()
            .await
            .context("deployment capability requires an available Docker runtime")?;
    }
    let _tunnel_task = (config.reachability == ExecutionTargetReachability::ReverseTunnel
        && config
            .capabilities
            .contains(&ExecutionTargetCapability::Deployment))
    .then(|| tokio::spawn(target_tunnel_supervisor(config.clone(), credential.clone())));

    let mut next_heartbeat = Instant::now();
    let mut retry_backoff = Duration::from_secs(1);
    loop {
        let cycle = agent_cycle(
            &client,
            &config,
            &credential,
            &data_dir,
            &ledger,
            &mut next_heartbeat,
        );
        let result = tokio::select! {
            signal = tokio::signal::ctrl_c() => {
                signal?;
                return Ok(());
            }
            result = cycle => result,
        };
        match result {
            Ok(()) => {
                retry_backoff = Duration::from_secs(1);
                tokio::select! {
                    signal = tokio::signal::ctrl_c() => {
                        signal?;
                        return Ok(());
                    }
                    _ = sleep(POLL_INTERVAL) => {}
                }
            }
            Err(error) => {
                eprintln!(
                    "target-agent cycle failed: {}",
                    safe_diagnostic(&format!("{error:#}"), &credential)
                );
                tokio::select! {
                    signal = tokio::signal::ctrl_c() => {
                        signal?;
                        return Ok(());
                    }
                    _ = sleep(retry_backoff) => {}
                }
                retry_backoff = retry_backoff.saturating_mul(2).min(MAX_RETRY_BACKOFF);
            }
        }
    }
}

enum AgentTunnelStreamState {
    Pending(Arc<()>),
    Active {
        reservation: Arc<()>,
        incoming_tx: mpsc::Sender<Vec<u8>>,
    },
}

type AgentTunnelStreams = Arc<AsyncMutex<HashMap<String, AgentTunnelStreamState>>>;

async fn target_tunnel_supervisor(config: AgentConfig, credential: String) {
    let mut backoff = Duration::from_secs(1);
    loop {
        match run_target_tunnel_connection(&config, &credential).await {
            Ok(()) => backoff = Duration::from_secs(1),
            Err(error) => {
                eprintln!(
                    "target-agent tunnel disconnected: {}",
                    safe_diagnostic(&format!("{error:#}"), &credential)
                );
            }
        }
        sleep(backoff).await;
        backoff = backoff.saturating_mul(2).min(MAX_RETRY_BACKOFF);
    }
}

async fn run_target_tunnel_connection(
    config: &AgentConfig,
    credential: &str,
) -> anyhow::Result<()> {
    let mut url = host_url(&config.endpoint, "/target-agent/v1/tunnel")?;
    let websocket_scheme = match url.scheme() {
        "http" => "ws",
        "https" => "wss",
        _ => anyhow::bail!("target tunnel Host endpoint scheme is invalid"),
    };
    url.set_scheme(websocket_scheme)
        .map_err(|_| anyhow::anyhow!("target tunnel Host endpoint scheme could not be set"))?;
    let mut request = url.as_str().into_client_request()?;
    request.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        tokio_tungstenite::tungstenite::http::HeaderValue::from_str(&format!(
            "PluroraTarget {credential}"
        ))?,
    );
    let (socket, _) = tokio_tungstenite::connect_async(request)
        .await
        .context("target reverse tunnel connection failed")?;
    let (mut socket_tx, mut socket_rx) = socket.split();
    let (control_tx, mut control_rx) =
        mpsc::channel::<TunnelMessage>(TARGET_TUNNEL_CONTROL_QUEUE_CAPACITY);
    let (data_tx, mut data_rx) = mpsc::channel::<TunnelMessage>(TARGET_TUNNEL_DATA_QUEUE_CAPACITY);
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
    let streams: AgentTunnelStreams = Arc::new(AsyncMutex::new(HashMap::new()));

    loop {
        tokio::select! {
            biased;
            changed = connection_failed_rx.changed() => {
                if changed.is_err() || *connection_failed_rx.borrow() {
                    break;
                }
            }
            message = socket_rx.next() => {
                let Some(message) = message else { break; };
                let message = message.context("target reverse tunnel receive failed")?;
                if !handle_target_tunnel_host_message(
                    message,
                    config,
                    streams.clone(),
                    control_tx.clone(),
                    data_tx.clone(),
                    connection_failed_tx.clone(),
                )
                .await
                {
                    break;
                }
            }
        }
    }
    streams.lock().await.clear();
    let _ = control_tx.try_send(TunnelMessage::Close(None));
    writer.abort();
    let _ = writer.await;
    Ok(())
}

async fn handle_target_tunnel_host_message(
    message: TunnelMessage,
    config: &AgentConfig,
    streams: AgentTunnelStreams,
    control_tx: mpsc::Sender<TunnelMessage>,
    data_tx: mpsc::Sender<TunnelMessage>,
    connection_failed_tx: watch::Sender<bool>,
) -> bool {
    match message {
        TunnelMessage::Text(text) => {
            let Ok(control) = serde_json::from_str::<TargetTunnelHostMessage>(&text) else {
                return false;
            };
            match control {
                TargetTunnelHostMessage::Open { stream } => {
                    if !valid_tunnel_stream_id(&stream.stream_id) {
                        return false;
                    }
                    let valid_authority = stream.target_id == config.target_id
                        && stream.lease_epoch == config.lease_epoch
                        && stream.policy_epoch == config.policy_epoch
                        && stream.port > 0;
                    if !valid_authority {
                        return send_agent_tunnel_control(
                            &control_tx,
                            TargetTunnelAgentMessage::Rejected {
                                stream_id: stream.stream_id,
                            },
                        );
                    }
                    let reservation = Arc::new(());
                    {
                        let mut active_streams = streams.lock().await;
                        if active_streams.contains_key(&stream.stream_id) {
                            return true;
                        }
                        if active_streams.len() >= TARGET_TUNNEL_MAX_STREAMS {
                            drop(active_streams);
                            return send_agent_tunnel_control(
                                &control_tx,
                                TargetTunnelAgentMessage::Rejected {
                                    stream_id: stream.stream_id,
                                },
                            );
                        }
                        active_streams.insert(
                            stream.stream_id.clone(),
                            AgentTunnelStreamState::Pending(reservation.clone()),
                        );
                    }
                    tokio::spawn(async move {
                        open_agent_tunnel_stream(
                            stream,
                            reservation,
                            streams,
                            control_tx,
                            data_tx,
                            connection_failed_tx,
                        )
                        .await;
                    });
                    true
                }
                TargetTunnelHostMessage::Close { stream_id } => {
                    if !valid_tunnel_stream_id(&stream_id) {
                        return false;
                    }
                    streams.lock().await.remove(&stream_id);
                    true
                }
            }
        }
        TunnelMessage::Binary(frame) => {
            let Some((stream_id, data)) = decode_target_tunnel_data(&frame) else {
                return false;
            };
            let active = streams.lock().await.get(&stream_id).and_then(|stream| {
                if let AgentTunnelStreamState::Active {
                    reservation,
                    incoming_tx,
                } = stream
                {
                    Some((reservation.clone(), incoming_tx.clone()))
                } else {
                    None
                }
            });
            let Some((reservation, incoming_tx)) = active else {
                return true;
            };
            if incoming_tx.try_send(data.to_vec()).is_ok() {
                return true;
            }
            remove_agent_tunnel_stream(&streams, &stream_id, &reservation).await;
            send_agent_tunnel_control(&control_tx, TargetTunnelAgentMessage::Closed { stream_id })
        }
        TunnelMessage::Ping(data) => control_tx.try_send(TunnelMessage::Pong(data)).is_ok(),
        TunnelMessage::Pong(_) => true,
        TunnelMessage::Close(_) | TunnelMessage::Frame(_) => false,
    }
}

async fn open_agent_tunnel_stream(
    stream: TargetTunnelOpen,
    reservation: Arc<()>,
    streams: AgentTunnelStreams,
    control_tx: mpsc::Sender<TunnelMessage>,
    data_tx: mpsc::Sender<TunnelMessage>,
    connection_failed_tx: watch::Sender<bool>,
) {
    let stream_id = stream.stream_id.clone();
    let connection = plurora_runtime::open_managed_target_tunnel_stream(
        &stream.target_id,
        &stream.route_id,
        &stream.port_lease_id,
        &stream.port_name,
        stream.port,
    )
    .await;
    let Ok(connection) = connection else {
        if remove_agent_tunnel_stream(&streams, &stream_id, &reservation).await {
            if !send_agent_tunnel_control(
                &control_tx,
                TargetTunnelAgentMessage::Rejected { stream_id },
            ) {
                let _ = connection_failed_tx.send(true);
            }
        }
        return;
    };
    let (incoming_tx, incoming_rx) = mpsc::channel(TARGET_TUNNEL_STREAM_QUEUE_CAPACITY);
    let mut active_streams = streams.lock().await;
    let still_pending = active_streams.get(&stream_id).is_some_and(|state| {
        matches!(state, AgentTunnelStreamState::Pending(current) if Arc::ptr_eq(current, &reservation))
    });
    if !still_pending {
        drop(active_streams);
        return;
    }
    active_streams.insert(
        stream_id.clone(),
        AgentTunnelStreamState::Active {
            reservation: reservation.clone(),
            incoming_tx,
        },
    );
    drop(active_streams);
    let still_active = streams.lock().await.get(&stream_id).is_some_and(|state| {
        matches!(state, AgentTunnelStreamState::Active { reservation: current, .. } if Arc::ptr_eq(current, &reservation))
    });
    if !still_active {
        return;
    }
    if !send_agent_tunnel_control(
        &control_tx,
        TargetTunnelAgentMessage::Opened {
            stream_id: stream_id.clone(),
        },
    ) {
        remove_agent_tunnel_stream(&streams, &stream_id, &reservation).await;
        let _ = connection_failed_tx.send(true);
        return;
    }
    tokio::spawn(pump_agent_tunnel_stream(
        stream_id,
        connection,
        incoming_rx,
        streams,
        reservation,
        control_tx,
        data_tx,
        connection_failed_tx,
    ));
}

async fn pump_agent_tunnel_stream(
    stream_id: String,
    connection: tokio::net::TcpStream,
    mut incoming_rx: mpsc::Receiver<Vec<u8>>,
    streams: AgentTunnelStreams,
    reservation: Arc<()>,
    control_tx: mpsc::Sender<TunnelMessage>,
    data_tx: mpsc::Sender<TunnelMessage>,
    connection_failed_tx: watch::Sender<bool>,
) {
    let (mut reader, mut writer) = connection.into_split();
    let mut buffer = vec![0u8; TARGET_TUNNEL_DATA_CHUNK_BYTES];
    loop {
        tokio::select! {
            read = reader.read(&mut buffer) => {
                let Ok(read) = read else { break; };
                if read == 0 { break; }
                let Some(frame) = encode_target_tunnel_data(&stream_id, &buffer[..read]) else {
                    break;
                };
                if data_tx.send(TunnelMessage::Binary(frame)).await.is_err() { break; }
            }
            incoming = incoming_rx.recv() => {
                let Some(incoming) = incoming else { break; };
                if writer.write_all(&incoming).await.is_err() { break; }
            }
        }
    }
    let _ = writer.shutdown().await;
    remove_agent_tunnel_stream(&streams, &stream_id, &reservation).await;
    if !send_agent_tunnel_control(&control_tx, TargetTunnelAgentMessage::Closed { stream_id }) {
        let _ = connection_failed_tx.send(true);
    }
}

async fn remove_agent_tunnel_stream(
    streams: &AgentTunnelStreams,
    stream_id: &str,
    reservation: &Arc<()>,
) -> bool {
    let mut streams = streams.lock().await;
    let matches_reservation = streams.get(stream_id).is_some_and(|state| match state {
        AgentTunnelStreamState::Pending(current)
        | AgentTunnelStreamState::Active {
            reservation: current,
            ..
        } => Arc::ptr_eq(current, reservation),
    });
    if matches_reservation {
        streams.remove(stream_id);
    }
    matches_reservation
}

fn send_agent_tunnel_control(
    outbound_tx: &mpsc::Sender<TunnelMessage>,
    message: TargetTunnelAgentMessage,
) -> bool {
    if let Ok(text) = serde_json::to_string(&message) {
        return outbound_tx.try_send(TunnelMessage::Text(text)).is_ok();
    }
    false
}

fn valid_tunnel_stream_id(stream_id: &str) -> bool {
    stream_id.len() == 32
        && stream_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn acquire_run_lock(data_dir: &Path) -> anyhow::Result<std::fs::File> {
    let path = data_dir.join("agent.lock");
    if path.exists() {
        let metadata = std::fs::symlink_metadata(&path)?;
        anyhow::ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "target agent run lock must be a regular file"
        );
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(path)?;
    file.try_lock_exclusive()
        .context("another target agent process already owns this data directory")?;
    Ok(file)
}

async fn agent_cycle(
    client: &Client,
    config: &AgentConfig,
    credential: &str,
    data_dir: &Path,
    ledger: &LocalOperationLedger,
    next_heartbeat: &mut Instant,
) -> anyhow::Result<()> {
    if Instant::now() >= *next_heartbeat {
        heartbeat(client, config, credential, data_dir, ledger).await?;
        *next_heartbeat =
            Instant::now() + Duration::from_secs(config.heartbeat_interval_seconds.max(1));
    }
    let next: NextTargetOperationResponse = request_json::<Value, _>(
        client,
        &config.endpoint,
        Some(credential),
        Method::GET,
        "/target-agent/v1/operations/next",
        None,
    )
    .await?;
    if let Some(operation) = next.operation {
        handle_operation(client, config, credential, data_dir, ledger, operation).await?;
    }
    Ok(())
}

async fn heartbeat(
    client: &Client,
    config: &AgentConfig,
    credential: &str,
    data_dir: &Path,
    ledger: &LocalOperationLedger,
) -> anyhow::Result<()> {
    let running_operation_count = ledger
        .load()
        .await?
        .operations
        .values()
        .filter(|operation| !operation.status.is_terminal())
        .count() as u64;
    let workload_count = if config
        .capabilities
        .contains(&ExecutionTargetCapability::Deployment)
    {
        plurora_runtime::count_managed_target_deployments(&config.target_id).await?
    } else {
        0
    };
    let response: TargetAgentHeartbeatResponse = request_json(
        client,
        &config.endpoint,
        Some(credential),
        Method::POST,
        "/target-agent/v1/heartbeat",
        Some(&TargetAgentHeartbeatRequest {
            protocol_version: config.protocol_version.clone(),
            lease_epoch: config.lease_epoch,
            declared_capabilities: config.capabilities.clone(),
            observed: ExecutionTargetObservedSummary {
                running_operation_count,
                workload_count,
                artifact_count: artifact_count(data_dir).await?,
            },
        }),
    )
    .await?;
    anyhow::ensure!(
        response.target.id == config.target_id
            && response.target.lease_epoch == config.lease_epoch
            && response.target.policy_epoch == config.policy_epoch
            && response.target.selected_protocol_version.as_deref()
                == Some(config.protocol_version.as_str()),
        "Host heartbeat response changed target identity or epoch"
    );
    Ok(())
}

async fn handle_operation(
    client: &Client,
    config: &AgentConfig,
    credential: &str,
    data_dir: &Path,
    ledger: &LocalOperationLedger,
    mut operation: TargetOperationRecord,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        matches!(
            operation.status,
            TargetOperationStatusKind::Requested
                | TargetOperationStatusKind::Accepted
                | TargetOperationStatusKind::Running
        ),
        "Host returned a terminal operation as pending work"
    );
    let projection = ledger.load().await?;
    let local = projection.operations.get(&operation.operation_id);
    if operation.status != TargetOperationStatusKind::Requested && local.is_none() {
        anyhow::bail!(
            "Host operation was already accepted without a matching durable local ledger entry"
        );
    }
    verify_target_operation_authority(
        &operation,
        credential,
        &config.target_id,
        config.lease_epoch,
        config.policy_epoch,
        Utc::now().timestamp_millis(),
        local.is_none(),
    )
    .map_err(anyhow::Error::msg)?;
    if let Some(local) = local {
        validate_local_binding(local, &operation)?;
    }

    let local = ledger.accept(&operation).await?;
    if operation.status == TargetOperationStatusKind::Requested {
        operation = post_progress(
            client,
            config,
            credential,
            &operation,
            &local.execution_id,
            TargetOperationStatusKind::Accepted,
        )
        .await?;
        validate_local_binding(&local, &operation)?;
    }
    if local.status.is_terminal() {
        let receipt = local
            .receipt
            .as_ref()
            .context("terminal local operation lost its receipt")?;
        post_receipt(client, config, credential, receipt).await?;
        return Ok(());
    }

    let local = ledger.mark_running(&operation).await?;
    if local.status.is_terminal() {
        let receipt = local
            .receipt
            .as_ref()
            .context("terminal local operation lost its receipt")?;
        post_receipt(client, config, credential, receipt).await?;
        return Ok(());
    }
    if operation.status != TargetOperationStatusKind::Running {
        operation = post_progress(
            client,
            config,
            credential,
            &operation,
            &local.execution_id,
            TargetOperationStatusKind::Running,
        )
        .await?;
        validate_local_binding(&local, &operation)?;
    }

    // Re-check the live target identity immediately before any local effect.
    // A revoke, stale epoch, or network partition therefore fails closed even
    // when this operation was restored from a durable Running ledger entry.
    heartbeat(client, config, credential, data_dir, ledger).await?;

    let (status, output, diagnostics) =
        match execute_operation(client, config, credential, data_dir, &operation).await {
            Ok(output) => (TargetOperationReceiptStatus::Succeeded, output, Vec::new()),
            Err(error) => {
                let status =
                    if plurora_runtime::is_managed_target_deployment_outcome_unknown(&error) {
                        TargetOperationReceiptStatus::OutcomeUnknown
                    } else {
                        TargetOperationReceiptStatus::Failed
                    };
                (
                    status,
                    Value::Null,
                    vec![safe_diagnostic(&format!("{error:#}"), credential)],
                )
            }
        };
    let completed_at_ms = Utc::now().timestamp_millis();
    let receipt = TargetOperationReceipt {
        operation_id: operation.operation_id.clone(),
        target_id: operation.target_id.clone(),
        execution_id: local.execution_id.clone(),
        step_id: operation.authority.step_id.clone(),
        request_digest: operation.authority.request_digest.clone(),
        authority_digest: operation.authority.authority_digest.clone(),
        status,
        completed_at_ms,
        output,
        diagnostics,
    };
    ledger.complete(&operation, &receipt).await?;
    post_receipt(client, config, credential, &receipt).await?;
    Ok(())
}

async fn post_progress(
    client: &Client,
    config: &AgentConfig,
    credential: &str,
    operation: &TargetOperationRecord,
    execution_id: &str,
    status: TargetOperationStatusKind,
) -> anyhow::Result<TargetOperationRecord> {
    request_json(
        client,
        &config.endpoint,
        Some(credential),
        Method::POST,
        &format!(
            "/target-agent/v1/operations/{}/progress",
            encoded_path_segment(&operation.operation_id)
        ),
        Some(&TargetOperationProgressRequest {
            request_digest: operation.authority.request_digest.clone(),
            authority_digest: operation.authority.authority_digest.clone(),
            execution_id: execution_id.to_string(),
            status,
        }),
    )
    .await
}

async fn post_receipt(
    client: &Client,
    config: &AgentConfig,
    credential: &str,
    receipt: &TargetOperationReceipt,
) -> anyhow::Result<TargetOperationRecord> {
    request_json(
        client,
        &config.endpoint,
        Some(credential),
        Method::POST,
        &format!(
            "/target-agent/v1/operations/{}/receipt",
            encoded_path_segment(&receipt.operation_id)
        ),
        Some(receipt),
    )
    .await
}

fn safe_diagnostic(message: &str, credential: &str) -> String {
    let redacted = message.replace(credential, "[redacted]");
    let mut output = String::new();
    for character in redacted.chars() {
        if output.len().saturating_add(character.len_utf8()) > 2 * 1024 {
            break;
        }
        if character.is_control() && !matches!(character, '\n' | '\r' | '\t') {
            output.push(' ');
        } else {
            output.push(character);
        }
    }
    if plurora_runtime::scan_effect_value_for_raw_secrets(
        &json!({ "diagnostic": output.clone() }),
        "receipt.diagnostics",
    )
    .has_findings()
    {
        "operation failed; diagnostic redacted".to_string()
    } else {
        output
    }
}

async fn execute_operation(
    client: &Client,
    config: &AgentConfig,
    credential: &str,
    data_dir: &Path,
    operation: &TargetOperationRecord,
) -> anyhow::Result<Value> {
    match &operation.spec {
        TargetOperationSpec::ArtifactMaterialize {
            digest,
            expected_size_bytes,
        } => {
            let (size_bytes, already_present) = materialize_artifact(
                client,
                config,
                credential,
                data_dir,
                operation,
                digest,
                *expected_size_bytes,
            )
            .await?;
            Ok(json!({
                "digest": digest,
                "size_bytes": size_bytes,
                "already_present": already_present
            }))
        }
        TargetOperationSpec::ArtifactRelease { digest } => {
            let released = release_artifact(data_dir, digest).await?;
            Ok(json!({ "digest": digest, "released": released }))
        }
        TargetOperationSpec::DeploymentApply { deployment } => {
            let reference = &deployment.deployment;
            let applied = plurora_runtime::apply_managed_target_deployment(
                &plurora_runtime::ManagedTargetDeploymentApply {
                    target_id: operation.target_id.clone(),
                    installation_id: operation.installation_id.clone(),
                    deployment_id: reference.deployment_id.clone(),
                    route_id: reference.route_id.clone(),
                    port_lease_id: reference.port_lease_id.clone(),
                    port_name: deployment.port_name.clone(),
                    image: deployment.image.clone(),
                    container_port: deployment.container_port,
                    requested_host_port: deployment.requested_host_port,
                    pull_if_missing: deployment.pull_if_missing,
                    operation_id: operation.operation_id.clone(),
                },
            )
            .await?;
            if let Err(error) = plurora_runtime::wait_for_managed_target_deployment_readiness(
                &applied,
                deployment.health_path.as_deref(),
            )
            .await
            {
                let cleanup = plurora_runtime::stop_managed_target_deployment(
                    &managed_deployment_ref(operation, &deployment.deployment),
                    0,
                    true,
                )
                .await;
                if cleanup.is_err() {
                    return Err(plurora_runtime::managed_target_deployment_outcome_unknown(
                        "candidate readiness cleanup",
                    ));
                }
                return Err(error);
            }
            Ok(serde_json::to_value(applied)?)
        }
        TargetOperationSpec::DeploymentObserve { deployment } => {
            let observed = plurora_runtime::observe_managed_target_deployment(
                &managed_deployment_ref(operation, deployment),
            )
            .await?;
            Ok(json!({ "deployment": observed }))
        }
        TargetOperationSpec::DeploymentDrain {
            deployment,
            grace_seconds,
        } => Ok(serde_json::to_value(
            plurora_runtime::drain_managed_target_deployment(
                &managed_deployment_ref(operation, deployment),
                *grace_seconds,
            )
            .await?,
        )?),
        TargetOperationSpec::DeploymentStop {
            deployment,
            grace_seconds,
            force_remove,
        } => Ok(serde_json::to_value(
            plurora_runtime::stop_managed_target_deployment(
                &managed_deployment_ref(operation, deployment),
                *grace_seconds,
                *force_remove,
            )
            .await?,
        )?),
        TargetOperationSpec::HealthProbe => Ok(json!({
            "healthy": true,
            "checked_at_ms": Utc::now().timestamp_millis()
        })),
        TargetOperationSpec::VerifierRun {
            verifier:
                DeclarativeVerifierDescriptor::ArtifactIntegrity {
                    digest,
                    expected_size_bytes,
                },
        } => {
            let path = artifact_path(data_dir, digest)?;
            let (actual_digest, size_bytes) = hash_regular_file(&path).await?;
            anyhow::ensure!(
                actual_digest == *digest,
                "artifact integrity verification failed"
            );
            if let Some(expected) = expected_size_bytes {
                anyhow::ensure!(size_bytes == *expected, "artifact size verification failed");
            }
            Ok(json!({
                "digest": digest,
                "size_bytes": size_bytes,
                "verified": true
            }))
        }
        TargetOperationSpec::VerifierRun {
            verifier:
                DeclarativeVerifierDescriptor::DockerBuild {
                    digest,
                    expected_size_bytes,
                    dockerfile,
                    network_mode,
                    build_id,
                    workspace_id,
                    source_tree_digest,
                    build_descriptor_hash,
                },
        } => {
            materialize_artifact(
                client,
                config,
                credential,
                data_dir,
                operation,
                digest,
                *expected_size_bytes,
            )
            .await?;
            let context_tar = tokio::fs::read(artifact_path(data_dir, digest)?).await?;
            Ok(serde_json::to_value(
                plurora_runtime::build_managed_target_image(
                    plurora_runtime::ManagedTargetImageBuild {
                        target_id: operation.target_id.clone(),
                        installation_id: operation.installation_id.clone(),
                        workspace_id: workspace_id.clone(),
                        build_id: build_id.clone(),
                        dockerfile: dockerfile.clone(),
                        network_mode: *network_mode,
                        source_tree_digest: source_tree_digest.clone(),
                        build_descriptor_hash: build_descriptor_hash.clone(),
                        context_digest: digest.clone(),
                        context_tar,
                    },
                )
                .await?,
            )?)
        }
    }
}

fn managed_deployment_ref(
    operation: &TargetOperationRecord,
    deployment: &TargetDeploymentRef,
) -> plurora_runtime::ManagedTargetDeploymentRef {
    plurora_runtime::ManagedTargetDeploymentRef {
        target_id: operation.target_id.clone(),
        installation_id: operation.installation_id.clone(),
        deployment_id: deployment.deployment_id.clone(),
        route_id: deployment.route_id.clone(),
        port_lease_id: deployment.port_lease_id.clone(),
    }
}

fn artifact_root(data_dir: &Path) -> PathBuf {
    data_dir.join("artifacts").join("sha256")
}

fn artifact_path(data_dir: &Path, digest: &str) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(is_sha256_digest(digest), "invalid sha256 artifact digest");
    Ok(artifact_root(data_dir).join(&digest[7..]))
}

async fn prepare_artifact_store(data_dir: &Path) -> anyhow::Result<()> {
    let root = artifact_root(data_dir);
    tokio::fs::create_dir_all(&root).await?;
    let metadata = tokio::fs::symlink_metadata(&root).await?;
    anyhow::ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "target agent artifact root must be a real directory"
    );
    let mut entries = tokio::fs::read_dir(&root).await?;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') && name.contains(".partial.") {
            let metadata = tokio::fs::symlink_metadata(entry.path()).await?;
            if metadata.is_file() && !metadata.file_type().is_symlink() {
                tokio::fs::remove_file(entry.path()).await?;
            }
        }
    }
    Ok(())
}

async fn artifact_count(data_dir: &Path) -> anyhow::Result<u64> {
    let root = artifact_root(data_dir);
    let mut count = 0u64;
    let mut entries = tokio::fs::read_dir(root).await?;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let file_type = entry.file_type().await?;
        if file_type.is_file()
            && !file_type.is_symlink()
            && name.len() == 64
            && name
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            count = count.saturating_add(1);
        }
    }
    Ok(count)
}

async fn materialize_artifact(
    client: &Client,
    config: &AgentConfig,
    credential: &str,
    data_dir: &Path,
    operation: &TargetOperationRecord,
    digest: &str,
    expected_size_bytes: Option<u64>,
) -> anyhow::Result<(u64, bool)> {
    let final_path = artifact_path(data_dir, digest)?;
    if final_path.exists() {
        let (actual_digest, size_bytes) = hash_regular_file(&final_path).await?;
        anyhow::ensure!(actual_digest == digest, "existing artifact is corrupt");
        if let Some(expected) = expected_size_bytes {
            anyhow::ensure!(size_bytes == expected, "existing artifact size is invalid");
        }
        return Ok((size_bytes, true));
    }

    let url = host_url(
        &config.endpoint,
        &format!(
            "/target-agent/v1/operations/{}/artifacts/{}",
            encoded_path_segment(&operation.operation_id),
            encoded_path_segment(digest)
        ),
    )?;
    let mut response = client
        .get(url)
        .header("authorization", format!("PluroraTarget {credential}"))
        .header("accept", "application/octet-stream")
        .send()
        .await
        .context("authorized artifact request failed")?;
    let status = response.status();
    anyhow::ensure!(
        status == StatusCode::OK,
        "authorized artifact request returned {status}; response details redacted"
    );
    let response_digest = response
        .headers()
        .get("x-plurora-artifact-digest")
        .and_then(|value| value.to_str().ok());
    anyhow::ensure!(
        response_digest == Some(digest),
        "authorized artifact response digest header did not match"
    );
    let response_size = response
        .content_length()
        .context("authorized artifact response omitted content length")?;
    if let Some(expected) = expected_size_bytes {
        anyhow::ensure!(
            response_size == expected,
            "authorized artifact response size did not match"
        );
    }

    let root = artifact_root(data_dir);
    let temporary_path = root.join(format!(
        ".{}.partial.{}",
        &digest[7..],
        uuid::Uuid::new_v4().simple()
    ));
    let result = async {
        let mut file = tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary_path)
            .await?;
        let mut hasher = Sha256::new();
        let mut size_bytes = 0u64;
        while let Some(chunk) = response.chunk().await? {
            size_bytes = size_bytes
                .checked_add(chunk.len() as u64)
                .context("artifact size overflow")?;
            anyhow::ensure!(
                size_bytes <= response_size,
                "artifact response exceeded declared content length"
            );
            hasher.update(&chunk);
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        file.sync_all().await?;
        drop(file);
        anyhow::ensure!(
            size_bytes == response_size,
            "artifact response ended before declared content length"
        );
        let actual_digest = format!("sha256:{:x}", hasher.finalize());
        anyhow::ensure!(
            actual_digest == digest,
            "artifact response failed digest verification"
        );
        tokio::fs::rename(&temporary_path, &final_path).await?;
        sync_directory(&root)?;
        Ok::<_, anyhow::Error>(size_bytes)
    }
    .await;
    match result {
        Ok(size_bytes) => Ok((size_bytes, false)),
        Err(error) => {
            let _ = tokio::fs::remove_file(&temporary_path).await;
            if final_path.exists() {
                let (actual_digest, size_bytes) = hash_regular_file(&final_path).await?;
                if actual_digest == digest
                    && expected_size_bytes.is_none_or(|expected| expected == size_bytes)
                {
                    return Ok((size_bytes, true));
                }
            }
            Err(error)
        }
    }
}

async fn release_artifact(data_dir: &Path, digest: &str) -> anyhow::Result<bool> {
    let path = artifact_path(data_dir, digest)?;
    let metadata = match tokio::fs::symlink_metadata(&path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    anyhow::ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "artifact release target must be a regular file"
    );
    tokio::fs::remove_file(&path).await?;
    sync_directory(&artifact_root(data_dir))?;
    Ok(true)
}

async fn hash_regular_file(path: &Path) -> anyhow::Result<(String, u64)> {
    let metadata = tokio::fs::symlink_metadata(path).await?;
    anyhow::ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "artifact path must be a regular file"
    );
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut size_bytes = 0u64;
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        size_bytes = size_bytes
            .checked_add(read as u64)
            .context("artifact size overflow")?;
        hasher.update(&buffer[..read]);
    }
    anyhow::ensure!(
        size_bytes == metadata.len(),
        "artifact changed while it was being verified"
    );
    Ok((format!("sha256:{:x}", hasher.finalize()), size_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use plurora_service::{TargetOperationAuthority, TargetOperationEffect};

    fn operation() -> TargetOperationRecord {
        let digest = format!("sha256:{}", "a".repeat(64));
        TargetOperationRecord {
            operation_id: "target-operation-test".to_string(),
            target_id: "remote-1".to_string(),
            installation_id: InstallationId::parse("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa").unwrap(),
            revision: 1,
            status: TargetOperationStatusKind::Requested,
            execution_id: None,
            spec: TargetOperationSpec::HealthProbe,
            authority: TargetOperationAuthority {
                target_id: "remote-1".to_string(),
                operation_id: "target-operation-test".to_string(),
                step_id: "execute".to_string(),
                installation_id: InstallationId::parse("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa")
                    .unwrap(),
                effect: TargetOperationEffect::HealthProbe,
                artifact_digests: Vec::new(),
                lease_epoch: 1,
                policy_epoch: 1,
                issued_at_ms: 1,
                expires_at_ms: i64::MAX,
                nonce: "nonce".to_string(),
                request_digest: digest.clone(),
                authority_digest: digest,
            },
            idempotency_key: None,
            receipt: None,
            created_at_ms: 1,
            updated_at_ms: 1,
        }
    }

    fn tunnel_config() -> AgentConfig {
        AgentConfig {
            schema_version: 1,
            endpoint: "http://127.0.0.1:3000".to_string(),
            target_id: "remote-1".to_string(),
            protocol_version: PROTOCOL_VERSION.to_string(),
            lease_epoch: 1,
            policy_epoch: 1,
            reachability: ExecutionTargetReachability::ReverseTunnel,
            capabilities: vec![ExecutionTargetCapability::Deployment],
            heartbeat_interval_seconds: 5,
        }
    }

    #[tokio::test]
    async fn local_ledger_replays_terminal_receipt() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let ledger = LocalOperationLedger::open(directory.path())?;
        let operation = operation();
        ledger.accept(&operation).await?;
        let running = ledger.mark_running(&operation).await?;
        let receipt = TargetOperationReceipt {
            operation_id: operation.operation_id.clone(),
            target_id: operation.target_id.clone(),
            execution_id: running.execution_id,
            step_id: "execute".to_string(),
            request_digest: operation.authority.request_digest.clone(),
            authority_digest: operation.authority.authority_digest.clone(),
            status: TargetOperationReceiptStatus::Succeeded,
            completed_at_ms: 2,
            output: json!({ "healthy": true }),
            diagnostics: Vec::new(),
        };
        ledger.complete(&operation, &receipt).await?;

        let reopened = LocalOperationLedger::open(directory.path())?;
        let restored = reopened.load().await?;
        let restored = restored.operations.get(&operation.operation_id).unwrap();
        assert_eq!(restored.status, TargetOperationStatusKind::Succeeded);
        assert_eq!(restored.receipt.as_ref(), Some(&receipt));
        let mut claimed_elsewhere = operation;
        claimed_elsewhere.execution_id = Some("d".repeat(32));
        assert!(validate_local_binding(restored, &claimed_elsewhere).is_err());
        Ok(())
    }

    #[test]
    fn one_process_owns_an_agent_data_directory() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let _first = acquire_run_lock(directory.path())?;
        assert!(acquire_run_lock(directory.path()).is_err());
        Ok(())
    }

    #[test]
    fn diagnostics_redact_raw_secrets_beyond_the_agent_credential() {
        assert_eq!(
            safe_diagnostic(
                "deployment failed with sk-Abcdefghijklmnopqrstuvwxyz123456",
                "unrelated-credential"
            ),
            "operation failed; diagnostic redacted"
        );
    }

    #[tokio::test]
    async fn artifact_paths_are_digest_derived_and_release_is_idempotent() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        prepare_artifact_store(directory.path()).await?;
        let bytes = b"verified artifact";
        let digest = format!("sha256:{:x}", Sha256::digest(bytes));
        let path = artifact_path(directory.path(), &digest)?;
        tokio::fs::write(&path, bytes).await?;
        let (actual, size) = hash_regular_file(&path).await?;
        assert_eq!(actual, digest);
        assert_eq!(size, bytes.len() as u64);
        assert!(release_artifact(directory.path(), &digest).await?);
        assert!(!release_artifact(directory.path(), &digest).await?);
        assert!(artifact_path(directory.path(), "../../escape").is_err());
        Ok(())
    }

    #[tokio::test]
    async fn tunnel_close_cancels_a_pending_open() -> anyhow::Result<()> {
        let stream_id = "a".repeat(32);
        let reservation = Arc::new(());
        let streams: AgentTunnelStreams = Arc::new(AsyncMutex::new(HashMap::from([(
            stream_id.clone(),
            AgentTunnelStreamState::Pending(reservation.clone()),
        )])));
        let (control_tx, _control_rx) = mpsc::channel(1);
        let (data_tx, _data_rx) = mpsc::channel(1);
        let (connection_failed_tx, _connection_failed_rx) = watch::channel(false);
        let close = TunnelMessage::Text(serde_json::to_string(&TargetTunnelHostMessage::Close {
            stream_id: stream_id.clone(),
        })?);

        assert!(
            handle_target_tunnel_host_message(
                close,
                &tunnel_config(),
                streams.clone(),
                control_tx,
                data_tx,
                connection_failed_tx,
            )
            .await
        );
        assert!(!streams.lock().await.contains_key(&stream_id));
        assert!(!remove_agent_tunnel_stream(&streams, &stream_id, &reservation).await);
        Ok(())
    }

    #[tokio::test]
    async fn slow_agent_stream_is_closed_without_blocking_controls() -> anyhow::Result<()> {
        let stream_id = "b".repeat(32);
        let reservation = Arc::new(());
        let (incoming_tx, _incoming_rx) = mpsc::channel(1);
        incoming_tx.try_send(vec![1])?;
        let streams: AgentTunnelStreams = Arc::new(AsyncMutex::new(HashMap::from([(
            stream_id.clone(),
            AgentTunnelStreamState::Active {
                reservation,
                incoming_tx,
            },
        )])));
        let (control_tx, mut control_rx) = mpsc::channel(1);
        let (data_tx, _data_rx) = mpsc::channel(1);
        let (connection_failed_tx, _connection_failed_rx) = watch::channel(false);
        let frame = encode_target_tunnel_data(&stream_id, b"overflow").expect("frame encodes");

        let handled = tokio::time::timeout(
            Duration::from_millis(100),
            handle_target_tunnel_host_message(
                TunnelMessage::Binary(frame),
                &tunnel_config(),
                streams.clone(),
                control_tx,
                data_tx,
                connection_failed_tx,
            ),
        )
        .await?;
        assert!(handled);
        assert!(!streams.lock().await.contains_key(&stream_id));
        let TunnelMessage::Text(closed) = control_rx.try_recv()? else {
            anyhow::bail!("expected closed control");
        };
        assert_eq!(
            serde_json::from_str::<TargetTunnelAgentMessage>(&closed)?,
            TargetTunnelAgentMessage::Closed { stream_id }
        );
        Ok(())
    }
}
