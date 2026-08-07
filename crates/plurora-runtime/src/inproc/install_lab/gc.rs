use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use plurora_core::Lockfile;

use super::layout::{profiles_dir, store_dir, store_path_for_hash};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct StoreGcReport {
    pub(crate) removed_paths: Vec<PathBuf>,
    pub(crate) orphaned_paths: Vec<PathBuf>,
    pub(crate) ignored_store_entries: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub(crate) struct StoreCandidate {
    path: PathBuf,
    canonical_path: PathBuf,
    tree_hash: String,
}

pub(crate) fn prune_orphaned_stores(data_dir_override: Option<&str>) -> Result<StoreGcReport> {
    let mut report = StoreGcReport::default();
    for candidate in collect_orphaned_stores(data_dir_override, &mut report)? {
        if !is_still_safe_directory_candidate(&candidate.path)? {
            report.ignored_store_entries.push(candidate.path);
            continue;
        }
        fs::remove_dir_all(&candidate.path)?;
        report.removed_paths.push(candidate.path);
    }
    Ok(report)
}

pub(crate) fn collect_orphaned_stores(
    data_dir_override: Option<&str>,
    report: &mut StoreGcReport,
) -> Result<Vec<StoreCandidate>> {
    let candidates = store_candidates(data_dir_override, report)?;
    let mut orphaned = Vec::new();
    for candidate in candidates {
        if store_path_refcount(&candidate, data_dir_override)? == 0 {
            report.orphaned_paths.push(candidate.path.clone());
            orphaned.push(candidate);
        }
    }
    Ok(orphaned)
}

fn store_path_refcount(
    candidate: &StoreCandidate,
    data_dir_override: Option<&str>,
) -> Result<usize> {
    let profiles = profiles_dir(data_dir_override)?;
    let Ok(entries) = fs::read_dir(&profiles) else {
        return Ok(0);
    };

    let mut count = 0usize;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !is_lockfile_path(&path) {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(lockfile) = toml::from_str::<Lockfile>(&raw) else {
            continue;
        };
        for package in lockfile.package {
            if !is_valid_tree_hash(&package.tree_hash) {
                continue;
            }
            if package.tree_hash != candidate.tree_hash {
                continue;
            }
            let expected = store_path_for_hash(&package.tree_hash, data_dir_override)?;
            if !same_existing_path(&expected, &candidate.canonical_path)? {
                continue;
            }
            if installed_store_points_to_candidate(
                Path::new(&package.installed_at_store),
                &candidate.canonical_path,
            )? {
                count += 1;
            }
        }
    }
    Ok(count)
}

fn store_candidates(
    data_dir_override: Option<&str>,
    report: &mut StoreGcReport,
) -> Result<Vec<StoreCandidate>> {
    let store = store_dir(data_dir_override)?;
    let Ok(store_canonical) = store.canonicalize() else {
        return Ok(Vec::new());
    };
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in fs::read_dir(&store)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();
        let Some(tree_hash) = tree_hash_from_store_entry_name(&file_name) else {
            report.ignored_store_entries.push(path);
            continue;
        };
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            report.ignored_store_entries.push(path);
            continue;
        }
        let Ok(canonical_path) = path.canonicalize() else {
            report.ignored_store_entries.push(path);
            continue;
        };
        if canonical_path.parent() != Some(store_canonical.as_path()) {
            report.ignored_store_entries.push(path);
            continue;
        }
        if seen.insert(canonical_path.clone()) {
            candidates.push(StoreCandidate {
                path,
                canonical_path,
                tree_hash,
            });
        }
    }
    Ok(candidates)
}

fn is_lockfile_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".lock.toml"))
}

fn installed_store_points_to_candidate(
    installed_at_store: &Path,
    candidate_canonical: &Path,
) -> Result<bool> {
    match installed_at_store.canonicalize() {
        Ok(canonical) => Ok(canonical == candidate_canonical),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn same_existing_path(path: &Path, expected_canonical: &Path) -> Result<bool> {
    match path.canonicalize() {
        Ok(canonical) => Ok(canonical == expected_canonical),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn is_still_safe_directory_candidate(path: &Path) -> Result<bool> {
    let metadata = fs::symlink_metadata(path)?;
    Ok(metadata.is_dir() && !metadata.file_type().is_symlink())
}

fn tree_hash_from_store_entry_name(name: &str) -> Option<String> {
    let hex = name.strip_prefix("sha256-")?;
    if hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        let tree_hash = format!("sha256:{hex}");
        if is_valid_tree_hash(&tree_hash) {
            return Some(tree_hash);
        }
    }
    None
}

fn is_valid_tree_hash(tree_hash: &str) -> bool {
    let Some(hex) = tree_hash.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use plurora_core::{LockEntry, LockSource};

    fn hash(ch: char) -> String {
        format!("sha256:{}", ch.to_string().repeat(64))
    }

    fn store_path(data: &Path, tree_hash: &str) -> PathBuf {
        data.join("store").join(tree_hash.replace(':', "-"))
    }

    fn write_lockfile(data: &Path, profile: &str, entries: Vec<LockEntry>) -> Result<()> {
        let profiles = data.join("profiles");
        fs::create_dir_all(&profiles)?;
        let mut lockfile = Lockfile::new(profile, "sha256:profile");
        lockfile.package = entries;
        fs::write(
            profiles.join(format!("{profile}.lock.toml")),
            toml::to_string_pretty(&lockfile)?,
        )?;
        Ok(())
    }

    fn lock_entry(id: &str, tree_hash: &str, installed_at_store: &Path) -> LockEntry {
        LockEntry {
            id: id.to_string(),
            version: "1.0.0".to_string(),
            source: LockSource::Local,
            url: None,
            source_path: None,
            r#ref: None,
            commit: None,
            tree_hash: tree_hash.to_string(),
            manifest_hash: hash('f'),
            surface_bundle_hash: None,
            package_envelope_digest: None,
            component_pins: Vec::new(),
            protocol_profile_pins: Vec::new(),
            content_roots: Vec::new(),
            signed: false,
            signed_by: None,
            installed_at_store: installed_at_store.to_string_lossy().to_string(),
            manifest_relative_path: None,
            granted_capabilities: Vec::new(),
            granted_network: Vec::new(),
            granted_secrets: Vec::new(),
            requires: Vec::new(),
        }
    }

    #[test]
    fn shared_store_path_is_preserved_until_last_reference_is_removed() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let data = tmp.path();
        let tree_hash = hash('c');
        let shared_store = store_path(data, &tree_hash);
        fs::create_dir_all(&shared_store)?;
        fs::write(shared_store.join("manifest.yaml"), "id: shared\n")?;
        write_lockfile(
            data,
            "alpha",
            vec![lock_entry("shared", &tree_hash, &shared_store)],
        )?;
        write_lockfile(
            data,
            "beta",
            vec![lock_entry("shared", &tree_hash, &shared_store)],
        )?;

        write_lockfile(data, "alpha", Vec::new())?;
        let first = prune_orphaned_stores(Some(&data.to_string_lossy()))?;
        assert!(first.removed_paths.is_empty());
        assert!(shared_store.exists());

        write_lockfile(data, "beta", Vec::new())?;
        let second = prune_orphaned_stores(Some(&data.to_string_lossy()))?;
        assert!(second.removed_paths.contains(&shared_store));
        assert!(!shared_store.exists());
        Ok(())
    }

    #[test]
    fn last_reference_removed_deletes_store_path() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let data = tmp.path();
        let tree_hash = hash('d');
        let only_store = store_path(data, &tree_hash);
        fs::create_dir_all(&only_store)?;
        fs::write(only_store.join("manifest.yaml"), "id: only\n")?;
        write_lockfile(
            data,
            "default",
            vec![lock_entry("only", &tree_hash, &only_store)],
        )?;

        write_lockfile(data, "default", Vec::new())?;
        let report = prune_orphaned_stores(Some(&data.to_string_lossy()))?;

        assert!(report.removed_paths.contains(&only_store));
        assert!(!only_store.exists());
        Ok(())
    }

    #[test]
    fn malformed_lockfile_and_store_paths_are_not_deleted_via_lockfile_trust() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let data = tmp.path();
        let malformed_store = data.join("store/not-a-valid-hash");
        let valid_orphan = store_path(data, &hash('a'));
        let outside = tmp.path().join("outside-target");
        fs::create_dir_all(&malformed_store)?;
        fs::create_dir_all(&valid_orphan)?;
        fs::create_dir_all(&outside)?;
        write_lockfile(
            data,
            "default",
            vec![lock_entry("bad", "sha256:not-valid", &outside)],
        )?;

        let report = prune_orphaned_stores(Some(&data.to_string_lossy()))?;

        assert!(report.removed_paths.contains(&valid_orphan));
        assert!(!valid_orphan.exists());
        assert!(malformed_store.exists());
        assert!(outside.exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symlink_candidate_to_outside_store_is_not_deleted() -> Result<()> {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir()?;
        let data = tmp.path();
        let tree_hash = hash('b');
        let link = store_path(data, &tree_hash);
        let outside = tmp.path().join("outside-target");
        fs::create_dir_all(data.join("store"))?;
        fs::create_dir_all(&outside)?;
        symlink(&outside, &link)?;

        let report = prune_orphaned_stores(Some(&data.to_string_lossy()))?;

        assert!(report.removed_paths.is_empty());
        assert!(link.exists());
        assert!(outside.exists());
        Ok(())
    }
}
