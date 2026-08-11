use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use plurora_core::ArtifactDescriptor;
use plurora_work::{
    InstallationId, RealizationId, RealizationPlan, RealizationPlanningGap, RealizationRevision,
    RealizedResource, WorkspaceId,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ManagedTargetBuildNetworkMode, ProxyRouteAccess};

pub const REALIZATION_PLAN_ACTION: &str = "realization.plan";
pub const REALIZATION_APPLY_ACTION: &str = "realization.apply";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OciImageBackendSelection {
    pub workload_id: String,
    pub execution_class: String,
    pub image: String,
    pub container_port: u16,
    pub port_name: String,
    pub route_id: String,
    #[serde(default)]
    pub route_access: ProxyRouteAccess,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_path: Option<String>,
    #[serde(default)]
    pub pull_if_missing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DockerBuildBackendSelection {
    pub workload_id: String,
    pub execution_class: String,
    pub workspace_id: WorkspaceId,
    pub build_context_ref: ArtifactDescriptor,
    pub dockerfile: String,
    pub network_mode: ManagedTargetBuildNetworkMode,
    pub source_tree_digest: String,
    pub build_descriptor_hash: String,
    pub container_port: u16,
    pub port_name: String,
    pub route_id: String,
    #[serde(default)]
    pub route_access: ProxyRouteAccess,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RealizationBackendSelection {
    OciImage(OciImageBackendSelection),
    DockerBuild(DockerBuildBackendSelection),
}

impl RealizationBackendSelection {
    pub fn workload_id(&self) -> &str {
        match self {
            Self::OciImage(selection) => &selection.workload_id,
            Self::DockerBuild(selection) => &selection.workload_id,
        }
    }

    pub fn execution_class(&self) -> &str {
        match self {
            Self::OciImage(selection) => &selection.execution_class,
            Self::DockerBuild(selection) => &selection.execution_class,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RealizationPlanRequest {
    pub installation_id: InstallationId,
    pub expected_installation_revision: u64,
    pub target_id: String,
    pub backends: Vec<RealizationBackendSelection>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RealizationPlanResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub realization: Option<RealizationRevision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_ref: Option<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<RealizationPlan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gaps: Vec<RealizationPlanningGap>,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RealizationListRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation_id: Option<InstallationId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RealizationGetRequest {
    pub installation_id: InstallationId,
    pub realization_id: RealizationId,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RealizationApplyRequest {
    pub installation_id: InstallationId,
    pub target_id: String,
    pub realization_id: RealizationId,
    pub expected_revision: u64,
    pub plan_ref: ArtifactDescriptor,
    pub approval: RealizationApproval,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RealizationStopRequest {
    pub installation_id: InstallationId,
    pub target_id: String,
    pub realization_id: RealizationId,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RealizationRollbackRequest {
    pub installation_id: InstallationId,
    pub target_id: String,
    pub realization_id: RealizationId,
    pub expected_revision: u64,
    pub rollback_to_realization_id: RealizationId,
    pub approval: RealizationApproval,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RealizationApproval {
    pub plan_digest: String,
    pub decision: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accepted_risks: Vec<String>,
    pub decided_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RealizationReconcileRequest {
    pub installation_id: InstallationId,
    pub target_id: String,
    pub realization_id: RealizationId,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RealizationMutationResult {
    pub realization: RealizationRevision,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gaps: Vec<RealizationPlanningGap>,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RealizationEffectKind {
    Build,
    Launch,
    Endpoint,
    State,
    Stop,
    Rollback,
    Reconcile,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RealizationEffectReceipt {
    pub realization_id: RealizationId,
    pub action_id: String,
    pub target_id: String,
    pub effect: RealizationEffectKind,
    pub request_digest: String,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observed_resources: Vec<RealizedResource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic_ref: Option<ArtifactDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RealizationAuthoritySubject {
    Plan {
        installation_id: InstallationId,
        target_id: String,
    },
    Apply {
        installation_id: InstallationId,
        target_id: String,
        realization_ids: Vec<RealizationId>,
    },
}

impl RealizationAuthoritySubject {
    pub fn action(&self) -> &'static str {
        match self {
            Self::Plan { .. } => REALIZATION_PLAN_ACTION,
            Self::Apply { .. } => REALIZATION_APPLY_ACTION,
        }
    }
}

#[async_trait]
pub trait RealizationAuthorityValidator: Send + Sync + 'static {
    async fn validate_current(
        &self,
        grant_id: &str,
        subject: &RealizationAuthoritySubject,
    ) -> anyhow::Result<()>;
}

#[derive(Clone)]
pub struct RealizationAuthorityRefresh(Arc<dyn RealizationAuthorityValidator>);

impl RealizationAuthorityRefresh {
    pub fn new(validator: Arc<dyn RealizationAuthorityValidator>) -> Self {
        Self(validator)
    }

    pub async fn validate_current(
        &self,
        grant_id: &str,
        subject: &RealizationAuthoritySubject,
    ) -> anyhow::Result<()> {
        self.0.validate_current(grant_id, subject).await
    }
}

impl std::fmt::Debug for RealizationAuthorityRefresh {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RealizationAuthorityRefresh(<trusted>)")
    }
}

impl PartialEq for RealizationAuthorityRefresh {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for RealizationAuthorityRefresh {}

#[derive(Debug, Clone)]
pub struct RealizationMutationAuthority {
    subject: RealizationAuthoritySubject,
    grant_id: Option<String>,
    expires_at_ms: Option<i64>,
    refresh: Option<RealizationAuthorityRefresh>,
}

impl RealizationMutationAuthority {
    pub(crate) fn verified(
        subject: RealizationAuthoritySubject,
        grant_id: Option<String>,
        expires_at_ms: Option<i64>,
        refresh: Option<RealizationAuthorityRefresh>,
    ) -> anyhow::Result<Self> {
        if grant_id.is_some() {
            anyhow::ensure!(
                expires_at_ms.is_some_and(|expiry| expiry > Utc::now().timestamp_millis()),
                "authority_denied: Realization authority is expired or unverified"
            );
        }
        Ok(Self {
            subject,
            grant_id,
            expires_at_ms,
            refresh,
        })
    }

    pub async fn refresh_current_for(
        &self,
        subject: &RealizationAuthoritySubject,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            &self.subject == subject
                && self.subject.action() == subject.action()
                && self
                    .expires_at_ms
                    .is_none_or(|expiry| expiry > Utc::now().timestamp_millis()),
            "authority_denied: current exact Realization authority is required"
        );
        if let Some(grant_id) = self.grant_id.as_deref() {
            self.refresh
                .as_ref()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "authority_denied: trusted Realization authority refresh is required"
                    )
                })?
                .validate_current(grant_id, subject)
                .await?;
        }
        Ok(())
    }

    pub fn subject(&self) -> &RealizationAuthoritySubject {
        &self.subject
    }
}

#[async_trait]
pub trait RealizationControl: Send + Sync {
    async fn plan(
        &self,
        request: RealizationPlanRequest,
        authority: RealizationMutationAuthority,
    ) -> anyhow::Result<RealizationPlanResult>;
    async fn apply(
        &self,
        request: RealizationApplyRequest,
        authority: RealizationMutationAuthority,
    ) -> anyhow::Result<RealizationMutationResult>;
    async fn list(
        &self,
        request: RealizationListRequest,
    ) -> anyhow::Result<Vec<RealizationRevision>>;
    async fn get(
        &self,
        request: RealizationGetRequest,
    ) -> anyhow::Result<Option<RealizationRevision>>;
    async fn stop(
        &self,
        request: RealizationStopRequest,
        authority: RealizationMutationAuthority,
    ) -> anyhow::Result<RealizationMutationResult>;
    async fn rollback(
        &self,
        request: RealizationRollbackRequest,
        authority: RealizationMutationAuthority,
    ) -> anyhow::Result<RealizationMutationResult>;
    async fn reconcile(
        &self,
        request: RealizationReconcileRequest,
        authority: RealizationMutationAuthority,
    ) -> anyhow::Result<RealizationMutationResult>;
}

#[derive(Debug, Default)]
pub struct UnavailableRealizationControl;

#[async_trait]
impl RealizationControl for UnavailableRealizationControl {
    async fn plan(
        &self,
        _request: RealizationPlanRequest,
        _authority: RealizationMutationAuthority,
    ) -> anyhow::Result<RealizationPlanResult> {
        anyhow::bail!("realization controller unavailable")
    }

    async fn apply(
        &self,
        _request: RealizationApplyRequest,
        _authority: RealizationMutationAuthority,
    ) -> anyhow::Result<RealizationMutationResult> {
        anyhow::bail!("realization controller unavailable")
    }

    async fn list(
        &self,
        _request: RealizationListRequest,
    ) -> anyhow::Result<Vec<RealizationRevision>> {
        Ok(Vec::new())
    }

    async fn get(
        &self,
        _request: RealizationGetRequest,
    ) -> anyhow::Result<Option<RealizationRevision>> {
        Ok(None)
    }

    async fn stop(
        &self,
        _request: RealizationStopRequest,
        _authority: RealizationMutationAuthority,
    ) -> anyhow::Result<RealizationMutationResult> {
        anyhow::bail!("realization controller unavailable")
    }

    async fn rollback(
        &self,
        _request: RealizationRollbackRequest,
        _authority: RealizationMutationAuthority,
    ) -> anyhow::Result<RealizationMutationResult> {
        anyhow::bail!("realization controller unavailable")
    }

    async fn reconcile(
        &self,
        _request: RealizationReconcileRequest,
        _authority: RealizationMutationAuthority,
    ) -> anyhow::Result<RealizationMutationResult> {
        anyhow::bail!("realization controller unavailable")
    }
}
