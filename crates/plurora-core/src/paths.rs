//! Canonical filesystem paths for Plurora state.
//!
//! Resolution order:
//! 1. `PLURORA_DATA_DIR` env var (explicit override)
//! 2. `XDG_DATA_HOME/plurora` (Linux/BSD; respects user XDG)
//! 3. `~/.plurora/` (default fallback)
//!
//! On macOS, prefer `~/Library/Application Support/plurora` only when
//! `XDG_DATA_HOME` isn't set. Most Plurora users will be in `~/.plurora`
//! since this matches the Plurora data name and is least surprising.

use std::path::PathBuf;

use anyhow::Result;

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

/// Immutable content-addressed object store.
/// Path: `<data_dir>/objects/`
pub fn objects_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("objects"))
}

/// Host-owned installation records and state.
/// Path: `<data_dir>/installations/`
pub fn installations_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("installations"))
}

/// Mutable authoring and import workspaces.
/// Path: `<data_dir>/workspaces/`
pub fn workspaces_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("workspaces"))
}

/// Runtime-local state and diagnostics.
/// Path: `<data_dir>/runtime/`
pub fn runtime_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("runtime"))
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
    fs::create_dir_all(objects_dir()?)?;
    fs::create_dir_all(installations_dir()?)?;
    fs::create_dir_all(workspaces_dir()?)?;
    fs::create_dir_all(runtime_dir()?)?;

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
        assert!(tmp.path().join("objects").exists());
        assert!(tmp.path().join("installations").exists());
        assert!(tmp.path().join("workspaces").exists());
        assert!(tmp.path().join("runtime").exists());
    }
}
