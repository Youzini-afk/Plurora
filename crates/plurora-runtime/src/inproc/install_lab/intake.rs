use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use super::executor::{
    compute_external_git_tree_hash, compute_external_tree_hash, invoke_package_capability,
};
use super::fs_copy::{copy_external_tree_bounded_into, ManagedDirectory};
use super::layout::{ensure_layout, workspaces_dir};
use super::planner::{foreign_plan, source_display_name};
use super::source::{parse_root_descriptor, value_str, SourceDescriptor};
use super::{detection::detect_source_kind, types::SourceKind};

pub(super) const EXTERNAL_WORKSPACE_MAX_FILES: u64 = super::fs_copy::MAX_FILES;
pub(super) const EXTERNAL_WORKSPACE_MAX_DIRECTORIES: u64 = super::fs_copy::MAX_DIRECTORIES;
pub(super) const EXTERNAL_WORKSPACE_MAX_BYTES: u64 = super::fs_copy::MAX_BYTES;
pub(super) const EXTERNAL_WORKSPACE_MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;

const WORKSPACE_SCHEMA: &str = "plurora.workspace-record.v1";
const GIT_FETCH_DESTINATION_NAME: &str = "tree";

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

struct IsolatedGitFetchRoot {
    directory: Option<ManagedDirectory>,
    root_path: PathBuf,
    identity: IsolatedDirectoryIdentity,
    destination: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct IsolatedDirectoryIdentity {
    fingerprint: [u64; 2],
}

impl IsolatedDirectoryIdentity {
    fn capture(directory: &ManagedDirectory) -> Result<Self> {
        Ok(Self {
            fingerprint: directory.identity_fingerprint()?,
        })
    }

    fn ensure_matches(self, directory: &ManagedDirectory) -> Result<()> {
        anyhow::ensure!(
            Self::capture(directory)? == self,
            "isolated Git fetch root identity changed"
        );
        Ok(())
    }
}

impl IsolatedGitFetchRoot {
    fn create(workspace: &ManagedDirectory) -> Result<Self> {
        workspace.ensure_path_identity()?;
        let root_name = OsString::from(format!(".git-fetch-{}", Uuid::new_v4()));
        let directory = workspace.create_child(&root_name)?;
        directory.ensure_child_absent(GIT_FETCH_DESTINATION_NAME.as_ref())?;
        let root_path = directory.path().to_path_buf();
        let identity = IsolatedDirectoryIdentity::capture(&directory)?;
        identity.ensure_matches(&directory)?;
        let destination = root_path.join(GIT_FETCH_DESTINATION_NAME);
        // Public fetch_tree receives an ordinary path. Unix keeps the owner-bearing
        // handle so later cleanup remains handle-relative. Windows must release it while
        // fetch_tree opens the same directory because delete sharing intentionally stays
        // closed there; the identity-checked handoff below restores ownership afterward.
        #[cfg(windows)]
        let directory = {
            drop(directory);
            None
        };
        #[cfg(not(windows))]
        let directory = Some(directory);
        let fetch_root = Self {
            directory,
            root_path,
            identity,
            destination,
        };
        anyhow::ensure!(
            fs::symlink_metadata(fetch_root.destination())
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
            "isolated Git fetch destination already exists"
        );
        Ok(fetch_root)
    }

    fn directory(&self) -> &ManagedDirectory {
        self.directory
            .as_ref()
            .expect("isolated Git fetch root was reopened and verified")
    }

    fn destination(&self) -> &Path {
        &self.destination
    }

    fn claim_fetched_staging(&mut self, workspace: &ManagedDirectory) -> Result<ManagedDirectory> {
        workspace.ensure_path_identity()?;
        anyhow::ensure!(
            self.root_path.parent() == Some(workspace.path()),
            "isolated Git fetch root escaped its Workspace"
        );
        let directory = match self.directory.take() {
            Some(directory) => directory,
            None => ManagedDirectory::open(&self.root_path)?,
        };
        self.identity.ensure_matches(&directory)?;
        let staging = directory
            .open_child(GIT_FETCH_DESTINATION_NAME.as_ref())
            .context("Git fetch did not create a real isolated source directory")?;
        self.directory = Some(directory);
        Ok(staging)
    }

    fn reopen_and_verify(&mut self, workspace: &ManagedDirectory) -> Result<()> {
        workspace.ensure_path_identity()?;
        anyhow::ensure!(
            self.root_path.parent() == Some(workspace.path()),
            "isolated Git fetch root escaped its Workspace"
        );
        let directory = match self.directory.take() {
            Some(directory) => directory,
            None => ManagedDirectory::open(&self.root_path)?,
        };
        self.identity.ensure_matches(&directory)?;
        self.directory = Some(directory);
        Ok(())
    }

    fn cleanup(mut self) -> Result<()> {
        self.directory
            .take()
            .context("isolated Git fetch root was not reopened for cleanup")?
            .remove_empty()
    }
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
            remove_empty_owned_workspace_root(workspace_root);
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
                if let Err(error) = copy_external_tree_bounded_into(&canonical, &staging) {
                    return finish_staging_cleanup(Err(error), staging);
                }
                digest = hash_and_promote_staging(workspace_root, staging, false).await?;
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
            let mut fetch_root = IsolatedGitFetchRoot::create(workspace_root)?;
            let fetch_outcome = invoke_package_capability(
                "plurora/git-tools-lab",
                "plurora/git-tools-lab/fetch_tree",
                git_fetch_tree_input(url, &commit_sha, resolved_ref, fetch_root.destination()),
            )
            .await;
            if let Err(error) = fetch_outcome {
                return finish_failed_isolated_git_fetch(workspace_root, fetch_root, error);
            }
            let staging = match fetch_root.claim_fetched_staging(workspace_root) {
                Ok(staging) => staging,
                Err(error) => return finish_isolated_git_fetch(fetch_root, Err(error)),
            };
            let digest_outcome = hash_and_promote_staging(workspace_root, staging, true).await;
            let digest = finish_isolated_git_fetch(fetch_root, digest_outcome)?;
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

fn git_fetch_tree_input(
    url: &str,
    commit_sha: &str,
    resolved_ref: &str,
    destination: &Path,
) -> Value {
    json!({
        "remote_url": url,
        "commit_sha": commit_sha,
        "ref_name": resolved_ref,
        "dest_dir": destination.to_string_lossy(),
        "max_files": EXTERNAL_WORKSPACE_MAX_FILES,
        "max_directories": EXTERNAL_WORKSPACE_MAX_DIRECTORIES,
        "max_total_bytes": EXTERNAL_WORKSPACE_MAX_BYTES,
        "max_download_bytes": EXTERNAL_WORKSPACE_MAX_DOWNLOAD_BYTES,
    })
}

fn finish_staging_cleanup<T>(outcome: Result<T>, staging: ManagedDirectory) -> Result<T> {
    let cleanup = staging.remove();
    match (outcome, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(cleanup_error)) => {
            Err(cleanup_error).context("failed to clean Git intake staging")
        }
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup_error)) => Err(error.context(format!(
            "Git intake staging cleanup also failed: {cleanup_error:#}"
        ))),
    }
}

async fn hash_and_promote_staging(
    workspace: &ManagedDirectory,
    staging: ManagedDirectory,
    tracked_git_tree: bool,
) -> Result<String> {
    let outcome = async {
        let stable_staging = staging.stable_access_path()?;
        staging.ensure_path_identity()?;
        anyhow::ensure!(
            detect_source_kind(&stable_staging) == SourceKind::Foreign,
            "external intake accepts only Foreign sources"
        );
        let digest = if tracked_git_tree {
            compute_external_git_tree_hash(&stable_staging).await?
        } else {
            compute_external_tree_hash(&stable_staging).await?
        };
        staging.ensure_path_identity()?;
        promote_workspace_source(workspace, &staging)?;
        Ok(digest)
    }
    .await;
    match outcome {
        Ok(digest) => Ok(digest),
        Err(error) => finish_staging_cleanup(Err(error), staging),
    }
}

fn finish_failed_isolated_git_fetch<T>(
    workspace: &ManagedDirectory,
    mut fetch_root: IsolatedGitFetchRoot,
    error: anyhow::Error,
) -> Result<T> {
    let cleanup = fetch_root.reopen_and_verify(workspace).and_then(|()| {
        fetch_root
            .directory()
            .ensure_child_absent(GIT_FETCH_DESTINATION_NAME.as_ref())?;
        fetch_root.cleanup()
    });
    match cleanup {
        Ok(()) => Err(error),
        Err(verification_error) => Err(error.context(format!(
            "isolated Git fetch ownership verification also failed: {verification_error:#}"
        ))),
    }
}

fn finish_isolated_git_fetch<T>(fetch_root: IsolatedGitFetchRoot, outcome: Result<T>) -> Result<T> {
    let cleanup = fetch_root.cleanup();
    match (outcome, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(cleanup_error)) => {
            Err(cleanup_error).context("failed to clean isolated Git fetch root")
        }
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup_error)) => Err(error.context(format!(
            "isolated Git fetch also failed cleanup: {cleanup_error:#}"
        ))),
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
    let actual_digest = match record.ownership {
        WorkspaceOwnership::Managed => {
            let workspace = ManagedDirectory::open(&workspace)?;
            let source = managed_workspace_source(&workspace)?;
            let access = source.stable_access_path()?;
            anyhow::ensure!(
                detect_source_kind(&access) == SourceKind::Foreign,
                "workspace source is no longer Foreign"
            );
            let digest = match record.source_kind {
                WorkspaceSourceKind::Git => compute_external_git_tree_hash(&access).await?,
                WorkspaceSourceKind::Local => compute_external_tree_hash(&access).await?,
            };
            source.ensure_path_identity()?;
            workspace.ensure_path_identity()?;
            digest
        }
        WorkspaceOwnership::LinkedLocal => {
            let source = fs::canonicalize(&record.source_locator)
                .map_err(|_| anyhow::anyhow!("linked source could not be opened"))?;
            anyhow::ensure!(source.is_dir(), "linked source must be a directory");
            anyhow::ensure!(
                detect_source_kind(&source) == SourceKind::Foreign,
                "workspace source is no longer Foreign"
            );
            compute_external_tree_hash(&source).await?
        }
    };
    anyhow::ensure!(
        actual_digest == record.source_digest,
        "workspace source changed after intake"
    );
    Ok(WorkspaceExecutionSource {
        source_digest: record.source_digest,
        display_name: record.display_name,
    })
}

fn managed_workspace_source(workspace: &ManagedDirectory) -> Result<ManagedDirectory> {
    workspace
        .open_child("source".as_ref())
        .map_err(|_| anyhow::anyhow!("managed workspace source must be a real directory"))
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

fn remove_empty_owned_workspace_root(workspace: ManagedDirectory) {
    // Every materializer owns and cleans its child identities. Removing only an empty
    // Workspace here prevents an error path from recursively deleting an entry that was
    // raced into one of those child names after identity verification failed.
    let _ = workspace.remove_empty();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    async fn with_production_runtime<F, T>(future: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        use std::sync::Arc;

        use crate::{InMemoryEventStore, Runtime, RuntimeConfig};
        use plurora_core::PackageManifest;

        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        );
        // Load the same manifests used by the shipped Install Lab and Integrity Lab so the
        // production capability route (including package permissions and provider resolution)
        // is exercised instead of calling the hash implementation directly.
        let install_manifest: PackageManifest = serde_yaml::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/plurora/install-lab/manifest.yaml"
        )))?;
        let integrity_manifest: PackageManifest = serde_yaml::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/plurora/integrity-lab/manifest.yaml"
        )))?;
        runtime.load_package(install_manifest).await?;
        runtime.load_package(integrity_manifest).await?;

        crate::inproc::with_runtime_invoker(runtime, None, future).await
    }

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
    fn git_intake_uses_an_ordinary_new_destination_without_a_private_flag() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let data = temporary.path().join("data");
        let workspace =
            create_workspace_root(Some(data.to_string_lossy().as_ref()), Uuid::new_v4())?;
        let mut fetch_root = IsolatedGitFetchRoot::create(&workspace)?;
        let input = git_fetch_tree_input(
            "https://example.com/repository.git",
            "0123456789abcdef0123456789abcdef01234567",
            "refs/heads/main",
            fetch_root.destination(),
        );

        assert!(!fetch_root.destination().exists());
        assert_eq!(
            fetch_root.destination().parent(),
            Some(fetch_root.root_path.as_path())
        );
        assert_eq!(fetch_root.root_path.parent(), Some(workspace.path()));
        let independently_opened = ManagedDirectory::open(
            fetch_root
                .destination()
                .parent()
                .expect("Git destination has a parent"),
        )?;
        fetch_root.identity.ensure_matches(&independently_opened)?;
        #[cfg(unix)]
        assert!(!fetch_root
            .destination()
            .to_string_lossy()
            .starts_with("/proc/self/fd/"));
        assert!(input.get("precreated_handle_anchored_dest").is_none());
        assert_eq!(
            input["max_download_bytes"],
            json!(EXTERNAL_WORKSPACE_MAX_DOWNLOAD_BYTES)
        );
        assert_eq!(input["max_files"], json!(EXTERNAL_WORKSPACE_MAX_FILES));
        assert_eq!(
            input["max_total_bytes"],
            json!(EXTERNAL_WORKSPACE_MAX_BYTES)
        );
        drop(independently_opened);
        fetch_root.reopen_and_verify(&workspace)?;
        fetch_root.cleanup()?;
        workspace.remove()?;
        Ok(())
    }

    #[test]
    fn ordinary_path_fetch_round_trip_is_claimed_by_both_identity_handles() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let data = temporary.path().join("data");
        let workspace =
            create_workspace_root(Some(data.to_string_lossy().as_ref()), Uuid::new_v4())?;
        let mut fetch_root = IsolatedGitFetchRoot::create(&workspace)?;
        let public_parent = ManagedDirectory::open(
            fetch_root
                .destination()
                .parent()
                .expect("Git destination has a parent"),
        )?;
        let published = public_parent.create_child(GIT_FETCH_DESTINATION_NAME.as_ref())?;
        published.write_new_file("README.md".as_ref(), b"fetched", false)?;
        drop(published);
        drop(public_parent);

        let staging = fetch_root.claim_fetched_staging(&workspace)?;
        promote_workspace_source(&workspace, &staging)?;

        assert_eq!(
            fs::read_to_string(workspace.path().join("source/README.md"))?,
            "fetched"
        );
        drop(staging);
        fetch_root.cleanup()?;
        workspace.remove()?;
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn production_git_hash_preserves_tracked_names_mode_and_contained_dangling_symlink(
    ) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let temporary = tempfile::tempdir()?;
        let data = temporary.path().join("data");
        let workspace =
            create_workspace_root(Some(data.to_string_lossy().as_ref()), Uuid::new_v4())?;
        let mut fetch_root = IsolatedGitFetchRoot::create(&workspace)?;
        fs::create_dir(fetch_root.destination())?;
        fs::create_dir_all(fetch_root.destination().join("target/node_modules/.venv"))?;
        fs::write(
            fetch_root
                .destination()
                .join("target/node_modules/.venv/tracked.txt"),
            "tracked",
        )?;
        let executable = fetch_root.destination().join("run.sh");
        fs::write(&executable, "#!/bin/sh\n")?;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755))?;
        std::os::unix::fs::symlink(
            "missing-inside-root",
            fetch_root.destination().join("alias"),
        )?;

        let staging = fetch_root.claim_fetched_staging(&workspace)?;
        with_production_runtime(async {
            let digest = hash_and_promote_staging(&workspace, staging, true).await?;
            fetch_root.cleanup()?;

            let source = workspace.path().join("source");
            assert_eq!(
                fs::read_to_string(source.join("target/node_modules/.venv/tracked.txt"))?,
                "tracked"
            );
            assert_ne!(
                fs::metadata(source.join("run.sh"))?.permissions().mode() & 0o111,
                0
            );
            assert_eq!(
                fs::read_link(source.join("alias"))?,
                Path::new("missing-inside-root")
            );
            let workspace_id = workspace
                .path()
                .file_name()
                .and_then(|name| name.to_str())
                .context("workspace id is not UTF-8")?
                .to_string();
            let record = WorkspaceRecord {
                schema: WORKSPACE_SCHEMA.to_string(),
                workspace_id: workspace_id.clone(),
                ownership: WorkspaceOwnership::Managed,
                source_kind: WorkspaceSourceKind::Git,
                source_locator: "https://example.test/repository.git".to_string(),
                source_ref: Some("0123456789abcdef0123456789abcdef01234567".to_string()),
                source_digest: digest.clone(),
                display_name: "repository".to_string(),
            };
            workspace.atomic_write("workspace.json".as_ref(), &serde_json::to_vec(&record)?)?;

            let verified =
                read_workspace_for_execution(Some(data.to_string_lossy().as_ref()), &workspace_id)
                    .await?;
            assert_eq!(verified.source_digest, digest);

            fs::set_permissions(source.join("run.sh"), fs::Permissions::from_mode(0o644))?;
            assert!(
                read_workspace_for_execution(Some(data.to_string_lossy().as_ref()), &workspace_id,)
                    .await
                    .is_err(),
                "Git executable-bit drift after promotion must invalidate intake"
            );
            fs::set_permissions(source.join("run.sh"), fs::Permissions::from_mode(0o755))?;

            fs::write(
                source.join("target/node_modules/.venv/tracked.txt"),
                "changed after intake",
            )?;
            assert!(
                read_workspace_for_execution(Some(data.to_string_lossy().as_ref()), &workspace_id,)
                    .await
                    .is_err(),
                "tracked Git entries excluded by local-copy policy must remain hash-significant"
            );
            workspace.remove()?;
            Ok(())
        })
        .await?;
        Ok(())
    }

    #[test]
    fn failed_git_fetch_never_deletes_an_unexpected_tree_from_its_temporary_root() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let data = temporary.path().join("data");
        let workspace =
            create_workspace_root(Some(data.to_string_lossy().as_ref()), Uuid::new_v4())?;
        let fetch_root = IsolatedGitFetchRoot::create(&workspace)?;
        let destination = fetch_root.destination().to_path_buf();
        fs::create_dir(fetch_root.destination())?;
        fs::write(fetch_root.destination().join("partial"), "partial")?;

        assert!(finish_failed_isolated_git_fetch::<()>(
            &workspace,
            fetch_root,
            anyhow::anyhow!("simulated fetch failure")
        )
        .is_err());
        assert!(destination.exists());
        assert_eq!(fs::read_to_string(destination.join("partial"))?, "partial");
        assert!(workspace.path().is_dir());
        assert!(!workspace.path().join("source").exists());

        // The managed Workspace is intentionally non-empty after the failed fetch.  Keep
        // the handle alive while asserting that the unexpected destination was preserved;
        // calling `remove_empty` here would itself claim a non-empty root before rejecting
        // it, which is unrelated to the fetch-failure contract under test.
        drop(workspace);
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
