use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use plurora_core::{paths, ProjectDescriptor, ProjectType};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::inproc::project_registry_from_inproc;

use super::executor::invoke_package_capability;
use super::intake::{
    EXTERNAL_WORKSPACE_MAX_BYTES, EXTERNAL_WORKSPACE_MAX_DIRECTORIES, EXTERNAL_WORKSPACE_MAX_FILES,
};
use super::layout::atomic_write;
use super::source::{
    manifest_path_in, parse_project_descriptor_at, parse_root_descriptor, value_str,
};
use super::types::{DetectKindInput, DetectedProjectKind, SourceDescriptor};

pub(super) async fn detect_kind(input: Value) -> Result<Value> {
    let input: DetectKindInput = serde_json::from_value(input)?;
    let source = input
        .path
        .or(input.url)
        .ok_or_else(|| anyhow::anyhow!("detect_kind requires path or url"))?;
    let root = parse_root_descriptor(&source, &input.root_ref)?;
    let detected = match root.source {
        SourceDescriptor::Local { path } => detect_project_kind(&path)?,
        SourceDescriptor::Git { url, ref_name } => {
            let resolved = invoke_package_capability(
                "plurora/git-tools-lab",
                "plurora/git-tools-lab/resolve_ref",
                json!({ "remote_url": url, "ref": ref_name }),
            )
            .await?;
            let commit_sha = value_str(&resolved, "commit_sha")?.to_string();
            let tmp = std::env::temp_dir().join(format!("plurora-detect-kind-{}", Uuid::new_v4()));
            let result = async {
                invoke_package_capability(
                    "plurora/git-tools-lab",
                    "plurora/git-tools-lab/fetch_tree",
                    json!({
                        "remote_url": url,
                        "commit_sha": commit_sha,
                        "dest_dir": tmp.to_string_lossy(),
                        "max_files": EXTERNAL_WORKSPACE_MAX_FILES,
                        "max_directories": EXTERNAL_WORKSPACE_MAX_DIRECTORIES,
                        "max_total_bytes": EXTERNAL_WORKSPACE_MAX_BYTES,
                    }),
                )
                .await?;
                detect_project_kind(&tmp)
            }
            .await;
            fs::remove_dir_all(&tmp).ok();
            result?
        }
        SourceDescriptor::Internal => anyhow::bail!("internal packages cannot be detected"),
    };
    Ok(serde_json::to_value(detected)?)
}

pub(super) fn detect_project_kind(staging_dir: &Path) -> Result<DetectedProjectKind> {
    let project_yaml = staging_dir.join("project.yaml");
    if project_yaml.exists() {
        let descriptor = read_project_descriptor(&project_yaml)?;
        return Ok(match descriptor.project.project_type {
            ProjectType::PluroraNative => DetectedProjectKind::Native { descriptor },
            ProjectType::ExternalWrapped | ProjectType::ExternalWorkspace => {
                DetectedProjectKind::DeclaredExternal { descriptor }
            }
        });
    }

    Ok(DetectedProjectKind::External {
        has_manifest_yaml: manifest_path_in(staging_dir).is_ok(),
    })
}

pub(super) fn read_project_descriptor(path: &Path) -> Result<ProjectDescriptor> {
    parse_project_descriptor_at(path)
}

pub(super) fn write_and_register_project(
    descriptor: ProjectDescriptor,
    data_dir_override: Option<&str>,
) -> Result<Value> {
    descriptor.validate()?;
    let project_id = descriptor.project.id.clone();
    let project_dir = ensure_project_initialized_for(&project_id, data_dir_override)?;
    let descriptor_path = project_dir.join("project.yaml");
    let descriptor_exists = match fs::symlink_metadata(&descriptor_path) {
        Ok(metadata) => {
            anyhow::ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "project descriptor must be a real file, not a symlink: {}",
                descriptor_path.display()
            );
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    if descriptor_exists {
        let existing = read_project_descriptor(&descriptor_path)?;
        if existing == descriptor {
            project_registry_from_inproc()?.register(descriptor)?;
            return Ok(json!({
                "project_id": project_id.as_str(),
                "project_dir": project_dir.to_string_lossy(),
                "created": false,
                "idempotent": true,
            }));
        }
        if matches!(
            existing.project.project_type,
            ProjectType::ExternalWrapped | ProjectType::ExternalWorkspace
        ) || matches!(
            descriptor.project.project_type,
            ProjectType::ExternalWrapped | ProjectType::ExternalWorkspace
        ) {
            anyhow::bail!(
                "external project id conflict for {}; existing descriptor differs",
                project_id
            );
        }
    }
    let yaml = serde_yaml::to_string(&descriptor)?;
    atomic_write(&descriptor_path, yaml.as_bytes())?;
    project_registry_from_inproc()?.register(descriptor)?;
    Ok(json!({
        "project_id": project_id.as_str(),
        "project_dir": project_dir.to_string_lossy(),
        "created": true,
        "idempotent": false,
    }))
}

pub(super) fn ensure_project_initialized_for(
    project_id: &plurora_core::ProjectId,
    data_dir_override: Option<&str>,
) -> Result<PathBuf> {
    let configured_data_dir = data_dir_override
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(paths::data_dir)?;
    let data_dir = fs::canonicalize(&configured_data_dir).with_context(|| {
        format!(
            "failed to canonicalize data directory {}",
            configured_data_dir.display()
        )
    })?;
    let projects = ensure_real_project_child(&data_dir, "projects", "projects root")?;
    let project_dir =
        ensure_real_project_child(&projects, project_id.as_str(), "managed project root")?;
    ensure_real_project_child(&project_dir, "sessions", "project sessions directory")?;
    ensure_real_project_child(&project_dir, "state", "project state directory")?;
    if data_dir_override.is_none() {
        ensure_real_project_child(&project_dir, "workspace", "project workspace directory")?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&project_dir)?.permissions();
        perms.set_mode(0o700);
        fs::set_permissions(&project_dir, perms)?;
    }
    Ok(project_dir)
}

fn ensure_real_project_child(parent: &Path, name: &str, label: &str) -> Result<PathBuf> {
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn project_initialization_rejects_symlinked_projects_root() -> Result<()> {
        let data_dir = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        symlink(outside.path(), data_dir.path().join("projects"))?;
        let project_id = plurora_core::ProjectId::new("external__boundary-test")?;

        let result = ensure_project_initialized_for(
            &project_id,
            Some(data_dir.path().to_string_lossy().as_ref()),
        );

        assert!(result.is_err());
        assert!(!outside.path().join(project_id.as_str()).exists());
        Ok(())
    }
}
