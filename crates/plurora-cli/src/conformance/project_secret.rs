//! Conformance tests for project-scoped secret resolution.

use std::path::PathBuf;
use std::sync::Arc;

use plurora_core::project::{
    ProjectDescriptor, ProjectId, ProjectInner, ProjectType, SecretPolicy,
};
use plurora_core::{
    CapabilityDescriptor, EntryDescriptor, NetworkDeclaration, NetworkPermissions, PackageEntry,
    PackageManifest, PermissionSet, SandboxPolicy,
};
use plurora_runtime::{
    CapabilityInvocationRequest, CompositeSecretResolver, FakeOutboundExecutor, InMemoryEventStore,
    OpenSessionRequest, OutboundExecutePolicyConfig, OutboundExecutorConfig,
    ProjectStoreSecretResolver, ProtocolContext, Runtime, RuntimeConfig, SecretResolverConfig,
    StoreSecretResolver, ACTIVE_PROJECT_SCOPE,
};
use serde_json::json;
use tempfile::TempDir;
use tokio::sync::Mutex;

use crate::commands::manifest;

const MANIFEST_PATH: &str = "packages/official/secret-store-lab/manifest.yaml";
const SECRET_STORE_PACKAGE_ID: &str = "official/secret-store-lab";

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

struct DataDirGuard {
    previous: Option<String>,
    _tmp: TempDir,
}

impl DataDirGuard {
    fn new() -> anyhow::Result<Self> {
        let tmp = tempfile::tempdir()?;
        let previous = std::env::var("PLURORA_DATA_DIR").ok();
        std::env::set_var("PLURORA_DATA_DIR", tmp.path().display().to_string());
        Ok(Self {
            previous,
            _tmp: tmp,
        })
    }
}

impl Drop for DataDirGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var("PLURORA_DATA_DIR", value),
            None => std::env::remove_var("PLURORA_DATA_DIR"),
        }
    }
}

fn project(
    id: &str,
    fallback_to_platform: bool,
    require_per_project: Vec<String>,
) -> ProjectDescriptor {
    ProjectDescriptor {
        schema_version: 1,
        project: ProjectInner {
            id: ProjectId::new(id).expect("valid test project id"),
            title: id.to_string(),
            description: String::new(),
            project_type: ProjectType::PluroraNative,
            icon: None,
            entry_surface_id: Some(format!("{id}/surface")),
            packages: vec!["packages/test/manifest.yaml".to_string()],
            optional_packages: Vec::new(),
            required_surfaces: Vec::new(),
            required_capabilities: Vec::new(),
            secret_policy: SecretPolicy {
                fallback_to_platform,
                require_per_project,
            },
            external: None,
            metadata: Default::default(),
        },
    }
}

fn project_secret_runtime() -> anyhow::Result<Runtime<InMemoryEventStore>> {
    let store = Arc::new(InMemoryEventStore::default());
    let platform = Arc::new(StoreSecretResolver::new()?);
    let project_resolver = ProjectStoreSecretResolver::new(|| {
        ACTIVE_PROJECT_SCOPE.try_with(|scope| scope.clone()).ok()
    })
    .with_platform_fallback(platform.clone());
    let resolver = CompositeSecretResolver::new()
        .with_store(platform)
        .with_project(Arc::new(project_resolver));
    let config = RuntimeConfig {
        secret_resolver: SecretResolverConfig::with_resolver(Arc::new(resolver)),
        outbound_executor: OutboundExecutorConfig::Custom(Arc::new(FakeOutboundExecutor::new())),
        outbound_execute_policy: OutboundExecutePolicyConfig {
            enabled: true,
            allowed_hosts: vec!["api.example.com".to_string()],
            https_only: true,
            timeout_ms: 30_000,
            allow_redirects: false,
            allow_insecure_loopback_for_tests: false,
        },
        ..RuntimeConfig::default()
    };
    Ok(Runtime::new(store, config))
}

async fn load_secret_store_lab(runtime: &Runtime<InMemoryEventStore>) -> anyhow::Result<()> {
    runtime
        .load_package(manifest::read_manifest(PathBuf::from(MANIFEST_PATH)).await?)
        .await?;
    Ok(())
}

async fn invoke_lab(
    runtime: &Runtime<InMemoryEventStore>,
    cap: &str,
    input: serde_json::Value,
) -> anyhow::Result<plurora_runtime::CapabilityInvocationResult> {
    runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(cap.to_string()),
            caller_package_id: None,
            provider_package_id: Some(SECRET_STORE_PACKAGE_ID.to_string()),
            version: None,
            session_id: None,
            input,
        })
        .await
        .map_err(Into::into)
}

async fn session_for_project(
    runtime: &Runtime<InMemoryEventStore>,
    project_id: &str,
) -> anyhow::Result<String> {
    Ok(runtime
        .open_session(OpenSessionRequest {
            metadata: json!({ "project_id": project_id }),
            ..OpenSessionRequest::default()
        })
        .await?
        .id)
}

fn outbound_package(id: &str, secret_ref: &str) -> PackageManifest {
    PackageManifest {
        schema_version: 1,
        id: id.to_string(),
        version: "0.1.0".to_string(),
        display_name: None,
        description: None,
        author: None,
        license: None,
        entry: EntryDescriptor::v1(PackageEntry::RustInproc {
            crate_ref: "example-echo-rust-inproc".to_string(),
            symbol: "register".to_string(),
            abi_version: 1,
        }),
        provides: vec![CapabilityDescriptor {
            id: format!("{id}/fetch"),
            version: "0.1.0".to_string(),
            input_schema: serde_json::Value::Null,
            output_schema: serde_json::Value::Null,
            streaming: false,
            side_effects: vec!["network".to_string()],
            description: None,
        }],
        consumes: Vec::new(),
        requires: Vec::new(),
        contributes: Default::default(),
        permissions: PermissionSet {
            network: NetworkPermissions {
                declarations: vec![NetworkDeclaration {
                    host: "api.example.com".to_string(),
                    methods: vec!["POST".to_string()],
                    purpose: Some("project secret conformance".to_string()),
                }],
                hosts: Vec::new(),
            },
            secret_refs: vec![secret_ref.to_string()],
            ..PermissionSet::default()
        },
        sandbox_policy: SandboxPolicy::default(),
    }
}

async fn call_outbound(
    runtime: &Runtime<InMemoryEventStore>,
    package_id: &str,
    session_id: Option<String>,
    secret_ref: &str,
) -> anyhow::Result<serde_json::Value> {
    let mut context = ProtocolContext::package(package_id, "conformance");
    context.session_id = session_id;
    runtime
        .call_protocol(
            &context,
            "kernel.v1.outbound.execute",
            json!({
                "capability_id": format!("{package_id}/fetch"),
                "destination_host": "api.example.com",
                "method": "POST",
                "secret_headers": {
                    "Authorization": { "secret_ref": secret_ref, "scheme": "bearer" }
                },
            }),
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.message))
}

pub(crate) async fn put_then_resolve_via_project_ref() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let _guard = DataDirGuard::new()?;
    let rt = project_secret_runtime()?;
    load_secret_store_lab(&rt).await?;
    rt.config()
        .project_registry
        .register(project("proj-put-resolve", true, vec![]))?;
    invoke_lab(
        &rt,
        "official/secret-store-lab/put_project_secret",
        json!({ "project_id": "proj-put-resolve", "name": "API_KEY", "value": "synthetic-project-value" }),
    )
    .await?;
    let session_id = session_for_project(&rt, "proj-put-resolve").await?;
    let resolved = rt
        .resolve_secret_ref_with_session("secret_ref:project:API_KEY", Some(&session_id))
        .await?;
    anyhow::ensure!(resolved == "synthetic-project-value");
    Ok(())
}

pub(crate) async fn fallback_to_platform_when_missing() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let _guard = DataDirGuard::new()?;
    let rt = project_secret_runtime()?;
    load_secret_store_lab(&rt).await?;
    rt.config()
        .project_registry
        .register(project("proj-fallback", true, vec![]))?;
    invoke_lab(
        &rt,
        "official/secret-store-lab/put_secret",
        json!({ "name": "API_KEY", "value": "synthetic-platform-value" }),
    )
    .await?;
    let session_id = session_for_project(&rt, "proj-fallback").await?;
    let resolved = rt
        .resolve_secret_ref_with_session("secret_ref:project:API_KEY", Some(&session_id))
        .await?;
    anyhow::ensure!(resolved == "synthetic-platform-value");
    Ok(())
}

pub(crate) async fn no_fallback_when_disabled() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let _guard = DataDirGuard::new()?;
    let rt = project_secret_runtime()?;
    rt.config()
        .project_registry
        .register(project("proj-no-fallback", false, vec![]))?;
    let session_id = session_for_project(&rt, "proj-no-fallback").await?;
    let err = rt
        .resolve_secret_ref_with_session("secret_ref:project:API_KEY", Some(&session_id))
        .await
        .expect_err("missing project secret without fallback must fail");
    anyhow::ensure!(err.to_string().contains("fallback is disabled"));
    Ok(())
}

pub(crate) async fn require_per_project_blocks_fallback() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let _guard = DataDirGuard::new()?;
    let rt = project_secret_runtime()?;
    load_secret_store_lab(&rt).await?;
    rt.config().project_registry.register(project(
        "proj-require",
        true,
        vec!["API_KEY".to_string()],
    ))?;
    invoke_lab(
        &rt,
        "official/secret-store-lab/put_secret",
        json!({ "name": "API_KEY", "value": "synthetic-platform-value" }),
    )
    .await?;
    let session_id = session_for_project(&rt, "proj-require").await?;
    let err = rt
        .resolve_secret_ref_with_session("secret_ref:project:API_KEY", Some(&session_id))
        .await
        .expect_err("require_per_project must block fallback");
    anyhow::ensure!(err.to_string().contains("required at project scope"));
    Ok(())
}

pub(crate) async fn isolation_between_projects() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let _guard = DataDirGuard::new()?;
    let rt = project_secret_runtime()?;
    load_secret_store_lab(&rt).await?;
    rt.config()
        .project_registry
        .register(project("proj-one", true, vec![]))?;
    rt.config()
        .project_registry
        .register(project("proj-two", true, vec![]))?;
    invoke_lab(
        &rt,
        "official/secret-store-lab/put_project_secret",
        json!({ "project_id": "proj-one", "name": "API_KEY", "value": "value-one" }),
    )
    .await?;
    invoke_lab(
        &rt,
        "official/secret-store-lab/put_project_secret",
        json!({ "project_id": "proj-two", "name": "API_KEY", "value": "value-two" }),
    )
    .await?;
    let session_one = session_for_project(&rt, "proj-one").await?;
    let session_two = session_for_project(&rt, "proj-two").await?;
    let resolved_one = rt
        .resolve_secret_ref_with_session("secret_ref:project:API_KEY", Some(&session_one))
        .await?;
    let resolved_two = rt
        .resolve_secret_ref_with_session("secret_ref:project:API_KEY", Some(&session_two))
        .await?;
    anyhow::ensure!(resolved_one == "value-one");
    anyhow::ensure!(resolved_two == "value-two");
    Ok(())
}

pub(crate) async fn no_session_context_fails_closed() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let _guard = DataDirGuard::new()?;
    let rt = project_secret_runtime()?;
    load_secret_store_lab(&rt).await?;
    rt.config()
        .project_registry
        .register(project("proj-no-session", true, vec![]))?;
    rt.load_package(outbound_package(
        "example/project-secret-no-session",
        "secret_ref:project:API_KEY",
    ))
    .await?;
    invoke_lab(
        &rt,
        "official/secret-store-lab/put_project_secret",
        json!({ "project_id": "proj-no-session", "name": "API_KEY", "value": "synthetic-project-value" }),
    )
    .await?;
    let result = call_outbound(
        &rt,
        "example/project-secret-no-session",
        None,
        "secret_ref:project:API_KEY",
    )
    .await;
    anyhow::ensure!(
        result.is_err(),
        "project outbound without session_id must fail"
    );
    Ok(())
}

pub(crate) async fn list_returns_names_not_values() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let _guard = DataDirGuard::new()?;
    let rt = project_secret_runtime()?;
    load_secret_store_lab(&rt).await?;
    invoke_lab(
        &rt,
        "official/secret-store-lab/put_project_secret",
        json!({ "project_id": "proj-list", "name": "KEY_ONE", "value": "project-secret-one" }),
    )
    .await?;
    invoke_lab(
        &rt,
        "official/secret-store-lab/put_project_secret",
        json!({ "project_id": "proj-list", "name": "KEY_TWO", "value": "project-secret-two" }),
    )
    .await?;
    let list = invoke_lab(
        &rt,
        "official/secret-store-lab/list_project_secrets",
        json!({ "project_id": "proj-list" }),
    )
    .await?;
    let text = serde_json::to_string(&list.output)?;
    anyhow::ensure!(text.contains("KEY_ONE"));
    anyhow::ensure!(text.contains("KEY_TWO"));
    anyhow::ensure!(!text.contains("project-secret-one"));
    anyhow::ensure!(!text.contains("project-secret-two"));
    Ok(())
}
