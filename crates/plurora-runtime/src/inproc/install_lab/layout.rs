use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use plurora_core::paths;
use uuid::Uuid;

use crate::inproc::integrity_lab::TREE_HASH_SCHEMA_VERSION;

const STORE_SCHEMA_MARKER: &str = ".schema_version";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreSchemaMigration {
    pub from: Option<u32>,
    pub to: u32,
    pub preserved_path: Option<PathBuf>,
    pub preserved_paths_count: usize,
}

pub(super) fn ensure_layout(data_dir_override: Option<&str>) -> Result<()> {
    if let Some(dir) = data_dir_override {
        let data = PathBuf::from(dir);
        fs::create_dir_all(&data)?;
        fs::create_dir_all(data.join("store"))?;
        fs::create_dir_all(data.join("profiles"))?;
        fs::create_dir_all(data.join("keys"))?;
        fs::create_dir_all(data.join("cache"))?;
        fs::create_dir_all(data.join("projects"))?;
        fs::create_dir_all(data.join("projects/.archived"))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&data)?.permissions();
            perms.set_mode(0o700);
            fs::set_permissions(&data, perms)?;
        }

        return Ok(());
    }
    paths::ensure_initialized()
}

pub fn ensure_store_schema(data_dir: &Path) -> Result<Option<StoreSchemaMigration>> {
    let store = data_dir.join("store");
    fs::create_dir_all(&store)?;
    ensure_store_schema_at(&store)
}

fn ensure_store_schema_at(store: &Path) -> Result<Option<StoreSchemaMigration>> {
    let marker = store.join(STORE_SCHEMA_MARKER);
    let current = read_schema_marker(&marker)?;
    if current == Some(TREE_HASH_SCHEMA_VERSION) {
        return Ok(None);
    }

    let mut preserved_paths_count = 0usize;
    for entry in fs::read_dir(store)? {
        let entry = entry?;
        if entry.file_name() == STORE_SCHEMA_MARKER {
            continue;
        }
        preserved_paths_count += 1;
    }

    let marker_exists = marker.try_exists()?;
    let preserved_path = if marker_exists || preserved_paths_count > 0 {
        let parent = store
            .parent()
            .ok_or_else(|| anyhow::anyhow!("install store has no parent directory"))?;
        let store_name = store
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("store");
        let from = current
            .map(|version| format!("v{version}"))
            .unwrap_or_else(|| "unknown".to_string());
        let preserved = parent.join(format!(
            "{store_name}.preserved-{from}-{}",
            Uuid::new_v4().simple()
        ));
        fs::rename(store, &preserved)?;
        fs::create_dir_all(store)?;
        Some(preserved)
    } else {
        None
    };

    atomic_write(&marker, TREE_HASH_SCHEMA_VERSION.to_string().as_bytes())?;
    Ok(Some(StoreSchemaMigration {
        from: current,
        to: TREE_HASH_SCHEMA_VERSION,
        preserved_path,
        preserved_paths_count,
    }))
}

fn read_schema_marker(marker: &Path) -> Result<Option<u32>> {
    match fs::read_to_string(marker) {
        Ok(raw) => Ok(raw.trim().parse::<u32>().ok()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn store_dir(data_dir_override: Option<&str>) -> Result<PathBuf> {
    if let Some(dir) = data_dir_override {
        return Ok(PathBuf::from(dir).join("store"));
    }
    paths::store_dir()
}

pub(super) fn profiles_dir(data_dir_override: Option<&str>) -> Result<PathBuf> {
    if let Some(dir) = data_dir_override {
        return Ok(PathBuf::from(dir).join("profiles"));
    }
    paths::profiles_dir()
}

pub(super) fn lockfile_path(profile: &str, data_dir_override: Option<&str>) -> Result<PathBuf> {
    if let Some(dir) = data_dir_override {
        return Ok(PathBuf::from(dir)
            .join("profiles")
            .join(format!("{profile}.lock.toml")));
    }
    paths::lockfile_path(profile)
}

pub(super) fn profile_path(profile: &str, data_dir_override: Option<&str>) -> Result<PathBuf> {
    if let Some(dir) = data_dir_override {
        return Ok(PathBuf::from(dir)
            .join("profiles")
            .join(format!("{profile}.yaml")));
    }
    paths::profile_path(profile)
}

pub(super) fn store_path_for_hash(
    tree_hash: &str,
    data_dir_override: Option<&str>,
) -> Result<PathBuf> {
    if let Some(dir) = data_dir_override {
        return Ok(PathBuf::from(dir)
            .join("store")
            .join(tree_hash.replace(':', "-")));
    }
    paths::store_path_for_hash(tree_hash)
}

pub(super) fn default_profile() -> String {
    "default".to_string()
}

pub(super) fn default_head_ref() -> String {
    "HEAD".to_string()
}

pub(super) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_migration_preserves_store_contents_and_is_idempotent() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let data = tmp.path();
        let store = data.join("store");
        fs::create_dir_all(store.join("sha256-old"))?;
        fs::write(store.join("sha256-old/manifest.yaml"), "id: old\n")?;
        fs::write(store.join("loose-file"), "stale")?;
        fs::write(store.join(STORE_SCHEMA_MARKER), "1\n")?;

        let migrated = ensure_store_schema(data)?.expect("schema should migrate");
        assert_eq!(migrated.from, Some(1));
        assert_eq!(migrated.to, TREE_HASH_SCHEMA_VERSION);
        assert_eq!(migrated.preserved_paths_count, 2);
        assert!(!store.join("sha256-old").exists());
        assert!(!store.join("loose-file").exists());
        let preserved = migrated
            .preserved_path
            .expect("old store should be preserved");
        assert_eq!(
            fs::read_to_string(preserved.join("sha256-old/manifest.yaml"))?,
            "id: old\n"
        );
        assert_eq!(fs::read_to_string(preserved.join("loose-file"))?, "stale");
        assert_eq!(
            fs::read_to_string(preserved.join(STORE_SCHEMA_MARKER))?,
            "1\n"
        );
        assert_eq!(
            fs::read_to_string(store.join(STORE_SCHEMA_MARKER))?,
            TREE_HASH_SCHEMA_VERSION.to_string()
        );

        fs::create_dir_all(store.join("sha256-current"))?;
        let second = ensure_store_schema(data)?;
        assert!(second.is_none());
        assert!(store.join("sha256-current").is_dir());
        assert_eq!(
            fs::read_to_string(store.join(STORE_SCHEMA_MARKER))?,
            TREE_HASH_SCHEMA_VERSION.to_string()
        );
        Ok(())
    }

    #[test]
    fn first_schema_marker_does_not_quarantine_an_empty_store() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let migration = ensure_store_schema(tmp.path())?.expect("schema should initialize");
        assert_eq!(migration.from, None);
        assert_eq!(migration.preserved_path, None);
        assert_eq!(migration.preserved_paths_count, 0);
        assert_eq!(
            fs::read_to_string(tmp.path().join("store").join(STORE_SCHEMA_MARKER))?,
            TREE_HASH_SCHEMA_VERSION.to_string()
        );
        Ok(())
    }
}
