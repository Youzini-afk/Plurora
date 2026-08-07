use std::collections::BTreeMap;
use std::fs;

use plurora_core::project::{
    ExternalProjectData, ProjectDescriptor, ProjectId, ProjectInner, ProjectType, SecretPolicy,
};
use plurora_core::SessionStatus;
use plurora_runtime::{
    CompositeSecretResolver, EventStore, ProjectStoreSecretResolver, ProtocolContext,
    RuntimeConfig, SecretResolverConfig, StoreSecretResolver, ACTIVE_PROJECT_SCOPE,
};
use serde_json::json;

use super::fixtures::*;

fn descriptor(id: &str) -> ProjectDescriptor {
    ProjectDescriptor {
        schema_version: 1,
        project: ProjectInner {
            id: ProjectId::new(id).expect("valid id"),
            title: id.to_string(),
            description: "protocol project test".to_string(),
            project_type: ProjectType::ExternalWorkspace,
            icon: None,
            entry_surface_id: Some("plurora/workspace-lab/workspace_view".to_string()),
            packages: vec![],
            optional_packages: vec![],
            required_surfaces: vec![],
            required_capabilities: vec![],
            secret_policy: SecretPolicy::default(),
            external: Some(ExternalProjectData {
                source: "/tmp/protocol-project".to_string(),
                source_ref: None,
                adapter_manifest: None,
                workspace_root: Some("/tmp/protocol-project".to_string()),
                source_kind: Some(plurora_core::project::ExternalSourceKind::Local),
                workspace_ownership: Some(
                    plurora_core::project::ExternalWorkspaceOwnership::LinkedLocal,
                ),
                source_digest: None,
            }),
            metadata: BTreeMap::new(),
        },
    }
}

fn project_secret_runtime(
) -> anyhow::Result<plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>> {
    let store = std::sync::Arc::new(plurora_runtime::InMemoryEventStore::default());
    let platform = std::sync::Arc::new(StoreSecretResolver::new()?);
    let project_resolver = ProjectStoreSecretResolver::new(|| {
        ACTIVE_PROJECT_SCOPE.try_with(|scope| scope.clone()).ok()
    })
    .with_platform_fallback(platform.clone());
    let resolver = CompositeSecretResolver::new()
        .with_store(platform)
        .with_project(std::sync::Arc::new(project_resolver));
    Ok(plurora_runtime::Runtime::new(
        store,
        RuntimeConfig {
            secret_resolver: SecretResolverConfig::with_resolver(std::sync::Arc::new(resolver)),
            ..RuntimeConfig::default()
        },
    ))
}

pub(crate) async fn project_list_returns_registered_projects() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .config()
        .project_registry
        .register(descriptor("proto-list-one__abc12345"))?;
    runtime
        .config()
        .project_registry
        .register(descriptor("proto-list-two__def67890"))?;
    let value = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.project.list",
            json!({}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let projects = value["projects"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("projects missing"))?;
    anyhow::ensure!(projects
        .iter()
        .any(|p| p["id"] == json!("proto-list-one__abc12345")));
    anyhow::ensure!(projects
        .iter()
        .any(|p| p["id"] == json!("proto-list-two__def67890")));
    Ok(())
}

pub(crate) async fn project_get_returns_full_descriptor() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .config()
        .project_registry
        .register(descriptor("proto-get__abc12345"))?;
    let value = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.project.get",
            json!({"project_id":"proto-get__abc12345"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(value["project"]["id"] == json!("proto-get__abc12345"));
    anyhow::ensure!(value["state"] == json!("installed"));
    anyhow::ensure!(!value.get("paths").is_some_and(|paths| paths.is_object()));
    anyhow::ensure!(value["storage_summary"].is_object());
    anyhow::ensure!(value["storage_summary"].get("total_bytes").is_some());
    anyhow::ensure!(value["storage_summary"].get("measured_at").is_some());
    Ok(())
}

pub(crate) async fn project_start_transitions_state() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .config()
        .project_registry
        .register(descriptor("proto-start__abc12345"))?;
    let value = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.project.start",
            json!({"project_id":"proto-start__abc12345"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(value["previous_state"] == json!("installed"));
    anyhow::ensure!(value["new_state"] == json!("running"));
    anyhow::ensure!(value["session_id"].as_str().is_some());
    anyhow::ensure!(value["already_running"] == json!(false));
    let list = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.project.list",
            json!({}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let project = list["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == json!("proto-start__abc12345"))
        .unwrap();
    anyhow::ensure!(project["state"] == json!("running"));
    Ok(())
}

pub(crate) async fn project_start_returns_session_id() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .config()
        .project_registry
        .register(descriptor("proto-start-session__abc12345"))?;
    let value = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.project.start",
            json!({"project_id":"proto-start-session__abc12345"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let session_id = value["session_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("session_id missing"))?;
    let session = runtime
        .get_session(session_id)
        .await
        .ok_or_else(|| anyhow::anyhow!("session missing"))?;
    anyhow::ensure!(session.metadata["project_id"] == json!("proto-start-session__abc12345"));
    anyhow::ensure!(session
        .labels
        .contains(&"project:proto-start-session__abc12345".to_string()));
    Ok(())
}

pub(crate) async fn project_start_idempotent_returns_existing_session() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .config()
        .project_registry
        .register(descriptor("proto-start-idempotent__abc12345"))?;
    let first = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.project.start",
            json!({"project_id":"proto-start-idempotent__abc12345"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let second = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.project.start",
            json!({"project_id":"proto-start-idempotent__abc12345"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(first["session_id"] == second["session_id"]);
    anyhow::ensure!(second["already_running"] == json!(true));
    Ok(())
}

pub(crate) async fn project_session_metadata_carries_project_id() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .config()
        .project_registry
        .register(descriptor("proto-session-meta__abc12345"))?;
    let started = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.project.start",
            json!({"project_id":"proto-session-meta__abc12345"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let session_id = started["session_id"].as_str().unwrap();
    let fetched = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "context.get",
            json!({"session_id": session_id}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(fetched["metadata"]["project_id"] == json!("proto-session-meta__abc12345"));
    anyhow::ensure!(fetched["status"] == json!("open"));
    Ok(())
}

pub(crate) async fn project_stop_closes_session() -> anyhow::Result<()> {
    let runtime = project_secret_runtime()?;
    runtime
        .config()
        .project_registry
        .register(descriptor("proto-stop-session__abc12345"))?;
    let started = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.project.start",
            json!({"project_id":"proto-stop-session__abc12345"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let session_id = started["session_id"].as_str().unwrap().to_string();
    let stopped = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.project.stop",
            json!({"project_id":"proto-stop-session__abc12345"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(stopped["session_id"] == json!(session_id));
    let session = runtime
        .get_session(&session_id)
        .await
        .ok_or_else(|| anyhow::anyhow!("session missing"))?;
    anyhow::ensure!(session.status == SessionStatus::Closed);
    let err = runtime
        .resolve_secret_ref_with_session("secret_ref:project:API_KEY", Some(&session_id))
        .await
        .expect_err("closed project session should not resolve project secrets");
    anyhow::ensure!(err.to_string().contains("closed"));
    Ok(())
}

pub(crate) async fn project_get_returns_running_session_id() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .config()
        .project_registry
        .register(descriptor("proto-get-session__abc12345"))?;
    let started = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.project.start",
            json!({"project_id":"proto-get-session__abc12345"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let got = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.project.get",
            json!({"project_id":"proto-get-session__abc12345"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(got["running_session_id"] == started["session_id"]);
    let status = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.project.status",
            json!({"project_id":"proto-get-session__abc12345"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(status["running_session_id"] == started["session_id"]);
    Ok(())
}

pub(crate) async fn project_methods_require_admin_principal() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let err = runtime
        .call_protocol(
            &ProtocolContext::package("example/not-admin", "conformance"),
            "host.project.list",
            json!({}),
        )
        .await
        .expect_err("package principal should be denied");
    anyhow::ensure!(err.code == "runtime/error/permission_denied");
    Ok(())
}

pub(crate) async fn project_lifecycle_event_emitted_on_start() -> anyhow::Result<()> {
    let (store, runtime) = runtime();
    runtime
        .config()
        .project_registry
        .register(descriptor("proto-event__abc12345"))?;
    let started = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.project.start",
            json!({"project_id":"proto-event__abc12345"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let session_id = started["session_id"].as_str().unwrap().to_string();
    let events = store.list_session(&session_id).await?;
    anyhow::ensure!(events
        .iter()
        .any(|event| event.kind == plurora_core::PROJECT_STARTED
            && event.payload["project_id"] == json!("proto-event__abc12345")));
    Ok(())
}

pub(crate) async fn surface_resolve_via_dev_path() -> anyhow::Result<()> {
    let store = std::sync::Arc::new(plurora_runtime::InMemoryEventStore::default());
    let mut config = RuntimeConfig::default();
    config.surface_dev_paths.insert(
        "ydltavern".to_string(),
        "/tmp/ydltavern-surface-dist".to_string(),
    );
    let runtime = plurora_runtime::Runtime::new(store, config);
    let value = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.surface.bundle.resolve",
            json!({"surface_id":"ydltavern/play"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(value["source"] == json!("dev_path"));
    anyhow::ensure!(value["bundle_url"]
        .as_str()
        .unwrap_or_default()
        .contains("/surface-bundles/ydltavern/"));
    anyhow::ensure!(value["export_name"] == json!("mountTavernPlaySurface"));
    Ok(())
}

pub(crate) async fn surface_resolve_via_installed_project() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let _env = EnvGuard::set("PLURORA_DATA_DIR", tmp.path().display().to_string());
    let (_store, runtime) = runtime();
    let project_id = ProjectId::new("surface-project__abc12345")?;
    let project_dir = plurora_core::paths::project_dir(&project_id)?;
    fs::create_dir_all(project_dir.join("dist/styles"))?;
    fs::write(
        project_dir.join("dist/bundle.mjs"),
        "export function mountSurface(){}",
    )?;
    fs::write(
        project_dir.join("dist/styles/surface.css"),
        ".plurora-surface{color:red}",
    )?;
    fs::write(
        project_dir.join("dist/styles/mobile.css"),
        "@media(max-width: 800px){}",
    )?;
    runtime
        .config()
        .project_registry
        .register(descriptor(project_id.as_str()))?;
    let value = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.surface.bundle.resolve",
            json!({"surface_id":"plurora/workspace-lab/workspace_view"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(value["source"] == json!("installed_project"));
    anyhow::ensure!(value["project_id"] == json!("surface-project__abc12345"));
    anyhow::ensure!(value["bundle_url"]
        .as_str()
        .unwrap_or_default()
        .contains("/surface-bundles/projects/surface-project__abc12345/"));
    anyhow::ensure!(value["bundle_url"]
        .as_str()
        .unwrap_or_default()
        .contains("?v="));
    anyhow::ensure!(value["wrapper_class"] == json!("plurora-surface"));
    let stylesheets = value["stylesheets"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("stylesheets missing"))?;
    anyhow::ensure!(
        stylesheets.len() == 2,
        "expected surface and mobile stylesheets: {stylesheets:?}"
    );
    anyhow::ensure!(stylesheets[0]
        .as_str()
        .unwrap_or_default()
        .contains("/surface-bundles/projects/surface-project__abc12345/styles/surface.css?v="));
    anyhow::ensure!(stylesheets[1]
        .as_str()
        .unwrap_or_default()
        .contains("/surface-bundles/projects/surface-project__abc12345/styles/mobile.css?v="));
    Ok(())
}

struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: String) -> Self {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}

pub(crate) async fn surface_resolve_unknown_fails() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let err = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.surface.bundle.resolve",
            json!({"surface_id":"unknown/surface"}),
        )
        .await
        .expect_err("unknown surface should fail closed");
    anyhow::ensure!(err.message.contains("surface_not_found"));
    Ok(())
}

pub(crate) async fn surface_resolve_admin_principal_required() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let err = runtime
        .call_protocol(
            &ProtocolContext::package("example/not-admin", "conformance"),
            "host.surface.bundle.resolve",
            json!({"surface_id":"ydltavern/play"}),
        )
        .await
        .expect_err("package principal should be denied");
    anyhow::ensure!(err.code == "runtime/error/permission_denied");
    Ok(())
}
