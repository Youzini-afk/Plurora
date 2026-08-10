use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use uuid::Uuid;

pub(super) fn ensure_layout(data_dir_override: Option<&str>) -> Result<PathBuf> {
    let configured = match data_dir_override {
        Some(path) => PathBuf::from(path),
        None => plurora_core::paths::data_dir()?,
    };
    fs::create_dir_all(&configured)?;
    let data = fs::canonicalize(&configured)
        .map_err(|_| anyhow::anyhow!("Host data directory could not be initialized"))?;
    anyhow::ensure!(data.is_dir(), "Host data root must be a directory");
    ensure_real_child(&data, "objects", "object root")?;
    ensure_real_child(&data, "workspaces", "workspace root")?;
    ensure_real_child(&data, "cache", "cache root")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&data)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&data, permissions)?;
    }
    Ok(data)
}

pub(super) fn objects_dir(data_dir_override: Option<&str>) -> Result<PathBuf> {
    Ok(ensure_layout(data_dir_override)?.join("objects"))
}

pub(super) fn workspaces_dir(data_dir_override: Option<&str>) -> Result<PathBuf> {
    Ok(ensure_layout(data_dir_override)?.join("workspaces"))
}

pub(super) fn ensure_real_child(parent: &Path, name: &str, label: &str) -> Result<PathBuf> {
    let path = parent.join(name);
    match fs::symlink_metadata(&path) {
        Ok(metadata) => anyhow::ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "{label} must be a real directory"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&path)?,
        Err(error) => return Err(error.into()),
    };
    let canonical = fs::canonicalize(&path)
        .map_err(|_| anyhow::anyhow!("{label} could not be canonicalized"))?;
    anyhow::ensure!(
        canonical.parent() == Some(parent),
        "{label} escaped its managed parent"
    );
    Ok(canonical)
}

pub(super) fn default_head_ref() -> String {
    "HEAD".to_string()
}

pub(super) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("atomic write target has no parent"))?;
    let temporary = parent.join(format!(".tmp-{}", Uuid::new_v4()));
    fs::write(&temporary, bytes)?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_creates_only_phase_three_install_lab_roots() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let data = ensure_layout(Some(temporary.path().to_string_lossy().as_ref()))?;
        assert!(data.join("objects").is_dir());
        assert!(data.join("workspaces").is_dir());
        assert!(data.join("cache").is_dir());
        assert!(!data.join("store").exists());
        assert!(!data.join("profiles").exists());
        assert!(!data.join("installations").exists());
        assert!(!data.join("runs").exists());
        Ok(())
    }
}
