use std::sync::{Arc, Weak};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use plurora_core::{
    validate_sha256, ArtifactDescriptor, ComponentBoundaryClaims, ComponentClaimStatus,
    ComponentTrustClass, PackageId, PackagedProtocolDescriptor, SessionId,
};
use plurora_work::{
    validate_artifact_descriptor, AcquisitionRecord, AvailabilityPolicy, BindingId, BindingPhase,
    ExposureId, ExposureRecord, ExposureStatus, InstallationId, NodeId, PortDescriptor,
    PortEndpoint, PortId, PortRole, ResourceSelector, RunId, SelectedTransport, WorkId,
    MAX_PROVIDER_CANDIDATES,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BindingDecisionStatus {
    Selected,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BindingEffectiveStatus {
    Detached,
    Active,
    Broken { reason_code: String },
}

const EXPOSURE_MANAGE_ACTION: &str = "exposure.manage";
const BINDING_MANAGE_ACTION: &str = "binding.manage";

/// Exact Run identity established by the Host. The three fields are one
/// optional unit: Launch-time selection can omit the entire value, while a
/// Runtime selection must carry all of it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunRevisionPin {
    pub run_id: RunId,
    pub run_revision: u64,
    pub context_id: String,
}

impl RunRevisionPin {
    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.run_revision > 0 && !self.context_id.trim().is_empty(),
            "Run revision pin requires a positive revision and non-empty Host context id"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstallationRevisionPin {
    pub installation_id: InstallationId,
    pub installation_revision: u64,
    pub work_revision: ArtifactDescriptor,
    pub assembly_lock: ArtifactDescriptor,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ResolvedPortPin {
    pub root_port: PortId,
    pub node_path: Vec<NodeId>,
    pub leaf_port: PortEndpoint,
    pub canonical_contract_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ComponentPin {
    pub package_id: PackageId,
    pub component_id: String,
    pub node_path: Vec<NodeId>,
    pub component_artifact: ArtifactDescriptor,
    pub behavior_digest: String,
    pub trust_class: ComponentTrustClass,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapabilityPin {
    pub capability_id: String,
    pub capability_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BindingEndpointPin {
    pub installation: InstallationRevisionPin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<RunRevisionPin>,
    pub port: ResolvedPortPin,
    pub component: ComponentPin,
}

/// Verified, public Work identity shown before a Binding decision. Descriptive
/// text is disclosure, never preference or execution authority.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BindingWorkDisclosure {
    pub work_id: WorkId,
    pub title: String,
}

/// Verified, public Installation origin shown before a Binding decision. It
/// intentionally excludes state bindings, secret policy, and host-local paths.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BindingInstallationDisclosure {
    pub installation_id: InstallationId,
    pub installation_revision: u64,
    pub display_name: String,
    pub source: AcquisitionRecord,
}

/// Host-verified Component identity and evidence. This is deliberately narrower
/// than ComponentDescriptor so annotations, surfaces, and content roots cannot
/// become an accidental disclosure channel.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BindingComponentDisclosure {
    pub package_id: PackageId,
    pub component_id: String,
    pub version: String,
    pub entry_kind: String,
    pub trust_class: ComponentTrustClass,
    pub claim_status: ComponentClaimStatus,
    pub enforced_boundaries: ComponentBoundaryClaims,
    pub component_artifact: ArtifactDescriptor,
    pub behavior: ArtifactDescriptor,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protocol_implementations: Vec<PackagedProtocolDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BindingCandidate {
    /// Digest of every canonical field below, including verified descriptive
    /// disclosure. Selection uses this digest, never an order-sensitive digest
    /// of the whole candidate set. Sorting ignores descriptive and publisher
    /// fields, but any disclosure drift still makes a prior decision stale.
    pub candidate_digest: String,
    pub exposure: ExposureView,
    pub consumer: BindingEndpointPin,
    pub consumer_port: PortDescriptor,
    pub provider: BindingEndpointPin,
    pub provider_port: PortDescriptor,
    pub provider_work: BindingWorkDisclosure,
    pub provider_installation: BindingInstallationDisclosure,
    pub provider_component: BindingComponentDisclosure,
    pub capability: CapabilityPin,
    pub transport: SelectedTransport,
    pub phase: BindingPhase,
    pub availability: AvailabilityPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_expires_at: Option<DateTime<Utc>>,
}

/// Host-only result of resolving one root Assembly Port through the verified
/// Work/AssemblyLock closure and the current Runtime package/capability fabric.
/// It deliberately contains no handle, path, secret, or mutable Runtime state.
#[derive(Debug, Clone)]
pub struct PowerboxEndpointInspection {
    pub endpoint: BindingEndpointPin,
    pub descriptor: PortDescriptor,
    pub work: BindingWorkDisclosure,
    pub installation: BindingInstallationDisclosure,
    pub component: BindingComponentDisclosure,
    pub phase: BindingPhase,
    pub availability: AvailabilityPolicy,
    pub capability: Option<CapabilityPin>,
}

impl BindingCandidate {
    fn sort_key(&self) -> (&str, &str, &str) {
        (
            self.provider.installation.installation_id.as_str(),
            self.provider.port.root_port.as_str(),
            self.exposure.record.exposure_id.as_str(),
        )
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        validate_sha256(&self.candidate_digest)?;
        validate_binding_endpoint(&self.consumer)?;
        validate_binding_endpoint(&self.provider)?;
        validate_candidate_disclosure(self)?;
        validate_capability_pin(&self.capability)?;
        validate_phase_run_pin(self.phase, &self.consumer)?;
        self.transport.validate()?;
        anyhow::ensure!(
            self.effective_expires_at
                .is_none_or(|expiry| expiry > Utc::now()),
            "binding candidate is expired"
        );
        Ok(())
    }
}

/// Apply the one protocol-sized candidate guard. Overflow is an error rather
/// than truncation because truncation would silently alter the user's choice.
pub fn sort_provider_candidates(
    mut candidates: Vec<BindingCandidate>,
) -> anyhow::Result<Vec<BindingCandidate>> {
    anyhow::ensure!(
        candidates.len() <= MAX_PROVIDER_CANDIDATES,
        "work_too_complex: binding candidate set exceeds MAX_PROVIDER_CANDIDATES ({MAX_PROVIDER_CANDIDATES})"
    );
    for candidate in &candidates {
        candidate.validate()?;
    }
    candidates.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
    Ok(candidates)
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BindingSelectionRecord {
    pub binding_id: BindingId,
    pub candidate_digest: String,
    pub exposure_id: ExposureId,
    pub exposure_revision: u64,
    pub consumer: BindingEndpointPin,
    pub provider: BindingEndpointPin,
    pub capability: CapabilityPin,
    pub transport: SelectedTransport,
    pub phase: BindingPhase,
    pub availability: AvailabilityPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_expires_at: Option<DateTime<Utc>>,
    pub status: BindingDecisionStatus,
}

impl BindingSelectionRecord {
    pub fn validate_selected(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.status == BindingDecisionStatus::Selected,
            "binding is not selected"
        );
        validate_sha256(&self.candidate_digest)?;
        validate_binding_endpoint(&self.consumer)?;
        validate_binding_endpoint(&self.provider)?;
        validate_capability_pin(&self.capability)?;
        validate_phase_run_pin(self.phase, &self.consumer)?;
        anyhow::ensure!(
            self.effective_expires_at
                .is_none_or(|expiry| expiry > Utc::now()),
            "binding is expired"
        );
        self.transport.validate()?;
        Ok(())
    }
}

fn validate_binding_endpoint(endpoint: &BindingEndpointPin) -> anyhow::Result<()> {
    validate_artifact_descriptor(&endpoint.installation.work_revision)?;
    validate_artifact_descriptor(&endpoint.installation.assembly_lock)?;
    if let Some(run) = &endpoint.run {
        run.validate()?;
    }
    anyhow::ensure!(
        !endpoint.port.node_path.is_empty()
            && endpoint.port.node_path.last() == Some(&endpoint.port.leaf_port.node_id),
        "binding endpoint resolved node path does not end at its leaf Port"
    );
    validate_sha256(&endpoint.port.canonical_contract_digest)?;
    anyhow::ensure!(
        !endpoint.component.package_id.trim().is_empty()
            && !endpoint.component.component_id.trim().is_empty()
            && endpoint.component.node_path == endpoint.port.node_path,
        "binding component pin does not identify the resolved leaf node"
    );
    validate_artifact_descriptor(&endpoint.component.component_artifact)?;
    validate_sha256(&endpoint.component.behavior_digest)?;
    Ok(())
}

fn validate_phase_run_pin(
    phase: BindingPhase,
    consumer: &BindingEndpointPin,
) -> anyhow::Result<()> {
    validate_phase_run_request(phase, consumer.run.as_ref())
}

fn validate_phase_run_request(
    phase: BindingPhase,
    consumer_run: Option<&RunRevisionPin>,
) -> anyhow::Result<()> {
    match (phase, consumer_run) {
        (BindingPhase::Launch, None) => Ok(()),
        (BindingPhase::Runtime, Some(run)) => run.validate(),
        (BindingPhase::Launch, Some(_)) => {
            anyhow::bail!("Launch Binding must not carry a consumer Run pin")
        }
        (BindingPhase::Runtime, None) => {
            anyhow::bail!("Runtime Binding requires an exact consumer Run revision and context")
        }
        (BindingPhase::Authoring | BindingPhase::Installation, _) => {
            anyhow::bail!("dynamic Binding supports only Launch or Runtime phase")
        }
    }
}

fn validate_candidate_disclosure(candidate: &BindingCandidate) -> anyhow::Result<()> {
    let exposure = &candidate.exposure;
    anyhow::ensure!(exposure.revision > 0, "Exposure revision must be positive");
    exposure.record.validate()?;
    anyhow::ensure!(
        exposure.record.status == ExposureStatus::Active,
        "binding candidate Exposure is not active"
    );
    anyhow::ensure!(
        exposure
            .record
            .expires_at
            .is_none_or(|expiry| expiry > Utc::now()),
        "binding candidate Exposure is expired"
    );
    anyhow::ensure!(
        exposure.record.installation_id == candidate.provider.installation.installation_id
            && exposure.record.run_id.as_ref()
                == candidate.provider.run.as_ref().map(|run| &run.run_id)
            && exposure.record.export_port == candidate.provider.port.root_port,
        "Exposure does not identify the exact provider endpoint"
    );
    anyhow::ensure!(
        candidate.consumer_port.port_id == candidate.consumer.port.leaf_port.port_id
            && matches!(candidate.consumer_port.role, PortRole::Import { .. }),
        "consumer disclosure is not the exact resolved import Port"
    );
    anyhow::ensure!(
        candidate.provider_port.port_id == candidate.provider.port.leaf_port.port_id
            && matches!(candidate.provider_port.role, PortRole::Export { .. }),
        "provider disclosure is not the exact resolved export Port"
    );
    candidate.consumer_port.validate()?;
    candidate.provider_port.validate()?;
    anyhow::ensure!(
        candidate.provider_installation.installation_id
            == candidate.provider.installation.installation_id
            && candidate.provider_installation.installation_revision
                == candidate.provider.installation.installation_revision
            && !candidate
                .provider_installation
                .display_name
                .trim()
                .is_empty()
            && !candidate.provider_work.title.trim().is_empty(),
        "provider Work or Installation disclosure does not match the exact pin"
    );
    if let Some(source_ref) = &candidate.provider_installation.source.source_ref {
        validate_artifact_descriptor(source_ref)?;
    }
    for reference in &candidate.provider_installation.source.provenance_refs {
        validate_artifact_descriptor(reference)?;
    }
    let component = &candidate.provider_component;
    anyhow::ensure!(
        component.package_id == candidate.provider.component.package_id
            && component.component_id == candidate.provider.component.component_id
            && component.component_artifact == candidate.provider.component.component_artifact
            && component.behavior.digest == candidate.provider.component.behavior_digest
            && component.trust_class == candidate.provider.component.trust_class
            && !component.version.trim().is_empty()
            && !component.entry_kind.trim().is_empty(),
        "provider Component disclosure does not match the exact pin"
    );
    validate_artifact_descriptor(&component.component_artifact)?;
    validate_artifact_descriptor(&component.behavior)?;
    for implementation in &component.protocol_implementations {
        validate_artifact_descriptor(&implementation.artifact)?;
    }
    Ok(())
}

fn validate_capability_pin(capability: &CapabilityPin) -> anyhow::Result<()> {
    anyhow::ensure!(
        !capability.capability_id.trim().is_empty()
            && !capability.capability_version.trim().is_empty()
            && !capability
                .capability_version
                .chars()
                .any(char::is_whitespace),
        "binding capability pin requires an exact non-empty capability id and version"
    );
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExposureView {
    pub record: ExposureRecord,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct BindingView {
    pub record: BindingSelectionRecord,
    pub revision: u64,
    pub effective_status: BindingEffectiveStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BindingGap {
    pub reason_code: String,
    pub next_step: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation_id: Option<InstallationId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<NodeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_id: Option<PortId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction_model: Option<String>,
}

impl BindingGap {
    pub fn unsupported_interaction(
        installation_id: InstallationId,
        node_id: NodeId,
        port_id: PortId,
        interaction_model: impl Into<String>,
    ) -> Self {
        Self {
            reason_code: "unsupported_interaction".to_string(),
            next_step: "Install or select an adapter/provider implementing this interaction"
                .to_string(),
            installation_id: Some(installation_id),
            run_id: None,
            node_id: Some(node_id),
            port_id: Some(port_id),
            interaction_model: Some(interaction_model.into()),
        }
    }
}

/// Host-established visibility and audience context for candidate discovery.
/// The controller must filter active, compatible candidates with this context
/// before returning them to `BindingCandidatesResult::stable`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PowerboxQueryContext {
    pub unrestricted: bool,
    pub visible_resources: Vec<ResourceSelector>,
    pub audience_principals: Vec<ResourceSelector>,
}

impl PowerboxQueryContext {
    pub fn allows_resource(&self, kind: &str, id: &str) -> bool {
        self.unrestricted
            || self
                .visible_resources
                .iter()
                .any(|selector| selector.kind == kind && (selector.id == id || selector.id == "*"))
    }

    pub fn is_in_audience(&self, audience: &[ResourceSelector]) -> bool {
        self.unrestricted
            || audience.iter().any(|allowed| {
                self.audience_principals.iter().any(|caller| {
                    caller == allowed || (caller.kind == allowed.kind && caller.id == "*")
                }) || self
                    .visible_resources
                    .iter()
                    .any(|caller| caller == allowed)
            })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PowerboxAuthoritySubject {
    ExposureCreate {
        installation_id: InstallationId,
        run_id: RunId,
        export_port: PortId,
    },
    ExposureRevoke {
        installation_id: InstallationId,
        run_id: RunId,
        export_port: PortId,
        exposure_id: ExposureId,
    },
    BindingSelect {
        consumer_installation_id: InstallationId,
        consumer_run: Option<RunRevisionPin>,
        import_port: PortId,
        exposure_id: ExposureId,
    },
    BindingRevoke {
        consumer_installation_id: InstallationId,
        consumer_run: Option<RunRevisionPin>,
        import_port: PortId,
        exposure_id: ExposureId,
        binding_id: BindingId,
    },
}

impl PowerboxAuthoritySubject {
    fn action(&self) -> &'static str {
        match self {
            Self::ExposureCreate { .. } | Self::ExposureRevoke { .. } => EXPOSURE_MANAGE_ACTION,
            Self::BindingSelect { .. } | Self::BindingRevoke { .. } => BINDING_MANAGE_ACTION,
        }
    }
}

#[async_trait]
pub trait PowerboxAuthorityValidator: Send + Sync + 'static {
    async fn validate_current(
        &self,
        grant_id: &str,
        subject: &PowerboxAuthoritySubject,
    ) -> anyhow::Result<()>;
}

#[derive(Clone)]
pub struct PowerboxAuthorityRefresh(Arc<dyn PowerboxAuthorityValidator>);

impl PowerboxAuthorityRefresh {
    pub fn new(validator: Arc<dyn PowerboxAuthorityValidator>) -> Self {
        Self(validator)
    }

    pub async fn validate_current(
        &self,
        grant_id: &str,
        subject: &PowerboxAuthoritySubject,
    ) -> anyhow::Result<()> {
        self.0.validate_current(grant_id, subject).await
    }
}

/// Durable, non-credential authority facts for one Exposure or Binding. The
/// Host persists this beside the decision so restart recovery and every later
/// effect can re-check the exact grant and resource set. It contains no bearer
/// token, capability handle, or other reusable credential material.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PowerboxAuthorityBasis {
    subject: PowerboxAuthoritySubject,
    grant_reference: Option<String>,
    expires_at_ms: Option<i64>,
}

impl PowerboxAuthorityBasis {
    pub fn host(subject: PowerboxAuthoritySubject) -> Self {
        Self {
            subject,
            grant_reference: None,
            expires_at_ms: None,
        }
    }

    pub fn subject(&self) -> &PowerboxAuthoritySubject {
        &self.subject
    }

    pub fn grant_reference(&self) -> Option<&str> {
        self.grant_reference.as_deref()
    }

    pub fn expires_at_ms(&self) -> Option<i64> {
        self.expires_at_ms
    }

    pub async fn validate_current(
        &self,
        refresh: Option<&PowerboxAuthorityRefresh>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.expires_at_ms
                .is_none_or(|expiry| expiry > Utc::now().timestamp_millis()),
            "authority_denied: durable Powerbox authority basis expired"
        );
        if let Some(grant_reference) = self.grant_reference.as_deref() {
            refresh
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "authority_denied: durable Powerbox grant validator is unavailable"
                    )
                })?
                .validate_current(grant_reference, &self.subject)
                .await
                .map_err(|_| {
                    anyhow::anyhow!(
                        "authority_denied: durable Powerbox authority is no longer current"
                    )
                })?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for PowerboxAuthorityRefresh {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PowerboxAuthorityRefresh(<trusted>)")
    }
}

impl PartialEq for PowerboxAuthorityRefresh {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for PowerboxAuthorityRefresh {}

/// Unwireable proof for one exact Powerbox mutation. Durable controllers call
/// `refresh_current_for` before every append or external effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PowerboxMutationAuthority {
    subject: PowerboxAuthoritySubject,
    action: &'static str,
    grant_id: Option<String>,
    expires_at_ms: Option<i64>,
    refresh: Option<PowerboxAuthorityRefresh>,
}

impl PowerboxMutationAuthority {
    pub(crate) fn verified(
        subject: PowerboxAuthoritySubject,
        grant_id: Option<String>,
        expires_at_ms: Option<i64>,
        refresh: Option<PowerboxAuthorityRefresh>,
    ) -> anyhow::Result<Self> {
        if grant_id.is_some() {
            anyhow::ensure!(
                expires_at_ms.is_some_and(|expiry| expiry > Utc::now().timestamp_millis()),
                "authority_denied: Powerbox mutation authority is expired or unverified"
            );
        }
        Ok(Self {
            action: subject.action(),
            subject,
            grant_id,
            expires_at_ms,
            refresh,
        })
    }

    pub async fn refresh_current_for(
        &self,
        subject: &PowerboxAuthoritySubject,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            &self.subject == subject
                && self.action == subject.action()
                && self
                    .expires_at_ms
                    .is_none_or(|expiry| expiry > Utc::now().timestamp_millis()),
            "authority_denied: current exact Powerbox mutation authority is required"
        );
        if let Some(grant_id) = self.grant_id.as_deref() {
            self.refresh
                .as_ref()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "authority_denied: trusted authority refresh is required for a device Powerbox mutation"
                    )
                })?
                .validate_current(grant_id, subject)
                .await
                .map_err(|_| {
                    anyhow::anyhow!("authority_denied: Powerbox mutation authority is no longer current")
                })?;
        }
        Ok(())
    }

    pub fn durable_basis(&self) -> PowerboxAuthorityBasis {
        PowerboxAuthorityBasis {
            subject: self.subject.clone(),
            grant_reference: self.grant_id.as_deref().map(powerbox_grant_reference),
            expires_at_ms: self.expires_at_ms,
        }
    }
}

pub fn powerbox_grant_reference(grant_id: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"plurora.powerbox.authority-basis/v1\0");
    digest.update(grant_id.as_bytes());
    format!("sha256:{:x}", digest.finalize())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExposureListRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation_id: Option<InstallationId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ExposureStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExposureCreateRequest {
    pub installation_id: InstallationId,
    pub expected_installation_revision: u64,
    pub run_id: RunId,
    pub expected_run_revision: u64,
    pub export_port: PortId,
    pub audience: Vec<ResourceSelector>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    pub idempotency_key: String,
    /// Runtime-only proof minted from authenticated protocol context. Request
    /// JSON, plans, and receipts cannot construct or replay it.
    #[serde(skip)]
    #[schemars(skip)]
    #[doc(hidden)]
    pub authority: Option<PowerboxMutationAuthority>,
}

impl ExposureCreateRequest {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_binding_idempotency_key(&self.idempotency_key)?;
        anyhow::ensure!(
            !self.audience.is_empty()
                && self.audience.iter().all(|selector| {
                    !selector.kind.trim().is_empty() && !selector.id.trim().is_empty()
                }),
            "exposure create requires a non-empty exact audience"
        );
        anyhow::ensure!(
            self.expires_at.is_none_or(|expiry| expiry > Utc::now()),
            "exposure create expiry must be in the future"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExposureRevokeRequest {
    pub installation_id: InstallationId,
    pub expected_installation_revision: u64,
    pub run_id: RunId,
    pub expected_run_revision: u64,
    pub export_port: PortId,
    pub exposure_id: ExposureId,
    pub expected_exposure_revision: u64,
    pub idempotency_key: String,
    #[serde(skip)]
    #[schemars(skip)]
    #[doc(hidden)]
    pub authority: Option<PowerboxMutationAuthority>,
}

impl ExposureRevokeRequest {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_binding_idempotency_key(&self.idempotency_key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExposureMutationResult {
    pub exposure: ExposureView,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_binding_ids: Vec<BindingId>,
    pub idempotent: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BindingListRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumer_installation_id: Option<InstallationId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<BindingDecisionStatus>,
    /// Trusted caller audience facts injected by the protocol handler. Binding
    /// views contain provider pins, so consumer observe authority alone is not
    /// sufficient to disclose them.
    #[serde(skip)]
    #[schemars(skip)]
    #[doc(hidden)]
    pub query: Option<PowerboxQueryContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BindingCandidatesRequest {
    pub consumer_installation_id: InstallationId,
    pub expected_consumer_installation_revision: u64,
    pub phase: BindingPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumer_run: Option<RunRevisionPin>,
    pub import_port: PortId,
    /// An ordering hint only. It never selects a candidate or grants authority.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preferences: Vec<ResourceSelector>,
    /// Trusted caller visibility/audience facts injected by the protocol
    /// handler. They are not request data and are consumed before applying the
    /// candidate-set cap.
    #[serde(skip)]
    #[schemars(skip)]
    #[doc(hidden)]
    pub query: Option<PowerboxQueryContext>,
}

impl BindingCandidatesRequest {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_phase_run_request(self.phase, self.consumer_run.as_ref())?;
        anyhow::ensure!(
            self.query.is_some(),
            "binding candidates require trusted caller visibility context"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct BindingCandidatesResult {
    pub candidates: Vec<BindingCandidate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gaps: Vec<BindingGap>,
}

impl BindingCandidatesResult {
    pub fn stable(mut self) -> anyhow::Result<Self> {
        self.candidates = sort_provider_candidates(self.candidates)?;
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BindingSelectRequest {
    pub consumer_installation_id: InstallationId,
    pub expected_consumer_installation_revision: u64,
    pub phase: BindingPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumer_run: Option<RunRevisionPin>,
    pub import_port: PortId,
    pub exposure_id: ExposureId,
    pub expected_exposure_revision: u64,
    pub provider_installation_id: InstallationId,
    pub expected_provider_installation_revision: u64,
    pub candidate_digest: String,
    pub idempotency_key: String,
    #[serde(skip)]
    #[schemars(skip)]
    #[doc(hidden)]
    pub query: Option<PowerboxQueryContext>,
    #[serde(skip)]
    #[schemars(skip)]
    #[doc(hidden)]
    pub authority: Option<PowerboxMutationAuthority>,
}

impl BindingSelectRequest {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_binding_idempotency_key(&self.idempotency_key)?;
        validate_sha256(&self.candidate_digest)?;
        validate_phase_run_request(self.phase, self.consumer_run.as_ref())?;
        anyhow::ensure!(
            self.query.is_some(),
            "binding selection requires trusted caller visibility context"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BindingRevokeRequest {
    pub consumer_installation_id: InstallationId,
    pub expected_consumer_installation_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumer_run: Option<RunRevisionPin>,
    pub import_port: PortId,
    pub exposure_id: ExposureId,
    pub binding_id: BindingId,
    pub expected_binding_revision: u64,
    pub idempotency_key: String,
    #[serde(skip)]
    #[schemars(skip)]
    #[doc(hidden)]
    pub authority: Option<PowerboxMutationAuthority>,
}

impl BindingRevokeRequest {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_binding_idempotency_key(&self.idempotency_key)?;
        if let Some(run) = &self.consumer_run {
            run.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct BindingMutationResult {
    pub binding: BindingView,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_binding_ids: Vec<BindingId>,
    pub idempotent: bool,
}

pub fn validate_binding_idempotency_key(key: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !key.trim().is_empty(),
        "exposure/binding mutation requires a non-empty idempotency_key"
    );
    Ok(())
}

/// Host-internal query used during preflight/prepare. It contains no capability
/// handle; handles only exist after a concrete component activation exists.
#[derive(Debug, Clone)]
pub struct RunBindingPreparationRequest {
    pub consumer_installation_id: InstallationId,
    pub consumer_installation_revision: u64,
    pub consumer_run: Option<RunRevisionPin>,
    pub required_imports: Vec<ResolvedPortPin>,
}

#[derive(Debug, Clone, Default)]
pub struct RunBindingPreparation {
    pub selected: Vec<BindingSelectionRecord>,
    pub gaps: Vec<BindingGap>,
}

#[derive(Debug, Clone)]
pub struct BindingAttachmentNotice {
    pub binding_id: BindingId,
    pub run_id: RunId,
    pub session_id: SessionId,
    pub component_activation_id: String,
    pub consumer_package_id: PackageId,
    pub consumer_node_id: NodeId,
    pub consumer_port: PortId,
}

#[derive(Debug, Clone)]
pub struct BindingCurrentValidationRequest {
    pub binding_id: BindingId,
    pub expected_binding: BindingSelectionRecord,
    pub run_id: RunId,
    pub session_id: SessionId,
    /// Attachment validation is false only during the attach transaction,
    /// before `binding_attached` installs the process-local generation notice.
    pub require_attachment: bool,
}

#[derive(Debug, Clone)]
pub struct BindingCleanupNotice {
    pub binding_id: BindingId,
    pub run_id: RunId,
    pub session_id: SessionId,
    pub reason_code: String,
}

#[derive(Debug, Clone, Default)]
pub struct PowerboxInvalidationResult {
    pub affected_binding_ids: Vec<BindingId>,
}

#[async_trait]
pub trait PowerboxControl: Send + Sync + 'static {
    /// Runtime construction installs a weak broker reference so the durable
    /// controller can enforce close-before-terminal without creating an owner
    /// cycle. Implementations that never mint Runtime handles may ignore it.
    fn install_runtime_broker(&self, _broker: Weak<crate::RunBindingBroker>) -> anyhow::Result<()> {
        Ok(())
    }

    async fn exposure_list(
        &self,
        _request: ExposureListRequest,
    ) -> anyhow::Result<Vec<ExposureView>> {
        unavailable()
    }

    async fn exposure_create(
        &self,
        request: ExposureCreateRequest,
    ) -> anyhow::Result<ExposureMutationResult> {
        request.validate()?;
        unavailable()
    }

    async fn exposure_revoke(
        &self,
        request: ExposureRevokeRequest,
    ) -> anyhow::Result<ExposureMutationResult> {
        request.validate()?;
        unavailable()
    }

    async fn binding_list(&self, _request: BindingListRequest) -> anyhow::Result<Vec<BindingView>> {
        unavailable()
    }

    async fn binding_candidates(
        &self,
        request: BindingCandidatesRequest,
    ) -> anyhow::Result<BindingCandidatesResult> {
        request.validate()?;
        unavailable()
    }

    async fn binding_select(
        &self,
        request: BindingSelectRequest,
    ) -> anyhow::Result<BindingMutationResult> {
        request.validate()?;
        unavailable()
    }

    async fn binding_revoke(
        &self,
        request: BindingRevokeRequest,
    ) -> anyhow::Result<BindingMutationResult> {
        request.validate()?;
        unavailable()
    }

    async fn prepare_run_bindings(
        &self,
        _request: RunBindingPreparationRequest,
    ) -> anyhow::Result<RunBindingPreparation> {
        unavailable()
    }

    async fn binding_attached(&self, _notice: BindingAttachmentNotice) -> anyhow::Result<()> {
        unavailable()
    }

    /// Revalidate the exact durable selection and Exposure immediately before
    /// an invocation effect boundary. A stale revision or pin must fail closed.
    async fn validate_binding_current(
        &self,
        _request: BindingCurrentValidationRequest,
    ) -> anyhow::Result<BindingSelectionRecord> {
        unavailable()
    }

    /// Persist a Host-private close obligation for a provider/endpoint/version
    /// drift discovered by the broker, close its generation, and publish the
    /// durable terminal only after that barrier completes.
    async fn binding_drifted(
        &self,
        _binding_id: &BindingId,
        _reason_code: &str,
    ) -> anyhow::Result<PowerboxInvalidationResult> {
        unavailable()
    }

    /// Idempotent cleanup acknowledgement. Implementations may be called again
    /// after an earlier failure; the broker retains the obligation in memory.
    async fn binding_detached(&self, _notice: BindingCleanupNotice) -> anyhow::Result<()> {
        unavailable()
    }

    async fn run_stopped(
        &self,
        _installation_id: &InstallationId,
        _run_id: &RunId,
        _session_id: &SessionId,
    ) -> anyhow::Result<PowerboxInvalidationResult> {
        // A Runtime with no Powerbox controller cannot have durable Exposure or
        // Binding decisions to invalidate. Real Host wiring overrides this hook.
        Ok(PowerboxInvalidationResult::default())
    }

    /// Host-internal reconciliation after a Host access mutation. This is not a
    /// public protocol method and cannot be invoked by Packages or clients.
    async fn reconcile_authority(&self) -> anyhow::Result<PowerboxInvalidationResult> {
        Ok(PowerboxInvalidationResult::default())
    }

    /// Host-internal reconciliation after a successful Installation update or
    /// removal. `current_revision=None` means the Installation is no longer
    /// active. The method never calls back into Installation control.
    async fn installation_changed(
        &self,
        _installation_id: &InstallationId,
        _current_revision: Option<u64>,
    ) -> anyhow::Result<PowerboxInvalidationResult> {
        Ok(PowerboxInvalidationResult::default())
    }
}

#[derive(Debug, Default)]
pub struct UnavailablePowerboxControl;

#[async_trait]
impl PowerboxControl for UnavailablePowerboxControl {}

fn unavailable<T>() -> anyhow::Result<T> {
    anyhow::bail!("Powerbox control unavailable")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DeniedRefresh;

    #[async_trait]
    impl PowerboxAuthorityValidator for DeniedRefresh {
        async fn validate_current(
            &self,
            _grant_id: &str,
            _subject: &PowerboxAuthoritySubject,
        ) -> anyhow::Result<()> {
            anyhow::bail!("revoked")
        }
    }

    #[test]
    fn mutation_requests_require_nonempty_idempotency_and_chosen_digest() {
        assert!(validate_binding_idempotency_key(" ").is_err());
        assert!(validate_binding_idempotency_key("request-1").is_ok());

        let mut request = BindingSelectRequest {
            consumer_installation_id: InstallationId::new(),
            expected_consumer_installation_revision: 3,
            phase: BindingPhase::Runtime,
            consumer_run: Some(RunRevisionPin {
                run_id: RunId::new(),
                run_revision: 5,
                context_id: "session-1".to_string(),
            }),
            import_port: PortId::parse("input").unwrap(),
            exposure_id: ExposureId::new(),
            expected_exposure_revision: 7,
            provider_installation_id: InstallationId::new(),
            expected_provider_installation_revision: 11,
            candidate_digest: format!("sha256:{}", "a".repeat(64)),
            idempotency_key: "request-1".to_string(),
            query: Some(PowerboxQueryContext {
                unrestricted: true,
                ..PowerboxQueryContext::default()
            }),
            authority: None,
        };
        request.validate().unwrap();
        request.phase = BindingPhase::Launch;
        assert!(request.validate().is_err());
        request.consumer_run = None;
        request.validate().unwrap();
        request.phase = BindingPhase::Runtime;
        assert!(request.validate().is_err());
        request.phase = BindingPhase::Launch;
        request.candidate_digest = "candidate-1".to_string();
        assert!(request.validate().is_err());
        request.candidate_digest = format!("sha256:{}", "a".repeat(64));
        request.idempotency_key = " ".to_string();
        assert!(request.validate().is_err());
    }

    #[test]
    fn candidate_requests_require_explicit_launch_or_runtime_phase_shape() {
        let missing_phase = serde_json::json!({
            "consumer_installation_id": InstallationId::new(),
            "expected_consumer_installation_revision": 1,
            "import_port": "input"
        });
        assert!(serde_json::from_value::<BindingCandidatesRequest>(missing_phase).is_err());

        let mut request = BindingCandidatesRequest {
            consumer_installation_id: InstallationId::new(),
            expected_consumer_installation_revision: 1,
            phase: BindingPhase::Launch,
            consumer_run: None,
            import_port: PortId::parse("input").unwrap(),
            preferences: Vec::new(),
            query: Some(PowerboxQueryContext {
                unrestricted: true,
                ..PowerboxQueryContext::default()
            }),
        };
        request.validate().unwrap();
        request.phase = BindingPhase::Runtime;
        assert!(request.validate().is_err());
        request.consumer_run = Some(RunRevisionPin {
            run_id: RunId::new(),
            run_revision: 2,
            context_id: "runtime-context".to_string(),
        });
        request.validate().unwrap();
        request.phase = BindingPhase::Launch;
        assert!(request.validate().is_err());
        request.phase = BindingPhase::Installation;
        assert!(request.validate().is_err());
    }

    #[tokio::test]
    async fn unavailable_control_fails_closed_for_public_and_has_no_internal_decisions() {
        let control = UnavailablePowerboxControl;
        assert!(control
            .exposure_list(ExposureListRequest::default())
            .await
            .is_err());
        assert!(control
            .binding_list(BindingListRequest::default())
            .await
            .is_err());
        assert!(control
            .run_stopped(&InstallationId::new(), &RunId::new(), &SessionId::new(),)
            .await
            .unwrap()
            .affected_binding_ids
            .is_empty());
    }

    #[tokio::test]
    async fn mutation_authority_is_unwireable_exact_and_refreshes_fail_closed() {
        let installation_id = InstallationId::new();
        let subject = PowerboxAuthoritySubject::BindingSelect {
            consumer_installation_id: installation_id.clone(),
            consumer_run: None,
            import_port: PortId::parse("input").unwrap(),
            exposure_id: ExposureId::new(),
        };
        let host = PowerboxMutationAuthority::verified(subject.clone(), None, None, None).unwrap();
        host.refresh_current_for(&subject).await.unwrap();
        let other = PowerboxAuthoritySubject::BindingSelect {
            consumer_installation_id: installation_id,
            consumer_run: None,
            import_port: PortId::parse("other").unwrap(),
            exposure_id: ExposureId::new(),
        };
        assert!(host.refresh_current_for(&other).await.is_err());

        let device = PowerboxMutationAuthority::verified(
            subject.clone(),
            Some("grant-1".to_string()),
            Some(Utc::now().timestamp_millis() + 60_000),
            Some(PowerboxAuthorityRefresh::new(Arc::new(DeniedRefresh))),
        )
        .unwrap();
        let durable_basis = serde_json::to_string(&device.durable_basis()).unwrap();
        assert!(!durable_basis.contains("grant-1"));
        assert!(durable_basis.contains(&powerbox_grant_reference("grant-1")));
        assert!(device.refresh_current_for(&subject).await.is_err());

        let request = BindingSelectRequest {
            consumer_installation_id: match &subject {
                PowerboxAuthoritySubject::BindingSelect {
                    consumer_installation_id,
                    ..
                } => consumer_installation_id.clone(),
                _ => unreachable!(),
            },
            expected_consumer_installation_revision: 1,
            phase: BindingPhase::Launch,
            consumer_run: None,
            import_port: PortId::parse("input").unwrap(),
            exposure_id: ExposureId::new(),
            expected_exposure_revision: 1,
            provider_installation_id: InstallationId::new(),
            expected_provider_installation_revision: 1,
            candidate_digest: format!("sha256:{}", "a".repeat(64)),
            idempotency_key: "wire-proof".to_string(),
            query: Some(PowerboxQueryContext {
                unrestricted: true,
                ..PowerboxQueryContext::default()
            }),
            authority: Some(host),
        };
        let value = serde_json::to_value(&request).unwrap();
        assert!(value.get("authority").is_none());
        let decoded: BindingSelectRequest = serde_json::from_value(value).unwrap();
        assert!(decoded.authority.is_none());
    }
}
