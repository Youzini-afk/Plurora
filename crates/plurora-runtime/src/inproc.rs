use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use plurora_core::{CapHandleId, CapabilityId, PackageId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::runtime::HandleTable;
use crate::{
    CapabilityInvocationRequest, CapabilityInvocationResult, EventStore, ProjectRegistry, Runtime,
};

mod agentic_forge_lab;
mod capability_tool_bridge_lab;
mod common;
mod context_lab;
mod docker_runtime_lab;
pub use docker_runtime_lab::{
    prepare_docker_build_context, DockerDeploymentReconcileSource, PreparedDockerBuildContext,
};
mod experience_observability_lab;
mod experience_runtime_lab;
mod git_tools_lab;
mod inference_local_lab;
mod inference_playtest_lab;
mod install_lab;
mod integrity_lab;
mod knowledge_lab;
mod memory_lab;
mod model_connector_lab;
mod model_provider_lab;
mod model_routing_lab;
mod persona_lab;
mod pi_agent_runtime_lab;
mod playable_creation_board;
mod playable_seed;
mod project_intake_lab;
mod projection_lab;
pub mod safety;
mod secret_store_lab;
mod sharing_lab;
mod storage_lab;
mod tdb_retrieval_lab;
mod text_transform_lab;
mod thirdparty_agent_runtime;
mod workspace_lab;

pub use integrity_lab::{compute_external_workspace_tree_hash, WorkspaceTreeHash};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InprocInvocation {
    pub capability_id: CapabilityId,
    pub provider_package_id: PackageId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub input: Value,
}

#[derive(Clone)]
pub struct ComponentEnv {
    pub package_id: PackageId,
    pub component_id: String,
    pub component_digest: String,
    pub bindings: HashMap<String, CapHandleId>,
    pub handles: Arc<HandleTable>,
}

#[async_trait]
pub trait InprocPackage: Send + Sync {
    fn init(&self, _env: &ComponentEnv) {}

    async fn invoke(&self, request: InprocInvocation) -> anyhow::Result<Value>;
}

pub trait InprocCapabilityInvoker: Send + Sync {
    fn invoke_capability(
        &self,
        request: CapabilityInvocationRequest,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<CapabilityInvocationResult>> + Send>>;

    fn project_registry(&self) -> Option<Arc<ProjectRegistry>> {
        None
    }

    fn append_platform_event(
        &self,
        _session_id: &str,
        _kind: &'static str,
        _payload: Value,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> {
        Box::pin(async { Ok(()) })
    }
}

struct RuntimeInprocInvoker<S>
where
    S: EventStore,
{
    runtime: Runtime<S>,
    session_id: Option<String>,
}

impl<S> InprocCapabilityInvoker for RuntimeInprocInvoker<S>
where
    S: EventStore,
{
    fn invoke_capability(
        &self,
        mut request: CapabilityInvocationRequest,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<CapabilityInvocationResult>> + Send>> {
        let runtime = self.runtime.clone();
        if request.session_id.is_none() {
            request.session_id = self.session_id.clone();
        }
        Box::pin(async move { runtime.invoke_capability(request).await })
    }

    fn project_registry(&self) -> Option<Arc<ProjectRegistry>> {
        Some(self.runtime.config().project_registry.clone())
    }

    fn append_platform_event(
        &self,
        session_id: &str,
        kind: &'static str,
        payload: Value,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> {
        let runtime = self.runtime.clone();
        let session_id = session_id.to_string();
        Box::pin(async move {
            runtime
                .append_platform_event(&session_id, kind, payload)
                .await
                .map(|_| ())
        })
    }
}

tokio::task_local! {
    static INPROC_INVOKER: Arc<dyn InprocCapabilityInvoker>;
}

pub(crate) async fn with_runtime_invoker<S, F, T>(
    runtime: Runtime<S>,
    session_id: Option<String>,
    future: F,
) -> T
where
    S: EventStore,
    F: Future<Output = T>,
{
    INPROC_INVOKER
        .scope(
            Arc::new(RuntimeInprocInvoker {
                runtime,
                session_id,
            }),
            future,
        )
        .await
}

pub(crate) async fn invoke_capability_from_inproc(
    request: CapabilityInvocationRequest,
) -> anyhow::Result<CapabilityInvocationResult> {
    let invoker = INPROC_INVOKER
        .try_with(Clone::clone)
        .map_err(|_| anyhow::anyhow!("inproc runtime invocation context is unavailable"))?;
    invoker.invoke_capability(request).await
}

pub(crate) fn project_registry_from_inproc() -> anyhow::Result<Arc<ProjectRegistry>> {
    let invoker = INPROC_INVOKER
        .try_with(Clone::clone)
        .map_err(|_| anyhow::anyhow!("inproc runtime invocation context is unavailable"))?;
    invoker
        .project_registry()
        .ok_or_else(|| anyhow::anyhow!("inproc project registry context is unavailable"))
}

pub(crate) async fn append_platform_event_from_inproc(
    session_id: &str,
    kind: &'static str,
    payload: Value,
) -> anyhow::Result<()> {
    let invoker = INPROC_INVOKER
        .try_with(Clone::clone)
        .map_err(|_| anyhow::anyhow!("inproc runtime invocation context is unavailable"))?;
    invoker
        .append_platform_event(session_id, kind, payload)
        .await
}

pub use install_lab::StoreSchemaMigration;

pub fn ensure_install_lab_store_schema(
    data_dir: &Path,
) -> anyhow::Result<Option<StoreSchemaMigration>> {
    install_lab::ensure_store_schema(data_dir)
}

#[derive(Clone, Default)]
pub struct InprocPackageCatalog {
    entries: Arc<HashMap<String, Arc<dyn InprocPackage>>>,
}

impl InprocPackageCatalog {
    pub fn with_default_examples() -> Self {
        let mut entries: HashMap<String, Arc<dyn InprocPackage>> = HashMap::new();
        entries.insert(
            entry_key("example-echo-rust-inproc", "register"),
            Arc::new(EchoInprocPackage),
        );
        entries.insert(
            entry_key("example-hook-inproc", "register"),
            Arc::new(HookInprocPackage),
        );
        entries.insert(
            entry_key("example-bindings-inproc", "register"),
            Arc::new(BindingsInprocPackage::default()),
        );
        entries.insert(
            entry_key("plurora-foundation", "register"),
            Arc::new(FirstPartyFoundationPackage),
        );
        entries.insert(
            entry_key("plurora-install-lab", "plurora_install_lab"),
            Arc::new(FirstPartyFoundationPackage),
        );
        entries.insert(
            entry_key("plurora-secret-store-lab", "plurora_secret_store_lab"),
            Arc::new(FirstPartyFoundationPackage),
        );
        entries.insert(
            entry_key("example-thirdparty-agent-runtime", "register"),
            Arc::new(ThirdpartyAgentRuntimePackage),
        );
        Self {
            entries: Arc::new(entries),
        }
    }

    pub fn lookup(&self, crate_ref: &str, symbol: &str) -> Option<Arc<dyn InprocPackage>> {
        self.entries.get(&entry_key(crate_ref, symbol)).cloned()
    }
}

fn entry_key(crate_ref: &str, symbol: &str) -> String {
    format!("{crate_ref}::{symbol}")
}

struct EchoInprocPackage;

#[async_trait]
impl InprocPackage for EchoInprocPackage {
    async fn invoke(&self, request: InprocInvocation) -> anyhow::Result<Value> {
        Ok(request.input)
    }
}

struct HookInprocPackage;

#[derive(Default)]
struct BindingsInprocPackage {
    bindings: std::sync::Mutex<HashMap<String, CapHandleId>>,
}

#[async_trait]
impl InprocPackage for BindingsInprocPackage {
    fn init(&self, env: &ComponentEnv) {
        *self.bindings.lock().expect("bindings mutex poisoned") = env.bindings.clone();
    }

    async fn invoke(&self, request: InprocInvocation) -> anyhow::Result<Value> {
        if request.capability_id.ends_with("/bindings") {
            let bindings = self.bindings.lock().expect("bindings mutex poisoned");
            Ok(serde_json::to_value(&*bindings)?)
        } else {
            Ok(request.input)
        }
    }
}

#[async_trait]
impl InprocPackage for HookInprocPackage {
    async fn invoke(&self, request: InprocInvocation) -> anyhow::Result<Value> {
        if request.capability_id.ends_with("/veto") {
            Ok(serde_json::json!({"decision": "veto", "reason": "hook package veto"}))
        } else if request.capability_id.ends_with("/trace") {
            Ok(
                serde_json::json!({"decision": "allow", "metadata_patch": {"hook_trace": request.provider_package_id}}),
            )
        } else {
            Ok(serde_json::json!({"decision": "allow"}))
        }
    }
}

/// Dispatch an first-party Package invocation using provider-package indexed dispatch.
///
/// Each `plurora/*` package is dispatched directly based on `provider_package_id`,
/// falling through to `common::try_handle` when the specific handler returns `None`
/// or when the package_id is an unknown first-party Package.
/// Third-party Packages are never served by `common::try_handle`.
async fn dispatch_first_party(mut request: InprocInvocation) -> anyhow::Result<Value> {
    // Try the package-specific handler first, then fall through to common
    // namespace-scoped handlers if the specific handler doesn't match.
    if request.provider_package_id == "plurora/install-lab" {
        if let Some(result) = install_lab::try_handle(&mut request).await {
            return result;
        }
    }

    let specific_result = match request.provider_package_id.as_str() {
        "plurora/persona-lab" => persona_lab::try_handle(&request),
        "plurora/knowledge-lab" => knowledge_lab::try_handle(&request),
        "plurora/context-lab" => context_lab::try_handle(&request),
        "plurora/docker-runtime-lab" => docker_runtime_lab::try_handle(&request),
        "plurora/text-transform-lab" => text_transform_lab::try_handle(&request),
        "plurora/model-connector-lab" => model_connector_lab::try_handle(&request),
        "plurora/model-provider-lab" => model_provider_lab::try_handle(&request),
        "plurora/model-routing-lab" => model_routing_lab::try_handle(&request),
        "plurora/pi-agent-runtime-lab" => pi_agent_runtime_lab::try_handle(&request),
        "plurora/agentic-forge-lab" => agentic_forge_lab::try_handle(&request),
        // projection-lab /diff must be tried before the generic /diff
        "plurora/projection-lab" => projection_lab::try_handle(&request),
        // playable-seed handlers checked before generic capability suffixes
        "plurora/playable-seed" => playable_seed::try_handle(&request),
        // capability-tool-bridge-lab handlers checked before generic capability suffixes
        "plurora/capability-tool-bridge-lab" => capability_tool_bridge_lab::try_handle(&request),
        "plurora/inference-local-lab" => inference_local_lab::try_handle(&request),
        "plurora/inference-playtest-lab" => inference_playtest_lab::try_handle(&request),
        "plurora/experience-runtime-lab" => experience_runtime_lab::try_handle(&request),
        "plurora/playable-creation-board" => playable_creation_board::try_handle(&request),
        "plurora/memory-lab" => memory_lab::try_handle(&request),
        "plurora/experience-observability-lab" => {
            experience_observability_lab::try_handle(&request)
        }
        "plurora/sharing-lab" => sharing_lab::try_handle(&request),
        "plurora/storage-lab" => storage_lab::try_handle(&request),
        "plurora/tdb-retrieval-lab" => tdb_retrieval_lab::try_handle(&request),
        "plurora/project-intake-lab" => project_intake_lab::try_handle(&request),
        "plurora/workspace-lab" => workspace_lab::try_handle(&request),
        "plurora/git-tools-lab" => git_tools_lab::try_handle(&request),
        "plurora/integrity-lab" => integrity_lab::try_handle(&request),
        "plurora/secret-store-lab" => secret_store_lab::try_handle(&request),
        _ => None,
    };

    if let Some(result) = specific_result {
        return result;
    }

    // Fall through to common namespace-scoped handlers for any first-party Package.
    // Third-party Packages are rejected by common::try_handle (it checks the prefix).
    if let Some(result) = common::try_handle(&mut request) {
        return result;
    }

    // No handler matched — fail loudly
    common::unhandled_capability(&request)
}

struct FirstPartyFoundationPackage;

#[async_trait]
impl InprocPackage for FirstPartyFoundationPackage {
    async fn invoke(&self, request: InprocInvocation) -> anyhow::Result<Value> {
        dispatch_first_party(request).await
    }
}

struct ThirdpartyAgentRuntimePackage;

#[async_trait]
impl InprocPackage for ThirdpartyAgentRuntimePackage {
    async fn invoke(&self, request: InprocInvocation) -> anyhow::Result<Value> {
        if let Some(result) = thirdparty_agent_runtime::try_handle(&request) {
            return result;
        }
        common::unhandled_capability(&request)
    }
}
