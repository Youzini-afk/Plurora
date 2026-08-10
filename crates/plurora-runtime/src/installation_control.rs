use std::any::Any;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use plurora_core::{
    canonical_json_bytes, secret_ref::is_installation_backed_ref, ArtifactDescriptor,
    ProtocolProfilePin, AUTHORITY_EVIDENCE_TYPE_URI,
};
use plurora_work::{
    validate_artifact_descriptor, validate_descriptor_type, validate_portable_model,
    AcquisitionRecord, AssemblyBinding, AssemblyNode, AssemblyPortExposure, BindingLock,
    InstallationId, InstallationRecord, InstallationSecretPolicy, InstallationStatus, NodeLock,
    StateBindingRecord, StateSlotDescriptor, StateSlotId, WorkEntrypoint, WorkId,
    ASSEMBLY_LOCK_TYPE_URI, WORK_REVISION_TYPE_URI,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const INSTALLATION_MANAGE_ACTION: &str = "installation.manage";

pub const INSTALLATION_STATE_SNAPSHOT_TYPE_URI: &str = "urn:plurora:installation-state-snapshot:v1";
pub const INSTALLATION_STATE_SNAPSHOT_MEDIA_TYPE: &str = "application/json";
pub const INSTALLATION_STATE_SNAPSHOT_SCHEMA: &str = "plurora.installation-state-snapshot.v1";
pub const INSTALLATION_STATE_RESET_RECEIPT_TYPE_URI: &str =
    "urn:plurora:installation-state-reset-decision-receipt:v1";
pub const INSTALLATION_STATE_RESET_RECEIPT_SCHEMA: &str =
    "plurora.installation-state-reset-decision-receipt.v1";
pub const INSTALLATION_STATE_REPLACEMENT_DECISION_RECEIPT_TYPE_URI: &str =
    "urn:plurora:installation-state-replacement-decision-receipt:v1";
pub const INSTALLATION_STATE_REPLACEMENT_DECISION_RECEIPT_SCHEMA: &str =
    "plurora.installation-state-replacement-decision-receipt.v1";
pub const INSTALLATION_STATE_RECEIPT_MEDIA_TYPE: &str = "application/json";
pub const INSTALLATION_STATE_AUTHORITY_EVIDENCE_TYPE_URI: &str = AUTHORITY_EVIDENCE_TYPE_URI;
pub const INSTALLATION_STATE_AUTHORITY_EVIDENCE_MEDIA_TYPE: &str = "application/json";
pub const INSTALLATION_STATE_AUTHORITY_EVIDENCE_SCHEMA: &str =
    "plurora.installation-state-authority-evidence.v1";
pub const INSTALLATION_STATE_OPERATION: &str = "installation.update.state";

#[async_trait]
pub trait InstallationAuthorityValidator: Send + Sync + 'static {
    async fn validate_current(
        &self,
        grant_id: &str,
        subject: &InstallationAuthoritySubject,
    ) -> anyhow::Result<()>;
}

/// Exact Host resource whose current `installation.manage` authority must be
/// revalidated before an Installation mutation crosses an effect boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallationAuthoritySubject {
    Work(WorkId),
    Installation(InstallationId),
}

#[derive(Clone)]
pub struct InstallationAuthorityRefresh(Arc<dyn InstallationAuthorityValidator>);

impl InstallationAuthorityRefresh {
    pub fn new(validator: Arc<dyn InstallationAuthorityValidator>) -> Self {
        Self(validator)
    }

    async fn validate_current(
        &self,
        grant_id: &str,
        subject: &InstallationAuthoritySubject,
    ) -> anyhow::Result<()> {
        self.0.validate_current(grant_id, subject).await
    }
}

impl std::fmt::Debug for InstallationAuthorityRefresh {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("InstallationAuthorityRefresh(<trusted>)")
    }
}

impl PartialEq for InstallationAuthorityRefresh {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for InstallationAuthorityRefresh {}

/// Runtime-minted proof that the trusted transport authenticated current,
/// resource-exact Installation management authority. It is deliberately not
/// serializable, so request JSON can never supply or replay it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationMutationAuthority {
    subject: InstallationAuthoritySubject,
    action: &'static str,
    grant_id: Option<String>,
    expires_at_ms: Option<i64>,
    refresh: Option<InstallationAuthorityRefresh>,
}

impl InstallationMutationAuthority {
    pub(crate) fn verified_for_work(
        work_id: WorkId,
        grant_id: Option<String>,
        expires_at_ms: Option<i64>,
        refresh: Option<InstallationAuthorityRefresh>,
    ) -> anyhow::Result<Self> {
        Self::verified(
            InstallationAuthoritySubject::Work(work_id),
            grant_id,
            expires_at_ms,
            refresh,
        )
    }

    pub(crate) fn verified_for_installation(
        installation_id: InstallationId,
        grant_id: Option<String>,
        expires_at_ms: Option<i64>,
        refresh: Option<InstallationAuthorityRefresh>,
    ) -> anyhow::Result<Self> {
        Self::verified(
            InstallationAuthoritySubject::Installation(installation_id),
            grant_id,
            expires_at_ms,
            refresh,
        )
    }

    fn verified(
        subject: InstallationAuthoritySubject,
        grant_id: Option<String>,
        expires_at_ms: Option<i64>,
        refresh: Option<InstallationAuthorityRefresh>,
    ) -> anyhow::Result<Self> {
        if grant_id.is_some() {
            anyhow::ensure!(
                expires_at_ms.is_some_and(|expiry| expiry > chrono::Utc::now().timestamp_millis()),
                "authority_denied: Installation management authority is expired or unverified"
            );
        }
        Ok(Self {
            subject,
            action: INSTALLATION_MANAGE_ACTION,
            grant_id,
            expires_at_ms,
            refresh,
        })
    }

    fn ensure_current_for(&self, subject: &InstallationAuthoritySubject) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.action == INSTALLATION_MANAGE_ACTION
                && &self.subject == subject
                && self
                    .expires_at_ms
                    .is_none_or(|expiry| expiry > chrono::Utc::now().timestamp_millis()),
            "authority_denied: current exact Installation management authority is required"
        );
        Ok(())
    }

    async fn refresh_current_for(
        &self,
        subject: &InstallationAuthoritySubject,
    ) -> anyhow::Result<()> {
        self.ensure_current_for(subject)?;
        if let Some(grant_id) = self.grant_id.as_deref() {
            let refresh = self.refresh.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "authority_denied: trusted authority refresh is required for device state mutation"
                )
            })?;
            refresh
                .validate_current(grant_id, subject)
                .await
                .map_err(|_| {
                    anyhow::anyhow!(
                        "authority_denied: Installation management authority is no longer current"
                    )
                })?;
            self.ensure_current_for(subject)?;
        }
        Ok(())
    }

    pub async fn refresh_current_for_work(&self, work_id: &WorkId) -> anyhow::Result<()> {
        self.refresh_current_for(&InstallationAuthoritySubject::Work(work_id.clone()))
            .await
    }

    pub async fn refresh_current_for_installation(
        &self,
        installation_id: &InstallationId,
    ) -> anyhow::Result<()> {
        self.refresh_current_for(&InstallationAuthoritySubject::Installation(
            installation_id.clone(),
        ))
        .await
    }

    pub fn grant_id(&self) -> Option<&str> {
        self.grant_id.as_deref()
    }

    pub fn expires_at_ms(&self) -> Option<i64> {
        self.expires_at_ms
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct InstallationView {
    pub record: InstallationRecord,
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback: Option<InstallationRollbackPointer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct InstallationRollbackPointer {
    pub revision: u64,
    pub work_revision: ArtifactDescriptor,
    pub assembly_lock: ArtifactDescriptor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_snapshot: Option<ArtifactDescriptor>,
}

/// A control-plane lease for one exact, Ready Installation secret-store revision.
///
/// The opaque lease must exclude Installation lifecycle transitions until this
/// value is dropped. This keeps a secret-store filesystem effect inside the same
/// Ready/revision precondition that was checked immediately before the effect.
pub struct InstallationSecretStoreGuard {
    installation_id: InstallationId,
    revision: u64,
    path: PathBuf,
    _lease: Box<dyn Any + Send>,
}

impl InstallationSecretStoreGuard {
    /// Construct a guard after the control plane has acquired its lifecycle lock
    /// and synchronized the authoritative Installation journal.
    pub fn verified(
        installation_id: &InstallationId,
        expected_revision: u64,
        view: &InstallationView,
        path: PathBuf,
        lease: Box<dyn Any + Send>,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            &view.record.installation_id == installation_id
                && view.revision == expected_revision
                && view.record.status == InstallationStatus::Ready,
            "installation secret store precondition is stale"
        );
        Ok(Self {
            installation_id: installation_id.clone(),
            revision: view.revision,
            path,
            _lease: lease,
        })
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn installation_id(&self) -> &InstallationId {
        &self.installation_id
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
}

impl std::fmt::Debug for InstallationSecretStoreGuard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InstallationSecretStoreGuard")
            .field("revision", &self.revision)
            .field("path", &"redacted")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct InstallationListRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<InstallationStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct InstallationGetRequest {
    pub installation_id: InstallationId,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstallationCreateRequest {
    pub work_id: WorkId,
    pub work_revision: ArtifactDescriptor,
    pub assembly_lock: ArtifactDescriptor,
    pub display_name: String,
    pub source: AcquisitionRecord,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_bindings: Vec<StateBindingRecord>,
    #[serde(default)]
    pub secret_policy: InstallationSecretPolicy,
    pub idempotency_key: String,
    /// Trusted sidecar minted after protocol authorization. It is excluded from
    /// the wire/schema contract, so request JSON can neither forge nor replay it.
    #[serde(skip)]
    #[schemars(skip)]
    #[doc(hidden)]
    pub authority: Option<InstallationMutationAuthority>,
}

impl InstallationCreateRequest {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_idempotency_key(&self.idempotency_key)?;
        validate_descriptor_type(&self.work_revision, WORK_REVISION_TYPE_URI)?;
        validate_descriptor_type(&self.assembly_lock, ASSEMBLY_LOCK_TYPE_URI)?;
        anyhow::ensure!(
            !self.display_name.trim().is_empty(),
            "installation create requires a non-empty display_name"
        );
        validate_acquisition_record(&self.source)?;
        validate_state_bindings(&self.state_bindings)?;
        validate_secret_policy(&self.secret_policy)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstallationUpdateRequest {
    pub installation_id: InstallationId,
    pub expected_revision: u64,
    pub work_revision: ArtifactDescriptor,
    pub assembly_lock: ArtifactDescriptor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<AcquisitionRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_bindings: Option<Vec<StateBindingRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_policy: Option<InstallationSecretPolicy>,
    pub state_action: InstallationStateAction,
    pub idempotency_key: String,
    /// Trusted sidecar minted after protocol authorization. It is excluded from
    /// the wire/schema contract and is required for every durable update.
    #[serde(skip)]
    #[schemars(skip)]
    #[doc(hidden)]
    pub authority: Option<InstallationMutationAuthority>,
}

impl InstallationUpdateRequest {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_idempotency_key(&self.idempotency_key)?;
        validate_descriptor_type(&self.work_revision, WORK_REVISION_TYPE_URI)?;
        validate_descriptor_type(&self.assembly_lock, ASSEMBLY_LOCK_TYPE_URI)?;
        if let Some(display_name) = &self.display_name {
            anyhow::ensure!(
                !display_name.trim().is_empty(),
                "installation update display_name must be non-empty when provided"
            );
        }
        if let Some(source) = &self.source {
            validate_acquisition_record(source)?;
        }
        if let Some(state_bindings) = &self.state_bindings {
            validate_state_bindings(state_bindings)?;
        }
        if let Some(secret_policy) = &self.secret_policy {
            validate_secret_policy(secret_policy)?;
        }
        validate_state_action(&self.state_action)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InstallationStateAction {
    Preserve,
    Replace {
        replacement_snapshot: ArtifactDescriptor,
    },
    Reset,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstallationStateSnapshot {
    pub schema: String,
    pub entries: Vec<InstallationStateSnapshotEntry>,
}

impl InstallationStateSnapshot {
    pub fn empty() -> Self {
        Self {
            schema: INSTALLATION_STATE_SNAPSHOT_SCHEMA.to_string(),
            entries: Vec::new(),
        }
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.schema == INSTALLATION_STATE_SNAPSHOT_SCHEMA,
            "state snapshot schema is invalid"
        );
        let mut files = BTreeSet::new();
        let mut previous: Option<&str> = None;
        for entry in &self.entries {
            validate_state_snapshot_path(&entry.path)?;
            anyhow::ensure!(
                files.insert(entry.path.as_str()),
                "state snapshot repeats a file"
            );
            if let Some(previous) = previous {
                anyhow::ensure!(
                    previous < entry.path.as_str(),
                    "state snapshot paths are not sorted"
                );
            }
            previous = Some(&entry.path);

            let mut ancestor = entry.path.as_str();
            while let Some((parent, _)) = ancestor.rsplit_once('/') {
                anyhow::ensure!(
                    !files.contains(parent),
                    "state snapshot file shadows a directory"
                );
                ancestor = parent;
            }
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> anyhow::Result<Vec<u8>> {
        self.validate()?;
        canonical_json_bytes(self)
    }

    pub fn artifact_descriptor(&self) -> anyhow::Result<ArtifactDescriptor> {
        let bytes = self.canonical_bytes()?;
        Ok(ArtifactDescriptor {
            artifact_type_uri: INSTALLATION_STATE_SNAPSHOT_TYPE_URI.to_string(),
            media_type: INSTALLATION_STATE_SNAPSHOT_MEDIA_TYPE.to_string(),
            digest: crate::object_store::sha256_digest(&bytes),
            size_bytes: u64::try_from(bytes.len())
                .map_err(|_| anyhow::anyhow!("state snapshot size cannot be represented"))?,
            references: Vec::new(),
            annotations: Default::default(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstallationStateSnapshotEntry {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallationStateDecision {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallationStateDecisionAction {
    Reset,
    Replace,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstallationStateDecisionReceipt {
    pub schema: String,
    pub installation_id: InstallationId,
    pub expected_revision: u64,
    pub candidate_work_digest: String,
    pub candidate_lock_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement_snapshot_digest: Option<String>,
    pub operation: String,
    pub action: InstallationStateDecisionAction,
    pub decision: InstallationStateDecision,
    pub authority_evidence: Vec<ArtifactDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstallationStateAuthorityEvidence {
    pub schema: String,
    pub action: String,
    pub installation_id: InstallationId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StateDisposition {
    Keep,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstallationRemoveRequest {
    pub installation_id: InstallationId,
    pub expected_revision: u64,
    pub state_disposition: StateDisposition,
    pub idempotency_key: String,
    /// Trusted sidecar minted after protocol authorization. It is excluded from
    /// the wire/schema contract, so request JSON can neither forge nor replay it.
    #[serde(skip)]
    #[schemars(skip)]
    #[doc(hidden)]
    pub authority: Option<InstallationMutationAuthority>,
}

impl InstallationRemoveRequest {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_idempotency_key(&self.idempotency_key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InstallationChange<T> {
    Added { after: T },
    Removed { before: T },
    Changed { before: T, after: T },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstallationItemDiff<T> {
    pub id: String,
    pub change: InstallationChange<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InstallationStateSlotChange {
    Added {
        after: StateSlotDescriptor,
    },
    Removed {
        before: StateSlotDescriptor,
    },
    Changed {
        before: StateSlotDescriptor,
        after: StateSlotDescriptor,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallationStateSlotRequirement {
    None,
    Replace,
    Reset,
}

impl Default for InstallationStateSlotRequirement {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstallationStateSlotDiff {
    pub state_slot_id: StateSlotId,
    pub change: InstallationStateSlotChange,
    /// Structural action required when the Installation state tree is non-empty.
    /// An empty state tree still reports the difference without forcing an effect.
    #[serde(default)]
    pub required_action: InstallationStateSlotRequirement,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct InstallationDiff {
    pub work_revision_changed: bool,
    pub assembly_lock_changed: bool,
    pub display_name_changed: bool,
    pub source_changed: bool,
    pub state_bindings_changed: bool,
    pub secret_policy_changed: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub work_entrypoints: Vec<InstallationItemDiff<WorkEntrypoint>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub work_content_roots: Vec<InstallationItemDiff<ArtifactDescriptor>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_rights: Option<InstallationChange<ArtifactDescriptor>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_transparency: Option<InstallationChange<ArtifactDescriptor>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_operational_intent: Option<InstallationChange<ArtifactDescriptor>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assembly_nodes: Vec<InstallationItemDiff<AssemblyNode>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assembly_bindings: Vec<InstallationItemDiff<AssemblyBinding>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assembly_exposed_ports: Vec<InstallationItemDiff<AssemblyPortExposure>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_slots: Vec<InstallationStateSlotDiff>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assembly_lock_nodes: Vec<InstallationItemDiff<NodeLock>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assembly_lock_bindings: Vec<InstallationItemDiff<BindingLock>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assembly_lock_protocol_profiles: Vec<InstallationItemDiff<ProtocolProfilePin>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assembly_lock_content_roots: Vec<InstallationItemDiff<ArtifactDescriptor>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct InstallationMutationResult {
    pub installation: InstallationView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<InstallationDiff>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub receipts: Vec<ArtifactDescriptor>,
    pub idempotent: bool,
}

pub fn validate_idempotency_key(key: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !key.trim().is_empty(),
        "installation mutation requires a non-empty idempotency_key"
    );
    Ok(())
}

fn validate_acquisition_record(source: &AcquisitionRecord) -> anyhow::Result<()> {
    if let Some(source_ref) = &source.source_ref {
        validate_artifact_descriptor(source_ref)?;
    }
    for provenance_ref in &source.provenance_refs {
        validate_artifact_descriptor(provenance_ref)?;
    }
    validate_portable_model(source)?;
    Ok(())
}

fn validate_state_bindings(state_bindings: &[StateBindingRecord]) -> anyhow::Result<()> {
    let mut state_slots = BTreeSet::new();
    for binding in state_bindings {
        anyhow::ensure!(
            !binding.binding_id.trim().is_empty(),
            "installation state binding requires a non-empty binding_id"
        );
        anyhow::ensure!(
            state_slots.insert(&binding.state_slot_id),
            "installation state bindings must not repeat a state_slot_id"
        );
        if let Some(provider_ref) = &binding.provider_ref {
            validate_artifact_descriptor(provider_ref)?;
        }
        validate_portable_model(binding)?;
    }
    Ok(())
}

fn validate_secret_policy(secret_policy: &InstallationSecretPolicy) -> anyhow::Result<()> {
    let mut allowed = BTreeSet::new();
    for reference in &secret_policy.allowed_secret_refs {
        anyhow::ensure!(
            is_installation_backed_ref(reference),
            "installation secret policy must contain only installation-scoped secret references"
        );
        anyhow::ensure!(
            allowed.insert(reference.as_str()),
            "installation secret policy must not contain duplicate references"
        );
    }
    validate_portable_model(secret_policy)?;
    Ok(())
}

fn validate_state_action(state_action: &InstallationStateAction) -> anyhow::Result<()> {
    match state_action {
        InstallationStateAction::Preserve | InstallationStateAction::Reset => {}
        InstallationStateAction::Replace {
            replacement_snapshot,
        } => {
            validate_artifact_descriptor(replacement_snapshot)?;
            anyhow::ensure!(
                replacement_snapshot.artifact_type_uri == INSTALLATION_STATE_SNAPSHOT_TYPE_URI
                    && replacement_snapshot.media_type == INSTALLATION_STATE_SNAPSHOT_MEDIA_TYPE,
                "replacement state snapshot descriptor type is invalid"
            );
        }
    }
    Ok(())
}

fn validate_state_snapshot_path(value: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !value.is_empty()
            && !value.starts_with('/')
            && !value.contains('\\')
            && !value.contains('\0'),
        "state snapshot path is invalid"
    );
    let segments = value.split('/').collect::<Vec<_>>();
    anyhow::ensure!(
        segments
            .iter()
            .all(|segment| !segment.is_empty() && *segment != "." && *segment != ".."),
        "state snapshot path is invalid"
    );
    anyhow::ensure!(
        !segments[0]
            .as_bytes()
            .get(1)
            .is_some_and(|byte| *byte == b':' && segments[0].as_bytes()[0].is_ascii_alphabetic()),
        "state snapshot path has a platform prefix"
    );
    Ok(())
}

#[async_trait]
pub trait InstallationControl: Send + Sync + 'static {
    async fn list(&self, request: InstallationListRequest)
        -> anyhow::Result<Vec<InstallationView>>;

    async fn get(
        &self,
        installation_id: &InstallationId,
    ) -> anyhow::Result<Option<InstallationView>>;

    async fn create(
        &self,
        request: InstallationCreateRequest,
    ) -> anyhow::Result<InstallationMutationResult>;

    async fn update(
        &self,
        request: InstallationUpdateRequest,
    ) -> anyhow::Result<InstallationMutationResult>;

    async fn remove(
        &self,
        request: InstallationRemoveRequest,
    ) -> anyhow::Result<InstallationMutationResult>;

    /// Confirm that an exact state receipt/evidence descriptor was issued by the
    /// authoritative journal for this Installation. Structure and CAS presence
    /// alone are insufficient. The default fails closed.
    async fn validate_issued_state_artifact(
        &self,
        _installation_id: &InstallationId,
        _descriptor: &ArtifactDescriptor,
    ) -> anyhow::Result<()> {
        anyhow::bail!("installation control unavailable")
    }

    fn installation_secret_store_path(
        &self,
        _installation_id: &InstallationId,
    ) -> anyhow::Result<PathBuf> {
        anyhow::bail!("installation control unavailable")
    }

    /// Acquire the secret-store path and an exclusion lease for an exact Ready
    /// Installation revision. Implementations must synchronize durable authority,
    /// acquire the same lock used by update/remove, check exact ID + revision +
    /// Ready status under that lock, resolve the contained path, and retain the
    /// lock in the returned guard. The default fails closed.
    async fn acquire_ready_secret_store(
        &self,
        _installation_id: &InstallationId,
        _expected_revision: u64,
    ) -> anyhow::Result<InstallationSecretStoreGuard> {
        anyhow::bail!("installation control unavailable")
    }
}

#[derive(Debug, Default)]
pub struct UnavailableInstallationControl;

fn unavailable<T>() -> anyhow::Result<T> {
    anyhow::bail!("installation control unavailable")
}

#[async_trait]
impl InstallationControl for UnavailableInstallationControl {
    async fn list(
        &self,
        _request: InstallationListRequest,
    ) -> anyhow::Result<Vec<InstallationView>> {
        unavailable()
    }

    async fn get(
        &self,
        _installation_id: &InstallationId,
    ) -> anyhow::Result<Option<InstallationView>> {
        unavailable()
    }

    async fn create(
        &self,
        request: InstallationCreateRequest,
    ) -> anyhow::Result<InstallationMutationResult> {
        request.validate()?;
        unavailable()
    }

    async fn update(
        &self,
        request: InstallationUpdateRequest,
    ) -> anyhow::Result<InstallationMutationResult> {
        request.validate()?;
        unavailable()
    }

    async fn remove(
        &self,
        request: InstallationRemoveRequest,
    ) -> anyhow::Result<InstallationMutationResult> {
        request.validate()?;
        unavailable()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(artifact_type_uri: &str, byte: char) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_type_uri: artifact_type_uri.to_string(),
            media_type: "application/json".to_string(),
            digest: format!("sha256:{}", byte.to_string().repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: Default::default(),
        }
    }

    fn create_request() -> InstallationCreateRequest {
        InstallationCreateRequest {
            work_id: WorkId::parse("tests/example").unwrap(),
            work_revision: artifact(WORK_REVISION_TYPE_URI, 'a'),
            assembly_lock: artifact(ASSEMBLY_LOCK_TYPE_URI, 'b'),
            display_name: "Example".to_string(),
            source: AcquisitionRecord {
                kind: plurora_work::AcquisitionKind::WorkBundle,
                source_ref: None,
                provenance_refs: Vec::new(),
                update_channel: None,
            },
            state_bindings: Vec::new(),
            secret_policy: InstallationSecretPolicy::default(),
            idempotency_key: "install-42".to_string(),
            authority: None,
        }
    }

    #[test]
    fn state_action_has_an_explicit_tag() {
        let value = serde_json::to_value(InstallationStateAction::Preserve).unwrap();
        assert_eq!(value, serde_json::json!({"kind": "preserve"}));
    }

    #[test]
    fn state_snapshot_accepts_only_sorted_unique_relative_paths() {
        let valid = InstallationStateSnapshot {
            schema: INSTALLATION_STATE_SNAPSHOT_SCHEMA.to_string(),
            entries: vec![
                InstallationStateSnapshotEntry {
                    path: "preferences.json".to_string(),
                    bytes: b"{}".to_vec(),
                },
                InstallationStateSnapshotEntry {
                    path: "saves/slot-1.bin".to_string(),
                    bytes: vec![0, 255],
                },
            ],
        };
        valid.validate().unwrap();
        let descriptor = valid.artifact_descriptor().unwrap();
        assert_eq!(
            descriptor.artifact_type_uri,
            INSTALLATION_STATE_SNAPSHOT_TYPE_URI
        );
        assert_eq!(
            descriptor.media_type,
            INSTALLATION_STATE_SNAPSHOT_MEDIA_TYPE
        );
        assert!(descriptor.references.is_empty());

        for path in [
            "",
            "/absolute",
            "\\absolute",
            "C:/prefixed",
            "../escape",
            "nested/../escape",
            "nested//empty",
            "nested/./dot",
            "nested\\backslash",
            "nul\0byte",
        ] {
            let snapshot = InstallationStateSnapshot {
                schema: INSTALLATION_STATE_SNAPSHOT_SCHEMA.to_string(),
                entries: vec![InstallationStateSnapshotEntry {
                    path: path.to_string(),
                    bytes: Vec::new(),
                }],
            };
            assert!(
                snapshot.validate().is_err(),
                "accepted invalid path {path:?}"
            );
        }

        let duplicate = InstallationStateSnapshot {
            schema: INSTALLATION_STATE_SNAPSHOT_SCHEMA.to_string(),
            entries: vec![
                InstallationStateSnapshotEntry {
                    path: "same".to_string(),
                    bytes: Vec::new(),
                },
                InstallationStateSnapshotEntry {
                    path: "same".to_string(),
                    bytes: Vec::new(),
                },
            ],
        };
        assert!(duplicate.validate().is_err());

        let shadowed = InstallationStateSnapshot {
            schema: INSTALLATION_STATE_SNAPSHOT_SCHEMA.to_string(),
            entries: vec![
                InstallationStateSnapshotEntry {
                    path: "file".to_string(),
                    bytes: Vec::new(),
                },
                InstallationStateSnapshotEntry {
                    path: "file/child".to_string(),
                    bytes: Vec::new(),
                },
            ],
        };
        assert!(shadowed.validate().is_err());
    }

    #[test]
    fn mutation_requests_reject_blank_idempotency_keys() {
        assert!(validate_idempotency_key("").is_err());
        assert!(validate_idempotency_key("  ").is_err());
        assert!(validate_idempotency_key("install-42").is_ok());
    }

    #[test]
    fn create_request_rejects_malformed_descriptors_and_secret_policy() {
        let mut request = create_request();
        request.work_revision.artifact_type_uri = ASSEMBLY_LOCK_TYPE_URI.to_string();
        assert!(request.validate().is_err());

        let mut request = create_request();
        request
            .secret_policy
            .allowed_secret_refs
            .push("plaintext-value".to_string());
        assert!(request.validate().is_err());

        let mut request = create_request();
        request
            .secret_policy
            .allowed_secret_refs
            .push("secret_ref:store:API_KEY".to_string());
        assert!(request.validate().is_err());

        let mut request = create_request();
        request.secret_policy.allowed_secret_refs = vec![
            "secret_ref:installation:API_KEY".to_string(),
            "secret_ref:installation:API_KEY".to_string(),
        ];
        assert!(request.validate().is_err());
    }

    #[tokio::test]
    async fn unavailable_control_fails_closed() {
        let control = UnavailableInstallationControl;
        assert!(control
            .list(InstallationListRequest::default())
            .await
            .is_err());
        assert!(control.create(create_request()).await.is_err());
    }
}
