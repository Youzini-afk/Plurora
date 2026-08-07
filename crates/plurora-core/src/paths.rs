//! Canonical filesystem paths for Plurora state.
//!
//! Resolution order:
//! 1. `PLURORA_DATA_DIR` env var (explicit override)
//! 2. `XDG_DATA_HOME/plurora` (Linux/BSD; respects user XDG)
//! 3. `~/.plurora/` (default fallback)
//!
//! On macOS, prefer `~/Library/Application Support/plurora` only when
//! `XDG_DATA_HOME` isn't set. Most Plurora users will be in `~/.plurora`
//! since this matches the project name and is least surprising.

use std::path::PathBuf;

use anyhow::Result;

use crate::project::ProjectId;

/// Top-level Plurora data directory.
pub fn data_dir() -> Result<PathBuf> {
    if let Ok(explicit) = std::env::var("PLURORA_DATA_DIR") {
        return Ok(PathBuf::from(explicit));
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("plurora"));
        }
    }
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("could not resolve home directory"))?;
    Ok(home.join(".plurora"))
}

/// Immutable content-addressed package store.
/// Path: `<data_dir>/store/`
pub fn store_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("store"))
}

/// Per-user mutable profiles.
/// Path: `<data_dir>/profiles/`
pub fn profiles_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("profiles"))
}

/// Trusted GPG public keys for signature verification.
/// Path: `<data_dir>/keys/`
pub fn keys_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("keys"))
}

/// Caches (git refs, packfiles, etc.).
/// Path: `<data_dir>/cache/`
pub fn cache_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("cache"))
}

/// Path to the encrypted secret store file.
/// Default: `<data_dir>/secrets.dat`
pub fn secret_store_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("secrets.dat"))
}

/// Path to the secret store master key file (fallback when keyring unavailable).
/// Default: `<data_dir>/secret-store.key`
pub fn secret_store_key_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("secret-store.key"))
}

/// Per-user mutable projects directory.
/// Path: `<data_dir>/projects/`
pub fn projects_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("projects"))
}

/// Path to a specific project's directory.
pub fn project_dir(id: &ProjectId) -> Result<PathBuf> {
    Ok(projects_dir()?.join(id.as_str()))
}

/// Path to a project's workspace clone directory.
/// Path: `<data_dir>/projects/<project_id>/workspace`
pub fn project_workspace_dir(id: &ProjectId) -> Result<PathBuf> {
    Ok(project_dir(id)?.join("workspace"))
}

/// Path to a project's workspace clone directory under an explicit data dir.
/// Path: `<data_dir>/projects/<project_id>/workspace`
pub fn project_workspace_dir_in(data_dir: impl AsRef<std::path::Path>, id: &ProjectId) -> PathBuf {
    data_dir
        .as_ref()
        .join("projects")
        .join(id.as_str())
        .join("workspace")
}

/// Path to a project's encrypted secret store.
pub fn project_secret_store_path(id: &ProjectId) -> Result<PathBuf> {
    Ok(project_dir(id)?.join("secrets.dat"))
}

/// Path to a project's lockfile.
pub fn project_lockfile_path(id: &ProjectId) -> Result<PathBuf> {
    Ok(project_dir(id)?.join("lockfile.toml"))
}

/// Path to a project's project.yaml descriptor.
pub fn project_descriptor_path(id: &ProjectId) -> Result<PathBuf> {
    Ok(project_dir(id)?.join("project.yaml"))
}

/// Per-project sessions directory (where event-store backends may store project data).
pub fn project_sessions_dir(id: &ProjectId) -> Result<PathBuf> {
    Ok(project_dir(id)?.join("sessions"))
}

/// Per-project state directory (where capability packages may store project-scoped state).
pub fn project_state_dir(id: &ProjectId) -> Result<PathBuf> {
    Ok(project_dir(id)?.join("state"))
}

/// Archived projects directory (soft-delete parking).
/// Path: `<data_dir>/projects/.archived/`
pub fn archived_projects_dir() -> Result<PathBuf> {
    Ok(projects_dir()?.join(".archived"))
}

/// Path where an archived project lives.
pub fn archived_project_dir(id: &ProjectId) -> Result<PathBuf> {
    Ok(archived_projects_dir()?.join(id.as_str()))
}

/// Initialize a project's directory layout. Creates dirs with 0700 on Unix.
pub fn ensure_project_initialized(id: &ProjectId) -> Result<()> {
    use std::fs;

    fs::create_dir_all(project_dir(id)?)?;
    fs::create_dir_all(project_sessions_dir(id)?)?;
    fs::create_dir_all(project_state_dir(id)?)?;
    fs::create_dir_all(project_workspace_dir(id)?)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let p = project_dir(id)?;
        let mut perms = fs::metadata(&p)?.permissions();
        perms.set_mode(0o700);
        fs::set_permissions(&p, perms)?;
    }

    Ok(())
}

/// Initialize the directory layout if missing. Creates all directories with
/// 0700 permissions on Unix.
pub fn ensure_initialized() -> Result<()> {
    use std::fs;

    let data = data_dir()?;
    fs::create_dir_all(&data)?;
    fs::create_dir_all(store_dir()?)?;
    fs::create_dir_all(profiles_dir()?)?;
    fs::create_dir_all(keys_dir()?)?;
    fs::create_dir_all(cache_dir()?)?;
    fs::create_dir_all(projects_dir()?)?;
    fs::create_dir_all(archived_projects_dir()?)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&data)?.permissions();
        perms.set_mode(0o700);
        fs::set_permissions(&data, perms)?;
    }

    Ok(())
}

/// Path to the lockfile for a specific profile.
pub fn lockfile_path(profile: &str) -> Result<PathBuf> {
    Ok(profiles_dir()?.join(format!("{profile}.lock.toml")))
}

/// Path to the profile YAML for a specific profile name.
pub fn profile_path(profile: &str) -> Result<PathBuf> {
    Ok(profiles_dir()?.join(format!("{profile}.yaml")))
}

/// Compute store path for a tree hash.
pub fn store_path_for_hash(tree_hash: &str) -> Result<PathBuf> {
    // tree_hash looks like "sha256:abc..."; convert to "sha256-abc..."
    // (filesystem-safe: no colons)
    let safe = tree_hash.replace(':', "-");
    Ok(store_dir()?.join(safe))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        plurora_data_dir: Option<String>,
        xdg_data_home: Option<String>,
        _lock: MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        fn lock() -> Self {
            let lock = ENV_LOCK
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            Self {
                plurora_data_dir: std::env::var("PLURORA_DATA_DIR").ok(),
                xdg_data_home: std::env::var("XDG_DATA_HOME").ok(),
                _lock: lock,
            }
        }
    }

    fn scope_env(key: &str, value: &str) -> EnvGuard {
        let guard = EnvGuard::lock();
        std::env::set_var(key, value);
        guard
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.plurora_data_dir {
                Some(value) => std::env::set_var("PLURORA_DATA_DIR", value),
                None => std::env::remove_var("PLURORA_DATA_DIR"),
            }
            match &self.xdg_data_home {
                Some(value) => std::env::set_var("XDG_DATA_HOME", value),
                None => std::env::remove_var("XDG_DATA_HOME"),
            }
        }
    }

    #[test]
    fn plurora_data_dir_env_override() {
        let _guard = EnvGuard::lock();
        std::env::set_var("PLURORA_DATA_DIR", "/tmp/test-plurora");
        let dir = data_dir().unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/test-plurora"));
        std::env::remove_var("PLURORA_DATA_DIR");
    }

    #[test]
    fn xdg_data_home_used_when_set() {
        let _guard = EnvGuard::lock();
        std::env::remove_var("PLURORA_DATA_DIR");
        std::env::set_var("XDG_DATA_HOME", "/tmp/test-xdg");
        let dir = data_dir().unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/test-xdg/plurora"));
        std::env::remove_var("XDG_DATA_HOME");
    }

    #[test]
    fn default_falls_back_to_home_dot_plurora() {
        let _guard = EnvGuard::lock();
        if let Some(home) = dirs::home_dir() {
            std::env::remove_var("PLURORA_DATA_DIR");
            std::env::remove_var("XDG_DATA_HOME");
            let dir = data_dir().unwrap();
            assert_eq!(dir, home.join(".plurora"));
        }
    }

    #[test]
    fn store_path_for_hash_strips_colon() {
        let _guard = EnvGuard::lock();
        std::env::set_var("PLURORA_DATA_DIR", "/tmp/test");
        let path = store_path_for_hash("sha256:abc123").unwrap();
        assert_eq!(path, PathBuf::from("/tmp/test/store/sha256-abc123"));
        std::env::remove_var("PLURORA_DATA_DIR");
    }

    #[test]
    fn secret_store_paths_use_data_dir() {
        let _guard = EnvGuard::lock();
        std::env::set_var("PLURORA_DATA_DIR", "/tmp/test-plurora-secrets");
        assert_eq!(
            secret_store_path().unwrap(),
            PathBuf::from("/tmp/test-plurora-secrets/secrets.dat")
        );
        assert_eq!(
            secret_store_key_path().unwrap(),
            PathBuf::from("/tmp/test-plurora-secrets/secret-store.key")
        );
        std::env::remove_var("PLURORA_DATA_DIR");
    }

    #[test]
    fn ensure_initialized_creates_layout() {
        let _guard = EnvGuard::lock();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("PLURORA_DATA_DIR", tmp.path().display().to_string());
        ensure_initialized().unwrap();
        assert!(tmp.path().join("store").exists());
        assert!(tmp.path().join("profiles").exists());
        assert!(tmp.path().join("keys").exists());
        assert!(tmp.path().join("cache").exists());
        assert!(tmp.path().join("projects").exists());
        assert!(tmp.path().join("projects/.archived").exists());
        std::env::remove_var("PLURORA_DATA_DIR");
    }

    #[test]
    fn project_dir_uses_data_dir() {
        let _guard = scope_env("PLURORA_DATA_DIR", "/tmp/plurora-test-paths");
        let id = ProjectId::new("foo__abc123").unwrap();
        let pd = project_dir(&id).unwrap();
        assert_eq!(
            pd,
            PathBuf::from("/tmp/plurora-test-paths/projects/foo__abc123")
        );
    }

    #[test]
    fn project_workspace_dir_uses_expected_layout() {
        let _guard = scope_env("PLURORA_DATA_DIR", "/tmp/plurora-test-paths");
        let id = ProjectId::new("foo__abc123").unwrap();
        assert_eq!(
            project_workspace_dir(&id).unwrap(),
            PathBuf::from("/tmp/plurora-test-paths/projects/foo__abc123/workspace")
        );
        assert_eq!(
            project_workspace_dir_in("/custom/data", &id),
            PathBuf::from("/custom/data/projects/foo__abc123/workspace")
        );
    }

    #[test]
    fn project_secret_store_path_format() {
        let _guard = scope_env("PLURORA_DATA_DIR", "/tmp/plurora-test-paths");
        let id = ProjectId::new("test__xyz").unwrap();
        let p = project_secret_store_path(&id).unwrap();
        assert!(p.ends_with("projects/test__xyz/secrets.dat"));
    }

    #[test]
    fn ensure_project_initialized_creates_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = scope_env("PLURORA_DATA_DIR", tmp.path().to_str().unwrap());
        let id = ProjectId::new("init__test").unwrap();
        ensure_project_initialized(&id).unwrap();
        assert!(project_dir(&id).unwrap().exists());
        assert!(project_sessions_dir(&id).unwrap().exists());
        assert!(project_state_dir(&id).unwrap().exists());
        assert!(project_workspace_dir(&id).unwrap().exists());
    }
}
