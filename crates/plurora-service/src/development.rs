use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path as FsPath, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::Context;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use plurora_core::project::{ExternalWorkspaceOwnership, ProjectDescriptor, ProjectType};
use plurora_core::{
    ArtifactDescriptor, ChangeCommit, ChangeCommitStatus, ChangeOperation, ChangePrecondition,
    ChangeSet, EffectReceipt, EffectReplayMode, EffectScope, EffectTerminalStatus, Intent,
    PolicyDecision, PolicyDecisionOutcome, PrincipalIdentity, ProjectId,
    COMPONENT_EVIDENCE_TYPE_URI, EFFECT_RECEIPT_TYPE_URI,
};
use plurora_core::{EventEnvelope, EventSequence};
use plurora_runtime::{
    ArtifactCommitRequest, EventStore, ProtocolContext, ProtocolResourceSelector, ProxyRouteAccess,
    Runtime,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex as AsyncMutex, OwnedSemaphorePermit, Semaphore};

use crate::host_access::{
    sync_host_access_journal, HostAccessIdentity, HostAccessIdentityKind, HostAccessRegistry,
    HostAccessScope,
};
use crate::target_agent::{
    CreateTargetOperationRequest, DeclarativeVerifierDescriptor, TargetDeploymentDescriptor,
    TargetDeploymentRef, TargetOperationRecord, TargetOperationSpec, TargetOperationStatusKind,
};
use crate::{
    call_host_protocol, deployment_effect_context, drain_previous_revision,
    invoke_docker_runtime_lab, now_millis, persist_revision_activation, require_built_image,
    require_identity_project, require_identity_target, required_string,
    restore_proxy_route_if_candidate_active, service_public_url_for_route, value_field, AppState,
    BuildDeployProjectGuard, DeploymentActionResponse, DeploymentAuthorityLease,
    DeploymentOperation, DeploymentRevision, DeploymentSourceKind, HostBuildDeployResponse,
    ServiceError,
};

const DEVELOPMENT_JOURNAL_PREFIX: &str = "host/control/v1/development.";
const DEVELOPMENT_SNAPSHOT_EVENT: &str = "host/control/v1/development.change.snapshot";
const DEVELOPMENT_JOURNAL_SESSION_PREFIX: &str = "host_control_development_project";
const DEVELOPMENT_JOURNAL_WRITER: &str = "host/control-plane";
const DEVELOPMENT_HOST_LEASE_SESSION: &str = "host_control_development_lease";
const DEVELOPMENT_HOST_LEASE_EVENT: &str = "host/control/v1/lease.development_host";
const DEVELOPMENT_HOST_LEASE_TTL_MS: i64 = 30_000;
const DEVELOPMENT_HOST_LEASE_HEARTBEAT_MS: u64 = 10_000;
const DEVELOPMENT_MAX_GLOBAL_ACTIVE: usize = 2;
const DEVELOPMENT_MAX_OPERATIONS: usize = 128;
const DEVELOPMENT_MAX_FILE_BYTES: usize = 4 * 1024 * 1024;
const DEVELOPMENT_MAX_TOTAL_INPUT_BYTES: usize = 16 * 1024 * 1024;
const DEVELOPMENT_MAX_EXISTING_FILE_BYTES: u64 = 16 * 1024 * 1024;
const DEVELOPMENT_MAX_EXISTING_TOTAL_BYTES: u64 = 32 * 1024 * 1024;
const DEVELOPMENT_WORKSPACE_MAX_FILES: u64 = 25_000;
const DEVELOPMENT_WORKSPACE_MAX_DIRECTORIES: u64 = 25_000;
const DEVELOPMENT_WORKSPACE_MAX_BYTES: u64 = 256 * 1024 * 1024;
const SOURCE_FILE_ARTIFACT_TYPE_URI: &str = "urn:plurora:artifact:source-file:v1";
const DEVELOPMENT_BUNDLE_ARTIFACT_TYPE_URI: &str =
    "urn:plurora:artifact:development-patch-bundle:v1";
const DEVELOPMENT_RESULT_ARTIFACT_TYPE_URI: &str = "urn:plurora:artifact:development-result:v1";
const DEVELOPMENT_BUILD_CONTEXT_ARTIFACT_TYPE_URI: &str =
    "urn:plurora:artifact:docker-build-context:v1";
const DEVELOPMENT_DEPLOYMENT_AUTHORITY_TYPE_URI: &str =
    "urn:plurora:artifact:deployment-authority:v1";
const DEVELOPMENT_DEPLOYMENT_PREVIEW_TYPE_URI: &str = "urn:plurora:artifact:deployment-preview:v1";
const DEVELOPMENT_DEPLOYMENT_OPERATION_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(15 * 60);

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentDraftRequest {
    pub goal: String,
    pub operations: Vec<DevelopmentFileOperationRequest>,
    #[serde(default)]
    pub verification: DevelopmentVerificationPlan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_tree_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum DevelopmentFileOperationRequest {
    FileWrite {
        path: String,
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        executable: Option<bool>,
    },
    FileDelete {
        path: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DevelopmentVerificationPlan {
    StaticValidation,
    DockerBuild {
        #[serde(default = "default_dockerfile")]
        dockerfile: String,
        #[serde(default)]
        network_mode: DevelopmentNetworkMode,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_secs: Option<u64>,
    },
}

impl Default for DevelopmentVerificationPlan {
    fn default() -> Self {
        Self::StaticValidation
    }
}

fn default_dockerfile() -> String {
    "Dockerfile".to_string()
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DevelopmentNetworkMode {
    #[default]
    None,
    Bridge,
}

impl DevelopmentNetworkMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bridge => "bridge",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DevelopmentWorkspaceOwnership {
    ManagedExternal,
    LinkedLocal,
    NativeManaged,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DevelopmentChangeStatus {
    Drafted,
    Approved,
    Rejected,
    Staging,
    Verifying,
    Promoting,
    Verified,
    Committed,
    RecoveryRequired,
    Failed,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DevelopmentRecoveryKind {
    DockerVerification,
    ManagedPromotion,
}

impl DevelopmentChangeStatus {
    fn terminal(self) -> bool {
        matches!(
            self,
            Self::Rejected
                | Self::Verified
                | Self::Committed
                | Self::RecoveryRequired
                | Self::Failed
        )
    }

    fn executing(self) -> bool {
        matches!(self, Self::Staging | Self::Verifying | Self::Promoting)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DevelopmentVerificationResult {
    pub kind: String,
    pub succeeded: bool,
    pub network_mode: DevelopmentNetworkMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_tail: Option<String>,
    pub artifact_ref: ArtifactDescriptor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment_artifact_ref: Option<ArtifactDescriptor>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentDeploymentPreviewRequest {
    pub target_id: String,
    pub container_port: u16,
    pub port_name: String,
    pub route_id: String,
    #[serde(default)]
    pub route_access: ProxyRouteAccess,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentDeploymentApprovalRequest {
    pub approved: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DevelopmentDeploymentStatus {
    Preparing,
    Building,
    Previewing,
    PreviewReady,
    Approved,
    Rejected,
    Activating,
    Active,
    RecoveryRequired,
    Failed,
}

impl DevelopmentDeploymentStatus {
    fn executing(self) -> bool {
        matches!(
            self,
            Self::Preparing | Self::Building | Self::Previewing | Self::Activating
        )
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct DevelopmentDeploymentPreview {
    pub route_id: String,
    pub public_url: String,
    pub port_lease_id: String,
    pub deployment: TargetDeploymentRef,
    pub image: String,
    pub image_id: String,
    pub container_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
    pub build_operation_id: String,
    pub deployment_operation_id: String,
    pub ready_at_ms: u128,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DevelopmentDeploymentRecord {
    pub schema_version: u16,
    pub deployment_id: String,
    pub status: DevelopmentDeploymentStatus,
    pub target_id: String,
    pub source_tree_digest: String,
    pub verification_ref: ArtifactDescriptor,
    pub build_context_ref: ArtifactDescriptor,
    pub authority_ref: ArtifactDescriptor,
    pub dockerfile: String,
    pub network_mode: DevelopmentNetworkMode,
    pub container_port: u16,
    pub port_name: String,
    pub route_id: String,
    #[serde(default)]
    pub route_access: ProxyRouteAccess,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_path: Option<String>,
    pub preview_route_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_port_lease_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_deployment_id: Option<String>,
    pub build_id: String,
    pub build_descriptor_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_operation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment_operation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<DevelopmentDeploymentPreview>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_ref: Option<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_decision: Option<PolicyDecision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_ref: Option<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation_revision_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_revision_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub created_at_ms: u128,
    pub updated_at_ms: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    pub request_digest: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DevelopmentManagedPromotion {
    pub previous_tree_digest: String,
    pub proposed_tree_digest: String,
    pub destination_preexisting: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DevelopmentChangeRecord {
    pub schema_version: u16,
    pub revision: u64,
    pub project_id: ProjectId,
    pub workspace_ownership: DevelopmentWorkspaceOwnership,
    pub intent: Intent,
    pub intent_ref: ArtifactDescriptor,
    pub change_set: ChangeSet,
    pub change_set_ref: ArtifactDescriptor,
    pub policy_decision: PolicyDecision,
    pub policy_decision_ref: ArtifactDescriptor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_decision: Option<PolicyDecision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_ref: Option<ArtifactDescriptor>,
    pub status: DevelopmentChangeStatus,
    pub base_tree_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposed_tree_digest: Option<String>,
    pub verification_plan: DevelopmentVerificationPlan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_result: Option<DevelopmentVerificationResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub managed_promotion: Option<DevelopmentManagedPromotion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_kind: Option<DevelopmentRecoveryKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<ChangeCommit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment: Option<DevelopmentDeploymentRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub created_at_ms: u128,
    pub updated_at_ms: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DevelopmentChangeSnapshot {
    record: DevelopmentChangeRecord,
    request_fingerprint: String,
}

#[derive(Debug, Clone)]
struct StoredDevelopmentChange {
    record: DevelopmentChangeRecord,
    request_fingerprint: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DevelopmentApprovalRequest {
    approved: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct DevelopmentChangeListResponse {
    changes: Vec<DevelopmentChangeRecord>,
}

#[derive(Debug, Serialize)]
struct DevelopmentExecuteResponse {
    accepted: bool,
    change: DevelopmentChangeRecord,
}

#[derive(Debug, Serialize)]
struct DevelopmentPatchBundle {
    schema_version: u16,
    project_id: ProjectId,
    change_set_id: String,
    base_tree_digest: String,
    operations: Vec<DevelopmentPatchBundleOperation>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum DevelopmentPatchBundleOperation {
    FileWrite {
        path: String,
        content: String,
        executable: bool,
        content_digest: String,
    },
    FileDelete {
        path: String,
    },
}

#[derive(Debug)]
pub struct DevelopmentRegistry {
    changes: Mutex<HashMap<String, StoredDevelopmentChange>>,
    idempotency_claims: Mutex<HashMap<(ProjectId, String), (String, String)>>,
    change_locks: Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
    global_sem: Arc<Semaphore>,
    project_active: Mutex<HashSet<ProjectId>>,
    project_journal_next: Mutex<HashMap<ProjectId, EventSequence>>,
    journal_apply: Mutex<()>,
    host_lease: Mutex<Option<DevelopmentHostLease>>,
}

impl Default for DevelopmentRegistry {
    fn default() -> Self {
        Self {
            changes: Mutex::new(HashMap::new()),
            idempotency_claims: Mutex::new(HashMap::new()),
            change_locks: Mutex::new(HashMap::new()),
            global_sem: Arc::new(Semaphore::new(DEVELOPMENT_MAX_GLOBAL_ACTIVE)),
            project_active: Mutex::new(HashSet::new()),
            project_journal_next: Mutex::new(HashMap::new()),
            journal_apply: Mutex::new(()),
            host_lease: Mutex::new(None),
        }
    }
}

pub fn development_registry() -> Arc<DevelopmentRegistry> {
    Arc::new(DevelopmentRegistry::default())
}

#[derive(Debug, Clone)]
pub struct DevelopmentHostLease {
    owner_id: String,
    valid: Arc<AtomicBool>,
    expires_at_ms: Arc<AtomicI64>,
}

impl DevelopmentHostLease {
    /// Check the locally observed lease state. Durable ownership is kept fresh
    /// by the heartbeat; a heartbeat failure invalidates this handle.
    pub fn ensure_active(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.valid.load(Ordering::Acquire),
            "development host lease is no longer active"
        );
        anyhow::ensure!(
            self.expires_at_ms.load(Ordering::Acquire) > Utc::now().timestamp_millis(),
            "development host lease expired"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DevelopmentHostLeaseEvent {
    owner_id: String,
    expires_at_ms: i64,
    released: bool,
}

async fn development_host_lease_tail<S>(
    store: &S,
) -> anyhow::Result<(EventSequence, Option<DevelopmentHostLeaseEvent>)>
where
    S: EventStore,
{
    let session_id = DEVELOPMENT_HOST_LEASE_SESSION.to_string();
    let next = store.next_sequence(&session_id).await?;
    if next == 0 {
        return Ok((0, None));
    }
    let after = next.checked_sub(2);
    let events = store
        .list_session_range(&session_id, after, Some(2))
        .await?;
    let event = events
        .last()
        .ok_or_else(|| anyhow::anyhow!("development host lease journal tail is missing"))?;
    anyhow::ensure!(
        event.sequence.saturating_add(1) == next && event.kind == DEVELOPMENT_HOST_LEASE_EVENT,
        "development host lease journal is invalid"
    );
    let payload = serde_json::from_value(event.payload.clone())?;
    Ok((next, Some(payload)))
}

async fn append_development_host_lease<S>(
    store: &S,
    expected_next: EventSequence,
    payload: &DevelopmentHostLeaseEvent,
) -> anyhow::Result<bool>
where
    S: EventStore,
{
    Ok(store
        .append_with_sequence_if_next(
            DEVELOPMENT_HOST_LEASE_SESSION.to_string(),
            expected_next,
            DEVELOPMENT_JOURNAL_WRITER.to_string(),
            DEVELOPMENT_HOST_LEASE_EVENT.to_string(),
            1,
            serde_json::to_value(payload)?,
            json!({ "owner": "host_control_plane", "lease": true }),
        )
        .await?
        .is_some())
}

pub async fn acquire_development_host_lease<S>(
    store: Arc<S>,
    registry: Arc<DevelopmentRegistry>,
) -> anyhow::Result<DevelopmentHostLease>
where
    S: EventStore,
{
    let owner_id = format!("host-{}", uuid::Uuid::new_v4().simple());
    for _ in 0..8 {
        let (expected_next, current) = development_host_lease_tail(store.as_ref()).await?;
        let now = Utc::now().timestamp_millis();
        if current.as_ref().is_some_and(|lease| {
            !lease.released && lease.expires_at_ms > now && lease.owner_id != owner_id
        }) {
            anyhow::bail!("another Host currently owns the development control-plane lease");
        }
        let payload = DevelopmentHostLeaseEvent {
            owner_id: owner_id.clone(),
            expires_at_ms: now.saturating_add(DEVELOPMENT_HOST_LEASE_TTL_MS),
            released: false,
        };
        if append_development_host_lease(store.as_ref(), expected_next, &payload).await? {
            let lease = DevelopmentHostLease {
                owner_id,
                valid: Arc::new(AtomicBool::new(true)),
                expires_at_ms: Arc::new(AtomicI64::new(payload.expires_at_ms)),
            };
            registry.install_host_lease(&lease);
            return Ok(lease);
        }
    }
    anyhow::bail!("development host lease changed too frequently to acquire safely")
}

async fn renew_development_host_lease<S>(
    store: &S,
    lease: &DevelopmentHostLease,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    for _ in 0..4 {
        let (expected_next, current) = development_host_lease_tail(store).await?;
        let current =
            current.ok_or_else(|| anyhow::anyhow!("development host lease disappeared"))?;
        anyhow::ensure!(
            !current.released && current.owner_id == lease.owner_id,
            "development host lease ownership changed"
        );
        let payload = DevelopmentHostLeaseEvent {
            owner_id: lease.owner_id.clone(),
            expires_at_ms: Utc::now()
                .timestamp_millis()
                .saturating_add(DEVELOPMENT_HOST_LEASE_TTL_MS),
            released: false,
        };
        if append_development_host_lease(store, expected_next, &payload).await? {
            lease
                .expires_at_ms
                .store(payload.expires_at_ms, Ordering::Release);
            return Ok(());
        }
    }
    anyhow::bail!("development host lease heartbeat conflicted repeatedly")
}

pub fn spawn_development_host_lease_heartbeat<S>(
    store: Arc<S>,
    lease: DevelopmentHostLease,
) -> tokio::task::JoinHandle<()>
where
    S: EventStore,
{
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(
            DEVELOPMENT_HOST_LEASE_HEARTBEAT_MS,
        ));
        interval.tick().await;
        loop {
            interval.tick().await;
            if let Err(error) = renew_development_host_lease(store.as_ref(), &lease).await {
                lease.valid.store(false, Ordering::Release);
                tracing::error!(error = %error, "development host lease heartbeat failed");
                break;
            }
        }
    })
}

pub async fn release_development_host_lease<S>(
    store: Arc<S>,
    lease: &DevelopmentHostLease,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    release_development_host_lease_inner(store, lease, false).await
}

/// Release the lease only if this handle is still the durable owner.
///
/// This stricter form is used when a successful release authorizes a later
/// effect, such as publishing a backup captured under the lease.
pub async fn release_owned_development_host_lease<S>(
    store: Arc<S>,
    lease: &DevelopmentHostLease,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    release_development_host_lease_inner(store, lease, true).await
}

async fn release_development_host_lease_inner<S>(
    store: Arc<S>,
    lease: &DevelopmentHostLease,
    require_ownership: bool,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    lease.valid.store(false, Ordering::Release);
    lease
        .expires_at_ms
        .store(Utc::now().timestamp_millis(), Ordering::Release);
    for _ in 0..4 {
        let (expected_next, current) = development_host_lease_tail(store.as_ref()).await?;
        let Some(current) = current else {
            anyhow::ensure!(
                !require_ownership,
                "development host lease disappeared before strict release"
            );
            return Ok(());
        };
        if current.owner_id != lease.owner_id {
            anyhow::ensure!(
                !require_ownership,
                "development host lease ownership changed before strict release"
            );
            return Ok(());
        }
        if current.released {
            return Ok(());
        }
        let payload = DevelopmentHostLeaseEvent {
            owner_id: lease.owner_id.clone(),
            expires_at_ms: Utc::now().timestamp_millis(),
            released: true,
        };
        if append_development_host_lease(store.as_ref(), expected_next, &payload).await? {
            return Ok(());
        }
    }
    anyhow::bail!("development host lease could not be released after repeated conflicts")
}

enum DraftClaim {
    Existing(DevelopmentChangeRecord),
    Reserved,
}

impl DevelopmentRegistry {
    fn install_host_lease(&self, lease: &DevelopmentHostLease) {
        *self
            .host_lease
            .lock()
            .expect("development host lease lock poisoned") = Some(lease.clone());
    }

    fn ensure_active_host_lease(&self) -> anyhow::Result<()> {
        let lease = self
            .host_lease
            .lock()
            .expect("development host lease lock poisoned")
            .clone()
            .ok_or_else(|| anyhow::anyhow!("development Host lease was not installed"))?;
        lease.ensure_active()
    }

    fn has_host_lease(&self) -> bool {
        self.host_lease
            .lock()
            .expect("development host lease lock poisoned")
            .is_some()
    }

    fn active_host_lease(&self) -> anyhow::Result<DevelopmentHostLease> {
        self.ensure_active_host_lease()?;
        self.host_lease
            .lock()
            .expect("development host lease lock poisoned")
            .clone()
            .ok_or_else(|| anyhow::anyhow!("development Host lease was not installed"))
    }

    fn get(&self, change_set_id: &str) -> Option<DevelopmentChangeRecord> {
        self.changes
            .lock()
            .expect("development changes lock poisoned")
            .get(change_set_id)
            .map(|stored| stored.record.clone())
    }

    fn list(&self, project_id: &ProjectId) -> Vec<DevelopmentChangeRecord> {
        let mut changes = self
            .changes
            .lock()
            .expect("development changes lock poisoned")
            .values()
            .filter(|stored| &stored.record.project_id == project_id)
            .map(|stored| stored.record.clone())
            .collect::<Vec<_>>();
        changes.sort_by_key(|record| std::cmp::Reverse(record.created_at_ms));
        changes
    }

    fn snapshot(&self, change_set_id: &str) -> Option<DevelopmentChangeSnapshot> {
        self.changes
            .lock()
            .expect("development changes lock poisoned")
            .get(change_set_id)
            .map(|stored| DevelopmentChangeSnapshot {
                record: stored.record.clone(),
                request_fingerprint: stored.request_fingerprint.clone(),
            })
    }

    fn apply_journal_event(&self, event: &EventEnvelope) -> anyhow::Result<()> {
        let _apply_guard = self
            .journal_apply
            .lock()
            .expect("development journal apply lock poisoned");
        anyhow::ensure!(
            event.kind == DEVELOPMENT_SNAPSHOT_EVENT,
            "unexpected development journal event kind"
        );
        let snapshot: DevelopmentChangeSnapshot = serde_json::from_value(event.payload.clone())
            .with_context(|| format!("invalid durable development snapshot {}", event.id))?;
        let project_id = snapshot.record.project_id.clone();
        anyhow::ensure!(
            event.session_id == development_project_session(&project_id),
            "development snapshot was written to the wrong project journal"
        );
        let expected_sequence = self.project_journal_next(&project_id);
        if event.sequence < expected_sequence {
            // Concurrent refreshes can observe the same immutable journal page.
            // A sequence below the applied tail is already represented locally.
            return Ok(());
        }
        anyhow::ensure!(
            event.sequence == expected_sequence,
            "development project journal sequence is not contiguous"
        );
        self.apply_snapshot(snapshot)?;
        self.project_journal_next
            .lock()
            .expect("development journal tails lock poisoned")
            .insert(project_id, event.sequence.saturating_add(1));
        Ok(())
    }

    fn apply_snapshot(&self, snapshot: DevelopmentChangeSnapshot) -> anyhow::Result<()> {
        let change_set_id = snapshot.record.change_set.id.clone();
        {
            let changes = self
                .changes
                .lock()
                .expect("development changes lock poisoned");
            match changes.get(&change_set_id) {
                Some(existing) => {
                    anyhow::ensure!(
                        existing.request_fingerprint == snapshot.request_fingerprint,
                        "development change fingerprint changed in durable journal"
                    );
                    anyhow::ensure!(
                        snapshot.record.revision == existing.record.revision.saturating_add(1),
                        "development change revision is not monotonic"
                    );
                    anyhow::ensure!(
                        snapshot.record.project_id == existing.record.project_id,
                        "development change project identity changed"
                    );
                }
                None => anyhow::ensure!(
                    snapshot.record.revision == 1,
                    "new development change must begin at revision 1"
                ),
            }
        }
        if let Some(key) = snapshot.record.idempotency_key.clone() {
            let mut claims = self
                .idempotency_claims
                .lock()
                .expect("development idempotency lock poisoned");
            let claim_key = (snapshot.record.project_id.clone(), key);
            if let Some((fingerprint, claimed_id)) = claims.get(&claim_key) {
                anyhow::ensure!(
                    fingerprint == &snapshot.request_fingerprint && claimed_id == &change_set_id,
                    "development idempotency claim conflicts with durable journal"
                );
            } else {
                claims.insert(
                    claim_key,
                    (snapshot.request_fingerprint.clone(), change_set_id.clone()),
                );
            }
        }
        self.changes
            .lock()
            .expect("development changes lock poisoned")
            .insert(
                change_set_id,
                StoredDevelopmentChange {
                    record: snapshot.record,
                    request_fingerprint: snapshot.request_fingerprint,
                },
            );
        Ok(())
    }

    fn project_journal_next(&self, project_id: &ProjectId) -> EventSequence {
        self.project_journal_next
            .lock()
            .expect("development journal tails lock poisoned")
            .get(project_id)
            .copied()
            .unwrap_or(0)
    }

    fn claim_draft(
        &self,
        project_id: &ProjectId,
        idempotency_key: Option<&str>,
        request_fingerprint: &str,
        change_set_id: &str,
    ) -> anyhow::Result<DraftClaim> {
        let Some(key) = idempotency_key else {
            return Ok(DraftClaim::Reserved);
        };
        let mut claims = self
            .idempotency_claims
            .lock()
            .expect("development idempotency lock poisoned");
        let claim_key = (project_id.clone(), key.to_string());
        if let Some((existing_fingerprint, existing_id)) = claims.get(&claim_key) {
            anyhow::ensure!(
                existing_fingerprint == request_fingerprint,
                "idempotency_key was already used for a different development request"
            );
            let existing = self.get(existing_id).ok_or_else(|| {
                anyhow::anyhow!("an identical development draft is still being created")
            })?;
            return Ok(DraftClaim::Existing(existing));
        }
        claims.insert(
            claim_key,
            (request_fingerprint.to_string(), change_set_id.to_string()),
        );
        Ok(DraftClaim::Reserved)
    }

    fn release_draft_claim(&self, project_id: &ProjectId, idempotency_key: Option<&str>) {
        let Some(key) = idempotency_key else {
            return;
        };
        self.idempotency_claims
            .lock()
            .expect("development idempotency lock poisoned")
            .remove(&(project_id.clone(), key.to_string()));
    }

    fn lock_for(&self, change_set_id: &str) -> Arc<AsyncMutex<()>> {
        self.change_locks
            .lock()
            .expect("development change locks poisoned")
            .entry(change_set_id.to_string())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone()
    }

    fn try_begin(&self, project_id: &ProjectId) -> anyhow::Result<OwnedSemaphorePermit> {
        let permit = self
            .global_sem
            .clone()
            .try_acquire_owned()
            .map_err(|_| anyhow::anyhow!("development global concurrency limit reached"))?;
        let mut active = self
            .project_active
            .lock()
            .expect("development project lock poisoned");
        let durable_active = self
            .changes
            .lock()
            .expect("development changes lock poisoned")
            .values()
            .any(|stored| {
                &stored.record.project_id == project_id && stored.record.status.executing()
            });
        anyhow::ensure!(
            !durable_active && active.insert(project_id.clone()),
            "another development change is already executing for this project"
        );
        Ok(permit)
    }

    fn release_project(&self, project_id: &ProjectId) {
        self.project_active
            .lock()
            .expect("development project lock poisoned")
            .remove(project_id);
    }
}

pub(super) fn routes<S>() -> Router<AppState<S>>
where
    S: EventStore,
{
    Router::new()
        .route(
            "/host/v1/projects/:project_id/changes",
            get(list_changes::<S>).post(draft_change::<S>),
        )
        .route(
            "/host/v1/projects/:project_id/changes/:change_set_id",
            get(get_change::<S>),
        )
        .route(
            "/host/v1/projects/:project_id/changes/:change_set_id/bundle",
            get(get_change_bundle::<S>),
        )
        .route(
            "/host/v1/projects/:project_id/changes/:change_set_id/approve",
            post(approve_change::<S>),
        )
        .route(
            "/host/v1/projects/:project_id/changes/:change_set_id/execute",
            post(execute_change::<S>),
        )
        .route(
            "/host/v1/projects/:project_id/changes/:change_set_id/recover",
            post(recover_change::<S>),
        )
        .route(
            "/host/v1/projects/:project_id/changes/:change_set_id/deployment/preview",
            post(create_deployment_preview::<S>),
        )
        .route(
            "/host/v1/projects/:project_id/changes/:change_set_id/deployment/approve",
            post(approve_deployment::<S>),
        )
        .route(
            "/host/v1/projects/:project_id/changes/:change_set_id/deployment/activate",
            post(activate_deployment::<S>),
        )
        .route(
            "/host/v1/projects/:project_id/changes/:change_set_id/deployment/reconcile",
            post(reconcile_deployment::<S>),
        )
}

async fn list_changes<S>(
    State(state): State<AppState<S>>,
    Path(project_id): Path<String>,
) -> Result<Json<DevelopmentChangeListResponse>, ServiceError>
where
    S: EventStore,
{
    let project_id = parse_project_id(&project_id)?;
    ensure_project_registered(&state, &project_id)?;
    refresh_development_project(&state, &project_id).await?;
    Ok(Json(DevelopmentChangeListResponse {
        changes: state.development.list(&project_id),
    }))
}

async fn get_change<S>(
    State(state): State<AppState<S>>,
    Path((project_id, change_set_id)): Path<(String, String)>,
) -> Result<Json<DevelopmentChangeRecord>, ServiceError>
where
    S: EventStore,
{
    let project_id = parse_project_id(&project_id)?;
    refresh_development_project(&state, &project_id).await?;
    let record = change_for_project(&state, &project_id, &change_set_id)?;
    Ok(Json(record))
}

async fn get_change_bundle<S>(
    State(state): State<AppState<S>>,
    Path((project_id, change_set_id)): Path<(String, String)>,
) -> Result<Json<DevelopmentPatchBundle>, ServiceError>
where
    S: EventStore,
{
    let project_id = parse_project_id(&project_id)?;
    refresh_development_project(&state, &project_id).await?;
    let record = change_for_project(&state, &project_id, &change_set_id)?;
    let bundle = materialize_patch_bundle(state.runtime.as_ref(), &record)
        .await
        .map_err(|error| internal_development_error("failed to read development bundle", error))?;
    Ok(Json(bundle))
}

async fn draft_change<S>(
    State(state): State<AppState<S>>,
    Path(project_id): Path<String>,
    Json(request): Json<DevelopmentDraftRequest>,
) -> Result<(StatusCode, Json<DevelopmentChangeRecord>), ServiceError>
where
    S: EventStore,
{
    ensure_development_host_lease(&state).await?;
    let project_id = parse_project_id(&project_id)?;
    validate_draft_request(&request)?;
    ensure_project_registered(&state, &project_id)?;
    sync_project_journal(
        state.runtime.store().as_ref(),
        state.development.as_ref(),
        &project_id,
    )
    .await
    .map_err(|error| internal_development_error("failed to refresh development journal", error))?;
    let request_fingerprint = development_request_fingerprint(&request).map_err(|error| {
        internal_development_error("failed to fingerprint development request", error)
    })?;
    let change_set_id = development_change_set_id(&project_id, &request);
    let claim = state
        .development
        .claim_draft(
            &project_id,
            request.idempotency_key.as_deref(),
            &request_fingerprint,
            &change_set_id,
        )
        .map_err(|error| {
            ServiceError::with_status(StatusCode::CONFLICT, safe_error_message(&error))
        })?;
    if let DraftClaim::Existing(record) = claim {
        return Ok((StatusCode::OK, Json(record)));
    }

    let result = draft_change_inner(
        &state,
        project_id.clone(),
        change_set_id,
        request.clone(),
        request_fingerprint,
    )
    .await;
    match result {
        Ok(record) => Ok((StatusCode::CREATED, Json(record))),
        Err(error) => {
            state
                .development
                .release_draft_claim(&project_id, request.idempotency_key.as_deref());
            Err(error)
        }
    }
}

async fn approve_change<S>(
    State(state): State<AppState<S>>,
    Path((project_id, change_set_id)): Path<(String, String)>,
    Json(request): Json<DevelopmentApprovalRequest>,
) -> Result<Json<DevelopmentChangeRecord>, ServiceError>
where
    S: EventStore,
{
    ensure_development_host_lease(&state).await?;
    let project_id = parse_project_id(&project_id)?;
    if let Some(reason) = request.reason.as_deref() {
        validate_short_text(reason, "approval reason", 2048)?;
    }
    let change_lock = state.development.lock_for(&change_set_id);
    let _guard = change_lock.lock().await;
    refresh_development_project(&state, &project_id).await?;
    let mut record = change_for_project(&state, &project_id, &change_set_id)?;
    match (record.status, request.approved) {
        (DevelopmentChangeStatus::Approved, true) | (DevelopmentChangeStatus::Rejected, false) => {
            return Ok(Json(record))
        }
        (DevelopmentChangeStatus::Drafted, _) => {}
        _ => {
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "development change can only be approved or rejected from drafted state",
            ));
        }
    }

    let outcome = if request.approved {
        PolicyDecisionOutcome::Allowed
    } else {
        PolicyDecisionOutcome::Denied
    };
    let decision = PolicyDecision {
        id: format!("decision-{}", uuid::Uuid::new_v4().simple()),
        decision_type_uri: plurora_core::POLICY_DECISION_TYPE_URI.to_string(),
        change_set_id: record.change_set.id.clone(),
        outcome,
        principal: PrincipalIdentity::HostAdmin,
        reason: request.reason,
        evaluated_authority: record.change_set.required_authority.clone(),
        decided_at: Utc::now(),
        policy_ref: None,
    };
    let approval_ref = commit_json_artifact(
        state.runtime.as_ref(),
        plurora_core::POLICY_DECISION_TYPE_URI,
        &decision,
        vec![record.change_set_ref.digest.clone()],
        BTreeMap::from([("role".to_string(), json!("explicit_host_approval"))]),
    )
    .await
    .map_err(|error| internal_development_error("failed to store approval decision", error))?;

    record.revision += 1;
    record.updated_at_ms = now_millis();
    record.approval_decision = Some(decision);
    record.approval_ref = Some(approval_ref);
    record.status = if request.approved {
        DevelopmentChangeStatus::Approved
    } else {
        DevelopmentChangeStatus::Rejected
    };
    persist_record(&state, record.clone())
        .await
        .map_err(|error| {
            development_persistence_error("failed to persist approval decision", error)
        })?;
    Ok(Json(record))
}

async fn execute_change<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path((project_id, change_set_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<DevelopmentExecuteResponse>), ServiceError>
where
    S: EventStore,
{
    ensure_development_host_lease(&state).await?;
    let project_id = parse_project_id(&project_id)?;
    require_identity_project(&identity, project_id.as_str())?;
    let change_lock = state.development.lock_for(&change_set_id);
    let _guard = change_lock.lock().await;
    refresh_development_project(&state, &project_id).await?;
    let mut record = change_for_project(&state, &project_id, &change_set_id)?;
    if matches!(
        record.status,
        DevelopmentChangeStatus::Staging
            | DevelopmentChangeStatus::Verifying
            | DevelopmentChangeStatus::Promoting
    ) {
        return Ok((
            StatusCode::OK,
            Json(DevelopmentExecuteResponse {
                accepted: false,
                change: record,
            }),
        ));
    }
    if record.status != DevelopmentChangeStatus::Approved {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "development change must be explicitly approved before execution",
        ));
    }
    let permit = state.development.try_begin(&project_id).map_err(|error| {
        let status = if error.to_string().contains("global concurrency") {
            StatusCode::TOO_MANY_REQUESTS
        } else {
            StatusCode::CONFLICT
        };
        ServiceError::with_status(status, safe_error_message(&error))
    })?;

    record.revision += 1;
    record.updated_at_ms = now_millis();
    record.status = DevelopmentChangeStatus::Staging;
    if let Err(error) = persist_record(&state, record.clone()).await {
        state.development.release_project(&project_id);
        drop(permit);
        return Err(development_persistence_error(
            "failed to persist development execution start",
            error,
        ));
    }

    let task_state = state.clone();
    let task_change_id = change_set_id.clone();
    let task_project_id = project_id.clone();
    let task_identity = identity.clone();
    tokio::spawn(async move {
        let _permit = permit;
        if let Err(error) =
            run_development_change(&task_state, &task_change_id, &task_identity).await
        {
            tracing::warn!(
                project_id = %task_project_id,
                change_set_id = %task_change_id,
                error = %error,
                "development execution failed"
            );
            let mut retry_delay = std::time::Duration::from_millis(100);
            loop {
                match complete_failed_change(&task_state, &task_change_id).await {
                    Ok(()) => break,
                    Err(persist_error) => {
                        tracing::warn!(
                            project_id = %task_project_id,
                            change_set_id = %task_change_id,
                            error = %persist_error,
                            "failed to persist terminal development failure; retrying while lease remains active"
                        );
                        if verify_development_host_lease(
                            task_state.runtime.store().as_ref(),
                            task_state.development.as_ref(),
                        )
                        .await
                        .is_err()
                        {
                            break;
                        }
                        tokio::time::sleep(retry_delay).await;
                        retry_delay = retry_delay
                            .saturating_mul(2)
                            .min(std::time::Duration::from_secs(5));
                    }
                }
            }
        }
        task_state.development.release_project(&task_project_id);
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(DevelopmentExecuteResponse {
            accepted: true,
            change: record,
        }),
    ))
}

async fn recover_change<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path((project_id, change_set_id)): Path<(String, String)>,
) -> Result<Json<DevelopmentChangeRecord>, ServiceError>
where
    S: EventStore,
{
    ensure_development_host_lease(&state).await?;
    let project_id = parse_project_id(&project_id)?;
    require_identity_project(&identity, project_id.as_str())?;
    let change_lock = state.development.lock_for(&change_set_id);
    let _guard = change_lock.lock().await;
    refresh_development_project(&state, &project_id).await?;
    let record = change_for_project(&state, &project_id, &change_set_id)?;
    if record.status != DevelopmentChangeStatus::RecoveryRequired {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "development change does not require recovery",
        ));
    }
    let recovered = match record.recovery_kind {
        Some(DevelopmentRecoveryKind::DockerVerification) => {
            reconcile_docker_verification(&state, record, &identity).await
        }
        Some(DevelopmentRecoveryKind::ManagedPromotion) => {
            reconcile_managed_promotion(&state, record, &identity).await
        }
        None => Err(anyhow::anyhow!("development recovery kind is missing")),
    }
        .map_err(|error| {
            tracing::warn!(project_id = %project_id, change_set_id, error = %error, "development recovery reconciliation failed");
            ServiceError::with_status(
                StatusCode::CONFLICT,
                "development side effects could not be reconciled automatically",
            )
        })?;
    Ok(Json(recovered))
}

async fn create_deployment_preview<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path((project_id, change_set_id)): Path<(String, String)>,
    Json(request): Json<DevelopmentDeploymentPreviewRequest>,
) -> Result<(StatusCode, Json<DevelopmentChangeRecord>), ServiceError>
where
    S: EventStore,
{
    ensure_development_host_lease(&state).await?;
    let project_id = parse_project_id(&project_id)?;
    require_identity_project(&identity, project_id.as_str())?;
    require_identity_target(&identity, &request.target_id)?;
    validate_deployment_preview_request(&request)?;

    let change_lock = state.development.lock_for(&change_set_id);
    let _change_guard = change_lock.lock().await;
    refresh_development_project(&state, &project_id).await?;
    let mut record = change_for_project(&state, &project_id, &change_set_id)?;
    if record.status != DevelopmentChangeStatus::Committed
        || record.workspace_ownership != DevelopmentWorkspaceOwnership::ManagedExternal
    {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "deployment preview requires a committed managed-external change",
        ));
    }

    let (dockerfile, network_mode) = match &record.verification_plan {
        DevelopmentVerificationPlan::DockerBuild {
            dockerfile,
            network_mode,
            ..
        } => (dockerfile.clone(), *network_mode),
        DevelopmentVerificationPlan::StaticValidation => {
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "deployment preview requires Docker build verification",
            ));
        }
    };
    let source_tree_digest = record.proposed_tree_digest.clone().ok_or_else(|| {
        ServiceError::with_status(
            StatusCode::CONFLICT,
            "committed development change has no verified source tree",
        )
    })?;
    let verification = record.verification_result.clone().ok_or_else(|| {
        ServiceError::with_status(
            StatusCode::CONFLICT,
            "committed development change has no verification result",
        )
    })?;
    let build_context_ref = validate_deployment_verification_provenance(
        &record,
        &verification,
        &dockerfile,
        network_mode,
        &source_tree_digest,
    )?;
    verify_deployment_artifact_content(state.runtime.as_ref(), &verification.artifact_ref).await?;
    verify_deployment_artifact_content(state.runtime.as_ref(), &build_context_ref).await?;

    let workspace = resolve_project_workspace(&state, &project_id).map_err(|error| {
        tracing::warn!(project_id = %project_id, error = %error, "deployment workspace resolution failed");
        ServiceError::with_status(
            StatusCode::CONFLICT,
            "managed project workspace is unavailable for deployment preview",
        )
    })?;
    let live_tree = workspace_tree_hash(&workspace.root)
        .await
        .map_err(|error| {
            internal_development_error("failed to verify the live deployment workspace", error)
        })?;
    ensure_descriptor_matches_workspace(&workspace, &live_tree.sha256).map_err(|error| {
        tracing::warn!(project_id = %project_id, error = %error, "deployment workspace descriptor changed");
        ServiceError::with_status(
            StatusCode::CONFLICT,
            "managed project workspace no longer matches its descriptor",
        )
    })?;
    if live_tree.sha256 != source_tree_digest {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "managed project workspace changed after Docker verification",
        ));
    }

    state
        .build_jobs
        .ensure_route_available_for_project(&request.route_id, &project_id)
        .map_err(|error| ServiceError::with_status(StatusCode::CONFLICT, error.to_string()))?;
    if state
        .target_agents
        .project_for_operation_route(&request.route_id)
        .is_some_and(|owner| owner != project_id)
    {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "deployment route is owned by another project",
        ));
    }

    let request_digest = deployment_preview_request_digest(
        &record,
        &request,
        &verification.artifact_ref,
        &build_context_ref,
    )
    .map_err(|error| {
        internal_development_error("failed to bind deployment preview request", error)
    })?;
    if let Some(existing) = record.deployment.as_ref() {
        if existing.request_digest == request_digest {
            return Ok((StatusCode::OK, Json(record)));
        }
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "this development change already has a different deployment request",
        ));
    }

    let deployment_suffix = uuid::Uuid::new_v4().simple().to_string();
    let deployment_id = format!("dep-{deployment_suffix}");
    let preview_route_id = format!("preview-{deployment_suffix}");
    if state
        .runtime
        .config()
        .proxy_route_registry
        .status(&preview_route_id)
        .await
        .is_some_and(|route| route.status != plurora_runtime::ProxyRouteStatusKind::Removed)
    {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "generated deployment preview route already exists",
        ));
    }
    let build_id = format!("verified-{deployment_suffix}");
    let build_descriptor_hash = deployment_build_descriptor_hash(
        &project_id,
        &build_context_ref,
        &source_tree_digest,
        &dockerfile,
        network_mode,
        &build_id,
    );
    let authority = DeploymentAuthorityLease::from_identity(
        format!("dop-{deployment_suffix}"),
        request.target_id.clone(),
        &identity,
    );
    deployment_effect_context(
        &state,
        Some(&authority),
        &project_id,
        "host_development_deployment_prepare",
    )
    .await
    .map_err(|error| {
        tracing::warn!(project_id = %project_id, change_set_id, error = %error, "deployment authority validation failed");
        ServiceError::with_status(
            StatusCode::FORBIDDEN,
            "deployment authority is no longer valid for the selected project and target",
        )
    })?;
    let authority_ref = commit_json_artifact(
        state.runtime.as_ref(),
        DEVELOPMENT_DEPLOYMENT_AUTHORITY_TYPE_URI,
        &json!({
            "schema_version": 1,
            "deployment_id": deployment_id,
            "project_id": project_id,
            "change_set_id": change_set_id,
            "target_id": request.target_id,
            "authority": authority,
        }),
        vec![
            record.intent_ref.digest.clone(),
            record
                .approval_ref
                .as_ref()
                .map(|item| item.digest.clone())
                .ok_or_else(|| {
                    ServiceError::with_status(
                        StatusCode::CONFLICT,
                        "committed development change has no approval artifact",
                    )
                })?,
            verification.artifact_ref.digest.clone(),
            build_context_ref.digest.clone(),
        ],
        BTreeMap::from([
            ("project_id".to_string(), json!(project_id.as_str())),
            ("change_set_id".to_string(), json!(change_set_id)),
            ("target_id".to_string(), json!(request.target_id)),
            ("deployment_id".to_string(), json!(deployment_id)),
        ]),
    )
    .await
    .map_err(|error| internal_development_error("failed to persist deployment authority", error))?;

    let permit = state
        .build_jobs
        .acquire_project_operation(&project_id)
        .await
        .map_err(|error| ServiceError::with_status(StatusCode::CONFLICT, error.to_string()))?;
    let project_guard = BuildDeployProjectGuard {
        registry: state.build_jobs.clone(),
        project_id: project_id.clone(),
    };
    let now = now_millis();
    record.revision += 1;
    record.updated_at_ms = now;
    record.deployment = Some(DevelopmentDeploymentRecord {
        schema_version: 1,
        deployment_id: deployment_id.clone(),
        status: DevelopmentDeploymentStatus::Preparing,
        target_id: request.target_id.clone(),
        source_tree_digest,
        verification_ref: verification.artifact_ref,
        build_context_ref,
        authority_ref,
        dockerfile,
        network_mode,
        container_port: request.container_port,
        port_name: request.port_name,
        route_id: request.route_id,
        route_access: request.route_access,
        health_path: request.health_path,
        preview_route_id,
        preview_port_lease_id: None,
        target_deployment_id: None,
        build_id,
        build_descriptor_hash,
        build_operation_id: None,
        deployment_operation_id: None,
        preview: None,
        preview_ref: None,
        approval_decision: None,
        approval_ref: None,
        activation_revision_id: None,
        previous_revision_id: None,
        error: None,
        created_at_ms: now,
        updated_at_ms: now,
        idempotency_key: request.idempotency_key,
        request_digest,
    });
    let record = match persist_record(&state, record).await {
        Ok(record) => record,
        Err(error) => {
            drop(project_guard);
            drop(permit);
            return Err(development_persistence_error(
                "failed to persist deployment preview start",
                error,
            ));
        }
    };

    let task_state = state.clone();
    let task_change_set_id = change_set_id.clone();
    let task_project_id = project_id.clone();
    tokio::spawn(async move {
        let _permit = permit;
        let _project_guard = project_guard;
        if let Err(error) =
            run_deployment_preview(&task_state, &task_change_set_id, &authority).await
        {
            tracing::warn!(
                project_id = %task_project_id,
                change_set_id = %task_change_set_id,
                error = %error,
                "development deployment preview failed"
            );
            let mut retry_delay = std::time::Duration::from_millis(100);
            loop {
                match complete_deployment_preview_failure(&task_state, &task_change_set_id).await {
                    Ok(()) => break,
                    Err(persist_error) => {
                        tracing::warn!(
                            project_id = %task_project_id,
                            change_set_id = %task_change_set_id,
                            error = %persist_error,
                            "failed to persist deployment preview failure; retrying while Host lease remains active"
                        );
                        if verify_development_host_lease(
                            task_state.runtime.store().as_ref(),
                            task_state.development.as_ref(),
                        )
                        .await
                        .is_err()
                        {
                            break;
                        }
                        tokio::time::sleep(retry_delay).await;
                        retry_delay = retry_delay
                            .saturating_mul(2)
                            .min(std::time::Duration::from_secs(5));
                    }
                }
            }
        }
    });

    Ok((StatusCode::ACCEPTED, Json(record)))
}

async fn approve_deployment<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path((project_id, change_set_id)): Path<(String, String)>,
    Json(request): Json<DevelopmentDeploymentApprovalRequest>,
) -> Result<Json<DevelopmentChangeRecord>, ServiceError>
where
    S: EventStore,
{
    ensure_development_host_lease(&state).await?;
    let project_id = parse_project_id(&project_id)?;
    require_identity_project(&identity, project_id.as_str())?;
    if let Some(reason) = request.reason.as_deref() {
        validate_short_text(reason, "deployment approval reason", 2048)?;
    }

    let record = {
        let change_lock = state.development.lock_for(&change_set_id);
        let _guard = change_lock.lock().await;
        refresh_development_project(&state, &project_id).await?;
        change_for_project(&state, &project_id, &change_set_id)?
    };
    let deployment = record.deployment.clone().ok_or_else(|| {
        ServiceError::with_status(
            StatusCode::CONFLICT,
            "development change has no deployment preview",
        )
    })?;
    require_identity_target(&identity, &deployment.target_id)?;
    match (deployment.status, request.approved) {
        (DevelopmentDeploymentStatus::Approved, true)
        | (DevelopmentDeploymentStatus::Rejected, false) => return Ok(Json(record)),
        (DevelopmentDeploymentStatus::PreviewReady, _) => {}
        _ => {
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "deployment can only be approved or rejected from preview-ready state",
            ));
        }
    }
    let preview_ref = deployment.preview_ref.clone().ok_or_else(|| {
        ServiceError::with_status(
            StatusCode::CONFLICT,
            "deployment preview has no durable evidence",
        )
    })?;
    if deployment.preview.is_none() {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "deployment preview has no ready candidate",
        ));
    }

    let cleanup_guard = if request.approved {
        None
    } else {
        let permit = state
            .build_jobs
            .acquire_project_operation(&project_id)
            .await
            .map_err(|error| ServiceError::with_status(StatusCode::CONFLICT, error.to_string()))?;
        Some((
            permit,
            BuildDeployProjectGuard {
                registry: state.build_jobs.clone(),
                project_id: project_id.clone(),
            },
        ))
    };
    let outcome = if request.approved {
        PolicyDecisionOutcome::Allowed
    } else {
        PolicyDecisionOutcome::Denied
    };
    let decision = PolicyDecision {
        id: format!("decision-{}", uuid::Uuid::new_v4().simple()),
        decision_type_uri: plurora_core::POLICY_DECISION_TYPE_URI.to_string(),
        change_set_id: format!(
            "{}:deployment:{}",
            record.change_set.id, deployment.deployment_id
        ),
        outcome,
        principal: PrincipalIdentity::HostAdmin,
        reason: request.reason,
        evaluated_authority: vec![
            "host.project.deploy".to_string(),
            format!("host.target.{}", deployment.target_id),
        ],
        decided_at: Utc::now(),
        policy_ref: None,
    };
    let approval_ref = commit_json_artifact(
        state.runtime.as_ref(),
        plurora_core::POLICY_DECISION_TYPE_URI,
        &decision,
        vec![
            preview_ref.digest.clone(),
            deployment.verification_ref.digest.clone(),
            deployment.build_context_ref.digest.clone(),
            deployment.authority_ref.digest.clone(),
        ],
        BTreeMap::from([
            ("role".to_string(), json!("explicit_deployment_approval")),
            ("project_id".to_string(), json!(project_id.as_str())),
            ("change_set_id".to_string(), json!(change_set_id)),
            ("deployment_id".to_string(), json!(deployment.deployment_id)),
            ("target_id".to_string(), json!(deployment.target_id)),
        ]),
    )
    .await
    .map_err(|error| {
        internal_development_error("failed to store deployment approval decision", error)
    })?;
    let mut updated = update_deployment_record(&state, &change_set_id, |current| {
        anyhow::ensure!(
            current.deployment_id == deployment.deployment_id
                && current.status == DevelopmentDeploymentStatus::PreviewReady,
            "deployment preview changed before approval persistence"
        );
        current.approval_decision = Some(decision);
        current.approval_ref = Some(approval_ref);
        current.status = if request.approved {
            DevelopmentDeploymentStatus::Approved
        } else {
            DevelopmentDeploymentStatus::Rejected
        };
        current.error = None;
        Ok(())
    })
    .await
    .map_err(|error| {
        development_persistence_error("failed to persist deployment approval decision", error)
    })?;

    if !request.approved {
        let cleanup_complete =
            match stop_completed_preview_candidate(&state, &project_id, &deployment).await {
                Ok(true) => cleanup_preview_host_resources(&state, &deployment)
                    .await
                    .unwrap_or(false),
                Ok(false) | Err(_) => false,
            };
        if !cleanup_complete {
            updated = update_deployment_record(&state, &change_set_id, |current| {
                if current.deployment_id == deployment.deployment_id
                    && current.status == DevelopmentDeploymentStatus::Rejected
                {
                    current.status = DevelopmentDeploymentStatus::RecoveryRequired;
                    current.error = Some(
                        "rejected deployment cleanup requires explicit reconciliation; details redacted"
                            .to_string(),
                    );
                }
                Ok(())
            })
            .await
            .map_err(|error| {
                development_persistence_error(
                    "failed to persist rejected deployment cleanup state",
                    error,
                )
            })?;
        }
    }
    drop(cleanup_guard);
    Ok(Json(updated))
}

async fn activate_deployment<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path((project_id, change_set_id)): Path<(String, String)>,
) -> Result<Json<DevelopmentChangeRecord>, ServiceError>
where
    S: EventStore,
{
    ensure_development_host_lease(&state).await?;
    let project_id = parse_project_id(&project_id)?;
    require_identity_project(&identity, project_id.as_str())?;
    let record = {
        let change_lock = state.development.lock_for(&change_set_id);
        let _guard = change_lock.lock().await;
        refresh_development_project(&state, &project_id).await?;
        change_for_project(&state, &project_id, &change_set_id)?
    };
    let deployment = record.deployment.clone().ok_or_else(|| {
        ServiceError::with_status(
            StatusCode::CONFLICT,
            "development change has no deployment preview",
        )
    })?;
    require_identity_target(&identity, &deployment.target_id)?;
    if deployment.status == DevelopmentDeploymentStatus::Active {
        return Ok(Json(record));
    }
    if deployment.status != DevelopmentDeploymentStatus::Approved
        || deployment
            .approval_decision
            .as_ref()
            .is_none_or(|decision| decision.outcome != PolicyDecisionOutcome::Allowed)
    {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "deployment requires an explicit approval before activation",
        ));
    }
    let preview = deployment.preview.clone().ok_or_else(|| {
        ServiceError::with_status(StatusCode::CONFLICT, "deployment has no preview candidate")
    })?;
    let preview_ref = deployment.preview_ref.clone().ok_or_else(|| {
        ServiceError::with_status(
            StatusCode::CONFLICT,
            "deployment preview has no durable evidence",
        )
    })?;
    let approval_ref = deployment.approval_ref.clone().ok_or_else(|| {
        ServiceError::with_status(
            StatusCode::CONFLICT,
            "deployment approval has no durable evidence",
        )
    })?;
    verify_deployment_artifact_content(state.runtime.as_ref(), &deployment.verification_ref)
        .await?;
    verify_deployment_artifact_content(state.runtime.as_ref(), &deployment.build_context_ref)
        .await?;
    let (evidence_deployment_id, evidence_preview, evidence_authority_ref) =
        read_verified_preview_evidence(
            state.runtime.as_ref(),
            &preview_ref,
            &project_id,
            &change_set_id,
            &deployment.target_id,
            &deployment.source_tree_digest,
            &deployment.verification_ref,
            &deployment.build_context_ref,
        )
        .await
        .map_err(|error| {
            tracing::warn!(project_id = %project_id, change_set_id, error = %error, "deployment preview artifact validation failed");
            ServiceError::with_status(
                StatusCode::CONFLICT,
                "deployment preview evidence is incomplete or inconsistent",
            )
        })?;
    let evidence_decision = read_verified_deployment_approval(
        state.runtime.as_ref(),
        &approval_ref,
        &preview_ref,
        &deployment.verification_ref,
        &deployment.build_context_ref,
        &evidence_authority_ref,
        &project_id,
        &change_set_id,
        &evidence_deployment_id,
        &deployment.target_id,
    )
    .await
    .map_err(|error| {
        tracing::warn!(project_id = %project_id, change_set_id, error = %error, "deployment approval artifact validation failed");
        ServiceError::with_status(
            StatusCode::CONFLICT,
            "deployment approval evidence is incomplete or inconsistent",
        )
    })?;
    if evidence_deployment_id != deployment.deployment_id
        || evidence_preview != preview
        || evidence_authority_ref != deployment.authority_ref
        || deployment.approval_decision.as_ref() != Some(&evidence_decision)
    {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "deployment approval is not bound to the ready preview candidate",
        ));
    }
    validate_preview_target_operations(&state, &project_id, &deployment, &preview).map_err(
        |error| {
            tracing::warn!(project_id = %project_id, change_set_id, error = %error, "deployment preview evidence validation failed");
            ServiceError::with_status(
                StatusCode::CONFLICT,
                "deployment preview evidence is incomplete or inconsistent",
            )
        },
    )?;
    ensure_preview_route_ready(&state, &deployment, &preview)
        .await
        .map_err(|error| {
            tracing::warn!(project_id = %project_id, change_set_id, error = %error, "deployment preview readiness validation failed");
            ServiceError::with_status(
                StatusCode::CONFLICT,
                "deployment preview is no longer ready",
            )
        })?;

    let permit = state
        .build_jobs
        .acquire_project_operation(&project_id)
        .await
        .map_err(|error| ServiceError::with_status(StatusCode::CONFLICT, error.to_string()))?;
    let _project_guard = BuildDeployProjectGuard {
        registry: state.build_jobs.clone(),
        project_id: project_id.clone(),
    };
    let _permit = permit;
    state
        .build_jobs
        .ensure_route_available_for_project(&deployment.route_id, &project_id)
        .map_err(|error| ServiceError::with_status(StatusCode::CONFLICT, error.to_string()))?;
    if state
        .target_agents
        .project_for_operation_route(&deployment.route_id)
        .is_some_and(|owner| owner != project_id)
    {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "deployment route is owned by another project",
        ));
    }
    let authority = DeploymentAuthorityLease::from_identity(
        format!("dop-{}", uuid::Uuid::new_v4().simple()),
        deployment.target_id.clone(),
        &identity,
    );
    let activation_context = deployment_effect_context(
        &state,
        Some(&authority),
        &project_id,
        "host_development_deployment_activate",
    )
    .await
    .map_err(|error| {
        tracing::warn!(project_id = %project_id, change_set_id, error = %error, "deployment activation authority validation failed");
        ServiceError::with_status(
            StatusCode::FORBIDDEN,
            "deployment authority is no longer valid for the selected project and target",
        )
    })?;
    ensure_preview_route_ready(&state, &deployment, &preview)
        .await
        .map_err(|_| {
            ServiceError::with_status(
                StatusCode::CONFLICT,
                "deployment preview is no longer ready",
            )
        })?;
    let previous_revision = state.build_jobs.active_revision(&project_id);
    let previous_route = state
        .runtime
        .config()
        .proxy_route_registry
        .status(&deployment.route_id)
        .await
        .filter(|route| route.status != plurora_runtime::ProxyRouteStatusKind::Removed);
    update_deployment_record(&state, &change_set_id, |current| {
        anyhow::ensure!(
            current.deployment_id == deployment.deployment_id
                && current.status == DevelopmentDeploymentStatus::Approved
                && current.approval_ref.as_ref() == Some(&approval_ref),
            "deployment approval changed before activation"
        );
        current.status = DevelopmentDeploymentStatus::Activating;
        current.error = None;
        Ok(())
    })
    .await
    .map_err(|error| {
        development_persistence_error("failed to persist deployment activation start", error)
    })?;

    let mut revision_committed = false;
    let activation = async {
        let route = call_host_protocol(
            &state,
            &activation_context,
            "host.proxy.register",
            json!({
                "route_id": deployment.route_id,
                "protocol": "http",
                "access": deployment.route_access,
                "upstream": {
                    "port_lease_id": preview.port_lease_id,
                    "port_name": deployment.port_name,
                },
            }),
        )
        .await
        .and_then(|value| value_field(value, "route", "host.proxy.register"))?;
        let route_id = required_string(&route, "id", "deployment activation route")?;
        anyhow::ensure!(
            route_id == deployment.route_id,
            "deployment route identity changed during activation"
        );
        let fallback_public_url =
            required_string(&route, "public_url", "deployment activation route")?;
        ensure_preview_route_ready(&state, &deployment, &preview).await?;
        anyhow::ensure!(
            state
                .runtime
                .config()
                .proxy_route_registry
                .set_ready_if_active_with_lease(&route_id, &preview.port_lease_id, true)
                .await
                .is_some(),
            "deployment route changed before readiness promotion"
        );
        let public_url = service_public_url_for_route(
            &state,
            &route_id,
            &fallback_public_url,
            deployment.route_access,
        );
        let receipt = HostBuildDeployResponse {
            route_id: route_id.clone(),
            public_url,
            route_access: deployment.route_access,
            port_lease_id: preview.port_lease_id.clone(),
            container_id: preview.container_id.clone(),
            container_name: preview.container_name.clone(),
            image: preview.image_id.clone(),
            build_id: deployment.build_id.clone(),
            source_commit: deployment.source_tree_digest.clone(),
            build_descriptor_hash: deployment.build_descriptor_hash.clone(),
            strategy: "verified_artifact".to_string(),
            runtime_env: Vec::new(),
            runtime_mounts: Vec::new(),
            warnings: Vec::new(),
        };
        let revision = DeploymentRevision {
            revision_id: format!(
                "drv-{}-{}",
                now_millis(),
                &uuid::Uuid::new_v4().simple().to_string()[..12]
            ),
            project_id: project_id.clone(),
            job_id: None,
            operation: DeploymentOperation::VerifiedActivate,
            parent_revision_id: previous_revision
                .as_ref()
                .map(|revision| revision.revision_id.clone()),
            created_at_ms: now_millis(),
            target_id: deployment.target_id.clone(),
            source_kind: DeploymentSourceKind::VerifiedArtifact,
            source_url: format!("artifact:{}", deployment.build_context_ref.digest),
            ref_name: change_set_id.clone(),
            dockerfile: Some(deployment.dockerfile.clone()),
            container_port: deployment.container_port,
            port_name: deployment.port_name.clone(),
            route_id,
            route_access: deployment.route_access,
            health_path: deployment.health_path.clone(),
            image: preview.image_id.clone(),
            build_id: deployment.build_id.clone(),
            source_commit: deployment.source_tree_digest.clone(),
            build_descriptor_hash: deployment.build_descriptor_hash.clone(),
            strategy: "verified_artifact".to_string(),
            runtime_env: Vec::new(),
            verified_change_set_id: Some(change_set_id.clone()),
            verification_ref: Some(deployment.verification_ref.clone()),
            build_context_ref: Some(deployment.build_context_ref.clone()),
            preview_ref: Some(preview_ref.clone()),
            approval_ref: Some(approval_ref.clone()),
            verified_build_network_mode: Some(development_target_network_mode(
                deployment.network_mode,
            )),
            target_deployment: Some(preview.deployment.clone()),
            recoverable: true,
            recovery_blockers: Vec::new(),
            receipt,
        };
        persist_revision_activation(&state, &revision, None, Some(authority.clone())).await?;
        revision_committed = true;
        let updated = update_deployment_record(&state, &change_set_id, |current| {
            anyhow::ensure!(
                current.deployment_id == deployment.deployment_id
                    && current.status == DevelopmentDeploymentStatus::Activating,
                "deployment changed before activation persistence"
            );
            current.status = DevelopmentDeploymentStatus::Active;
            current.activation_revision_id = Some(revision.revision_id.clone());
            current.previous_revision_id = previous_revision
                .as_ref()
                .map(|previous| previous.revision_id.clone());
            current.error = None;
            Ok(())
        })
        .await?;
        Ok::<_, anyhow::Error>((updated, revision))
    }
    .await;

    match activation {
        Ok((updated, revision)) => {
            if let Some(previous) = previous_revision.as_ref() {
                for warning in drain_previous_revision(&state, previous, &revision.route_id).await {
                    tracing::warn!(project_id = %project_id, revision_id = %previous.revision_id, warning, "previous deployment revision cleanup incomplete");
                }
            }
            Ok(Json(updated))
        }
        Err(error) => {
            tracing::warn!(project_id = %project_id, change_set_id, error = %error, revision_committed, "development deployment activation failed");
            if revision_committed {
                if let Some(active) =
                    state
                        .build_jobs
                        .active_revision(&project_id)
                        .filter(|active| {
                            active.operation == DeploymentOperation::VerifiedActivate
                                && active.verified_change_set_id.as_deref()
                                    == Some(change_set_id.as_str())
                                && active.target_deployment.as_ref() == Some(&preview.deployment)
                                && active.receipt.port_lease_id == preview.port_lease_id
                        })
                {
                    match update_deployment_record(&state, &change_set_id, |current| {
                        anyhow::ensure!(
                            current.deployment_id == deployment.deployment_id
                                && current.status == DevelopmentDeploymentStatus::Activating,
                            "deployment changed after durable activation"
                        );
                        current.status = DevelopmentDeploymentStatus::Active;
                        current.activation_revision_id = Some(active.revision_id.clone());
                        current.previous_revision_id = previous_revision
                            .as_ref()
                            .map(|previous| previous.revision_id.clone());
                        current.error = None;
                        Ok(())
                    })
                    .await
                    {
                        Ok(updated) => {
                            if let Some(previous) = previous_revision.as_ref() {
                                for warning in
                                    drain_previous_revision(&state, previous, &active.route_id)
                                        .await
                                {
                                    tracing::warn!(project_id = %project_id, revision_id = %previous.revision_id, warning, "previous deployment revision cleanup incomplete");
                                }
                            }
                            return Ok(Json(updated));
                        }
                        Err(persist_error) => {
                            tracing::warn!(project_id = %project_id, change_set_id, error = %persist_error, "failed to reconcile development record after durable activation");
                        }
                    }
                }
            } else {
                let compensated = compensate_deployment_activation_route(
                    &state,
                    &deployment,
                    &preview,
                    previous_route.as_ref(),
                )
                .await;
                if let Err(persist_error) = update_deployment_record(
                    &state,
                    &change_set_id,
                    |current| {
                        if current.deployment_id == deployment.deployment_id
                            && current.status == DevelopmentDeploymentStatus::Activating
                        {
                            current.status = if compensated {
                                DevelopmentDeploymentStatus::Approved
                            } else {
                                DevelopmentDeploymentStatus::RecoveryRequired
                            };
                            current.error = (!compensated).then(|| {
                                "deployment activation requires explicit reconciliation; details redacted"
                                    .to_string()
                            });
                        }
                        Ok(())
                    },
                )
                .await
                {
                    tracing::warn!(project_id = %project_id, change_set_id, error = %persist_error, "failed to persist deployment activation compensation state");
                }
            }
            Err(ServiceError::with_status(
                StatusCode::INTERNAL_SERVER_ERROR,
                "deployment activation failed; details redacted",
            ))
        }
    }
}

async fn reconcile_deployment<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path((project_id, change_set_id)): Path<(String, String)>,
) -> Result<Json<DevelopmentChangeRecord>, ServiceError>
where
    S: EventStore,
{
    ensure_development_host_lease(&state).await?;
    let project_id = parse_project_id(&project_id)?;
    require_identity_project(&identity, project_id.as_str())?;
    refresh_development_project(&state, &project_id).await?;
    let initial = change_for_project(&state, &project_id, &change_set_id)?;
    let initial_deployment = initial.deployment.as_ref().ok_or_else(|| {
        ServiceError::with_status(
            StatusCode::CONFLICT,
            "development change has no deployment to reconcile",
        )
    })?;
    require_identity_target(&identity, &initial_deployment.target_id)?;
    if initial_deployment.status != DevelopmentDeploymentStatus::RecoveryRequired {
        if matches!(
            initial_deployment.status,
            DevelopmentDeploymentStatus::Approved
                | DevelopmentDeploymentStatus::Rejected
                | DevelopmentDeploymentStatus::Active
                | DevelopmentDeploymentStatus::Failed
        ) {
            return Ok(Json(initial));
        }
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "development deployment does not require reconciliation",
        ));
    }
    let initial_deployment_id = initial_deployment.deployment_id.clone();
    let initial_target_id = initial_deployment.target_id.clone();

    let permit = state
        .build_jobs
        .acquire_project_operation(&project_id)
        .await
        .map_err(|error| ServiceError::with_status(StatusCode::CONFLICT, error.to_string()))?;
    let _project_guard = BuildDeployProjectGuard {
        registry: state.build_jobs.clone(),
        project_id: project_id.clone(),
    };
    let _permit = permit;

    refresh_development_project(&state, &project_id).await?;
    let record = change_for_project(&state, &project_id, &change_set_id)?;
    let deployment = record.deployment.clone().ok_or_else(|| {
        ServiceError::with_status(
            StatusCode::CONFLICT,
            "development deployment disappeared during reconciliation",
        )
    })?;
    if deployment.deployment_id != initial_deployment_id
        || deployment.target_id != initial_target_id
    {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "development deployment changed before reconciliation began",
        ));
    }
    if deployment.status != DevelopmentDeploymentStatus::RecoveryRequired {
        if matches!(
            deployment.status,
            DevelopmentDeploymentStatus::Approved
                | DevelopmentDeploymentStatus::Rejected
                | DevelopmentDeploymentStatus::Active
                | DevelopmentDeploymentStatus::Failed
        ) {
            return Ok(Json(record));
        }
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "development deployment changed before reconciliation began",
        ));
    }

    let authority = DeploymentAuthorityLease::from_identity(
        format!("dop-{}", uuid::Uuid::new_v4().simple()),
        deployment.target_id.clone(),
        &identity,
    );
    deployment_effect_context(
        &state,
        Some(&authority),
        &project_id,
        "host_development_deployment_reconcile",
    )
    .await
    .map_err(|error| {
        tracing::warn!(project_id = %project_id, change_set_id, error = %error, "deployment reconciliation authority validation failed");
        ServiceError::with_status(
            StatusCode::FORBIDDEN,
            "deployment reconciliation authority is no longer valid",
        )
    })?;

    let active_revision = state.build_jobs.active_revision(&project_id);
    if let Some(active) = active_revision
        .as_ref()
        .filter(|active| verified_activation_matches_deployment(active, &record, &deployment))
    {
        let updated = update_deployment_record(&state, &change_set_id, |current| {
            anyhow::ensure!(
                current.deployment_id == deployment.deployment_id
                    && current.status == DevelopmentDeploymentStatus::RecoveryRequired,
                "deployment changed before durable activation adoption"
            );
            current.status = DevelopmentDeploymentStatus::Active;
            current.activation_revision_id = Some(active.revision_id.clone());
            current.previous_revision_id = active.parent_revision_id.clone();
            current.error = None;
            Ok(())
        })
        .await
        .map_err(|error| {
            development_persistence_error(
                "failed to adopt the durable deployment activation",
                error,
            )
        })?;
        if let Some(previous) = active
            .parent_revision_id
            .as_deref()
            .and_then(|revision_id| state.build_jobs.revision(&project_id, revision_id))
        {
            for warning in drain_previous_revision(&state, &previous, &active.route_id).await {
                tracing::warn!(project_id = %project_id, revision_id = %previous.revision_id, warning, "previous deployment revision cleanup incomplete after reconciliation");
            }
        }
        return Ok(Json(updated));
    }
    if active_revision
        .as_ref()
        .is_some_and(|active| durable_revision_claims_deployment_candidate(active, &deployment))
    {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "the durable active revision claims this candidate but its provenance is inconsistent; manual reconciliation is required",
        ));
    }

    if deployment
        .approval_decision
        .as_ref()
        .is_some_and(|decision| decision.outcome == PolicyDecisionOutcome::Allowed)
    {
        let preview = deployment.preview.as_ref().ok_or_else(|| {
            ServiceError::with_status(
                StatusCode::CONFLICT,
                "approved deployment reconciliation has no candidate receipt",
            )
        })?;
        let previous_route = active_revision
            .as_ref()
            .filter(|previous| previous.route_id == deployment.route_id)
            .map(|previous| {
                (
                    previous.receipt.port_lease_id.as_str(),
                    previous.port_name.as_str(),
                    previous.route_access,
                    false,
                )
            });
        if !compensate_route_alias_to(
            &state,
            &deployment.route_id,
            &preview.port_lease_id,
            previous_route,
            "host_development_deployment_reconcile_activation",
        )
        .await
        {
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "deployment activation route still requires reconciliation",
            ));
        }
    }

    if deployment.preview_port_lease_id.is_none()
        && unrecorded_preview_port_lease_may_exist(&state, &deployment).await
    {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "deployment port allocation has no durable identity; inspect unclaimed target leases before retrying reconciliation",
        ));
    }

    if !stop_preview_candidate_for_reconciliation(&state, &record.project_id, &deployment).await?
        || !cleanup_preview_host_resources(&state, &deployment)
            .await
            .map_err(|error| {
                tracing::warn!(project_id = %project_id, change_set_id, error = %error, "deployment reconciliation cleanup failed");
                ServiceError::with_status(
                    StatusCode::CONFLICT,
                    "deployment candidate cleanup still requires reconciliation",
                )
            })?
    {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "deployment candidate cleanup still requires reconciliation",
        ));
    }

    let rejected = deployment
        .approval_decision
        .as_ref()
        .is_some_and(|decision| decision.outcome == PolicyDecisionOutcome::Denied);
    let updated = update_deployment_record(&state, &change_set_id, |current| {
        anyhow::ensure!(
            current.deployment_id == deployment.deployment_id
                && current.status == DevelopmentDeploymentStatus::RecoveryRequired,
            "deployment changed before reconciliation persistence"
        );
        current.status = if rejected {
            DevelopmentDeploymentStatus::Rejected
        } else {
            DevelopmentDeploymentStatus::Failed
        };
        current.error = (!rejected).then(|| {
            "deployment candidate was conservatively cleaned after reconciliation".to_string()
        });
        Ok(())
    })
    .await
    .map_err(|error| {
        development_persistence_error("failed to persist deployment reconciliation", error)
    })?;
    Ok(Json(updated))
}

fn development_target_network_mode(
    mode: DevelopmentNetworkMode,
) -> plurora_runtime::ManagedTargetBuildNetworkMode {
    match mode {
        DevelopmentNetworkMode::None => plurora_runtime::ManagedTargetBuildNetworkMode::None,
        DevelopmentNetworkMode::Bridge => plurora_runtime::ManagedTargetBuildNetworkMode::Bridge,
    }
}

fn verified_activation_matches_deployment(
    active: &DeploymentRevision,
    record: &DevelopmentChangeRecord,
    deployment: &DevelopmentDeploymentRecord,
) -> bool {
    let (
        Some(preview),
        Some(preview_ref),
        Some(approval_ref),
        Some(port_lease_id),
        Some(target_deployment_id),
    ) = (
        deployment.preview.as_ref(),
        deployment.preview_ref.as_ref(),
        deployment.approval_ref.as_ref(),
        deployment.preview_port_lease_id.as_deref(),
        deployment.target_deployment_id.as_deref(),
    )
    else {
        return false;
    };
    deployment
        .approval_decision
        .as_ref()
        .is_some_and(|decision| {
            decision.outcome == PolicyDecisionOutcome::Allowed
                && decision.principal == PrincipalIdentity::HostAdmin
                && decision.change_set_id
                    == format!(
                        "{}:deployment:{}",
                        record.change_set.id, deployment.deployment_id
                    )
        })
        && preview.route_id == deployment.preview_route_id
        && preview.port_lease_id == port_lease_id
        && preview.deployment.deployment_id == target_deployment_id
        && preview.deployment.route_id == deployment.preview_route_id
        && preview.deployment.port_lease_id == port_lease_id
        && deployment.build_operation_id.as_deref() == Some(preview.build_operation_id.as_str())
        && deployment.deployment_operation_id.as_deref()
            == Some(preview.deployment_operation_id.as_str())
        && active.project_id == record.project_id
        && active.job_id.is_none()
        && active.operation == DeploymentOperation::VerifiedActivate
        && active.source_kind == DeploymentSourceKind::VerifiedArtifact
        && active.verified_change_set_id.as_deref() == Some(record.change_set.id.as_str())
        && active.source_url == format!("artifact:{}", deployment.build_context_ref.digest)
        && active.ref_name == record.change_set.id
        && active.target_id == deployment.target_id
        && active.target_deployment.as_ref() == Some(&preview.deployment)
        && active.verification_ref.as_ref() == Some(&deployment.verification_ref)
        && active.build_context_ref.as_ref() == Some(&deployment.build_context_ref)
        && active.preview_ref.as_ref() == Some(preview_ref)
        && active.approval_ref.as_ref() == Some(approval_ref)
        && active.verified_build_network_mode
            == Some(development_target_network_mode(deployment.network_mode))
        && active.source_commit == deployment.source_tree_digest
        && active.dockerfile.as_deref() == Some(deployment.dockerfile.as_str())
        && active.container_port == deployment.container_port
        && active.port_name == deployment.port_name
        && active.route_id == deployment.route_id
        && active.route_access == deployment.route_access
        && active.health_path == deployment.health_path
        && active.image == preview.image_id
        && active.build_id == deployment.build_id
        && active.build_descriptor_hash == deployment.build_descriptor_hash
        && active.strategy == "verified_artifact"
        && active.runtime_env.is_empty()
        && active.recoverable
        && active.recovery_blockers.is_empty()
        && active.receipt.route_id == deployment.route_id
        && active.receipt.route_access == deployment.route_access
        && active.receipt.port_lease_id == preview.port_lease_id
        && active.receipt.container_id == preview.container_id
        && active.receipt.container_name == preview.container_name
        && active.receipt.image == preview.image_id
        && active.receipt.build_id == deployment.build_id
        && active.receipt.source_commit == deployment.source_tree_digest
        && active.receipt.build_descriptor_hash == deployment.build_descriptor_hash
        && active.receipt.strategy == "verified_artifact"
        && active.receipt.runtime_env.is_empty()
        && active.receipt.runtime_mounts.is_empty()
        && active.receipt.warnings.is_empty()
}

fn durable_revision_claims_deployment_candidate(
    active: &DeploymentRevision,
    deployment: &DevelopmentDeploymentRecord,
) -> bool {
    let Some(preview) = deployment.preview.as_ref() else {
        return false;
    };
    active.target_id == deployment.target_id
        && (active.target_deployment.as_ref() == Some(&preview.deployment)
            || active.receipt.port_lease_id == preview.port_lease_id
            || active.receipt.container_id == preview.container_id)
}

fn validate_preview_target_operations<S>(
    state: &AppState<S>,
    project_id: &ProjectId,
    deployment: &DevelopmentDeploymentRecord,
    preview: &DevelopmentDeploymentPreview,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    let build = state
        .target_agents
        .operation(&preview.build_operation_id)
        .ok_or_else(|| anyhow::anyhow!("target build operation disappeared"))?;
    require_succeeded_target_operation(build.clone(), "target Docker build")?;
    anyhow::ensure!(
        build.target_id == deployment.target_id && build.project_id == *project_id,
        "target build operation belongs to another project or target"
    );
    let expected_verifier = DeclarativeVerifierDescriptor::DockerBuild {
        digest: deployment.build_context_ref.digest.clone(),
        expected_size_bytes: Some(deployment.build_context_ref.size_bytes),
        dockerfile: deployment.dockerfile.clone(),
        network_mode: development_target_network_mode(deployment.network_mode),
        build_id: deployment.build_id.clone(),
        source_tree_digest: deployment.source_tree_digest.clone(),
        build_descriptor_hash: deployment.build_descriptor_hash.clone(),
    };
    anyhow::ensure!(
        build.spec
            == TargetOperationSpec::VerifierRun {
                verifier: expected_verifier,
            },
        "target build operation does not match verified provenance"
    );
    let build_output = &build
        .receipt
        .as_ref()
        .expect("succeeded operation receipt")
        .output;
    anyhow::ensure!(
        required_string(build_output, "image_id", "target Docker build receipt")?
            == preview.image_id
            && required_string(
                build_output,
                "context_digest",
                "target Docker build receipt"
            )? == deployment.build_context_ref.digest
            && required_string(
                build_output,
                "source_tree_digest",
                "target Docker build receipt"
            )? == deployment.source_tree_digest
            && required_string(
                build_output,
                "build_descriptor_hash",
                "target Docker build receipt"
            )? == deployment.build_descriptor_hash,
        "target build receipt does not match preview provenance"
    );

    let apply = state
        .target_agents
        .operation(&preview.deployment_operation_id)
        .ok_or_else(|| anyhow::anyhow!("target deployment operation disappeared"))?;
    require_succeeded_target_operation(apply.clone(), "target deployment apply")?;
    anyhow::ensure!(
        apply.target_id == deployment.target_id
            && apply.project_id == *project_id
            && apply.spec
                == TargetOperationSpec::DeploymentApply {
                    deployment: TargetDeploymentDescriptor {
                        deployment: preview.deployment.clone(),
                        port_name: deployment.port_name.clone(),
                        image: preview.image_id.clone(),
                        container_port: deployment.container_port,
                        requested_host_port: None,
                        pull_if_missing: false,
                        health_path: deployment.health_path.clone(),
                    },
                },
        "target deployment operation does not match the preview candidate"
    );
    let apply_output = &apply
        .receipt
        .as_ref()
        .expect("succeeded operation receipt")
        .output;
    anyhow::ensure!(
        apply_output.get("running").and_then(Value::as_bool) == Some(true)
            && required_string(apply_output, "container_id", "target deployment receipt")?
                == preview.container_id
            && required_string(apply_output, "image_id", "target deployment receipt")?
                == preview.image_id,
        "target deployment receipt does not match the preview candidate"
    );
    Ok(())
}

async fn ensure_preview_route_ready<S>(
    state: &AppState<S>,
    deployment: &DevelopmentDeploymentRecord,
    preview: &DevelopmentDeploymentPreview,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    let route = state
        .runtime
        .config()
        .proxy_route_registry
        .status(&preview.route_id)
        .await
        .ok_or_else(|| anyhow::anyhow!("deployment preview route disappeared"))?;
    anyhow::ensure!(
        preview.route_id == deployment.preview_route_id
            && preview.port_lease_id == preview.deployment.port_lease_id
            && preview.route_id == preview.deployment.route_id
            && route.status == plurora_runtime::ProxyRouteStatusKind::Active
            && route.ready
            && route.upstream.port_lease_id == preview.port_lease_id
            && route.upstream.port_name == deployment.port_name,
        "deployment preview route is not ready for the recorded candidate"
    );
    Ok(())
}

async fn compensate_deployment_activation_route<S>(
    state: &AppState<S>,
    deployment: &DevelopmentDeploymentRecord,
    preview: &DevelopmentDeploymentPreview,
    previous_route: Option<&plurora_runtime::ProxyRouteRecord>,
) -> bool
where
    S: EventStore,
{
    compensate_route_alias(
        state,
        &deployment.route_id,
        &preview.port_lease_id,
        previous_route,
        "host_development_deployment_activation_rollback",
    )
    .await
}

async fn compensate_route_alias<S>(
    state: &AppState<S>,
    route_id: &str,
    candidate_lease_id: &str,
    previous_route: Option<&plurora_runtime::ProxyRouteRecord>,
    transport: &str,
) -> bool
where
    S: EventStore,
{
    compensate_route_alias_to(
        state,
        route_id,
        candidate_lease_id,
        previous_route.map(|previous| {
            (
                previous.upstream.port_lease_id.as_str(),
                previous.upstream.port_name.as_str(),
                previous.access,
                previous.ready,
            )
        }),
        transport,
    )
    .await
}

async fn compensate_route_alias_to<S>(
    state: &AppState<S>,
    route_id: &str,
    candidate_lease_id: &str,
    previous_route: Option<(&str, &str, ProxyRouteAccess, bool)>,
    transport: &str,
) -> bool
where
    S: EventStore,
{
    let current = state
        .runtime
        .config()
        .proxy_route_registry
        .status(route_id)
        .await;
    if current.as_ref().is_none_or(|route| {
        route.status == plurora_runtime::ProxyRouteStatusKind::Removed
            || route.upstream.port_lease_id != candidate_lease_id
    }) {
        return true;
    }
    if let Some((previous_lease_id, previous_port_name, previous_access, previous_ready)) =
        previous_route
    {
        return match restore_proxy_route_if_candidate_active(
            state,
            route_id,
            candidate_lease_id,
            previous_lease_id,
            previous_port_name,
            previous_access,
            previous_ready,
            transport,
        )
        .await
        {
            Ok(true) => true,
            Ok(false) => state
                .runtime
                .config()
                .proxy_route_registry
                .status(route_id)
                .await
                .is_none_or(|route| {
                    route.status == plurora_runtime::ProxyRouteStatusKind::Removed
                        || route.upstream.port_lease_id != candidate_lease_id
                }),
            Err(_) => {
                unregister_route_alias_if_candidate(state, route_id, candidate_lease_id, transport)
                    .await
            }
        };
    }
    unregister_route_alias_if_candidate(state, route_id, candidate_lease_id, transport).await
}

async fn unregister_route_alias_if_candidate<S>(
    state: &AppState<S>,
    route_id: &str,
    candidate_lease_id: &str,
    transport: &str,
) -> bool
where
    S: EventStore,
{
    let current = state
        .runtime
        .config()
        .proxy_route_registry
        .status(route_id)
        .await;
    if current.as_ref().is_none_or(|route| {
        route.status == plurora_runtime::ProxyRouteStatusKind::Removed
            || route.upstream.port_lease_id != candidate_lease_id
    }) {
        return true;
    }
    let context = ProtocolContext::host_dev(transport);
    if call_host_protocol(
        state,
        &context,
        "host.proxy.unregister",
        json!({ "route_id": route_id }),
    )
    .await
    .is_err()
    {
        return false;
    }
    state
        .runtime
        .config()
        .proxy_route_registry
        .status(route_id)
        .await
        .is_none_or(|route| route.status == plurora_runtime::ProxyRouteStatusKind::Removed)
}

pub(crate) async fn activate_verified_persisted_revision<S>(
    state: &AppState<S>,
    previous: Option<&DeploymentRevision>,
    target: &DeploymentRevision,
    operation: DeploymentOperation,
    authority: &DeploymentAuthorityLease,
) -> Result<DeploymentActionResponse, ServiceError>
where
    S: EventStore,
{
    let invalid_revision = || {
        ServiceError::with_status(
            StatusCode::CONFLICT,
            "verified deployment revision has incomplete or inconsistent provenance",
        )
    };
    if authority.target_id != target.target_id
        || !matches!(
            operation,
            DeploymentOperation::Recover | DeploymentOperation::Rollback
        )
        || target.strategy != "verified_artifact"
        || !target.runtime_env.is_empty()
    {
        return Err(invalid_revision());
    }
    let change_set_id = target
        .verified_change_set_id
        .clone()
        .ok_or_else(invalid_revision)?;
    let verification_ref = target
        .verification_ref
        .clone()
        .ok_or_else(invalid_revision)?;
    let build_context_ref = target
        .build_context_ref
        .clone()
        .ok_or_else(invalid_revision)?;
    let preview_ref = target.preview_ref.clone().ok_or_else(invalid_revision)?;
    let approval_ref = target.approval_ref.clone().ok_or_else(invalid_revision)?;
    let dockerfile = target.dockerfile.clone().ok_or_else(invalid_revision)?;
    let network_mode = target
        .verified_build_network_mode
        .ok_or_else(invalid_revision)?;
    let expected_descriptor_hash = deployment_build_descriptor_hash(
        &target.project_id,
        &build_context_ref,
        &target.source_commit,
        &dockerfile,
        target_development_network_mode(network_mode),
        &target.build_id,
    );
    let provenance_valid = verification_ref.artifact_type_uri
        == DEVELOPMENT_RESULT_ARTIFACT_TYPE_URI
        && verification_ref.media_type == "application/json"
        && verification_ref
            .references
            .contains(&build_context_ref.digest)
        && build_context_ref.artifact_type_uri == DEVELOPMENT_BUILD_CONTEXT_ARTIFACT_TYPE_URI
        && build_context_ref.media_type == "application/x-tar"
        && build_context_ref
            .annotations
            .get("project_id")
            .and_then(Value::as_str)
            == Some(target.project_id.as_str())
        && build_context_ref
            .annotations
            .get("change_set_id")
            .and_then(Value::as_str)
            == Some(change_set_id.as_str())
        && build_context_ref
            .annotations
            .get("tree_digest")
            .and_then(Value::as_str)
            == Some(target.source_commit.as_str())
        && build_context_ref
            .annotations
            .get("dockerfile")
            .and_then(Value::as_str)
            == Some(dockerfile.as_str())
        && target.build_descriptor_hash == expected_descriptor_hash
        && target.receipt.route_id == target.route_id
        && target.receipt.route_access == target.route_access
        && target.receipt.build_id == target.build_id
        && target.receipt.source_commit == target.source_commit
        && target.receipt.build_descriptor_hash == target.build_descriptor_hash;
    if !provenance_valid {
        return Err(invalid_revision());
    }
    for descriptor in [&verification_ref, &build_context_ref] {
        verify_deployment_artifact_content(state.runtime.as_ref(), descriptor).await?;
    }
    let (evidence_deployment_id, _evidence_preview, evidence_authority_ref) =
        read_verified_preview_evidence(
            state.runtime.as_ref(),
            &preview_ref,
            &target.project_id,
            &change_set_id,
            &target.target_id,
            &target.source_commit,
            &verification_ref,
            &build_context_ref,
        )
        .await
        .map_err(|error| {
            tracing::warn!(project_id = %target.project_id, revision_id = %target.revision_id, error = %error, "verified deployment preview evidence validation failed");
            invalid_revision()
        })?;
    read_verified_deployment_approval(
        state.runtime.as_ref(),
        &approval_ref,
        &preview_ref,
        &verification_ref,
        &build_context_ref,
        &evidence_authority_ref,
        &target.project_id,
        &change_set_id,
        &evidence_deployment_id,
        &target.target_id,
    )
    .await
    .map_err(|error| {
        tracing::warn!(project_id = %target.project_id, revision_id = %target.revision_id, error = %error, "verified deployment approval evidence validation failed");
        invalid_revision()
    })?;
    let replay_context = deployment_effect_context(
        state,
        Some(authority),
        &target.project_id,
        "host_verified_deployment_replay",
    )
    .await
    .map_err(|error| {
        tracing::warn!(project_id = %target.project_id, revision_id = %target.revision_id, error = %error, "verified deployment replay authority validation failed");
        ServiceError::with_status(
            StatusCode::FORBIDDEN,
            "deployment authority is no longer valid for the revision target",
        )
    })?;
    state
        .build_jobs
        .ensure_route_available_for_project(&target.route_id, &target.project_id)
        .map_err(|error| ServiceError::with_status(StatusCode::CONFLICT, error.to_string()))?;
    if state
        .target_agents
        .project_for_operation_route(&target.route_id)
        .is_some_and(|owner| owner != target.project_id)
    {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "deployment route is owned by another project",
        ));
    }

    let replay_suffix = uuid::Uuid::new_v4().simple().to_string();
    let build_operation = crate::target_agent::submit_host_operation(
        state,
        &target.target_id,
        CreateTargetOperationRequest {
            project_id: target.project_id.clone(),
            spec: TargetOperationSpec::VerifierRun {
                verifier: DeclarativeVerifierDescriptor::DockerBuild {
                    digest: build_context_ref.digest.clone(),
                    expected_size_bytes: Some(build_context_ref.size_bytes),
                    dockerfile: dockerfile.clone(),
                    network_mode,
                    build_id: target.build_id.clone(),
                    source_tree_digest: target.source_commit.clone(),
                    build_descriptor_hash: target.build_descriptor_hash.clone(),
                },
            },
            idempotency_key: Some(format!("{replay_suffix}:build")),
            expires_in_seconds: Some(15 * 60),
        },
    )
    .await
    .map_err(|error| error.error)?;
    let build_operation = await_target_operation(state, &target.target_id, build_operation)
        .await
        .and_then(|operation| require_succeeded_target_operation(operation, "target Docker build"))
        .map_err(|error| {
            tracing::warn!(project_id = %target.project_id, revision_id = %target.revision_id, error = %error, "verified deployment artifact rebuild failed");
            ServiceError::with_status(
                StatusCode::CONFLICT,
                "verified deployment artifact could not be rebuilt on the selected target",
            )
        })?;
    let build_output = &build_operation
        .receipt
        .as_ref()
        .expect("succeeded target operation has a receipt")
        .output;
    let _image = required_string(build_output, "image", "target Docker build receipt")?;
    let image_id = required_string(build_output, "image_id", "target Docker build receipt")?;
    let output_matches = required_string(
        build_output,
        "context_digest",
        "target Docker build receipt",
    )? == build_context_ref.digest
        && required_string(
            build_output,
            "source_tree_digest",
            "target Docker build receipt",
        )? == target.source_commit
        && required_string(
            build_output,
            "build_descriptor_hash",
            "target Docker build receipt",
        )? == target.build_descriptor_hash
        && required_string(build_output, "build_id", "target Docker build receipt")?
            == target.build_id;
    if !output_matches {
        return Err(invalid_revision());
    }

    let lease = call_host_protocol(
        state,
        &replay_context,
        "host.port.lease",
        json!({
            "target_id": target.target_id,
            "port_name": target.port_name,
            "protocol": "tcp",
        }),
    )
    .await
    .and_then(|value| value_field(value, "lease", "host.port.lease"))?;
    let port_lease_id = required_string(&lease, "id", "verified replay port lease")?;
    let management_route_id = format!("replay-{replay_suffix}");
    let target_deployment = TargetDeploymentRef {
        deployment_id: format!("replay-{replay_suffix}"),
        route_id: management_route_id.clone(),
        port_lease_id: port_lease_id.clone(),
    };
    let management_route = call_host_protocol(
        state,
        &replay_context,
        "host.proxy.register",
        json!({
            "route_id": management_route_id,
            "protocol": "http",
            "access": ProxyRouteAccess::HostAuthenticated,
            "upstream": {
                "port_lease_id": port_lease_id,
                "port_name": target.port_name,
            },
        }),
    )
    .await
    .and_then(|value| value_field(value, "route", "host.proxy.register"))
    .and_then(|route| {
        anyhow::ensure!(
            required_string(&route, "id", "verified replay management route")?
                == management_route_id,
            "verified replay management route identity changed"
        );
        Ok(route)
    });
    if let Err(error) = management_route {
        tracing::warn!(project_id = %target.project_id, revision_id = %target.revision_id, error = %error, "verified replay management route registration failed");
        let _ = cleanup_target_host_resources(state, &target_deployment, &target.route_id).await;
        return Err(ServiceError::with_status(
            StatusCode::INTERNAL_SERVER_ERROR,
            "deployment replay preparation failed; details redacted",
        ));
    }

    let apply_operation = crate::target_agent::submit_host_operation(
        state,
        &target.target_id,
        CreateTargetOperationRequest {
            project_id: target.project_id.clone(),
            spec: TargetOperationSpec::DeploymentApply {
                deployment: TargetDeploymentDescriptor {
                    deployment: target_deployment.clone(),
                    port_name: target.port_name.clone(),
                    image: image_id.clone(),
                    container_port: target.container_port,
                    requested_host_port: None,
                    pull_if_missing: false,
                    health_path: target.health_path.clone(),
                },
            },
            idempotency_key: Some(format!("{replay_suffix}:apply")),
            expires_in_seconds: Some(15 * 60),
        },
    )
    .await;
    let apply_operation = match apply_operation {
        Ok(operation) => operation,
        Err(error) => {
            tracing::warn!(project_id = %target.project_id, revision_id = %target.revision_id, error = %error.error, "verified replay candidate submission failed");
            let _ =
                cleanup_target_host_resources(state, &target_deployment, &target.route_id).await;
            return Err(ServiceError::with_status(
                StatusCode::INTERNAL_SERVER_ERROR,
                "deployment replay candidate failed; details redacted",
            ));
        }
    };
    let apply_operation_id = apply_operation.operation_id.clone();
    let apply_operation = match await_target_operation(state, &target.target_id, apply_operation)
        .await
    {
        Ok(operation) => operation,
        Err(error) => {
            tracing::warn!(project_id = %target.project_id, revision_id = %target.revision_id, error = %error, "verified replay candidate outcome is unknown");
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "deployment replay candidate requires reconciliation; details redacted",
            ));
        }
    };
    let apply_operation = match require_succeeded_target_operation(
        apply_operation,
        "target deployment apply",
    ) {
        Ok(operation) => operation,
        Err(error) => {
            tracing::warn!(project_id = %target.project_id, revision_id = %target.revision_id, error = %error, "verified replay candidate did not start");
            if !target_operation_outcome_is_uncertain(state, &apply_operation_id) {
                let _ = cleanup_target_host_resources(state, &target_deployment, &target.route_id)
                    .await;
            }
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "deployment replay candidate did not become ready",
            ));
        }
    };
    let apply_output = &apply_operation
        .receipt
        .as_ref()
        .expect("succeeded target operation has a receipt")
        .output;
    let candidate = async {
        let container_id = required_string(
            apply_output,
            "container_id",
            "target deployment apply receipt",
        )?;
        let container_name = apply_output
            .get("container_name")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        anyhow::ensure!(
            apply_output.get("running").and_then(Value::as_bool) == Some(true)
                && required_string(apply_output, "image_id", "target deployment apply receipt")?
                    == image_id,
            "target deployment receipt does not match the replay candidate"
        );
        ensure_target_management_route_ready(state, &target_deployment, &target.port_name).await?;
        Ok::<_, anyhow::Error>((container_id, container_name))
    }
    .await;
    let (container_id, container_name) = match candidate {
        Ok(candidate) => candidate,
        Err(error) => {
            tracing::warn!(project_id = %target.project_id, revision_id = %target.revision_id, error = %error, "verified replay candidate receipt validation failed");
            let _ = compensate_verified_replay_candidate(
                state,
                &target.project_id,
                &target.target_id,
                &target_deployment,
                &target.route_id,
                None,
            )
            .await;
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "deployment replay candidate did not become ready",
            ));
        }
    };

    let previous_route = state
        .runtime
        .config()
        .proxy_route_registry
        .status(&target.route_id)
        .await
        .filter(|route| route.status != plurora_runtime::ProxyRouteStatusKind::Removed);
    let route_switch = async {
        let route = call_host_protocol(
            state,
            &replay_context,
            "host.proxy.register",
            json!({
                "route_id": target.route_id,
                "protocol": "http",
                "access": target.route_access,
                "upstream": {
                    "port_lease_id": port_lease_id,
                    "port_name": target.port_name,
                },
            }),
        )
        .await
        .and_then(|value| value_field(value, "route", "host.proxy.register"))?;
        let route_id = required_string(&route, "id", "verified replay route")?;
        anyhow::ensure!(
            route_id == target.route_id,
            "deployment replay route changed"
        );
        let fallback_public_url = required_string(&route, "public_url", "verified replay route")?;
        ensure_target_management_route_ready(state, &target_deployment, &target.port_name).await?;
        anyhow::ensure!(
            state
                .runtime
                .config()
                .proxy_route_registry
                .set_ready_if_active_with_lease(&route_id, &port_lease_id, true)
                .await
                .is_some(),
            "deployment replay route changed before readiness promotion"
        );
        Ok::<_, anyhow::Error>((route_id, fallback_public_url))
    }
    .await;
    let (route_id, fallback_public_url) = match route_switch {
        Ok(route) => route,
        Err(error) => {
            tracing::warn!(project_id = %target.project_id, revision_id = %target.revision_id, error = %error, "verified deployment replay route switch failed");
            let _ = compensate_verified_replay_candidate(
                state,
                &target.project_id,
                &target.target_id,
                &target_deployment,
                &target.route_id,
                previous_route.as_ref(),
            )
            .await;
            return Err(ServiceError::with_status(
                StatusCode::INTERNAL_SERVER_ERROR,
                "deployment replay activation failed; details redacted",
            ));
        }
    };
    let public_url =
        service_public_url_for_route(state, &route_id, &fallback_public_url, target.route_access);
    let receipt = HostBuildDeployResponse {
        route_id: route_id.clone(),
        public_url,
        route_access: target.route_access,
        port_lease_id: port_lease_id.clone(),
        container_id,
        container_name,
        image: image_id.clone(),
        build_id: target.build_id.clone(),
        source_commit: target.source_commit.clone(),
        build_descriptor_hash: target.build_descriptor_hash.clone(),
        strategy: target.strategy.clone(),
        runtime_env: Vec::new(),
        runtime_mounts: Vec::new(),
        warnings: Vec::new(),
    };
    let revision = DeploymentRevision {
        revision_id: format!(
            "drv-{}-{}",
            now_millis(),
            &uuid::Uuid::new_v4().simple().to_string()[..12]
        ),
        project_id: target.project_id.clone(),
        job_id: None,
        operation,
        parent_revision_id: previous.map(|revision| revision.revision_id.clone()),
        created_at_ms: now_millis(),
        target_id: target.target_id.clone(),
        source_kind: DeploymentSourceKind::VerifiedArtifact,
        source_url: target.source_url.clone(),
        ref_name: target.ref_name.clone(),
        dockerfile: Some(dockerfile),
        container_port: target.container_port,
        port_name: target.port_name.clone(),
        route_id,
        route_access: target.route_access,
        health_path: target.health_path.clone(),
        image: image_id,
        build_id: target.build_id.clone(),
        source_commit: target.source_commit.clone(),
        build_descriptor_hash: target.build_descriptor_hash.clone(),
        strategy: target.strategy.clone(),
        runtime_env: Vec::new(),
        verified_change_set_id: Some(change_set_id),
        verification_ref: Some(verification_ref),
        build_context_ref: Some(build_context_ref),
        preview_ref: Some(preview_ref),
        approval_ref: Some(approval_ref),
        verified_build_network_mode: Some(network_mode),
        target_deployment: Some(target_deployment.clone()),
        recoverable: true,
        recovery_blockers: Vec::new(),
        receipt,
    };
    if let Err(error) =
        persist_revision_activation(state, &revision, None, Some(authority.clone())).await
    {
        tracing::warn!(project_id = %target.project_id, revision_id = %target.revision_id, error = %error, "verified deployment replay journal commit failed");
        let _ = compensate_verified_replay_candidate(
            state,
            &target.project_id,
            &target.target_id,
            &target_deployment,
            &target.route_id,
            previous_route.as_ref(),
        )
        .await;
        return Err(ServiceError::with_status(
            StatusCode::INTERNAL_SERVER_ERROR,
            "deployment replay journal commit failed; details redacted",
        ));
    }
    let warnings = match previous {
        Some(previous) => drain_previous_revision(state, previous, &revision.route_id).await,
        None => Vec::new(),
    };
    Ok(DeploymentActionResponse {
        operation,
        previous_revision_id: previous.map(|revision| revision.revision_id.clone()),
        revision,
        warnings,
    })
}

fn target_development_network_mode(
    mode: plurora_runtime::ManagedTargetBuildNetworkMode,
) -> DevelopmentNetworkMode {
    match mode {
        plurora_runtime::ManagedTargetBuildNetworkMode::None => DevelopmentNetworkMode::None,
        plurora_runtime::ManagedTargetBuildNetworkMode::Bridge => DevelopmentNetworkMode::Bridge,
    }
}

async fn ensure_target_management_route_ready<S>(
    state: &AppState<S>,
    deployment: &TargetDeploymentRef,
    port_name: &str,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    let route = state
        .runtime
        .config()
        .proxy_route_registry
        .status(&deployment.route_id)
        .await
        .ok_or_else(|| anyhow::anyhow!("target deployment management route disappeared"))?;
    anyhow::ensure!(
        route.status == plurora_runtime::ProxyRouteStatusKind::Active
            && route.ready
            && route.upstream.port_lease_id == deployment.port_lease_id
            && route.upstream.port_name == port_name,
        "target deployment management route is not ready"
    );
    Ok(())
}

async fn compensate_verified_replay_candidate<S>(
    state: &AppState<S>,
    project_id: &ProjectId,
    target_id: &str,
    deployment: &TargetDeploymentRef,
    production_route_id: &str,
    previous_route: Option<&plurora_runtime::ProxyRouteRecord>,
) -> bool
where
    S: EventStore,
{
    let route_restored = compensate_route_alias(
        state,
        production_route_id,
        &deployment.port_lease_id,
        previous_route,
        "host_verified_deployment_replay_rollback",
    )
    .await;
    let stop = crate::target_agent::submit_host_operation(
        state,
        target_id,
        CreateTargetOperationRequest {
            project_id: project_id.clone(),
            spec: TargetOperationSpec::DeploymentStop {
                deployment: deployment.clone(),
                grace_seconds: 0,
                force_remove: true,
            },
            idempotency_key: Some(format!("{}:compensate", deployment.deployment_id)),
            expires_in_seconds: Some(15 * 60),
        },
    )
    .await;
    let stopped = match stop {
        Ok(operation) => await_target_operation(state, target_id, operation)
            .await
            .is_ok_and(|operation| operation.status == TargetOperationStatusKind::Succeeded),
        Err(_) => false,
    };
    let host_cleaned = if stopped {
        cleanup_target_host_resources(state, deployment, production_route_id).await
    } else {
        false
    };
    route_restored && stopped && host_cleaned
}

async fn cleanup_target_host_resources<S>(
    state: &AppState<S>,
    deployment: &TargetDeploymentRef,
    protected_route_id: &str,
) -> bool
where
    S: EventStore,
{
    let context = ProtocolContext::host_dev("host_target_deployment_resource_cleanup");
    if deployment.route_id != protected_route_id {
        if let Some(route) = state
            .runtime
            .config()
            .proxy_route_registry
            .status(&deployment.route_id)
            .await
        {
            if route.status != plurora_runtime::ProxyRouteStatusKind::Removed
                && route.upstream.port_lease_id == deployment.port_lease_id
                && call_host_protocol(
                    state,
                    &context,
                    "host.proxy.unregister",
                    json!({ "route_id": deployment.route_id }),
                )
                .await
                .is_err()
            {
                return false;
            }
        }
    }
    if state
        .runtime
        .config()
        .proxy_route_registry
        .list()
        .await
        .into_iter()
        .any(|route| {
            route.status != plurora_runtime::ProxyRouteStatusKind::Removed
                && route.upstream.port_lease_id == deployment.port_lease_id
        })
    {
        return false;
    }
    if state
        .runtime
        .config()
        .port_lease_registry
        .status(&deployment.port_lease_id)
        .await
        .is_some_and(|lease| lease.status != plurora_runtime::PortLeaseStatusKind::Released)
        && call_host_protocol(
            state,
            &context,
            "host.port.release",
            json!({ "lease_id": deployment.port_lease_id }),
        )
        .await
        .is_err()
    {
        return false;
    }
    state
        .runtime
        .config()
        .port_lease_registry
        .status(&deployment.port_lease_id)
        .await
        .is_none_or(|lease| lease.status == plurora_runtime::PortLeaseStatusKind::Released)
}

pub(crate) async fn drain_target_revision<S>(
    state: &AppState<S>,
    previous: &DeploymentRevision,
    active_route_id: &str,
) -> Vec<String>
where
    S: EventStore,
{
    let Some(deployment) = previous.target_deployment.as_ref() else {
        tracing::warn!(project_id = %previous.project_id, revision_id = %previous.revision_id, "verified deployment revision has no target deployment reference for cleanup");
        return vec!["previous target deployment requires manual cleanup".to_string()];
    };
    let stop = crate::target_agent::submit_host_operation(
        state,
        &previous.target_id,
        CreateTargetOperationRequest {
            project_id: previous.project_id.clone(),
            spec: TargetOperationSpec::DeploymentStop {
                deployment: deployment.clone(),
                grace_seconds: 10,
                force_remove: true,
            },
            idempotency_key: None,
            expires_in_seconds: Some(15 * 60),
        },
    )
    .await;
    let stopped = match stop {
        Ok(operation) => {
            match await_target_operation(state, &previous.target_id, operation).await {
                Ok(operation) if operation.status == TargetOperationStatusKind::Succeeded => true,
                Ok(operation) => {
                    tracing::warn!(project_id = %previous.project_id, revision_id = %previous.revision_id, status = ?operation.status, "previous target deployment did not stop cleanly");
                    false
                }
                Err(error) => {
                    tracing::warn!(project_id = %previous.project_id, revision_id = %previous.revision_id, error = %error, "previous target deployment stop outcome is unknown");
                    false
                }
            }
        }
        Err(error) => {
            tracing::warn!(project_id = %previous.project_id, revision_id = %previous.revision_id, error = %error.error, "previous target deployment stop submission failed");
            false
        }
    };
    if !stopped {
        return vec!["previous target deployment cleanup was not confirmed".to_string()];
    }
    if !cleanup_target_host_resources(state, deployment, active_route_id).await {
        return vec!["previous target deployment Host resources require cleanup".to_string()];
    }
    Vec::new()
}

fn validate_deployment_preview_request(
    request: &DevelopmentDeploymentPreviewRequest,
) -> Result<(), ServiceError> {
    let valid_target = !request.target_id.is_empty()
        && request.target_id.len() <= 128
        && request
            .target_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._".contains(&byte));
    let valid_port_name = !request.port_name.is_empty()
        && request.port_name.len() <= 64
        && request
            .port_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._".contains(&byte));
    let valid_health_path = request.health_path.as_deref().is_none_or(|path| {
        path.starts_with('/')
            && !path.starts_with("//")
            && path.len() <= 256
            && !path.contains(['\r', '\n'])
    });
    let valid_idempotency = request.idempotency_key.as_deref().is_none_or(|key| {
        !key.is_empty()
            && key.len() <= 128
            && key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-._:".contains(&byte))
    });
    if !valid_target
        || request.container_port == 0
        || !valid_port_name
        || !crate::is_safe_route_token(&request.route_id)
        || !valid_health_path
        || !valid_idempotency
    {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "deployment preview contains an invalid target, port, route, health path, or idempotency key",
        ));
    }
    Ok(())
}

fn validate_deployment_verification_provenance(
    record: &DevelopmentChangeRecord,
    verification: &DevelopmentVerificationResult,
    dockerfile: &str,
    network_mode: DevelopmentNetworkMode,
    source_tree_digest: &str,
) -> Result<ArtifactDescriptor, ServiceError> {
    let context = verification
        .deployment_artifact_ref
        .clone()
        .ok_or_else(|| {
            ServiceError::with_status(
                StatusCode::CONFLICT,
                "Docker verification predates deployable artifact provenance",
            )
        })?;
    let valid = verification.succeeded
        && verification.kind == "docker_build"
        && verification.network_mode == network_mode
        && verification.artifact_ref.artifact_type_uri == DEVELOPMENT_RESULT_ARTIFACT_TYPE_URI
        && verification.artifact_ref.media_type == "application/json"
        && verification
            .artifact_ref
            .references
            .contains(&record.change_set_ref.digest)
        && verification
            .artifact_ref
            .references
            .contains(&context.digest)
        && context.artifact_type_uri == DEVELOPMENT_BUILD_CONTEXT_ARTIFACT_TYPE_URI
        && context.media_type == "application/x-tar"
        && context.references.contains(&record.change_set_ref.digest)
        && context
            .annotations
            .get("project_id")
            .and_then(Value::as_str)
            == Some(record.project_id.as_str())
        && context
            .annotations
            .get("change_set_id")
            .and_then(Value::as_str)
            == Some(record.change_set.id.as_str())
        && context
            .annotations
            .get("tree_digest")
            .and_then(Value::as_str)
            == Some(source_tree_digest)
        && context
            .annotations
            .get("dockerfile")
            .and_then(Value::as_str)
            == Some(dockerfile);
    if !valid {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "Docker verification provenance is incomplete or inconsistent",
        ));
    }
    Ok(context)
}

async fn verify_deployment_artifact_content<S>(
    runtime: &Runtime<S>,
    descriptor: &ArtifactDescriptor,
) -> Result<(), ServiceError>
where
    S: EventStore,
{
    let info = runtime
        .object_store()
        .verify(&descriptor.digest)
        .await
        .map_err(|error| {
            internal_development_error("verified deployment artifact is unavailable", error)
        })?;
    if info.size_bytes != descriptor.size_bytes {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "verified deployment artifact descriptor does not match stored content",
        ));
    }
    Ok(())
}

async fn read_verified_preview_evidence<S>(
    runtime: &Runtime<S>,
    preview_ref: &ArtifactDescriptor,
    project_id: &ProjectId,
    change_set_id: &str,
    target_id: &str,
    source_tree_digest: &str,
    verification_ref: &ArtifactDescriptor,
    build_context_ref: &ArtifactDescriptor,
) -> anyhow::Result<(String, DevelopmentDeploymentPreview, ArtifactDescriptor)>
where
    S: EventStore,
{
    anyhow::ensure!(
        preview_ref.artifact_type_uri == DEVELOPMENT_DEPLOYMENT_PREVIEW_TYPE_URI
            && preview_ref.media_type == "application/json"
            && preview_ref.references.contains(&verification_ref.digest)
            && preview_ref.references.contains(&build_context_ref.digest)
            && preview_ref
                .annotations
                .get("project_id")
                .and_then(Value::as_str)
                == Some(project_id.as_str())
            && preview_ref
                .annotations
                .get("change_set_id")
                .and_then(Value::as_str)
                == Some(change_set_id)
            && preview_ref
                .annotations
                .get("target_id")
                .and_then(Value::as_str)
                == Some(target_id)
            && preview_ref
                .annotations
                .get("source_tree_digest")
                .and_then(Value::as_str)
                == Some(source_tree_digest),
        "deployment preview descriptor is not bound to the verified change"
    );
    verify_deployment_artifact_content(runtime, preview_ref)
        .await
        .map_err(|_| anyhow::anyhow!("deployment preview content verification failed"))?;
    let payload: Value =
        serde_json::from_slice(&runtime.object_store().get(&preview_ref.digest).await?)?;
    anyhow::ensure!(
        payload.get("schema_version").and_then(Value::as_u64) == Some(1)
            && payload.get("project_id").and_then(Value::as_str) == Some(project_id.as_str())
            && payload.get("change_set_id").and_then(Value::as_str) == Some(change_set_id)
            && payload.get("source_tree_digest").and_then(Value::as_str)
                == Some(source_tree_digest),
        "deployment preview payload is not bound to the verified change"
    );
    let deployment_id = payload
        .get("deployment_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("deployment preview payload has no deployment id"))?
        .to_string();
    let stored_verification: ArtifactDescriptor = serde_json::from_value(
        payload
            .get("verification_ref")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("deployment preview has no verification reference"))?,
    )?;
    let stored_context: ArtifactDescriptor =
        serde_json::from_value(payload.get("build_context_ref").cloned().ok_or_else(|| {
            anyhow::anyhow!("deployment preview has no build context reference")
        })?)?;
    let authority_ref: ArtifactDescriptor = serde_json::from_value(
        payload
            .get("authority_ref")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("deployment preview has no authority reference"))?,
    )?;
    let preview: DevelopmentDeploymentPreview = serde_json::from_value(
        payload
            .get("preview")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("deployment preview payload has no candidate"))?,
    )?;
    anyhow::ensure!(
        stored_verification == *verification_ref
            && stored_context == *build_context_ref
            && authority_ref.artifact_type_uri == DEVELOPMENT_DEPLOYMENT_AUTHORITY_TYPE_URI
            && authority_ref.media_type == "application/json"
            && authority_ref
                .annotations
                .get("project_id")
                .and_then(Value::as_str)
                == Some(project_id.as_str())
            && authority_ref
                .annotations
                .get("change_set_id")
                .and_then(Value::as_str)
                == Some(change_set_id)
            && authority_ref
                .annotations
                .get("target_id")
                .and_then(Value::as_str)
                == Some(target_id)
            && preview_ref.references.contains(&authority_ref.digest),
        "deployment preview evidence chain is inconsistent"
    );
    verify_deployment_artifact_content(runtime, &authority_ref)
        .await
        .map_err(|_| anyhow::anyhow!("deployment authority content verification failed"))?;
    Ok((deployment_id, preview, authority_ref))
}

async fn read_verified_deployment_approval<S>(
    runtime: &Runtime<S>,
    approval_ref: &ArtifactDescriptor,
    preview_ref: &ArtifactDescriptor,
    verification_ref: &ArtifactDescriptor,
    build_context_ref: &ArtifactDescriptor,
    authority_ref: &ArtifactDescriptor,
    project_id: &ProjectId,
    change_set_id: &str,
    deployment_id: &str,
    target_id: &str,
) -> anyhow::Result<PolicyDecision>
where
    S: EventStore,
{
    anyhow::ensure!(
        approval_ref.artifact_type_uri == plurora_core::POLICY_DECISION_TYPE_URI
            && approval_ref.media_type == "application/json"
            && approval_ref.references.contains(&preview_ref.digest)
            && approval_ref.references.contains(&verification_ref.digest)
            && approval_ref.references.contains(&build_context_ref.digest)
            && approval_ref.references.contains(&authority_ref.digest)
            && approval_ref.annotations.get("role").and_then(Value::as_str)
                == Some("explicit_deployment_approval")
            && approval_ref
                .annotations
                .get("project_id")
                .and_then(Value::as_str)
                == Some(project_id.as_str())
            && approval_ref
                .annotations
                .get("change_set_id")
                .and_then(Value::as_str)
                == Some(change_set_id)
            && approval_ref
                .annotations
                .get("deployment_id")
                .and_then(Value::as_str)
                == Some(deployment_id)
            && approval_ref
                .annotations
                .get("target_id")
                .and_then(Value::as_str)
                == Some(target_id),
        "deployment approval descriptor is not bound to the preview"
    );
    verify_deployment_artifact_content(runtime, approval_ref)
        .await
        .map_err(|_| anyhow::anyhow!("deployment approval content verification failed"))?;
    let decision: PolicyDecision =
        serde_json::from_slice(&runtime.object_store().get(&approval_ref.digest).await?)?;
    anyhow::ensure!(
        decision.outcome == PolicyDecisionOutcome::Allowed
            && decision.principal == PrincipalIdentity::HostAdmin
            && decision.change_set_id == format!("{change_set_id}:deployment:{deployment_id}")
            && decision
                .evaluated_authority
                .contains(&"host.project.deploy".to_string())
            && decision
                .evaluated_authority
                .contains(&format!("host.target.{target_id}")),
        "deployment approval payload is not an explicit approval for the preview"
    );
    Ok(decision)
}

fn deployment_preview_request_digest(
    record: &DevelopmentChangeRecord,
    request: &DevelopmentDeploymentPreviewRequest,
    verification_ref: &ArtifactDescriptor,
    build_context_ref: &ArtifactDescriptor,
) -> anyhow::Result<String> {
    let bytes = serde_json::to_vec(&json!({
        "schema_version": 1,
        "project_id": record.project_id,
        "change_set_id": record.change_set.id,
        "request": request,
        "verification_digest": verification_ref.digest,
        "build_context_digest": build_context_ref.digest,
    }))?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn deployment_build_descriptor_hash(
    project_id: &ProjectId,
    build_context_ref: &ArtifactDescriptor,
    source_tree_digest: &str,
    dockerfile: &str,
    network_mode: DevelopmentNetworkMode,
    build_id: &str,
) -> String {
    let mut hasher = Sha256::new();
    for value in [
        "plurora.verified-deployment-build.v1",
        project_id.as_str(),
        &build_context_ref.digest,
        source_tree_digest,
        dockerfile,
        network_mode.as_str(),
        build_id,
    ] {
        hasher.update(value.as_bytes());
        hasher.update(b"\0");
    }
    format!("sha256:{:x}", hasher.finalize())
}

async fn update_deployment_record<S, F>(
    state: &AppState<S>,
    change_set_id: &str,
    mutate: F,
) -> anyhow::Result<DevelopmentChangeRecord>
where
    S: EventStore,
    F: FnOnce(&mut DevelopmentDeploymentRecord) -> anyhow::Result<()>,
{
    verify_development_host_lease(state.runtime.store().as_ref(), state.development.as_ref())
        .await?;
    let change_lock = state.development.lock_for(change_set_id);
    let _guard = change_lock.lock().await;
    let mut record = state
        .development
        .get(change_set_id)
        .ok_or_else(|| anyhow::anyhow!("development change disappeared"))?;
    anyhow::ensure!(
        record.status == DevelopmentChangeStatus::Committed,
        "deployment parent change is no longer committed"
    );
    let deployment = record
        .deployment
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("development deployment disappeared"))?;
    mutate(deployment)?;
    deployment.updated_at_ms = now_millis();
    record.revision = record.revision.saturating_add(1);
    record.updated_at_ms = now_millis();
    persist_record(state, record.clone()).await?;
    Ok(record)
}

async fn await_target_operation<S>(
    state: &AppState<S>,
    target_id: &str,
    operation: TargetOperationRecord,
) -> anyhow::Result<TargetOperationRecord>
where
    S: EventStore,
{
    if operation.status.is_terminal() {
        Ok(operation)
    } else {
        crate::target_agent::wait_for_host_operation(
            state,
            target_id,
            &operation.operation_id,
            DEVELOPMENT_DEPLOYMENT_OPERATION_TIMEOUT,
        )
        .await
    }
}

fn require_succeeded_target_operation(
    operation: TargetOperationRecord,
    effect: &str,
) -> anyhow::Result<TargetOperationRecord> {
    anyhow::ensure!(
        operation.status == TargetOperationStatusKind::Succeeded,
        "{effect} did not succeed"
    );
    let receipt = operation
        .receipt
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("{effect} completed without a receipt"))?;
    anyhow::ensure!(
        receipt.status == crate::target_agent::TargetOperationReceiptStatus::Succeeded,
        "{effect} receipt did not succeed"
    );
    Ok(operation)
}

async fn run_deployment_preview<S>(
    state: &AppState<S>,
    change_set_id: &str,
    authority: &DeploymentAuthorityLease,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    let building = update_deployment_record(state, change_set_id, |deployment| {
        anyhow::ensure!(
            matches!(
                deployment.status,
                DevelopmentDeploymentStatus::Preparing | DevelopmentDeploymentStatus::Building
            ),
            "deployment preview is not in its build phase"
        );
        deployment.status = DevelopmentDeploymentStatus::Building;
        deployment.error = None;
        Ok(())
    })
    .await?;
    let deployment = building
        .deployment
        .clone()
        .ok_or_else(|| anyhow::anyhow!("development deployment disappeared"))?;
    let project_id = building.project_id.clone();
    deployment_effect_context(
        state,
        Some(authority),
        &project_id,
        "host_development_deployment_build",
    )
    .await?;
    let build_operation = crate::target_agent::submit_host_operation(
        state,
        &deployment.target_id,
        CreateTargetOperationRequest {
            project_id: project_id.clone(),
            spec: TargetOperationSpec::VerifierRun {
                verifier: DeclarativeVerifierDescriptor::DockerBuild {
                    digest: deployment.build_context_ref.digest.clone(),
                    expected_size_bytes: Some(deployment.build_context_ref.size_bytes),
                    dockerfile: deployment.dockerfile.clone(),
                    network_mode: match deployment.network_mode {
                        DevelopmentNetworkMode::None => {
                            plurora_runtime::ManagedTargetBuildNetworkMode::None
                        }
                        DevelopmentNetworkMode::Bridge => {
                            plurora_runtime::ManagedTargetBuildNetworkMode::Bridge
                        }
                    },
                    build_id: deployment.build_id.clone(),
                    source_tree_digest: deployment.source_tree_digest.clone(),
                    build_descriptor_hash: deployment.build_descriptor_hash.clone(),
                },
            },
            idempotency_key: Some(format!("{}:build", deployment.deployment_id)),
            expires_in_seconds: Some(15 * 60),
        },
    )
    .await
    .map_err(|error| error.error)?;
    let build_operation_id = build_operation.operation_id.clone();
    update_deployment_record(state, change_set_id, |current| {
        anyhow::ensure!(
            current.deployment_id == deployment.deployment_id,
            "deployment preview identity changed"
        );
        current.build_operation_id = Some(build_operation_id.clone());
        Ok(())
    })
    .await?;
    let build_operation = require_succeeded_target_operation(
        await_target_operation(state, &deployment.target_id, build_operation).await?,
        "target Docker build",
    )?;
    let build_output = &build_operation
        .receipt
        .as_ref()
        .expect("succeeded target operation has a receipt")
        .output;
    let image = required_string(build_output, "image", "target Docker build receipt")?;
    let image_id = required_string(build_output, "image_id", "target Docker build receipt")?;
    anyhow::ensure!(
        required_string(
            build_output,
            "context_digest",
            "target Docker build receipt"
        )? == deployment.build_context_ref.digest
            && required_string(
                build_output,
                "source_tree_digest",
                "target Docker build receipt"
            )? == deployment.source_tree_digest
            && required_string(
                build_output,
                "build_descriptor_hash",
                "target Docker build receipt"
            )? == deployment.build_descriptor_hash
            && required_string(build_output, "build_id", "target Docker build receipt")?
                == deployment.build_id,
        "target Docker build receipt does not match the verified build descriptor"
    );

    let port_context = deployment_effect_context(
        state,
        Some(authority),
        &project_id,
        "host_development_deployment_port_lease",
    )
    .await?;
    let lease = call_host_protocol(
        state,
        &port_context,
        "host.port.lease",
        json!({
            "target_id": deployment.target_id,
            "port_name": deployment.port_name,
            "protocol": "tcp",
        }),
    )
    .await
    .and_then(|value| value_field(value, "lease", "host.port.lease"))?;
    let port_lease_id = required_string(&lease, "id", "deployment preview port lease")?;
    let target_deployment_id = format!("preview-{}", deployment.deployment_id);
    let previewing = update_deployment_record(state, change_set_id, |current| {
        anyhow::ensure!(
            current.deployment_id == deployment.deployment_id
                && current.status == DevelopmentDeploymentStatus::Building,
            "deployment preview changed before candidate preparation"
        );
        current.status = DevelopmentDeploymentStatus::Previewing;
        current.preview_port_lease_id = Some(port_lease_id.clone());
        current.target_deployment_id = Some(target_deployment_id.clone());
        Ok(())
    })
    .await?;
    let deployment = previewing
        .deployment
        .clone()
        .ok_or_else(|| anyhow::anyhow!("development deployment disappeared"))?;

    let route_context = deployment_effect_context(
        state,
        Some(authority),
        &project_id,
        "host_development_deployment_preview_route",
    )
    .await?;
    let route = call_host_protocol(
        state,
        &route_context,
        "host.proxy.register",
        json!({
            "route_id": deployment.preview_route_id,
            "protocol": "http",
            "access": ProxyRouteAccess::HostAuthenticated,
            "upstream": {
                "port_lease_id": port_lease_id,
                "port_name": deployment.port_name,
            },
        }),
    )
    .await
    .and_then(|value| value_field(value, "route", "host.proxy.register"))?;
    let registered_route_id = required_string(&route, "id", "deployment preview route")?;
    anyhow::ensure!(
        registered_route_id == deployment.preview_route_id,
        "deployment preview route identity changed during registration"
    );
    let fallback_public_url = required_string(&route, "public_url", "deployment preview route")?;
    let public_url = service_public_url_for_route(
        state,
        &registered_route_id,
        &fallback_public_url,
        ProxyRouteAccess::HostAuthenticated,
    );

    deployment_effect_context(
        state,
        Some(authority),
        &project_id,
        "host_development_deployment_candidate_apply",
    )
    .await?;
    let target_deployment = TargetDeploymentRef {
        deployment_id: target_deployment_id,
        route_id: deployment.preview_route_id.clone(),
        port_lease_id: port_lease_id.clone(),
    };
    let apply_operation = crate::target_agent::submit_host_operation(
        state,
        &deployment.target_id,
        CreateTargetOperationRequest {
            project_id: project_id.clone(),
            spec: TargetOperationSpec::DeploymentApply {
                deployment: TargetDeploymentDescriptor {
                    deployment: target_deployment.clone(),
                    port_name: deployment.port_name.clone(),
                    image: image_id.clone(),
                    container_port: deployment.container_port,
                    requested_host_port: None,
                    pull_if_missing: false,
                    health_path: deployment.health_path.clone(),
                },
            },
            idempotency_key: Some(format!("{}:apply", deployment.deployment_id)),
            expires_in_seconds: Some(15 * 60),
        },
    )
    .await
    .map_err(|error| error.error)?;
    let deployment_operation_id = apply_operation.operation_id.clone();
    update_deployment_record(state, change_set_id, |current| {
        anyhow::ensure!(
            current.deployment_id == deployment.deployment_id
                && current.status == DevelopmentDeploymentStatus::Previewing,
            "deployment preview changed before candidate receipt persistence"
        );
        current.deployment_operation_id = Some(deployment_operation_id.clone());
        Ok(())
    })
    .await?;
    let apply_operation = require_succeeded_target_operation(
        await_target_operation(state, &deployment.target_id, apply_operation).await?,
        "target deployment apply",
    )?;
    let apply_output = &apply_operation
        .receipt
        .as_ref()
        .expect("succeeded target operation has a receipt")
        .output;
    anyhow::ensure!(
        apply_output.get("running").and_then(Value::as_bool) == Some(true),
        "target deployment receipt is not running"
    );
    let container_id = required_string(
        apply_output,
        "container_id",
        "target deployment apply receipt",
    )?;
    let container_name = apply_output
        .get("container_name")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    anyhow::ensure!(
        required_string(apply_output, "image_id", "target deployment apply receipt")? == image_id,
        "target deployment used a different image than the verified target build"
    );
    let ready_route = state
        .runtime
        .config()
        .proxy_route_registry
        .status(&deployment.preview_route_id)
        .await
        .ok_or_else(|| anyhow::anyhow!("deployment preview route disappeared"))?;
    anyhow::ensure!(
        ready_route.status == plurora_runtime::ProxyRouteStatusKind::Active
            && ready_route.ready
            && ready_route.upstream.port_lease_id == port_lease_id,
        "deployment preview route did not become ready"
    );

    let preview = DevelopmentDeploymentPreview {
        route_id: registered_route_id,
        public_url,
        port_lease_id,
        deployment: target_deployment,
        image,
        image_id,
        container_id,
        container_name,
        build_operation_id,
        deployment_operation_id,
        ready_at_ms: now_millis(),
    };
    let preview_ref = commit_json_artifact(
        state.runtime.as_ref(),
        DEVELOPMENT_DEPLOYMENT_PREVIEW_TYPE_URI,
        &json!({
            "schema_version": 1,
            "project_id": project_id,
            "change_set_id": change_set_id,
            "deployment_id": deployment.deployment_id,
            "source_tree_digest": deployment.source_tree_digest,
            "verification_ref": deployment.verification_ref,
            "build_context_ref": deployment.build_context_ref,
            "authority_ref": deployment.authority_ref,
            "preview": preview,
        }),
        vec![
            deployment.verification_ref.digest.clone(),
            deployment.build_context_ref.digest.clone(),
            deployment.authority_ref.digest.clone(),
        ],
        BTreeMap::from([
            ("project_id".to_string(), json!(project_id.as_str())),
            ("change_set_id".to_string(), json!(change_set_id)),
            ("deployment_id".to_string(), json!(deployment.deployment_id)),
            ("target_id".to_string(), json!(deployment.target_id)),
            (
                "source_tree_digest".to_string(),
                json!(deployment.source_tree_digest),
            ),
        ]),
    )
    .await?;
    update_deployment_record(state, change_set_id, |current| {
        anyhow::ensure!(
            current.deployment_id == deployment.deployment_id
                && current.status == DevelopmentDeploymentStatus::Previewing,
            "deployment preview changed before readiness persistence"
        );
        current.status = DevelopmentDeploymentStatus::PreviewReady;
        current.preview = Some(preview);
        current.preview_ref = Some(preview_ref);
        current.error = None;
        Ok(())
    })
    .await?;
    Ok(())
}

fn target_operation_outcome_is_uncertain<S>(state: &AppState<S>, operation_id: &str) -> bool
where
    S: EventStore,
{
    state
        .target_agents
        .operation(operation_id)
        .is_none_or(|operation| {
            !operation.status.is_terminal()
                || operation.status == TargetOperationStatusKind::OutcomeUnknown
        })
}

async fn unrecorded_preview_port_lease_may_exist<S>(
    state: &AppState<S>,
    deployment: &DevelopmentDeploymentRecord,
) -> bool
where
    S: EventStore,
{
    let mut claimed_lease_ids = state
        .build_jobs
        .durable_routes()
        .into_iter()
        .map(|route| route.port_lease_id)
        .collect::<HashSet<_>>();
    claimed_lease_ids.extend(
        state
            .runtime
            .config()
            .proxy_route_registry
            .list()
            .await
            .into_iter()
            .filter(|route| route.status != plurora_runtime::ProxyRouteStatusKind::Removed)
            .map(|route| route.upstream.port_lease_id),
    );
    claimed_lease_ids.extend(
        state
            .target_agents
            .operations_for_target(&deployment.target_id)
            .into_iter()
            .filter_map(|operation| match operation.spec {
                TargetOperationSpec::DeploymentApply { deployment } => {
                    Some(deployment.deployment.port_lease_id)
                }
                TargetOperationSpec::DeploymentObserve { deployment }
                | TargetOperationSpec::DeploymentDrain { deployment, .. }
                | TargetOperationSpec::DeploymentStop { deployment, .. } => {
                    Some(deployment.port_lease_id)
                }
                _ => None,
            }),
    );
    state
        .runtime
        .config()
        .port_lease_registry
        .list()
        .await
        .into_iter()
        .any(|lease| {
            lease.target_id == deployment.target_id
                && lease.port_name == deployment.port_name
                && lease.protocol == plurora_runtime::PortProtocol::Tcp
                && lease.bind == plurora_runtime::PortBindScope::LoopbackOnly
                && lease.host == "127.0.0.1"
                && lease.status != plurora_runtime::PortLeaseStatusKind::Released
                && !claimed_lease_ids.contains(&lease.id)
        })
}

async fn stop_preview_candidate_for_reconciliation<S>(
    state: &AppState<S>,
    project_id: &ProjectId,
    deployment: &DevelopmentDeploymentRecord,
) -> Result<bool, ServiceError>
where
    S: EventStore,
{
    let (Some(target_deployment_id), Some(port_lease_id)) = (
        deployment.target_deployment_id.as_ref(),
        deployment.preview_port_lease_id.as_ref(),
    ) else {
        return Ok(state
            .target_agents
            .project_for_operation_route(&deployment.preview_route_id)
            .is_none());
    };
    let target_deployment = TargetDeploymentRef {
        deployment_id: target_deployment_id.clone(),
        route_id: deployment.preview_route_id.clone(),
        port_lease_id: port_lease_id.clone(),
    };
    let latest_candidate_effect = state
        .target_agents
        .operations_for_target(&deployment.target_id)
        .into_iter()
        .filter(|operation| {
            if operation.project_id != *project_id {
                return false;
            }
            match &operation.spec {
                TargetOperationSpec::DeploymentApply { deployment } => {
                    deployment.deployment == target_deployment
                }
                TargetOperationSpec::DeploymentStop { deployment, .. } => {
                    *deployment == target_deployment
                }
                _ => false,
            }
        })
        .last();
    if let Some(latest) = latest_candidate_effect {
        if latest.status == TargetOperationStatusKind::Succeeded
            && matches!(latest.spec, TargetOperationSpec::DeploymentStop { .. })
        {
            return Ok(true);
        }
        if matches!(latest.spec, TargetOperationSpec::DeploymentApply { .. })
            && matches!(
                latest.status,
                TargetOperationStatusKind::Failed
                    | TargetOperationStatusKind::Cancelled
                    | TargetOperationStatusKind::Expired
            )
        {
            return Ok(true);
        }
    }
    let stop = crate::target_agent::submit_host_operation(
        state,
        &deployment.target_id,
        CreateTargetOperationRequest {
            project_id: project_id.clone(),
            spec: TargetOperationSpec::DeploymentStop {
                deployment: target_deployment,
                grace_seconds: 0,
                force_remove: true,
            },
            idempotency_key: Some(format!(
                "{}:reconcile:{}",
                deployment.deployment_id,
                &uuid::Uuid::new_v4().simple().to_string()[..12]
            )),
            expires_in_seconds: Some(15 * 60),
        },
    )
    .await
    .map_err(|error| {
        tracing::warn!(project_id = %project_id, deployment_id = %deployment.deployment_id, error = %error.error, "failed to submit deployment reconciliation stop");
        ServiceError::with_status(
            StatusCode::CONFLICT,
            "deployment target is not ready for candidate reconciliation",
        )
    })?;
    let stop = await_target_operation(state, &deployment.target_id, stop)
        .await
        .map_err(|error| {
            tracing::warn!(project_id = %project_id, deployment_id = %deployment.deployment_id, error = %error, "deployment reconciliation stop outcome is unresolved");
            ServiceError::with_status(
                StatusCode::CONFLICT,
                "deployment candidate stop outcome is still unresolved",
            )
        })?;
    Ok(stop.status == TargetOperationStatusKind::Succeeded)
}

async fn stop_completed_preview_candidate<S>(
    state: &AppState<S>,
    project_id: &ProjectId,
    deployment: &DevelopmentDeploymentRecord,
) -> anyhow::Result<bool>
where
    S: EventStore,
{
    let Some(operation_id) = deployment.deployment_operation_id.as_deref() else {
        return Ok(state
            .target_agents
            .project_for_operation_route(&deployment.preview_route_id)
            .is_none());
    };
    let Some(operation) = state.target_agents.operation(operation_id) else {
        return Ok(false);
    };
    match operation.status {
        TargetOperationStatusKind::Succeeded => {}
        TargetOperationStatusKind::Failed
        | TargetOperationStatusKind::Cancelled
        | TargetOperationStatusKind::Expired => return Ok(true),
        TargetOperationStatusKind::Requested
        | TargetOperationStatusKind::Accepted
        | TargetOperationStatusKind::Running
        | TargetOperationStatusKind::OutcomeUnknown => return Ok(false),
    }
    let (Some(target_deployment_id), Some(port_lease_id)) = (
        deployment.target_deployment_id.as_ref(),
        deployment.preview_port_lease_id.as_ref(),
    ) else {
        return Ok(false);
    };
    let cleanup = crate::target_agent::submit_host_operation(
        state,
        &deployment.target_id,
        CreateTargetOperationRequest {
            project_id: project_id.clone(),
            spec: TargetOperationSpec::DeploymentStop {
                deployment: TargetDeploymentRef {
                    deployment_id: target_deployment_id.clone(),
                    route_id: deployment.preview_route_id.clone(),
                    port_lease_id: port_lease_id.clone(),
                },
                grace_seconds: 0,
                force_remove: true,
            },
            idempotency_key: Some(format!("{}:cleanup", deployment.deployment_id)),
            expires_in_seconds: Some(15 * 60),
        },
    )
    .await;
    let Ok(cleanup) = cleanup else {
        return Ok(false);
    };
    let cleanup = match await_target_operation(state, &deployment.target_id, cleanup).await {
        Ok(cleanup) => cleanup,
        Err(_) => return Ok(false),
    };
    Ok(cleanup.status == TargetOperationStatusKind::Succeeded)
}

async fn cleanup_preview_host_resources<S>(
    state: &AppState<S>,
    deployment: &DevelopmentDeploymentRecord,
) -> anyhow::Result<bool>
where
    S: EventStore,
{
    let Some(port_lease_id) = deployment.preview_port_lease_id.as_deref() else {
        return Ok(true);
    };
    let context = ProtocolContext::host_dev("host_development_deployment_compensation");
    if let Some(route) = state
        .runtime
        .config()
        .proxy_route_registry
        .status(&deployment.preview_route_id)
        .await
    {
        if route.status != plurora_runtime::ProxyRouteStatusKind::Removed {
            if route.upstream.port_lease_id != port_lease_id {
                return Ok(false);
            }
            if let Err(error) = call_host_protocol(
                state,
                &context,
                "host.proxy.unregister",
                json!({ "route_id": deployment.preview_route_id }),
            )
            .await
            {
                tracing::warn!(
                    deployment_id = %deployment.deployment_id,
                    error = %error,
                    "deployment preview route cleanup failed"
                );
            }
        }
    }
    let route_removed = state
        .runtime
        .config()
        .proxy_route_registry
        .status(&deployment.preview_route_id)
        .await
        .is_none_or(|route| route.status == plurora_runtime::ProxyRouteStatusKind::Removed);
    if !route_removed {
        return Ok(false);
    }

    if state
        .runtime
        .config()
        .port_lease_registry
        .status(port_lease_id)
        .await
        .is_some_and(|lease| lease.status != plurora_runtime::PortLeaseStatusKind::Released)
    {
        if let Err(error) = call_host_protocol(
            state,
            &context,
            "host.port.release",
            json!({ "lease_id": port_lease_id }),
        )
        .await
        {
            tracing::warn!(
                deployment_id = %deployment.deployment_id,
                error = %error,
                "deployment preview port cleanup failed"
            );
        }
    }
    Ok(state
        .runtime
        .config()
        .port_lease_registry
        .status(port_lease_id)
        .await
        .is_none_or(|lease| lease.status == plurora_runtime::PortLeaseStatusKind::Released))
}

async fn complete_deployment_preview_failure<S>(
    state: &AppState<S>,
    change_set_id: &str,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    verify_development_host_lease(state.runtime.store().as_ref(), state.development.as_ref())
        .await?;
    let record = state
        .development
        .get(change_set_id)
        .ok_or_else(|| anyhow::anyhow!("development change disappeared"))?;
    let Some(deployment) = record.deployment.clone() else {
        anyhow::bail!("development deployment disappeared");
    };
    if !deployment.status.executing() {
        return Ok(());
    }
    let operation_uncertain = deployment
        .build_operation_id
        .as_deref()
        .is_some_and(|id| target_operation_outcome_is_uncertain(state, id))
        || deployment
            .deployment_operation_id
            .as_deref()
            .is_some_and(|id| target_operation_outcome_is_uncertain(state, id));
    let cleanup_complete = if operation_uncertain {
        false
    } else {
        stop_completed_preview_candidate(state, &record.project_id, &deployment).await?
            && cleanup_preview_host_resources(state, &deployment).await?
    };
    update_deployment_record(state, change_set_id, |current| {
        if !current.status.executing() {
            return Ok(());
        }
        if operation_uncertain || !cleanup_complete {
            current.status = DevelopmentDeploymentStatus::RecoveryRequired;
            current.error = Some(
                "deployment preview outcome requires explicit reconciliation; details redacted"
                    .to_string(),
            );
        } else {
            current.status = DevelopmentDeploymentStatus::Failed;
            current.error = Some("deployment preview failed; details redacted".to_string());
        }
        Ok(())
    })
    .await?;
    Ok(())
}

fn development_project_session(project_id: &ProjectId) -> String {
    format!(
        "{DEVELOPMENT_JOURNAL_SESSION_PREFIX}/{}",
        project_id.as_str()
    )
}

async fn verify_development_host_lease<S>(
    store: &S,
    registry: &DevelopmentRegistry,
) -> anyhow::Result<DevelopmentHostLease>
where
    S: EventStore,
{
    let lease = registry.active_host_lease()?;
    let (_, current) = development_host_lease_tail(store).await?;
    let current = current.ok_or_else(|| anyhow::anyhow!("development Host lease disappeared"))?;
    let current_owner = current.owner_id == lease.owner_id;
    let current_live = !current.released && current.expires_at_ms > Utc::now().timestamp_millis();
    if !current_owner || !current_live {
        lease.valid.store(false, Ordering::Release);
        anyhow::bail!("development Host lease is no longer the durable owner");
    }
    lease
        .expires_at_ms
        .store(current.expires_at_ms, Ordering::Release);
    Ok(lease)
}

fn validate_development_authority(
    identity: &HostAccessIdentity,
    registry: &HostAccessRegistry,
    project_id: &ProjectId,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        identity.allows(HostAccessScope::DevelopExecute),
        "development authority does not include develop_execute"
    );
    anyhow::ensure!(
        identity.allows_project(project_id.as_str()),
        "development authority does not include the project"
    );
    if let Some(expires_at_ms) = identity.expires_at_ms {
        anyhow::ensure!(
            expires_at_ms > Utc::now().timestamp_millis(),
            "development authority expired"
        );
    }
    if identity.kind == HostAccessIdentityKind::Device {
        let grant_id = identity
            .grant_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("device development authority has no grant id"))?;
        anyhow::ensure!(
            registry.grant_is_currently_active(grant_id),
            "development authority grant is revoked or expired"
        );
    }
    Ok(())
}

fn development_authority_context(
    identity: &HostAccessIdentity,
    project_id: &ProjectId,
    transport: &str,
) -> ProtocolContext {
    identity.protocol_context(transport).with_host_operation(
        HostAccessScope::DevelopExecute.as_str(),
        vec![ProtocolResourceSelector {
            owner: "host".to_string(),
            kind: "project".to_string(),
            id: Some(project_id.to_string()),
        }],
    )
}

async fn verify_development_authority<S>(
    state: &AppState<S>,
    identity: &HostAccessIdentity,
    project_id: &ProjectId,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    verify_development_host_lease(state.runtime.store().as_ref(), state.development.as_ref())
        .await?;
    if identity.kind == HostAccessIdentityKind::Device {
        sync_host_access_journal(state.runtime.store().as_ref(), state.host_access.as_ref())
            .await?;
    }
    validate_development_authority(identity, state.host_access.as_ref(), project_id)
}

pub(crate) async fn verify_host_control_plane_lease_if_installed<S>(
    store: &S,
    registry: &DevelopmentRegistry,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    if !registry.has_host_lease() {
        return Ok(());
    }
    verify_development_host_lease(store, registry)
        .await
        .map(|_| ())
}

async fn ensure_development_host_lease<S>(state: &AppState<S>) -> Result<(), ServiceError>
where
    S: EventStore,
{
    verify_development_host_lease(state.runtime.store().as_ref(), state.development.as_ref())
        .await
        .map(|_| ())
        .map_err(|_| {
            ServiceError::with_status(
                StatusCode::SERVICE_UNAVAILABLE,
                "development control plane does not hold the active Host lease",
            )
        })
}

async fn renew_current_development_host_lease<S>(state: &AppState<S>) -> anyhow::Result<()>
where
    S: EventStore,
{
    let lease =
        verify_development_host_lease(state.runtime.store().as_ref(), state.development.as_ref())
            .await?;
    if let Err(error) = renew_development_host_lease(state.runtime.store().as_ref(), &lease).await {
        lease.valid.store(false, Ordering::Release);
        return Err(error);
    }
    Ok(())
}

async fn append_development_journal_event<S>(
    store: &S,
    snapshot: &DevelopmentChangeSnapshot,
    expected_next_sequence: EventSequence,
) -> anyhow::Result<Option<EventEnvelope>>
where
    S: EventStore,
{
    store
        .append_with_sequence_if_next(
            development_project_session(&snapshot.record.project_id),
            expected_next_sequence,
            DEVELOPMENT_JOURNAL_WRITER.to_string(),
            DEVELOPMENT_SNAPSHOT_EVENT.to_string(),
            1,
            serde_json::to_value(snapshot)?,
            json!({
                "owner": "host_control_plane",
                "content_classification": "source_artifact_references"
            }),
        )
        .await
}

async fn sync_project_journal<S>(
    store: &S,
    registry: &DevelopmentRegistry,
    project_id: &ProjectId,
) -> anyhow::Result<usize>
where
    S: EventStore,
{
    let session_id = development_project_session(project_id);
    let mut loaded = 0usize;
    loop {
        let next = registry.project_journal_next(project_id);
        let after = next.checked_sub(1);
        let events = store
            .list_session_range(&session_id, after, Some(1_000))
            .await?;
        if events.is_empty() {
            break;
        }
        for event in &events {
            registry.apply_journal_event(event)?;
            loaded = loaded.saturating_add(1);
        }
        if events.len() < 1_000 {
            break;
        }
    }
    Ok(loaded)
}

async fn refresh_development_project<S>(
    state: &AppState<S>,
    project_id: &ProjectId,
) -> Result<(), ServiceError>
where
    S: EventStore,
{
    sync_project_journal(
        state.runtime.store().as_ref(),
        state.development.as_ref(),
        project_id,
    )
    .await
    .map(|_| ())
    .map_err(|error| internal_development_error("failed to refresh development journal", error))
}

fn snapshots_match(
    current: &DevelopmentChangeSnapshot,
    expected: &DevelopmentChangeSnapshot,
) -> anyhow::Result<bool> {
    Ok(serde_json::to_value(current)? == serde_json::to_value(expected)?)
}

async fn persist_snapshot<S>(
    state: &AppState<S>,
    snapshot: DevelopmentChangeSnapshot,
    new_record: bool,
) -> anyhow::Result<DevelopmentChangeRecord>
where
    S: EventStore,
{
    let project_id = snapshot.record.project_id.clone();
    let change_set_id = snapshot.record.change_set.id.clone();
    for _ in 0..4 {
        verify_development_host_lease(state.runtime.store().as_ref(), state.development.as_ref())
            .await?;
        sync_project_journal(
            state.runtime.store().as_ref(),
            state.development.as_ref(),
            &project_id,
        )
        .await?;
        match state.development.snapshot(&change_set_id) {
            Some(current) if new_record => {
                anyhow::ensure!(
                    current.request_fingerprint == snapshot.request_fingerprint,
                    "development journal conflict: change id was claimed by another request"
                );
                return Ok(current.record);
            }
            Some(current) => anyhow::ensure!(
                current.request_fingerprint == snapshot.request_fingerprint
                    && current.record.revision.saturating_add(1) == snapshot.record.revision,
                "development journal conflict: change state advanced concurrently"
            ),
            None if new_record => anyhow::ensure!(
                snapshot.record.revision == 1,
                "new development change must begin at revision 1"
            ),
            None => anyhow::bail!("development change disappeared before persistence"),
        }

        let expected_next = state.development.project_journal_next(&project_id);
        verify_development_host_lease(state.runtime.store().as_ref(), state.development.as_ref())
            .await?;
        match append_development_journal_event(
            state.runtime.store().as_ref(),
            &snapshot,
            expected_next,
        )
        .await
        {
            Ok(Some(event)) => {
                state.development.apply_journal_event(&event)?;
                return Ok(snapshot.record);
            }
            Ok(None) => continue,
            Err(append_error) => {
                if sync_project_journal(
                    state.runtime.store().as_ref(),
                    state.development.as_ref(),
                    &project_id,
                )
                .await
                .is_ok()
                {
                    if let Some(current) = state.development.snapshot(&change_set_id) {
                        if snapshots_match(&current, &snapshot)? {
                            return Ok(current.record);
                        }
                    }
                }
                return Err(append_error);
            }
        }
    }
    anyhow::bail!("development journal conflict: project state advanced concurrently")
}

async fn persist_record<S>(
    state: &AppState<S>,
    record: DevelopmentChangeRecord,
) -> anyhow::Result<DevelopmentChangeRecord>
where
    S: EventStore,
{
    let existing = state
        .development
        .snapshot(&record.change_set.id)
        .ok_or_else(|| anyhow::anyhow!("development change disappeared before persistence"))?;
    persist_snapshot(
        state,
        DevelopmentChangeSnapshot {
            record,
            request_fingerprint: existing.request_fingerprint,
        },
        false,
    )
    .await
}

async fn persist_new_record<S>(
    state: &AppState<S>,
    snapshot: DevelopmentChangeSnapshot,
) -> anyhow::Result<DevelopmentChangeRecord>
where
    S: EventStore,
{
    persist_snapshot(state, snapshot, true).await
}

pub async fn hydrate_development_control_plane<S>(
    store: Arc<S>,
    registry: Arc<DevelopmentRegistry>,
) -> anyhow::Result<usize>
where
    S: EventStore,
{
    verify_development_host_lease(store.as_ref(), registry.as_ref()).await?;
    let mut events = store.list_kind_prefix(DEVELOPMENT_JOURNAL_PREFIX).await?;
    events.sort_by(|left, right| {
        left.session_id
            .cmp(&right.session_id)
            .then(left.sequence.cmp(&right.sequence))
    });
    for event in &events {
        if event.kind != DEVELOPMENT_SNAPSHOT_EVENT {
            continue;
        }
        registry.apply_journal_event(event)?;
    }

    let interrupted = {
        registry
            .changes
            .lock()
            .expect("development changes lock poisoned")
            .values()
            .filter(|stored| stored.record.status.executing())
            .map(|stored| stored.record.change_set.id.clone())
            .collect::<Vec<_>>()
    };
    for change_set_id in interrupted {
        verify_development_host_lease(store.as_ref(), registry.as_ref()).await?;
        let Some(mut snapshot) = registry.snapshot(&change_set_id) else {
            continue;
        };
        snapshot.record.revision += 1;
        snapshot.record.updated_at_ms = now_millis();
        if snapshot.record.status == DevelopmentChangeStatus::Promoting {
            snapshot.record.status = DevelopmentChangeStatus::RecoveryRequired;
            snapshot.record.recovery_kind = Some(DevelopmentRecoveryKind::ManagedPromotion);
            snapshot.record.error = Some(
                "host restarted during managed promotion; recovery reconciliation is required"
                    .to_string(),
            );
            snapshot.record.commit = Some(recovery_required_change_commit(
                &snapshot.record.change_set.id,
                DevelopmentRecoveryKind::ManagedPromotion,
            ));
        } else if snapshot.record.status == DevelopmentChangeStatus::Verifying
            && matches!(
                snapshot.record.verification_plan,
                DevelopmentVerificationPlan::DockerBuild { .. }
            )
        {
            snapshot.record.status = DevelopmentChangeStatus::RecoveryRequired;
            snapshot.record.recovery_kind = Some(DevelopmentRecoveryKind::DockerVerification);
            snapshot.record.error = Some(
                "host restarted during Docker verification; the labeled verification image must be reconciled"
                    .to_string(),
            );
            snapshot.record.commit = Some(recovery_required_change_commit(
                &snapshot.record.change_set.id,
                DevelopmentRecoveryKind::DockerVerification,
            ));
        } else {
            snapshot.record.status = DevelopmentChangeStatus::Failed;
            snapshot.record.recovery_kind = None;
            snapshot.record.error = Some(
                "host restarted during development staging or verification; no workspace promotion was resumed"
                    .to_string(),
            );
            snapshot.record.commit = Some(failed_change_commit(
                &snapshot.record.change_set.id,
                "host restarted during development staging or verification",
            ));
        }
        let expected_next = registry.project_journal_next(&snapshot.record.project_id);
        let event = append_development_journal_event(store.as_ref(), &snapshot, expected_next)
            .await?
            .ok_or_else(|| anyhow::anyhow!("development journal changed during recovery"))?;
        registry.apply_journal_event(&event)?;
        if snapshot.record.status == DevelopmentChangeStatus::Failed {
            cleanup_change_root(&snapshot.record.project_id, &change_set_id);
        }
    }

    let interrupted_deployments = {
        registry
            .changes
            .lock()
            .expect("development changes lock poisoned")
            .values()
            .filter(|stored| {
                stored
                    .record
                    .deployment
                    .as_ref()
                    .is_some_and(|deployment| deployment.status.executing())
            })
            .map(|stored| stored.record.change_set.id.clone())
            .collect::<Vec<_>>()
    };
    for change_set_id in interrupted_deployments {
        verify_development_host_lease(store.as_ref(), registry.as_ref()).await?;
        let Some(mut snapshot) = registry.snapshot(&change_set_id) else {
            continue;
        };
        let Some(deployment) = snapshot.record.deployment.as_mut() else {
            continue;
        };
        if !deployment.status.executing() {
            continue;
        }
        deployment.status = DevelopmentDeploymentStatus::RecoveryRequired;
        deployment.error = Some(
            "host restarted during deployment preview; target operation reconciliation is required"
                .to_string(),
        );
        deployment.updated_at_ms = now_millis();
        snapshot.record.revision = snapshot.record.revision.saturating_add(1);
        snapshot.record.updated_at_ms = now_millis();
        let expected_next = registry.project_journal_next(&snapshot.record.project_id);
        let event = append_development_journal_event(store.as_ref(), &snapshot, expected_next)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("development journal changed during deployment recovery")
            })?;
        registry.apply_journal_event(&event)?;
    }
    Ok(events.len())
}

async fn draft_change_inner<S>(
    state: &AppState<S>,
    project_id: ProjectId,
    change_set_id: String,
    request: DevelopmentDraftRequest,
    request_fingerprint: String,
) -> Result<DevelopmentChangeRecord, ServiceError>
where
    S: EventStore,
{
    let workspace = resolve_project_workspace(state, &project_id).map_err(|error| {
        tracing::warn!(project_id = %project_id, error = %error, "development workspace resolution failed");
        ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "project workspace is unavailable or failed its ownership checks",
        )
    })?;
    if workspace.ownership == DevelopmentWorkspaceOwnership::LinkedLocal {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "linked-local projects are proposal-only; import a managed workspace before Host verification",
        ));
    }
    let base_summary = workspace_tree_hash(&workspace.root)
        .await
        .map_err(|error| {
            internal_development_error("failed to inspect project workspace", error)
        })?;
    ensure_descriptor_matches_workspace(&workspace, &base_summary.sha256).map_err(|error| {
        tracing::warn!(project_id = %project_id, error = %error, "managed workspace digest validation failed");
        ServiceError::with_status(
            StatusCode::CONFLICT,
            "managed workspace content no longer matches its immutable descriptor",
        )
    })?;
    if let Some(expected) = request.expected_tree_digest.as_deref() {
        if expected != base_summary.sha256 {
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "expected_tree_digest does not match the current project workspace",
            ));
        }
    }

    let now = Utc::now();
    let intent = Intent {
        id: format!("intent-{}", uuid::Uuid::new_v4().simple()),
        intent_type_uri: plurora_core::INTENT_TYPE_URI.to_string(),
        principal: PrincipalIdentity::HostDev,
        goal: json!({
            "kind": "project_development",
            "project_id": project_id.as_str(),
            "summary": request.goal,
        }),
        target_session_id: None,
        target_branch_id: None,
        created_at: now,
        annotations: BTreeMap::from([
            ("owner".to_string(), json!("host_control_plane")),
            ("source_content_persisted".to_string(), json!(false)),
        ]),
    };
    let intent_ref = commit_json_artifact(
        state.runtime.as_ref(),
        plurora_core::INTENT_TYPE_URI,
        &intent,
        Vec::new(),
        BTreeMap::new(),
    )
    .await
    .map_err(|error| internal_development_error("failed to store development intent", error))?;

    let (operations, preconditions) = prepare_change_operations(
        state.runtime.as_ref(),
        &workspace.root,
        &request.operations,
        &base_summary.sha256,
    )
    .await?;
    let required_authority =
        required_development_authority(workspace.ownership, &request.verification);
    let change_set = ChangeSet {
        id: change_set_id.clone(),
        change_set_type_uri: plurora_core::CHANGE_SET_TYPE_URI.to_string(),
        intent_id: intent.id.clone(),
        operations,
        preconditions,
        required_authority,
        expected_effects: json!({
            "kind": if workspace.ownership == DevelopmentWorkspaceOwnership::ManagedExternal {
                "workspace_tree_promotion"
            } else {
                "verified_patch_bundle"
            },
            "project_id": project_id.as_str(),
            "verification": request.verification,
            "linked_local_source_write": false,
        }),
        idempotency_key: request.idempotency_key.clone(),
        created_at: now,
    };
    let change_references = std::iter::once(intent_ref.digest.clone())
        .chain(
            change_set
                .operations
                .iter()
                .flat_map(|operation| operation.input_refs.iter().map(|item| item.digest.clone())),
        )
        .collect::<Vec<_>>();
    let change_set_ref = commit_json_artifact(
        state.runtime.as_ref(),
        plurora_core::CHANGE_SET_TYPE_URI,
        &change_set,
        change_references,
        BTreeMap::from([("project_id".to_string(), json!(project_id.as_str()))]),
    )
    .await
    .map_err(|error| internal_development_error("failed to store development change set", error))?;
    let policy_decision = PolicyDecision {
        id: format!("decision-{}", uuid::Uuid::new_v4().simple()),
        decision_type_uri: plurora_core::POLICY_DECISION_TYPE_URI.to_string(),
        change_set_id: change_set.id.clone(),
        outcome: PolicyDecisionOutcome::RequiresApproval,
        principal: PrincipalIdentity::HostDev,
        reason: Some(
            "filesystem writes and code verification require explicit Host approval".to_string(),
        ),
        evaluated_authority: change_set.required_authority.clone(),
        decided_at: now,
        policy_ref: None,
    };
    let policy_decision_ref = commit_json_artifact(
        state.runtime.as_ref(),
        plurora_core::POLICY_DECISION_TYPE_URI,
        &policy_decision,
        vec![change_set_ref.digest.clone()],
        BTreeMap::new(),
    )
    .await
    .map_err(|error| internal_development_error("failed to store policy decision", error))?;

    let timestamp_ms = now_millis();
    let record = DevelopmentChangeRecord {
        schema_version: 1,
        revision: 1,
        project_id,
        workspace_ownership: workspace.ownership,
        intent,
        intent_ref,
        change_set,
        change_set_ref,
        policy_decision,
        policy_decision_ref,
        approval_decision: None,
        approval_ref: None,
        status: DevelopmentChangeStatus::Drafted,
        base_tree_digest: base_summary.sha256,
        proposed_tree_digest: None,
        verification_plan: request.verification,
        verification_result: None,
        managed_promotion: None,
        recovery_kind: None,
        commit: None,
        deployment: None,
        error: None,
        created_at_ms: timestamp_ms,
        updated_at_ms: timestamp_ms,
        idempotency_key: request.idempotency_key,
    };
    let persisted = persist_new_record(
        state,
        DevelopmentChangeSnapshot {
            record: record.clone(),
            request_fingerprint,
        },
    )
    .await
    .map_err(|error| development_persistence_error("failed to persist development draft", error))?;
    Ok(persisted)
}

async fn prepare_change_operations<S>(
    runtime: &Runtime<S>,
    workspace_root: &FsPath,
    requests: &[DevelopmentFileOperationRequest],
    base_tree_digest: &str,
) -> Result<(Vec<ChangeOperation>, Vec<ChangePrecondition>), ServiceError>
where
    S: EventStore,
{
    let mut operations = Vec::with_capacity(requests.len());
    let mut preconditions = vec![ChangePrecondition {
        kind: "workspace.tree_digest".to_string(),
        target: None,
        expected: json!({ "sha256": base_tree_digest }),
    }];
    let mut seen = HashSet::new();
    let mut existing_total_bytes = 0u64;
    for request in requests {
        let (raw_path, write) = match request {
            DevelopmentFileOperationRequest::FileWrite {
                path,
                content,
                executable,
            } => (path, Some((content, *executable))),
            DevelopmentFileOperationRequest::FileDelete { path } => (path, None),
        };
        let (relative, portable) = safe_workspace_relative_path(raw_path)?;
        if !seen.insert(portable.clone()) {
            return Err(ServiceError::with_status(
                StatusCode::BAD_REQUEST,
                "development operations must not target the same path more than once",
            ));
        }
        let current = inspect_workspace_target(workspace_root, &relative).map_err(|error| {
            tracing::warn!(target = %portable, error = %error, "development target inspection failed");
            ServiceError::with_status(
                StatusCode::BAD_REQUEST,
                "a development target failed path or file ownership validation",
            )
        })?;
        existing_total_bytes = existing_total_bytes.saturating_add(current.size_bytes);
        if current.size_bytes > DEVELOPMENT_MAX_EXISTING_FILE_BYTES
            || existing_total_bytes > DEVELOPMENT_MAX_EXISTING_TOTAL_BYTES
        {
            return Err(ServiceError::with_status(
                StatusCode::BAD_REQUEST,
                "development target backup size limit exceeded",
            ));
        }
        preconditions.push(ChangePrecondition {
            kind: "workspace.file".to_string(),
            target: Some(portable.clone()),
            expected: json!({
                "exists": current.exists,
                "sha256": current.sha256,
                "size_bytes": current.size_bytes,
                "executable": current.executable,
            }),
        });

        match write {
            Some((content, executable)) => {
                let artifact = commit_source_file_artifact(runtime, content.as_bytes(), &portable)
                    .await
                    .map_err(|error| {
                        internal_development_error("failed to store source change", error)
                    })?;
                operations.push(ChangeOperation {
                    op: "host.file.write".to_string(),
                    target: Some(portable),
                    input_refs: vec![artifact],
                    payload: json!({
                        "encoding": "utf-8",
                        "executable": executable.unwrap_or(current.executable),
                    }),
                });
            }
            None => operations.push(ChangeOperation {
                op: "host.file.delete".to_string(),
                target: Some(portable),
                input_refs: Vec::new(),
                payload: json!({}),
            }),
        }
    }
    Ok((operations, preconditions))
}

fn required_development_authority(
    ownership: DevelopmentWorkspaceOwnership,
    verification: &DevelopmentVerificationPlan,
) -> Vec<String> {
    let mut authority = vec![
        "host.project.develop".to_string(),
        "host.workspace.stage".to_string(),
    ];
    if ownership == DevelopmentWorkspaceOwnership::ManagedExternal {
        authority.push("host.workspace.promote".to_string());
    }
    if let DevelopmentVerificationPlan::DockerBuild { network_mode, .. } = verification {
        authority.push("host.docker.build".to_string());
        if *network_mode == DevelopmentNetworkMode::Bridge {
            authority.push("host.network.egress".to_string());
        }
    }
    authority
}

fn ensure_descriptor_matches_workspace(
    workspace: &ResolvedProjectWorkspace,
    actual_digest: &str,
) -> anyhow::Result<()> {
    if workspace.ownership != DevelopmentWorkspaceOwnership::ManagedExternal {
        return Ok(());
    }
    let descriptor_digest = workspace
        .descriptor
        .project
        .external
        .as_ref()
        .and_then(|external| external.source_digest.as_deref())
        .ok_or_else(|| anyhow::anyhow!("managed workspace descriptor digest is missing"))?;
    anyhow::ensure!(
        descriptor_digest == actual_digest,
        "managed workspace content digest does not match its descriptor"
    );
    Ok(())
}

#[derive(Debug, Clone)]
struct ResolvedProjectWorkspace {
    descriptor: ProjectDescriptor,
    descriptor_path: PathBuf,
    descriptor_handle: Arc<same_file::Handle>,
    root: PathBuf,
    ownership: DevelopmentWorkspaceOwnership,
    managed_external_root: Option<PathBuf>,
}

fn resolve_project_workspace<S>(
    state: &AppState<S>,
    project_id: &ProjectId,
) -> anyhow::Result<ResolvedProjectWorkspace>
where
    S: EventStore,
{
    let entry = state
        .runtime
        .config()
        .project_registry
        .get(project_id)
        .ok_or_else(|| anyhow::anyhow!("project is not registered"))?;
    let data_dir = plurora_core::paths::data_dir()?;
    let data_dir = canonical_real_directory(&data_dir, "data directory")?;
    let projects = canonical_owned_directory(&data_dir, "projects", "projects root")?;
    let project_dir = canonical_owned_directory(&projects, project_id.as_str(), "project root")?;
    let descriptor_path = project_dir.join("project.yaml");
    let descriptor_handle = Arc::new(open_expected_project_descriptor(
        &descriptor_path,
        &entry.descriptor,
    )?);

    match entry.descriptor.project.project_type {
        ProjectType::PluroraNative => {
            let root = canonical_owned_directory(&project_dir, "workspace", "project workspace")?;
            Ok(ResolvedProjectWorkspace {
                descriptor: entry.descriptor,
                descriptor_path,
                descriptor_handle,
                root,
                ownership: DevelopmentWorkspaceOwnership::NativeManaged,
                managed_external_root: None,
            })
        }
        ProjectType::ExternalWorkspace | ProjectType::ExternalWrapped => {
            let external = entry
                .descriptor
                .project
                .external
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("external project metadata is missing"))?;
            let workspace_root = external
                .workspace_root
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| anyhow::anyhow!("external workspace root is missing"))?;
            let workspace_path = PathBuf::from(workspace_root);
            anyhow::ensure!(
                workspace_path.is_absolute(),
                "workspace root must be absolute"
            );
            let root = canonical_real_directory(&workspace_path, "external workspace root")?;
            match external.workspace_ownership {
                Some(ExternalWorkspaceOwnership::Managed) => {
                    let workspaces =
                        canonical_owned_directory(&data_dir, "workspaces", "workspaces root")?;
                    let external_root = canonical_owned_directory(
                        &workspaces,
                        "external",
                        "external workspaces root",
                    )?;
                    let managed_project_root = canonical_owned_directory(
                        &external_root,
                        project_id.as_str(),
                        "managed external project root",
                    )?;
                    anyhow::ensure!(
                        root.parent() == Some(managed_project_root.as_path()),
                        "managed workspace escaped its project root"
                    );
                    let digest = external
                        .source_digest
                        .as_deref()
                        .ok_or_else(|| anyhow::anyhow!("managed workspace digest is missing"))?;
                    let digest_dir = digest_directory_name(digest)?;
                    anyhow::ensure!(
                        root.file_name().and_then(|value| value.to_str())
                            == Some(digest_dir.as_str()),
                        "managed workspace path does not match its source digest"
                    );
                    Ok(ResolvedProjectWorkspace {
                        descriptor: entry.descriptor,
                        descriptor_path,
                        descriptor_handle,
                        root,
                        ownership: DevelopmentWorkspaceOwnership::ManagedExternal,
                        managed_external_root: Some(managed_project_root),
                    })
                }
                Some(ExternalWorkspaceOwnership::LinkedLocal) => Ok(ResolvedProjectWorkspace {
                    descriptor: entry.descriptor,
                    descriptor_path,
                    descriptor_handle,
                    root,
                    ownership: DevelopmentWorkspaceOwnership::LinkedLocal,
                    managed_external_root: None,
                }),
                None => anyhow::bail!("external workspace ownership is missing"),
            }
        }
    }
}

fn open_expected_project_descriptor(
    path: &FsPath,
    expected: &ProjectDescriptor,
) -> anyhow::Result<same_file::Handle> {
    let metadata = fs::symlink_metadata(path)?;
    anyhow::ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "project descriptor must be a real file"
    );
    let mut file = fs::File::open(path)?;
    let handle = same_file::Handle::from_file(file.try_clone()?)?;
    let opened = file.metadata()?;
    let current = fs::symlink_metadata(path)?;
    anyhow::ensure!(
        opened.is_file()
            && current.is_file()
            && !current.file_type().is_symlink()
            && same_file::Handle::from_path(path)? == handle,
        "project descriptor changed while it was being opened"
    );
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() <= 1024 * 1024,
        "project descriptor exceeds 1 MiB"
    );
    let actual: ProjectDescriptor = serde_yaml::from_slice(&bytes)?;
    anyhow::ensure!(
        serde_json::to_value(&actual)? == serde_json::to_value(expected)?,
        "project descriptor changed after it was loaded into the registry"
    );
    anyhow::ensure!(
        same_file::Handle::from_path(path)? == handle,
        "project descriptor changed while it was being read"
    );
    Ok(handle)
}

fn canonical_real_directory(path: &FsPath, label: &str) -> anyhow::Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {label} {}", path.display()))?;
    anyhow::ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "{label} must be a real directory, not a symlink"
    );
    fs::canonicalize(path)
        .with_context(|| format!("failed to canonicalize {label} {}", path.display()))
}

fn canonical_owned_directory(parent: &FsPath, name: &str, label: &str) -> anyhow::Result<PathBuf> {
    let canonical = canonical_real_directory(&parent.join(name), label)?;
    anyhow::ensure!(
        canonical.parent() == Some(parent),
        "{label} escaped its Host-owned parent"
    );
    Ok(canonical)
}

fn ensure_owned_directory(parent: &FsPath, name: &str, label: &str) -> anyhow::Result<PathBuf> {
    let path = parent.join(name);
    match fs::symlink_metadata(&path) {
        Ok(metadata) => anyhow::ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "{label} must be a real directory, not a symlink"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&path)
            .with_context(|| format!("failed to create {label} {}", path.display()))?,
        Err(error) => return Err(error.into()),
    }
    let canonical = fs::canonicalize(&path)?;
    anyhow::ensure!(
        canonical.parent() == Some(parent),
        "{label} escaped its Host-owned parent"
    );
    Ok(canonical)
}

fn digest_directory_name(digest: &str) -> anyhow::Result<String> {
    let value = digest
        .strip_prefix("sha256:")
        .ok_or_else(|| anyhow::anyhow!("workspace digest must use sha256"))?;
    anyhow::ensure!(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')),
        "workspace digest must contain 64 lowercase hexadecimal characters"
    );
    Ok(value.to_string())
}

fn safe_workspace_relative_path(raw: &str) -> Result<(PathBuf, String), ServiceError> {
    if raw.is_empty() || raw.len() > 512 || raw.contains('\\') || raw.contains('\0') {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "development target path is empty, too long, or non-portable",
        ));
    }
    let path = FsPath::new(raw);
    if path.is_absolute() {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "development target path must be relative",
        ));
    }
    let mut components = Vec::new();
    for component in path.components() {
        let Component::Normal(component) = component else {
            return Err(ServiceError::with_status(
                StatusCode::BAD_REQUEST,
                "development target path contains a special component",
            ));
        };
        let Some(component) = component.to_str() else {
            return Err(ServiceError::with_status(
                StatusCode::BAD_REQUEST,
                "development target path must be UTF-8",
            ));
        };
        if component.is_empty() || component.len() > 255 || forbidden_workspace_component(component)
        {
            return Err(ServiceError::with_status(
                StatusCode::BAD_REQUEST,
                "development target path enters a protected or unsupported location",
            ));
        }
        components.push(component.to_string());
    }
    if components.is_empty() {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "development target path must identify a file",
        ));
    }
    let portable = components.join("/");
    Ok((components.iter().collect::<PathBuf>(), portable))
}

fn forbidden_workspace_component(component: &str) -> bool {
    let lower = component.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        ".git"
            | ".hg"
            | ".svn"
            | ".plurora"
            | ".env"
            | ".npmrc"
            | ".pypirc"
            | ".netrc"
            | "id_rsa"
            | "id_ed25519"
            | "credentials"
    )
}

#[derive(Debug)]
struct WorkspaceFileState {
    exists: bool,
    sha256: Option<String>,
    size_bytes: u64,
    executable: bool,
}

fn inspect_workspace_target(
    root: &FsPath,
    relative: &FsPath,
) -> anyhow::Result<WorkspaceFileState> {
    let target = safe_target_path(root, relative, false)?;
    let metadata = match fs::symlink_metadata(&target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(WorkspaceFileState {
                exists: false,
                sha256: None,
                size_bytes: 0,
                executable: false,
            })
        }
        Err(error) => return Err(error.into()),
    };
    anyhow::ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "development target must be a regular file or absent"
    );
    Ok(WorkspaceFileState {
        exists: true,
        sha256: Some(sha256_file(&target)?),
        size_bytes: metadata.len(),
        executable: file_is_executable(&metadata),
    })
}

fn safe_target_path(
    root: &FsPath,
    relative: &FsPath,
    create_parents: bool,
) -> anyhow::Result<PathBuf> {
    let root = canonical_real_directory(root, "workspace root")?;
    let components = relative.components().collect::<Vec<_>>();
    anyhow::ensure!(!components.is_empty(), "target path is empty");
    let mut current = root.clone();
    for component in &components[..components.len() - 1] {
        let Component::Normal(name) = component else {
            anyhow::bail!("target path contains a special component");
        };
        let next = current.join(name);
        match fs::symlink_metadata(&next) {
            Ok(metadata) => anyhow::ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "target ancestor must be a real directory"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && create_parents => {
                fs::create_dir(&next)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(root.join(relative));
            }
            Err(error) => return Err(error.into()),
        }
        let canonical = fs::canonicalize(&next)?;
        anyhow::ensure!(
            canonical.parent() == Some(current.as_path()),
            "target ancestor escaped its workspace parent"
        );
        current = canonical;
    }
    let target = root.join(relative);
    anyhow::ensure!(
        target.starts_with(&root),
        "target escaped the workspace root"
    );
    Ok(target)
}

fn sha256_file(path: &FsPath) -> anyhow::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

#[cfg(unix)]
fn file_is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn file_is_executable(_metadata: &fs::Metadata) -> bool {
    false
}

async fn workspace_tree_hash(path: &FsPath) -> anyhow::Result<plurora_runtime::WorkspaceTreeHash> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        plurora_runtime::compute_external_workspace_tree_hash(&path)
    })
    .await
    .context("workspace hashing task failed")?
}

fn validate_draft_request(request: &DevelopmentDraftRequest) -> Result<(), ServiceError> {
    validate_short_text(&request.goal, "development goal", 4096)?;
    if request.operations.is_empty() || request.operations.len() > DEVELOPMENT_MAX_OPERATIONS {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            format!("development request must contain 1..={DEVELOPMENT_MAX_OPERATIONS} operations"),
        ));
    }
    if let Some(key) = request.idempotency_key.as_deref() {
        validate_identifier(key, "idempotency_key", 128)?;
    }
    if let Some(digest) = request.expected_tree_digest.as_deref() {
        digest_directory_name(digest).map_err(|_| {
            ServiceError::with_status(
                StatusCode::BAD_REQUEST,
                "expected_tree_digest must be a lowercase sha256 digest",
            )
        })?;
    }
    let mut total_bytes = 0usize;
    for operation in &request.operations {
        match operation {
            DevelopmentFileOperationRequest::FileWrite { path, content, .. } => {
                safe_workspace_relative_path(path)?;
                if content.as_bytes().len() > DEVELOPMENT_MAX_FILE_BYTES {
                    return Err(ServiceError::with_status(
                        StatusCode::BAD_REQUEST,
                        "development source file exceeds the per-file content limit",
                    ));
                }
                total_bytes = total_bytes.saturating_add(content.len());
                if content.contains('\0') || contains_obvious_private_secret(content) {
                    return Err(ServiceError::with_status(
                        StatusCode::BAD_REQUEST,
                        "development source content contains binary data or obvious raw secret material",
                    ));
                }
            }
            DevelopmentFileOperationRequest::FileDelete { path } => {
                safe_workspace_relative_path(path)?;
            }
        }
    }
    if total_bytes > DEVELOPMENT_MAX_TOTAL_INPUT_BYTES {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "development source content exceeds the request byte limit",
        ));
    }
    if let DevelopmentVerificationPlan::DockerBuild {
        dockerfile,
        timeout_secs,
        ..
    } = &request.verification
    {
        safe_workspace_relative_path(dockerfile)?;
        if timeout_secs.is_some_and(|value| value == 0 || value > 3600) {
            return Err(ServiceError::with_status(
                StatusCode::BAD_REQUEST,
                "docker verification timeout_secs must be in 1..=3600",
            ));
        }
    }
    Ok(())
}

fn validate_short_text(value: &str, label: &str, max_len: usize) -> Result<(), ServiceError> {
    if value.trim().is_empty() || value.len() > max_len || contains_obvious_private_secret(value) {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            format!("{label} is empty, too long, or contains obvious raw secret material"),
        ));
    }
    Ok(())
}

fn validate_identifier(value: &str, label: &str, max_len: usize) -> Result<(), ServiceError> {
    if value.is_empty()
        || value.len() > max_len
        || value.starts_with('.')
        || value.contains("..")
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            format!("{label} contains unsupported characters"),
        ));
    }
    Ok(())
}

fn contains_obvious_private_secret(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    value.contains("-----BEGIN PRIVATE KEY-----")
        || value.contains("-----BEGIN OPENSSH PRIVATE KEY-----")
        || value.contains("github_pat_")
        || value.contains("ghp_")
        || value.contains("AKIA")
        || lower.contains("authorization: bearer ")
        || lower.contains("password=")
        || lower.contains("secret=")
        || lower.contains("token=sk-")
}

fn development_request_fingerprint(request: &DevelopmentDraftRequest) -> anyhow::Result<String> {
    let bytes = serde_json::to_vec(request)?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn development_change_set_id(project_id: &ProjectId, request: &DevelopmentDraftRequest) -> String {
    let Some(idempotency_key) = request.idempotency_key.as_deref() else {
        return format!("chg-{}", uuid::Uuid::new_v4().simple());
    };
    let mut hasher = Sha256::new();
    hasher.update(b"plurora.host-development.idempotency.v1\0");
    hasher.update(project_id.as_str().as_bytes());
    hasher.update(b"\0");
    hasher.update(idempotency_key.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    format!("chg-{}", &digest[..32])
}

async fn commit_source_file_artifact<S>(
    runtime: &Runtime<S>,
    bytes: &[u8],
    target: &str,
) -> anyhow::Result<ArtifactDescriptor>
where
    S: EventStore,
{
    let mut annotations = BTreeMap::new();
    annotations.insert("target".to_string(), json!(target));
    annotations.insert("encoding".to_string(), json!("utf-8"));
    runtime
        .commit_artifact(ArtifactCommitRequest {
            artifact_type_uri: SOURCE_FILE_ARTIFACT_TYPE_URI.to_string(),
            media_type: "text/plain; charset=utf-8".to_string(),
            bytes: bytes.to_vec().into(),
            references: Vec::new(),
            annotations,
        })
        .await
        .map_err(Into::into)
}

async fn commit_json_artifact<S, T>(
    runtime: &Runtime<S>,
    artifact_type_uri: &str,
    value: &T,
    references: Vec<String>,
    annotations: BTreeMap<String, Value>,
) -> anyhow::Result<ArtifactDescriptor>
where
    S: EventStore,
    T: Serialize,
{
    let bytes = serde_json::to_vec(value)?;
    runtime
        .commit_artifact(ArtifactCommitRequest {
            artifact_type_uri: artifact_type_uri.to_string(),
            media_type: "application/json".to_string(),
            bytes: bytes.into(),
            references,
            annotations,
        })
        .await
        .map_err(Into::into)
}

async fn materialize_patch_bundle<S>(
    runtime: &Runtime<S>,
    record: &DevelopmentChangeRecord,
) -> anyhow::Result<DevelopmentPatchBundle>
where
    S: EventStore,
{
    let mut operations = Vec::with_capacity(record.change_set.operations.len());
    for operation in &record.change_set.operations {
        let target = operation
            .target
            .clone()
            .ok_or_else(|| anyhow::anyhow!("development operation target is missing"))?;
        match operation.op.as_str() {
            "host.file.write" => {
                anyhow::ensure!(
                    operation.input_refs.len() == 1,
                    "file.write must contain one content artifact"
                );
                let descriptor = &operation.input_refs[0];
                anyhow::ensure!(
                    descriptor.artifact_type_uri == SOURCE_FILE_ARTIFACT_TYPE_URI,
                    "file.write content artifact has the wrong type"
                );
                let bytes = runtime.read_artifact(descriptor).await?;
                let content = String::from_utf8(bytes.to_vec())
                    .context("source content artifact is not UTF-8")?;
                operations.push(DevelopmentPatchBundleOperation::FileWrite {
                    path: target,
                    content,
                    executable: operation
                        .payload
                        .get("executable")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    content_digest: descriptor.digest.clone(),
                });
            }
            "host.file.delete" => {
                anyhow::ensure!(
                    operation.input_refs.is_empty(),
                    "file.delete must not contain content artifacts"
                );
                operations.push(DevelopmentPatchBundleOperation::FileDelete { path: target });
            }
            other => anyhow::bail!("unsupported development operation '{other}'"),
        }
    }
    Ok(DevelopmentPatchBundle {
        schema_version: 1,
        project_id: record.project_id.clone(),
        change_set_id: record.change_set.id.clone(),
        base_tree_digest: record.base_tree_digest.clone(),
        operations,
    })
}

fn parse_project_id(raw: &str) -> Result<ProjectId, ServiceError> {
    ProjectId::new(raw)
        .map_err(|_| ServiceError::with_status(StatusCode::BAD_REQUEST, "project_id is invalid"))
}

fn ensure_project_registered<S>(
    state: &AppState<S>,
    project_id: &ProjectId,
) -> Result<(), ServiceError>
where
    S: EventStore,
{
    if state
        .runtime
        .config()
        .project_registry
        .get(project_id)
        .is_none()
    {
        return Err(ServiceError::with_status(
            StatusCode::NOT_FOUND,
            "project was not found",
        ));
    }
    Ok(())
}

fn change_for_project<S>(
    state: &AppState<S>,
    project_id: &ProjectId,
    change_set_id: &str,
) -> Result<DevelopmentChangeRecord, ServiceError>
where
    S: EventStore,
{
    let record = state.development.get(change_set_id).ok_or_else(|| {
        ServiceError::with_status(StatusCode::NOT_FOUND, "development change was not found")
    })?;
    if &record.project_id != project_id {
        return Err(ServiceError::with_status(
            StatusCode::NOT_FOUND,
            "development change was not found",
        ));
    }
    Ok(record)
}

fn internal_development_error(context: &str, error: impl Into<anyhow::Error>) -> ServiceError {
    let error = error.into();
    tracing::warn!(error = %error, "{context}");
    ServiceError::with_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("{context}; details redacted"),
    )
}

fn development_persistence_error(context: &str, error: anyhow::Error) -> ServiceError {
    if error.to_string().contains("development journal conflict") {
        tracing::info!(error = %error, "{context}");
        ServiceError::with_status(
            StatusCode::CONFLICT,
            "development state changed concurrently; refresh before retrying",
        )
    } else {
        internal_development_error(context, error)
    }
}

fn safe_error_message(error: &anyhow::Error) -> String {
    let message = error.to_string();
    if message.contains("idempotency_key") {
        "idempotency_key conflicts with an existing development request".to_string()
    } else if message.contains("still being created") {
        "an identical development draft is still being created".to_string()
    } else if message.contains("global concurrency") {
        "development global concurrency limit reached".to_string()
    } else if message.contains("already executing") {
        "another development change is already executing for this project".to_string()
    } else {
        "development request could not be accepted".to_string()
    }
}

#[derive(Debug, Clone)]
struct DevelopmentScratch {
    workspace: PathBuf,
}

async fn run_development_change<S>(
    state: &AppState<S>,
    change_set_id: &str,
    authority: &HostAccessIdentity,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    let initial = state
        .development
        .get(change_set_id)
        .ok_or_else(|| anyhow::anyhow!("development change disappeared"))?;
    verify_development_authority(state, authority, &initial.project_id).await?;
    anyhow::ensure!(
        initial.status == DevelopmentChangeStatus::Staging,
        "development change is not in staging state"
    );
    let source = resolve_project_workspace(state, &initial.project_id)?;
    anyhow::ensure!(
        source.ownership == initial.workspace_ownership,
        "project workspace ownership changed after approval"
    );
    let live_base = workspace_tree_hash(&source.root).await?;
    ensure_descriptor_matches_workspace(&source, &live_base.sha256)?;
    anyhow::ensure!(
        live_base.sha256 == initial.base_tree_digest,
        "project workspace changed after the development draft was approved"
    );

    let scratch = create_development_scratch(&initial.project_id, change_set_id)?;
    if let Err(error) = copy_workspace_snapshot(&source.root, &scratch.workspace).await {
        cleanup_change_root(&initial.project_id, change_set_id);
        return Err(error);
    }
    let copied = workspace_tree_hash(&scratch.workspace).await?;
    anyhow::ensure!(
        copied.sha256 == initial.base_tree_digest,
        "scratch snapshot did not reproduce the approved workspace tree"
    );
    apply_change_set_to_scratch(state.runtime.as_ref(), &initial, &scratch.workspace).await?;
    let proposed = workspace_tree_hash(&scratch.workspace).await?;

    let verifying = update_development_record(state, change_set_id, |record| {
        record.status = DevelopmentChangeStatus::Verifying;
        record.proposed_tree_digest = Some(proposed.sha256.clone());
        record.error = None;
    })
    .await?;
    verify_development_authority(state, authority, &verifying.project_id).await?;
    let verification =
        verify_development_scratch(state, &verifying, &scratch.workspace, authority).await?;
    verify_development_authority(state, authority, &verifying.project_id).await?;

    if verifying.workspace_ownership != DevelopmentWorkspaceOwnership::ManagedExternal {
        let final_record = finalize_development_success(
            state,
            verifying,
            verification,
            DevelopmentChangeStatus::Verified,
            "host.workspace.patch.verified",
        )
        .await?;
        persist_record(state, final_record).await?;
        cleanup_change_root(&initial.project_id, change_set_id);
        return Ok(());
    }

    let source = resolve_project_workspace(state, &verifying.project_id)?;
    let current = workspace_tree_hash(&source.root).await?;
    ensure_descriptor_matches_workspace(&source, &current.sha256)?;
    anyhow::ensure!(
        current.sha256 == verifying.base_tree_digest,
        "project workspace changed before promotion"
    );
    let proposed_digest = verifying
        .proposed_tree_digest
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("proposed workspace digest is missing"))?;
    let scratch_digest = workspace_tree_hash(&scratch.workspace).await?;
    anyhow::ensure!(
        scratch_digest.sha256 == proposed_digest,
        "verified scratch workspace changed before promotion"
    );
    let promotion = prepare_managed_promotion(&verifying, &source).await?;
    let promoting = update_development_record(state, change_set_id, |record| {
        record.status = DevelopmentChangeStatus::Promoting;
        record.verification_result = Some(verification.clone());
        record.managed_promotion = Some(promotion.clone());
    })
    .await?;

    renew_current_development_host_lease(state).await?;
    verify_development_authority(state, authority, &promoting.project_id).await?;
    let rollback =
        promote_development_workspace(state, &promoting, &source, &scratch, authority).await?;
    if let Err(error) = verify_development_authority(state, authority, &promoting.project_id).await
    {
        rollback_promotion(state, rollback)?;
        return Err(error);
    }
    let final_record = match finalize_development_success(
        state,
        promoting,
        verification,
        DevelopmentChangeStatus::Committed,
        "host.workspace.promote",
    )
    .await
    {
        Ok(record) => record,
        Err(error) => {
            rollback_promotion(state, rollback)?;
            return Err(error);
        }
    };
    if let Err(error) = persist_record(state, final_record).await {
        rollback_promotion(state, rollback)?;
        return Err(error);
    }
    cleanup_change_root(&initial.project_id, change_set_id);
    Ok(())
}

async fn update_development_record<S, F>(
    state: &AppState<S>,
    change_set_id: &str,
    mutate: F,
) -> anyhow::Result<DevelopmentChangeRecord>
where
    S: EventStore,
    F: FnOnce(&mut DevelopmentChangeRecord),
{
    let change_lock = state.development.lock_for(change_set_id);
    let _guard = change_lock.lock().await;
    let mut record = state
        .development
        .get(change_set_id)
        .ok_or_else(|| anyhow::anyhow!("development change disappeared"))?;
    anyhow::ensure!(!record.status.terminal(), "development change is terminal");
    mutate(&mut record);
    record.revision += 1;
    record.updated_at_ms = now_millis();
    persist_record(state, record.clone()).await?;
    Ok(record)
}

fn create_development_scratch(
    project_id: &ProjectId,
    change_set_id: &str,
) -> anyhow::Result<DevelopmentScratch> {
    validate_identifier(change_set_id, "change_set_id", 64)
        .map_err(|_| anyhow::anyhow!("development change id is invalid"))?;
    let data_dir = canonical_real_directory(&plurora_core::paths::data_dir()?, "data directory")?;
    let projects = canonical_owned_directory(&data_dir, "projects", "projects root")?;
    let project = canonical_owned_directory(&projects, project_id.as_str(), "project root")?;
    let development = ensure_owned_directory(&project, "development", "development root")?;
    let change_path = development.join(change_set_id);
    match fs::symlink_metadata(&change_path) {
        Ok(_) => anyhow::bail!("development scratch already exists"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    fs::create_dir(&change_path)?;
    let change_root =
        canonical_owned_directory(&development, change_set_id, "development change root")?;
    let workspace = ensure_owned_directory(&change_root, "workspace", "scratch workspace")?;
    Ok(DevelopmentScratch { workspace })
}

#[derive(Debug, Default)]
struct WorkspaceCopyStats {
    files: u64,
    directories: u64,
    bytes: u64,
}

async fn copy_workspace_snapshot(source: &FsPath, destination: &FsPath) -> anyhow::Result<()> {
    let source = source.to_path_buf();
    let destination = destination.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let source = canonical_real_directory(&source, "source workspace")?;
        let destination = canonical_real_directory(&destination, "scratch workspace")?;
        anyhow::ensure!(
            !source.starts_with(&destination) && !destination.starts_with(&source),
            "source and scratch workspace roots must not overlap"
        );
        let mut stats = WorkspaceCopyStats::default();
        copy_workspace_directory(&source, &source, &destination, &mut stats)
    })
    .await
    .context("workspace copy task failed")?
}

fn copy_workspace_directory(
    source_root: &FsPath,
    source_dir: &FsPath,
    destination_dir: &FsPath,
    stats: &mut WorkspaceCopyStats,
) -> anyhow::Result<()> {
    let directory_handle = validated_snapshot_directory_handle(source_root, source_dir)?;
    let mut entries = fs::read_dir(source_dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name();
        if name
            .to_str()
            .is_some_and(development_snapshot_excluded_name)
        {
            continue;
        }
        let source = entry.path();
        let destination = destination_dir.join(&name);
        let metadata = fs::symlink_metadata(&source)?;
        if metadata.file_type().is_symlink() {
            anyhow::bail!(
                "development scratch does not support source symlinks: {}",
                source.strip_prefix(source_root)?.display()
            );
        }
        if metadata.is_dir() {
            stats.directories = stats.directories.saturating_add(1);
            anyhow::ensure!(
                stats.directories <= DEVELOPMENT_WORKSPACE_MAX_DIRECTORIES,
                "development workspace directory limit exceeded"
            );
            fs::create_dir(&destination)?;
            copy_workspace_directory(source_root, &source, &destination, stats)?;
        } else if metadata.is_file() {
            stats.files = stats.files.saturating_add(1);
            anyhow::ensure!(
                stats.files <= DEVELOPMENT_WORKSPACE_MAX_FILES,
                "development workspace file limit exceeded"
            );
            copy_workspace_file_bounded(&source, &destination, &metadata, stats)?;
        } else {
            anyhow::bail!("development workspace contains an unsupported file type");
        }
    }
    anyhow::ensure!(
        validated_snapshot_directory_handle(source_root, source_dir)? == directory_handle,
        "workspace directory changed during snapshot copy"
    );
    Ok(())
}

fn validated_snapshot_directory_handle(
    source_root: &FsPath,
    source_dir: &FsPath,
) -> anyhow::Result<same_file::Handle> {
    let metadata = fs::symlink_metadata(source_dir)?;
    anyhow::ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "development workspace contains a directory symlink"
    );
    let canonical = fs::canonicalize(source_dir)?;
    anyhow::ensure!(
        canonical.starts_with(source_root),
        "development workspace directory escaped its root"
    );
    Ok(same_file::Handle::from_path(source_dir)?)
}

fn copy_workspace_file_bounded(
    source: &FsPath,
    destination: &FsPath,
    inspected: &fs::Metadata,
    stats: &mut WorkspaceCopyStats,
) -> anyhow::Result<()> {
    ensure_single_link_source(inspected)?;
    let input = fs::File::open(source)?;
    let opened_handle = same_file::Handle::from_file(input.try_clone()?)?;
    let opened = input.metadata()?;
    anyhow::ensure!(
        opened.is_file() && same_file_identity(inspected, &opened),
        "workspace file changed while the snapshot was being opened"
    );
    ensure_single_link_source(&opened)?;
    let remaining = DEVELOPMENT_WORKSPACE_MAX_BYTES
        .checked_sub(stats.bytes)
        .ok_or_else(|| anyhow::anyhow!("development workspace byte limit exceeded"))?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let copied = std::io::copy(&mut input.take(remaining.saturating_add(1)), &mut output)?;
    anyhow::ensure!(
        copied <= remaining,
        "development workspace byte limit exceeded"
    );
    anyhow::ensure!(
        copied == opened.len(),
        "workspace file size changed during snapshot copy"
    );
    output.flush()?;
    output.sync_all()?;
    fs::set_permissions(destination, inspected.permissions())?;
    let after = fs::symlink_metadata(source)?;
    anyhow::ensure!(
        after.is_file()
            && !after.file_type().is_symlink()
            && same_file_identity(inspected, &after)
            && same_file::Handle::from_path(source)? == opened_handle
            && after.len() == copied,
        "workspace file changed during snapshot copy"
    );
    stats.bytes = stats.bytes.saturating_add(copied);
    Ok(())
}

#[cfg(unix)]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.created().ok() == right.created().ok()
}

#[cfg(unix)]
fn ensure_single_link_source(metadata: &fs::Metadata) -> anyhow::Result<()> {
    use std::os::unix::fs::MetadataExt;
    anyhow::ensure!(
        metadata.nlink() == 1,
        "development workspace hard-linked files are not supported"
    );
    Ok(())
}

#[cfg(not(unix))]
fn ensure_single_link_source(_metadata: &fs::Metadata) -> anyhow::Result<()> {
    Ok(())
}

fn development_snapshot_excluded_name(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".hg"
            | ".svn"
            | ".DS_Store"
            | "node_modules"
            | "target"
            | ".venv"
            | "venv"
            | "__pycache__"
            | ".pytest_cache"
            | ".mypy_cache"
            | ".ruff_cache"
    )
}

async fn apply_change_set_to_scratch<S>(
    runtime: &Runtime<S>,
    record: &DevelopmentChangeRecord,
    scratch: &FsPath,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    let bundle = materialize_patch_bundle(runtime, record).await?;
    for operation in bundle.operations {
        match operation {
            DevelopmentPatchBundleOperation::FileWrite {
                path,
                content,
                executable,
                ..
            } => {
                let (relative, _) = safe_workspace_relative_path(&path)
                    .map_err(|_| anyhow::anyhow!("stored development target path is invalid"))?;
                let target = safe_target_path(scratch, &relative, true)?;
                if let Ok(metadata) = fs::symlink_metadata(&target) {
                    anyhow::ensure!(
                        metadata.is_file() && !metadata.file_type().is_symlink(),
                        "scratch write target must be a regular file or absent"
                    );
                }
                write_file_atomic(&target, content.as_bytes(), executable)?;
            }
            DevelopmentPatchBundleOperation::FileDelete { path } => {
                let (relative, _) = safe_workspace_relative_path(&path)
                    .map_err(|_| anyhow::anyhow!("stored development target path is invalid"))?;
                let target = safe_target_path(scratch, &relative, false)?;
                match fs::symlink_metadata(&target) {
                    Ok(metadata) => {
                        anyhow::ensure!(
                            metadata.is_file() && !metadata.file_type().is_symlink(),
                            "scratch delete target must be a regular file"
                        );
                        fs::remove_file(target)?;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
    }
    Ok(())
}

fn write_file_atomic(path: &FsPath, bytes: &[u8], executable: bool) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("file target parent is missing"))?;
    let parent = canonical_real_directory(parent, "file target parent")?;
    let mut temporary = tempfile::NamedTempFile::new_in(&parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file_mut().flush()?;
    temporary.as_file().sync_all()?;
    set_file_executable(temporary.path(), executable)?;
    temporary
        .persist(path)
        .map_err(|error| anyhow::anyhow!("failed to atomically replace file: {}", error.error))?;
    sync_directory(&parent)?;
    Ok(())
}

#[cfg(unix)]
fn set_file_executable(path: &FsPath, executable: bool) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    let mut mode = permissions.mode();
    if executable {
        mode |= 0o111;
    } else {
        mode &= !0o111;
    }
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_file_executable(_path: &FsPath, _executable: bool) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn sync_directory(path: &FsPath) -> anyhow::Result<()> {
    fs::File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_directory(_path: &FsPath) -> anyhow::Result<()> {
    Ok(())
}

async fn verify_development_scratch<S>(
    state: &AppState<S>,
    record: &DevelopmentChangeRecord,
    scratch: &FsPath,
    authority: &HostAccessIdentity,
) -> anyhow::Result<DevelopmentVerificationResult>
where
    S: EventStore,
{
    match &record.verification_plan {
        DevelopmentVerificationPlan::StaticValidation => {
            let payload = json!({
                "kind": "static_validation",
                "succeeded": true,
                "change_set_id": record.change_set.id,
                "tree_digest": record.proposed_tree_digest,
                "network_mode": "none",
                "code_executed": false,
            });
            let artifact_ref = commit_json_artifact(
                state.runtime.as_ref(),
                DEVELOPMENT_RESULT_ARTIFACT_TYPE_URI,
                &payload,
                vec![record.change_set_ref.digest.clone()],
                BTreeMap::from([("result_kind".to_string(), json!("static_validation"))]),
            )
            .await?;
            Ok(DevelopmentVerificationResult {
                kind: "static_validation".to_string(),
                succeeded: true,
                network_mode: DevelopmentNetworkMode::None,
                image: None,
                log_tail: None,
                artifact_ref,
                deployment_artifact_ref: None,
            })
        }
        DevelopmentVerificationPlan::DockerBuild {
            dockerfile,
            network_mode,
            timeout_secs,
        } => {
            let build_id = development_build_id(&record.change_set.id);
            let descriptor_hash = record
                .change_set_ref
                .digest
                .strip_prefix("sha256:")
                .unwrap_or(&record.change_set_ref.digest);
            let build_input = json!({
                "approved": true,
                "strategy": "dockerfile",
                "project_id": record.project_id.as_str(),
                "build_id": build_id,
                "development_change_id": record.change_set.id,
                "context_dir": scratch.to_string_lossy(),
                "dockerfile": dockerfile,
                "build_descriptor_hash": descriptor_hash,
                "network_mode": network_mode.as_str(),
                "build_timeout_secs": timeout_secs.unwrap_or(900),
                "max_context_bytes": DEVELOPMENT_WORKSPACE_MAX_BYTES,
                "max_context_files": DEVELOPMENT_WORKSPACE_MAX_FILES,
            });
            let prepared = tokio::task::spawn_blocking({
                let build_input = build_input.clone();
                move || plurora_runtime::prepare_docker_build_context(&build_input)
            })
            .await
            .context("deployable build context task failed")??;
            let deployment_artifact_ref = state
                .runtime
                .commit_artifact(ArtifactCommitRequest {
                    artifact_type_uri: DEVELOPMENT_BUILD_CONTEXT_ARTIFACT_TYPE_URI.to_string(),
                    media_type: "application/x-tar".to_string(),
                    bytes: prepared.bytes.into(),
                    references: vec![record.change_set_ref.digest.clone()],
                    annotations: BTreeMap::from([
                        ("project_id".to_string(), json!(record.project_id.as_str())),
                        ("change_set_id".to_string(), json!(record.change_set.id)),
                        (
                            "tree_digest".to_string(),
                            json!(record.proposed_tree_digest),
                        ),
                        ("dockerfile".to_string(), json!(dockerfile)),
                        ("files".to_string(), json!(prepared.files)),
                        ("total_bytes".to_string(), json!(prepared.total_bytes)),
                    ]),
                })
                .await?;
            let mut verified_build_input = build_input;
            verified_build_input["expected_context_digest"] = json!(deployment_artifact_ref.digest);
            verify_development_authority(state, authority, &record.project_id).await?;
            let context = development_authority_context(
                authority,
                &record.project_id,
                "host_development_verification",
            );
            let output = invoke_docker_runtime_lab(
                state,
                &context,
                "plurora/docker-runtime-lab/build_image",
                verified_build_input,
            )
            .await?;
            let image = require_built_image(&output)?;
            let diagnostic_log_digest = output
                .get("log_tail")
                .and_then(Value::as_str)
                .map(|value| format!("sha256:{:x}", Sha256::digest(value.as_bytes())));
            verify_development_authority(state, authority, &record.project_id).await?;
            let cleanup = invoke_docker_runtime_lab(
                state,
                &context,
                "plurora/docker-runtime-lab/remove_image",
                json!({
                    "approved": true,
                    "project_id": record.project_id.as_str(),
                    "build_id": build_id,
                    "development_change_id": record.change_set.id,
                }),
            )
            .await?;
            anyhow::ensure!(
                cleanup
                    .get("image_removed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                "development verification image cleanup failed"
            );
            verify_development_authority(state, authority, &record.project_id).await?;
            let payload = json!({
                "kind": "docker_build",
                "succeeded": true,
                "change_set_id": record.change_set.id,
                "tree_digest": record.proposed_tree_digest,
                "network_mode": network_mode,
                "image_ref": image,
                "image_retained": false,
                "deployment_artifact_ref": deployment_artifact_ref,
                "diagnostic_log_digest": diagnostic_log_digest,
            });
            let artifact_ref = commit_json_artifact(
                state.runtime.as_ref(),
                DEVELOPMENT_RESULT_ARTIFACT_TYPE_URI,
                &payload,
                vec![
                    record.change_set_ref.digest.clone(),
                    deployment_artifact_ref.digest.clone(),
                ],
                BTreeMap::from([("result_kind".to_string(), json!("docker_build"))]),
            )
            .await?;
            Ok(DevelopmentVerificationResult {
                kind: "docker_build".to_string(),
                succeeded: true,
                network_mode: *network_mode,
                image: None,
                log_tail: None,
                artifact_ref,
                deployment_artifact_ref: Some(deployment_artifact_ref),
            })
        }
    }
}

fn development_build_id(change_set_id: &str) -> String {
    format!(
        "dev-{}",
        change_set_id.strip_prefix("chg-").unwrap_or(change_set_id)
    )
}

async fn finalize_development_success<S>(
    state: &AppState<S>,
    mut record: DevelopmentChangeRecord,
    verification: DevelopmentVerificationResult,
    status: DevelopmentChangeStatus,
    effect_kind: &str,
) -> anyhow::Result<DevelopmentChangeRecord>
where
    S: EventStore,
{
    let bundle = materialize_patch_bundle(state.runtime.as_ref(), &record).await?;
    let bundle_references = record
        .change_set
        .operations
        .iter()
        .flat_map(|operation| operation.input_refs.iter().map(|item| item.digest.clone()))
        .collect::<Vec<_>>();
    let bundle_ref = commit_json_artifact(
        state.runtime.as_ref(),
        DEVELOPMENT_BUNDLE_ARTIFACT_TYPE_URI,
        &bundle,
        bundle_references,
        BTreeMap::from([
            ("project_id".to_string(), json!(record.project_id.as_str())),
            ("change_set_id".to_string(), json!(record.change_set.id)),
        ]),
    )
    .await?;
    let actual = json!({
        "project_id": record.project_id.as_str(),
        "workspace_ownership": record.workspace_ownership,
        "base_tree_digest": record.base_tree_digest,
        "proposed_tree_digest": record.proposed_tree_digest,
        "source_workspace_modified": status == DevelopmentChangeStatus::Committed,
        "linked_local_source_write": false,
    });
    let mut result_references = vec![
        record.change_set_ref.digest.clone(),
        verification.artifact_ref.digest.clone(),
        bundle_ref.digest.clone(),
    ];
    if let Some(artifact) = verification.deployment_artifact_ref.as_ref() {
        result_references.push(artifact.digest.clone());
    }
    let result_ref = commit_json_artifact(
        state.runtime.as_ref(),
        DEVELOPMENT_RESULT_ARTIFACT_TYPE_URI,
        &actual,
        result_references,
        BTreeMap::from([("result_kind".to_string(), json!(effect_kind))]),
    )
    .await?;
    let component_ref = commit_json_artifact(
        state.runtime.as_ref(),
        COMPONENT_EVIDENCE_TYPE_URI,
        &json!({
            "component": "host/development-executor",
            "version": 1,
            "boundary": "host_control_plane",
        }),
        Vec::new(),
        BTreeMap::new(),
    )
    .await?;
    let started_at = Utc::now();
    let completed_at = Utc::now();
    let approval_ref = record
        .approval_ref
        .clone()
        .ok_or_else(|| anyhow::anyhow!("approval artifact is missing"))?;
    let mut output_refs = vec![
        verification.artifact_ref.clone(),
        bundle_ref.clone(),
        result_ref.clone(),
    ];
    if let Some(artifact) = verification.deployment_artifact_ref.clone() {
        output_refs.push(artifact);
    }
    let receipt = EffectReceipt {
        schema_version: 1,
        receipt_type_uri: plurora_core::EFFECT_RECEIPT_TYPE_URI.to_string(),
        receipt_id: format!("receipt-{}", uuid::Uuid::new_v4().simple()),
        effect_kind: effect_kind.to_string(),
        principal: PrincipalIdentity::HostDev,
        component_ref,
        protocol_profiles: vec!["host/control/v1".to_string()],
        input_refs: vec![record.change_set_ref.clone()],
        output_refs: output_refs.clone(),
        external_effect_refs: Vec::new(),
        authority_ref: None,
        policy_decision_ref: Some(approval_ref.clone()),
        approval_ref: Some(approval_ref),
        status: EffectTerminalStatus::Succeeded,
        started_at,
        completed_at,
        latency_ms: 0,
        usage: json!({}),
        cost: json!({}),
        trace_id: record.change_set.id.clone(),
        parent_receipts: Vec::new(),
        replay_mode: EffectReplayMode::Live,
        scope: EffectScope::default(),
        planned: record.change_set.expected_effects.clone(),
        actual,
        annotations: BTreeMap::from([("owner".to_string(), json!("host_control_plane"))]),
    };
    let receipt_ref = commit_json_artifact(
        state.runtime.as_ref(),
        EFFECT_RECEIPT_TYPE_URI,
        &receipt,
        receipt.referenced_digests(),
        BTreeMap::new(),
    )
    .await?;
    let now = Utc::now();
    record.commit = Some(ChangeCommit {
        id: format!("commit-{}", uuid::Uuid::new_v4().simple()),
        commit_type_uri: plurora_core::CHANGE_COMMIT_TYPE_URI.to_string(),
        change_set_id: record.change_set.id.clone(),
        status: ChangeCommitStatus::Committed,
        started_at,
        completed_at: now,
        operation_receipts: vec![receipt_ref],
        result_refs: output_refs,
        error: None,
        branch_id: None,
        idempotency_key: record.change_set.idempotency_key.clone(),
    });
    record.status = status;
    record.verification_result = Some(verification);
    record.recovery_kind = None;
    record.error = None;
    record.revision += 1;
    record.updated_at_ms = now_millis();
    Ok(record)
}

enum PromotionRollback {
    Managed {
        descriptor_path: PathBuf,
        descriptor_handle: Arc<same_file::Handle>,
        previous_descriptor: ProjectDescriptor,
        destination: PathBuf,
        destination_created: bool,
    },
}

async fn prepare_managed_promotion(
    record: &DevelopmentChangeRecord,
    source: &ResolvedProjectWorkspace,
) -> anyhow::Result<DevelopmentManagedPromotion> {
    anyhow::ensure!(
        record.workspace_ownership == DevelopmentWorkspaceOwnership::ManagedExternal,
        "only managed external workspaces support automatic promotion"
    );
    let proposed_tree_digest = record
        .proposed_tree_digest
        .clone()
        .ok_or_else(|| anyhow::anyhow!("proposed tree digest is missing"))?;
    let digest_dir = digest_directory_name(&proposed_tree_digest)?;
    let managed_root = source
        .managed_external_root
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("managed external root is missing"))?;
    let destination = managed_root.join(digest_dir);
    let destination_preexisting = match fs::symlink_metadata(&destination) {
        Ok(metadata) => {
            anyhow::ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "managed workspace destination must be a real directory"
            );
            let destination = fs::canonicalize(&destination)?;
            anyhow::ensure!(
                destination.parent() == Some(managed_root),
                "managed workspace destination escaped its project root"
            );
            let existing = workspace_tree_hash(&destination).await?;
            anyhow::ensure!(
                existing.sha256 == proposed_tree_digest,
                "managed workspace digest destination contains different content"
            );
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    Ok(DevelopmentManagedPromotion {
        previous_tree_digest: record.base_tree_digest.clone(),
        proposed_tree_digest,
        destination_preexisting,
    })
}

async fn promote_development_workspace<S>(
    state: &AppState<S>,
    record: &DevelopmentChangeRecord,
    source: &ResolvedProjectWorkspace,
    scratch: &DevelopmentScratch,
    authority: &HostAccessIdentity,
) -> anyhow::Result<PromotionRollback>
where
    S: EventStore,
{
    anyhow::ensure!(
        record.workspace_ownership == DevelopmentWorkspaceOwnership::ManagedExternal,
        "only managed external workspaces support automatic promotion"
    );
    promote_managed_external_workspace(state, record, source, scratch, authority).await
}

async fn promote_managed_external_workspace<S>(
    state: &AppState<S>,
    record: &DevelopmentChangeRecord,
    source: &ResolvedProjectWorkspace,
    scratch: &DevelopmentScratch,
    authority: &HostAccessIdentity,
) -> anyhow::Result<PromotionRollback>
where
    S: EventStore,
{
    verify_development_authority(state, authority, &record.project_id).await?;
    let digest = record
        .proposed_tree_digest
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("proposed tree digest is missing"))?;
    let digest_dir = digest_directory_name(digest)?;
    let managed_root = source
        .managed_external_root
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("managed external root is missing"))?;
    let destination = managed_root.join(&digest_dir);
    let promotion = record
        .managed_promotion
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("durable managed promotion plan is missing"))?;
    anyhow::ensure!(
        promotion.previous_tree_digest == record.base_tree_digest
            && promotion.proposed_tree_digest == digest,
        "durable managed promotion plan does not match the change"
    );
    let destination_created = !promotion.destination_preexisting;
    if promotion.destination_preexisting {
        let destination = canonical_real_directory(&destination, "managed workspace destination")?;
        anyhow::ensure!(
            destination.parent() == Some(managed_root),
            "managed workspace destination escaped its project root"
        );
        let existing = workspace_tree_hash(&destination).await?;
        anyhow::ensure!(
            existing.sha256 == digest,
            "managed workspace digest destination contains different content"
        );
        fs::remove_dir_all(&scratch.workspace)?;
    } else {
        anyhow::ensure!(
            fs::symlink_metadata(&destination)
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
            "managed workspace destination appeared after promotion was prepared"
        );
        fs::rename(&scratch.workspace, &destination).with_context(|| {
            format!(
                "failed to promote scratch workspace {}",
                scratch.workspace.display()
            )
        })?;
    }
    let destination = canonical_real_directory(&destination, "promoted managed workspace")?;
    anyhow::ensure!(
        destination.parent() == Some(managed_root),
        "promoted managed workspace escaped its project root"
    );

    let previous_descriptor = source.descriptor.clone();
    let mut updated_descriptor = previous_descriptor.clone();
    let external = updated_descriptor
        .project
        .external
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("external project metadata is missing"))?;
    external.workspace_root = Some(destination.to_string_lossy().to_string());
    external.source_digest = Some(digest.to_string());
    external.workspace_ownership = Some(ExternalWorkspaceOwnership::Managed);
    updated_descriptor.validate()?;
    verify_development_authority(state, authority, &record.project_id).await?;
    let updated_descriptor_handle = match write_project_descriptor_atomic(
        &source.descriptor_path,
        source.descriptor_handle.as_ref(),
        &updated_descriptor,
    ) {
        Ok(handle) => Arc::new(handle),
        Err(error) => {
            state
                .runtime
                .config()
                .project_registry
                .register(previous_descriptor.clone())
                .ok();
            if destination_created {
                move_promoted_workspace_back(&destination, &scratch.workspace).ok();
            }
            return Err(error);
        }
    };
    if let Err(error) = state
        .runtime
        .config()
        .project_registry
        .register(updated_descriptor)
    {
        write_project_descriptor_atomic(
            &source.descriptor_path,
            updated_descriptor_handle.as_ref(),
            &previous_descriptor,
        )
        .ok();
        if destination_created {
            move_promoted_workspace_back(&destination, &scratch.workspace).ok();
        }
        return Err(error);
    }
    Ok(PromotionRollback::Managed {
        descriptor_path: source.descriptor_path.clone(),
        descriptor_handle: updated_descriptor_handle,
        previous_descriptor,
        destination,
        destination_created,
    })
}

fn move_promoted_workspace_back(destination: &FsPath, scratch: &FsPath) -> anyhow::Result<()> {
    if fs::symlink_metadata(scratch).is_ok() {
        fs::remove_dir_all(scratch)?;
    }
    fs::rename(destination, scratch)?;
    Ok(())
}

fn rollback_promotion<S>(state: &AppState<S>, rollback: PromotionRollback) -> anyhow::Result<()>
where
    S: EventStore,
{
    match rollback {
        PromotionRollback::Managed {
            descriptor_path,
            descriptor_handle,
            previous_descriptor,
            destination,
            destination_created,
        } => {
            write_project_descriptor_atomic(
                &descriptor_path,
                descriptor_handle.as_ref(),
                &previous_descriptor,
            )?;
            state
                .runtime
                .config()
                .project_registry
                .register(previous_descriptor)?;
            if destination_created {
                match fs::symlink_metadata(&destination) {
                    Ok(metadata) => {
                        anyhow::ensure!(
                            metadata.is_dir() && !metadata.file_type().is_symlink(),
                            "promotion rollback destination must be a real directory"
                        );
                        fs::remove_dir_all(destination)?;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
            }
            Ok(())
        }
    }
}

async fn reconcile_managed_promotion<S>(
    state: &AppState<S>,
    mut record: DevelopmentChangeRecord,
    authority: &HostAccessIdentity,
) -> anyhow::Result<DevelopmentChangeRecord>
where
    S: EventStore,
{
    anyhow::ensure!(
        record.workspace_ownership == DevelopmentWorkspaceOwnership::ManagedExternal,
        "only managed external promotion can be reconciled"
    );
    verify_development_authority(state, authority, &record.project_id).await?;
    let promotion = record
        .managed_promotion
        .clone()
        .ok_or_else(|| anyhow::anyhow!("durable managed promotion plan is missing"))?;
    let current = resolve_project_workspace(state, &record.project_id)?;
    let current_tree = workspace_tree_hash(&current.root).await?;
    ensure_descriptor_matches_workspace(&current, &current_tree.sha256)?;
    let current_digest = current
        .descriptor
        .project
        .external
        .as_ref()
        .and_then(|external| external.source_digest.as_deref())
        .ok_or_else(|| anyhow::anyhow!("managed descriptor digest is missing"))?;

    if current_digest == promotion.proposed_tree_digest {
        verify_development_authority(state, authority, &record.project_id).await?;
        let verification = record
            .verification_result
            .clone()
            .ok_or_else(|| anyhow::anyhow!("durable verification result is missing"))?;
        let committed = finalize_development_success(
            state,
            record,
            verification,
            DevelopmentChangeStatus::Committed,
            "host.workspace.promote.reconciled",
        )
        .await?;
        let committed = persist_record(state, committed).await?;
        cleanup_change_root(&committed.project_id, &committed.change_set.id);
        return Ok(committed);
    }

    anyhow::ensure!(
        current_digest == promotion.previous_tree_digest,
        "managed descriptor points to neither the previous nor proposed tree"
    );
    if !promotion.destination_preexisting {
        let managed_root = current
            .managed_external_root
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("managed external root is missing"))?;
        let destination =
            managed_root.join(digest_directory_name(&promotion.proposed_tree_digest)?);
        match fs::symlink_metadata(&destination) {
            Ok(metadata) => {
                anyhow::ensure!(
                    metadata.is_dir() && !metadata.file_type().is_symlink(),
                    "recovery destination must be a real directory"
                );
                let destination = fs::canonicalize(destination)?;
                anyhow::ensure!(
                    destination.parent() == Some(managed_root),
                    "recovery destination escaped its managed project root"
                );
                let digest = workspace_tree_hash(&destination).await?;
                anyhow::ensure!(
                    digest.sha256 == promotion.proposed_tree_digest,
                    "recovery destination content did not match the proposed digest"
                );
                verify_development_authority(state, authority, &record.project_id).await?;
                fs::remove_dir_all(destination)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    record.revision = record.revision.saturating_add(1);
    record.updated_at_ms = now_millis();
    record.status = DevelopmentChangeStatus::Failed;
    record.error =
        Some("managed promotion was reconciled to the previous immutable workspace".to_string());
    record.commit = Some(failed_change_commit(
        &record.change_set.id,
        "managed promotion was rolled back before descriptor activation",
    ));
    let failed = persist_record(state, record).await?;
    cleanup_change_root(&failed.project_id, &failed.change_set.id);
    Ok(failed)
}

async fn reconcile_docker_verification<S>(
    state: &AppState<S>,
    mut record: DevelopmentChangeRecord,
    authority: &HostAccessIdentity,
) -> anyhow::Result<DevelopmentChangeRecord>
where
    S: EventStore,
{
    anyhow::ensure!(
        record.recovery_kind == Some(DevelopmentRecoveryKind::DockerVerification),
        "change does not require Docker verification recovery"
    );
    verify_development_authority(state, authority, &record.project_id).await?;
    let context =
        development_authority_context(authority, &record.project_id, "host_development_recovery");
    let build_id = development_build_id(&record.change_set.id);
    let cleanup = invoke_docker_runtime_lab(
        state,
        &context,
        "plurora/docker-runtime-lab/remove_image",
        json!({
            "approved": true,
            "project_id": record.project_id.as_str(),
            "build_id": build_id,
            "development_change_id": record.change_set.id,
        }),
    )
    .await?;
    anyhow::ensure!(
        cleanup
            .get("image_removed")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "Docker verification image cleanup was not confirmed"
    );
    record.revision = record.revision.saturating_add(1);
    record.updated_at_ms = now_millis();
    record.status = DevelopmentChangeStatus::Failed;
    record.recovery_kind = None;
    record.error = Some(
        "interrupted Docker verification was reconciled and its labeled image was removed or absent"
            .to_string(),
    );
    record.commit = Some(failed_change_commit(
        &record.change_set.id,
        "Docker verification was interrupted before a durable success receipt",
    ));
    let failed = persist_record(state, record).await?;
    cleanup_change_root(&failed.project_id, &failed.change_set.id);
    Ok(failed)
}

fn write_project_descriptor_atomic(
    path: &FsPath,
    expected_handle: &same_file::Handle,
    descriptor: &ProjectDescriptor,
) -> anyhow::Result<same_file::Handle> {
    let metadata = fs::symlink_metadata(path)?;
    anyhow::ensure!(
        metadata.is_file()
            && !metadata.file_type().is_symlink()
            && same_file::Handle::from_path(path)? == *expected_handle,
        "project descriptor must be a real file"
    );
    let bytes = serde_yaml::to_string(descriptor)?.into_bytes();
    write_file_atomic(path, &bytes, false)?;
    let metadata = fs::symlink_metadata(path)?;
    anyhow::ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "project descriptor replacement must be a real file"
    );
    Ok(same_file::Handle::from_path(path)?)
}

async fn complete_failed_change<S>(state: &AppState<S>, change_set_id: &str) -> anyhow::Result<()>
where
    S: EventStore,
{
    let change_lock = state.development.lock_for(change_set_id);
    let _guard = change_lock.lock().await;
    let Some(mut record) = state.development.get(change_set_id) else {
        return Ok(());
    };
    if record.status.terminal() {
        return Ok(());
    }
    record.revision += 1;
    record.updated_at_ms = now_millis();
    if record.status == DevelopmentChangeStatus::Promoting {
        record.status = DevelopmentChangeStatus::RecoveryRequired;
        record.recovery_kind = Some(DevelopmentRecoveryKind::ManagedPromotion);
        record.error = Some(
            "development promotion was interrupted; Host recovery must reconcile the descriptor and promoted tree"
                .to_string(),
        );
        record.commit = Some(recovery_required_change_commit(
            &record.change_set.id,
            DevelopmentRecoveryKind::ManagedPromotion,
        ));
    } else if record.status == DevelopmentChangeStatus::Verifying
        && matches!(
            record.verification_plan,
            DevelopmentVerificationPlan::DockerBuild { .. }
        )
    {
        record.status = DevelopmentChangeStatus::RecoveryRequired;
        record.recovery_kind = Some(DevelopmentRecoveryKind::DockerVerification);
        record.error = Some(
            "Docker verification was interrupted; the labeled verification image must be reconciled"
                .to_string(),
        );
        record.commit = Some(recovery_required_change_commit(
            &record.change_set.id,
            DevelopmentRecoveryKind::DockerVerification,
        ));
    } else {
        record.status = DevelopmentChangeStatus::Failed;
        record.recovery_kind = None;
        record.error = Some("development execution failed; details redacted".to_string());
        record.commit = Some(failed_change_commit(
            &record.change_set.id,
            "development execution failed; details redacted",
        ));
    }
    persist_record(state, record.clone()).await?;
    if record.status != DevelopmentChangeStatus::RecoveryRequired {
        cleanup_change_root(&record.project_id, change_set_id);
    }
    Ok(())
}

fn recovery_required_change_commit(
    change_set_id: &str,
    recovery_kind: DevelopmentRecoveryKind,
) -> ChangeCommit {
    let now = Utc::now();
    ChangeCommit {
        id: format!("commit-{}", uuid::Uuid::new_v4().simple()),
        commit_type_uri: plurora_core::CHANGE_COMMIT_TYPE_URI.to_string(),
        change_set_id: change_set_id.to_string(),
        status: ChangeCommitStatus::Partial,
        started_at: now,
        completed_at: Utc::now(),
        operation_receipts: Vec::new(),
        result_refs: Vec::new(),
        error: Some(format!(
            "{} requires recovery reconciliation",
            match recovery_kind {
                DevelopmentRecoveryKind::DockerVerification => "Docker verification",
                DevelopmentRecoveryKind::ManagedPromotion => "managed promotion",
            }
        )),
        branch_id: None,
        idempotency_key: None,
    }
}

fn failed_change_commit(change_set_id: &str, error: &str) -> ChangeCommit {
    let now = Utc::now();
    ChangeCommit {
        id: format!("commit-{}", uuid::Uuid::new_v4().simple()),
        commit_type_uri: plurora_core::CHANGE_COMMIT_TYPE_URI.to_string(),
        change_set_id: change_set_id.to_string(),
        status: ChangeCommitStatus::Failed,
        started_at: now,
        completed_at: Utc::now(),
        operation_receipts: Vec::new(),
        result_refs: Vec::new(),
        error: Some(error.to_string()),
        branch_id: None,
        idempotency_key: None,
    }
}

fn cleanup_change_root(project_id: &ProjectId, change_set_id: &str) {
    let result = (|| -> anyhow::Result<()> {
        let data_dir =
            canonical_real_directory(&plurora_core::paths::data_dir()?, "data directory")?;
        let projects = canonical_owned_directory(&data_dir, "projects", "projects root")?;
        let project = canonical_owned_directory(&projects, project_id.as_str(), "project root")?;
        let development = match fs::symlink_metadata(project.join("development")) {
            Ok(_) => canonical_owned_directory(&project, "development", "development root")?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        let change_path = development.join(change_set_id);
        match fs::symlink_metadata(&change_path) {
            Ok(metadata) => anyhow::ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "development change root must be a real directory"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        }
        let change_path = fs::canonicalize(change_path)?;
        anyhow::ensure!(
            change_path.parent() == Some(development.as_path()),
            "development change root escaped its parent"
        );
        fs::remove_dir_all(change_path)?;
        Ok(())
    })();
    if let Err(error) = result {
        tracing::warn!(
            project_id = %project_id,
            change_set_id,
            error = %error,
            "failed to clean development scratch"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use plurora_runtime::{
        InMemoryEventStore, PortLeaseRequest, PortProtocol, ProxyProtocol,
        ProxyRouteRegisterRequest, ProxyRouteUpstream, RuntimeConfig,
    };
    use tower::ServiceExt;

    fn artifact(seed: char) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_type_uri: "urn:test:artifact".to_string(),
            media_type: "application/json".to_string(),
            digest: format!("sha256:{}", seed.to_string().repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    fn record(status: DevelopmentChangeStatus) -> DevelopmentChangeRecord {
        let now = Utc::now();
        let project_id = ProjectId::new("project-1").unwrap();
        let intent = Intent {
            id: "intent-1".to_string(),
            intent_type_uri: plurora_core::INTENT_TYPE_URI.to_string(),
            principal: PrincipalIdentity::HostDev,
            goal: json!({ "summary": "test" }),
            target_session_id: None,
            target_branch_id: None,
            created_at: now,
            annotations: BTreeMap::new(),
        };
        let change_set = ChangeSet {
            id: "chg-0123456789abcdef".to_string(),
            change_set_type_uri: plurora_core::CHANGE_SET_TYPE_URI.to_string(),
            intent_id: intent.id.clone(),
            operations: Vec::new(),
            preconditions: Vec::new(),
            required_authority: vec!["host.project.develop".to_string()],
            expected_effects: json!({}),
            idempotency_key: Some("test-key".to_string()),
            created_at: now,
        };
        let policy_decision = PolicyDecision {
            id: "decision-1".to_string(),
            decision_type_uri: plurora_core::POLICY_DECISION_TYPE_URI.to_string(),
            change_set_id: change_set.id.clone(),
            outcome: PolicyDecisionOutcome::Allowed,
            principal: PrincipalIdentity::HostAdmin,
            reason: None,
            evaluated_authority: change_set.required_authority.clone(),
            decided_at: now,
            policy_ref: None,
        };
        DevelopmentChangeRecord {
            schema_version: 1,
            revision: 1,
            project_id,
            workspace_ownership: DevelopmentWorkspaceOwnership::ManagedExternal,
            intent,
            intent_ref: artifact('a'),
            change_set,
            change_set_ref: artifact('b'),
            policy_decision,
            policy_decision_ref: artifact('c'),
            approval_decision: None,
            approval_ref: Some(artifact('d')),
            status,
            base_tree_digest: format!("sha256:{}", "e".repeat(64)),
            proposed_tree_digest: Some(format!("sha256:{}", "f".repeat(64))),
            verification_plan: DevelopmentVerificationPlan::StaticValidation,
            verification_result: None,
            managed_promotion: None,
            recovery_kind: None,
            commit: None,
            deployment: None,
            error: None,
            created_at_ms: 1,
            updated_at_ms: 1,
            idempotency_key: Some("test-key".to_string()),
        }
    }

    fn deployment(status: DevelopmentDeploymentStatus) -> DevelopmentDeploymentRecord {
        DevelopmentDeploymentRecord {
            schema_version: 1,
            deployment_id: "dep-0123456789abcdef".to_string(),
            status,
            target_id: "local".to_string(),
            source_tree_digest: format!("sha256:{}", "1".repeat(64)),
            verification_ref: artifact('2'),
            build_context_ref: artifact('3'),
            authority_ref: artifact('4'),
            dockerfile: "Dockerfile".to_string(),
            network_mode: DevelopmentNetworkMode::None,
            container_port: 8080,
            port_name: "web".to_string(),
            route_id: "project-web".to_string(),
            route_access: ProxyRouteAccess::HostAuthenticated,
            health_path: Some("/healthz".to_string()),
            preview_route_id: "preview-0123456789abcdef".to_string(),
            preview_port_lease_id: None,
            target_deployment_id: None,
            build_id: "verified-0123456789abcdef".to_string(),
            build_descriptor_hash: format!("sha256:{}", "5".repeat(64)),
            build_operation_id: None,
            deployment_operation_id: None,
            preview: None,
            preview_ref: None,
            approval_decision: None,
            approval_ref: None,
            activation_revision_id: None,
            previous_revision_id: None,
            error: None,
            created_at_ms: 1,
            updated_at_ms: 1,
            idempotency_key: Some("preview-1".to_string()),
            request_digest: format!("sha256:{}", "6".repeat(64)),
        }
    }

    #[test]
    fn development_authority_fails_closed_for_unknown_device_grants() {
        let project_id = ProjectId::new("project-1").unwrap();
        let registry = HostAccessRegistry::default();
        assert!(validate_development_authority(
            &HostAccessIdentity::root(),
            &registry,
            &project_id
        )
        .is_ok());

        let mut device = HostAccessIdentity::root();
        device.kind = HostAccessIdentityKind::Device;
        device.grant_id = Some("missing-grant".to_string());
        assert!(validate_development_authority(&device, &registry, &project_id).is_err());
    }

    #[test]
    fn development_paths_reject_traversal_and_sensitive_locations() {
        assert!(safe_workspace_relative_path("src/lib.rs").is_ok());
        assert!(safe_workspace_relative_path("../outside").is_err());
        assert!(safe_workspace_relative_path(".git/config").is_err());
        assert!(safe_workspace_relative_path("config/.env").is_err());
        assert!(safe_workspace_relative_path("C:\\outside").is_err());
    }

    #[test]
    fn deployment_preview_requires_exact_verification_provenance() {
        let mut record = record(DevelopmentChangeStatus::Committed);
        record.verification_plan = DevelopmentVerificationPlan::DockerBuild {
            dockerfile: "Dockerfile".to_string(),
            network_mode: DevelopmentNetworkMode::None,
            timeout_secs: None,
        };
        let source_tree_digest = record.proposed_tree_digest.clone().unwrap();
        let mut context = artifact('1');
        context.artifact_type_uri = DEVELOPMENT_BUILD_CONTEXT_ARTIFACT_TYPE_URI.to_string();
        context.media_type = "application/x-tar".to_string();
        context.references = vec![record.change_set_ref.digest.clone()];
        context.annotations = BTreeMap::from([
            ("project_id".to_string(), json!(record.project_id.as_str())),
            ("change_set_id".to_string(), json!(record.change_set.id)),
            ("tree_digest".to_string(), json!(source_tree_digest)),
            ("dockerfile".to_string(), json!("Dockerfile")),
        ]);
        let mut result = artifact('2');
        result.artifact_type_uri = DEVELOPMENT_RESULT_ARTIFACT_TYPE_URI.to_string();
        result.references = vec![record.change_set_ref.digest.clone(), context.digest.clone()];
        let verification = DevelopmentVerificationResult {
            kind: "docker_build".to_string(),
            succeeded: true,
            network_mode: DevelopmentNetworkMode::None,
            image: None,
            log_tail: None,
            artifact_ref: result,
            deployment_artifact_ref: Some(context.clone()),
        };
        assert!(validate_deployment_verification_provenance(
            &record,
            &verification,
            "Dockerfile",
            DevelopmentNetworkMode::None,
            &source_tree_digest,
        )
        .is_ok());

        let mut tampered = verification;
        tampered
            .deployment_artifact_ref
            .as_mut()
            .unwrap()
            .annotations
            .insert(
                "tree_digest".to_string(),
                json!(format!("sha256:{}", "0".repeat(64))),
            );
        assert!(validate_deployment_verification_provenance(
            &record,
            &tampered,
            "Dockerfile",
            DevelopmentNetworkMode::None,
            &source_tree_digest,
        )
        .is_err());
    }

    #[test]
    fn deployment_preview_request_is_typed_and_label_safe() {
        let mut request = DevelopmentDeploymentPreviewRequest {
            target_id: "target-1".to_string(),
            container_port: 8080,
            port_name: "web".to_string(),
            route_id: "project-web".to_string(),
            route_access: ProxyRouteAccess::HostAuthenticated,
            health_path: Some("/healthz".to_string()),
            idempotency_key: Some("preview-1".to_string()),
        };
        assert!(validate_deployment_preview_request(&request).is_ok());
        request.health_path = Some("//remote.example".to_string());
        assert!(validate_deployment_preview_request(&request).is_err());
    }

    #[test]
    fn deployment_reconciliation_adopts_only_the_exact_durable_activation() {
        let mut record = record(DevelopmentChangeStatus::Committed);
        let mut deployment = deployment(DevelopmentDeploymentStatus::RecoveryRequired);
        let preview_ref = artifact('7');
        let approval_ref = artifact('8');
        let preview = DevelopmentDeploymentPreview {
            route_id: deployment.preview_route_id.clone(),
            public_url: "/p/preview/".to_string(),
            port_lease_id: "port-lease-1".to_string(),
            deployment: TargetDeploymentRef {
                deployment_id: "preview-deployment-1".to_string(),
                route_id: deployment.preview_route_id.clone(),
                port_lease_id: "port-lease-1".to_string(),
            },
            image: "plurora/verified:test".to_string(),
            image_id: format!("sha256:{}", "9".repeat(64)),
            container_id: "container-1".to_string(),
            container_name: Some("candidate-1".to_string()),
            build_operation_id: "operation-build".to_string(),
            deployment_operation_id: "operation-apply".to_string(),
            ready_at_ms: 1,
        };
        deployment.preview_port_lease_id = Some(preview.port_lease_id.clone());
        deployment.target_deployment_id = Some(preview.deployment.deployment_id.clone());
        deployment.build_operation_id = Some(preview.build_operation_id.clone());
        deployment.deployment_operation_id = Some(preview.deployment_operation_id.clone());
        deployment.preview_ref = Some(preview_ref.clone());
        deployment.approval_ref = Some(approval_ref.clone());
        let mut approval_decision = record.policy_decision.clone();
        approval_decision.change_set_id = format!(
            "{}:deployment:{}",
            record.change_set.id, deployment.deployment_id
        );
        deployment.approval_decision = Some(approval_decision);
        deployment.preview = Some(preview.clone());
        record.deployment = Some(deployment.clone());
        let mut active = DeploymentRevision {
            revision_id: "revision-1".to_string(),
            project_id: record.project_id.clone(),
            job_id: None,
            operation: DeploymentOperation::VerifiedActivate,
            parent_revision_id: None,
            created_at_ms: 1,
            target_id: deployment.target_id.clone(),
            source_kind: DeploymentSourceKind::VerifiedArtifact,
            source_url: format!("artifact:{}", deployment.build_context_ref.digest),
            ref_name: record.change_set.id.clone(),
            dockerfile: Some(deployment.dockerfile.clone()),
            container_port: deployment.container_port,
            port_name: deployment.port_name.clone(),
            route_id: deployment.route_id.clone(),
            route_access: deployment.route_access,
            health_path: deployment.health_path.clone(),
            image: preview.image_id.clone(),
            build_id: deployment.build_id.clone(),
            source_commit: deployment.source_tree_digest.clone(),
            build_descriptor_hash: deployment.build_descriptor_hash.clone(),
            strategy: "verified_artifact".to_string(),
            runtime_env: Vec::new(),
            verified_change_set_id: Some(record.change_set.id.clone()),
            verification_ref: Some(deployment.verification_ref.clone()),
            build_context_ref: Some(deployment.build_context_ref.clone()),
            preview_ref: Some(preview_ref),
            approval_ref: Some(approval_ref),
            verified_build_network_mode: Some(development_target_network_mode(
                deployment.network_mode,
            )),
            target_deployment: Some(preview.deployment.clone()),
            recoverable: true,
            recovery_blockers: Vec::new(),
            receipt: HostBuildDeployResponse {
                route_id: deployment.route_id.clone(),
                public_url: "/p/project-web/".to_string(),
                route_access: deployment.route_access,
                port_lease_id: preview.port_lease_id.clone(),
                container_id: preview.container_id.clone(),
                container_name: preview.container_name.clone(),
                image: preview.image_id,
                build_id: deployment.build_id.clone(),
                source_commit: deployment.source_tree_digest.clone(),
                build_descriptor_hash: deployment.build_descriptor_hash.clone(),
                strategy: "verified_artifact".to_string(),
                runtime_env: Vec::new(),
                runtime_mounts: Vec::new(),
                warnings: Vec::new(),
            },
        };
        assert!(verified_activation_matches_deployment(
            &active,
            &record,
            &deployment
        ));
        active.approval_ref = Some(artifact('0'));
        assert!(!verified_activation_matches_deployment(
            &active,
            &record,
            &deployment
        ));
        active.approval_ref = deployment.approval_ref.clone();
        active.receipt.container_id = "another-container".to_string();
        assert!(durable_revision_claims_deployment_candidate(
            &active,
            &deployment
        ));
        assert!(!verified_activation_matches_deployment(
            &active,
            &record,
            &deployment
        ));
    }

    #[tokio::test]
    async fn deployment_reconciliation_does_not_guess_an_unrecorded_port_lease() {
        let runtime = Arc::new(Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        ));
        let lease = runtime
            .config()
            .port_lease_registry
            .lease(PortLeaseRequest {
                target_id: "local".to_string(),
                port_name: "web".to_string(),
                protocol: PortProtocol::Tcp,
                requested_port: None,
            })
            .await
            .lease;
        let state = AppState {
            runtime: runtime.clone(),
            static_dir: None,
            access_token: None,
            app_base_domain: None,
            build_jobs: crate::build_deploy_job_registry(),
            development: development_registry(),
            host_access: crate::host_access_registry(),
            target_agents: crate::target_agent_registry(),
        };
        let deployment = deployment(DevelopmentDeploymentStatus::RecoveryRequired);

        assert!(unrecorded_preview_port_lease_may_exist(&state, &deployment).await);
        runtime
            .config()
            .proxy_route_registry
            .register(ProxyRouteRegisterRequest {
                route_id: Some("another-route".to_string()),
                upstream: ProxyRouteUpstream {
                    port_lease_id: lease.id.clone(),
                    port_name: lease.port_name.clone(),
                },
                protocol: ProxyProtocol::Http,
                access: ProxyRouteAccess::HostAuthenticated,
            })
            .await;
        assert!(!unrecorded_preview_port_lease_may_exist(&state, &deployment).await);
        runtime
            .config()
            .proxy_route_registry
            .unregister("another-route")
            .await;
        runtime
            .config()
            .port_lease_registry
            .release(&lease.id)
            .await;
        assert!(!unrecorded_preview_port_lease_may_exist(&state, &deployment).await);
    }

    #[tokio::test]
    async fn deployment_approval_evidence_binds_the_exact_preview() -> anyhow::Result<()> {
        let runtime = Arc::new(Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        ));
        let project_id = ProjectId::new("project-1")?;
        let change_set_id = "chg-0123456789abcdef";
        let target_id = "local";
        let source_tree_digest = format!("sha256:{}", "1".repeat(64));
        let context_ref = runtime
            .commit_artifact(ArtifactCommitRequest {
                artifact_type_uri: DEVELOPMENT_BUILD_CONTEXT_ARTIFACT_TYPE_URI.to_string(),
                media_type: "application/x-tar".to_string(),
                bytes: b"context".to_vec().into(),
                references: Vec::new(),
                annotations: BTreeMap::from([
                    ("project_id".to_string(), json!(project_id.as_str())),
                    ("change_set_id".to_string(), json!(change_set_id)),
                    ("tree_digest".to_string(), json!(source_tree_digest)),
                    ("dockerfile".to_string(), json!("Dockerfile")),
                ]),
            })
            .await?;
        let verification_ref = commit_json_artifact(
            runtime.as_ref(),
            DEVELOPMENT_RESULT_ARTIFACT_TYPE_URI,
            &json!({ "succeeded": true }),
            vec![context_ref.digest.clone()],
            BTreeMap::new(),
        )
        .await?;
        let deployment_id = "dep-0123456789abcdef";
        let authority_ref = commit_json_artifact(
            runtime.as_ref(),
            DEVELOPMENT_DEPLOYMENT_AUTHORITY_TYPE_URI,
            &json!({ "deployment_id": deployment_id }),
            vec![verification_ref.digest.clone(), context_ref.digest.clone()],
            BTreeMap::from([
                ("project_id".to_string(), json!(project_id.as_str())),
                ("change_set_id".to_string(), json!(change_set_id)),
                ("target_id".to_string(), json!(target_id)),
                ("deployment_id".to_string(), json!(deployment_id)),
            ]),
        )
        .await?;
        let preview = DevelopmentDeploymentPreview {
            route_id: "preview-0123456789abcdef".to_string(),
            public_url: "/p/preview-0123456789abcdef/".to_string(),
            port_lease_id: "port-lease-000001".to_string(),
            deployment: TargetDeploymentRef {
                deployment_id: "preview-dep-0123456789abcdef".to_string(),
                route_id: "preview-0123456789abcdef".to_string(),
                port_lease_id: "port-lease-000001".to_string(),
            },
            image: "plurora/verified:test".to_string(),
            image_id: format!("sha256:{}", "2".repeat(64)),
            container_id: "container-1".to_string(),
            container_name: Some("candidate-1".to_string()),
            build_operation_id: "operation-build".to_string(),
            deployment_operation_id: "operation-apply".to_string(),
            ready_at_ms: 1,
        };
        let preview_ref = commit_json_artifact(
            runtime.as_ref(),
            DEVELOPMENT_DEPLOYMENT_PREVIEW_TYPE_URI,
            &json!({
                "schema_version": 1,
                "project_id": project_id,
                "change_set_id": change_set_id,
                "deployment_id": deployment_id,
                "source_tree_digest": source_tree_digest,
                "verification_ref": verification_ref,
                "build_context_ref": context_ref,
                "authority_ref": authority_ref,
                "preview": preview,
            }),
            vec![
                verification_ref.digest.clone(),
                context_ref.digest.clone(),
                authority_ref.digest.clone(),
            ],
            BTreeMap::from([
                ("project_id".to_string(), json!(project_id.as_str())),
                ("change_set_id".to_string(), json!(change_set_id)),
                ("deployment_id".to_string(), json!(deployment_id)),
                ("target_id".to_string(), json!(target_id)),
                ("source_tree_digest".to_string(), json!(source_tree_digest)),
            ]),
        )
        .await?;
        let decision = PolicyDecision {
            id: "decision-deployment-1".to_string(),
            decision_type_uri: plurora_core::POLICY_DECISION_TYPE_URI.to_string(),
            change_set_id: format!("{change_set_id}:deployment:{deployment_id}"),
            outcome: PolicyDecisionOutcome::Allowed,
            principal: PrincipalIdentity::HostAdmin,
            reason: None,
            evaluated_authority: vec![
                "host.project.deploy".to_string(),
                format!("host.target.{target_id}"),
            ],
            decided_at: Utc::now(),
            policy_ref: None,
        };
        let approval_ref = commit_json_artifact(
            runtime.as_ref(),
            plurora_core::POLICY_DECISION_TYPE_URI,
            &decision,
            vec![
                preview_ref.digest.clone(),
                verification_ref.digest.clone(),
                context_ref.digest.clone(),
                authority_ref.digest.clone(),
            ],
            BTreeMap::from([
                ("role".to_string(), json!("explicit_deployment_approval")),
                ("project_id".to_string(), json!(project_id.as_str())),
                ("change_set_id".to_string(), json!(change_set_id)),
                ("deployment_id".to_string(), json!(deployment_id)),
                ("target_id".to_string(), json!(target_id)),
            ]),
        )
        .await?;

        let (stored_deployment_id, stored_preview, stored_authority) =
            read_verified_preview_evidence(
                runtime.as_ref(),
                &preview_ref,
                &project_id,
                change_set_id,
                target_id,
                &source_tree_digest,
                &verification_ref,
                &context_ref,
            )
            .await?;
        assert_eq!(stored_deployment_id, deployment_id);
        assert_eq!(stored_preview, preview);
        assert_eq!(stored_authority, authority_ref);
        assert_eq!(
            read_verified_deployment_approval(
                runtime.as_ref(),
                &approval_ref,
                &preview_ref,
                &verification_ref,
                &context_ref,
                &authority_ref,
                &project_id,
                change_set_id,
                deployment_id,
                target_id,
            )
            .await?,
            decision
        );

        let mut detached = approval_ref;
        detached
            .references
            .retain(|digest| digest != &preview_ref.digest);
        assert!(read_verified_deployment_approval(
            runtime.as_ref(),
            &detached,
            &preview_ref,
            &verification_ref,
            &context_ref,
            &authority_ref,
            &project_id,
            change_set_id,
            deployment_id,
            target_id,
        )
        .await
        .is_err());
        Ok(())
    }

    #[test]
    fn development_registry_idempotency_conflicts_fail_closed() {
        let registry = DevelopmentRegistry::default();
        let project_id = ProjectId::new("project-1").unwrap();
        assert!(matches!(
            registry
                .claim_draft(
                    &project_id,
                    Some("key"),
                    "sha256:first",
                    "chg-0123456789abcdef"
                )
                .unwrap(),
            DraftClaim::Reserved
        ));
        assert!(registry
            .claim_draft(
                &project_id,
                Some("key"),
                "sha256:different",
                "chg-fedcba9876543210"
            )
            .is_err());
    }

    #[test]
    fn development_registry_requires_an_installed_live_host_lease() {
        let registry = DevelopmentRegistry::default();
        assert!(registry.ensure_active_host_lease().is_err());
    }

    #[tokio::test]
    async fn development_journal_apply_is_idempotent_for_concurrent_refreshes() -> anyhow::Result<()>
    {
        let store = InMemoryEventStore::default();
        let snapshot = DevelopmentChangeSnapshot {
            record: record(DevelopmentChangeStatus::Drafted),
            request_fingerprint: "sha256:request".to_string(),
        };
        let event = append_development_journal_event(&store, &snapshot, 0)
            .await?
            .ok_or_else(|| anyhow::anyhow!("failed to append development snapshot"))?;
        let registry = DevelopmentRegistry::default();

        registry.apply_journal_event(&event)?;
        registry.apply_journal_event(&event)?;

        assert_eq!(
            registry.project_journal_next(&snapshot.record.project_id),
            1
        );
        assert_eq!(
            registry
                .get(&snapshot.record.change_set.id)
                .expect("development snapshot remains available")
                .revision,
            1
        );
        Ok(())
    }

    #[test]
    fn development_docker_build_ids_preserve_the_full_change_identity() {
        let left = development_build_id("chg-aaaaaaaaaaaaaaaaaaaaaaaa11111111");
        let right = development_build_id("chg-aaaaaaaaaaaaaaaaaaaaaaaa22222222");
        assert_ne!(left, right);
        assert!(left.ends_with("11111111"));
        assert!(right.ends_with("22222222"));
    }

    #[tokio::test]
    async fn development_hydration_interrupts_incomplete_execution() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let snapshot = DevelopmentChangeSnapshot {
            record: record(DevelopmentChangeStatus::Verifying),
            request_fingerprint: "sha256:request".to_string(),
        };
        append_development_journal_event(store.as_ref(), &snapshot, 0)
            .await?
            .ok_or_else(|| anyhow::anyhow!("failed to append initial development snapshot"))?;

        let registry = development_registry();
        let lease = acquire_development_host_lease(store.clone(), registry.clone()).await?;
        let loaded = hydrate_development_control_plane(store.clone(), registry.clone()).await?;
        assert_eq!(loaded, 1);
        let restored = registry.get("chg-0123456789abcdef").unwrap();
        assert_eq!(restored.status, DevelopmentChangeStatus::Failed);
        assert_eq!(restored.commit.unwrap().status, ChangeCommitStatus::Failed);
        assert_eq!(
            store
                .list_kind_prefix(DEVELOPMENT_JOURNAL_PREFIX)
                .await?
                .len(),
            2
        );
        release_development_host_lease(store, &lease).await?;
        Ok(())
    }

    #[tokio::test]
    async fn development_hydration_requires_recovery_for_interrupted_promotion(
    ) -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let mut record = record(DevelopmentChangeStatus::Promoting);
        record.managed_promotion = Some(DevelopmentManagedPromotion {
            previous_tree_digest: record.base_tree_digest.clone(),
            proposed_tree_digest: record.proposed_tree_digest.clone().unwrap(),
            destination_preexisting: false,
        });
        let snapshot = DevelopmentChangeSnapshot {
            record,
            request_fingerprint: "sha256:request".to_string(),
        };
        append_development_journal_event(store.as_ref(), &snapshot, 0)
            .await?
            .ok_or_else(|| anyhow::anyhow!("failed to append initial development snapshot"))?;

        let registry = development_registry();
        let lease = acquire_development_host_lease(store.clone(), registry.clone()).await?;
        hydrate_development_control_plane(store.clone(), registry.clone()).await?;
        let restored = registry.get("chg-0123456789abcdef").unwrap();
        assert_eq!(restored.status, DevelopmentChangeStatus::RecoveryRequired);
        assert_eq!(
            restored.recovery_kind,
            Some(DevelopmentRecoveryKind::ManagedPromotion)
        );
        assert_eq!(restored.commit.unwrap().status, ChangeCommitStatus::Partial);
        release_development_host_lease(store, &lease).await?;
        Ok(())
    }

    #[tokio::test]
    async fn development_hydration_requires_recovery_for_interrupted_preview() -> anyhow::Result<()>
    {
        let store = Arc::new(InMemoryEventStore::default());
        let mut record = record(DevelopmentChangeStatus::Committed);
        record.deployment = Some(deployment(DevelopmentDeploymentStatus::Previewing));
        let snapshot = DevelopmentChangeSnapshot {
            record,
            request_fingerprint: "sha256:request".to_string(),
        };
        append_development_journal_event(store.as_ref(), &snapshot, 0)
            .await?
            .ok_or_else(|| anyhow::anyhow!("failed to append initial development snapshot"))?;

        let registry = development_registry();
        let lease = acquire_development_host_lease(store.clone(), registry.clone()).await?;
        hydrate_development_control_plane(store.clone(), registry.clone()).await?;
        let restored = registry.get("chg-0123456789abcdef").unwrap();
        assert_eq!(restored.status, DevelopmentChangeStatus::Committed);
        assert_eq!(
            restored.deployment.unwrap().status,
            DevelopmentDeploymentStatus::RecoveryRequired
        );
        assert_eq!(
            store
                .list_kind_prefix(DEVELOPMENT_JOURNAL_PREFIX)
                .await?
                .len(),
            2
        );
        release_development_host_lease(store, &lease).await?;
        Ok(())
    }

    #[tokio::test]
    async fn development_host_lease_has_one_live_owner() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let first_registry = development_registry();
        let first = acquire_development_host_lease(store.clone(), first_registry).await?;
        let second_registry = development_registry();
        assert!(
            acquire_development_host_lease(store.clone(), second_registry.clone())
                .await
                .is_err()
        );
        release_development_host_lease(store.clone(), &first).await?;
        let second = acquire_development_host_lease(store.clone(), second_registry).await?;
        release_development_host_lease(store, &second).await?;
        Ok(())
    }

    #[tokio::test]
    async fn strict_development_host_lease_release_rejects_a_new_owner() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let first = acquire_development_host_lease(store.clone(), development_registry()).await?;
        release_development_host_lease(store.clone(), &first).await?;
        let second = acquire_development_host_lease(store.clone(), development_registry()).await?;

        assert!(release_owned_development_host_lease(store.clone(), &first)
            .await
            .is_err());
        release_development_host_lease(store, &second).await?;
        Ok(())
    }

    #[tokio::test]
    async fn development_routes_remain_behind_the_host_token_gate() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let app = crate::app_with_state(AppState {
            runtime,
            static_dir: None,
            access_token: Some("development-token".to_string()),
            app_base_domain: None,
            build_jobs: crate::build_deploy_job_registry(),
            development: development_registry(),
            host_access: crate::host_access_registry(),
            target_agents: crate::target_agent_registry(),
        });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/host/v1/projects/project-1/changes")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"goal":"test","operations":[{"op":"file_delete","path":"src/a.rs"}]}"#,
                    ))?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
