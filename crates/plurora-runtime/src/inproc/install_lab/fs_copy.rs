use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use uuid::Uuid;

use super::executor::{compute_manifest_hash, compute_tree_hash};
use super::source::manifest_path_in;
use super::types::PlannedPackage;

pub(super) fn copy_dir_atomic(src: &Path, staging: &Path, dest: &Path) -> Result<()> {
    if staging.exists() {
        fs::remove_dir_all(staging).ok();
    }
    copy_dir_recursive(src, staging)?;
    fs::rename(staging, dest).with_context(|| {
        format!(
            "failed to atomically move {} to {}",
            staging.display(),
            dest.display()
        )
    })?;
    Ok(())
}

pub(super) fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        let meta = fs::symlink_metadata(&from)?;
        if meta.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if meta.is_file() {
            fs::copy(&from, &to)?;
        } else if meta.file_type().is_symlink() {
            let target = fs::read_link(&from)?;
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target, &to)?;
            #[cfg(windows)]
            {
                let resolved = from.parent().unwrap_or(src).join(&target);
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

pub(super) fn replace_dir_atomic(src: &Path, dest: &Path) -> Result<()> {
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(
        ".{}.tmp-{}",
        dest.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("dist"),
        Uuid::new_v4()
    ));
    let backup = parent.join(format!(
        ".{}.bak-{}",
        dest.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("dist"),
        Uuid::new_v4()
    ));
    if tmp.exists() {
        fs::remove_dir_all(&tmp).ok();
    }
    copy_dir_recursive(src, &tmp)?;
    let had_dest = dest.exists();
    if had_dest {
        fs::rename(dest, &backup).with_context(|| {
            format!(
                "failed to move existing {} to {}",
                dest.display(),
                backup.display()
            )
        })?;
    }
    if let Err(error) = fs::rename(&tmp, dest) {
        if had_dest {
            let _ = fs::rename(&backup, dest);
        }
        let _ = fs::remove_dir_all(&tmp);
        return Err(error).with_context(|| {
            format!(
                "failed to atomically replace {} with {}",
                dest.display(),
                src.display()
            )
        });
    }
    if had_dest {
        fs::remove_dir_all(&backup).ok();
    }
    Ok(())
}

pub(super) async fn verify_installed_hashes(pkg: &PlannedPackage, store_path: &Path) -> Result<()> {
    let manifest_hash = compute_manifest_hash(&installed_manifest_path(pkg, store_path)?).await?;
    if manifest_hash != pkg.manifest_hash {
        anyhow::bail!("manifest hash mismatch for {}", pkg.id);
    }
    let tree_hash = compute_tree_hash(store_path).await?;
    if tree_hash != pkg.tree_hash {
        anyhow::bail!("tree hash mismatch for {}", pkg.id);
    }
    Ok(())
}

pub(super) fn installed_manifest_path(
    pkg: &PlannedPackage,
    store_path: &Path,
) -> Result<std::path::PathBuf> {
    if let Some(relative) = &pkg.manifest_relative_path {
        return Ok(store_path.join(safe_relative_path(relative)?));
    }
    manifest_path_in(store_path)
}

pub(super) fn safe_relative_path(path: &str) -> Result<std::path::PathBuf> {
    let path = Path::new(path);
    if path.is_absolute() {
        anyhow::bail!("relative path must not be absolute");
    }
    let mut out = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(part) => out.push(part),
            std::path::Component::CurDir => {}
            _ => anyhow::bail!("relative path must stay inside root"),
        }
    }
    if out.as_os_str().is_empty() {
        anyhow::bail!("relative path must not be empty");
    }
    Ok(out)
}
