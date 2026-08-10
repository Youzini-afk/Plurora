use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Extension, Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use plurora_core::{EventEnvelope, EventSequence};
use plurora_runtime::{
    EventStore, ExecutionTarget, ExecutionTargetCapability, ExecutionTargetObservedSummary,
    ExecutionTargetReachability, ExecutionTargetRegistry, ExecutionTargetStatusKind,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::host_access::{constant_time_eq, HostAccessIdentity};
use crate::{require_identity_target, AppState, ServiceError};

mod driver;
mod operation;
mod tunnel;

pub(crate) use operation::{
    submit_host_operation, synchronize_target_operations_as_owner, wait_for_host_operation,
};
pub use operation::{
    verify_target_operation_authority, CreateTargetOperationRequest, CreateTargetOperationResponse,
    DeclarativeVerifierDescriptor, NextTargetOperationResponse, TargetDeploymentDescriptor,
    TargetDeploymentRef, TargetOperationAuthority, TargetOperationEffect,
    TargetOperationProgressRequest, TargetOperationReceipt, TargetOperationReceiptStatus,
    TargetOperationRecord, TargetOperationSpec, TargetOperationStatusKind,
};
pub use tunnel::{
    decode_target_tunnel_data, encode_target_tunnel_data, TargetTunnelAgentMessage,
    TargetTunnelHostMessage, TargetTunnelOpen, TARGET_TUNNEL_DATA_CHUNK_BYTES,
    TARGET_TUNNEL_MAX_STREAMS,
};

const JOURNAL_SESSION: &str = "host_control_target_agents";
const JOURNAL_EVENT: &str = "host/control/v1/target_agent.transition";
const JOURNAL_WRITER: &str = "host/control-plane";
const PROTOCOL_VERSION: &str = "target-agent.v1";
const DEFAULT_ENROLLMENT_TTL_SECS: u64 = 10 * 60;
const MAX_ENROLLMENT_TTL_SECS: u64 = 15 * 60;
const HEARTBEAT_INTERVAL_SECS: u64 = 15;
const HEARTBEAT_TTL_MS: i64 = 45_000;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum EnrollmentStatus {
    Pending,
    Claimed,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct StoredEnrollment {
    id: String,
    target_id: String,
    display_name: String,
    reachability: ExecutionTargetReachability,
    allowed_capabilities: Vec<ExecutionTargetCapability>,
    labels: BTreeMap<String, String>,
    secret_digest: String,
    created_at_ms: i64,
    expires_at_ms: i64,
    status: EnrollmentStatus,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct StoredAgent {
    credential_digest: String,
    allowed_capabilities: Vec<ExecutionTargetCapability>,
    target: ExecutionTarget,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TargetAgentJournalEvent {
    EnrollmentCreated {
        enrollment: StoredEnrollment,
    },
    EnrollmentClaimed {
        enrollment_id: String,
        claimed_at_ms: i64,
        agent: StoredAgent,
    },
    Heartbeat {
        target: ExecutionTarget,
    },
    Revoked {
        target: ExecutionTarget,
    },
}

#[derive(Debug, Default)]
struct TargetAgentState {
    next_sequence: EventSequence,
    enrollments: HashMap<String, StoredEnrollment>,
    agents: HashMap<String, StoredAgent>,
    credential_digests: HashMap<(String, u64), String>,
}

#[derive(Debug, Default)]
pub struct TargetAgentRegistry {
    state: Mutex<TargetAgentState>,
    operations: Mutex<operation::TargetOperationState>,
    tunnels: tunnel::TargetTunnelRegistry,
}

pub fn target_agent_registry() -> Arc<TargetAgentRegistry> {
    Arc::new(TargetAgentRegistry::default())
}

impl TargetAgentRegistry {
    pub(crate) fn tunnel_connected(&self, target_id: &str) -> bool {
        self.tunnels.connected(target_id)
    }

    pub(crate) async fn open_tunnel_stream(
        &self,
        stream: TargetTunnelOpen,
    ) -> anyhow::Result<tokio::io::DuplexStream> {
        self.tunnels.open(stream).await
    }

    fn next_sequence(&self) -> EventSequence {
        self.state
            .lock()
            .expect("target agent state lock poisoned")
            .next_sequence
    }

    fn enrollment(&self, enrollment_id: &str) -> Option<StoredEnrollment> {
        self.state
            .lock()
            .expect("target agent state lock poisoned")
            .enrollments
            .get(enrollment_id)
            .cloned()
    }

    fn agent(&self, target_id: &str) -> Option<StoredAgent> {
        self.state
            .lock()
            .expect("target agent state lock poisoned")
            .agents
            .get(target_id)
            .cloned()
    }

    fn authenticate_agent(&self, credential: &str) -> Option<StoredAgent> {
        let target_id = credential_target_id(credential)?;
        let agent = self.agent(target_id)?;
        let candidate_digest = credential_digest("agent", credential);
        (agent.target.status != ExecutionTargetStatusKind::Revoked
            && constant_time_eq(
                candidate_digest.as_bytes(),
                agent.credential_digest.as_bytes(),
            ))
        .then_some(agent)
    }

    fn operation_authority_key(&self, target_id: &str, lease_epoch: u64) -> Option<String> {
        if target_id == "local" && lease_epoch == 1 {
            return Some(credential_digest("local-target-operation", "host:local:v1"));
        }
        self.state
            .lock()
            .expect("target agent state lock poisoned")
            .credential_digests
            .get(&(target_id.to_string(), lease_epoch))
            .cloned()
    }

    fn has_live_enrollment(&self, target_id: &str, now_ms: i64) -> bool {
        self.state
            .lock()
            .expect("target agent state lock poisoned")
            .enrollments
            .values()
            .any(|enrollment| {
                enrollment.target_id == target_id
                    && enrollment.status == EnrollmentStatus::Pending
                    && enrollment.expires_at_ms > now_ms
            })
    }

    fn execution_targets(&self) -> Vec<ExecutionTarget> {
        let now_ms = Utc::now().timestamp_millis();
        let state = self.state.lock().expect("target agent state lock poisoned");
        let mut targets = state
            .agents
            .iter()
            .map(|(id, agent)| (id.clone(), agent.target.clone()))
            .collect::<HashMap<_, _>>();
        for enrollment in state.enrollments.values().filter(|enrollment| {
            enrollment.status == EnrollmentStatus::Pending && enrollment.expires_at_ms > now_ms
        }) {
            targets.insert(enrollment.target_id.clone(), enrollment_target(enrollment));
        }
        targets.into_values().collect()
    }

    fn mark_offline_after_hydration(&self) {
        let mut state = self.state.lock().expect("target agent state lock poisoned");
        for agent in state.agents.values_mut() {
            if agent.target.status != ExecutionTargetStatusKind::Revoked {
                agent.target.status = ExecutionTargetStatusKind::Offline;
                agent.target.heartbeat_expires_at_ms = None;
            }
        }
    }

    fn apply_event(&self, envelope: &EventEnvelope) -> anyhow::Result<()> {
        anyhow::ensure!(
            envelope.session_id == JOURNAL_SESSION && envelope.kind == JOURNAL_EVENT,
            "invalid target agent journal envelope"
        );
        let transition: TargetAgentJournalEvent = serde_json::from_value(envelope.payload.clone())?;
        let mut state = self.state.lock().expect("target agent state lock poisoned");
        if envelope.sequence < state.next_sequence {
            return Ok(());
        }
        anyhow::ensure!(
            envelope.sequence == state.next_sequence,
            "target agent journal sequence is not contiguous"
        );
        match transition {
            TargetAgentJournalEvent::EnrollmentCreated { enrollment } => {
                anyhow::ensure!(
                    enrollment.status == EnrollmentStatus::Pending,
                    "new target enrollment must be pending"
                );
                anyhow::ensure!(
                    enrollment.target_id != "local"
                        && !state.enrollments.contains_key(&enrollment.id),
                    "target enrollment identity was reused"
                );
                state.enrollments.insert(enrollment.id.clone(), enrollment);
            }
            TargetAgentJournalEvent::EnrollmentClaimed {
                enrollment_id,
                claimed_at_ms,
                agent,
            } => {
                let enrollment = state
                    .enrollments
                    .get(&enrollment_id)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("claimed target enrollment does not exist"))?;
                anyhow::ensure!(
                    enrollment.status == EnrollmentStatus::Pending
                        && claimed_at_ms >= enrollment.created_at_ms
                        && claimed_at_ms <= enrollment.expires_at_ms
                        && agent.target.id == enrollment.target_id
                        && agent.target.name == enrollment.display_name
                        && agent.target.reachability == enrollment.reachability
                        && agent.allowed_capabilities == enrollment.allowed_capabilities,
                    "enrolled target does not match its challenge"
                );
                if let Some(existing) = state.agents.get(&agent.target.id) {
                    anyhow::ensure!(
                        existing.target.status == ExecutionTargetStatusKind::Revoked
                            && agent.target.lease_epoch > existing.target.lease_epoch,
                        "active target identity cannot be replaced"
                    );
                }
                state
                    .enrollments
                    .get_mut(&enrollment_id)
                    .expect("claimed enrollment disappeared while journal lock was held")
                    .status = EnrollmentStatus::Claimed;
                state.credential_digests.insert(
                    (agent.target.id.clone(), agent.target.lease_epoch),
                    agent.credential_digest.clone(),
                );
                state.agents.insert(agent.target.id.clone(), agent);
            }
            TargetAgentJournalEvent::Heartbeat { target } => {
                let agent = state
                    .agents
                    .get_mut(&target.id)
                    .ok_or_else(|| anyhow::anyhow!("heartbeat target is not enrolled"))?;
                anyhow::ensure!(
                    agent.target.status != ExecutionTargetStatusKind::Revoked
                        && agent.target.identity_ref == target.identity_ref
                        && agent.target.lease_epoch == target.lease_epoch
                        && agent.target.policy_epoch == target.policy_epoch,
                    "heartbeat target identity or epoch changed"
                );
                agent.target = target;
            }
            TargetAgentJournalEvent::Revoked { target } => {
                let agent = state
                    .agents
                    .get_mut(&target.id)
                    .ok_or_else(|| anyhow::anyhow!("revoked target is not enrolled"))?;
                anyhow::ensure!(
                    agent.target.status != ExecutionTargetStatusKind::Revoked
                        && target.status == ExecutionTargetStatusKind::Revoked
                        && target.lease_epoch == agent.target.lease_epoch.saturating_add(1),
                    "target revoke transition is invalid"
                );
                agent.target = target;
            }
        }
        state.next_sequence = envelope.sequence.saturating_add(1);
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateTargetEnrollmentRequest {
    pub display_name: String,
    pub reachability: ExecutionTargetReachability,
    #[serde(default)]
    pub allowed_capabilities: Vec<ExecutionTargetCapability>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    pub expires_in_seconds: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct CreateTargetEnrollmentResponse {
    pub enrollment_id: String,
    pub target_id: String,
    pub enrollment_token: String,
    pub expires_at_ms: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ClaimTargetEnrollmentRequest {
    pub enrollment_token: String,
    pub protocol_versions: Vec<String>,
    #[serde(default)]
    pub declared_capabilities: Vec<ExecutionTargetCapability>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ClaimTargetEnrollmentResponse {
    pub target: ExecutionTarget,
    pub agent_credential: String,
    pub selected_protocol_version: String,
    pub heartbeat_interval_seconds: u64,
    pub heartbeat_timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TargetAgentHeartbeatRequest {
    pub protocol_version: String,
    pub lease_epoch: u64,
    #[serde(default)]
    pub declared_capabilities: Vec<ExecutionTargetCapability>,
    #[serde(default)]
    pub observed: ExecutionTargetObservedSummary,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TargetAgentHeartbeatResponse {
    pub target: ExecutionTarget,
    pub heartbeat_interval_seconds: u64,
}

pub(super) fn protected_routes<S>() -> Router<AppState<S>>
where
    S: EventStore,
{
    Router::new()
        .route(
            "/host/v1/targets/:target_id/enrollments",
            post(create_enrollment::<S>),
        )
        .route(
            "/host/v1/targets/:target_id/observe",
            get(observe_target::<S>),
        )
        .route(
            "/host/v1/targets/:target_id/revoke",
            post(revoke_target::<S>),
        )
        .merge(operation::protected_routes::<S>())
        .layer(middleware::from_fn(target_agent_response_headers))
}

pub(super) fn public_routes<S>() -> Router<AppState<S>>
where
    S: EventStore,
{
    Router::new()
        .route("/target-agent/v1/enroll", post(claim_enrollment::<S>))
        .route("/target-agent/v1/heartbeat", post(heartbeat::<S>))
        .route("/target-agent/v1/tunnel", get(connect_tunnel::<S>))
        .merge(operation::agent_routes::<S>())
        .layer(middleware::from_fn(target_agent_response_headers))
}

async fn connect_tunnel<S>(
    State(state): State<AppState<S>>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Result<Response, ServiceError>
where
    S: EventStore,
{
    let credential = target_credential(&headers).ok_or_else(target_unauthorized)?;
    sync_target_agent_journal(
        state.runtime.store().as_ref(),
        state.target_agents.as_ref(),
        state.runtime.config().target_registry.as_ref(),
    )
    .await
    .map_err(target_internal_error)?;
    let agent = state
        .target_agents
        .authenticate_agent(credential)
        .ok_or_else(target_unauthorized)?;
    let live_target = state
        .runtime
        .config()
        .target_registry
        .status(&agent.target.id)
        .await
        .ok_or_else(target_unauthorized)?;
    if live_target.status != ExecutionTargetStatusKind::Available
        || live_target.reachability != ExecutionTargetReachability::ReverseTunnel
        || live_target.identity_ref != agent.target.identity_ref
        || live_target.lease_epoch != agent.target.lease_epoch
        || live_target.policy_epoch != agent.target.policy_epoch
        || !live_target
            .capabilities
            .contains(&ExecutionTargetCapability::Deployment)
        || state.target_agents.tunnels.claimed(&live_target.id)
    {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "target must be live, reverse-tunnel reachable, and have no active tunnel",
        ));
    }
    let registration = state
        .target_agents
        .tunnels
        .register(&live_target.id)
        .map_err(|_| {
            ServiceError::with_status(
                StatusCode::CONFLICT,
                "target must be live, reverse-tunnel reachable, and have no active tunnel",
            )
        })?;
    if !tunnel::target_tunnel_identity_is_live(&state, &agent).await {
        state
            .target_agents
            .tunnels
            .remove(&live_target.id, registration.connection_id());
        return Err(target_unauthorized());
    }
    let cleanup_target_id = live_target.id.clone();
    let cleanup_connection_id = registration.connection_id().to_string();
    let cleanup_registry = state.target_agents.clone();
    Ok(upgrade
        .max_frame_size(TARGET_TUNNEL_DATA_CHUNK_BYTES + 32)
        .max_message_size(TARGET_TUNNEL_DATA_CHUNK_BYTES + 32)
        .on_failed_upgrade(move |_| {
            cleanup_registry
                .tunnels
                .remove(&cleanup_target_id, &cleanup_connection_id);
        })
        .on_upgrade(move |socket| tunnel::serve_target_tunnel(state, agent, registration, socket)))
}

async fn target_agent_response_headers(request: axum::extract::Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

async fn create_enrollment<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path(target_id): Path<String>,
    Json(request): Json<CreateTargetEnrollmentRequest>,
) -> Result<(StatusCode, Json<CreateTargetEnrollmentResponse>), ServiceError>
where
    S: EventStore,
{
    require_identity_target(&identity, &target_id)?;
    validate_enrollment_request(&target_id, &request)?;
    let (enrollment, token) = create_enrollment_record(
        state.runtime.store().as_ref(),
        state.target_agents.as_ref(),
        state.runtime.config().target_registry.as_ref(),
        target_id,
        request,
    )
    .await
    .map_err(target_conflict_error)?;
    Ok((
        StatusCode::CREATED,
        Json(CreateTargetEnrollmentResponse {
            enrollment_id: enrollment.id,
            target_id: enrollment.target_id,
            enrollment_token: token,
            expires_at_ms: enrollment.expires_at_ms,
        }),
    ))
}

async fn claim_enrollment<S>(
    State(state): State<AppState<S>>,
    Json(request): Json<ClaimTargetEnrollmentRequest>,
) -> Result<Json<ClaimTargetEnrollmentResponse>, ServiceError>
where
    S: EventStore,
{
    let (target, agent_credential) = claim_enrollment_record(
        state.runtime.store().as_ref(),
        state.target_agents.as_ref(),
        state.runtime.config().target_registry.as_ref(),
        request,
    )
    .await
    .map_err(target_conflict_error)?;
    Ok(Json(ClaimTargetEnrollmentResponse {
        target,
        agent_credential,
        selected_protocol_version: PROTOCOL_VERSION.to_string(),
        heartbeat_interval_seconds: HEARTBEAT_INTERVAL_SECS,
        heartbeat_timeout_seconds: (HEARTBEAT_TTL_MS / 1_000) as u64,
    }))
}

async fn heartbeat<S>(
    State(state): State<AppState<S>>,
    headers: HeaderMap,
    Json(request): Json<TargetAgentHeartbeatRequest>,
) -> Result<Json<TargetAgentHeartbeatResponse>, ServiceError>
where
    S: EventStore,
{
    let credential = target_credential(&headers).ok_or_else(target_unauthorized)?;
    let target = heartbeat_record(
        state.runtime.store().as_ref(),
        state.target_agents.as_ref(),
        state.runtime.config().target_registry.as_ref(),
        credential,
        request,
    )
    .await
    .map_err(|error| match error {
        TargetHeartbeatError::Unauthorized => target_unauthorized(),
        TargetHeartbeatError::Internal(error) => target_internal_error(error),
    })?;
    if let Err(error) = operation::reconcile_target_deployment_projections(&state, &target.id).await
    {
        tracing::warn!(target_id = %target.id, error = %error, "target deployment projection could not refresh after heartbeat");
    }
    Ok(Json(TargetAgentHeartbeatResponse {
        target,
        heartbeat_interval_seconds: HEARTBEAT_INTERVAL_SECS,
    }))
}

async fn observe_target<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path(target_id): Path<String>,
) -> Result<Json<ExecutionTarget>, ServiceError>
where
    S: EventStore,
{
    require_identity_target(&identity, &target_id)?;
    sync_target_agent_journal(
        state.runtime.store().as_ref(),
        state.target_agents.as_ref(),
        state.runtime.config().target_registry.as_ref(),
    )
    .await
    .map_err(target_internal_error)?;
    let target = state
        .runtime
        .config()
        .target_registry
        .status(&target_id)
        .await
        .ok_or_else(|| ServiceError::with_status(StatusCode::NOT_FOUND, "target not found"))?;
    Ok(Json(target))
}

async fn revoke_target<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path(target_id): Path<String>,
) -> Result<Json<ExecutionTarget>, ServiceError>
where
    S: EventStore,
{
    require_identity_target(&identity, &target_id)?;
    let target = revoke_target_record(
        state.runtime.store().as_ref(),
        state.target_agents.as_ref(),
        state.runtime.config().target_registry.as_ref(),
        &target_id,
    )
    .await
    .map_err(target_conflict_error)?;
    state.target_agents.tunnels.disconnect(&target.id);
    if let Err(error) = operation::reconcile_target_deployment_projections(&state, &target.id).await
    {
        tracing::warn!(target_id = %target.id, error = %error, "target deployment projection could not fence revoked target routes");
    }
    Ok(Json(target))
}

fn validate_enrollment_request(
    target_id: &str,
    request: &CreateTargetEnrollmentRequest,
) -> Result<(), ServiceError> {
    if !valid_target_id(target_id) || target_id == "local" {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "target_id must use 1..=80 lowercase ASCII letters, digits, '.', '-', or '_' and cannot be 'local'",
        ));
    }
    let name = request.display_name.trim();
    if name.is_empty()
        || name.len() > 120
        || name != request.display_name
        || name.chars().any(char::is_control)
    {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "target display_name is invalid",
        ));
    }
    if request.reachability == ExecutionTargetReachability::LocalHost {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "remote enrollment cannot create a local_host target",
        ));
    }
    validate_capabilities(&request.allowed_capabilities)?;
    if request.labels.len() > 32
        || request.labels.iter().any(|(key, value)| {
            key.is_empty()
                || key.len() > 64
                || value.len() > 256
                || key.chars().chain(value.chars()).any(char::is_control)
        })
    {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "target labels exceed their count or size limits",
        ));
    }
    let ttl = request
        .expires_in_seconds
        .unwrap_or(DEFAULT_ENROLLMENT_TTL_SECS);
    if ttl == 0 || ttl > MAX_ENROLLMENT_TTL_SECS {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "target enrollment expiry is outside the supported range",
        ));
    }
    Ok(())
}

fn validate_capabilities(capabilities: &[ExecutionTargetCapability]) -> Result<(), ServiceError> {
    let unique = capabilities.iter().copied().collect::<BTreeSet<_>>();
    let supported = [
        ExecutionTargetCapability::ArtifactTransfer,
        ExecutionTargetCapability::DeclarativeVerifier,
        ExecutionTargetCapability::HealthProbe,
        ExecutionTargetCapability::Deployment,
    ];
    if unique.is_empty()
        || unique.len() != capabilities.len()
        || !unique
            .iter()
            .all(|capability| supported.contains(capability))
    {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "target capabilities must be unique and supported by the current agent protocol",
        ));
    }
    Ok(())
}

async fn create_enrollment_record<S>(
    store: &S,
    registry: &TargetAgentRegistry,
    targets: &ExecutionTargetRegistry,
    target_id: String,
    request: CreateTargetEnrollmentRequest,
) -> anyhow::Result<(StoredEnrollment, String)>
where
    S: EventStore,
{
    let id = plurora_core::new_id("target-enrollment");
    let token = format!("plurora_enroll.{id}.{}", random_secret());
    let now_ms = Utc::now().timestamp_millis();
    let ttl_ms = i64::try_from(
        request
            .expires_in_seconds
            .unwrap_or(DEFAULT_ENROLLMENT_TTL_SECS),
    )
    .unwrap_or(i64::MAX)
    .saturating_mul(1_000);
    let enrollment = StoredEnrollment {
        id,
        target_id,
        display_name: request.display_name,
        reachability: request.reachability,
        allowed_capabilities: request.allowed_capabilities,
        labels: request.labels,
        secret_digest: credential_digest("enrollment", &token),
        created_at_ms: now_ms,
        expires_at_ms: now_ms.saturating_add(ttl_ms),
        status: EnrollmentStatus::Pending,
    };
    let event = TargetAgentJournalEvent::EnrollmentCreated {
        enrollment: enrollment.clone(),
    };
    for _ in 0..8 {
        sync_target_agent_journal(store, registry, targets).await?;
        anyhow::ensure!(
            !registry.has_live_enrollment(&enrollment.target_id, now_ms),
            "target already has a pending enrollment"
        );
        if let Some(agent) = registry.agent(&enrollment.target_id) {
            anyhow::ensure!(
                agent.target.status == ExecutionTargetStatusKind::Revoked,
                "target is already enrolled"
            );
        }
        if append_target_agent_event(store, registry, targets, registry.next_sequence(), &event)
            .await?
            .is_some()
        {
            return Ok((enrollment, token));
        }
    }
    anyhow::bail!("target agent journal changed too frequently to create enrollment")
}

async fn claim_enrollment_record<S>(
    store: &S,
    registry: &TargetAgentRegistry,
    targets: &ExecutionTargetRegistry,
    request: ClaimTargetEnrollmentRequest,
) -> anyhow::Result<(ExecutionTarget, String)>
where
    S: EventStore,
{
    let enrollment_id = enrollment_token_id(&request.enrollment_token)
        .ok_or_else(|| anyhow::anyhow!("invalid target enrollment token"))?;
    anyhow::ensure!(
        request
            .protocol_versions
            .iter()
            .any(|version| version == PROTOCOL_VERSION),
        "target agent protocol is incompatible"
    );
    let declared = request
        .declared_capabilities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        declared.len() == request.declared_capabilities.len(),
        "declared target capabilities contain duplicates"
    );
    for _ in 0..8 {
        sync_target_agent_journal(store, registry, targets).await?;
        let enrollment = registry
            .enrollment(enrollment_id)
            .ok_or_else(|| anyhow::anyhow!("target enrollment does not exist"))?;
        let now_ms = Utc::now().timestamp_millis();
        anyhow::ensure!(
            enrollment.status == EnrollmentStatus::Pending && enrollment.expires_at_ms > now_ms,
            "target enrollment is expired or consumed"
        );
        let candidate_digest = credential_digest("enrollment", &request.enrollment_token);
        anyhow::ensure!(
            constant_time_eq(
                candidate_digest.as_bytes(),
                enrollment.secret_digest.as_bytes()
            ),
            "target enrollment secret did not match"
        );
        let allowed = enrollment
            .allowed_capabilities
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let capabilities = declared.intersection(&allowed).copied().collect::<Vec<_>>();
        anyhow::ensure!(
            !capabilities.is_empty(),
            "target declared no policy-approved capability"
        );
        let (lease_epoch, policy_epoch) =
            registry
                .agent(&enrollment.target_id)
                .map_or((1, 1), |agent| {
                    (
                        agent.target.lease_epoch.saturating_add(1),
                        agent.target.policy_epoch.saturating_add(1),
                    )
                });
        let credential = format!("plurora_agent.{}.{}", enrollment.target_id, random_secret());
        let target = ExecutionTarget {
            id: enrollment.target_id.clone(),
            name: enrollment.display_name.clone(),
            reachability: enrollment.reachability,
            declared_capabilities: declared.iter().copied().collect(),
            capabilities,
            status: ExecutionTargetStatusKind::Available,
            protocol_versions: request.protocol_versions.clone(),
            selected_protocol_version: Some(PROTOCOL_VERSION.to_string()),
            identity_ref: Some(format!("target:{}:{lease_epoch}", enrollment.target_id)),
            labels: enrollment.labels.clone(),
            observed: Some(ExecutionTargetObservedSummary::default()),
            last_seen_at_ms: Some(now_ms),
            heartbeat_expires_at_ms: Some(now_ms.saturating_add(HEARTBEAT_TTL_MS)),
            enrolled_at_ms: Some(now_ms),
            revoked_at_ms: None,
            lease_epoch,
            policy_epoch,
        };
        let event = TargetAgentJournalEvent::EnrollmentClaimed {
            enrollment_id: enrollment.id,
            claimed_at_ms: now_ms,
            agent: StoredAgent {
                credential_digest: credential_digest("agent", &credential),
                allowed_capabilities: enrollment.allowed_capabilities,
                target: target.clone(),
            },
        };
        if append_target_agent_event(store, registry, targets, registry.next_sequence(), &event)
            .await?
            .is_some()
        {
            return Ok((target, credential));
        }
    }
    anyhow::bail!("target agent journal changed too frequently to claim enrollment")
}

#[derive(Debug)]
enum TargetHeartbeatError {
    Unauthorized,
    Internal(anyhow::Error),
}

async fn heartbeat_record<S>(
    store: &S,
    registry: &TargetAgentRegistry,
    targets: &ExecutionTargetRegistry,
    credential: &str,
    request: TargetAgentHeartbeatRequest,
) -> Result<ExecutionTarget, TargetHeartbeatError>
where
    S: EventStore,
{
    credential_target_id(credential).ok_or(TargetHeartbeatError::Unauthorized)?;
    if request.protocol_version != PROTOCOL_VERSION {
        return Err(TargetHeartbeatError::Unauthorized);
    }
    let declared = request
        .declared_capabilities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if declared.len() != request.declared_capabilities.len() {
        return Err(TargetHeartbeatError::Unauthorized);
    }
    for _ in 0..8 {
        sync_target_agent_journal(store, registry, targets)
            .await
            .map_err(TargetHeartbeatError::Internal)?;
        let agent = registry
            .authenticate_agent(credential)
            .ok_or(TargetHeartbeatError::Unauthorized)?;
        if request.lease_epoch != agent.target.lease_epoch {
            return Err(TargetHeartbeatError::Unauthorized);
        }
        let allowed = agent
            .allowed_capabilities
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let capabilities = declared.intersection(&allowed).copied().collect::<Vec<_>>();
        if capabilities.is_empty() {
            return Err(TargetHeartbeatError::Unauthorized);
        }
        let now_ms = Utc::now().timestamp_millis();
        let mut target = agent.target;
        target.declared_capabilities = declared.iter().copied().collect();
        target.capabilities = capabilities;
        target.status = ExecutionTargetStatusKind::Available;
        target.observed = Some(request.observed.clone());
        target.last_seen_at_ms = Some(now_ms);
        target.heartbeat_expires_at_ms = Some(now_ms.saturating_add(HEARTBEAT_TTL_MS));
        let event = TargetAgentJournalEvent::Heartbeat {
            target: target.clone(),
        };
        // ponytail: compact heartbeat snapshots only after journal growth is measured in practice.
        if append_target_agent_event(store, registry, targets, registry.next_sequence(), &event)
            .await
            .map_err(TargetHeartbeatError::Internal)?
            .is_some()
        {
            return Ok(target);
        }
    }
    Err(TargetHeartbeatError::Internal(anyhow::anyhow!(
        "target agent journal changed too frequently to record heartbeat"
    )))
}

async fn revoke_target_record<S>(
    store: &S,
    registry: &TargetAgentRegistry,
    targets: &ExecutionTargetRegistry,
    target_id: &str,
) -> anyhow::Result<ExecutionTarget>
where
    S: EventStore,
{
    for _ in 0..8 {
        sync_target_agent_journal(store, registry, targets).await?;
        let agent = registry
            .agent(target_id)
            .ok_or_else(|| anyhow::anyhow!("target identity does not exist"))?;
        if agent.target.status == ExecutionTargetStatusKind::Revoked {
            return Ok(agent.target);
        }
        let mut target = agent.target;
        let now_ms = Utc::now().timestamp_millis();
        target.status = ExecutionTargetStatusKind::Revoked;
        target.revoked_at_ms = Some(now_ms);
        target.heartbeat_expires_at_ms = None;
        target.lease_epoch = target.lease_epoch.saturating_add(1);
        target.policy_epoch = target.policy_epoch.saturating_add(1);
        let event = TargetAgentJournalEvent::Revoked {
            target: target.clone(),
        };
        if append_target_agent_event(store, registry, targets, registry.next_sequence(), &event)
            .await?
            .is_some()
        {
            return Ok(target);
        }
    }
    anyhow::bail!("target agent journal changed too frequently to revoke target")
}

async fn append_target_agent_event<S>(
    store: &S,
    registry: &TargetAgentRegistry,
    targets: &ExecutionTargetRegistry,
    expected_next: EventSequence,
    transition: &TargetAgentJournalEvent,
) -> anyhow::Result<Option<EventEnvelope>>
where
    S: EventStore,
{
    let event = store
        .append_with_sequence_if_next(
            JOURNAL_SESSION.to_string(),
            expected_next,
            JOURNAL_WRITER.to_string(),
            JOURNAL_EVENT.to_string(),
            1,
            serde_json::to_value(transition)?,
            json!({
                "owner": "host_control_plane",
                "credential_material": "sha256_digest_only"
            }),
        )
        .await?;
    if let Some(event) = &event {
        registry.apply_event(event)?;
        mirror_targets(registry, targets).await;
    }
    Ok(event)
}

async fn sync_target_agent_journal<S>(
    store: &S,
    registry: &TargetAgentRegistry,
    targets: &ExecutionTargetRegistry,
) -> anyhow::Result<usize>
where
    S: EventStore,
{
    let mut loaded = 0usize;
    loop {
        let next = registry.next_sequence();
        let events = store
            .list_session_range(
                &JOURNAL_SESSION.to_string(),
                next.checked_sub(1),
                Some(1_000),
            )
            .await?;
        if events.is_empty() {
            break;
        }
        for event in &events {
            registry.apply_event(event)?;
            loaded = loaded.saturating_add(1);
        }
        if events.len() < 1_000 {
            break;
        }
    }
    mirror_targets(registry, targets).await;
    Ok(loaded)
}

async fn mirror_targets(registry: &TargetAgentRegistry, targets: &ExecutionTargetRegistry) {
    let desired = registry.execution_targets();
    let desired_ids = desired
        .iter()
        .map(|target| target.id.clone())
        .collect::<BTreeSet<_>>();
    for existing in targets.list().await {
        if existing.id != "local" && !desired_ids.contains(&existing.id) {
            targets.remove_control_plane_projection(&existing.id).await;
        }
    }
    for target in desired {
        targets.replace_control_plane_projection(target).await;
    }
}

pub async fn hydrate_target_agent_control_plane<S>(
    store: Arc<S>,
    registry: Arc<TargetAgentRegistry>,
    targets: Arc<ExecutionTargetRegistry>,
) -> anyhow::Result<usize>
where
    S: EventStore,
{
    let mut loaded =
        sync_target_agent_journal(store.as_ref(), registry.as_ref(), targets.as_ref()).await?;
    loaded = loaded.saturating_add(
        operation::sync_target_operation_journal(store.as_ref(), registry.as_ref()).await?,
    );
    registry.mark_offline_after_hydration();
    mirror_targets(registry.as_ref(), targets.as_ref()).await;
    Ok(loaded)
}

pub async fn reconcile_target_deployment_control_plane<S>(
    state: &AppState<S>,
) -> anyhow::Result<usize>
where
    S: EventStore,
{
    operation::recover_local_operations_after_restart(state).await?;
    operation::reconcile_all_target_deployment_projections(state).await
}

fn enrollment_target(enrollment: &StoredEnrollment) -> ExecutionTarget {
    ExecutionTarget {
        id: enrollment.target_id.clone(),
        name: enrollment.display_name.clone(),
        reachability: enrollment.reachability,
        declared_capabilities: Vec::new(),
        capabilities: Vec::new(),
        status: ExecutionTargetStatusKind::Enrolling,
        protocol_versions: Vec::new(),
        selected_protocol_version: None,
        identity_ref: None,
        labels: enrollment.labels.clone(),
        observed: None,
        last_seen_at_ms: None,
        heartbeat_expires_at_ms: None,
        enrolled_at_ms: None,
        revoked_at_ms: None,
        lease_epoch: 0,
        policy_epoch: 0,
    }
}

fn random_secret() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn credential_digest(domain: &str, credential: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"plurora-target-agent-v1\0");
    hasher.update(domain.as_bytes());
    hasher.update(b"\0");
    hasher.update(credential.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn enrollment_token_id(token: &str) -> Option<&str> {
    let remainder = token.strip_prefix("plurora_enroll.")?;
    let (id, secret) = remainder.rsplit_once('.')?;
    (!id.is_empty() && secret.len() == 64 && secret.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then_some(id)
}

fn credential_target_id(credential: &str) -> Option<&str> {
    let remainder = credential.strip_prefix("plurora_agent.")?;
    let (target_id, secret) = remainder.rsplit_once('.')?;
    (valid_target_id(target_id)
        && target_id != "local"
        && secret.len() == 64
        && secret.bytes().all(|byte| byte.is_ascii_hexdigit()))
    .then_some(target_id)
}

fn valid_target_id(target_id: &str) -> bool {
    (1..=80).contains(&target_id.len())
        && target_id.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || ".-_".contains(character)
        })
}

fn target_credential(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("PluroraTarget ")
        .filter(|credential| credential_target_id(credential).is_some())
}

fn target_unauthorized() -> ServiceError {
    ServiceError::with_status(StatusCode::UNAUTHORIZED, "invalid target agent credential")
}

fn target_internal_error(error: anyhow::Error) -> ServiceError {
    tracing::warn!(error = %error, "target agent control-plane operation failed");
    ServiceError::with_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        "target agent control plane failed; details redacted",
    )
}

fn target_conflict_error(error: anyhow::Error) -> ServiceError {
    tracing::info!(error = %error, "target agent transition was rejected");
    ServiceError::with_status(
        StatusCode::CONFLICT,
        "target agent state changed or the request is no longer eligible",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use plurora_runtime::InMemoryEventStore;

    fn enrollment_request() -> CreateTargetEnrollmentRequest {
        CreateTargetEnrollmentRequest {
            display_name: "remote verifier".to_string(),
            reachability: ExecutionTargetReachability::ReverseTunnel,
            allowed_capabilities: vec![
                ExecutionTargetCapability::ArtifactTransfer,
                ExecutionTargetCapability::DeclarativeVerifier,
            ],
            labels: BTreeMap::new(),
            expires_in_seconds: None,
        }
    }

    #[tokio::test]
    async fn enrollment_is_single_use_and_hydrates_offline_without_credentials(
    ) -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let registry = target_agent_registry();
        let targets = Arc::new(ExecutionTargetRegistry::default());
        let (enrollment, token) = create_enrollment_record(
            store.as_ref(),
            registry.as_ref(),
            targets.as_ref(),
            "remote-1".to_string(),
            enrollment_request(),
        )
        .await?;
        let (target, credential) = claim_enrollment_record(
            store.as_ref(),
            registry.as_ref(),
            targets.as_ref(),
            ClaimTargetEnrollmentRequest {
                enrollment_token: token.clone(),
                protocol_versions: vec![PROTOCOL_VERSION.to_string()],
                declared_capabilities: enrollment.allowed_capabilities.clone(),
            },
        )
        .await?;
        assert_eq!(target.status, ExecutionTargetStatusKind::Available);
        assert_eq!(credential_target_id(&credential), Some("remote-1"));
        assert!(claim_enrollment_record(
            store.as_ref(),
            registry.as_ref(),
            targets.as_ref(),
            ClaimTargetEnrollmentRequest {
                enrollment_token: token.clone(),
                protocol_versions: vec![PROTOCOL_VERSION.to_string()],
                declared_capabilities: enrollment.allowed_capabilities,
            },
        )
        .await
        .is_err());

        let events = store.list_session(&JOURNAL_SESSION.to_string()).await?;
        let serialized = serde_json::to_string(&events)?;
        assert!(!serialized.contains(&token));
        assert!(!serialized.contains(&credential));

        let restored_registry = target_agent_registry();
        let restored_targets = Arc::new(ExecutionTargetRegistry::default());
        hydrate_target_agent_control_plane(store, restored_registry, restored_targets.clone())
            .await?;
        assert_eq!(
            restored_targets.status("remote-1").await.unwrap().status,
            ExecutionTargetStatusKind::Offline
        );
        Ok(())
    }

    #[tokio::test]
    async fn revoke_fences_old_credential_and_reenrollment_advances_epochs() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let registry = target_agent_registry();
        let targets = Arc::new(ExecutionTargetRegistry::default());
        let (enrollment, token) = create_enrollment_record(
            store.as_ref(),
            registry.as_ref(),
            targets.as_ref(),
            "remote-1".to_string(),
            enrollment_request(),
        )
        .await?;
        let (first, old_credential) = claim_enrollment_record(
            store.as_ref(),
            registry.as_ref(),
            targets.as_ref(),
            ClaimTargetEnrollmentRequest {
                enrollment_token: token,
                protocol_versions: vec![PROTOCOL_VERSION.to_string()],
                declared_capabilities: enrollment.allowed_capabilities,
            },
        )
        .await?;
        assert_eq!((first.lease_epoch, first.policy_epoch), (1, 1));

        let heartbeat = heartbeat_record(
            store.as_ref(),
            registry.as_ref(),
            targets.as_ref(),
            &old_credential,
            TargetAgentHeartbeatRequest {
                protocol_version: PROTOCOL_VERSION.to_string(),
                lease_epoch: first.lease_epoch,
                declared_capabilities: first.declared_capabilities.clone(),
                observed: ExecutionTargetObservedSummary {
                    workload_count: 2,
                    ..ExecutionTargetObservedSummary::default()
                },
            },
        )
        .await
        .map_err(|error| anyhow::anyhow!("initial heartbeat failed: {error:?}"))?;
        assert_eq!(heartbeat.observed.unwrap().workload_count, 2);

        let revoked = revoke_target_record(
            store.as_ref(),
            registry.as_ref(),
            targets.as_ref(),
            "remote-1",
        )
        .await?;
        assert_eq!((revoked.lease_epoch, revoked.policy_epoch), (2, 2));
        assert!(matches!(
            heartbeat_record(
                store.as_ref(),
                registry.as_ref(),
                targets.as_ref(),
                &old_credential,
                TargetAgentHeartbeatRequest {
                    protocol_version: PROTOCOL_VERSION.to_string(),
                    lease_epoch: first.lease_epoch,
                    declared_capabilities: first.declared_capabilities,
                    observed: ExecutionTargetObservedSummary::default(),
                },
            )
            .await,
            Err(TargetHeartbeatError::Unauthorized)
        ));

        let (replacement_enrollment, replacement_token) = create_enrollment_record(
            store.as_ref(),
            registry.as_ref(),
            targets.as_ref(),
            "remote-1".to_string(),
            enrollment_request(),
        )
        .await?;
        let (replacement, replacement_credential) = claim_enrollment_record(
            store.as_ref(),
            registry.as_ref(),
            targets.as_ref(),
            ClaimTargetEnrollmentRequest {
                enrollment_token: replacement_token,
                protocol_versions: vec![PROTOCOL_VERSION.to_string()],
                declared_capabilities: replacement_enrollment.allowed_capabilities,
            },
        )
        .await?;
        assert_ne!(replacement_credential, old_credential);
        assert_eq!((replacement.lease_epoch, replacement.policy_epoch), (3, 3));
        Ok(())
    }

    #[test]
    fn target_credentials_have_distinct_strict_formats() {
        let secret = "a".repeat(64);
        assert_eq!(
            credential_target_id(&format!("plurora_agent.region.remote-1.{secret}")),
            Some("region.remote-1")
        );
        assert!(credential_target_id(&format!("plurora_access.remote-1.{secret}")).is_none());
        assert!(enrollment_token_id(&format!("plurora_enroll.id.{secret}")).is_some());
        assert!(enrollment_token_id(&format!("plurora_pair.id.{secret}")).is_none());
    }
}
