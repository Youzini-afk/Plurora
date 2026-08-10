use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use super::executor::{compute_external_tree_hash, invoke_package_capability};
use super::fs_copy::{copy_external_tree_bounded_into, ManagedDirectory};
use super::layout::{ensure_layout, workspaces_dir};
use super::planner::{foreign_plan, source_display_name};
use super::source::{parse_root_descriptor, value_str, SourceDescriptor};
use super::{detection::detect_source_kind, types::SourceKind};

pub(super) const EXTERNAL_WORKSPACE_MAX_FILES: u64 = super::fs_copy::MAX_FILES;
pub(super) const EXTERNAL_WORKSPACE_MAX_DIRECTORIES: u64 = super::fs_copy::MAX_DIRECTORIES;
pub(super) const EXTERNAL_WORKSPACE_MAX_BYTES: u64 = super::fs_copy::MAX_BYTES;

const WORKSPACE_SCHEMA: &str = "plurora.workspace-record.v1";

#[derive(Debug, Deserialize)]
pub(super) struct PrepareExternalIntakeInput {
    source: String,
    #[serde(default = "super::layout::default_head_ref")]
    root_ref: String,
    #[serde(default)]
    data_dir: Option<String>,
    #[serde(default)]
    linked_local: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum WorkspaceOwnership {
    Managed,
    LinkedLocal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum WorkspaceSourceKind {
    Local,
    Git,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkspaceRecord {
    schema: String,
    workspace_id: String,
    ownership: WorkspaceOwnership,
    source_kind: WorkspaceSourceKind,
    source_locator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_ref: Option<String>,
    source_digest: String,
    display_name: String,
}

pub(super) struct WorkspaceExecutionSource {
    pub(super) source_digest: String,
    pub(super) display_name: String,
}

pub(super) async fn prepare_external_intake(input: Value) -> Result<Value> {
    let input: PrepareExternalIntakeInput = serde_json::from_value(input)?;
    ensure_layout(input.data_dir.as_deref())?;
    let source = parse_root_descriptor(&input.source, &input.root_ref)?;
    let workspace_id = Uuid::new_v4();
    let workspace_root = create_workspace_root(input.data_dir.as_deref(), workspace_id)?;
    let materialized = materialize_workspace_source(
        &source,
        &workspace_root,
        input.linked_local,
        &input.root_ref,
    )
    .await;
    let record = match materialized {
        Ok(record) => record,
        Err(error) => {
            remove_owned_workspace_root(workspace_root);
            return Err(error);
        }
    };
    let encoded = serde_json::to_vec_pretty(&record)?;
    if let Err(error) = workspace_root.atomic_write("workspace.json".as_ref(), &encoded) {
        remove_owned_workspace_root(workspace_root);
        return Err(error);
    }
    if let Err(error) = workspace_root.ensure_path_identity() {
        remove_owned_workspace_root(workspace_root);
        return Err(error);
    }

    let plan = foreign_plan(&record.source_digest, record.display_name.clone())?;
    let work_candidate = plan.work_candidate.clone();
    Ok(json!({
        "plan": plan,
        "workspace": workspace_output(&record),
        "work_candidate": work_candidate,
    }))
}

async fn materialize_workspace_source(
    source: &SourceDescriptor,
    workspace_root: &ManagedDirectory,
    linked_local: bool,
    root_ref: &str,
) -> Result<WorkspaceRecord> {
    match source {
        SourceDescriptor::Local { path } => {
            let canonical = fs::canonicalize(path)
                .map_err(|_| anyhow::anyhow!("local source could not be opened"))?;
            anyhow::ensure!(canonical.is_dir(), "local source must be a directory");
            anyhow::ensure!(
                detect_source_kind(&canonical) == SourceKind::Foreign,
                "external intake accepts only Foreign sources"
            );
            let digest;
            let ownership;
            if linked_local {
                digest = compute_external_tree_hash(&canonical).await?;
                ownership = WorkspaceOwnership::LinkedLocal;
            } else {
                let staging_name = format!(".source-{}", Uuid::new_v4());
                let staging = workspace_root.create_child(staging_name.as_ref())?;
                copy_external_tree_bounded_into(&canonical, &staging)?;
                let stable_staging = staging.stable_access_path()?;
                digest = compute_external_tree_hash(&stable_staging).await?;
                staging.ensure_path_identity()?;
                promote_workspace_source(workspace_root, &staging)?;
                ownership = WorkspaceOwnership::Managed;
            }
            Ok(WorkspaceRecord {
                schema: WORKSPACE_SCHEMA.to_string(),
                workspace_id: workspace_root
                    .path()
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_string(),
                ownership,
                source_kind: WorkspaceSourceKind::Local,
                source_locator: canonical.to_string_lossy().to_string(),
                source_ref: None,
                source_digest: digest,
                display_name: source_display_name(source, "Foreign source"),
            })
        }
        SourceDescriptor::Git { url, ref_name } => {
            anyhow::ensure!(
                !linked_local,
                "linked_local is valid only for a local source"
            );
            ensure_persistable_git_source(url)?;
            let resolved = invoke_package_capability(
                "plurora/git-tools-lab",
                "plurora/git-tools-lab/resolve_ref",
                json!({ "remote_url": url, "ref": ref_name }),
            )
            .await?;
            let commit_sha = value_str(&resolved, "commit_sha")?.to_string();
            let resolved_ref = resolved
                .get("ref_name")
                .and_then(Value::as_str)
                .unwrap_or(root_ref);
            let staging_name = format!(".source-{}", Uuid::new_v4());
            let staging = workspace_root.create_child(staging_name.as_ref())?;
            let stable_staging = staging.stable_access_path()?;
            let fetch_result = invoke_package_capability(
                "plurora/git-tools-lab",
                "plurora/git-tools-lab/fetch_tree",
                json!({
                    "remote_url": url,
                    "commit_sha": commit_sha,
                    "ref_name": resolved_ref,
                    "dest_dir": stable_staging.to_string_lossy(),
                    "max_files": EXTERNAL_WORKSPACE_MAX_FILES,
                    "max_directories": EXTERNAL_WORKSPACE_MAX_DIRECTORIES,
                    "max_total_bytes": EXTERNAL_WORKSPACE_MAX_BYTES,
                }),
            )
            .await;
            if let Err(error) = fetch_result {
                let _ = staging.remove();
                return Err(error);
            }
            staging.ensure_path_identity()?;
            anyhow::ensure!(
                detect_source_kind(&stable_staging) == SourceKind::Foreign,
                "external intake accepts only Foreign sources"
            );
            let digest = compute_external_tree_hash(&stable_staging).await?;
            staging.ensure_path_identity()?;
            promote_workspace_source(workspace_root, &staging)?;
            Ok(WorkspaceRecord {
                schema: WORKSPACE_SCHEMA.to_string(),
                workspace_id: workspace_root
                    .path()
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_string(),
                ownership: WorkspaceOwnership::Managed,
                source_kind: WorkspaceSourceKind::Git,
                source_locator: url.clone(),
                source_ref: Some(commit_sha),
                source_digest: digest,
                display_name: source_display_name(source, "Foreign source"),
            })
        }
        SourceDescriptor::Internal => {
            anyhow::bail!("internal source cannot be imported into a Workspace")
        }
    }
}

fn create_workspace_root(data_dir: Option<&str>, workspace_id: Uuid) -> Result<ManagedDirectory> {
    let root = workspaces_dir(data_dir)?;
    let owner = ManagedDirectory::open(&root)?;
    owner.create_child(workspace_id.to_string().as_ref())
}

fn promote_workspace_source(
    workspace: &ManagedDirectory,
    staging: &ManagedDirectory,
) -> Result<()> {
    workspace
        .promote_child(staging, "source".as_ref())
        .map_err(|_| anyhow::anyhow!("managed workspace source could not be promoted"))
}

fn workspace_output(record: &WorkspaceRecord) -> Value {
    json!({
        "workspace_id": record.workspace_id,
        "source_digest": record.source_digest,
        "ownership": record.ownership,
    })
}

pub(super) async fn read_workspace_for_execution(
    data_dir: Option<&str>,
    workspace_id: &str,
) -> Result<WorkspaceExecutionSource> {
    let workspace_id = Uuid::parse_str(workspace_id)
        .map_err(|_| anyhow::anyhow!("workspace_id must be a UUID"))?;
    let root = workspaces_dir(data_dir)?;
    let workspace = existing_owned_workspace(&root, &workspace_id.to_string())?;
    let raw = fs::read(workspace.join("workspace.json"))
        .map_err(|_| anyhow::anyhow!("workspace record could not be read"))?;
    let record: WorkspaceRecord = serde_json::from_slice(&raw)
        .map_err(|_| anyhow::anyhow!("workspace record is malformed"))?;
    validate_workspace_record(&record, workspace_id)?;
    let source = match record.ownership {
        WorkspaceOwnership::Managed => managed_workspace_source(&workspace)?,
        WorkspaceOwnership::LinkedLocal => {
            let source = fs::canonicalize(&record.source_locator)
                .map_err(|_| anyhow::anyhow!("linked source could not be opened"))?;
            anyhow::ensure!(source.is_dir(), "linked source must be a directory");
            source
        }
    };
    anyhow::ensure!(
        detect_source_kind(&source) == SourceKind::Foreign,
        "workspace source is no longer Foreign"
    );
    let actual_digest = compute_external_tree_hash(&source).await?;
    anyhow::ensure!(
        actual_digest == record.source_digest,
        "workspace source changed after intake"
    );
    Ok(WorkspaceExecutionSource {
        source_digest: record.source_digest,
        display_name: record.display_name,
    })
}

fn managed_workspace_source(workspace: &Path) -> Result<PathBuf> {
    let source = workspace.join("source");
    let metadata = fs::symlink_metadata(&source)
        .map_err(|_| anyhow::anyhow!("managed workspace source does not exist"))?;
    anyhow::ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "managed workspace source must be a real directory"
    );
    let canonical = fs::canonicalize(&source)
        .map_err(|_| anyhow::anyhow!("managed workspace source could not be opened"))?;
    anyhow::ensure!(
        canonical.parent() == Some(workspace),
        "managed workspace source escaped its Workspace"
    );
    Ok(canonical)
}

fn validate_workspace_record(record: &WorkspaceRecord, expected_id: Uuid) -> Result<()> {
    anyhow::ensure!(
        record.schema == WORKSPACE_SCHEMA && record.workspace_id == expected_id.to_string(),
        "workspace record identity is invalid"
    );
    let digest = record
        .source_digest
        .strip_prefix("sha256:")
        .ok_or_else(|| anyhow::anyhow!("workspace source digest is invalid"))?;
    anyhow::ensure!(
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "workspace source digest is invalid"
    );
    Ok(())
}

fn existing_owned_workspace(root: &Path, workspace_id: &str) -> Result<PathBuf> {
    let path = root.join(workspace_id);
    let metadata =
        fs::symlink_metadata(&path).map_err(|_| anyhow::anyhow!("workspace does not exist"))?;
    anyhow::ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "workspace root must be a real directory"
    );
    let canonical = fs::canonicalize(&path)
        .map_err(|_| anyhow::anyhow!("workspace root could not be canonicalized"))?;
    anyhow::ensure!(
        canonical.parent() == Some(root),
        "workspace root escaped its managed parent"
    );
    Ok(canonical)
}

fn ensure_persistable_git_source(source: &str) -> Result<()> {
    let parsed =
        url::Url::parse(source).map_err(|_| anyhow::anyhow!("Git source URL is invalid"))?;
    anyhow::ensure!(
        parsed.scheme() == "https"
            && parsed.password().is_none()
            && parsed.query().is_none()
            && parsed.username().is_empty(),
        "Git Workspace intake requires HTTPS without inline credentials or query parameters"
    );
    Ok(())
}

#[cfg(test)]
fn cleanup_workspace(data_dir: Option<&str>, workspace_id: &str) -> Result<()> {
    let workspace_id = Uuid::parse_str(workspace_id)
        .map_err(|_| anyhow::anyhow!("workspace_id must be a UUID"))?;
    let root = workspaces_dir(data_dir)?;
    let workspace = existing_owned_workspace(&root, &workspace_id.to_string())?;
    let raw = fs::read(workspace.join("workspace.json"))
        .map_err(|_| anyhow::anyhow!("workspace record could not be read"))?;
    let record: WorkspaceRecord = serde_json::from_slice(&raw)
        .map_err(|_| anyhow::anyhow!("workspace record is malformed"))?;
    validate_workspace_record(&record, workspace_id)?;
    // The linked source is never a descendant of this Host-owned record root.
    // Cleanup removes only the opaque Workspace directory in both modes.
    fs::remove_dir_all(&workspace)?;
    Ok(())
}

fn remove_owned_workspace_root(workspace: ManagedDirectory) {
    let _ = workspace.remove();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest() -> String {
        format!("sha256:{}", "a".repeat(64))
    }

    #[test]
    fn workspace_output_is_opaque_and_uses_a_uuid() -> Result<()> {
        let workspace_id = Uuid::new_v4();
        let record = WorkspaceRecord {
            schema: WORKSPACE_SCHEMA.to_string(),
            workspace_id: workspace_id.to_string(),
            ownership: WorkspaceOwnership::LinkedLocal,
            source_kind: WorkspaceSourceKind::Local,
            source_locator: "C:\\private\\source".to_string(),
            source_ref: None,
            source_digest: digest(),
            display_name: "source".to_string(),
        };
        let output = workspace_output(&record);
        assert_eq!(
            Uuid::parse_str(output["workspace_id"].as_str().unwrap())?,
            workspace_id
        );
        assert!(!output.to_string().contains("private"));
        assert!(!output.to_string().contains("source_locator"));

        let plan = foreign_plan(&record.source_digest, record.display_name.clone())?;
        let response = json!({
            "plan": plan,
            "workspace": output,
        });
        assert!(!response.to_string().contains("private"));
        assert!(!response.to_string().contains("source_locator"));
        Ok(())
    }

    #[test]
    fn managed_local_intake_promotes_source_within_the_pinned_workspace() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let data = temporary.path().join("data");
        let source = temporary.path().join("local-source");
        fs::create_dir_all(&source)?;
        fs::write(source.join("public.txt"), "public")?;
        let workspace_id = Uuid::new_v4();
        let workspace = create_workspace_root(Some(data.to_string_lossy().as_ref()), workspace_id)?;
        let staging = workspace.create_child(".source-staging".as_ref())?;
        copy_external_tree_bounded_into(&source, &staging)?;
        promote_workspace_source(&workspace, &staging)?;
        assert_eq!(
            fs::read_to_string(workspace.path().join("source/public.txt"))?,
            "public"
        );
        assert!(!fs::read_dir(workspace.path())?.any(|entry| {
            entry
                .ok()
                .and_then(|entry| entry.file_name().into_string().ok())
                .is_some_and(|name| name.starts_with(".source-"))
        }));
        drop(staging);
        workspace.remove()?;
        assert!(source.join("public.txt").is_file());
        Ok(())
    }

    #[test]
    fn linked_local_source_survives_workspace_cleanup() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let data = temporary.path().join("data");
        let source = temporary.path().join("linked-source");
        fs::create_dir_all(&source)?;
        fs::write(source.join("owned-by-user"), "keep")?;
        let workspace_id = Uuid::new_v4();
        let workspace = create_workspace_root(Some(data.to_string_lossy().as_ref()), workspace_id)?;
        let workspace_path = workspace.path().to_path_buf();
        let record = WorkspaceRecord {
            schema: WORKSPACE_SCHEMA.to_string(),
            workspace_id: workspace_id.to_string(),
            ownership: WorkspaceOwnership::LinkedLocal,
            source_kind: WorkspaceSourceKind::Local,
            source_locator: fs::canonicalize(&source)?.to_string_lossy().to_string(),
            source_ref: None,
            source_digest: digest(),
            display_name: "linked-source".to_string(),
        };
        workspace.atomic_write("workspace.json".as_ref(), &serde_json::to_vec(&record)?)?;
        drop(workspace);

        cleanup_workspace(
            Some(data.to_string_lossy().as_ref()),
            &workspace_id.to_string(),
        )?;
        assert!(!workspace_path.exists());
        assert!(source.join("owned-by-user").is_file());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn workspace_root_rejects_a_symlinked_ancestor() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let data = temporary.path().join("data");
        let outside = temporary.path().join("outside");
        fs::create_dir_all(&data)?;
        fs::create_dir_all(&outside)?;
        std::os::unix::fs::symlink(&outside, data.join("workspaces"))?;
        assert!(
            create_workspace_root(Some(data.to_string_lossy().as_ref()), Uuid::new_v4()).is_err()
        );
        assert!(fs::read_dir(outside)?.next().is_none());
        Ok(())
    }
}
