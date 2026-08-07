use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use plurora_core::project::{
    ExternalProjectData, ProjectDescriptor, ProjectId, ProjectInner, ProjectState, ProjectType,
    SecretPolicy,
};
use plurora_runtime::{CapabilityInvocationRequest, InMemoryEventStore, Runtime, RuntimeConfig};
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::sync::Mutex;

use crate::commands::{install::OutputFormat, manifest, project, uninstall};

const INSTALL_MANIFEST: &str = "packages/plurora/install-lab/manifest.yaml";
const GIT_MANIFEST: &str = "packages/plurora/git-tools-lab/manifest.yaml";
const INTEGRITY_MANIFEST: &str = "packages/plurora/integrity-lab/manifest.yaml";
const PACKAGE_ID: &str = "plurora/install-lab";

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

struct DataDirGuard {
    previous: Option<String>,
    dir: TempDir,
}

impl DataDirGuard {
    fn new() -> anyhow::Result<Self> {
        let dir = tempfile::tempdir()?;
        let previous = std::env::var("PLURORA_DATA_DIR").ok();
        std::env::set_var("PLURORA_DATA_DIR", dir.path().display().to_string());
        Ok(Self { previous, dir })
    }

    fn path(&self) -> &Path {
        self.dir.path()
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

async fn install_runtime() -> anyhow::Result<Runtime<InMemoryEventStore>> {
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store, RuntimeConfig::default());
    for path in [GIT_MANIFEST, INTEGRITY_MANIFEST, INSTALL_MANIFEST] {
        runtime
            .load_package(manifest::read_manifest(PathBuf::from(path)).await?)
            .await?;
    }
    Ok(runtime)
}

async fn invoke(
    runtime: &Runtime<InMemoryEventStore>,
    cap: &str,
    input: Value,
) -> anyhow::Result<plurora_runtime::CapabilityInvocationResult> {
    runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(cap.to_string()),
            caller_package_id: None,
            provider_package_id: Some(PACKAGE_ID.to_string()),
            version: None,
            session_id: None,
            input,
        })
        .await
        .map_err(Into::into)
}

pub(crate) async fn detect_native_yaml() -> anyhow::Result<()> {
    let rt = install_runtime().await?;
    let tmp = tempfile::tempdir()?;
    write_native_project(tmp.path(), "native-proj__abc12345")?;
    std::fs::write(tmp.path().join("manifest.yaml"), manifest_yaml())?;
    let out = invoke(
        &rt,
        "plurora/install-lab/detect_kind",
        json!({ "path": tmp.path() }),
    )
    .await?;
    anyhow::ensure!(out.output["kind"] == json!("native"));
    anyhow::ensure!(out.output["descriptor"]["project"]["id"] == json!("native-proj__abc12345"));
    Ok(())
}

pub(crate) async fn detect_no_yaml() -> anyhow::Result<()> {
    let rt = install_runtime().await?;
    let tmp = tempfile::tempdir()?;
    std::fs::write(tmp.path().join("manifest.yaml"), manifest_yaml())?;
    let out = invoke(
        &rt,
        "plurora/install-lab/detect_kind",
        json!({ "path": tmp.path() }),
    )
    .await?;
    anyhow::ensure!(out.output["kind"] == json!("external"));
    anyhow::ensure!(out.output["has_manifest_yaml"] == json!(true));
    Ok(())
}

pub(crate) async fn detect_invalid_yaml_rejected() -> anyhow::Result<()> {
    let rt = install_runtime().await?;
    let tmp = tempfile::tempdir()?;
    std::fs::write(tmp.path().join("project.yaml"), "project: [broken")?;
    let err = invoke(
        &rt,
        "plurora/install-lab/detect_kind",
        json!({ "path": tmp.path() }),
    )
    .await
    .expect_err("invalid project.yaml should be rejected");
    anyhow::ensure!(err.to_string().contains("invalid project.yaml"));
    Ok(())
}

pub(crate) async fn register_creates_project_dir() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let guard = DataDirGuard::new()?;
    let rt = install_runtime().await?;
    let descriptor = external_descriptor("reg-proj__abc12345", guard.path());
    let out = invoke(
        &rt,
        "plurora/install-lab/register_project",
        json!({ "descriptor": descriptor, "data_dir": guard.path() }),
    )
    .await?;
    let dir = PathBuf::from(out.output["project_dir"].as_str().context("project_dir")?);
    anyhow::ensure!(dir.is_dir());
    anyhow::ensure!(dir.join("project.yaml").is_file());
    anyhow::ensure!(
        std::fs::read_to_string(dir.join("project.yaml"))?.contains("reg-proj__abc12345")
    );
    Ok(())
}

pub(crate) async fn list_returns_registered() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let guard = DataDirGuard::new()?;
    write_descriptor_to_data_dir(
        guard.path(),
        external_descriptor("list-one__abc12345", guard.path()),
    )?;
    write_descriptor_to_data_dir(
        guard.path(),
        external_descriptor("list-two__def67890", guard.path()),
    )?;
    let rt = Runtime::new(
        Arc::new(InMemoryEventStore::default()),
        RuntimeConfig::default(),
    );
    rt.config().project_registry.load_from_disk()?;
    let ids = rt
        .config()
        .project_registry
        .list()
        .into_iter()
        .map(|entry| entry.descriptor.project.id.to_string())
        .collect::<Vec<_>>();
    anyhow::ensure!(ids.contains(&"list-one__abc12345".to_string()));
    anyhow::ensure!(ids.contains(&"list-two__def67890".to_string()));
    project::run_list(
        project::ProjectListArgs {
            format: OutputFormat::Json,
        },
        Some(guard.path().to_path_buf()),
    )
    .await?;
    Ok(())
}

pub(crate) async fn state_transitions() -> anyhow::Result<()> {
    let rt = Runtime::new(
        Arc::new(InMemoryEventStore::default()),
        RuntimeConfig::default(),
    );
    let descriptor = external_descriptor("state-proj__abc12345", Path::new("/tmp/state-proj"));
    let id = descriptor.project.id.clone();
    rt.config().project_registry.register(descriptor)?;
    for state in [
        ProjectState::Starting,
        ProjectState::Running,
        ProjectState::Stopping,
        ProjectState::Stopped,
    ] {
        rt.config().project_registry.set_state(&id, state)?;
        let current = rt
            .config()
            .project_registry
            .get(&id)
            .context("project entry")?;
        anyhow::ensure!(current.state == state);
    }
    Ok(())
}

pub(crate) async fn archive_keeps_data() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let guard = DataDirGuard::new()?;
    let id = ProjectId::new("archive-proj__abc12345")?;
    let project_dir = guard.path().join("projects").join(id.as_str());
    std::fs::create_dir_all(&project_dir)?;
    std::fs::write(project_dir.join("data.txt"), "keep me")?;
    uninstall::archive_project(&id, guard.path())?;
    let archived = guard
        .path()
        .join("projects/.archived")
        .join(id.as_str())
        .join("data.txt");
    anyhow::ensure!(archived.is_file());
    anyhow::ensure!(std::fs::read_to_string(archived)? == "keep me");
    Ok(())
}

fn write_native_project(path: &Path, id: &str) -> anyhow::Result<()> {
    std::fs::write(
        path.join("project.yaml"),
        format!(
            r#"schema_version: 1
project:
  id: {id}
  title: Native Project
  description: Test native project
  type: plurora_native
  entry_surface_id: test/surface
  packages:
    - manifest.yaml
"#
        ),
    )?;
    Ok(())
}

fn manifest_yaml() -> &'static str {
    r#"schema_version: 1
id: fixture/project-lifecycle
version: 0.1.0
entry:
  kind: rust_inproc
  contract: v1
  crate_ref: example-echo-rust-inproc
  symbol: register
provides: []
"#
}

fn external_descriptor(id: &str, root: &Path) -> ProjectDescriptor {
    ProjectDescriptor {
        schema_version: 1,
        project: ProjectInner {
            id: ProjectId::new(id).expect("valid id"),
            title: id.to_string(),
            description: "external test".to_string(),
            project_type: ProjectType::ExternalWorkspace,
            icon: None,
            entry_surface_id: Some("plurora/workspace-lab/workspace_view".to_string()),
            packages: vec![],
            optional_packages: vec![],
            required_surfaces: vec![],
            required_capabilities: vec![],
            secret_policy: SecretPolicy::default(),
            external: Some(ExternalProjectData {
                source: root.display().to_string(),
                source_ref: None,
                adapter_manifest: None,
                workspace_root: Some(root.display().to_string()),
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

fn write_descriptor_to_data_dir(
    data_dir: &Path,
    descriptor: ProjectDescriptor,
) -> anyhow::Result<()> {
    let dir = data_dir
        .join("projects")
        .join(descriptor.project.id.as_str());
    std::fs::create_dir_all(dir.join("sessions"))?;
    std::fs::create_dir_all(dir.join("state"))?;
    std::fs::write(
        dir.join("project.yaml"),
        serde_yaml::to_string(&descriptor)?,
    )?;
    Ok(())
}
