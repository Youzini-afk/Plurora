use super::*;

use anyhow::Context;
use axum::body::Body;
use plurora_runtime::scan_effect_value_for_raw_secrets;
use plurora_runtime::InstallationControl;
use plurora_work::{InstallationId, WorkspaceId};
use serde_json::Value;
use tokio::io::AsyncReadExt;

use super::driver::{resolve_target_driver, TargetDriverKind};
use crate::require_identity_installation;

const OPERATION_JOURNAL_SESSION: &str = "host_control_target_operations";
const OPERATION_JOURNAL_EVENT: &str = "host/control/v1/target_operation.snapshot";
const OPERATION_HOST_FENCE_EVENT: &str = "host/control/v1/target_operation.host_fence";
const OPERATION_EXECUTOR_CLAIM_EVENT: &str = "host/control/v1/target_operation.executor_claim";
const DEFAULT_AUTHORITY_TTL_SECS: u64 = 5 * 60;
const MAX_AUTHORITY_TTL_SECS: u64 = 15 * 60;
const MAX_RECEIPT_OUTPUT_BYTES: usize = 256 * 1024;
const MAX_RECEIPT_DIAGNOSTICS: usize = 32;
const MAX_RECEIPT_DIAGNOSTIC_BYTES: usize = 2 * 1024;
const OPERATION_STEP_ID: &str = "execute";

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetOperationEffect {
    ArtifactMaterialize,
    ArtifactRelease,
    DeploymentApply,
    DeploymentObserve,
    DeploymentDrain,
    DeploymentStop,
    HealthProbe,
    VerifierRun,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetDeploymentRef {
    pub deployment_id: String,
    pub route_id: String,
    pub port_lease_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetDeploymentDescriptor {
    pub deployment: TargetDeploymentRef,
    pub port_name: String,
    pub image: String,
    pub container_port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_host_port: Option<u16>,
    #[serde(default)]
    pub pull_if_missing: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeclarativeVerifierDescriptor {
    ArtifactIntegrity {
        digest: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_size_bytes: Option<u64>,
    },
    DockerBuild {
        digest: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_size_bytes: Option<u64>,
        dockerfile: String,
        network_mode: plurora_runtime::ManagedTargetBuildNetworkMode,
        disposition: plurora_runtime::ManagedTargetImageDisposition,
        build_id: String,
        workspace_id: WorkspaceId,
        source_tree_digest: String,
        build_descriptor_hash: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TargetOperationSpec {
    ArtifactMaterialize {
        digest: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_size_bytes: Option<u64>,
    },
    ArtifactRelease {
        digest: String,
    },
    DeploymentApply {
        deployment: TargetDeploymentDescriptor,
    },
    DeploymentObserve {
        deployment: TargetDeploymentRef,
    },
    DeploymentDrain {
        deployment: TargetDeploymentRef,
        grace_seconds: u16,
    },
    DeploymentStop {
        deployment: TargetDeploymentRef,
        grace_seconds: u16,
        #[serde(default)]
        force_remove: bool,
    },
    HealthProbe,
    VerifierRun {
        verifier: DeclarativeVerifierDescriptor,
    },
}

impl TargetOperationSpec {
    pub fn effect(&self) -> TargetOperationEffect {
        match self {
            Self::ArtifactMaterialize { .. } => TargetOperationEffect::ArtifactMaterialize,
            Self::ArtifactRelease { .. } => TargetOperationEffect::ArtifactRelease,
            Self::DeploymentApply { .. } => TargetOperationEffect::DeploymentApply,
            Self::DeploymentObserve { .. } => TargetOperationEffect::DeploymentObserve,
            Self::DeploymentDrain { .. } => TargetOperationEffect::DeploymentDrain,
            Self::DeploymentStop { .. } => TargetOperationEffect::DeploymentStop,
            Self::HealthProbe => TargetOperationEffect::HealthProbe,
            Self::VerifierRun { .. } => TargetOperationEffect::VerifierRun,
        }
    }

    pub fn artifact_digests(&self) -> Vec<String> {
        match self {
            Self::ArtifactMaterialize { digest, .. } | Self::ArtifactRelease { digest } => {
                vec![digest.clone()]
            }
            Self::DeploymentApply { .. }
            | Self::DeploymentObserve { .. }
            | Self::DeploymentDrain { .. }
            | Self::DeploymentStop { .. }
            | Self::HealthProbe => Vec::new(),
            Self::VerifierRun { verifier } => match verifier {
                DeclarativeVerifierDescriptor::ArtifactIntegrity { digest, .. }
                | DeclarativeVerifierDescriptor::DockerBuild { digest, .. } => {
                    vec![digest.clone()]
                }
            },
        }
    }

    fn expected_size(&self, candidate_digest: &str) -> Option<u64> {
        match self {
            Self::ArtifactMaterialize {
                digest,
                expected_size_bytes,
            }
            | Self::VerifierRun {
                verifier:
                    DeclarativeVerifierDescriptor::ArtifactIntegrity {
                        digest,
                        expected_size_bytes,
                    },
            } if digest == candidate_digest => *expected_size_bytes,
            Self::VerifierRun {
                verifier:
                    DeclarativeVerifierDescriptor::DockerBuild {
                        digest,
                        expected_size_bytes,
                        ..
                    },
            } if digest == candidate_digest => *expected_size_bytes,
            _ => None,
        }
    }

    fn validate(&self) -> Result<(), ServiceError> {
        for digest in self.artifact_digests() {
            validate_sha256_digest(&digest)?;
        }
        match self {
            Self::DeploymentApply { deployment } => validate_deployment_descriptor(deployment)?,
            Self::DeploymentObserve { deployment } => validate_deployment_ref(deployment)?,
            Self::DeploymentDrain {
                deployment,
                grace_seconds,
            }
            | Self::DeploymentStop {
                deployment,
                grace_seconds,
                ..
            } => {
                validate_deployment_ref(deployment)?;
                if *grace_seconds > 300 {
                    return Err(ServiceError::with_status(
                        StatusCode::BAD_REQUEST,
                        "deployment grace_seconds must be <= 300",
                    ));
                }
            }
            Self::VerifierRun {
                verifier:
                    DeclarativeVerifierDescriptor::DockerBuild {
                        dockerfile,
                        build_id,
                        source_tree_digest,
                        build_descriptor_hash,
                        ..
                    },
            } => {
                crate::validate_relative_dockerfile(dockerfile).map_err(|_| {
                    ServiceError::with_status(
                        StatusCode::BAD_REQUEST,
                        "Docker build verifier contains an invalid dockerfile",
                    )
                })?;
                crate::validate_build_id(build_id).map_err(|_| {
                    ServiceError::with_status(
                        StatusCode::BAD_REQUEST,
                        "Docker build verifier contains an invalid build_id",
                    )
                })?;
                validate_sha256_digest(source_tree_digest)?;
                validate_sha256_digest(build_descriptor_hash)?;
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct TargetOperationAuthority {
    pub target_id: String,
    pub operation_id: String,
    pub step_id: String,
    pub installation_id: InstallationId,
    pub effect: TargetOperationEffect,
    pub artifact_digests: Vec<String>,
    pub lease_epoch: u64,
    pub policy_epoch: u64,
    pub issued_at_ms: i64,
    pub expires_at_ms: i64,
    pub nonce: String,
    pub request_digest: String,
    pub authority_digest: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetOperationStatusKind {
    Requested,
    Accepted,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    OutcomeUnknown,
    Expired,
}

impl TargetOperationStatusKind {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::OutcomeUnknown | Self::Expired
        )
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetOperationReceiptStatus {
    Succeeded,
    Failed,
    Cancelled,
    OutcomeUnknown,
}

impl From<TargetOperationReceiptStatus> for TargetOperationStatusKind {
    fn from(value: TargetOperationReceiptStatus) -> Self {
        match value {
            TargetOperationReceiptStatus::Succeeded => Self::Succeeded,
            TargetOperationReceiptStatus::Failed => Self::Failed,
            TargetOperationReceiptStatus::Cancelled => Self::Cancelled,
            TargetOperationReceiptStatus::OutcomeUnknown => Self::OutcomeUnknown,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TargetOperationReceipt {
    pub operation_id: String,
    pub target_id: String,
    pub execution_id: String,
    pub step_id: String,
    pub request_digest: String,
    pub authority_digest: String,
    pub status: TargetOperationReceiptStatus,
    pub completed_at_ms: i64,
    #[serde(default)]
    pub output: Value,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct TargetOperationRecord {
    pub operation_id: String,
    pub target_id: String,
    pub installation_id: InstallationId,
    pub revision: u64,
    pub status: TargetOperationStatusKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    pub spec: TargetOperationSpec,
    pub authority: TargetOperationAuthority,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<TargetOperationReceipt>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateTargetOperationRequest {
    pub installation_id: InstallationId,
    pub spec: TargetOperationSpec,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub expires_in_seconds: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct CreateTargetOperationResponse {
    pub operation: TargetOperationRecord,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NextTargetOperationResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<TargetOperationRecord>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TargetOperationProgressRequest {
    pub request_digest: String,
    pub authority_digest: String,
    pub execution_id: String,
    pub status: TargetOperationStatusKind,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TargetOperationSnapshot {
    record: TargetOperationRecord,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct TargetOperationHostFence {
    owner_id: String,
    generation: u64,
    fenced_at_ms: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct TargetOperationExecutorClaim {
    operation_id: String,
    target_id: String,
    owner_id: String,
    host_fence_generation: u64,
    generation: u64,
    execution_id: String,
    claimed_at_ms: i64,
}

#[derive(Debug, Default)]
pub(super) struct TargetOperationState {
    next_sequence: EventSequence,
    operations: HashMap<String, TargetOperationRecord>,
    idempotency: HashMap<(String, InstallationId, String), (String, String)>,
    host_fence: Option<TargetOperationHostFence>,
    executor_claims: HashMap<String, TargetOperationExecutorClaim>,
    terminal_projection_order: Vec<String>,
    #[cfg(test)]
    projection_invocations: usize,
    local_execution_locks: HashMap<String, Arc<tokio::sync::Mutex<()>>>,
}

impl TargetAgentRegistry {
    fn operation_next_sequence(&self) -> EventSequence {
        self.operations
            .lock()
            .expect("target operation state lock poisoned")
            .next_sequence
    }

    pub(crate) fn operation(&self, operation_id: &str) -> Option<TargetOperationRecord> {
        self.operations
            .lock()
            .expect("target operation state lock poisoned")
            .operations
            .get(operation_id)
            .cloned()
    }

    pub(crate) fn operations_for_target(&self, target_id: &str) -> Vec<TargetOperationRecord> {
        let mut operations = self
            .operations
            .lock()
            .expect("target operation state lock poisoned")
            .operations
            .values()
            .filter(|record| record.target_id == target_id)
            .cloned()
            .collect::<Vec<_>>();
        operations.sort_by(|left, right| {
            left.created_at_ms
                .cmp(&right.created_at_ms)
                .then_with(|| left.operation_id.cmp(&right.operation_id))
        });
        operations
    }

    fn terminal_operations_for_target(&self, target_id: &str) -> Vec<TargetOperationRecord> {
        let state = self
            .operations
            .lock()
            .expect("target operation state lock poisoned");
        state
            .terminal_projection_order
            .iter()
            .filter_map(|operation_id| state.operations.get(operation_id))
            .filter(|operation| operation.target_id == target_id)
            .cloned()
            .collect()
    }

    #[cfg(test)]
    fn projection_invocations(&self) -> usize {
        self.operations
            .lock()
            .expect("target operation state lock poisoned")
            .projection_invocations
    }

    pub(crate) fn installation_for_operation_route(
        &self,
        route_id: &str,
    ) -> Option<InstallationId> {
        let state = self
            .operations
            .lock()
            .expect("target operation state lock poisoned");
        let mut installations = state
            .operations
            .values()
            .filter(|operation| {
                operation_deployment_ref(&operation.spec)
                    .is_some_and(|deployment| deployment.route_id == route_id)
            })
            .map(|operation| operation.installation_id.clone())
            .collect::<Vec<_>>();
        installations.sort();
        installations.dedup();
        (installations.len() == 1).then(|| installations.remove(0))
    }

    fn operation_target_ids(&self) -> Vec<String> {
        let state = self
            .operations
            .lock()
            .expect("target operation state lock poisoned");
        let mut targets = state
            .operations
            .values()
            .map(|operation| operation.target_id.clone())
            .collect::<Vec<_>>();
        targets.sort();
        targets.dedup();
        targets
    }

    fn local_execution_lock(&self, operation_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        self.operations
            .lock()
            .expect("target operation state lock poisoned")
            .local_execution_locks
            .entry(operation_id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    fn operation_host_fence(&self) -> Option<TargetOperationHostFence> {
        self.operations
            .lock()
            .expect("target operation state lock poisoned")
            .host_fence
            .clone()
    }

    fn executor_claim(&self, operation_id: &str) -> Option<TargetOperationExecutorClaim> {
        self.operations
            .lock()
            .expect("target operation state lock poisoned")
            .executor_claims
            .get(operation_id)
            .cloned()
    }

    fn idempotent_operation(
        &self,
        target_id: &str,
        installation_id: &InstallationId,
        key: &str,
    ) -> Option<(String, TargetOperationRecord)> {
        let state = self
            .operations
            .lock()
            .expect("target operation state lock poisoned");
        let (request_digest, operation_id) = state.idempotency.get(&(
            target_id.to_string(),
            installation_id.clone(),
            key.to_string(),
        ))?;
        state
            .operations
            .get(operation_id)
            .cloned()
            .map(|record| (request_digest.clone(), record))
    }

    fn apply_operation_event(&self, envelope: &EventEnvelope) -> anyhow::Result<()> {
        anyhow::ensure!(
            envelope.session_id == OPERATION_JOURNAL_SESSION
                && matches!(
                    envelope.kind.as_str(),
                    OPERATION_JOURNAL_EVENT
                        | OPERATION_HOST_FENCE_EVENT
                        | OPERATION_EXECUTOR_CLAIM_EVENT
                ),
            "invalid target operation journal envelope"
        );
        let mut state = self
            .operations
            .lock()
            .expect("target operation state lock poisoned");
        if envelope.sequence < state.next_sequence {
            return Ok(());
        }
        anyhow::ensure!(
            envelope.sequence == state.next_sequence,
            "target operation journal sequence is not contiguous"
        );

        match envelope.kind.as_str() {
            OPERATION_HOST_FENCE_EVENT => {
                let fence: TargetOperationHostFence =
                    serde_json::from_value(envelope.payload.clone())?;
                anyhow::ensure!(
                    !fence.owner_id.is_empty() && fence.generation > 0,
                    "target operation Host fence is invalid"
                );
                if let Some(previous) = state.host_fence.as_ref() {
                    anyhow::ensure!(
                        fence.owner_id != previous.owner_id
                            && fence.generation == previous.generation.saturating_add(1)
                            && fence.fenced_at_ms >= previous.fenced_at_ms,
                        "target operation Host fence transition is invalid"
                    );
                } else {
                    anyhow::ensure!(
                        fence.generation == 1,
                        "initial target operation Host fence generation is invalid"
                    );
                }
                state.host_fence = Some(fence);
            }
            OPERATION_EXECUTOR_CLAIM_EVENT => {
                let claim: TargetOperationExecutorClaim =
                    serde_json::from_value(envelope.payload.clone())?;
                let fence = state.host_fence.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("target operation executor claim has no Host fence")
                })?;
                let operation = state.operations.get(&claim.operation_id).ok_or_else(|| {
                    anyhow::anyhow!("target operation executor claim has no operation")
                })?;
                anyhow::ensure!(
                    claim.target_id == "local"
                        && operation.target_id == claim.target_id
                        && !operation.status.is_terminal()
                        && claim.owner_id == fence.owner_id
                        && claim.host_fence_generation == fence.generation
                        && claim.generation > 0
                        && is_execution_id(&claim.execution_id),
                    "target operation executor claim is invalid"
                );
                if let Some(previous) = state.executor_claims.get(&claim.operation_id) {
                    anyhow::ensure!(
                        claim.owner_id != previous.owner_id
                            && claim.generation == previous.generation.saturating_add(1)
                            && claim.claimed_at_ms >= previous.claimed_at_ms,
                        "target operation executor claim transition is invalid"
                    );
                    if let Some(execution_id) = operation.execution_id.as_deref() {
                        anyhow::ensure!(
                            claim.execution_id == execution_id,
                            "target operation takeover changed the execution identity"
                        );
                    }
                } else {
                    anyhow::ensure!(
                        claim.generation == 1,
                        "initial target operation executor generation is invalid"
                    );
                }
                state
                    .executor_claims
                    .insert(claim.operation_id.clone(), claim);
            }
            OPERATION_JOURNAL_EVENT => {
                let snapshot: TargetOperationSnapshot =
                    serde_json::from_value(envelope.payload.clone())?;
                let record = snapshot.record;
                let authority_key = self
                    .operation_authority_key(&record.target_id, record.authority.lease_epoch)
                    .ok_or_else(|| anyhow::anyhow!("target operation authority key is unknown"))?;
                validate_record_integrity(&record, &authority_key)?;

                if let Some(previous) = state.operations.get(&record.operation_id) {
                    anyhow::ensure!(
                        record.revision == previous.revision.saturating_add(1)
                            && immutable_operation_fields_match(previous, &record)
                            && valid_execution_owner_transition(previous, &record)
                            && record.updated_at_ms >= previous.updated_at_ms
                            && valid_status_transition(previous.status, record.status),
                        "target operation snapshot transition is invalid"
                    );
                    if !previous.status.is_terminal() && record.status.is_terminal() {
                        state
                            .terminal_projection_order
                            .push(record.operation_id.clone());
                    }
                } else {
                    anyhow::ensure!(
                        record.revision == 1
                            && record.status == TargetOperationStatusKind::Requested
                            && record.execution_id.is_none()
                            && record.receipt.is_none(),
                        "new target operation snapshot is invalid"
                    );
                }

                if let Some(key) = record.idempotency_key.as_deref() {
                    let index_key = (
                        record.target_id.clone(),
                        record.installation_id.clone(),
                        key.to_string(),
                    );
                    if let Some((request_digest, operation_id)) = state.idempotency.get(&index_key)
                    {
                        anyhow::ensure!(
                            request_digest == &record.authority.request_digest
                                && operation_id == &record.operation_id,
                            "target operation idempotency key was reused"
                        );
                    } else {
                        state.idempotency.insert(
                            index_key,
                            (
                                record.authority.request_digest.clone(),
                                record.operation_id.clone(),
                            ),
                        );
                    }
                }
                state.operations.insert(record.operation_id.clone(), record);
            }
            _ => unreachable!("validated target operation journal event kind"),
        }
        state.next_sequence = envelope.sequence.saturating_add(1);
        Ok(())
    }
}

fn immutable_operation_fields_match(
    previous: &TargetOperationRecord,
    next: &TargetOperationRecord,
) -> bool {
    previous.operation_id == next.operation_id
        && previous.target_id == next.target_id
        && previous.installation_id == next.installation_id
        && previous.spec == next.spec
        && previous.authority == next.authority
        && previous.idempotency_key == next.idempotency_key
        && previous.created_at_ms == next.created_at_ms
}

fn valid_execution_owner_transition(
    previous: &TargetOperationRecord,
    next: &TargetOperationRecord,
) -> bool {
    if previous.status == TargetOperationStatusKind::Requested {
        previous.execution_id.is_none()
            && match next.status {
                TargetOperationStatusKind::Accepted => next.execution_id.is_some(),
                TargetOperationStatusKind::Expired => next.execution_id.is_none(),
                _ => false,
            }
    } else {
        previous.execution_id.is_some() && previous.execution_id == next.execution_id
    }
}

fn valid_status_transition(
    previous: TargetOperationStatusKind,
    next: TargetOperationStatusKind,
) -> bool {
    matches!(
        (previous, next),
        (
            TargetOperationStatusKind::Requested,
            TargetOperationStatusKind::Accepted | TargetOperationStatusKind::Expired
        ) | (
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

#[derive(Serialize)]
struct OperationRequestDigestInput<'a> {
    target_id: &'a str,
    installation_id: &'a InstallationId,
    step_id: &'a str,
    spec: &'a TargetOperationSpec,
}

#[derive(Serialize)]
struct UnsignedOperationAuthority<'a> {
    target_id: &'a str,
    operation_id: &'a str,
    step_id: &'a str,
    installation_id: &'a InstallationId,
    effect: TargetOperationEffect,
    artifact_digests: &'a [String],
    lease_epoch: u64,
    policy_epoch: u64,
    issued_at_ms: i64,
    expires_at_ms: i64,
    nonce: &'a str,
    request_digest: &'a str,
}

fn digest_serializable<T>(domain: &str, value: &T) -> anyhow::Result<String>
where
    T: Serialize + ?Sized,
{
    let mut hasher = Sha256::new();
    hasher.update(b"plurora-target-operation-v1\0");
    hasher.update(domain.as_bytes());
    hasher.update(b"\0");
    hasher.update(serde_json::to_vec(value)?);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn operation_request_digest(
    target_id: &str,
    installation_id: &InstallationId,
    spec: &TargetOperationSpec,
) -> anyhow::Result<String> {
    digest_serializable(
        "request",
        &OperationRequestDigestInput {
            target_id,
            installation_id,
            step_id: OPERATION_STEP_ID,
            spec,
        },
    )
}

fn operation_authority_digest(
    authority: &TargetOperationAuthority,
    authority_key: &str,
) -> anyhow::Result<String> {
    let mut message = b"plurora-target-operation-authority-v1\0".to_vec();
    message.extend(serde_json::to_vec(&UnsignedOperationAuthority {
        target_id: &authority.target_id,
        operation_id: &authority.operation_id,
        step_id: &authority.step_id,
        installation_id: &authority.installation_id,
        effect: authority.effect,
        artifact_digests: &authority.artifact_digests,
        lease_epoch: authority.lease_epoch,
        policy_epoch: authority.policy_epoch,
        issued_at_ms: authority.issued_at_ms,
        expires_at_ms: authority.expires_at_ms,
        nonce: &authority.nonce,
        request_digest: &authority.request_digest,
    })?);
    Ok(format!(
        "sha256:{:x}",
        hmac_sha256(authority_key.as_bytes(), &message)
    ))
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> sha2::digest::Output<Sha256> {
    const BLOCK_BYTES: usize = 64;
    let mut key_block = [0u8; BLOCK_BYTES];
    if key.len() > BLOCK_BYTES {
        key_block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }
    let mut inner_pad = [0x36u8; BLOCK_BYTES];
    let mut outer_pad = [0x5cu8; BLOCK_BYTES];
    for index in 0..BLOCK_BYTES {
        inner_pad[index] ^= key_block[index];
        outer_pad[index] ^= key_block[index];
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(message);
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner.finalize());
    outer.finalize()
}

fn validate_record_integrity(
    record: &TargetOperationRecord,
    authority_key: &str,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        !record.operation_id.is_empty()
            && record.target_id == record.authority.target_id
            && record.operation_id == record.authority.operation_id
            && record.installation_id == record.authority.installation_id
            && record.authority.step_id == OPERATION_STEP_ID
            && record.spec.effect() == record.authority.effect
            && record.spec.artifact_digests() == record.authority.artifact_digests
            && record.authority.lease_epoch > 0
            && record.authority.policy_epoch > 0
            && record.authority.issued_at_ms < record.authority.expires_at_ms
            && record.created_at_ms == record.authority.issued_at_ms
            && record.updated_at_ms >= record.created_at_ms,
        "target operation authority fields are inconsistent"
    );
    anyhow::ensure!(
        match record.status {
            TargetOperationStatusKind::Requested | TargetOperationStatusKind::Expired => {
                record.execution_id.is_none()
            }
            _ => record.execution_id.as_deref().is_some_and(is_execution_id),
        },
        "target operation execution owner is invalid"
    );
    for digest in &record.authority.artifact_digests {
        anyhow::ensure!(is_sha256_digest(digest), "invalid artifact digest");
    }
    anyhow::ensure!(
        operation_request_digest(&record.target_id, &record.installation_id, &record.spec)?
            == record.authority.request_digest,
        "target operation request digest did not match"
    );
    anyhow::ensure!(
        operation_authority_digest(&record.authority, authority_key)?
            == record.authority.authority_digest,
        "target operation authority MAC did not match"
    );

    match (record.status, record.receipt.as_ref()) {
        (TargetOperationStatusKind::Expired, None) => {}
        (status, Some(receipt)) if status.is_terminal() => {
            validate_receipt_for_record(record, receipt)?;
        }
        (status, None) if !status.is_terminal() => {}
        _ => anyhow::bail!("target operation receipt does not match terminal state"),
    }
    Ok(())
}

fn validate_receipt_for_record(
    record: &TargetOperationRecord,
    receipt: &TargetOperationReceipt,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        receipt.operation_id == record.operation_id
            && receipt.target_id == record.target_id
            && Some(receipt.execution_id.as_str()) == record.execution_id.as_deref()
            && receipt.step_id == OPERATION_STEP_ID
            && receipt.request_digest == record.authority.request_digest
            && receipt.authority_digest == record.authority.authority_digest
            && TargetOperationStatusKind::from(receipt.status) == record.status
            && receipt.completed_at_ms >= record.authority.issued_at_ms,
        "target operation receipt binding is invalid"
    );
    anyhow::ensure!(
        serde_json::to_vec(&receipt.output)?.len() <= MAX_RECEIPT_OUTPUT_BYTES,
        "target operation receipt output is too large"
    );
    anyhow::ensure!(
        !scan_effect_value_for_raw_secrets(&receipt.output, "receipt.output").has_findings(),
        "target operation receipt contains raw secret material"
    );
    anyhow::ensure!(
        receipt.diagnostics.len() <= MAX_RECEIPT_DIAGNOSTICS
            && receipt.diagnostics.iter().all(|diagnostic| {
                diagnostic.len() <= MAX_RECEIPT_DIAGNOSTIC_BYTES
                    && !scan_effect_value_for_raw_secrets(
                        &json!({ "diagnostic": diagnostic }),
                        "receipt.diagnostics",
                    )
                    .has_findings()
                    && !diagnostic.chars().any(|character| {
                        character.is_control()
                            && character != '\n'
                            && character != '\r'
                            && character != '\t'
                    })
            }),
        "target operation receipt diagnostics exceed their limits"
    );
    if receipt.status == TargetOperationReceiptStatus::Succeeded {
        validate_succeeded_operation_output(record, &receipt.output)?;
    }
    Ok(())
}

fn validate_succeeded_operation_output(
    record: &TargetOperationRecord,
    output: &Value,
) -> anyhow::Result<()> {
    let TargetOperationSpec::VerifierRun {
        verifier:
            DeclarativeVerifierDescriptor::DockerBuild {
                digest,
                dockerfile,
                network_mode,
                disposition,
                build_id,
                workspace_id,
                source_tree_digest,
                build_descriptor_hash,
                ..
            },
    } = &record.spec
    else {
        return Ok(());
    };
    let build: plurora_runtime::ManagedTargetImageBuildReceipt =
        serde_json::from_value(output.clone())
            .context("target Docker build receipt output is malformed")?;
    plurora_runtime::validate_managed_target_image_build_receipt(&build)?;
    anyhow::ensure!(
        build.target_id == record.target_id
            && build.installation_id == record.installation_id
            && build.workspace_id == *workspace_id
            && build.build_id == *build_id
            && build.dockerfile == *dockerfile
            && build.network_mode == *network_mode
            && build.disposition == *disposition
            && build.context_digest == *digest
            && build.source_tree_digest == *source_tree_digest
            && build.build_descriptor_hash == *build_descriptor_hash,
        "target Docker build receipt output does not match its immutable operation spec"
    );
    Ok(())
}

fn is_execution_id(execution_id: &str) -> bool {
    execution_id.len() == 32
        && execution_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Validates the complete Host authority before a native agent accepts work.
/// `require_unexpired` must be true before first acceptance and may be false
/// only when a durable local ledger proves the same step was already accepted.
pub fn verify_target_operation_authority(
    record: &TargetOperationRecord,
    credential: &str,
    expected_target_id: &str,
    expected_lease_epoch: u64,
    expected_policy_epoch: u64,
    now_ms: i64,
    require_unexpired: bool,
) -> Result<(), String> {
    let authority_key = credential_digest("agent", credential);
    validate_record_integrity(record, &authority_key)
        .map_err(|_| "operation authority is malformed".to_string())?;
    if record.target_id != expected_target_id
        || record.authority.lease_epoch != expected_lease_epoch
        || record.authority.policy_epoch != expected_policy_epoch
        || now_ms < record.authority.issued_at_ms
        || (require_unexpired && now_ms >= record.authority.expires_at_ms)
    {
        return Err("operation authority audience, epoch, or expiry did not match".to_string());
    }
    Ok(())
}

fn is_sha256_digest(digest: &str) -> bool {
    digest.len() == 71
        && digest.starts_with("sha256:")
        && digest[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_sha256_digest(digest: &str) -> Result<(), ServiceError> {
    if is_sha256_digest(digest) {
        Ok(())
    } else {
        Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "artifact digest must be sha256:<64 lowercase hexadecimal characters>",
        ))
    }
}

fn validate_deployment_ref(deployment: &TargetDeploymentRef) -> Result<(), ServiceError> {
    if [
        deployment.deployment_id.as_str(),
        deployment.route_id.as_str(),
        deployment.port_lease_id.as_str(),
    ]
    .into_iter()
    .any(|value| {
        value.is_empty()
            || value.len() > 256
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-._:/".contains(&byte))
    }) {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "deployment identity fields must be label-safe ASCII",
        ));
    }
    Ok(())
}

fn validate_deployment_descriptor(
    deployment: &TargetDeploymentDescriptor,
) -> Result<(), ServiceError> {
    validate_deployment_ref(&deployment.deployment)?;
    if deployment.port_name.is_empty()
        || deployment.port_name.len() > 64
        || !deployment
            .port_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._".contains(&byte))
        || !valid_deployment_image_reference(&deployment.image)
        || scan_effect_value_for_raw_secrets(
            &json!({ "image": deployment.image.as_str() }),
            "operation.spec",
        )
        .has_findings()
        || deployment.container_port == 0
        || deployment.requested_host_port == Some(0)
        || deployment.health_path.as_deref().is_some_and(|path| {
            !path.starts_with('/')
                || path.starts_with("//")
                || path.len() > 256
                || path.contains('\r')
                || path.contains('\n')
        })
    {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "deployment descriptor contains an invalid image, port, or port name",
        ));
    }
    Ok(())
}

fn valid_deployment_image_reference(image: &str) -> bool {
    if image.is_empty()
        || image.len() > 512
        || image.contains("://")
        || !image
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/@+-".contains(&byte))
    {
        return false;
    }
    let Some((name, digest)) = image.rsplit_once('@') else {
        return true;
    };
    !name.is_empty() && !name.contains('@') && is_sha256_digest(digest)
}

fn operation_deployment_ref(spec: &TargetOperationSpec) -> Option<&TargetDeploymentRef> {
    match spec {
        TargetOperationSpec::DeploymentApply { deployment } => Some(&deployment.deployment),
        TargetOperationSpec::DeploymentObserve { deployment }
        | TargetOperationSpec::DeploymentDrain { deployment, .. }
        | TargetOperationSpec::DeploymentStop { deployment, .. } => Some(deployment),
        _ => None,
    }
}

fn validate_create_request(
    target_id: &str,
    request: &CreateTargetOperationRequest,
) -> Result<(), ServiceError> {
    if !valid_target_id(target_id) {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "target operation requires a valid target_id",
        ));
    }
    request.spec.validate()?;
    if let Some(key) = request.idempotency_key.as_deref() {
        if key.is_empty()
            || key.len() > 128
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-._:".contains(&byte))
        {
            return Err(ServiceError::with_status(
                StatusCode::BAD_REQUEST,
                "idempotency_key must use 1..=128 label-safe ASCII characters",
            ));
        }
    }
    let ttl = request
        .expires_in_seconds
        .unwrap_or(DEFAULT_AUTHORITY_TTL_SECS);
    if ttl == 0 || ttl > MAX_AUTHORITY_TTL_SECS {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "operation authority expiry is outside the supported range",
        ));
    }
    Ok(())
}

fn required_capabilities(spec: &TargetOperationSpec) -> &'static [ExecutionTargetCapability] {
    match spec {
        TargetOperationSpec::ArtifactMaterialize { .. }
        | TargetOperationSpec::ArtifactRelease { .. } => {
            &[ExecutionTargetCapability::ArtifactTransfer]
        }
        TargetOperationSpec::DeploymentApply { .. }
        | TargetOperationSpec::DeploymentObserve { .. }
        | TargetOperationSpec::DeploymentDrain { .. }
        | TargetOperationSpec::DeploymentStop { .. } => &[ExecutionTargetCapability::Deployment],
        TargetOperationSpec::HealthProbe => &[ExecutionTargetCapability::HealthProbe],
        TargetOperationSpec::VerifierRun { .. } => &[
            ExecutionTargetCapability::ArtifactTransfer,
            ExecutionTargetCapability::DeclarativeVerifier,
        ],
    }
}

async fn validate_deployment_topology<S>(
    state: &AppState<S>,
    target_id: &str,
    request: &CreateTargetOperationRequest,
) -> Result<(), ServiceError>
where
    S: EventStore,
{
    let Some(deployment) = operation_deployment_ref(&request.spec) else {
        return Ok(());
    };
    for owner in [
        state
            .build_jobs
            .installation_for_route(&deployment.route_id),
        state
            .target_agents
            .installation_for_operation_route(&deployment.route_id),
    ]
    .into_iter()
    .flatten()
    {
        if owner != request.installation_id {
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "deployment route is owned by another installation",
            ));
        }
    }

    let lease = state
        .runtime
        .config()
        .port_lease_registry
        .status(&deployment.port_lease_id)
        .await
        .ok_or_else(|| {
            ServiceError::with_status(
                StatusCode::CONFLICT,
                "deployment operation requires an existing target port lease",
            )
        })?;
    let route = state
        .runtime
        .config()
        .proxy_route_registry
        .status(&deployment.route_id)
        .await
        .ok_or_else(|| {
            ServiceError::with_status(
                StatusCode::CONFLICT,
                "deployment operation requires an existing proxy route",
            )
        })?;
    if lease.target_id != target_id
        || lease.host != "127.0.0.1"
        || lease.bind != plurora_runtime::PortBindScope::LoopbackOnly
        || lease.protocol != plurora_runtime::PortProtocol::Tcp
        || route.upstream.port_lease_id != deployment.port_lease_id
        || route.upstream.port_name != lease.port_name
    {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "deployment route and port lease ownership do not match the target",
        ));
    }
    if let TargetOperationSpec::DeploymentApply { deployment } = &request.spec {
        if lease.status == plurora_runtime::PortLeaseStatusKind::Released
            || route.status == plurora_runtime::ProxyRouteStatusKind::Removed
            || deployment.port_name != lease.port_name
        {
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "deployment apply requires a live matching route and port lease",
            ));
        }
    }
    Ok(())
}

async fn current_operation_host_fence<S>(
    state: &AppState<S>,
) -> anyhow::Result<(
    crate::development::DevelopmentHostLease,
    TargetOperationHostFence,
)>
where
    S: EventStore,
{
    let lease = state.development.active_host_lease()?;
    establish_operation_host_fence(
        state.runtime.store().as_ref(),
        state.target_agents.as_ref(),
        &lease,
    )
    .await
    .map(|fence| (lease, fence))
}

async fn establish_operation_host_fence<S>(
    store: &S,
    registry: &TargetAgentRegistry,
    lease: &crate::development::DevelopmentHostLease,
) -> anyhow::Result<TargetOperationHostFence>
where
    S: EventStore,
{
    for _ in 0..8 {
        lease.ensure_durable_owner().await?;
        sync_target_operation_journal(store, registry).await?;
        if let Some(current) = registry.operation_host_fence() {
            if current.owner_id == lease.owner_id() {
                lease.ensure_durable_owner().await?;
                return Ok(current);
            }
        }
        let previous = registry.operation_host_fence();
        let fence = TargetOperationHostFence {
            owner_id: lease.owner_id().to_string(),
            generation: previous
                .as_ref()
                .map_or(1, |fence| fence.generation.saturating_add(1)),
            fenced_at_ms: Utc::now().timestamp_millis(),
        };
        lease.ensure_durable_owner().await?;
        let event = store
            .append_with_sequence_if_next(
                OPERATION_JOURNAL_SESSION.to_string(),
                registry.operation_next_sequence(),
                JOURNAL_WRITER.to_string(),
                OPERATION_HOST_FENCE_EVENT.to_string(),
                1,
                serde_json::to_value(&fence)?,
                json!({ "owner": "host_control_plane", "durable_fence": true }),
            )
            .await?;
        if let Some(event) = event {
            registry.apply_operation_event(&event)?;
            return Ok(fence);
        }
    }
    anyhow::bail!("target operation Host fence changed too frequently")
}

async fn ensure_operation_host_fence<S>(
    store: &S,
    registry: &TargetAgentRegistry,
    lease: &crate::development::DevelopmentHostLease,
    fence: &TargetOperationHostFence,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    lease.ensure_durable_owner().await?;
    sync_target_operation_journal(store, registry).await?;
    anyhow::ensure!(
        registry.operation_host_fence().as_ref() == Some(fence)
            && fence.owner_id == lease.owner_id(),
        "target operation Host fence is no longer current"
    );
    Ok(())
}

async fn claim_local_operation<S>(
    store: &S,
    registry: &TargetAgentRegistry,
    lease: &crate::development::DevelopmentHostLease,
    fence: &TargetOperationHostFence,
    operation_id: &str,
) -> anyhow::Result<Option<TargetOperationExecutorClaim>>
where
    S: EventStore,
{
    for _ in 0..8 {
        ensure_operation_host_fence(store, registry, lease, fence).await?;
        let operation = registry
            .operation(operation_id)
            .ok_or_else(|| anyhow::anyhow!("local target operation disappeared"))?;
        if operation.status.is_terminal() {
            return Ok(None);
        }
        if let Some(current) = registry.executor_claim(operation_id) {
            if current.owner_id == fence.owner_id
                && current.host_fence_generation == fence.generation
            {
                return Ok(Some(current));
            }
        }
        let previous = registry.executor_claim(operation_id);
        let execution_id = operation
            .execution_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
        let claim = TargetOperationExecutorClaim {
            operation_id: operation.operation_id,
            target_id: operation.target_id,
            owner_id: fence.owner_id.clone(),
            host_fence_generation: fence.generation,
            generation: previous
                .as_ref()
                .map_or(1, |claim| claim.generation.saturating_add(1)),
            execution_id,
            claimed_at_ms: Utc::now().timestamp_millis(),
        };
        lease.ensure_durable_owner().await?;
        let event = store
            .append_with_sequence_if_next(
                OPERATION_JOURNAL_SESSION.to_string(),
                registry.operation_next_sequence(),
                JOURNAL_WRITER.to_string(),
                OPERATION_EXECUTOR_CLAIM_EVENT.to_string(),
                1,
                serde_json::to_value(&claim)?,
                json!({ "owner": "host_control_plane", "executor_fence": true }),
            )
            .await?;
        if let Some(event) = event {
            registry.apply_operation_event(&event)?;
            return Ok(Some(claim));
        }
    }
    anyhow::bail!("target operation executor claim changed too frequently")
}

async fn ensure_executor_claim<S>(
    store: &S,
    registry: &TargetAgentRegistry,
    lease: &crate::development::DevelopmentHostLease,
    fence: &TargetOperationHostFence,
    claim: &TargetOperationExecutorClaim,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    ensure_operation_host_fence(store, registry, lease, fence).await?;
    anyhow::ensure!(
        registry.executor_claim(&claim.operation_id).as_ref() == Some(claim),
        "target operation executor generation is no longer current"
    );
    Ok(())
}

pub(crate) async fn synchronize_target_operations_as_owner<S>(
    state: &AppState<S>,
) -> anyhow::Result<usize>
where
    S: EventStore,
{
    let (_, fence) = current_operation_host_fence(state).await?;
    let loaded =
        sync_target_operation_journal(state.runtime.store().as_ref(), state.target_agents.as_ref())
            .await?;
    anyhow::ensure!(
        state.target_agents.operation_host_fence().as_ref() == Some(&fence),
        "target operation Host fence changed during synchronization"
    );
    Ok(loaded)
}

pub(super) fn protected_routes<S>() -> Router<AppState<S>>
where
    S: EventStore,
{
    Router::new()
        .route(
            "/host/v1/targets/:target_id/operations",
            post(create_operation::<S>).get(list_operations::<S>),
        )
        .route(
            "/host/v1/targets/:target_id/operations/:operation_id",
            get(get_operation::<S>),
        )
}

pub(super) fn agent_routes<S>() -> Router<AppState<S>>
where
    S: EventStore,
{
    Router::new()
        .route("/target-agent/v1/operations/next", get(next_operation::<S>))
        .route(
            "/target-agent/v1/operations/:operation_id/progress",
            post(progress_operation::<S>),
        )
        .route(
            "/target-agent/v1/operations/:operation_id/receipt",
            post(complete_operation::<S>),
        )
        .route(
            "/target-agent/v1/operations/:operation_id/artifacts/:digest",
            get(stream_operation_artifact::<S>),
        )
}

async fn create_operation<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path(target_id): Path<String>,
    Json(request): Json<CreateTargetOperationRequest>,
) -> Result<(StatusCode, Json<CreateTargetOperationResponse>), ServiceError>
where
    S: EventStore,
{
    require_identity_target(&identity, &target_id)?;
    require_identity_installation(&identity, request.installation_id.as_str())?;
    let operation = submit_host_operation(&state, &target_id, request).await?;
    Ok((
        StatusCode::CREATED,
        Json(CreateTargetOperationResponse { operation }),
    ))
}

pub(crate) async fn submit_host_operation<S>(
    state: &AppState<S>,
    target_id: &str,
    request: CreateTargetOperationRequest,
) -> Result<TargetOperationRecord, ServiceError>
where
    S: EventStore,
{
    validate_create_request(&target_id, &request)?;
    let (host_lease, host_fence) = current_operation_host_fence(state)
        .await
        .map_err(target_internal_error)?;
    if state
        .installations
        .get(&request.installation_id)
        .await
        .map_err(target_internal_error)?
        .is_none()
    {
        return Err(ServiceError::with_status(
            StatusCode::NOT_FOUND,
            "installation is not registered",
        ));
    }
    validate_deployment_topology(state, target_id, &request).await?;
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
        .status(target_id)
        .await
        .ok_or_else(|| ServiceError::with_status(StatusCode::NOT_FOUND, "target not found"))?;
    let driver = resolve_target_driver(&target);
    let authority_key = state
        .target_agents
        .operation_authority_key(&target.id, target.lease_epoch)
        .ok_or_else(|| {
            ServiceError::with_status(
                StatusCode::CONFLICT,
                "target identity has no operation authority key",
            )
        })?;
    if target.status != ExecutionTargetStatusKind::Available
        || required_capabilities(&request.spec)
            .iter()
            .any(|required| !target.capabilities.contains(required))
    {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "target is offline or lacks a required effective capability",
        ));
    }
    if driver == TargetDriverKind::Local
        && matches!(
            &request.spec,
            TargetOperationSpec::DeploymentApply { .. }
                | TargetOperationSpec::DeploymentObserve { .. }
                | TargetOperationSpec::DeploymentDrain { .. }
                | TargetOperationSpec::DeploymentStop { .. }
        )
        && plurora_runtime::validate_managed_target_deployment_runtime()
            .await
            .is_err()
    {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "local target deployment runtime is unavailable",
        ));
    }

    let operation = create_operation_record(
        state.runtime.store().as_ref(),
        state.target_agents.as_ref(),
        &host_lease,
        &host_fence,
        &authority_key,
        target,
        request,
    )
    .await
    .map_err(target_conflict_error)?;
    let operation = match driver {
        TargetDriverKind::Local if !operation.status.is_terminal() => {
            drive_local_operation(state, &operation.operation_id)
                .await
                .map_err(target_internal_error)?
        }
        TargetDriverKind::Local => operation,
        TargetDriverKind::Agent => operation,
    };
    Ok(operation)
}

pub(crate) async fn wait_for_host_operation<S>(
    state: &AppState<S>,
    target_id: &str,
    operation_id: &str,
    max_wait: std::time::Duration,
) -> anyhow::Result<TargetOperationRecord>
where
    S: EventStore,
{
    let deadline = tokio::time::Instant::now() + max_wait;
    loop {
        sync_target_operation_journal(state.runtime.store().as_ref(), state.target_agents.as_ref())
            .await?;
        let operation = state
            .target_agents
            .operation(operation_id)
            .filter(|operation| operation.target_id == target_id)
            .ok_or_else(|| anyhow::anyhow!("target operation disappeared"))?;
        if operation.status.is_terminal() {
            return Ok(operation);
        }
        anyhow::ensure!(
            tokio::time::Instant::now() < deadline,
            "target operation did not reach a terminal state in time"
        );
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
}

async fn list_operations<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path(target_id): Path<String>,
) -> Result<Json<Vec<TargetOperationRecord>>, ServiceError>
where
    S: EventStore,
{
    require_identity_target(&identity, &target_id)?;
    sync_target_operation_journal(state.runtime.store().as_ref(), state.target_agents.as_ref())
        .await
        .map_err(target_internal_error)?;
    let operations = state
        .target_agents
        .operations_for_target(&target_id)
        .into_iter()
        .filter(|operation| identity.allows_installation(operation.installation_id.as_str()))
        .collect();
    Ok(Json(operations))
}

async fn get_operation<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path((target_id, operation_id)): Path<(String, String)>,
) -> Result<Json<TargetOperationRecord>, ServiceError>
where
    S: EventStore,
{
    require_identity_target(&identity, &target_id)?;
    sync_target_operation_journal(state.runtime.store().as_ref(), state.target_agents.as_ref())
        .await
        .map_err(target_internal_error)?;
    let operation = state
        .target_agents
        .operation(&operation_id)
        .filter(|operation| operation.target_id == target_id)
        .ok_or_else(|| ServiceError::with_status(StatusCode::NOT_FOUND, "operation not found"))?;
    require_identity_installation(&identity, operation.installation_id.as_str())?;
    Ok(Json(operation))
}

async fn authenticated_agent<S>(
    state: &AppState<S>,
    headers: &HeaderMap,
) -> Result<StoredAgent, ServiceError>
where
    S: EventStore,
{
    let credential = target_credential(headers).ok_or_else(target_unauthorized)?;
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
        || live_target.identity_ref != agent.target.identity_ref
        || live_target.lease_epoch != agent.target.lease_epoch
        || live_target.policy_epoch != agent.target.policy_epoch
    {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "target must heartbeat before requesting operation work",
        ));
    }
    Ok(agent)
}

async fn next_operation<S>(
    State(state): State<AppState<S>>,
    headers: HeaderMap,
) -> Result<Json<NextTargetOperationResponse>, ServiceError>
where
    S: EventStore,
{
    let agent = authenticated_agent(&state, &headers).await?;
    let (host_lease, host_fence) = current_operation_host_fence(&state)
        .await
        .map_err(target_internal_error)?;
    'refresh: for _ in 0..8 {
        sync_target_operation_journal(state.runtime.store().as_ref(), state.target_agents.as_ref())
            .await
            .map_err(target_internal_error)?;
        let now_ms = Utc::now().timestamp_millis();
        for operation in state.target_agents.operations_for_target(&agent.target.id) {
            if operation.status.is_terminal() {
                continue;
            }
            let epoch_matches = operation.authority.lease_epoch == agent.target.lease_epoch
                && operation.authority.policy_epoch == agent.target.policy_epoch;
            if operation.status == TargetOperationStatusKind::Requested
                && (!epoch_matches || now_ms >= operation.authority.expires_at_ms)
            {
                let mut expired = operation;
                expired.revision = expired.revision.saturating_add(1);
                expired.status = TargetOperationStatusKind::Expired;
                expired.updated_at_ms = now_ms;
                if append_target_operation_snapshot(
                    state.runtime.store().as_ref(),
                    state.target_agents.as_ref(),
                    &host_lease,
                    &host_fence,
                    None,
                    state.target_agents.operation_next_sequence(),
                    &expired,
                )
                .await
                .map_err(target_internal_error)?
                .is_some()
                {
                    continue 'refresh;
                }
                continue 'refresh;
            }
            if epoch_matches {
                return Ok(Json(NextTargetOperationResponse {
                    operation: Some(operation),
                }));
            }
        }
        return Ok(Json(NextTargetOperationResponse { operation: None }));
    }
    Err(ServiceError::with_status(
        StatusCode::CONFLICT,
        "target operation queue changed too frequently",
    ))
}

async fn progress_operation<S>(
    State(state): State<AppState<S>>,
    headers: HeaderMap,
    Path(operation_id): Path<String>,
    Json(request): Json<TargetOperationProgressRequest>,
) -> Result<Json<TargetOperationRecord>, ServiceError>
where
    S: EventStore,
{
    if !matches!(
        request.status,
        TargetOperationStatusKind::Accepted | TargetOperationStatusKind::Running
    ) {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "agent progress may only report accepted or running",
        ));
    }
    let agent = authenticated_agent(&state, &headers).await?;
    let (host_lease, host_fence) = current_operation_host_fence(&state)
        .await
        .map_err(target_internal_error)?;
    for _ in 0..8 {
        sync_target_operation_journal(state.runtime.store().as_ref(), state.target_agents.as_ref())
            .await
            .map_err(target_internal_error)?;
        let operation = state
            .target_agents
            .operation(&operation_id)
            .filter(|operation| operation.target_id == agent.target.id)
            .ok_or_else(|| {
                ServiceError::with_status(StatusCode::NOT_FOUND, "operation not found")
            })?;
        validate_agent_operation_binding(
            &operation,
            &agent,
            &request.request_digest,
            &request.authority_digest,
            Some(&request.execution_id),
        )?;
        if operation.status == request.status {
            return Ok(Json(operation));
        }
        let now_ms = Utc::now().timestamp_millis();
        if operation.status == TargetOperationStatusKind::Requested
            && (request.status != TargetOperationStatusKind::Accepted
                || now_ms >= operation.authority.expires_at_ms)
        {
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "operation must be accepted before expiry and before it can run",
            ));
        }
        if operation.status == TargetOperationStatusKind::Accepted
            && request.status != TargetOperationStatusKind::Running
        {
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "operation progress transition is invalid",
            ));
        }
        if operation.status.is_terminal()
            || !matches!(
                operation.status,
                TargetOperationStatusKind::Requested | TargetOperationStatusKind::Accepted
            )
        {
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "operation no longer accepts progress",
            ));
        }
        let mut next = operation;
        next.revision = next.revision.saturating_add(1);
        next.status = request.status;
        if next.execution_id.is_none() {
            next.execution_id = Some(request.execution_id.clone());
        }
        next.updated_at_ms = now_ms;
        if append_target_operation_snapshot(
            state.runtime.store().as_ref(),
            state.target_agents.as_ref(),
            &host_lease,
            &host_fence,
            None,
            state.target_agents.operation_next_sequence(),
            &next,
        )
        .await
        .map_err(target_internal_error)?
        .is_some()
        {
            return Ok(Json(next));
        }
    }
    Err(ServiceError::with_status(
        StatusCode::CONFLICT,
        "target operation journal changed too frequently",
    ))
}

async fn complete_operation<S>(
    State(state): State<AppState<S>>,
    headers: HeaderMap,
    Path(operation_id): Path<String>,
    Json(receipt): Json<TargetOperationReceipt>,
) -> Result<Json<TargetOperationRecord>, ServiceError>
where
    S: EventStore,
{
    if receipt.operation_id != operation_id {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "receipt operation_id does not match the request path",
        ));
    }
    let now_ms = Utc::now().timestamp_millis();
    if receipt.completed_at_ms > now_ms.saturating_add(60_000) {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "receipt completion time is too far in the future",
        ));
    }
    let agent = authenticated_agent(&state, &headers).await?;
    sync_target_operation_journal(state.runtime.store().as_ref(), state.target_agents.as_ref())
        .await
        .map_err(target_internal_error)?;
    let operation = state
        .target_agents
        .operation(&operation_id)
        .filter(|operation| operation.target_id == agent.target.id)
        .ok_or_else(|| ServiceError::with_status(StatusCode::NOT_FOUND, "operation not found"))?;
    validate_agent_operation_binding(
        &operation,
        &agent,
        &receipt.request_digest,
        &receipt.authority_digest,
        Some(&receipt.execution_id),
    )?;
    if operation.status.is_terminal() {
        if operation.receipt.as_ref() == Some(&receipt) {
            return Ok(Json(operation));
        }
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "operation already has a different terminal receipt",
        ));
    }

    let (host_lease, host_fence) = current_operation_host_fence(&state)
        .await
        .map_err(target_internal_error)?;
    for _ in 0..8 {
        sync_target_operation_journal(state.runtime.store().as_ref(), state.target_agents.as_ref())
            .await
            .map_err(target_internal_error)?;
        let operation = state
            .target_agents
            .operation(&operation_id)
            .filter(|operation| operation.target_id == agent.target.id)
            .ok_or_else(|| {
                ServiceError::with_status(StatusCode::NOT_FOUND, "operation not found")
            })?;
        validate_agent_operation_binding(
            &operation,
            &agent,
            &receipt.request_digest,
            &receipt.authority_digest,
            Some(&receipt.execution_id),
        )?;
        if operation.status.is_terminal() {
            if operation.receipt.as_ref() == Some(&receipt) {
                return Ok(Json(operation));
            }
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "operation already has a different terminal receipt",
            ));
        }
        if !matches!(
            operation.status,
            TargetOperationStatusKind::Accepted | TargetOperationStatusKind::Running
        ) {
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "operation must be durably accepted before terminal receipt",
            ));
        }
        let mut next = operation;
        next.revision = next.revision.saturating_add(1);
        next.status = receipt.status.into();
        next.updated_at_ms = now_ms.max(receipt.completed_at_ms);
        next.receipt = Some(receipt.clone());
        let authority_key = state
            .target_agents
            .operation_authority_key(&next.target_id, next.authority.lease_epoch)
            .ok_or_else(|| {
                ServiceError::with_status(
                    StatusCode::CONFLICT,
                    "target operation authority key is no longer available",
                )
            })?;
        validate_record_integrity(&next, &authority_key).map_err(|_| {
            ServiceError::with_status(
                StatusCode::BAD_REQUEST,
                "terminal receipt is malformed, oversized, or contains raw secret material",
            )
        })?;
        if append_target_operation_snapshot(
            state.runtime.store().as_ref(),
            state.target_agents.as_ref(),
            &host_lease,
            &host_fence,
            None,
            state.target_agents.operation_next_sequence(),
            &next,
        )
        .await
        .map_err(target_internal_error)?
        .is_some()
        {
            installation_deployment_operation(&state, &next)
                .await
                .map_err(target_internal_error)?;
            return Ok(Json(next));
        }
    }
    Err(ServiceError::with_status(
        StatusCode::CONFLICT,
        "target operation journal changed too frequently",
    ))
}

fn validate_agent_operation_binding(
    operation: &TargetOperationRecord,
    agent: &StoredAgent,
    request_digest: &str,
    authority_digest: &str,
    execution_id: Option<&str>,
) -> Result<(), ServiceError> {
    if operation.authority.lease_epoch != agent.target.lease_epoch
        || operation.authority.policy_epoch != agent.target.policy_epoch
        || operation.authority.request_digest != request_digest
        || operation.authority.authority_digest != authority_digest
        || execution_id.is_some_and(|candidate| {
            !is_execution_id(candidate)
                || operation
                    .execution_id
                    .as_deref()
                    .is_some_and(|expected| expected != candidate)
        })
    {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "operation authority, request digest, or target epoch did not match",
        ));
    }
    Ok(())
}

async fn stream_operation_artifact<S>(
    State(state): State<AppState<S>>,
    headers: HeaderMap,
    Path((operation_id, digest)): Path<(String, String)>,
) -> Result<Response, ServiceError>
where
    S: EventStore,
{
    validate_sha256_digest(&digest)?;
    let agent = authenticated_agent(&state, &headers).await?;
    current_operation_host_fence(&state)
        .await
        .map_err(target_internal_error)?;
    sync_target_operation_journal(state.runtime.store().as_ref(), state.target_agents.as_ref())
        .await
        .map_err(target_internal_error)?;
    let operation = state
        .target_agents
        .operation(&operation_id)
        .filter(|operation| operation.target_id == agent.target.id)
        .ok_or_else(|| ServiceError::with_status(StatusCode::NOT_FOUND, "operation not found"))?;
    validate_agent_operation_binding(
        &operation,
        &agent,
        &operation.authority.request_digest,
        &operation.authority.authority_digest,
        None,
    )?;
    if !matches!(
        operation.status,
        TargetOperationStatusKind::Accepted | TargetOperationStatusKind::Running
    ) || !operation.authority.artifact_digests.contains(&digest)
    {
        return Err(ServiceError::with_status(
            StatusCode::FORBIDDEN,
            "artifact is not authorized for this accepted operation",
        ));
    }

    let object_store = state.runtime.object_store();
    let info = object_store
        .verify(&digest)
        .await
        .map_err(|error| match error {
            plurora_runtime::ObjectStoreError::NotFound { .. } => ServiceError::with_status(
                StatusCode::NOT_FOUND,
                "authorized artifact was not found",
            ),
            other => target_internal_error(other.into()),
        })?;
    if operation
        .spec
        .expected_size(&digest)
        .is_some_and(|expected| expected != info.size_bytes)
    {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "authorized artifact size does not match the operation descriptor",
        ));
    }
    let reader = object_store
        .stream(&digest)
        .await
        .map_err(|error| target_internal_error(error.into()))?;
    let body_stream = futures::stream::try_unfold(reader, |mut reader| async move {
        let mut chunk = vec![0u8; 64 * 1024];
        let read = reader.read(&mut chunk).await?;
        if read == 0 {
            Ok::<_, std::io::Error>(None)
        } else {
            chunk.truncate(read);
            Ok::<_, std::io::Error>(Some((chunk, reader)))
        }
    });
    let etag = format!("\"{}\"", info.digest);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, info.size_bytes)
        .header(header::ETAG, etag)
        .header("x-plurora-artifact-digest", info.digest)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from_stream(body_stream))
        .map_err(|error| target_internal_error(error.into()))
}

async fn drive_local_operation<S>(
    state: &AppState<S>,
    operation_id: &str,
) -> anyhow::Result<TargetOperationRecord>
where
    S: EventStore,
{
    drive_local_operation_with_driver(state, operation_id, &RuntimeLocalOperationDriver).await
}

struct RuntimeLocalOperationDriver;

#[async_trait::async_trait]
trait LocalOperationDriver: Send + Sync {
    async fn execute<S>(
        &self,
        state: &AppState<S>,
        operation: &TargetOperationRecord,
        execution_fence: &LocalOperationExecutionFence,
    ) -> anyhow::Result<Value>
    where
        S: EventStore;
}

#[async_trait::async_trait]
impl LocalOperationDriver for RuntimeLocalOperationDriver {
    async fn execute<S>(
        &self,
        state: &AppState<S>,
        operation: &TargetOperationRecord,
        execution_fence: &LocalOperationExecutionFence,
    ) -> anyhow::Result<Value>
    where
        S: EventStore,
    {
        execute_local_operation(state, operation, execution_fence).await
    }
}

#[derive(Clone)]
struct LocalOperationExecutionFence {
    host_lease: crate::development::DevelopmentHostLease,
    host_fence: TargetOperationHostFence,
    executor_claim: TargetOperationExecutorClaim,
}

impl LocalOperationExecutionFence {
    async fn ensure_current<S>(&self, state: &AppState<S>) -> anyhow::Result<TargetOperationRecord>
    where
        S: EventStore,
    {
        ensure_executor_claim(
            state.runtime.store().as_ref(),
            state.target_agents.as_ref(),
            &self.host_lease,
            &self.host_fence,
            &self.executor_claim,
        )
        .await?;
        let operation = state
            .target_agents
            .operation(&self.executor_claim.operation_id)
            .context("local target operation disappeared while executing")?;
        validate_local_executor_binding(&operation, &self.executor_claim)?;
        let target = state
            .runtime
            .config()
            .target_registry
            .status(&operation.target_id)
            .await
            .context("local target disappeared while executing")?;
        anyhow::ensure!(
            target.status == ExecutionTargetStatusKind::Available
                && resolve_target_driver(&target) == TargetDriverKind::Local
                && target.lease_epoch == operation.authority.lease_epoch
                && target.policy_epoch == operation.authority.policy_epoch
                && required_capabilities(&operation.spec)
                    .iter()
                    .all(|required| target.capabilities.contains(required)),
            "local target authority changed while executing"
        );
        Ok(operation)
    }
}

struct LocalRuntimeEffectGuard<'a, S>
where
    S: EventStore,
{
    state: &'a AppState<S>,
    execution_fence: &'a LocalOperationExecutionFence,
}

#[async_trait::async_trait]
impl<S> plurora_runtime::ManagedTargetEffectGuard for LocalRuntimeEffectGuard<'_, S>
where
    S: EventStore,
{
    async fn ensure_current(&self) -> anyhow::Result<()> {
        self.execution_fence
            .ensure_current(self.state)
            .await
            .map(|_| ())
    }
}

fn validate_local_executor_binding(
    operation: &TargetOperationRecord,
    claim: &TargetOperationExecutorClaim,
) -> anyhow::Result<()> {
    let execution_matches = match operation.status {
        TargetOperationStatusKind::Requested => operation
            .execution_id
            .as_deref()
            .is_none_or(|execution_id| execution_id == claim.execution_id),
        TargetOperationStatusKind::Accepted | TargetOperationStatusKind::Running => {
            operation.execution_id.as_deref() == Some(claim.execution_id.as_str())
        }
        TargetOperationStatusKind::Succeeded
        | TargetOperationStatusKind::Failed
        | TargetOperationStatusKind::Cancelled
        | TargetOperationStatusKind::OutcomeUnknown
        | TargetOperationStatusKind::Expired => false,
    };
    anyhow::ensure!(
        operation.operation_id == claim.operation_id
            && operation.target_id == "local"
            && claim.target_id == operation.target_id
            && execution_matches,
        "local target operation no longer permits this executor"
    );
    Ok(())
}

struct LocalOperationAdvance {
    record: TargetOperationRecord,
    appended: bool,
}

async fn drive_local_operation_with_driver<S, D>(
    state: &AppState<S>,
    operation_id: &str,
    driver: &D,
) -> anyhow::Result<TargetOperationRecord>
where
    S: EventStore,
    D: LocalOperationDriver,
{
    sync_target_operation_journal(state.runtime.store().as_ref(), state.target_agents.as_ref())
        .await?;
    let current = state
        .target_agents
        .operation(operation_id)
        .context("local target operation disappeared")?;
    if current.status.is_terminal() {
        return Ok(current);
    }

    let execution_lock = state.target_agents.local_execution_lock(operation_id);
    let _execution_guard = execution_lock.lock().await;
    sync_target_operation_journal(state.runtime.store().as_ref(), state.target_agents.as_ref())
        .await?;
    let current = state
        .target_agents
        .operation(operation_id)
        .context("local target operation disappeared")?;
    if current.status.is_terminal() {
        return Ok(current);
    }
    let (host_lease, host_fence) = current_operation_host_fence(state).await?;
    let executor_claim = claim_local_operation(
        state.runtime.store().as_ref(),
        state.target_agents.as_ref(),
        &host_lease,
        &host_fence,
        operation_id,
    )
    .await?
    .context("local target operation became terminal before executor claim")?;
    let execution_fence = LocalOperationExecutionFence {
        host_lease,
        host_fence,
        executor_claim,
    };

    // Once Running is durable, a crash or Host takeover cannot distinguish an
    // effect that never started from one whose outcome was lost. Replaying it
    // would violate at-most-once execution, so close it as OutcomeUnknown.
    let current = execution_fence.ensure_current(state).await?;
    if current.status == TargetOperationStatusKind::Running {
        let completed_at_ms = Utc::now().timestamp_millis().max(current.updated_at_ms);
        return complete_local_operation(
            state,
            &execution_fence,
            TargetOperationReceipt {
                operation_id: current.operation_id.clone(),
                target_id: current.target_id.clone(),
                execution_id: execution_fence.executor_claim.execution_id.clone(),
                step_id: OPERATION_STEP_ID.to_string(),
                request_digest: current.authority.request_digest.clone(),
                authority_digest: current.authority.authority_digest.clone(),
                status: TargetOperationReceiptStatus::OutcomeUnknown,
                completed_at_ms,
                output: Value::Null,
                diagnostics: vec![
                    "local target effect outcome was unresolved before executor takeover"
                        .to_string(),
                ],
            },
        )
        .await;
    }

    let accepted =
        advance_local_operation(state, &execution_fence, TargetOperationStatusKind::Accepted)
            .await?;
    if accepted.record.status.is_terminal() {
        return Ok(accepted.record);
    }
    let running =
        advance_local_operation(state, &execution_fence, TargetOperationStatusKind::Running)
            .await?;
    if running.record.status.is_terminal() {
        return Ok(running.record);
    }
    anyhow::ensure!(
        running.appended,
        "local target operation was already Running before this executor"
    );
    execution_fence.ensure_current(state).await?;
    let (status, output, diagnostics) = match driver
        .execute(state, &running.record, &execution_fence)
        .await
    {
        Ok(output) => (TargetOperationReceiptStatus::Succeeded, output, Vec::new()),
        Err(error) if plurora_runtime::is_managed_target_deployment_outcome_unknown(&error) => (
            TargetOperationReceiptStatus::OutcomeUnknown,
            Value::Null,
            vec!["local target deployment outcome is unknown".to_string()],
        ),
        Err(_) => (
            TargetOperationReceiptStatus::Failed,
            Value::Null,
            vec!["local target operation failed".to_string()],
        ),
    };
    execution_fence.ensure_current(state).await?;
    let completed_at_ms = Utc::now().timestamp_millis();
    complete_local_operation(
        state,
        &execution_fence,
        TargetOperationReceipt {
            operation_id: operation_id.to_string(),
            target_id: running.record.target_id.clone(),
            execution_id: execution_fence.executor_claim.execution_id.clone(),
            step_id: OPERATION_STEP_ID.to_string(),
            request_digest: running.record.authority.request_digest.clone(),
            authority_digest: running.record.authority.authority_digest.clone(),
            status,
            completed_at_ms,
            output,
            diagnostics,
        },
    )
    .await
}

async fn advance_local_operation<S>(
    state: &AppState<S>,
    execution_fence: &LocalOperationExecutionFence,
    requested_status: TargetOperationStatusKind,
) -> anyhow::Result<LocalOperationAdvance>
where
    S: EventStore,
{
    anyhow::ensure!(
        matches!(
            requested_status,
            TargetOperationStatusKind::Accepted | TargetOperationStatusKind::Running
        ),
        "local target progress status is invalid"
    );
    for _ in 0..8 {
        let current = execution_fence.ensure_current(state).await?;
        if current.status.is_terminal()
            || current.status == requested_status
            || (requested_status == TargetOperationStatusKind::Accepted
                && current.status == TargetOperationStatusKind::Running)
        {
            return Ok(LocalOperationAdvance {
                record: current,
                appended: false,
            });
        }
        let now_ms = Utc::now().timestamp_millis();
        if current.status == TargetOperationStatusKind::Requested
            && now_ms >= current.authority.expires_at_ms
        {
            let mut expired = current;
            expired.revision = expired.revision.saturating_add(1);
            expired.status = TargetOperationStatusKind::Expired;
            expired.updated_at_ms = now_ms;
            if append_target_operation_snapshot(
                state.runtime.store().as_ref(),
                state.target_agents.as_ref(),
                &execution_fence.host_lease,
                &execution_fence.host_fence,
                None,
                state.target_agents.operation_next_sequence(),
                &expired,
            )
            .await?
            .is_some()
            {
                return Ok(LocalOperationAdvance {
                    record: expired,
                    appended: true,
                });
            }
            continue;
        }
        anyhow::ensure!(
            matches!(
                (current.status, requested_status),
                (
                    TargetOperationStatusKind::Requested,
                    TargetOperationStatusKind::Accepted
                ) | (
                    TargetOperationStatusKind::Accepted,
                    TargetOperationStatusKind::Running
                )
            ),
            "local target operation progress transition is invalid"
        );
        let mut next = current;
        next.revision = next.revision.saturating_add(1);
        next.status = requested_status;
        if next.execution_id.is_none() {
            next.execution_id = Some(execution_fence.executor_claim.execution_id.clone());
        }
        anyhow::ensure!(
            next.execution_id.as_deref()
                == Some(execution_fence.executor_claim.execution_id.as_str()),
            "local target operation execution owner changed"
        );
        next.updated_at_ms = now_ms;
        if append_target_operation_snapshot(
            state.runtime.store().as_ref(),
            state.target_agents.as_ref(),
            &execution_fence.host_lease,
            &execution_fence.host_fence,
            Some(&execution_fence.executor_claim),
            state.target_agents.operation_next_sequence(),
            &next,
        )
        .await?
        .is_some()
        {
            return Ok(LocalOperationAdvance {
                record: next,
                appended: true,
            });
        }
    }
    anyhow::bail!("local target operation journal changed too frequently")
}

async fn execute_local_operation<S>(
    state: &AppState<S>,
    operation: &TargetOperationRecord,
    execution_fence: &LocalOperationExecutionFence,
) -> anyhow::Result<Value>
where
    S: EventStore,
{
    let runtime_guard = LocalRuntimeEffectGuard {
        state,
        execution_fence,
    };
    match &operation.spec {
        TargetOperationSpec::ArtifactMaterialize {
            digest,
            expected_size_bytes,
        } => {
            let info = state.runtime.object_store().verify(digest).await?;
            anyhow::ensure!(
                expected_size_bytes.is_none_or(|expected| expected == info.size_bytes),
                "local target artifact size did not match"
            );
            Ok(json!({
                "digest": digest,
                "size_bytes": info.size_bytes,
                "already_present": true
            }))
        }
        TargetOperationSpec::ArtifactRelease { digest } => Ok(json!({
            "digest": digest,
            "released": false,
            "retained_by_host": true
        })),
        TargetOperationSpec::DeploymentApply { deployment } => {
            let reference = &deployment.deployment;
            execution_fence.ensure_current(state).await?;
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
                &runtime_guard,
            )
            .await?;
            execution_fence.ensure_current(state).await?;
            if let Err(error) = plurora_runtime::wait_for_managed_target_deployment_readiness(
                &applied,
                deployment.health_path.as_deref(),
                &runtime_guard,
            )
            .await
            {
                if execution_fence.ensure_current(state).await.is_err() {
                    return Err(plurora_runtime::managed_target_deployment_outcome_unknown(
                        "candidate readiness after executor fencing",
                    ));
                }
                let cleanup = plurora_runtime::stop_managed_target_deployment(
                    &managed_deployment_ref(operation, &deployment.deployment),
                    0,
                    true,
                    &runtime_guard,
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
            execution_fence.ensure_current(state).await?;
            let observed = plurora_runtime::observe_managed_target_deployment(
                &managed_deployment_ref(operation, deployment),
                &runtime_guard,
            )
            .await?;
            Ok(json!({ "deployment": observed }))
        }
        TargetOperationSpec::DeploymentDrain {
            deployment,
            grace_seconds,
        } => {
            execution_fence.ensure_current(state).await?;
            Ok(serde_json::to_value(
                plurora_runtime::drain_managed_target_deployment(
                    &managed_deployment_ref(operation, deployment),
                    *grace_seconds,
                    &runtime_guard,
                )
                .await?,
            )?)
        }
        TargetOperationSpec::DeploymentStop {
            deployment,
            grace_seconds,
            force_remove,
        } => {
            execution_fence.ensure_current(state).await?;
            Ok(serde_json::to_value(
                plurora_runtime::stop_managed_target_deployment(
                    &managed_deployment_ref(operation, deployment),
                    *grace_seconds,
                    *force_remove,
                    &runtime_guard,
                )
                .await?,
            )?)
        }
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
            let info = state.runtime.object_store().verify(digest).await?;
            anyhow::ensure!(
                expected_size_bytes.is_none_or(|expected| expected == info.size_bytes),
                "local target artifact size did not match"
            );
            Ok(json!({
                "digest": digest,
                "size_bytes": info.size_bytes,
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
                    disposition,
                    build_id,
                    workspace_id,
                    source_tree_digest,
                    build_descriptor_hash,
                },
        } => {
            let context_tar = state.runtime.object_store().get(digest).await?;
            anyhow::ensure!(
                expected_size_bytes.is_none_or(|expected| expected == context_tar.len() as u64),
                "local target build context size did not match"
            );
            execution_fence.ensure_current(state).await?;
            let built = plurora_runtime::build_managed_target_image(
                plurora_runtime::ManagedTargetImageBuild {
                    target_id: operation.target_id.clone(),
                    installation_id: operation.installation_id.clone(),
                    workspace_id: workspace_id.clone(),
                    build_id: build_id.clone(),
                    dockerfile: dockerfile.clone(),
                    network_mode: *network_mode,
                    disposition: *disposition,
                    source_tree_digest: source_tree_digest.clone(),
                    build_descriptor_hash: build_descriptor_hash.clone(),
                    context_digest: digest.clone(),
                    context_tar: context_tar.to_vec(),
                },
                &runtime_guard,
            )
            .await?;
            execution_fence.ensure_current(state).await?;
            Ok(serde_json::to_value(
                plurora_runtime::finalize_managed_target_image_build(built, &runtime_guard).await?,
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

async fn installation_deployment_operation<S>(
    state: &AppState<S>,
    operation: &TargetOperationRecord,
) -> anyhow::Result<bool>
where
    S: EventStore,
{
    #[cfg(test)]
    {
        state
            .target_agents
            .operations
            .lock()
            .expect("target operation state lock poisoned")
            .projection_invocations += 1;
    }
    if operation.status != TargetOperationStatusKind::Succeeded {
        return Ok(false);
    }
    let Some(receipt) = operation.receipt.as_ref() else {
        return Ok(false);
    };
    match &operation.spec {
        TargetOperationSpec::DeploymentApply { deployment } => {
            installation_running_deployment(
                state,
                operation,
                &deployment.deployment,
                &deployment.port_name,
                &receipt.output,
            )
            .await?;
        }
        TargetOperationSpec::DeploymentObserve { deployment } => {
            let Some(observation) = receipt.output.get("deployment") else {
                anyhow::bail!("deployment observation receipt has no deployment field");
            };
            if observation.is_null()
                || !observation
                    .get("running")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            {
                installation_stopped_deployment(state, deployment).await?;
            } else {
                let lease = required_deployment_lease(state, deployment).await?;
                installation_running_deployment(
                    state,
                    operation,
                    deployment,
                    &lease.port_name,
                    observation,
                )
                .await?;
            }
        }
        TargetOperationSpec::DeploymentDrain { deployment, .. }
        | TargetOperationSpec::DeploymentStop { deployment, .. } => {
            installation_stopped_deployment(state, deployment).await?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

async fn required_deployment_lease<S>(
    state: &AppState<S>,
    deployment: &TargetDeploymentRef,
) -> anyhow::Result<plurora_runtime::PortLeaseRecord>
where
    S: EventStore,
{
    state
        .runtime
        .config()
        .port_lease_registry
        .status(&deployment.port_lease_id)
        .await
        .context("target deployment port lease disappeared")
}

async fn installation_running_deployment<S>(
    state: &AppState<S>,
    operation: &TargetOperationRecord,
    deployment: &TargetDeploymentRef,
    port_name: &str,
    observation: &Value,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    anyhow::ensure!(
        observation.get("bind_host").and_then(Value::as_str) == Some("127.0.0.1")
            && observation.get("running").and_then(Value::as_bool) == Some(true),
        "target deployment receipt is not a running loopback observation"
    );
    let host_port = observation
        .get("host_port")
        .and_then(Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port > 0)
        .context("target deployment receipt has no actual host port")?;
    let lease = required_deployment_lease(state, deployment).await?;
    let route = state
        .runtime
        .config()
        .proxy_route_registry
        .status(&deployment.route_id)
        .await
        .context("target deployment proxy route disappeared")?;
    if route.upstream.port_lease_id != deployment.port_lease_id {
        // A later candidate may have atomically moved the durable route to a
        // different lease. Historical receipts must not move it back.
        return Ok(());
    }
    anyhow::ensure!(
        lease.target_id == operation.target_id
            && lease.port_name == port_name
            && lease.host == "127.0.0.1"
            && lease.bind == plurora_runtime::PortBindScope::LoopbackOnly
            && lease.protocol == plurora_runtime::PortProtocol::Tcp
            && route.upstream.port_name == port_name,
        "target deployment receipt conflicts with its Host route or lease"
    );
    if lease.status == plurora_runtime::PortLeaseStatusKind::Released
        || route.status == plurora_runtime::ProxyRouteStatusKind::Removed
    {
        return Ok(());
    }
    state
        .runtime
        .config()
        .port_lease_registry
        .bind_actual_port(
            &deployment.port_lease_id,
            &operation.target_id,
            port_name,
            host_port,
        )
        .await
        .context("target deployment actual port could not bind its Host lease")?;
    let target_available = state
        .runtime
        .config()
        .target_registry
        .status(&operation.target_id)
        .await
        .is_some_and(|target| target.status == ExecutionTargetStatusKind::Available);
    for alias in state.runtime.config().proxy_route_registry.list().await {
        if alias.status == plurora_runtime::ProxyRouteStatusKind::Removed
            || alias.upstream.port_lease_id != deployment.port_lease_id
        {
            continue;
        }
        anyhow::ensure!(
            alias.upstream.port_name == port_name,
            "target deployment route alias conflicts with its Host lease"
        );
        state
            .runtime
            .config()
            .proxy_route_registry
            .set_status(&alias.id, plurora_runtime::ProxyRouteStatusKind::Active)
            .await
            .context("target deployment route alias disappeared during promotion")?;
        state
            .runtime
            .config()
            .proxy_route_registry
            .set_ready_if_active_with_lease(&alias.id, &deployment.port_lease_id, target_available)
            .await
            .context("target deployment route alias changed during promotion")?;
    }
    Ok(())
}

async fn installation_stopped_deployment<S>(
    state: &AppState<S>,
    deployment: &TargetDeploymentRef,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    for route in state.runtime.config().proxy_route_registry.list().await {
        if route.upstream.port_lease_id == deployment.port_lease_id
            && route.status != plurora_runtime::ProxyRouteStatusKind::Removed
        {
            let _ = state
                .runtime
                .config()
                .proxy_route_registry
                .set_ready(&route.id, false)
                .await;
            let _ = state
                .runtime
                .config()
                .proxy_route_registry
                .set_status(&route.id, plurora_runtime::ProxyRouteStatusKind::Stale)
                .await;
        }
    }
    if let Some(lease) = state
        .runtime
        .config()
        .port_lease_registry
        .status(&deployment.port_lease_id)
        .await
    {
        if lease.status != plurora_runtime::PortLeaseStatusKind::Released {
            let _ = state
                .runtime
                .config()
                .port_lease_registry
                .set_status(
                    &deployment.port_lease_id,
                    plurora_runtime::PortLeaseStatusKind::Reserved,
                )
                .await;
        }
    }
    Ok(())
}

pub(super) async fn reconcile_target_deployment_projections<S>(
    state: &AppState<S>,
    target_id: &str,
) -> anyhow::Result<usize>
where
    S: EventStore,
{
    let mut projected = 0usize;
    for operation in state
        .target_agents
        .terminal_operations_for_target(target_id)
    {
        if installation_deployment_operation(state, &operation).await? {
            projected = projected.saturating_add(1);
        }
    }
    Ok(projected)
}

pub(super) async fn reconcile_all_target_deployment_projections<S>(
    state: &AppState<S>,
) -> anyhow::Result<usize>
where
    S: EventStore,
{
    let mut projected = 0usize;
    for target_id in state.target_agents.operation_target_ids() {
        projected = projected
            .saturating_add(reconcile_target_deployment_projections(state, &target_id).await?);
    }
    Ok(projected)
}

pub(super) async fn mark_target_deployment_routes_unready<S>(
    state: &AppState<S>,
    target_id: &str,
) -> usize
where
    S: EventStore,
{
    let lease_ids = state
        .runtime
        .config()
        .port_lease_registry
        .list()
        .await
        .into_iter()
        .filter(|lease| lease.target_id == target_id)
        .map(|lease| lease.id)
        .collect::<std::collections::HashSet<_>>();
    let mut updated = 0usize;
    for route in state.runtime.config().proxy_route_registry.list().await {
        if route.ready
            && lease_ids.contains(&route.upstream.port_lease_id)
            && state
                .runtime
                .config()
                .proxy_route_registry
                .set_ready(&route.id, false)
                .await
                .is_some()
        {
            updated = updated.saturating_add(1);
        }
    }
    updated
}

async fn complete_local_operation<S>(
    state: &AppState<S>,
    execution_fence: &LocalOperationExecutionFence,
    receipt: TargetOperationReceipt,
) -> anyhow::Result<TargetOperationRecord>
where
    S: EventStore,
{
    for _ in 0..8 {
        ensure_executor_claim(
            state.runtime.store().as_ref(),
            state.target_agents.as_ref(),
            &execution_fence.host_lease,
            &execution_fence.host_fence,
            &execution_fence.executor_claim,
        )
        .await?;
        let current = state
            .target_agents
            .operation(&execution_fence.executor_claim.operation_id)
            .context("local target operation disappeared while completing")?;
        if current.status.is_terminal() {
            return Ok(current);
        }
        execution_fence.ensure_current(state).await?;
        anyhow::ensure!(
            matches!(
                current.status,
                TargetOperationStatusKind::Accepted | TargetOperationStatusKind::Running
            ),
            "local target operation was not durably accepted"
        );
        let mut next = current;
        next.revision = next.revision.saturating_add(1);
        next.status = receipt.status.into();
        next.updated_at_ms = receipt.completed_at_ms.max(Utc::now().timestamp_millis());
        next.receipt = Some(receipt.clone());
        let authority_key = state
            .target_agents
            .operation_authority_key(&next.target_id, next.authority.lease_epoch)
            .ok_or_else(|| anyhow::anyhow!("local target authority key disappeared"))?;
        validate_record_integrity(&next, &authority_key)?;
        if append_target_operation_snapshot(
            state.runtime.store().as_ref(),
            state.target_agents.as_ref(),
            &execution_fence.host_lease,
            &execution_fence.host_fence,
            Some(&execution_fence.executor_claim),
            state.target_agents.operation_next_sequence(),
            &next,
        )
        .await?
        .is_some()
        {
            execution_fence.host_lease.ensure_durable_owner().await?;
            installation_deployment_operation(state, &next).await?;
            return Ok(next);
        }
    }
    anyhow::bail!("local target operation journal changed too frequently")
}

async fn create_operation_record<S>(
    store: &S,
    registry: &TargetAgentRegistry,
    host_lease: &crate::development::DevelopmentHostLease,
    host_fence: &TargetOperationHostFence,
    authority_key: &str,
    target: ExecutionTarget,
    request: CreateTargetOperationRequest,
) -> anyhow::Result<TargetOperationRecord>
where
    S: EventStore,
{
    let request_digest =
        operation_request_digest(&target.id, &request.installation_id, &request.spec)?;
    let operation_id = plurora_core::new_id("target-operation");
    let now_ms = Utc::now().timestamp_millis();
    let expires_at_ms = now_ms.saturating_add(
        i64::try_from(
            request
                .expires_in_seconds
                .unwrap_or(DEFAULT_AUTHORITY_TTL_SECS),
        )
        .unwrap_or(i64::MAX)
        .saturating_mul(1_000),
    );
    let mut authority = TargetOperationAuthority {
        target_id: target.id.clone(),
        operation_id: operation_id.clone(),
        step_id: OPERATION_STEP_ID.to_string(),
        installation_id: request.installation_id.clone(),
        effect: request.spec.effect(),
        artifact_digests: request.spec.artifact_digests(),
        lease_epoch: target.lease_epoch,
        policy_epoch: target.policy_epoch,
        issued_at_ms: now_ms,
        expires_at_ms,
        nonce: random_secret(),
        request_digest,
        authority_digest: String::new(),
    };
    authority.authority_digest = operation_authority_digest(&authority, authority_key)?;
    let record = TargetOperationRecord {
        operation_id,
        target_id: target.id,
        installation_id: request.installation_id,
        revision: 1,
        status: TargetOperationStatusKind::Requested,
        execution_id: None,
        spec: request.spec,
        authority,
        idempotency_key: request.idempotency_key,
        receipt: None,
        created_at_ms: now_ms,
        updated_at_ms: now_ms,
    };
    validate_record_integrity(&record, authority_key)?;

    for _ in 0..8 {
        ensure_operation_host_fence(store, registry, host_lease, host_fence).await?;
        if let Some(key) = record.idempotency_key.as_deref() {
            if let Some((existing_digest, existing)) =
                registry.idempotent_operation(&record.target_id, &record.installation_id, key)
            {
                anyhow::ensure!(
                    existing_digest == record.authority.request_digest,
                    "idempotency_key was already used for a different target operation"
                );
                return Ok(existing);
            }
        }
        if append_target_operation_snapshot(
            store,
            registry,
            host_lease,
            host_fence,
            None,
            registry.operation_next_sequence(),
            &record,
        )
        .await?
        .is_some()
        {
            return Ok(record);
        }
    }
    anyhow::bail!("target operation journal changed too frequently to create operation")
}

async fn append_target_operation_snapshot<S>(
    store: &S,
    registry: &TargetAgentRegistry,
    host_lease: &crate::development::DevelopmentHostLease,
    host_fence: &TargetOperationHostFence,
    executor_claim: Option<&TargetOperationExecutorClaim>,
    expected_next: EventSequence,
    record: &TargetOperationRecord,
) -> anyhow::Result<Option<EventEnvelope>>
where
    S: EventStore,
{
    ensure_operation_host_fence(store, registry, host_lease, host_fence).await?;
    if record.target_id == "local"
        && !matches!(
            record.status,
            TargetOperationStatusKind::Requested | TargetOperationStatusKind::Expired
        )
    {
        let claim = executor_claim.ok_or_else(|| {
            anyhow::anyhow!("local target operation append has no executor claim")
        })?;
        ensure_executor_claim(store, registry, host_lease, host_fence, claim).await?;
        anyhow::ensure!(
            claim.operation_id == record.operation_id
                && record.execution_id.as_deref() == Some(claim.execution_id.as_str()),
            "local target operation append does not match its executor claim"
        );
    } else {
        anyhow::ensure!(
            executor_claim.is_none(),
            "non-executing target operation append unexpectedly has an executor claim"
        );
    }
    let authority_key = registry
        .operation_authority_key(&record.target_id, record.authority.lease_epoch)
        .ok_or_else(|| anyhow::anyhow!("target operation authority key is unknown"))?;
    validate_record_integrity(record, &authority_key)?;
    host_lease.ensure_durable_owner().await?;
    let event = store
        .append_with_sequence_if_next(
            OPERATION_JOURNAL_SESSION.to_string(),
            expected_next,
            JOURNAL_WRITER.to_string(),
            OPERATION_JOURNAL_EVENT.to_string(),
            1,
            serde_json::to_value(TargetOperationSnapshot {
                record: record.clone(),
            })?,
            json!({
                "owner": "host_control_plane",
                "redacted": true,
                "authority_material": "digest_bound"
            }),
        )
        .await?;
    if let Some(event) = &event {
        registry.apply_operation_event(event)?;
    }
    Ok(event)
}

pub(super) async fn sync_target_operation_journal<S>(
    store: &S,
    registry: &TargetAgentRegistry,
) -> anyhow::Result<usize>
where
    S: EventStore,
{
    let mut loaded = 0usize;
    loop {
        let next = registry.operation_next_sequence();
        let events = store
            .list_session_range(
                &OPERATION_JOURNAL_SESSION.to_string(),
                next.checked_sub(1),
                Some(1_000),
            )
            .await?;
        if events.is_empty() {
            break;
        }
        for event in &events {
            registry.apply_operation_event(event)?;
            loaded = loaded.saturating_add(1);
        }
        if events.len() < 1_000 {
            break;
        }
    }
    Ok(loaded)
}

pub(super) async fn recover_local_operations_after_restart<S>(
    state: &AppState<S>,
) -> anyhow::Result<usize>
where
    S: EventStore,
{
    let (host_lease, host_fence) = current_operation_host_fence(state).await?;
    // The Host fence is in the same CAS journal as every operation snapshot.
    // Once it is established and synchronized, an old writer cannot append
    // behind a missing/Running conclusion made below.
    sync_target_operation_journal(state.runtime.store().as_ref(), state.target_agents.as_ref())
        .await?;
    let candidates = state
        .target_agents
        .operations_for_target("local")
        .into_iter()
        .filter(|operation| operation.status == TargetOperationStatusKind::Running)
        .map(|operation| operation.operation_id)
        .collect::<Vec<_>>();
    let mut recovered = 0usize;
    for operation_id in candidates {
        let Some(executor_claim) = claim_local_operation(
            state.runtime.store().as_ref(),
            state.target_agents.as_ref(),
            &host_lease,
            &host_fence,
            &operation_id,
        )
        .await?
        else {
            continue;
        };
        let execution_fence = LocalOperationExecutionFence {
            host_lease: host_lease.clone(),
            host_fence: host_fence.clone(),
            executor_claim,
        };
        let current = execution_fence.ensure_current(state).await?;
        if current.status != TargetOperationStatusKind::Running {
            continue;
        }
        let completed_at_ms = Utc::now()
            .timestamp_millis()
            .max(current.updated_at_ms)
            .max(current.authority.issued_at_ms);
        complete_local_operation(
            state,
            &execution_fence,
            TargetOperationReceipt {
                operation_id: current.operation_id.clone(),
                target_id: current.target_id.clone(),
                execution_id: execution_fence.executor_claim.execution_id.clone(),
                step_id: OPERATION_STEP_ID.to_string(),
                request_digest: current.authority.request_digest.clone(),
                authority_digest: current.authority.authority_digest.clone(),
                status: TargetOperationReceiptStatus::OutcomeUnknown,
                completed_at_ms,
                output: Value::Null,
                diagnostics: vec![
                    "Host restarted while the local effect outcome was unresolved".to_string(),
                ],
            },
        )
        .await?;
        recovered = recovered.saturating_add(1);
    }
    Ok(recovered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;
    use plurora_runtime::InMemoryEventStore;

    const TEST_CREDENTIAL: &str =
        "plurora_agent.remote-1.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn test_registry() -> TargetAgentRegistry {
        let registry = TargetAgentRegistry::default();
        registry.state.lock().unwrap().credential_digests.insert(
            ("remote-1".to_string(), 7),
            credential_digest("agent", TEST_CREDENTIAL),
        );
        registry
    }

    fn test_target() -> ExecutionTarget {
        ExecutionTarget {
            id: "remote-1".to_string(),
            name: "remote verifier".to_string(),
            reachability: ExecutionTargetReachability::ReverseTunnel,
            declared_capabilities: vec![
                ExecutionTargetCapability::ArtifactTransfer,
                ExecutionTargetCapability::DeclarativeVerifier,
            ],
            capabilities: vec![
                ExecutionTargetCapability::ArtifactTransfer,
                ExecutionTargetCapability::DeclarativeVerifier,
            ],
            status: ExecutionTargetStatusKind::Available,
            protocol_versions: vec![PROTOCOL_VERSION.to_string()],
            selected_protocol_version: Some(PROTOCOL_VERSION.to_string()),
            identity_ref: Some("target:remote-1:7".to_string()),
            labels: BTreeMap::new(),
            observed: Some(ExecutionTargetObservedSummary::default()),
            last_seen_at_ms: Some(Utc::now().timestamp_millis()),
            heartbeat_expires_at_ms: Some(Utc::now().timestamp_millis() + HEARTBEAT_TTL_MS),
            enrolled_at_ms: Some(Utc::now().timestamp_millis()),
            revoked_at_ms: None,
            lease_epoch: 7,
            policy_epoch: 11,
        }
    }

    fn operation_request(key: &str) -> CreateTargetOperationRequest {
        CreateTargetOperationRequest {
            installation_id: InstallationId::parse("11111111-1111-4111-8111-111111111111").unwrap(),
            spec: TargetOperationSpec::VerifierRun {
                verifier: DeclarativeVerifierDescriptor::ArtifactIntegrity {
                    digest: format!("sha256:{}", "a".repeat(64)),
                    expected_size_bytes: Some(42),
                },
            },
            idempotency_key: Some(key.to_string()),
            expires_in_seconds: Some(120),
        }
    }

    async fn explain_service_error(error: ServiceError) -> anyhow::Error {
        use axum::response::IntoResponse;

        let response = error.into_response();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_else(|body_error| format!("<body decode failed: {body_error}>"));
        anyhow::anyhow!("status={status} body={body}")
    }

    async fn test_operation_authority(
        store: Arc<InMemoryEventStore>,
        development: Arc<crate::DevelopmentRegistry>,
        registry: Arc<TargetAgentRegistry>,
    ) -> anyhow::Result<(crate::DevelopmentHostLease, TargetOperationHostFence)> {
        let lease = crate::acquire_development_host_lease(store.clone(), development).await?;
        let fence =
            establish_operation_host_fence(store.as_ref(), registry.as_ref(), &lease).await?;
        Ok((lease, fence))
    }

    fn test_state(
        store: Arc<InMemoryEventStore>,
        objects: Arc<plurora_runtime::InMemoryObjectStore>,
        target_agents: Arc<TargetAgentRegistry>,
    ) -> anyhow::Result<AppState<InMemoryEventStore>> {
        let installations = crate::InstallationRegistry::ephemeral(store.clone(), objects.clone())?;
        let runtime = Arc::new(plurora_runtime::Runtime::new(
            store,
            plurora_runtime::RuntimeConfig {
                object_store: objects,
                installation_control: installations.clone(),
                ..plurora_runtime::RuntimeConfig::default()
            },
        ));
        Ok(AppState {
            runtime,
            static_dir: None,
            access_token: None,
            app_base_domain: None,
            build_jobs: Arc::new(crate::BuildDeployJobRegistry::default()),
            development: crate::development_registry(),
            host_access: crate::host_access_registry(),
            installations,
            target_agents,
        })
    }

    #[derive(Default)]
    struct CountingLocalOperationDriver {
        executions: std::sync::atomic::AtomicUsize,
    }

    impl CountingLocalOperationDriver {
        fn executions(&self) -> usize {
            self.executions.load(std::sync::atomic::Ordering::Acquire)
        }
    }

    #[async_trait::async_trait]
    impl LocalOperationDriver for CountingLocalOperationDriver {
        async fn execute<S>(
            &self,
            state: &AppState<S>,
            operation: &TargetOperationRecord,
            execution_fence: &LocalOperationExecutionFence,
        ) -> anyhow::Result<Value>
        where
            S: EventStore,
        {
            let current = execution_fence.ensure_current(state).await?;
            anyhow::ensure!(
                current == *operation && current.status == TargetOperationStatusKind::Running,
                "test driver received a non-current operation"
            );
            self.executions
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            Ok(json!({ "healthy": true }))
        }
    }

    #[derive(Default)]
    struct BarrierLocalOperationDriver {
        entered_effect: tokio::sync::Notify,
        release_effect: tokio::sync::Notify,
        effects: std::sync::atomic::AtomicUsize,
    }

    impl BarrierLocalOperationDriver {
        fn effects(&self) -> usize {
            self.effects.load(std::sync::atomic::Ordering::Acquire)
        }
    }

    #[async_trait::async_trait]
    impl LocalOperationDriver for BarrierLocalOperationDriver {
        async fn execute<S>(
            &self,
            state: &AppState<S>,
            operation: &TargetOperationRecord,
            execution_fence: &LocalOperationExecutionFence,
        ) -> anyhow::Result<Value>
        where
            S: EventStore,
        {
            execution_fence.ensure_current(state).await?;
            anyhow::ensure!(
                operation.status == TargetOperationStatusKind::Running,
                "barrier driver requires a durable Running operation"
            );
            self.effects
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            self.entered_effect.notify_one();
            self.release_effect.notified().await;
            execution_fence.ensure_current(state).await?;
            self.effects
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            Ok(json!({ "healthy": true }))
        }
    }

    #[tokio::test]
    async fn operation_idempotency_and_hydration_preserve_one_authority() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let registry = Arc::new(test_registry());
        let development = crate::development_registry();
        let (lease, fence) =
            test_operation_authority(store.clone(), development, registry.clone()).await?;
        let authority_key = credential_digest("agent", TEST_CREDENTIAL);
        let first = create_operation_record(
            store.as_ref(),
            registry.as_ref(),
            &lease,
            &fence,
            &authority_key,
            test_target(),
            operation_request("retry-1"),
        )
        .await?;
        let duplicate = create_operation_record(
            store.as_ref(),
            registry.as_ref(),
            &lease,
            &fence,
            &authority_key,
            test_target(),
            operation_request("retry-1"),
        )
        .await?;
        assert_eq!(first.operation_id, duplicate.operation_id);
        assert_eq!(first.authority, duplicate.authority);

        let restored = test_registry();
        assert_eq!(
            sync_target_operation_journal(store.as_ref(), &restored).await?,
            2
        );
        assert_eq!(restored.operation(&first.operation_id), Some(first));
        Ok(())
    }

    #[tokio::test]
    async fn authority_tampering_and_raw_secret_receipts_fail_closed() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let registry = Arc::new(test_registry());
        let development = crate::development_registry();
        let (lease, fence) =
            test_operation_authority(store.clone(), development, registry.clone()).await?;
        let authority_key = credential_digest("agent", TEST_CREDENTIAL);
        let record = create_operation_record(
            store.as_ref(),
            registry.as_ref(),
            &lease,
            &fence,
            &authority_key,
            test_target(),
            operation_request("retry-1"),
        )
        .await?;
        verify_target_operation_authority(
            &record,
            TEST_CREDENTIAL,
            "remote-1",
            7,
            11,
            record.created_at_ms,
            true,
        )
        .map_err(anyhow::Error::msg)?;
        assert!(verify_target_operation_authority(
            &record,
            "plurora_agent.remote-1.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "remote-1",
            7,
            11,
            record.created_at_ms,
            true,
        )
        .is_err());

        let mut tampered = record.clone();
        tampered.authority.artifact_digests.clear();
        assert!(verify_target_operation_authority(
            &tampered,
            TEST_CREDENTIAL,
            "remote-1",
            7,
            11,
            tampered.created_at_ms,
            true,
        )
        .is_err());

        let mut completed = record;
        completed.revision = 2;
        completed.status = TargetOperationStatusKind::Succeeded;
        completed.execution_id = Some("b".repeat(32));
        completed.receipt = Some(TargetOperationReceipt {
            operation_id: completed.operation_id.clone(),
            target_id: completed.target_id.clone(),
            execution_id: "b".repeat(32),
            step_id: OPERATION_STEP_ID.to_string(),
            request_digest: completed.authority.request_digest.clone(),
            authority_digest: completed.authority.authority_digest.clone(),
            status: TargetOperationReceiptStatus::Succeeded,
            completed_at_ms: completed.created_at_ms,
            output: json!({ "access_token": "sk-abcdefghijklmnopqrstuvwxyz123456" }),
            diagnostics: Vec::new(),
        });
        assert!(validate_record_integrity(&completed, &authority_key).is_err());
        let receipt = completed.receipt.as_mut().expect("receipt exists");
        receipt.output = Value::Null;
        receipt.diagnostics = vec!["sk-Abcdefghijklmnopqrstuvwxyz123456".to_string()];
        assert!(validate_record_integrity(&completed, &authority_key).is_err());
        Ok(())
    }

    #[tokio::test]
    async fn fresh_local_driver_binds_claim_and_executes_exactly_once() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let objects = Arc::new(plurora_runtime::InMemoryObjectStore::default());
        let registry = Arc::new(TargetAgentRegistry::default());
        let state = test_state(store.clone(), objects, registry.clone())?;
        let (lease, fence) =
            test_operation_authority(store.clone(), state.development.clone(), registry.clone())
                .await?;
        let runtime = state.runtime.clone();
        let target = runtime
            .config()
            .target_registry
            .status("local")
            .await
            .expect("local target exists");
        let authority_key = registry
            .operation_authority_key("local", target.lease_epoch)
            .expect("local target authority key exists");
        let requested = create_operation_record(
            store.as_ref(),
            registry.as_ref(),
            &lease,
            &fence,
            &authority_key,
            target,
            CreateTargetOperationRequest {
                installation_id: InstallationId::parse("11111111-1111-4111-8111-111111111111")?,
                spec: TargetOperationSpec::HealthProbe,
                idempotency_key: Some("local-health-1".to_string()),
                expires_in_seconds: Some(120),
            },
        )
        .await?;

        assert_eq!(requested.status, TargetOperationStatusKind::Requested);
        assert_eq!(requested.execution_id, None);
        let driver = CountingLocalOperationDriver::default();
        let completed =
            drive_local_operation_with_driver(&state, &requested.operation_id, &driver).await?;
        assert_eq!(completed.status, TargetOperationStatusKind::Succeeded);
        assert_eq!(completed.revision, 4);
        assert_eq!(driver.executions(), 1);
        assert_eq!(registry.projection_invocations(), 1);
        assert_eq!(
            completed
                .receipt
                .as_ref()
                .and_then(|receipt| receipt.output.get("healthy"))
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            drive_local_operation_with_driver(&state, &requested.operation_id, &driver).await?,
            completed
        );
        assert_eq!(driver.executions(), 1);
        assert_eq!(registry.projection_invocations(), 1);

        let restored = TargetAgentRegistry::default();
        assert_eq!(
            sync_target_operation_journal(store.as_ref(), &restored).await?,
            6
        );
        assert_eq!(restored.operation(&requested.operation_id), Some(completed));
        Ok(())
    }

    #[tokio::test]
    async fn terminal_local_replay_after_owner_release_is_zero_journal() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let objects = Arc::new(plurora_runtime::InMemoryObjectStore::default());
        let owner_a_registry = Arc::new(TargetAgentRegistry::default());
        let owner_a_state = test_state(store.clone(), objects.clone(), owner_a_registry.clone())?;
        let (owner_a_lease, owner_a_fence) = test_operation_authority(
            store.clone(),
            owner_a_state.development.clone(),
            owner_a_registry.clone(),
        )
        .await?;
        let target = owner_a_state
            .runtime
            .config()
            .target_registry
            .status("local")
            .await
            .context("local target missing")?;
        let authority_key = owner_a_registry
            .operation_authority_key("local", target.lease_epoch)
            .context("local target authority key missing")?;
        let requested = create_operation_record(
            store.as_ref(),
            owner_a_registry.as_ref(),
            &owner_a_lease,
            &owner_a_fence,
            &authority_key,
            target,
            CreateTargetOperationRequest {
                installation_id: InstallationId::parse("11111111-1111-4111-8111-111111111111")?,
                spec: TargetOperationSpec::HealthProbe,
                idempotency_key: Some("local-terminal-owner-replay".to_string()),
                expires_in_seconds: Some(120),
            },
        )
        .await?;
        let owner_a_driver = CountingLocalOperationDriver::default();
        let completed = drive_local_operation_with_driver(
            &owner_a_state,
            &requested.operation_id,
            &owner_a_driver,
        )
        .await?;
        assert_eq!(completed.status, TargetOperationStatusKind::Succeeded);
        assert_eq!(owner_a_driver.executions(), 1);
        assert_eq!(owner_a_registry.projection_invocations(), 1);

        let journal_before = store
            .list_session_range(&OPERATION_JOURNAL_SESSION.to_string(), None, None)
            .await?;
        let snapshot_count_before = journal_before
            .iter()
            .filter(|event| event.kind == OPERATION_JOURNAL_EVENT)
            .count();
        let host_fence_count_before = journal_before
            .iter()
            .filter(|event| event.kind == OPERATION_HOST_FENCE_EVENT)
            .count();
        assert_eq!(journal_before.len(), 6);
        assert_eq!(snapshot_count_before, 4);
        assert_eq!(host_fence_count_before, 1);
        crate::release_development_host_lease(store.clone(), &owner_a_lease).await?;

        let owner_b_registry = Arc::new(TargetAgentRegistry::default());
        let owner_b_state = test_state(store.clone(), objects, owner_b_registry.clone())?;
        let owner_b_driver = CountingLocalOperationDriver::default();
        let replayed = drive_local_operation_with_driver(
            &owner_b_state,
            &requested.operation_id,
            &owner_b_driver,
        )
        .await?;
        assert_eq!(replayed, completed);
        assert_eq!(owner_b_driver.executions(), 0);
        assert_eq!(owner_b_registry.projection_invocations(), 0);
        assert!(owner_b_registry
            .operations
            .lock()
            .expect("target operation state lock poisoned")
            .local_execution_locks
            .is_empty());

        let journal_after = store
            .list_session_range(&OPERATION_JOURNAL_SESSION.to_string(), None, None)
            .await?;
        assert_eq!(journal_after.len(), journal_before.len());
        assert_eq!(
            journal_after
                .iter()
                .filter(|event| event.kind == OPERATION_JOURNAL_EVENT)
                .count(),
            snapshot_count_before
        );
        assert_eq!(
            journal_after
                .iter()
                .filter(|event| event.kind == OPERATION_HOST_FENCE_EVENT)
                .count(),
            host_fence_count_before
        );
        Ok(())
    }

    #[tokio::test]
    async fn local_executor_binding_is_state_exact_and_accept_binds_the_claim() -> anyhow::Result<()>
    {
        let store = Arc::new(InMemoryEventStore::default());
        let objects = Arc::new(plurora_runtime::InMemoryObjectStore::default());
        let registry = Arc::new(TargetAgentRegistry::default());
        let state = test_state(store.clone(), objects, registry.clone())?;
        let (lease, fence) =
            test_operation_authority(store.clone(), state.development.clone(), registry.clone())
                .await?;
        let target = state
            .runtime
            .config()
            .target_registry
            .status("local")
            .await
            .expect("local target exists");
        let authority_key = registry
            .operation_authority_key("local", target.lease_epoch)
            .expect("local target authority key exists");
        let requested = create_operation_record(
            store.as_ref(),
            registry.as_ref(),
            &lease,
            &fence,
            &authority_key,
            target,
            CreateTargetOperationRequest {
                installation_id: InstallationId::parse("11111111-1111-4111-8111-111111111111")?,
                spec: TargetOperationSpec::HealthProbe,
                idempotency_key: Some("local-binding-matrix".to_string()),
                expires_in_seconds: Some(120),
            },
        )
        .await?;
        let executor_claim = claim_local_operation(
            store.as_ref(),
            registry.as_ref(),
            &lease,
            &fence,
            &requested.operation_id,
        )
        .await?
        .context("executor claim was not created")?;
        let execution_fence = LocalOperationExecutionFence {
            host_lease: lease,
            host_fence: fence,
            executor_claim: executor_claim.clone(),
        };

        validate_local_executor_binding(&requested, &executor_claim)?;
        let mut requested_with_owner = requested.clone();
        requested_with_owner.execution_id = Some(executor_claim.execution_id.clone());
        validate_local_executor_binding(&requested_with_owner, &executor_claim)?;
        requested_with_owner.execution_id = Some("f".repeat(32));
        assert!(validate_local_executor_binding(&requested_with_owner, &executor_claim).is_err());

        let accepted = advance_local_operation(
            &state,
            &execution_fence,
            TargetOperationStatusKind::Accepted,
        )
        .await?;
        assert!(accepted.appended);
        assert_eq!(accepted.record.status, TargetOperationStatusKind::Accepted);
        assert_eq!(
            accepted.record.execution_id.as_deref(),
            Some(executor_claim.execution_id.as_str())
        );

        for status in [
            TargetOperationStatusKind::Accepted,
            TargetOperationStatusKind::Running,
        ] {
            let mut candidate = accepted.record.clone();
            candidate.status = status;
            candidate.execution_id = None;
            assert!(validate_local_executor_binding(&candidate, &executor_claim).is_err());
            candidate.execution_id = Some("f".repeat(32));
            assert!(validate_local_executor_binding(&candidate, &executor_claim).is_err());
            candidate.execution_id = Some(executor_claim.execution_id.clone());
            validate_local_executor_binding(&candidate, &executor_claim)?;
        }

        let mut terminal = accepted.record;
        terminal.status = TargetOperationStatusKind::Failed;
        assert!(validate_local_executor_binding(&terminal, &executor_claim).is_err());
        Ok(())
    }

    #[tokio::test]
    async fn executor_takeover_fences_old_owner_and_does_not_repeat_running_effect(
    ) -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let objects = Arc::new(plurora_runtime::InMemoryObjectStore::default());
        let first_registry = Arc::new(TargetAgentRegistry::default());
        let first_state = test_state(store.clone(), objects.clone(), first_registry.clone())?;
        let (first_lease, first_host_fence) = test_operation_authority(
            store.clone(),
            first_state.development.clone(),
            first_registry.clone(),
        )
        .await?;
        let target = first_state
            .runtime
            .config()
            .target_registry
            .status("local")
            .await
            .expect("local target exists");
        let authority_key = first_registry
            .operation_authority_key("local", target.lease_epoch)
            .expect("local target authority key exists");
        let requested = create_operation_record(
            store.as_ref(),
            first_registry.as_ref(),
            &first_lease,
            &first_host_fence,
            &authority_key,
            target,
            CreateTargetOperationRequest {
                installation_id: InstallationId::parse("11111111-1111-4111-8111-111111111111")?,
                spec: TargetOperationSpec::HealthProbe,
                idempotency_key: Some("local-owner-takeover".to_string()),
                expires_in_seconds: Some(120),
            },
        )
        .await?;
        let first_claim = claim_local_operation(
            store.as_ref(),
            first_registry.as_ref(),
            &first_lease,
            &first_host_fence,
            &requested.operation_id,
        )
        .await?
        .context("first executor claim was not created")?;
        let first_execution_fence = LocalOperationExecutionFence {
            host_lease: first_lease.clone(),
            host_fence: first_host_fence.clone(),
            executor_claim: first_claim.clone(),
        };
        advance_local_operation(
            &first_state,
            &first_execution_fence,
            TargetOperationStatusKind::Accepted,
        )
        .await?;
        let running = advance_local_operation(
            &first_state,
            &first_execution_fence,
            TargetOperationStatusKind::Running,
        )
        .await?;
        assert!(running.appended);

        crate::release_development_host_lease(store.clone(), &first_lease).await?;
        let second_registry = Arc::new(TargetAgentRegistry::default());
        let second_state = test_state(store.clone(), objects, second_registry.clone())?;
        let (second_lease, second_host_fence) = test_operation_authority(
            store.clone(),
            second_state.development.clone(),
            second_registry.clone(),
        )
        .await?;
        let second_claim = claim_local_operation(
            store.as_ref(),
            second_registry.as_ref(),
            &second_lease,
            &second_host_fence,
            &requested.operation_id,
        )
        .await?
        .context("takeover executor claim was not created")?;
        assert_ne!(second_claim.owner_id, first_claim.owner_id);
        assert_eq!(second_claim.generation, first_claim.generation + 1);
        assert_eq!(second_claim.execution_id, first_claim.execution_id);
        assert!(ensure_executor_claim(
            store.as_ref(),
            second_registry.as_ref(),
            &second_lease,
            &second_host_fence,
            &first_claim,
        )
        .await
        .is_err());

        assert!(first_execution_fence
            .ensure_current(&first_state)
            .await
            .is_err());
        assert!(advance_local_operation(
            &first_state,
            &first_execution_fence,
            TargetOperationStatusKind::Accepted,
        )
        .await
        .is_err());
        assert!(complete_local_operation(
            &first_state,
            &first_execution_fence,
            TargetOperationReceipt {
                operation_id: requested.operation_id.clone(),
                target_id: "local".to_string(),
                execution_id: first_claim.execution_id.clone(),
                step_id: OPERATION_STEP_ID.to_string(),
                request_digest: requested.authority.request_digest.clone(),
                authority_digest: requested.authority.authority_digest.clone(),
                status: TargetOperationReceiptStatus::Succeeded,
                completed_at_ms: Utc::now().timestamp_millis(),
                output: json!({ "healthy": true }),
                diagnostics: Vec::new(),
            },
        )
        .await
        .is_err());

        let driver = CountingLocalOperationDriver::default();
        assert!(
            drive_local_operation_with_driver(&first_state, &requested.operation_id, &driver,)
                .await
                .is_err()
        );
        assert_eq!(driver.executions(), 0);
        let recovered =
            drive_local_operation_with_driver(&second_state, &requested.operation_id, &driver)
                .await?;
        assert_eq!(recovered.status, TargetOperationStatusKind::OutcomeUnknown);
        assert_eq!(
            recovered.receipt.as_ref().map(|receipt| receipt.status),
            Some(TargetOperationReceiptStatus::OutcomeUnknown)
        );
        assert_eq!(driver.executions(), 0);
        crate::release_development_host_lease(store, &second_lease).await?;
        Ok(())
    }

    #[tokio::test]
    async fn two_registry_takeover_stops_an_inflight_owner_before_its_next_effect_and_append(
    ) -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let objects = Arc::new(plurora_runtime::InMemoryObjectStore::default());
        let first_registry = Arc::new(TargetAgentRegistry::default());
        let first_state = test_state(store.clone(), objects.clone(), first_registry.clone())?;
        let (first_lease, first_fence) = test_operation_authority(
            store.clone(),
            first_state.development.clone(),
            first_registry.clone(),
        )
        .await?;
        let target = first_state
            .runtime
            .config()
            .target_registry
            .status("local")
            .await
            .context("local target missing")?;
        let authority_key = first_registry
            .operation_authority_key("local", target.lease_epoch)
            .context("local authority key missing")?;
        let requested = create_operation_record(
            store.as_ref(),
            first_registry.as_ref(),
            &first_lease,
            &first_fence,
            &authority_key,
            target,
            CreateTargetOperationRequest {
                installation_id: InstallationId::parse("11111111-1111-4111-8111-111111111111")?,
                spec: TargetOperationSpec::HealthProbe,
                idempotency_key: Some("barrier-owner-takeover".to_string()),
                expires_in_seconds: Some(120),
            },
        )
        .await?;

        let driver = Arc::new(BarrierLocalOperationDriver::default());
        let first_task_state = first_state.clone();
        let first_task_driver = driver.clone();
        let operation_id = requested.operation_id.clone();
        let first_task = tokio::spawn(async move {
            drive_local_operation_with_driver(
                &first_task_state,
                &operation_id,
                first_task_driver.as_ref(),
            )
            .await
        });
        driver.entered_effect.notified().await;
        assert_eq!(driver.effects(), 1);

        crate::release_development_host_lease(store.clone(), &first_lease).await?;
        let second_registry = Arc::new(TargetAgentRegistry::default());
        let second_state = test_state(store.clone(), objects, second_registry.clone())?;
        let second_lease =
            crate::acquire_development_host_lease(store.clone(), second_state.development.clone())
                .await?;
        synchronize_target_operations_as_owner(&second_state).await?;
        let second_claim = claim_local_operation(
            store.as_ref(),
            second_registry.as_ref(),
            &second_lease,
            &second_registry
                .operation_host_fence()
                .context("second Host fence missing")?,
            &requested.operation_id,
        )
        .await?
        .context("second executor claim missing")?;
        assert_eq!(
            second_claim.execution_id,
            first_registry
                .operation(&requested.operation_id)
                .and_then(|operation| operation.execution_id)
                .context("first execution identity missing")?
        );

        let second_driver = CountingLocalOperationDriver::default();
        let recovered = drive_local_operation_with_driver(
            &second_state,
            &requested.operation_id,
            &second_driver,
        )
        .await?;
        assert_eq!(recovered.status, TargetOperationStatusKind::OutcomeUnknown);
        assert_eq!(second_driver.executions(), 0);

        driver.release_effect.notify_one();
        assert!(first_task.await?.is_err());
        assert_eq!(driver.effects(), 1);
        sync_target_operation_journal(store.as_ref(), first_registry.as_ref()).await?;
        assert_eq!(
            first_registry
                .operation(&requested.operation_id)
                .context("final operation missing")?,
            recovered
        );
        crate::release_development_host_lease(store, &second_lease).await?;
        Ok(())
    }

    #[tokio::test]
    async fn hydration_marks_interrupted_local_effect_outcome_unknown() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let objects = Arc::new(plurora_runtime::InMemoryObjectStore::default());
        let registry = Arc::new(TargetAgentRegistry::default());
        let state = test_state(store.clone(), objects, registry.clone())?;
        let (lease, fence) =
            test_operation_authority(store.clone(), state.development.clone(), registry.clone())
                .await?;
        let target = state
            .runtime
            .config()
            .target_registry
            .status("local")
            .await
            .expect("local target exists");
        let authority_key = registry
            .operation_authority_key("local", target.lease_epoch)
            .expect("local target authority key exists");
        let requested = create_operation_record(
            store.as_ref(),
            registry.as_ref(),
            &lease,
            &fence,
            &authority_key,
            target,
            CreateTargetOperationRequest {
                installation_id: InstallationId::parse("11111111-1111-4111-8111-111111111111")?,
                spec: TargetOperationSpec::HealthProbe,
                idempotency_key: Some("interrupted-local-health".to_string()),
                expires_in_seconds: Some(120),
            },
        )
        .await?;
        let executor_claim = claim_local_operation(
            store.as_ref(),
            registry.as_ref(),
            &lease,
            &fence,
            &requested.operation_id,
        )
        .await?
        .context("executor claim was not created")?;
        let mut accepted = requested.clone();
        accepted.revision = 2;
        accepted.status = TargetOperationStatusKind::Accepted;
        accepted.execution_id = Some(executor_claim.execution_id.clone());
        accepted.updated_at_ms = accepted.updated_at_ms.saturating_add(1);
        append_target_operation_snapshot(
            store.as_ref(),
            registry.as_ref(),
            &lease,
            &fence,
            Some(&executor_claim),
            registry.operation_next_sequence(),
            &accepted,
        )
        .await?
        .context("accepted snapshot was not appended")?;
        let mut running = accepted.clone();
        running.revision = 3;
        running.status = TargetOperationStatusKind::Running;
        running.updated_at_ms = running.updated_at_ms.saturating_add(1);
        append_target_operation_snapshot(
            store.as_ref(),
            registry.as_ref(),
            &lease,
            &fence,
            Some(&executor_claim),
            registry.operation_next_sequence(),
            &running,
        )
        .await?
        .context("running snapshot was not appended")?;
        assert_eq!(accepted.status, TargetOperationStatusKind::Accepted);
        assert_eq!(running.status, TargetOperationStatusKind::Running);

        crate::release_development_host_lease(store.clone(), &lease).await?;
        let restored = Arc::new(TargetAgentRegistry::default());
        let restored_state = test_state(
            store.clone(),
            Arc::new(plurora_runtime::InMemoryObjectStore::default()),
            restored.clone(),
        )?;
        let restored_lease = crate::acquire_development_host_lease(
            store.clone(),
            restored_state.development.clone(),
        )
        .await?;
        assert_eq!(
            sync_target_operation_journal(store.as_ref(), restored.as_ref()).await?,
            5
        );
        assert_eq!(
            recover_local_operations_after_restart(&restored_state).await?,
            1
        );
        let recovered = restored
            .operation(&requested.operation_id)
            .expect("recovered operation exists");
        assert_eq!(recovered.status, TargetOperationStatusKind::OutcomeUnknown);
        assert_eq!(
            recovered.receipt.as_ref().map(|receipt| receipt.status),
            Some(TargetOperationReceiptStatus::OutcomeUnknown)
        );

        let replayed = TargetAgentRegistry::default();
        assert_eq!(
            sync_target_operation_journal(store.as_ref(), &replayed).await?,
            8
        );
        assert_eq!(replayed.operation(&requested.operation_id), Some(recovered));
        crate::release_development_host_lease(store, &restored_lease).await?;
        Ok(())
    }

    #[tokio::test]
    async fn deployment_receipt_installations_actual_port_and_route_readiness() -> anyhow::Result<()>
    {
        let store = Arc::new(InMemoryEventStore::default());
        let objects = Arc::new(plurora_runtime::InMemoryObjectStore::default());
        let installations = crate::InstallationRegistry::ephemeral(store.clone(), objects.clone())?;
        let runtime = Arc::new(plurora_runtime::Runtime::new(
            store,
            plurora_runtime::RuntimeConfig {
                object_store: objects,
                installation_control: installations.clone(),
                ..plurora_runtime::RuntimeConfig::default()
            },
        ));
        let state = AppState {
            runtime: runtime.clone(),
            static_dir: None,
            access_token: None,
            app_base_domain: None,
            build_jobs: Arc::new(crate::BuildDeployJobRegistry::default()),
            development: crate::development_registry(),
            host_access: crate::host_access_registry(),
            installations,
            target_agents: Arc::new(TargetAgentRegistry::default()),
        };
        let lease = runtime
            .config()
            .port_lease_registry
            .lease(plurora_runtime::PortLeaseRequest {
                target_id: "local".to_string(),
                port_name: "http".to_string(),
                protocol: plurora_runtime::PortProtocol::Tcp,
                requested_port: Some(40_001),
            })
            .await
            .lease;
        runtime
            .config()
            .proxy_route_registry
            .register(plurora_runtime::ProxyRouteRegisterRequest {
                route_id: Some("route-1".to_string()),
                upstream: plurora_runtime::ProxyRouteUpstream {
                    port_lease_id: lease.id.clone(),
                    port_name: "http".to_string(),
                },
                protocol: plurora_runtime::ProxyProtocol::Http,
                access: plurora_runtime::ProxyRouteAccess::HostAuthenticated,
            })
            .await;
        runtime
            .config()
            .proxy_route_registry
            .register(plurora_runtime::ProxyRouteRegisterRequest {
                route_id: Some("public-route".to_string()),
                upstream: plurora_runtime::ProxyRouteUpstream {
                    port_lease_id: lease.id.clone(),
                    port_name: "http".to_string(),
                },
                protocol: plurora_runtime::ProxyProtocol::Http,
                access: plurora_runtime::ProxyRouteAccess::Public,
            })
            .await;
        let deployment = TargetDeploymentDescriptor {
            deployment: TargetDeploymentRef {
                deployment_id: "deployment-1".to_string(),
                route_id: "route-1".to_string(),
                port_lease_id: lease.id.clone(),
            },
            port_name: "http".to_string(),
            image: "registry.example/app:latest".to_string(),
            container_port: 8080,
            requested_host_port: None,
            pull_if_missing: false,
            health_path: None,
        };
        let create = CreateTargetOperationRequest {
            installation_id: InstallationId::parse("11111111-1111-4111-8111-111111111111")?,
            spec: TargetOperationSpec::DeploymentApply {
                deployment: deployment.clone(),
            },
            idempotency_key: None,
            expires_in_seconds: Some(120),
        };
        assert!(validate_deployment_topology(&state, "local", &create)
            .await
            .is_ok());
        assert!(validate_deployment_topology(&state, "remote-1", &create)
            .await
            .is_err());

        let now_ms = Utc::now().timestamp_millis();
        let mut operation = TargetOperationRecord {
            operation_id: "operation-1".to_string(),
            target_id: "local".to_string(),
            installation_id: create.installation_id,
            revision: 4,
            status: TargetOperationStatusKind::Succeeded,
            execution_id: Some("a".repeat(32)),
            spec: create.spec,
            authority: TargetOperationAuthority {
                target_id: "local".to_string(),
                operation_id: "operation-1".to_string(),
                step_id: OPERATION_STEP_ID.to_string(),
                installation_id: InstallationId::parse("11111111-1111-4111-8111-111111111111")?,
                effect: TargetOperationEffect::DeploymentApply,
                artifact_digests: Vec::new(),
                lease_epoch: 1,
                policy_epoch: 1,
                issued_at_ms: now_ms,
                expires_at_ms: now_ms + 120_000,
                nonce: "nonce".to_string(),
                request_digest: format!("sha256:{}", "a".repeat(64)),
                authority_digest: format!("sha256:{}", "b".repeat(64)),
            },
            idempotency_key: None,
            receipt: Some(TargetOperationReceipt {
                operation_id: "operation-1".to_string(),
                target_id: "local".to_string(),
                execution_id: "a".repeat(32),
                step_id: OPERATION_STEP_ID.to_string(),
                request_digest: format!("sha256:{}", "a".repeat(64)),
                authority_digest: format!("sha256:{}", "b".repeat(64)),
                status: TargetOperationReceiptStatus::Succeeded,
                completed_at_ms: now_ms,
                output: json!({
                    "bind_host": "127.0.0.1",
                    "host_port": 49_152,
                    "running": true
                }),
                diagnostics: Vec::new(),
            }),
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
        };
        assert!(installation_deployment_operation(&state, &operation).await?);
        assert_eq!(
            runtime
                .config()
                .port_lease_registry
                .status(&lease.id)
                .await
                .map(|lease| (lease.port, lease.status)),
            Some((49_152, plurora_runtime::PortLeaseStatusKind::Active))
        );
        assert!(runtime
            .config()
            .proxy_route_registry
            .status("route-1")
            .await
            .is_some_and(|route| route.ready));
        assert!(runtime
            .config()
            .proxy_route_registry
            .status("public-route")
            .await
            .is_some_and(|route| route.ready));

        operation.spec = TargetOperationSpec::DeploymentDrain {
            deployment: deployment.deployment,
            grace_seconds: 10,
        };
        operation.authority.effect = TargetOperationEffect::DeploymentDrain;
        assert!(installation_deployment_operation(&state, &operation).await?);
        assert_eq!(
            runtime
                .config()
                .port_lease_registry
                .status(&lease.id)
                .await
                .map(|lease| lease.status),
            Some(plurora_runtime::PortLeaseStatusKind::Reserved)
        );
        assert!(runtime
            .config()
            .proxy_route_registry
            .status("route-1")
            .await
            .is_some_and(|route| {
                !route.ready && route.status == plurora_runtime::ProxyRouteStatusKind::Stale
            }));
        assert!(runtime
            .config()
            .proxy_route_registry
            .status("public-route")
            .await
            .is_some_and(|route| {
                !route.ready && route.status == plurora_runtime::ProxyRouteStatusKind::Stale
            }));
        Ok(())
    }

    #[tokio::test]
    async fn terminal_receipt_replay_has_zero_projection_effect_and_hydration_uses_journal_order(
    ) -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let objects = Arc::new(plurora_runtime::InMemoryObjectStore::default());
        let registry = Arc::new(TargetAgentRegistry::default());
        let state = test_state(store.clone(), objects, registry.clone())?;
        let (enrollment, enrollment_token) = create_enrollment_record(
            store.as_ref(),
            registry.as_ref(),
            state.runtime.config().target_registry.as_ref(),
            "remote-1".to_string(),
            CreateTargetEnrollmentRequest {
                display_name: "remote deployment target".to_string(),
                reachability: ExecutionTargetReachability::ReverseTunnel,
                allowed_capabilities: vec![ExecutionTargetCapability::Deployment],
                labels: BTreeMap::new(),
                expires_in_seconds: Some(120),
            },
        )
        .await?;
        let (target, agent_credential) = claim_enrollment_record(
            store.as_ref(),
            registry.as_ref(),
            state.runtime.config().target_registry.as_ref(),
            ClaimTargetEnrollmentRequest {
                enrollment_token,
                protocol_versions: vec![PROTOCOL_VERSION.to_string()],
                declared_capabilities: enrollment.allowed_capabilities,
            },
        )
        .await?;
        let (host_lease, host_fence) =
            test_operation_authority(store.clone(), state.development.clone(), registry.clone())
                .await?;
        let installation_id = InstallationId::parse("11111111-1111-4111-8111-111111111111")?;
        let lease = state
            .runtime
            .config()
            .port_lease_registry
            .lease(plurora_runtime::PortLeaseRequest {
                target_id: target.id.clone(),
                port_name: "http".to_string(),
                protocol: plurora_runtime::PortProtocol::Tcp,
                requested_port: Some(40_001),
            })
            .await
            .lease;
        state
            .runtime
            .config()
            .proxy_route_registry
            .register(plurora_runtime::ProxyRouteRegisterRequest {
                route_id: Some("route-replay".to_string()),
                upstream: plurora_runtime::ProxyRouteUpstream {
                    port_lease_id: lease.id.clone(),
                    port_name: "http".to_string(),
                },
                protocol: plurora_runtime::ProxyProtocol::Http,
                access: plurora_runtime::ProxyRouteAccess::HostAuthenticated,
            })
            .await;
        let deployment = TargetDeploymentDescriptor {
            deployment: TargetDeploymentRef {
                deployment_id: "deployment-replay".to_string(),
                route_id: "route-replay".to_string(),
                port_lease_id: lease.id.clone(),
            },
            port_name: "http".to_string(),
            image: "registry.example/app:latest".to_string(),
            container_port: 8080,
            requested_host_port: None,
            pull_if_missing: false,
            health_path: None,
        };
        let authority_key = registry
            .operation_authority_key(&target.id, target.lease_epoch)
            .context("remote target operation authority key missing")?;

        let mut apply = create_operation_record(
            store.as_ref(),
            registry.as_ref(),
            &host_lease,
            &host_fence,
            &authority_key,
            target.clone(),
            CreateTargetOperationRequest {
                installation_id: installation_id.clone(),
                spec: TargetOperationSpec::DeploymentApply {
                    deployment: deployment.clone(),
                },
                idempotency_key: Some("projection-apply".to_string()),
                expires_in_seconds: Some(120),
            },
        )
        .await?;
        let execution_id = "a".repeat(32);
        for status in [
            TargetOperationStatusKind::Accepted,
            TargetOperationStatusKind::Running,
        ] {
            apply.revision = apply.revision.saturating_add(1);
            apply.status = status;
            apply.execution_id = Some(execution_id.clone());
            apply.updated_at_ms = apply.updated_at_ms.saturating_add(1);
            append_target_operation_snapshot(
                store.as_ref(),
                registry.as_ref(),
                &host_lease,
                &host_fence,
                None,
                registry.operation_next_sequence(),
                &apply,
            )
            .await?
            .context("remote apply progress snapshot lost")?;
        }
        let apply_receipt = TargetOperationReceipt {
            operation_id: apply.operation_id.clone(),
            target_id: target.id.clone(),
            execution_id: execution_id.clone(),
            step_id: OPERATION_STEP_ID.to_string(),
            request_digest: apply.authority.request_digest.clone(),
            authority_digest: apply.authority.authority_digest.clone(),
            status: TargetOperationReceiptStatus::Succeeded,
            completed_at_ms: apply.updated_at_ms.saturating_add(1),
            output: json!({
                "bind_host": "127.0.0.1",
                "host_port": 49_152,
                "running": true
            }),
            diagnostics: Vec::new(),
        };
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("PluroraTarget {agent_credential}"))?,
        );
        let applied = match complete_operation::<InMemoryEventStore>(
            State(state.clone()),
            headers.clone(),
            Path(apply.operation_id.clone()),
            Json(apply_receipt.clone()),
        )
        .await
        {
            Ok(Json(record)) => record,
            Err(error) => return Err(explain_service_error(error).await),
        };
        assert_eq!(applied.status, TargetOperationStatusKind::Succeeded);
        assert_eq!(registry.projection_invocations(), 1);

        let mut stop = create_operation_record(
            store.as_ref(),
            registry.as_ref(),
            &host_lease,
            &host_fence,
            &authority_key,
            target.clone(),
            CreateTargetOperationRequest {
                installation_id,
                spec: TargetOperationSpec::DeploymentStop {
                    deployment: deployment.deployment.clone(),
                    grace_seconds: 0,
                    force_remove: true,
                },
                idempotency_key: Some("projection-stop".to_string()),
                expires_in_seconds: Some(120),
            },
        )
        .await?;
        let stop_execution_id = "b".repeat(32);
        for status in [
            TargetOperationStatusKind::Accepted,
            TargetOperationStatusKind::Running,
        ] {
            stop.revision = stop.revision.saturating_add(1);
            stop.status = status;
            stop.execution_id = Some(stop_execution_id.clone());
            stop.updated_at_ms = stop.updated_at_ms.saturating_add(1);
            append_target_operation_snapshot(
                store.as_ref(),
                registry.as_ref(),
                &host_lease,
                &host_fence,
                None,
                registry.operation_next_sequence(),
                &stop,
            )
            .await?
            .context("remote stop progress snapshot lost")?;
        }
        let stop_receipt = TargetOperationReceipt {
            operation_id: stop.operation_id.clone(),
            target_id: stop.target_id.clone(),
            execution_id: stop_execution_id,
            step_id: OPERATION_STEP_ID.to_string(),
            request_digest: stop.authority.request_digest.clone(),
            authority_digest: stop.authority.authority_digest.clone(),
            status: TargetOperationReceiptStatus::Succeeded,
            completed_at_ms: stop.updated_at_ms.saturating_add(1),
            output: json!({ "stopped": true, "removed": true }),
            diagnostics: Vec::new(),
        };
        let Json(stopped) = match complete_operation::<InMemoryEventStore>(
            State(state.clone()),
            headers.clone(),
            Path(stop.operation_id.clone()),
            Json(stop_receipt),
        )
        .await
        {
            Ok(record) => record,
            Err(error) => return Err(explain_service_error(error).await),
        };
        assert_eq!(stopped.status, TargetOperationStatusKind::Succeeded);
        assert_eq!(registry.projection_invocations(), 2);
        assert!(state
            .runtime
            .config()
            .proxy_route_registry
            .status("route-replay")
            .await
            .is_some_and(|route| {
                !route.ready && route.status == plurora_runtime::ProxyRouteStatusKind::Stale
            }));

        let journal_before = store
            .list_session_range(&OPERATION_JOURNAL_SESSION.to_string(), None, None)
            .await?;
        let snapshot_count_before = journal_before
            .iter()
            .filter(|event| event.kind == OPERATION_JOURNAL_EVENT)
            .count();
        let host_fence_count_before = journal_before
            .iter()
            .filter(|event| event.kind == OPERATION_HOST_FENCE_EVENT)
            .count();
        assert_eq!(journal_before.len(), 9);
        assert_eq!(snapshot_count_before, 8);
        assert_eq!(host_fence_count_before, 1);
        let lease_before = state
            .runtime
            .config()
            .port_lease_registry
            .status(&lease.id)
            .await
            .map(|lease| (lease.port, lease.status));
        let route_before = state
            .runtime
            .config()
            .proxy_route_registry
            .status("route-replay")
            .await
            .map(|route| {
                (
                    route.status,
                    route.ready,
                    route.upstream.port_lease_id,
                    route.upstream.port_name,
                )
            });
        crate::release_development_host_lease(store.clone(), &host_lease).await?;

        let replay_registry = Arc::new(TargetAgentRegistry::default());
        let replay_state = AppState {
            runtime: state.runtime.clone(),
            static_dir: None,
            access_token: None,
            app_base_domain: None,
            build_jobs: state.build_jobs.clone(),
            development: crate::development_registry(),
            host_access: state.host_access.clone(),
            installations: state.installations.clone(),
            target_agents: replay_registry.clone(),
        };
        let unauthenticated = complete_operation::<InMemoryEventStore>(
            State(replay_state.clone()),
            HeaderMap::new(),
            Path(apply.operation_id.clone()),
            Json(apply_receipt.clone()),
        )
        .await
        .expect_err("terminal operation must not be visible without the current agent credential")
        .into_response();
        assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

        let Json(replayed) = match complete_operation::<InMemoryEventStore>(
            State(replay_state.clone()),
            headers.clone(),
            Path(apply.operation_id.clone()),
            Json(apply_receipt.clone()),
        )
        .await
        {
            Ok(record) => record,
            Err(error) => return Err(explain_service_error(error).await),
        };
        assert_eq!(replayed, applied);
        assert_eq!(replay_registry.projection_invocations(), 0);

        let mut different_receipt = apply_receipt;
        different_receipt.diagnostics = vec!["different terminal receipt".to_string()];
        let different = complete_operation::<InMemoryEventStore>(
            State(replay_state),
            headers,
            Path(apply.operation_id.clone()),
            Json(different_receipt),
        )
        .await
        .expect_err("a different terminal receipt must conflict")
        .into_response();
        assert_eq!(different.status(), StatusCode::CONFLICT);
        assert_eq!(replay_registry.projection_invocations(), 0);

        let journal_after = store
            .list_session_range(&OPERATION_JOURNAL_SESSION.to_string(), None, None)
            .await?;
        assert_eq!(journal_after.len(), journal_before.len());
        assert_eq!(
            journal_after
                .iter()
                .filter(|event| event.kind == OPERATION_JOURNAL_EVENT)
                .count(),
            snapshot_count_before
        );
        assert_eq!(
            journal_after
                .iter()
                .filter(|event| event.kind == OPERATION_HOST_FENCE_EVENT)
                .count(),
            host_fence_count_before
        );
        assert_eq!(
            state
                .runtime
                .config()
                .port_lease_registry
                .status(&lease.id)
                .await
                .map(|lease| (lease.port, lease.status)),
            lease_before
        );
        assert_eq!(
            state
                .runtime
                .config()
                .proxy_route_registry
                .status("route-replay")
                .await
                .map(|route| {
                    (
                        route.status,
                        route.ready,
                        route.upstream.port_lease_id,
                        route.upstream.port_name,
                    )
                }),
            route_before
        );

        state
            .runtime
            .config()
            .port_lease_registry
            .set_status(&lease.id, plurora_runtime::PortLeaseStatusKind::Active)
            .await;
        state
            .runtime
            .config()
            .proxy_route_registry
            .set_status(
                "route-replay",
                plurora_runtime::ProxyRouteStatusKind::Active,
            )
            .await;
        state
            .runtime
            .config()
            .proxy_route_registry
            .set_ready("route-replay", true)
            .await;
        let restored_registry = Arc::new(TargetAgentRegistry::default());
        sync_target_agent_journal(
            store.as_ref(),
            restored_registry.as_ref(),
            state.runtime.config().target_registry.as_ref(),
        )
        .await?;
        sync_target_operation_journal(store.as_ref(), restored_registry.as_ref()).await?;
        let restored_state = AppState {
            runtime: state.runtime.clone(),
            static_dir: None,
            access_token: None,
            app_base_domain: None,
            build_jobs: state.build_jobs.clone(),
            development: state.development.clone(),
            host_access: state.host_access.clone(),
            installations: state.installations.clone(),
            target_agents: restored_registry,
        };
        reconcile_target_deployment_projections(&restored_state, "remote-1").await?;
        assert!(restored_state
            .runtime
            .config()
            .proxy_route_registry
            .status("route-replay")
            .await
            .is_some_and(|route| {
                !route.ready && route.status == plurora_runtime::ProxyRouteStatusKind::Stale
            }));
        Ok(())
    }

    #[test]
    fn deployment_operation_binds_ownership_and_rejects_unknown_fields() -> anyhow::Result<()> {
        let installation_id = InstallationId::parse("11111111-1111-4111-8111-111111111111")?;
        let spec = TargetOperationSpec::DeploymentApply {
            deployment: TargetDeploymentDescriptor {
                deployment: TargetDeploymentRef {
                    deployment_id: "deployment-1".to_string(),
                    route_id: "route-1".to_string(),
                    port_lease_id: "lease-1".to_string(),
                },
                port_name: "http".to_string(),
                image: format!("registry.example/app@sha256:{}", "a".repeat(64)),
                container_port: 8080,
                requested_host_port: None,
                pull_if_missing: false,
                health_path: None,
            },
        };
        assert!(spec.validate().is_ok());
        assert_eq!(spec.effect(), TargetOperationEffect::DeploymentApply);
        assert!(spec.artifact_digests().is_empty());

        let mut changed = spec.clone();
        let TargetOperationSpec::DeploymentApply { deployment } = &mut changed else {
            unreachable!()
        };
        deployment.deployment.route_id = "route-2".to_string();
        assert_ne!(
            operation_request_digest("remote-1", &installation_id, &spec)?,
            operation_request_digest("remote-1", &installation_id, &changed)?
        );

        assert!(serde_json::from_value::<TargetOperationSpec>(json!({
            "kind": "deployment_stop",
            "deployment": {
                "deployment_id": "deployment-1",
                "route_id": "route-1",
                "port_lease_id": "lease-1"
            },
            "grace_seconds": 10,
            "force_remove": false,
            "command": "whoami"
        }))
        .is_err());
        let mut raw_secret_spec = spec.clone();
        let TargetOperationSpec::DeploymentApply { deployment } = &mut raw_secret_spec else {
            unreachable!()
        };
        deployment.image = "sk-Abcdefghijklmnopqrstuvwxyz123456".to_string();
        assert!(validate_create_request(
            "remote-1",
            &CreateTargetOperationRequest {
                installation_id,
                spec: raw_secret_spec,
                idempotency_key: None,
                expires_in_seconds: Some(120),
            }
        )
        .is_err());
        assert!(
            serde_json::from_value::<TargetOperationProgressRequest>(json!({
                "request_digest": format!("sha256:{}", "a".repeat(64)),
                "authority_digest": format!("sha256:{}", "b".repeat(64)),
                "execution_id": "c".repeat(32),
                "status": "running",
                "unexpected": true
            }))
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn docker_build_verifier_binds_the_exact_context_and_recipe() -> anyhow::Result<()> {
        let installation_id = InstallationId::parse("11111111-1111-4111-8111-111111111111")?;
        let workspace_id = WorkspaceId::parse("22222222-2222-4222-8222-222222222222")?;
        let spec = TargetOperationSpec::VerifierRun {
            verifier: DeclarativeVerifierDescriptor::DockerBuild {
                digest: format!("sha256:{}", "a".repeat(64)),
                expected_size_bytes: Some(1024),
                dockerfile: "docker/Dockerfile".to_string(),
                network_mode: plurora_runtime::ManagedTargetBuildNetworkMode::None,
                disposition:
                    plurora_runtime::ManagedTargetImageDisposition::RemoveAfterVerification,
                build_id: "build-1".to_string(),
                workspace_id,
                source_tree_digest: format!("sha256:{}", "b".repeat(64)),
                build_descriptor_hash: format!("sha256:{}", "c".repeat(64)),
            },
        };
        assert!(spec.validate().is_ok());
        assert_eq!(spec.effect(), TargetOperationEffect::VerifierRun);
        assert_eq!(spec.artifact_digests().len(), 1);

        let mut changed = spec.clone();
        let TargetOperationSpec::VerifierRun {
            verifier: DeclarativeVerifierDescriptor::DockerBuild { dockerfile, .. },
        } = &mut changed
        else {
            unreachable!()
        };
        *dockerfile = "Dockerfile".to_string();
        assert_ne!(
            operation_request_digest("remote-1", &installation_id, &spec)?,
            operation_request_digest("remote-1", &installation_id, &changed)?
        );
        let mut changed_disposition = spec.clone();
        let TargetOperationSpec::VerifierRun {
            verifier: DeclarativeVerifierDescriptor::DockerBuild { disposition, .. },
        } = &mut changed_disposition
        else {
            unreachable!()
        };
        *disposition = plurora_runtime::ManagedTargetImageDisposition::RetainForDeployment;
        assert_ne!(
            operation_request_digest("remote-1", &installation_id, &spec)?,
            operation_request_digest("remote-1", &installation_id, &changed_disposition)?
        );
        let serialized = serde_json::to_value(&spec)?;
        let mut missing_disposition = serialized.clone();
        missing_disposition["verifier"]
            .as_object_mut()
            .expect("verifier object")
            .remove("disposition");
        assert!(serde_json::from_value::<TargetOperationSpec>(missing_disposition).is_err());
        let mut unknown_disposition = serialized;
        unknown_disposition["verifier"]["disposition"] = json!("delete_maybe");
        assert!(serde_json::from_value::<TargetOperationSpec>(unknown_disposition).is_err());

        let now_ms = Utc::now().timestamp_millis();
        let operation = TargetOperationRecord {
            operation_id: "operation-docker-build".to_string(),
            target_id: "remote-1".to_string(),
            installation_id: installation_id.clone(),
            revision: 4,
            status: TargetOperationStatusKind::Succeeded,
            execution_id: Some("d".repeat(32)),
            spec: spec.clone(),
            authority: TargetOperationAuthority {
                target_id: "remote-1".to_string(),
                operation_id: "operation-docker-build".to_string(),
                step_id: OPERATION_STEP_ID.to_string(),
                installation_id: installation_id.clone(),
                effect: TargetOperationEffect::VerifierRun,
                artifact_digests: spec.artifact_digests(),
                lease_epoch: 7,
                policy_epoch: 11,
                issued_at_ms: now_ms,
                expires_at_ms: now_ms + 120_000,
                nonce: "nonce".to_string(),
                request_digest: format!("sha256:{}", "d".repeat(64)),
                authority_digest: format!("sha256:{}", "e".repeat(64)),
            },
            idempotency_key: Some("docker-build-1".to_string()),
            receipt: None,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
        };
        let output = serde_json::to_value(plurora_runtime::ManagedTargetImageBuildReceipt {
            target_id: "remote-1".to_string(),
            image: format!("plurora/{}:build-1", installation_id.as_str()),
            image_id: format!("sha256:{}", "f".repeat(64)),
            installation_id,
            workspace_id: WorkspaceId::parse("22222222-2222-4222-8222-222222222222")?,
            build_id: "build-1".to_string(),
            dockerfile: "docker/Dockerfile".to_string(),
            network_mode: plurora_runtime::ManagedTargetBuildNetworkMode::None,
            disposition: plurora_runtime::ManagedTargetImageDisposition::RemoveAfterVerification,
            context_digest: format!("sha256:{}", "a".repeat(64)),
            source_tree_digest: format!("sha256:{}", "b".repeat(64)),
            build_descriptor_hash: format!("sha256:{}", "c".repeat(64)),
            image_removed: true,
            image_retained: false,
        })?;
        validate_succeeded_operation_output(&operation, &output)?;
        let mut retained = output.clone();
        retained["image_removed"] = json!(false);
        retained["image_retained"] = json!(true);
        assert!(validate_succeeded_operation_output(&operation, &retained).is_err());
        let mut wrong_target = output;
        wrong_target["target_id"] = json!("remote-2");
        assert!(validate_succeeded_operation_output(&operation, &wrong_target).is_err());
        Ok(())
    }
}
