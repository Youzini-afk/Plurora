use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use plurora_core::{
    ArtifactDescriptor, AssetRecord, EventEnvelope, PackageId, SessionId, SessionRecord,
    SessionStatus, EVENT_ASSET_PUT, EVENT_DEPLOYMENT_RECONCILED, EVENT_EXEC_COMPLETED,
    EVENT_EXEC_DENIED, EVENT_EXEC_FAILED, EVENT_EXEC_STARTED, EVENT_EXEC_STOPPED,
    EVENT_PERMISSION_GRANTED, EVENT_PERMISSION_REVOKED, EVENT_PORT_LEASED, EVENT_PORT_RELEASED,
    EVENT_PROJECTION_UPDATED, EVENT_PROXY_REGISTERED, EVENT_PROXY_UNREGISTERED,
    EVENT_SESSION_FORKED,
};
use plurora_work::{InstallationId, InstallationStatus};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{Mutex, RwLock};

use crate::{
    EventStore, HostPolicy, InMemoryObjectStore, InprocPackageCatalog, InstallationControl,
    InstallationScopeContext, ObjectStore, ProtocolContext, ProtocolPrincipal,
    SecretResolverConfig, UnavailableInstallationControl,
};

mod artifacts;
mod assets;
mod audit;
mod branches;
mod capabilities;
mod effects;
mod events;
mod handles;
mod hooks;
mod local_exec;
mod network;
mod outbound;
mod outbound_sse;
mod outbound_websocket;
mod packages;
mod permissions;
mod projections;
mod proposals;
mod protocol;
mod protocol_dispatch;
mod remote;
mod session;
mod streaming;
mod wasm;
mod world_bundle;

// Re-export public types so old paths like plurora_runtime::runtime::AssetPutRequest keep working.
pub use self::artifacts::{ArtifactCommitRequest, GENERIC_BLOB_ARTIFACT_TYPE_URI};
pub use self::assets::{
    content_address, exact_artifact_upload, legacy_content_address, standard_asset_metadata,
    AssetContentEncoding, AssetGetParams, AssetGetResponse, AssetPutRequest, ExactArtifactUpload,
    InstallationStateArtifactGetParams, InstallationStateArtifactGetResponse, ObjectGetRequest,
    ObjectGetResponse, ObjectPutResponse, ObjectPutScope,
};
pub use self::audit::{
    AuditPackageParams, DeclaredAuthority, PackageAuditReport, TighteningSuggestion,
    UnusedAuthority, UsedAuthority,
};
pub use self::branches::BranchRecord;
pub use self::capabilities::CapabilityReexecutionResult;
pub use self::effects::{EffectReplayResult, EFFECT_RECEIPT_MEDIA_TYPE, EFFECT_VALUE_MEDIA_TYPE};
pub use self::events::{AppendEventRequest, EventListRequest};
pub use self::handles::HandleTable;
pub use self::local_exec::{
    DenyAllLocalExecExecutor, DeploymentReconcileSource, EmptyReconcileSource, ExecCommand, ExecId,
    ExecLifecyclePolicy, ExecRegistry, ExecResourceLimits, ExecStatus, ExecStatusKind,
    ExecutionTarget, ExecutionTargetCapability, ExecutionTargetId, ExecutionTargetObservedSummary,
    ExecutionTargetReachability, ExecutionTargetRegistry, ExecutionTargetStatusKind,
    FakeLocalExecExecutor, LiveLocalExecExecutor, LiveLocalExecExecutorConfig, LocalExecExecutor,
    LocalExecExecutorConfig, LocalExecListResponse, LocalExecLogLine, LocalExecLogStream,
    LocalExecLogsRequest, LocalExecLogsResponse, LocalExecStartRequest, LocalExecStartResponse,
    LocalExecStatusRequest, LocalExecStatusResponse, LocalExecStopRequest, LocalExecStopResponse,
    ManagedContainerReport, PortBindScope, PortLeaseId, PortLeaseRecord, PortLeaseRegistry,
    PortLeaseRequest, PortLeaseResponse, PortLeaseStatusKind, PortProtocol, ProxyProtocol,
    ProxyRouteAccess, ProxyRouteId, ProxyRouteRecord, ProxyRouteRegisterRequest,
    ProxyRouteRegisterResponse, ProxyRouteRegistry, ProxyRouteStatusKind, ProxyRouteUpstream,
    ReadinessProbe, ReadinessProbeKind,
};
pub use self::network::{
    check_network_policy, NetworkPolicyDecision, OutboundExecuteCompletion, OutboundRequest,
    OutboundStreamCompletion, OutboundWebSocketCompletion,
};
pub use self::outbound::{
    is_secret_header_name, is_static_header_allowed, CancelSignal, DenyAllOutboundExecutor,
    ExecutorKind, FakeOutboundExecutor, LiveHttpOutboundExecutor, LiveHttpOutboundExecutorConfig,
    OutboundExecutePolicyConfig, OutboundExecutor, OutboundExecutorConfig, OutboundExecutorRequest,
    OutboundExecutorResponse, OutboundFrameKind, OutboundSecretHeaderSpec, OutboundStaticHeader,
    OutboundStreamFrame, OutboundStreamResponse, OutboundStreamSummary, RedactedHeaderValue,
    ResolvedSecretHeader, SecretHeaderSpec, StaticHeader, StreamEmitter, StreamFormat,
    StreamStartStatus, STATIC_HEADER_ALLOWLIST,
};
pub use self::outbound_sse::{SseEvent, SseParser};
pub use self::outbound_websocket::{
    DenyAllWebSocketExecutor, FakeWebSocketExecutor, FrameDirection, FrameKind,
    LiveWebSocketExecutor, LiveWebSocketExecutorConfig, LiveWebSocketProfile,
    OutboundWebSocketFrame, OutboundWebSocketOpenRequest, OutboundWebSocketSession, SendStatus,
    WebSocketEvent, WebSocketExecutor, WebSocketFramePayload,
};
pub use self::permissions::PermissionGrantRecord;
pub use self::projections::ProjectionDefinition;
pub use self::proposals::{ProposalApproval, ProposalOperation, ProposalRecord, ProposalStatus};
pub use self::session::OpenSessionRequest;
pub use self::streaming::StreamRegistry;
pub use self::world_bundle::{
    audit_world_bundle_archive, replay_world_bundle_archive, verify_world_bundle_archive,
    WorldBundleAuditReport, WorldBundleExportRequest, WorldBundleImportResult,
    WorldBundleReceiptReplay, WorldBundleReplayResult, WorldJournalSelection,
};

tokio::task_local! {
    pub static ACTIVE_INSTALLATION_SCOPE: InstallationScopeContext;
}

// ---------------------------------------------------------------------------
// RuntimeConfig
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct RuntimeConfig {
    pub default_labels: Vec<String>,
    pub host_policy: HostPolicy,
    pub inproc_packages: InprocPackageCatalog,
    pub secret_resolver: SecretResolverConfig,
    /// Content-addressed object storage. Defaults to an in-memory SHA-256 store.
    pub object_store: Arc<dyn ObjectStore>,
    /// Host-owned Installation control plane. The default fails closed.
    pub installation_control: Arc<dyn InstallationControl>,
    /// Outbound executor configuration. Defaults to `DenyAll` (fail-closed).
    pub outbound_executor: OutboundExecutorConfig,
    /// Outbound execute host-level policy. Defaults disabled (fail-closed). (Y1)
    pub outbound_execute_policy: OutboundExecutePolicyConfig,
    /// Outbound WebSocket executor. Defaults to DenyAll (fail-closed).
    pub outbound_websocket_executor: Arc<dyn WebSocketExecutor>,
    /// Local exec executor. Defaults to DenyAll (fail-closed).
    pub local_exec_executor: LocalExecExecutorConfig,
    /// In-memory local exec status registry for Phase 1 fake/deny dispatch.
    pub exec_registry: Arc<ExecRegistry>,
    /// In-memory execution target registry. Defaults with local/local-host.
    pub target_registry: Arc<ExecutionTargetRegistry>,
    /// In-memory loopback-only port lease registry.
    pub port_lease_registry: Arc<PortLeaseRegistry>,
    /// In-memory placeholder proxy route registry.
    pub proxy_route_registry: Arc<ProxyRouteRegistry>,
    /// Restart reconciliation truth source. Defaults empty (fail-safe cleanup).
    pub deployment_reconcile_source: Arc<dyn DeploymentReconcileSource>,
    /// Development-mode surface bundle path overrides. Maps a surface_id prefix
    /// to a filesystem directory containing built bundles.
    pub surface_dev_paths: BTreeMap<String, String>,
    /// Host-local package root hints keyed by package id. Used only when a host
    /// profile loads manifests from disk so relative subprocess commands run
    /// from the manifest's package directory without adding local paths to
    /// protocol manifest payloads.
    pub package_roots: BTreeMap<PackageId, PathBuf>,
}

impl fmt::Debug for RuntimeConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let outbound_executor = match &self.outbound_executor {
            OutboundExecutorConfig::DenyAll => "deny_all",
            OutboundExecutorConfig::Custom(_) => "custom",
            OutboundExecutorConfig::LiveHttp(_) => "live_http",
        };
        let local_exec_executor = match &self.local_exec_executor {
            LocalExecExecutorConfig::DenyAll => "deny_all",
            LocalExecExecutorConfig::Custom(_) => "custom",
            LocalExecExecutorConfig::Fake => "fake",
        };

        formatter
            .debug_struct("RuntimeConfig")
            .field("default_labels", &self.default_labels)
            .field("host_policy", &self.host_policy)
            .field("inproc_packages", &"configured")
            .field("secret_resolver", &"configured")
            .field("object_store", &"configured")
            .field("installation_control", &"configured")
            .field("outbound_executor", &outbound_executor)
            .field("outbound_execute_policy", &self.outbound_execute_policy)
            .field("outbound_websocket_executor", &"configured")
            .field("local_exec_executor", &local_exec_executor)
            .field("exec_registry", &"configured")
            .field("target_registry", &"configured")
            .field("port_lease_registry", &"configured")
            .field("proxy_route_registry", &"configured")
            .field("deployment_reconcile_source", &"configured")
            .field("surface_dev_path_count", &self.surface_dev_paths.len())
            .field("package_root_count", &self.package_roots.len())
            .finish()
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            default_labels: vec!["platform_runtime".to_string()],
            host_policy: HostPolicy::default(),
            inproc_packages: InprocPackageCatalog::with_default_examples(),
            secret_resolver: SecretResolverConfig::default(),
            object_store: Arc::new(InMemoryObjectStore::new()),
            installation_control: Arc::new(UnavailableInstallationControl),
            outbound_executor: OutboundExecutorConfig::default(),
            outbound_execute_policy: OutboundExecutePolicyConfig::default(),
            outbound_websocket_executor: Arc::new(DenyAllWebSocketExecutor),
            local_exec_executor: LocalExecExecutorConfig::default(),
            exec_registry: Arc::new(ExecRegistry::default()),
            target_registry: Arc::new(ExecutionTargetRegistry::default()),
            port_lease_registry: Arc::new(PortLeaseRegistry::default()),
            proxy_route_registry: Arc::new(ProxyRouteRegistry::default()),
            deployment_reconcile_source: Arc::new(EmptyReconcileSource),
            surface_dev_paths: BTreeMap::new(),
            package_roots: BTreeMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// StoredAsset (crate-private)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub(crate) struct StoredAsset {
    pub record: AssetRecord,
    pub content_encoding: AssetContentEncoding,
}

struct HydratedSubstrateState {
    assets: HashMap<String, StoredAsset>,
    branches: HashMap<String, BranchRecord>,
    projections: HashMap<String, ProjectionDefinition>,
    grants: HashMap<String, PermissionGrantRecord>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
pub struct DeploymentReconcileSummary {
    pub execs_failed: usize,
    pub routes_promoted: usize,
    pub routes_removed: usize,
    pub leases_promoted: usize,
    pub leases_released: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
pub struct DeploymentHealthEventPayload {
    pub route_id: ProxyRouteId,
    pub port_lease_id: Option<PortLeaseId>,
    pub previous_ready: bool,
    pub ready: bool,
    pub reason: String,
    pub failure_count: u32,
    pub probe: DeploymentHealthProbe,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
pub struct DeploymentHealthProbe {
    pub kind: String,
}

// ---------------------------------------------------------------------------
// Runtime<S>
// ---------------------------------------------------------------------------

pub struct Runtime<S>
where
    S: EventStore,
{
    pub(crate) store: Arc<S>,
    pub(crate) packages: Arc<crate::PackageRegistry>,
    pub(crate) capabilities: Arc<crate::CapabilityFabric>,
    pub(crate) handles: Arc<HandleTable>,
    pub(crate) extensions: Arc<crate::ExtensionRegistry>,
    pub(crate) subprocesses: Arc<crate::SubprocessSupervisor>,
    pub(crate) sessions: Arc<RwLock<HashMap<SessionId, SessionRecord>>>,
    pub(crate) assets: Arc<RwLock<HashMap<String, StoredAsset>>>,
    pub(crate) projections: Arc<RwLock<HashMap<String, ProjectionDefinition>>>,
    pub(crate) branches: Arc<RwLock<HashMap<String, BranchRecord>>>,
    pub(crate) grants: Arc<RwLock<HashMap<String, PermissionGrantRecord>>>,
    pub(crate) proposals: Arc<RwLock<HashMap<String, ProposalRecord>>>,
    pub(crate) streams: Arc<StreamRegistry>,
    pub(crate) world_bundle_import_lock: Arc<Mutex<()>>,
    pub(crate) config: RuntimeConfig,
}

impl<S> Clone for Runtime<S>
where
    S: EventStore,
{
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            packages: self.packages.clone(),
            capabilities: self.capabilities.clone(),
            handles: self.handles.clone(),
            extensions: self.extensions.clone(),
            subprocesses: self.subprocesses.clone(),
            sessions: self.sessions.clone(),
            assets: self.assets.clone(),
            projections: self.projections.clone(),
            branches: self.branches.clone(),
            grants: self.grants.clone(),
            proposals: self.proposals.clone(),
            streams: self.streams.clone(),
            world_bundle_import_lock: self.world_bundle_import_lock.clone(),
            config: self.config.clone(),
        }
    }
}

impl<S> Runtime<S>
where
    S: EventStore,
{
    pub fn new(store: Arc<S>, config: RuntimeConfig) -> Self {
        Self {
            store,
            packages: Arc::new(crate::PackageRegistry::default()),
            capabilities: Arc::new(crate::CapabilityFabric::default()),
            handles: Arc::new(HandleTable::default()),
            extensions: Arc::new(crate::ExtensionRegistry::default()),
            subprocesses: Arc::new(crate::SubprocessSupervisor::default()),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            assets: Arc::new(RwLock::new(HashMap::new())),
            projections: Arc::new(RwLock::new(HashMap::new())),
            branches: Arc::new(RwLock::new(HashMap::new())),
            grants: Arc::new(RwLock::new(HashMap::new())),
            proposals: Arc::new(RwLock::new(HashMap::new())),
            streams: Arc::new(StreamRegistry::default()),
            world_bundle_import_lock: Arc::new(Mutex::new(())),
            config,
        }
    }

    pub fn store(&self) -> Arc<S> {
        self.store.clone()
    }

    pub fn object_store(&self) -> Arc<dyn ObjectStore> {
        self.config.object_store.clone()
    }

    pub fn packages(&self) -> Arc<crate::PackageRegistry> {
        self.packages.clone()
    }

    pub fn outbound_websocket_executor(&self) -> Arc<dyn WebSocketExecutor> {
        self.config.outbound_websocket_executor.clone()
    }

    pub fn capabilities(&self) -> Arc<crate::CapabilityFabric> {
        self.capabilities.clone()
    }

    pub fn handles(&self) -> Arc<HandleTable> {
        self.handles.clone()
    }

    pub fn extensions(&self) -> Arc<crate::ExtensionRegistry> {
        self.extensions.clone()
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    /// Resolve a secret reference using the configured host secret resolver.
    ///
    /// This is a host-internal method (not a protocol method) for use by
    /// the host during capability invocation. It delegates to
    /// `self.config.secret_resolver.resolver.resolve(ref_id)`.
    ///
    /// Returns the raw secret string on success, or an error if the
    /// reference cannot be resolved. Raw values must never be written
    /// to events, proposals, logs, or audit records.
    pub async fn resolve_secret_ref(&self, ref_id: &str) -> anyhow::Result<String> {
        self.resolve_secret_ref_with_session(ref_id, None).await
    }

    /// Resolve a secret reference with an explicit Installation scope.
    ///
    /// This is used by host-owned brokers that operate on an Installation but do
    /// not have a bound session yet. Raw values must never be written to events,
    /// proposals, logs, or audit records.
    pub async fn resolve_secret_ref_for_installation(
        &self,
        ref_id: &str,
        installation_id: &InstallationId,
    ) -> anyhow::Result<String> {
        if plurora_core::secret_ref::is_installation_backed_ref(ref_id) {
            let scope = self.build_installation_scope(installation_id).await?;
            return self
                .with_active_installation_scope(scope, async {
                    self.config.secret_resolver.resolver.resolve(ref_id).await
                })
                .await;
        }

        self.config.secret_resolver.resolver.resolve(ref_id).await
    }

    /// Resolve a secret reference with optional session context.
    ///
    /// For `secret_ref:installation:NAME`, the `session_id` is used to look up
    /// `metadata.installation_id`, which scopes the resolution.
    pub async fn resolve_secret_ref_with_session(
        &self,
        ref_id: &str,
        session_id: Option<&str>,
    ) -> anyhow::Result<String> {
        if plurora_core::secret_ref::is_installation_backed_ref(ref_id) {
            let installation_id = self.lookup_installation_id_from_session(session_id).await?;
            let scope = self.build_installation_scope(&installation_id).await?;
            return self
                .with_active_installation_scope(scope, async {
                    self.config.secret_resolver.resolver.resolve(ref_id).await
                })
                .await;
        }

        self.config.secret_resolver.resolver.resolve(ref_id).await
    }

    async fn lookup_installation_id_from_session(
        &self,
        session_id: Option<&str>,
    ) -> anyhow::Result<InstallationId> {
        let sid = session_id.ok_or_else(|| {
            anyhow::anyhow!("installation secret resolution requires session_id in context")
        })?;
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(sid)
            .ok_or_else(|| anyhow::anyhow!("session '{}' not found", sid))?;
        if session.status != SessionStatus::Open {
            anyhow::bail!("session '{}' is closed", sid);
        }
        let pid_str = session
            .metadata
            .get("installation_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("session '{}' has no metadata.installation_id", sid))?;
        Ok(InstallationId::parse(pid_str)?)
    }

    pub(crate) async fn ensure_host_session_access(
        &self,
        context: &ProtocolContext,
        action: &str,
        session_id: &str,
    ) -> anyhow::Result<()> {
        if !context.is_host_device() {
            return match context.principal {
                ProtocolPrincipal::HostAdmin | ProtocolPrincipal::HostDev => Ok(()),
                _ => anyhow::bail!("principal is not authenticated as a Host controller"),
            };
        }
        if !context.allows_host_action(action) {
            anyhow::bail!("Host device authority does not include action '{action}'");
        }
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| anyhow::anyhow!("session '{}' not found", session_id))?;
        if session.status != SessionStatus::Open {
            anyhow::bail!("session '{}' is closed", session_id);
        }
        match session
            .metadata
            .get("installation_id")
            .and_then(Value::as_str)
        {
            Some(installation_id)
                if context.allows_host_resource("host", "installation", installation_id) =>
            {
                Ok(())
            }
            Some(installation_id) => anyhow::bail!(
                "Host device authority does not include installation '{}'",
                installation_id
            ),
            None if context.allows_all_host_resources("host", "installation") => Ok(()),
            None => {
                anyhow::bail!("installation-scoped Host device cannot access an unbound session")
            }
        }
    }

    async fn build_installation_scope(
        &self,
        installation_id: &InstallationId,
    ) -> anyhow::Result<InstallationScopeContext> {
        let view = self
            .config
            .installation_control
            .get(installation_id)
            .await
            .map_err(|_| anyhow::anyhow!("installation control unavailable"))?
            .ok_or_else(|| anyhow::anyhow!("installation not found"))?;
        anyhow::ensure!(
            view.record.installation_id == *installation_id
                && view.record.status == InstallationStatus::Ready,
            "installation is not active for secret resolution"
        );
        Ok(InstallationScopeContext {
            installation_id: installation_id.clone(),
            revision: view.revision,
            secret_policy: view.record.secret_policy,
            installation_control: self.config.installation_control.clone(),
        })
    }

    async fn with_active_installation_scope<F, T>(
        &self,
        scope: InstallationScopeContext,
        future: F,
    ) -> T
    where
        F: Future<Output = T>,
    {
        ACTIVE_INSTALLATION_SCOPE.scope(scope, future).await
    }

    pub async fn get_session(&self, session_id: &str) -> Option<SessionRecord> {
        self.sessions.read().await.get(session_id).cloned()
    }

    pub async fn hydrate_substrate_from_events(&self) -> anyhow::Result<()> {
        let events = self.store.list_all().await?;
        let state = self.build_substrate_state(&events).await?;
        self.apply_substrate_state(state).await;
        Ok(())
    }

    async fn build_substrate_state(
        &self,
        events: &[EventEnvelope],
    ) -> anyhow::Result<HydratedSubstrateState> {
        let mut assets = HashMap::new();
        let mut branches = HashMap::new();
        let mut projections = HashMap::new();
        let mut grants = HashMap::new();
        for event in events {
            match event.kind.as_str() {
                EVENT_ASSET_PUT => {
                    let stored = self.hydrate_asset_event(event).await?;
                    assets.insert(stored.record.id.clone(), stored);
                }
                EVENT_SESSION_FORKED => {
                    let branch: BranchRecord = serde_json::from_value(event.payload.clone())?;
                    branches.insert(branch.id.clone(), branch);
                }
                EVENT_PROJECTION_UPDATED => {
                    let projection: ProjectionDefinition =
                        serde_json::from_value(event.payload.clone())?;
                    projections.insert(projection.id.clone(), projection);
                }
                EVENT_PERMISSION_GRANTED => {
                    let record: PermissionGrantRecord =
                        serde_json::from_value(event.payload.clone())?;
                    grants.insert(record.id.clone(), record);
                }
                EVENT_PERMISSION_REVOKED => {
                    let record: PermissionGrantRecord =
                        serde_json::from_value(event.payload.clone())?;
                    // Overwrite with revoked version (revoked_at is set)
                    grants.insert(record.id.clone(), record);
                }
                _ => {}
            }
        }
        Ok(HydratedSubstrateState {
            assets,
            branches,
            projections,
            grants,
        })
    }

    async fn apply_substrate_state(&self, state: HydratedSubstrateState) {
        *self.assets.write().await = state.assets;
        *self.branches.write().await = state.branches;
        *self.projections.write().await = state.projections;
        *self.grants.write().await = state.grants;
    }

    async fn merge_substrate_state(&self, state: HydratedSubstrateState) {
        self.assets.write().await.extend(state.assets);
        self.branches.write().await.extend(state.branches);
        self.projections.write().await.extend(state.projections);
        self.grants.write().await.extend(state.grants);
    }

    pub async fn hydrate_deployment_from_events(&self) -> anyhow::Result<()> {
        let events = self.store.list_all().await?;
        let mut port_leases: HashMap<PortLeaseId, PortLeaseRecord> = HashMap::new();
        let mut proxy_routes: HashMap<ProxyRouteId, ProxyRouteRecord> = HashMap::new();
        let mut executions: HashMap<ExecId, ExecStatus> = HashMap::new();
        let mut exec_effect_contexts: HashMap<ExecId, local_exec::ExecEffectContext> =
            HashMap::new();
        let mut exec_terminal_receipts: HashMap<ExecId, ArtifactDescriptor> = HashMap::new();
        let mut exec_operation_receipts: HashMap<String, ArtifactDescriptor> = HashMap::new();

        for event in events {
            let payload = &event.payload;
            match event.kind.as_str() {
                EVENT_PORT_LEASED => {
                    if let Some(record) = port_lease_record_from_payload(payload) {
                        port_leases.insert(record.id.clone(), record);
                    }
                }
                EVENT_PORT_RELEASED => {
                    if let Some(lease_id) = payload_str(payload, "lease_id") {
                        if let Some(lease) = port_leases.get_mut(&lease_id) {
                            lease.status = PortLeaseStatusKind::Released;
                        }
                    }
                }
                EVENT_PROXY_REGISTERED => {
                    if let Some(record) = proxy_route_record_from_payload(payload) {
                        proxy_routes.insert(record.id.clone(), record);
                    }
                }
                EVENT_PROXY_UNREGISTERED => {
                    if let Some(route_id) = payload_str(payload, "route_id") {
                        if let Some(route) = proxy_routes.get_mut(&route_id) {
                            route.status = ProxyRouteStatusKind::Removed;
                        }
                    }
                }
                EVENT_EXEC_STARTED => {
                    if let Some(status) = exec_status_from_payload(payload) {
                        if let Some(exec_id) = status.exec_id.clone() {
                            if let Some(context) = payload
                                .get("effect_context")
                                .cloned()
                                .and_then(|value| serde_json::from_value(value).ok())
                            {
                                exec_effect_contexts.insert(exec_id.clone(), context);
                            }
                            executions.insert(exec_id, status);
                        }
                    }
                }
                EVENT_EXEC_COMPLETED | EVENT_EXEC_FAILED => {
                    let effect_kind = payload_str(payload, "effect_kind")
                        .unwrap_or_else(|| "exec.run".to_string());
                    if effect_kind == "exec.run" {
                        if let Some(status) = exec_status_from_payload(payload) {
                            if let Some(exec_id) = status.exec_id.clone() {
                                if let Some(receipt) = artifact_descriptor_from_payload(payload) {
                                    exec_terminal_receipts.insert(exec_id.clone(), receipt);
                                }
                                executions.insert(exec_id, status);
                            }
                        }
                    }
                }
                EVENT_EXEC_DENIED => {
                    if let (Some(exec_id), Some(effect_kind), Some(receipt)) = (
                        payload_str(payload, "exec_id"),
                        payload_str(payload, "effect_kind"),
                        artifact_descriptor_from_payload(payload),
                    ) {
                        exec_operation_receipts.insert(format!("{effect_kind}:{exec_id}"), receipt);
                    }
                }
                EVENT_EXEC_STOPPED => {
                    if let Some(exec_id) = payload_str(payload, "exec_id") {
                        let status =
                            executions
                                .entry(exec_id.clone())
                                .or_insert_with(|| ExecStatus {
                                    exec_id: Some(exec_id.clone()),
                                    target_id: None,
                                    kind: ExecStatusKind::Stopped,
                                    exit_code: None,
                                    message: None,
                                    ready: false,
                                });
                        status.kind = ExecStatusKind::Stopped;
                        status.ready = false;
                        if let Some(receipt) = artifact_descriptor_from_payload(payload) {
                            exec_terminal_receipts.insert(exec_id, receipt);
                        }
                    }
                }
                _ => {}
            }
        }

        for mut lease in port_leases.into_values() {
            if lease.status == PortLeaseStatusKind::Active {
                lease.status = PortLeaseStatusKind::Reserved;
            }
            self.config.port_lease_registry.restore(lease).await;
        }

        for mut route in proxy_routes.into_values() {
            if route.status == ProxyRouteStatusKind::Active {
                route.status = ProxyRouteStatusKind::Stale;
            }
            route.ready = false;
            self.config.proxy_route_registry.restore(route).await;
        }

        for mut status in executions.into_values() {
            if matches!(
                status.kind,
                ExecStatusKind::Running | ExecStatusKind::Pending
            ) || status.ready
            {
                status.kind = ExecStatusKind::Unknown;
                status.ready = false;
            }
            self.config.exec_registry.restore(status).await;
        }
        for (exec_id, context) in exec_effect_contexts {
            self.config
                .exec_registry
                .record_effect_context(exec_id, context)
                .await;
        }
        for (exec_id, receipt) in exec_terminal_receipts {
            self.config
                .exec_registry
                .record_terminal_receipt(exec_id, receipt)
                .await;
        }
        for (key, receipt) in exec_operation_receipts {
            self.config
                .exec_registry
                .record_operation_receipt(key, receipt)
                .await;
        }

        Ok(())
    }

    pub async fn reconcile_deployment(&self) -> anyhow::Result<DeploymentReconcileSummary> {
        let reports = self
            .config
            .deployment_reconcile_source
            .list_managed()
            .await?;
        let mut running_by_route: HashMap<String, Vec<ManagedContainerReport>> = HashMap::new();
        for report in reports.into_iter().filter(|report| report.running) {
            running_by_route
                .entry(report.route_id.clone())
                .or_default()
                .push(report);
        }

        let mut summary = DeploymentReconcileSummary {
            execs_failed: self
                .config
                .exec_registry
                .reconcile_unknown_to_failed()
                .await,
            ..DeploymentReconcileSummary::default()
        };

        let routes = self.config.proxy_route_registry.list().await;
        let mut promoted_lease_ids: HashSet<PortLeaseId> = HashSet::new();
        for route in routes {
            if route.status != ProxyRouteStatusKind::Stale {
                continue;
            }
            let remote_target = self
                .config
                .port_lease_registry
                .status(&route.upstream.port_lease_id)
                .await
                .is_some_and(|lease| lease.target_id != "local");
            if remote_target {
                // Host-local Docker observation cannot decide a remote target
                // route. Its durable target-operation receipt owns recovery.
                continue;
            }
            let matching_running_container =
                running_by_route.get(&route.id).is_some_and(|reports| {
                    reports
                        .iter()
                        .any(|report| report.port_lease_id == route.upstream.port_lease_id)
                });
            if matching_running_container {
                if self
                    .config
                    .proxy_route_registry
                    .set_status(&route.id, ProxyRouteStatusKind::Active)
                    .await
                    .is_some()
                {
                    let _ = self
                        .config
                        .proxy_route_registry
                        .set_ready(&route.id, true)
                        .await;
                    summary.routes_promoted += 1;
                    promoted_lease_ids.insert(route.upstream.port_lease_id);
                }
            } else if self
                .config
                .proxy_route_registry
                .set_status(&route.id, ProxyRouteStatusKind::Removed)
                .await
                .is_some()
            {
                summary.routes_removed += 1;
            }
        }

        let leases = self.config.port_lease_registry.list().await;
        for lease in leases {
            if lease.status != PortLeaseStatusKind::Reserved {
                continue;
            }
            if lease.target_id != "local" {
                continue;
            }
            if promoted_lease_ids.contains(&lease.id) {
                if self
                    .config
                    .port_lease_registry
                    .set_status(&lease.id, PortLeaseStatusKind::Active)
                    .await
                    .is_some()
                {
                    summary.leases_promoted += 1;
                }
            } else if self
                .config
                .port_lease_registry
                .set_status(&lease.id, PortLeaseStatusKind::Released)
                .await
                .is_some()
            {
                summary.leases_released += 1;
            }
        }

        self.append_platform_event(
            &"host_deployment_reconcile".to_string(),
            EVENT_DEPLOYMENT_RECONCILED,
            serde_json::to_value(&summary)?,
        )
        .await?;

        Ok(summary)
    }

    // Private helper used across submodules — event-appending via kernel identity.
    pub(crate) async fn append_platform_event(
        &self,
        session_id: &SessionId,
        kind: &'static str,
        payload: Value,
    ) -> anyhow::Result<EventEnvelope> {
        self.append_platform_event_with_metadata(session_id, kind, payload, json!({}))
            .await
    }

    pub(crate) async fn append_platform_event_with_metadata(
        &self,
        session_id: &SessionId,
        kind: &'static str,
        payload: Value,
        metadata: Value,
    ) -> anyhow::Result<EventEnvelope> {
        self.append_event_unchecked(AppendEventRequest {
            session_id: session_id.clone(),
            writer_package_id: plurora_core::PLATFORM_RUNTIME_ID.to_string(),
            kind: kind.to_string(),
            payload,
            metadata,
        })
        .await
    }
}

fn payload_str(payload: &Value, field: &str) -> Option<String> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn payload_u16(payload: &Value, field: &str) -> Option<u16> {
    payload
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
}

fn payload_i32(payload: &Value, field: &str) -> Option<i32> {
    payload
        .get(field)
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
}

fn payload_bool(payload: &Value, field: &str) -> bool {
    payload.get(field).and_then(Value::as_bool).unwrap_or(false)
}

fn enum_from_payload<T>(payload: &Value, field: &str) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    payload
        .get(field)
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
}

fn port_lease_record_from_payload(payload: &Value) -> Option<PortLeaseRecord> {
    Some(PortLeaseRecord {
        id: payload_str(payload, "lease_id")?,
        target_id: payload_str(payload, "target_id")?,
        port_name: payload_str(payload, "port_name")?,
        host: payload_str(payload, "host")?,
        port: payload_u16(payload, "port")?,
        protocol: enum_from_payload(payload, "protocol").unwrap_or(PortProtocol::Tcp),
        bind: enum_from_payload(payload, "bind").unwrap_or(PortBindScope::LoopbackOnly),
        status: enum_from_payload(payload, "status").unwrap_or(PortLeaseStatusKind::Active),
    })
}

fn proxy_route_record_from_payload(payload: &Value) -> Option<ProxyRouteRecord> {
    Some(ProxyRouteRecord {
        id: payload_str(payload, "route_id")?,
        upstream: ProxyRouteUpstream {
            port_lease_id: payload_str(payload, "port_lease_id")?,
            port_name: payload_str(payload, "port_name")?,
        },
        protocol: enum_from_payload(payload, "protocol").unwrap_or(ProxyProtocol::Http),
        access: enum_from_payload(payload, "access").unwrap_or_default(),
        public_url: payload_str(payload, "public_url")?,
        iframe_url: payload_str(payload, "iframe_url")?,
        status: enum_from_payload(payload, "status").unwrap_or(ProxyRouteStatusKind::Active),
        ready: payload_bool(payload, "ready"),
    })
}

fn exec_status_from_payload(payload: &Value) -> Option<ExecStatus> {
    Some(ExecStatus {
        exec_id: Some(payload_str(payload, "exec_id")?),
        target_id: payload_str(payload, "target_id"),
        kind: enum_from_payload(payload, "status").unwrap_or(ExecStatusKind::Unknown),
        exit_code: payload_i32(payload, "exit_code"),
        message: payload_str(payload, "error"),
        ready: payload_bool(payload, "ready"),
    })
}

fn artifact_descriptor_from_payload(payload: &Value) -> Option<ArtifactDescriptor> {
    payload
        .get("receipt")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
}

#[cfg(test)]
mod runtime_config_tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    struct StatusInstallationControl {
        view: crate::InstallationView,
        secret_path_requested: AtomicBool,
    }

    #[async_trait::async_trait]
    impl InstallationControl for StatusInstallationControl {
        async fn list(
            &self,
            _request: crate::InstallationListRequest,
        ) -> anyhow::Result<Vec<crate::InstallationView>> {
            Ok(vec![self.view.clone()])
        }

        async fn get(
            &self,
            _installation_id: &InstallationId,
        ) -> anyhow::Result<Option<crate::InstallationView>> {
            Ok(Some(self.view.clone()))
        }

        async fn create(
            &self,
            _request: crate::InstallationCreateRequest,
        ) -> anyhow::Result<crate::InstallationMutationResult> {
            anyhow::bail!("not used")
        }

        async fn update(
            &self,
            _request: crate::InstallationUpdateRequest,
        ) -> anyhow::Result<crate::InstallationMutationResult> {
            anyhow::bail!("not used")
        }

        async fn remove(
            &self,
            _request: crate::InstallationRemoveRequest,
        ) -> anyhow::Result<crate::InstallationMutationResult> {
            anyhow::bail!("not used")
        }

        fn installation_secret_store_path(
            &self,
            _installation_id: &InstallationId,
        ) -> anyhow::Result<PathBuf> {
            self.secret_path_requested.store(true, Ordering::SeqCst);
            Ok(PathBuf::from("sensitive-secret-root"))
        }
    }

    fn installation_control_with_status(
        status: InstallationStatus,
    ) -> (Arc<StatusInstallationControl>, InstallationId) {
        let installation_id = InstallationId::new();
        let artifact = ArtifactDescriptor {
            artifact_type_uri: "urn:plurora:test:artifact:v1".to_string(),
            media_type: "application/json".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let now = chrono::Utc::now();
        let control = Arc::new(StatusInstallationControl {
            view: crate::InstallationView {
                record: plurora_work::InstallationRecord {
                    schema_version: plurora_work::InstallationRecord::SCHEMA_VERSION,
                    installation_id: installation_id.clone(),
                    work_revision: artifact.clone(),
                    assembly_lock: artifact,
                    display_name: "Runtime status fixture".to_string(),
                    source: plurora_work::AcquisitionRecord {
                        kind: plurora_work::AcquisitionKind::LocalImport,
                        source_ref: None,
                        provenance_refs: Vec::new(),
                        update_channel: None,
                    },
                    state_bindings: Vec::new(),
                    secret_policy: plurora_work::InstallationSecretPolicy::default(),
                    created_at: now,
                    updated_at: now,
                    status,
                },
                revision: 1,
                rollback: None,
            },
            secret_path_requested: AtomicBool::new(false),
        });
        (control, installation_id)
    }

    fn assert_debug_clone_default<T: fmt::Debug + Clone + Default>() {}

    #[test]
    fn runtime_config_supports_debug_clone_and_default_without_exposing_paths() {
        assert_debug_clone_default::<RuntimeConfig>();
        let mut config = RuntimeConfig::default();
        config.package_roots.insert(
            "example/runtime-config".to_string(),
            PathBuf::from("sensitive-package-root"),
        );
        let debug = format!("{config:?}");
        assert!(debug.contains("installation_control"));
        assert!(debug.contains("package_root_count: 1"));
        assert!(!debug.contains("sensitive-package-root"));
        let _cloned = config.clone();
    }

    #[tokio::test]
    async fn installation_secret_scope_captures_only_ready_revision_authority() {
        for status in [
            InstallationStatus::Resolving,
            InstallationStatus::Updating,
            InstallationStatus::Blocked,
            InstallationStatus::Failed,
            InstallationStatus::Removing,
            InstallationStatus::Removed,
        ] {
            let (control, installation_id) = installation_control_with_status(status);
            let runtime = Runtime::new(
                Arc::new(crate::InMemoryEventStore::default()),
                RuntimeConfig {
                    installation_control: control.clone(),
                    ..RuntimeConfig::default()
                },
            );
            let error = runtime
                .build_installation_scope(&installation_id)
                .await
                .expect_err("non-ready Installation must not create a secret scope");
            let message = error.to_string();
            assert_eq!(message, "installation is not active for secret resolution");
            assert!(!message.contains(installation_id.as_str()));
            assert!(!message.contains("sensitive-secret-root"));
            assert!(!control.secret_path_requested.load(Ordering::SeqCst));
        }

        let (control, installation_id) =
            installation_control_with_status(InstallationStatus::Ready);
        let runtime = Runtime::new(
            Arc::new(crate::InMemoryEventStore::default()),
            RuntimeConfig {
                installation_control: control.clone(),
                ..RuntimeConfig::default()
            },
        );
        let scope = runtime
            .build_installation_scope(&installation_id)
            .await
            .expect("Ready Installation creates a revision-bound scope");
        assert_eq!(scope.installation_id, installation_id);
        assert_eq!(scope.revision, 1);
        assert!(Arc::ptr_eq(
            &scope.installation_control,
            &(control.clone() as Arc<dyn InstallationControl>)
        ));
        assert!(
            !control.secret_path_requested.load(Ordering::SeqCst),
            "scope creation must not cache a reusable Installation filesystem path"
        );
    }
}
