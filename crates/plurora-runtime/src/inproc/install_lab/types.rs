use std::path::PathBuf;

use plurora_core::{
    conformance::PackageConformanceReport, ArtifactDescriptor, ComponentLockPin, ProjectDescriptor,
    ProtocolProfilePin,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub(super) struct ResolvePlanInput {
    pub(super) root_url: String,
    #[serde(default)]
    pub(super) root_ref: String,
    #[serde(default)]
    pub(super) lockfile: Option<String>,
    /// Require GPG-signed git tags. When false (default), unsigned packages
    /// install without signature verification (matches cargo/npm/pip baseline).
    /// When true, missing or invalid signatures abort install.
    #[serde(default)]
    pub(super) require_signed: bool,
    /// Block install on conformance failure. When false (default), failures
    /// are reported as warnings but install proceeds. When true, any conformance
    /// failure aborts install.
    #[serde(default)]
    pub(super) strict_conformance: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct ExecutePlanInput {
    pub(super) plan: InstallPlan,
    pub(super) consent: Consent,
    #[serde(default)]
    pub(super) project_descriptor: Option<ProjectDescriptor>,
    #[serde(default = "super::layout::default_profile")]
    pub(super) profile: String,
    #[serde(default)]
    pub(super) data_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProfileInput {
    #[serde(default = "super::layout::default_profile")]
    pub(super) profile: String,
    #[serde(default)]
    pub(super) data_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CheckForUpdatesInput {
    #[serde(default = "super::layout::default_profile")]
    pub(super) profile: String,
    #[serde(default)]
    pub(super) data_dir: Option<String>,
    #[serde(default)]
    pub(super) project_id: Option<String>,
    #[serde(default)]
    pub(super) package_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateProjectInput {
    #[serde(default = "super::layout::default_profile")]
    pub(super) profile: String,
    #[serde(default)]
    pub(super) data_dir: Option<String>,
    #[serde(default)]
    pub(super) project_id: Option<String>,
    #[serde(default)]
    pub(super) package_id: Option<String>,
    #[serde(default)]
    pub(super) force: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct DetectKindInput {
    #[serde(default)]
    pub(super) path: Option<String>,
    #[serde(default)]
    pub(super) url: Option<String>,
    #[serde(default = "super::layout::default_head_ref")]
    pub(super) root_ref: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct RegisterProjectInput {
    pub(super) descriptor: ProjectDescriptor,
    #[serde(default)]
    pub(super) data_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UninstallInput {
    #[serde(default)]
    pub(super) package_id: Option<String>,
    #[serde(default)]
    pub(super) package_ids: Vec<String>,
    #[serde(default)]
    pub(super) project_id: Option<String>,
    #[serde(default)]
    pub(super) delete_project_data: bool,
    #[serde(default = "default_purge_orphaned_stores")]
    pub(super) purge_orphaned_stores: bool,
    #[serde(default = "super::layout::default_profile")]
    pub(super) profile: String,
    #[serde(default)]
    pub(super) data_dir: Option<String>,
}

pub(super) fn default_purge_orphaned_stores() -> bool {
    true
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct InstallPlan {
    pub(super) root_id: String,
    pub(super) packages: Vec<PlannedPackage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) project_descriptor: Option<ProjectDescriptor>,
    pub(super) permissions_summary: PermissionsSummary,
    pub(super) signature_summary: SignatureSummary,
    pub(super) integrity_summary: IntegritySummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum DetectedProjectKind {
    /// Has project.yaml that parses as PluroraNative.
    Native { descriptor: ProjectDescriptor },
    /// Has project.yaml that parses as ExternalWrapped or ExternalWorkspace.
    DeclaredExternal { descriptor: ProjectDescriptor },
    /// No project.yaml; this is an external project that needs wrapping or workspace mode.
    External { has_manifest_yaml: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PlannedPackage {
    pub(super) id: String,
    pub(super) version: String,
    pub(super) source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) url: Option<String>,
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub(super) ref_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) commit_sha: Option<String>,
    pub(super) manifest_hash: String,
    pub(super) tree_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) surface_bundle_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) package_envelope_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) component_pins: Vec<ComponentLockPin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) protocol_profile_pins: Vec<ProtocolProfilePin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) content_roots: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) manifest_relative_path: Option<String>,
    pub(super) signed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) signed_by: Option<String>,
    pub(super) permissions: PlannedPermissions,
    #[serde(default)]
    pub(super) requires: Vec<PlannedRequirement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) conformance: Option<PackageConformanceReport>,
}

#[derive(Debug)]
pub(super) struct ResolvedPackages {
    pub(super) packages: Vec<PlannedPackage>,
    pub(super) project_descriptor: Option<ProjectDescriptor>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PlannedRequirement {
    pub(super) id: String,
    pub(super) source: Value,
    #[serde(default)]
    pub(super) version: String,
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
    pub(super) manifest_hashes_match_lockfile: bool,
    pub(super) drift_detected: Vec<Value>,
}

#[derive(Debug, Clone)]
pub(super) struct PackageDescriptor {
    pub(super) source: SourceDescriptor,
}

#[derive(Debug, Clone)]
pub(super) enum SourceDescriptor {
    Git { url: String, ref_name: String },
    Local { path: PathBuf },
    Internal,
}
