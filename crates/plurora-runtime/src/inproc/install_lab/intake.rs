use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use plurora_core::project::{
    ExternalProjectData, ExternalSourceKind, ExternalWorkspaceOwnership, ProjectDescriptor,
    ProjectId, ProjectInner, ProjectType, SecretPolicy,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::executor::{compute_external_tree_hash, invoke_package_capability};
use super::layout::ensure_layout;
use super::project_kind::{detect_project_kind, read_project_descriptor};
use super::source::{parse_root_descriptor, value_str};
use super::types::{
    DetectedProjectKind, InstallPlan, IntegritySummary, PermissionsSummary, SignatureSummary,
    SourceDescriptor,
};

const EXTERNAL_WORKSPACE_EXCLUDED_NAMES: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    ".DS_Store",
    "node_modules",
    "target",
    ".venv",
    "venv",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
];
pub(super) const EXTERNAL_WORKSPACE_MAX_FILES: u64 = 25_000;
pub(super) const EXTERNAL_WORKSPACE_MAX_DIRECTORIES: u64 = 25_000;
pub(super) const EXTERNAL_WORKSPACE_MAX_BYTES: u64 = 256 * 1024 * 1024;

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

pub(super) async fn prepare_external_intake(input: Value) -> Result<Value> {
    let input: PrepareExternalIntakeInput = serde_json::from_value(input)?;
    ensure_layout(input.data_dir.as_deref())?;
    let data_dir = canonical_data_dir(input.data_dir.as_deref())?;
    let root = parse_root_descriptor(&input.source, &input.root_ref)?;

    let materialized = match root.source {
        SourceDescriptor::Local { path } => {
            prepare_local_source(&path, &data_dir, input.linked_local).await?
        }
        SourceDescriptor::Git { url, ref_name } => {
            anyhow::ensure!(
                !input.linked_local,
                "linked_local is only valid for a local source"
            );
            prepare_git_source(&url, &ref_name, &data_dir).await?
        }
        SourceDescriptor::Internal => {
            anyhow::bail!("internal packages cannot be external projects")
        }
    };

    ensure_bare_external(&materialized.workspace_root)?;
    let descriptor = create_workspace_descriptor(&materialized)?;
    ensure_existing_descriptor_is_compatible(&descriptor, &data_dir)?;
    let plan = InstallPlan {
        root_id: descriptor.project.id.as_str().to_string(),
        packages: Vec::new(),
        project_descriptor: Some(descriptor.clone()),
        permissions_summary: PermissionsSummary::default(),
        signature_summary: SignatureSummary {
            all_signed: true,
            unsigned_packages: Vec::new(),
        },
        integrity_summary: IntegritySummary {
            manifest_hashes_match_lockfile: true,
            drift_detected: Vec::new(),
        },
    };

    Ok(json!({
        "plan": plan,
        "intake": {
            "project_id": descriptor.project.id.as_str(),
            "source": materialized.source,
            "source_kind": materialized.source_kind,
            "source_ref": materialized.source_ref,
            "source_digest": materialized.source_digest,
            "workspace_root": materialized.workspace_root,
            "workspace_ownership": materialized.workspace_ownership,
            "reused": materialized.reused,
        }
    }))
}

#[derive(Debug)]
struct MaterializedExternalSource {
    source: String,
    source_kind: ExternalSourceKind,
    source_ref: Option<String>,
    source_digest: Option<String>,
    workspace_root: PathBuf,
    workspace_ownership: ExternalWorkspaceOwnership,
    reused: bool,
}

async fn prepare_local_source(
    source: &Path,
    data_dir: &Path,
    linked_local: bool,
) -> Result<MaterializedExternalSource> {
    let source = fs::canonicalize(source).with_context(|| {
        format!(
            "failed to canonicalize external source {}",
            source.display()
        )
    })?;
    anyhow::ensure!(source.is_dir(), "external source must be a directory");
    let source_text = source.to_string_lossy().to_string();

    if linked_local {
        return Ok(MaterializedExternalSource {
            source: source_text,
            source_kind: ExternalSourceKind::Local,
            source_ref: None,
            source_digest: None,
            workspace_root: source,
            workspace_ownership: ExternalWorkspaceOwnership::LinkedLocal,
            reused: true,
        });
    }

    let project_id = derive_project_id(&source_text)?;
    let project_root = external_project_workspace_root(data_dir, &project_id)?;
    ensure_non_overlapping_roots(&source, &project_root)?;
    let staging = create_staging_dir(&project_root)?;
    let result = async {
        copy_external_tree_bounded(&source, &staging).with_context(|| {
            format!(
                "failed to materialize local external project {}",
                source.display()
            )
        })?;
        let digest = compute_external_tree_hash(&staging).await?;
        let (workspace_root, reused) = promote_staging(&staging, &project_root, &digest).await?;
        Ok(MaterializedExternalSource {
            source: source_text,
            source_kind: ExternalSourceKind::Local,
            source_ref: None,
            source_digest: Some(digest),
            workspace_root,
            workspace_ownership: ExternalWorkspaceOwnership::Managed,
            reused,
        })
    }
    .await;
    if result.is_err() {
        fs::remove_dir_all(&staging).ok();
    }
    result
}

async fn prepare_git_source(
    url: &str,
    ref_name: &str,
    data_dir: &Path,
) -> Result<MaterializedExternalSource> {
    ensure_persistable_git_source(url)?;
    let resolved = invoke_package_capability(
        "official/git-tools-lab",
        "official/git-tools-lab/resolve_ref",
        json!({ "remote_url": url, "ref": ref_name }),
    )
    .await?;
    let commit_sha = value_str(&resolved, "commit_sha")?.to_string();
    let resolved_ref = resolved
        .get("ref_name")
        .and_then(Value::as_str)
        .unwrap_or(ref_name)
        .to_string();
    let project_id = derive_project_id(url)?;
    let project_root = external_project_workspace_root(data_dir, &project_id)?;
    let staging = create_staging_dir(&project_root)?;
    let result = async {
        invoke_package_capability(
            "official/git-tools-lab",
            "official/git-tools-lab/fetch_tree",
            json!({
                "remote_url": url,
                "commit_sha": commit_sha,
                "ref_name": resolved_ref,
                "dest_dir": staging.to_string_lossy(),
                "max_files": EXTERNAL_WORKSPACE_MAX_FILES,
                "max_directories": EXTERNAL_WORKSPACE_MAX_DIRECTORIES,
                "max_total_bytes": EXTERNAL_WORKSPACE_MAX_BYTES,
            }),
        )
        .await?;
        let digest = compute_external_tree_hash(&staging).await?;
        let (workspace_root, reused) = promote_staging(&staging, &project_root, &digest).await?;
        Ok(MaterializedExternalSource {
            source: url.to_string(),
            source_kind: ExternalSourceKind::Git,
            source_ref: Some(commit_sha),
            source_digest: Some(digest),
            workspace_root,
            workspace_ownership: ExternalWorkspaceOwnership::Managed,
            reused,
        })
    }
    .await;
    if result.is_err() {
        fs::remove_dir_all(&staging).ok();
    }
    result
}

fn canonical_data_dir(data_dir_override: Option<&str>) -> Result<PathBuf> {
    let path = data_dir_override
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(plurora_core::paths::data_dir)?;
    fs::canonicalize(&path)
        .with_context(|| format!("failed to canonicalize data directory {}", path.display()))
}

fn external_project_workspace_root(data_dir: &Path, project_id: &ProjectId) -> Result<PathBuf> {
    let workspaces = ensure_owned_directory(data_dir, "workspaces", "workspace root")?;
    let external = ensure_owned_directory(&workspaces, "external", "external workspace root")?;
    ensure_owned_directory(
        &external,
        project_id.as_str(),
        "managed external project root",
    )
}

fn create_staging_dir(project_root: &Path) -> Result<PathBuf> {
    let staging_root = ensure_owned_directory(project_root, ".staging", "staging root")?;
    for _ in 0..8 {
        let staging = staging_root.join(Uuid::new_v4().to_string());
        match fs::symlink_metadata(&staging) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(staging),
            Ok(_) => continue,
            Err(error) => return Err(error.into()),
        }
    }
    anyhow::bail!("failed to allocate a unique managed staging path")
}

fn ensure_owned_directory(parent: &Path, name: &str, label: &str) -> Result<PathBuf> {
    let path = parent.join(name);
    match fs::symlink_metadata(&path) {
        Ok(metadata) => anyhow::ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "{label} must be a real directory, not a symlink: {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&path)
            .with_context(|| format!("failed to create {label} {}", path.display()))?,
        Err(error) => return Err(error.into()),
    }
    let canonical = fs::canonicalize(&path)
        .with_context(|| format!("failed to canonicalize {label} {}", path.display()))?;
    anyhow::ensure!(
        canonical.parent() == Some(parent),
        "{label} escaped its managed parent: {}",
        canonical.display()
    );
    Ok(canonical)
}

fn existing_owned_directory(parent: &Path, path: &Path, label: &str) -> Result<Option<PathBuf>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    anyhow::ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "{label} must be a real directory, not a symlink: {}",
        path.display()
    );
    let canonical = fs::canonicalize(path)
        .with_context(|| format!("failed to canonicalize {label} {}", path.display()))?;
    anyhow::ensure!(
        canonical.parent() == Some(parent),
        "{label} escaped its managed parent: {}",
        canonical.display()
    );
    Ok(Some(canonical))
}

fn ensure_persistable_git_source(source: &str) -> Result<()> {
    let parsed = url::Url::parse(source)?;
    anyhow::ensure!(
        parsed.scheme() == "https"
            && parsed.password().is_none()
            && parsed.query().is_none()
            && parsed.username().is_empty(),
        "external Git intake requires HTTPS without inline credentials or query parameters; supply credentials out of band through the host"
    );
    Ok(())
}

#[derive(Default)]
struct ExternalTreeStats {
    files: u64,
    directories: u64,
    bytes: u64,
}

fn copy_external_tree_bounded(source: &Path, destination: &Path) -> Result<()> {
    let source_root = fs::canonicalize(source).with_context(|| {
        format!(
            "failed to canonicalize external source root {}",
            source.display()
        )
    })?;
    let mut stats = ExternalTreeStats::default();
    copy_external_tree_entry(&source_root, &source_root, destination, &mut stats)
}

fn copy_external_tree_entry(
    source_root: &Path,
    source: &Path,
    destination: &Path,
    stats: &mut ExternalTreeStats,
) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        if name
            .to_str()
            .is_some_and(|name| EXTERNAL_WORKSPACE_EXCLUDED_NAMES.contains(&name))
        {
            continue;
        }
        let from = entry.path();
        let to = destination.join(&name);
        let metadata = fs::symlink_metadata(&from)?;
        if metadata.is_dir() {
            add_external_tree_directory(stats)?;
            copy_external_tree_entry(source_root, &from, &to, stats)?;
        } else if metadata.is_file() {
            add_external_tree_entry(stats, metadata.len())?;
            fs::copy(&from, &to)?;
        } else if metadata.file_type().is_symlink() {
            let target = fs::read_link(&from)?;
            anyhow::ensure!(
                !target.is_absolute(),
                "external workspace contains an absolute symlink: {}",
                from.display()
            );
            let resolved = fs::canonicalize(from.parent().unwrap_or(source_root).join(&target))
                .with_context(|| {
                    format!(
                        "external workspace contains a dangling symlink: {}",
                        from.display()
                    )
                })?;
            anyhow::ensure!(
                resolved.starts_with(source_root),
                "external workspace symlink escapes its root: {}",
                from.display()
            );
            add_external_tree_entry(stats, target.as_os_str().len() as u64)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target, &to)?;
            #[cfg(windows)]
            {
                if resolved.is_dir() {
                    std::os::windows::fs::symlink_dir(&target, &to)?;
                } else {
                    std::os::windows::fs::symlink_file(&target, &to)?;
                }
            }
        }
    }
    Ok(())
}

fn add_external_tree_directory(stats: &mut ExternalTreeStats) -> Result<()> {
    stats.directories = stats.directories.saturating_add(1);
    anyhow::ensure!(
        stats.directories <= EXTERNAL_WORKSPACE_MAX_DIRECTORIES,
        "external workspace directory count limit exceeded"
    );
    Ok(())
}

fn add_external_tree_entry(stats: &mut ExternalTreeStats, bytes: u64) -> Result<()> {
    stats.files = stats.files.saturating_add(1);
    stats.bytes = stats.bytes.saturating_add(bytes);
    anyhow::ensure!(
        stats.files <= EXTERNAL_WORKSPACE_MAX_FILES,
        "external workspace file count limit exceeded"
    );
    anyhow::ensure!(
        stats.bytes <= EXTERNAL_WORKSPACE_MAX_BYTES,
        "external workspace byte limit exceeded"
    );
    Ok(())
}

async fn promote_staging(
    staging: &Path,
    project_root: &Path,
    digest: &str,
) -> Result<(PathBuf, bool)> {
    let digest_dir = digest
        .strip_prefix("sha256:")
        .unwrap_or(digest)
        .replace(|character: char| !character.is_ascii_alphanumeric(), "-");
    anyhow::ensure!(!digest_dir.is_empty(), "external source digest is empty");
    let destination = project_root.join(digest_dir);
    if let Some(destination) =
        existing_owned_directory(project_root, &destination, "managed workspace digest root")?
    {
        let existing_digest = compute_external_tree_hash(&destination).await?;
        anyhow::ensure!(
            existing_digest == digest,
            "managed workspace digest conflict at {}",
            destination.display()
        );
        fs::remove_dir_all(staging).ok();
        return Ok((destination, true));
    }
    match fs::rename(staging, &destination) {
        Ok(()) => Ok((destination, false)),
        Err(error) => {
            let Some(destination) = existing_owned_directory(
                project_root,
                &destination,
                "managed workspace digest root",
            )?
            else {
                return Err(error).with_context(|| {
                    format!(
                        "failed to atomically promote external workspace {} to {}",
                        staging.display(),
                        destination.display()
                    )
                });
            };
            let existing_digest = compute_external_tree_hash(&destination).await?;
            anyhow::ensure!(
                existing_digest == digest,
                "concurrent managed workspace digest conflict at {}",
                destination.display()
            );
            fs::remove_dir_all(staging).ok();
            Ok((destination, true))
        }
    }
}

fn ensure_non_overlapping_roots(source: &Path, project_root: &Path) -> Result<()> {
    if project_root.starts_with(source) || source.starts_with(project_root) {
        anyhow::bail!(
            "external source and managed workspace roots must not overlap: {} and {}",
            source.display(),
            project_root.display()
        );
    }
    Ok(())
}

fn ensure_bare_external(workspace_root: &Path) -> Result<()> {
    match detect_project_kind(workspace_root)? {
        DetectedProjectKind::External { .. } => Ok(()),
        DetectedProjectKind::Native { .. } | DetectedProjectKind::DeclaredExternal { .. } => {
            anyhow::bail!("source declares project.yaml and must use normal project installation")
        }
    }
}

fn create_workspace_descriptor(source: &MaterializedExternalSource) -> Result<ProjectDescriptor> {
    let id = derive_project_id(&source.source)?;
    let title = derive_title(&source.source);
    let descriptor = ProjectDescriptor {
        schema_version: 1,
        project: ProjectInner {
            id,
            title,
            description: format!("Managed external workspace from {}", source.source),
            project_type: ProjectType::ExternalWorkspace,
            icon: None,
            entry_surface_id: None,
            packages: Vec::new(),
            optional_packages: Vec::new(),
            required_surfaces: Vec::new(),
            required_capabilities: Vec::new(),
            secret_policy: SecretPolicy::default(),
            external: Some(ExternalProjectData {
                source: source.source.clone(),
                source_ref: source.source_ref.clone(),
                adapter_manifest: None,
                workspace_root: Some(source.workspace_root.to_string_lossy().to_string()),
                source_kind: Some(source.source_kind),
                workspace_ownership: Some(source.workspace_ownership),
                source_digest: source.source_digest.clone(),
            }),
            metadata: BTreeMap::new(),
        },
    };
    descriptor.validate()?;
    Ok(descriptor)
}

fn ensure_existing_descriptor_is_compatible(
    descriptor: &ProjectDescriptor,
    data_dir: &Path,
) -> Result<()> {
    let existing_path = data_dir
        .join("projects")
        .join(descriptor.project.id.as_str())
        .join("project.yaml");
    if !existing_path.is_file() {
        return Ok(());
    }
    let existing = read_project_descriptor(&existing_path)?;
    anyhow::ensure!(
        existing == *descriptor,
        "external project id conflict for {}; uninstall the existing project or use its original source/ref",
        descriptor.project.id
    );
    Ok(())
}

fn derive_project_id(source: &str) -> Result<ProjectId> {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let hash = hasher.finalize();
    let suffix = hash[..12]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let slug = source_slug(source);
    let mut safe_slug = slug
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    safe_slug = safe_slug.trim_matches('_').to_string();
    if safe_slug.is_empty() {
        safe_slug = "project".to_string();
    }
    safe_slug.truncate(64);
    ProjectId::new(format!("{safe_slug}__{suffix}"))
}

fn derive_title(source: &str) -> String {
    let slug = source_slug(source);
    if slug.is_empty() {
        "External Project".to_string()
    } else {
        slug
    }
}

fn source_slug(source: &str) -> String {
    if let Ok(parsed) = url::Url::parse(source) {
        return parsed
            .path()
            .trim_matches('/')
            .trim_end_matches(".git")
            .rsplit('/')
            .next()
            .unwrap_or("project")
            .to_string();
    }
    Path::new(source)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_id_uses_stable_wide_source_hash() {
        let first = derive_project_id("https://example.com/acme/tool.git").unwrap();
        let second = derive_project_id("https://example.com/acme/tool.git").unwrap();
        assert_eq!(first, second);
        assert!(first.as_str().starts_with("tool__"));
        assert_eq!(first.as_str().rsplit("__").next().unwrap().len(), 24);
    }

    #[test]
    fn project_id_distinguishes_sources_with_the_same_slug() {
        let first = derive_project_id("https://example.com/acme/tool.git").unwrap();
        let second = derive_project_id("https://mirror.example/acme/tool.git").unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn persisted_git_source_rejects_inline_credentials_and_query_parameters() {
        assert!(ensure_persistable_git_source("https://github.com/acme/tool.git").is_ok());
        assert!(ensure_persistable_git_source("ssh://git@github.com/acme/tool.git").is_err());
        assert!(ensure_persistable_git_source("https://token@github.com/acme/tool.git").is_err());
        assert!(
            ensure_persistable_git_source("https://github.com/acme/tool.git?token=secret").is_err()
        );
    }

    #[test]
    fn managed_local_copy_preserves_source_metadata_and_skips_dependency_caches() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let source = tmp.path().join("source");
        let destination = tmp.path().join("destination");
        fs::create_dir_all(source.join("node_modules"))?;
        fs::create_dir_all(source.join("target"))?;
        fs::write(source.join("app.ts"), "export const app = true;\n")?;
        fs::write(source.join(".gitignore"), "dist/\n")?;
        fs::write(source.join("node_modules/dependency.js"), "ignored\n")?;
        fs::write(source.join("target/artifact"), "ignored\n")?;

        copy_external_tree_bounded(&source, &destination)?;
        assert!(destination.join("app.ts").is_file());
        assert!(destination.join(".gitignore").is_file());
        assert!(!destination.join("node_modules").exists());
        assert!(!destination.join("target").exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn managed_workspace_root_rejects_symlinked_ancestor() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let data_dir = tmp.path().join("data");
        let outside = tmp.path().join("outside");
        fs::create_dir_all(&data_dir)?;
        fs::create_dir_all(&outside)?;
        std::os::unix::fs::symlink(&outside, data_dir.join("workspaces"))?;

        let project_id = ProjectId::new("safe-project")?;
        let error = external_project_workspace_root(&data_dir, &project_id)
            .expect_err("symlinked workspace ancestor must fail closed");
        assert!(error.to_string().contains("not a symlink"));
        Ok(())
    }
}
