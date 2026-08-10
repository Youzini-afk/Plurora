use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;
use plurora_core::{ArtifactDescriptor, PackageId};
use plurora_work::{InstallationId, RunId, RunRecord, RunStatus};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const RUN_ACTION: &str = "run";

#[async_trait]
pub trait RunAuthorityValidator: Send + Sync + 'static {
    async fn validate_current(
        &self,
        grant_id: &str,
        installation_id: &InstallationId,
        run_id: Option<&RunId>,
    ) -> anyhow::Result<()>;
}

#[derive(Clone)]
pub struct RunAuthorityRefresh(Arc<dyn RunAuthorityValidator>);

impl RunAuthorityRefresh {
    pub fn new(validator: Arc<dyn RunAuthorityValidator>) -> Self {
        Self(validator)
    }

    async fn validate_current(
        &self,
        grant_id: &str,
        installation_id: &InstallationId,
        run_id: Option<&RunId>,
    ) -> anyhow::Result<()> {
        self.0
            .validate_current(grant_id, installation_id, run_id)
            .await
    }
}

impl std::fmt::Debug for RunAuthorityRefresh {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RunAuthorityRefresh(<trusted>)")
    }
}

impl PartialEq for RunAuthorityRefresh {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for RunAuthorityRefresh {}

/// Runtime-minted proof of current `run` authority for the exact parent
/// Installation. A RunId is Host-generated after preflight, so creation cannot
/// require a client to pre-authorize an unknown child resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunMutationAuthority {
    installation_id: InstallationId,
    run_id: Option<RunId>,
    action: &'static str,
    grant_id: Option<String>,
    expires_at_ms: Option<i64>,
    refresh: Option<RunAuthorityRefresh>,
}

impl RunMutationAuthority {
    pub(crate) fn verified_for_installation(
        installation_id: InstallationId,
        grant_id: Option<String>,
        expires_at_ms: Option<i64>,
        refresh: Option<RunAuthorityRefresh>,
    ) -> anyhow::Result<Self> {
        if grant_id.is_some() {
            anyhow::ensure!(
                expires_at_ms.is_some_and(|expiry| expiry > chrono::Utc::now().timestamp_millis()),
                "authority_denied: Run authority is expired or unverified"
            );
        }
        Ok(Self {
            installation_id,
            run_id: None,
            action: RUN_ACTION,
            grant_id,
            expires_at_ms,
            refresh,
        })
    }

    pub(crate) fn verified_for_run(
        installation_id: InstallationId,
        run_id: RunId,
        grant_id: Option<String>,
        expires_at_ms: Option<i64>,
        refresh: Option<RunAuthorityRefresh>,
    ) -> anyhow::Result<Self> {
        let mut authority =
            Self::verified_for_installation(installation_id, grant_id, expires_at_ms, refresh)?;
        authority.run_id = Some(run_id);
        Ok(authority)
    }

    pub async fn refresh_current_for_installation(
        &self,
        installation_id: &InstallationId,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.action == RUN_ACTION
                && &self.installation_id == installation_id
                && self.run_id.is_none()
                && self
                    .expires_at_ms
                    .is_none_or(|expiry| expiry > chrono::Utc::now().timestamp_millis()),
            "authority_denied: current exact Installation Run authority is required"
        );
        if let Some(grant_id) = self.grant_id.as_deref() {
            self.refresh
                .as_ref()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "authority_denied: trusted authority refresh is required for a device Run mutation"
                    )
                })?
                .validate_current(grant_id, installation_id, None)
                .await
                .map_err(|_| {
                    anyhow::anyhow!("authority_denied: Run authority is no longer current")
                })?;
        }
        Ok(())
    }

    pub async fn refresh_current_for_run(
        &self,
        installation_id: &InstallationId,
        run_id: &RunId,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.action == RUN_ACTION
                && &self.installation_id == installation_id
                && self.run_id.as_ref() == Some(run_id)
                && self
                    .expires_at_ms
                    .is_none_or(|expiry| expiry > chrono::Utc::now().timestamp_millis()),
            "authority_denied: current exact Installation and Run authority is required"
        );
        if let Some(grant_id) = self.grant_id.as_deref() {
            self.refresh
                .as_ref()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "authority_denied: trusted authority refresh is required for a device Run mutation"
                    )
                })?
                .validate_current(grant_id, installation_id, Some(run_id))
                .await
                .map_err(|_| {
                    anyhow::anyhow!("authority_denied: Run authority is no longer current")
                })?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RunView {
    pub record: RunRecord,
    pub revision: u64,
    pub installation_revision: u64,
    pub entrypoint_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunListRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation_id: Option<InstallationId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<RunStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunGetRequest {
    pub installation_id: InstallationId,
    pub run_id: RunId,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunStatusRequest {
    pub installation_id: InstallationId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunEntrypointPreflight {
    pub entrypoint_id: String,
    pub gaps: Vec<RunGap>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunStatusView {
    pub installation_id: InstallationId,
    pub installation_revision: u64,
    pub work_revision: ArtifactDescriptor,
    pub active_run: Option<RunView>,
    pub preflight: Option<RunEntrypointPreflight>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunStartRequest {
    pub installation_id: InstallationId,
    pub expected_installation_revision: u64,
    pub entrypoint_id: String,
    pub idempotency_key: String,
    #[serde(skip)]
    #[schemars(skip)]
    #[doc(hidden)]
    pub authority: Option<RunMutationAuthority>,
}

impl RunStartRequest {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.entrypoint_id.trim().is_empty(),
            "Run start requires a non-empty entrypoint_id"
        );
        validate_run_idempotency_key(&self.idempotency_key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunStopRequest {
    pub installation_id: InstallationId,
    pub run_id: RunId,
    pub expected_revision: u64,
    pub idempotency_key: String,
    #[serde(skip)]
    #[schemars(skip)]
    #[doc(hidden)]
    pub authority: Option<RunMutationAuthority>,
}

impl RunStopRequest {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_run_idempotency_key(&self.idempotency_key)
    }
}

pub fn validate_run_idempotency_key(key: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !key.trim().is_empty(),
        "Run mutation requires a non-empty idempotency_key"
    );
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunGap {
    pub reason_code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_id: Option<String>,
    pub next_step: String,
}

impl RunGap {
    pub fn new(reason_code: impl Into<String>, next_step: impl Into<String>) -> Self {
        Self {
            reason_code: reason_code.into(),
            node_id: None,
            port_id: None,
            next_step: next_step.into(),
        }
    }

    pub fn for_node(mut self, node_id: impl ToString) -> Self {
        self.node_id = Some(node_id.to_string());
        self
    }

    pub fn for_port(mut self, port_id: impl ToString) -> Self {
        self.port_id = Some(port_id.to_string());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RunStartResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<RunView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gaps: Vec<RunGap>,
    pub idempotent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RunMutationResult {
    pub run: RunView,
    pub idempotent: bool,
}

pub struct RunPreparation {
    pub installation_revision: u64,
    pub entrypoint_id: String,
    pub gaps: Vec<RunGap>,
    pub(crate) opaque: Option<Box<dyn Any + Send>>,
}

pub struct RunStatusInspection {
    pub installation_revision: u64,
    pub work_revision: ArtifactDescriptor,
    pub preflight: Option<RunEntrypointPreflight>,
}

impl RunPreparation {
    pub fn blocked(installation_revision: u64, entrypoint_id: String, gaps: Vec<RunGap>) -> Self {
        Self {
            installation_revision,
            entrypoint_id,
            gaps,
            opaque: None,
        }
    }

    pub fn ready(
        installation_revision: u64,
        entrypoint_id: String,
        opaque: Box<dyn Any + Send>,
    ) -> Self {
        Self {
            installation_revision,
            entrypoint_id,
            gaps: Vec::new(),
            opaque: Some(opaque),
        }
    }

    pub(crate) fn take<T: Any + Send>(mut self) -> anyhow::Result<T> {
        self.opaque
            .take()
            .ok_or_else(|| anyhow::anyhow!("Run preparation has no activation state"))?
            .downcast::<T>()
            .map(|boxed| *boxed)
            .map_err(|_| anyhow::anyhow!("Run preparation state type is invalid"))
    }
}

pub struct RunActivation {
    pub context_id: Option<String>,
    pub node_instances: Vec<plurora_work::NodeInstanceRecord>,
    pub bindings: Vec<plurora_work::ActiveBindingRecord>,
    pub(crate) opaque: Option<Box<dyn Any + Send>>,
}

impl RunActivation {
    pub fn new(
        context_id: Option<String>,
        node_instances: Vec<plurora_work::NodeInstanceRecord>,
        bindings: Vec<plurora_work::ActiveBindingRecord>,
        opaque: Box<dyn Any + Send>,
    ) -> Self {
        Self {
            context_id,
            node_instances,
            bindings,
            opaque: Some(opaque),
        }
    }

    pub(crate) fn take<T: Any + Send>(&mut self) -> anyhow::Result<T> {
        self.opaque
            .take()
            .ok_or_else(|| anyhow::anyhow!("Run activation state was already released"))?
            .downcast::<T>()
            .map(|boxed| *boxed)
            .map_err(|_| anyhow::anyhow!("Run activation state type is invalid"))
    }
}

#[async_trait]
pub trait RunLifecycleDriver: Send + Sync + 'static {
    async fn inspect_status(
        &self,
        request: &RunStatusRequest,
    ) -> anyhow::Result<RunStatusInspection>;

    async fn prepare_start(&self, request: &RunStartRequest) -> anyhow::Result<RunPreparation>;

    async fn activate(
        &self,
        run_id: &RunId,
        preparation: RunPreparation,
    ) -> anyhow::Result<RunActivation>;

    async fn stop(&self, run_id: &RunId, activation: &mut RunActivation) -> anyhow::Result<()>;
}

#[async_trait]
pub trait RunControl: Send + Sync + 'static {
    async fn list(&self, request: RunListRequest) -> anyhow::Result<Vec<RunView>>;
    async fn get(&self, request: RunGetRequest) -> anyhow::Result<Option<RunView>>;
    async fn status(&self, request: RunStatusRequest) -> anyhow::Result<RunStatusView>;
    async fn start(&self, request: RunStartRequest) -> anyhow::Result<RunStartResult>;
    async fn stop(&self, request: RunStopRequest) -> anyhow::Result<RunMutationResult>;

    /// Host-internal notification that an activated Package transport was lost.
    /// This is deliberately absent from the public wire contract: Package and
    /// Shell principals cannot manufacture lifecycle authority through it.
    async fn package_activation_lost(
        &self,
        package_id: &PackageId,
        run_ids: Vec<RunId>,
    ) -> anyhow::Result<()>;
}

#[derive(Debug, Default)]
pub struct UnavailableRunControl;

#[async_trait]
impl RunControl for UnavailableRunControl {
    async fn list(&self, _request: RunListRequest) -> anyhow::Result<Vec<RunView>> {
        anyhow::bail!("Run control unavailable")
    }

    async fn get(&self, _request: RunGetRequest) -> anyhow::Result<Option<RunView>> {
        anyhow::bail!("Run control unavailable")
    }

    async fn status(&self, _request: RunStatusRequest) -> anyhow::Result<RunStatusView> {
        anyhow::bail!("Run control unavailable")
    }

    async fn start(&self, request: RunStartRequest) -> anyhow::Result<RunStartResult> {
        request.validate()?;
        anyhow::bail!("Run control unavailable")
    }

    async fn stop(&self, request: RunStopRequest) -> anyhow::Result<RunMutationResult> {
        request.validate()?;
        anyhow::bail!("Run control unavailable")
    }

    async fn package_activation_lost(
        &self,
        _package_id: &PackageId,
        _run_ids: Vec<RunId>,
    ) -> anyhow::Result<()> {
        anyhow::bail!("Run control unavailable")
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    struct CurrentGrant {
        current: AtomicBool,
    }

    #[async_trait]
    impl RunAuthorityValidator for CurrentGrant {
        async fn validate_current(
            &self,
            _grant_id: &str,
            _installation_id: &InstallationId,
            _run_id: Option<&RunId>,
        ) -> anyhow::Result<()> {
            anyhow::ensure!(self.current.load(Ordering::SeqCst), "revoked");
            Ok(())
        }
    }

    #[tokio::test]
    async fn mutation_authority_is_exact_unwireable_and_revalidated() -> anyhow::Result<()> {
        let installation_id = InstallationId::new();
        let other = InstallationId::new();
        let validator = Arc::new(CurrentGrant {
            current: AtomicBool::new(true),
        });
        let authority = RunMutationAuthority::verified_for_installation(
            installation_id.clone(),
            Some("grant".to_string()),
            Some(chrono::Utc::now().timestamp_millis() + 60_000),
            Some(RunAuthorityRefresh::new(validator.clone())),
        )?;
        authority
            .refresh_current_for_installation(&installation_id)
            .await?;
        assert!(authority
            .refresh_current_for_installation(&other)
            .await
            .is_err());
        validator.current.store(false, Ordering::SeqCst);
        assert!(authority
            .refresh_current_for_installation(&installation_id)
            .await
            .is_err());

        validator.current.store(true, Ordering::SeqCst);
        let run_id = RunId::new();
        let run_authority = RunMutationAuthority::verified_for_run(
            installation_id.clone(),
            run_id.clone(),
            Some("grant".to_string()),
            Some(chrono::Utc::now().timestamp_millis() + 60_000),
            Some(RunAuthorityRefresh::new(validator)),
        )?;
        run_authority
            .refresh_current_for_run(&installation_id, &run_id)
            .await?;
        assert!(run_authority
            .refresh_current_for_run(&installation_id, &RunId::new())
            .await
            .is_err());
        assert!(run_authority
            .refresh_current_for_installation(&installation_id)
            .await
            .is_err());
        Ok(())
    }

    #[test]
    fn device_authority_requires_verified_unexpired_transport_fact() {
        assert!(RunMutationAuthority::verified_for_installation(
            InstallationId::new(),
            Some("grant".to_string()),
            Some(chrono::Utc::now().timestamp_millis() - 1),
            None,
        )
        .is_err());
    }
}
