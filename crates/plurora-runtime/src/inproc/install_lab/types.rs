use plurora_core::{ArtifactDescriptor, ComponentLockPin, ProtocolProfilePin};
use plurora_work::{AcquisitionRecord, InstallationSecretPolicy, StateBindingRecord};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub(super) struct ResolvePlanInput {
    pub(super) root_url: String,
    #[serde(default = "super::layout::default_head_ref")]
    pub(super) root_ref: String,
    #[serde(default)]
    pub(super) require_signed: bool,
    #[serde(default)]
    pub(super) strict_conformance: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct ExecutePlanInput {
    pub(super) plan: InstallPlan,
    pub(super) consent: Consent,
    #[serde(default)]
    pub(super) root_url: Option<String>,
    #[serde(default = "super::layout::default_head_ref")]
    pub(super) root_ref: String,
    #[serde(default)]
    pub(super) workspace_id: Option<String>,
    #[serde(default)]
    pub(super) require_signed: bool,
    #[serde(default)]
    pub(super) strict_conformance: bool,
    #[serde(default)]
    pub(super) data_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct DetectSourceInput {
    #[serde(default)]
    pub(super) path: Option<String>,
    #[serde(default)]
    pub(super) url: Option<String>,
    #[serde(default = "super::layout::default_head_ref")]
    pub(super) root_ref: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct Consent {
    #[serde(default)]
    pub(super) approved_capabilities: Vec<String>,
    #[serde(default)]
    pub(super) approved_network_hosts: Vec<String>,
    #[serde(default)]
    pub(super) approved_secret_refs: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum SourceKind {
    Work,
    Package,
    Foreign,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum WorkCandidateStatus {
    Installable,
    AuthoringRequired,
    ForeignBindingRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct WorkCandidate {
    pub(super) source_kind: SourceKind,
    pub(super) status: WorkCandidateStatus,
    pub(super) display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) work_revision: Option<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) assembly_lock: Option<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) closure: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) diagnostics: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct InstallPlan {
    pub(super) source_kind: SourceKind,
    pub(super) root_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) packages: Vec<PlannedPackage>,
    pub(super) work_candidate: WorkCandidate,
    pub(super) permissions_summary: PermissionsSummary,
    pub(super) signature_summary: SignatureSummary,
    pub(super) integrity_summary: IntegritySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PlannedPackage {
    pub(super) id: String,
    pub(super) version: String,
    pub(super) source_kind: PlannedPackageSourceKind,
    pub(super) manifest_hash: String,
    pub(super) tree_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) commit_sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) package_envelope_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) component_pins: Vec<ComponentLockPin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) protocol_profile_pins: Vec<ProtocolProfilePin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) content_roots: Vec<ArtifactDescriptor>,
    pub(super) signed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) signed_by: Option<String>,
    pub(super) permissions: PlannedPermissions,
    #[serde(default)]
    pub(super) requires: Vec<PlannedRequirement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) conformance: Option<PlannedConformance>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum PlannedPackageSourceKind {
    Local,
    Git,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PlannedRequirement {
    pub(super) id: String,
    pub(super) source_kind: String,
    #[serde(default)]
    pub(super) version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PlannedConformance {
    pub(super) passed_blocking: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) failed_checks: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct PlannedPermissions {
    #[serde(default)]
    pub(super) capabilities_invoke: Vec<String>,
    #[serde(default)]
    pub(super) network_hosts: Vec<String>,
    #[serde(default)]
    pub(super) secret_refs: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct PermissionsSummary {
    pub(super) new_capabilities: Vec<String>,
    pub(super) new_network_hosts: Vec<String>,
    pub(super) new_secret_refs: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct SignatureSummary {
    pub(super) all_signed: bool,
    pub(super) unsigned_packages: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct IntegritySummary {
    pub(super) all_objects_content_addressed: bool,
    #[serde(default)]
    pub(super) drift_detected: Vec<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct InstallationCandidate {
    pub(super) work_revision: ArtifactDescriptor,
    pub(super) assembly_lock: ArtifactDescriptor,
    pub(super) display_name: String,
    pub(super) source: AcquisitionRecord,
    pub(super) state_bindings: Vec<StateBindingRecord>,
    pub(super) secret_policy: InstallationSecretPolicy,
    pub(super) closure: Vec<ArtifactDescriptor>,
    pub(super) diagnostics: Vec<Value>,
}
