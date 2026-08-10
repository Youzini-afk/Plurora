use std::fs;
use std::path::Path;

use anyhow::Result;
use serde_json::{json, Value};
use uuid::Uuid;

use super::executor::invoke_package_capability;
use super::intake::{
    EXTERNAL_WORKSPACE_MAX_BYTES, EXTERNAL_WORKSPACE_MAX_DIRECTORIES, EXTERNAL_WORKSPACE_MAX_FILES,
};
use super::source::{parse_root_descriptor, value_str, SourceDescriptor};
use super::types::{DetectSourceInput, SourceKind};

pub(super) async fn detect_source(input: Value) -> Result<Value> {
    let input: DetectSourceInput = serde_json::from_value(input)?;
    let source = input
        .path
        .or(input.url)
        .ok_or_else(|| anyhow::anyhow!("detect_source requires path or url"))?;
    let descriptor = parse_root_descriptor(&source, &input.root_ref)?;
    let source_kind = detect_descriptor(&descriptor).await?;
    Ok(json!({ "source_kind": source_kind }))
}

pub(super) async fn detect_descriptor(source: &SourceDescriptor) -> Result<SourceKind> {
    match source {
        SourceDescriptor::Local { path } => {
            let root = fs::canonicalize(path)
                .map_err(|_| anyhow::anyhow!("source root could not be opened"))?;
            anyhow::ensure!(root.is_dir(), "source root must be a directory");
            Ok(detect_source_kind(&root))
        }
        SourceDescriptor::Git { url, ref_name } => {
            let resolved = invoke_package_capability(
                "plurora/git-tools-lab",
                "plurora/git-tools-lab/resolve_ref",
                json!({ "remote_url": url, "ref": ref_name }),
            )
            .await?;
            let commit_sha = value_str(&resolved, "commit_sha")?.to_string();
            let temporary =
                std::env::temp_dir().join(format!("plurora-detect-source-{}", Uuid::new_v4()));
            let result = async {
                invoke_package_capability(
                    "plurora/git-tools-lab",
                    "plurora/git-tools-lab/fetch_tree",
                    json!({
                        "remote_url": url,
                        "commit_sha": commit_sha,
                        "ref_name": ref_name,
                        "dest_dir": temporary.to_string_lossy(),
                        "max_files": EXTERNAL_WORKSPACE_MAX_FILES,
                        "max_directories": EXTERNAL_WORKSPACE_MAX_DIRECTORIES,
                        "max_total_bytes": EXTERNAL_WORKSPACE_MAX_BYTES,
                    }),
                )
                .await?;
                Ok(detect_source_kind(&temporary))
            }
            .await;
            remove_temporary_tree(&temporary);
            result
        }
        SourceDescriptor::Internal => anyhow::bail!("internal source cannot be detected"),
    }
}

pub(super) fn detect_source_kind(root: &Path) -> SourceKind {
    if root.join("work.yaml").is_file() {
        SourceKind::Work
    } else if root.join("manifest.yaml").is_file() || root.join("manifest.json").is_file() {
        SourceKind::Package
    } else {
        SourceKind::Foreign
    }
}

fn remove_temporary_tree(path: &Path) {
    if path.is_dir() && !path.is_symlink() {
        let _ = fs::remove_dir_all(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_descriptor_is_neither_opened_nor_used_for_classification() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        fs::write(temporary.path().join("project.yaml"), [0xff, 0xfe, 0xfd])?;
        assert_eq!(detect_source_kind(temporary.path()), SourceKind::Foreign);

        fs::write(temporary.path().join("manifest.json"), "not parsed here")?;
        assert_eq!(detect_source_kind(temporary.path()), SourceKind::Package);

        fs::write(temporary.path().join("work.yaml"), "not parsed here")?;
        assert_eq!(detect_source_kind(temporary.path()), SourceKind::Work);
        Ok(())
    }
}
