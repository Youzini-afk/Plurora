use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Utc;
use plurora_runtime::SqliteEventStore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::host::resolve_profile_path;
use crate::cli::{HostEventStoreProfile, HostProfile};

const BACKUP_FORMAT_VERSION: u32 = 1;
const BACKUP_MANIFEST: &str = "manifest.json";
const BACKUP_PAYLOAD: &str = "data";

#[derive(Debug, Serialize, Deserialize)]
struct HostBackupManifest {
    format_version: u32,
    created_at_ms: i64,
    profile_path: String,
    event_store_path: String,
    files: Vec<HostBackupFile>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HostBackupFile {
    path: String,
    size: u64,
    sha256: String,
}

pub(crate) async fn backup(
    data_dir: PathBuf,
    profile_path: PathBuf,
    output: PathBuf,
) -> Result<()> {
    let data_dir = data_dir
        .canonicalize()
        .with_context(|| format!("failed to resolve data directory {}", data_dir.display()))?;
    anyhow::ensure!(data_dir.is_dir(), "Host data directory is not a directory");

    let profile_path = canonicalize_from_current_dir(&profile_path)
        .with_context(|| format!("failed to resolve Host profile {}", profile_path.display()))?;
    let profile_relative = portable_relative_path(&data_dir, &profile_path)
        .context("Host profile must be a regular file inside the data directory")?;
    let profile: HostProfile = serde_yaml::from_str(
        &fs::read_to_string(&profile_path)
            .with_context(|| format!("failed to read Host profile {}", profile_path.display()))?,
    )
    .with_context(|| format!("failed to parse Host profile {}", profile_path.display()))?;
    let configured_event_path = match profile.event_store {
        HostEventStoreProfile::Sqlite { path } => path,
        HostEventStoreProfile::Memory => {
            anyhow::bail!("memory-backed Hosts have no durable event store to back up")
        }
        HostEventStoreProfile::Postgres { .. } => {
            anyhow::bail!("Postgres Host backup is not supported by this offline command")
        }
    };
    anyhow::ensure!(
        configured_event_path.is_relative(),
        "Host backup requires a relative SQLite path so restores remain portable"
    );
    let event_store_path = resolve_profile_path(&profile_path, configured_event_path)
        .canonicalize()
        .context("failed to resolve the profile SQLite event store")?;
    let event_store_relative = portable_relative_path(&data_dir, &event_store_path)
        .context("profile SQLite event store must be inside the data directory")?;
    anyhow::ensure!(
        event_store_path.is_file(),
        "profile SQLite event store is not a regular file"
    );

    let (output, output_parent) = new_output_path(&output)?;
    anyhow::ensure!(
        !output.starts_with(&data_dir),
        "backup output must be outside the Host data directory"
    );
    let staging = output_parent.join(format!(
        ".plurora-host-backup-{}-{}",
        output
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("snapshot"),
        Uuid::new_v4().simple()
    ));
    fs::create_dir(&staging).with_context(|| {
        format!(
            "failed to create backup staging directory {}",
            staging.display()
        )
    })?;
    restrict_directory_permissions(&staging)?;

    let store = Arc::new(SqliteEventStore::open(&event_store_path)?);
    let registry = plurora_service::development_registry();
    let lease = match plurora_service::acquire_development_host_lease(store.clone(), registry).await
    {
        Ok(lease) => lease,
        Err(error) => {
            cleanup_staging(&staging, &output_parent);
            return Err(error).context(
                "Host backup requires exclusive control-plane ownership; stop the running Host first",
            );
        }
    };
    let heartbeat =
        plurora_service::spawn_development_host_lease_heartbeat(store.clone(), lease.clone());

    let capture_result = capture_backup_snapshot(
        &data_dir,
        &event_store_path,
        &event_store_relative,
        &staging,
        &lease,
    )
    .await;
    let capture_result = capture_result.and_then(|()| lease.ensure_active());
    heartbeat.abort();
    let _ = heartbeat.await;
    let capture_result = capture_result.and_then(|()| lease.ensure_active());
    let release_result = plurora_service::release_owned_development_host_lease(store, &lease).await;

    if let Err(error) = capture_result {
        cleanup_staging(&staging, &output_parent);
        if let Err(release_error) = release_result {
            return Err(error).context(format!(
                "backup failed and the source Host lease could not be released: {release_error}"
            ));
        }
        return Err(error);
    }
    if let Err(error) = release_result {
        cleanup_staging(&staging, &output_parent);
        return Err(error)
            .context("backup snapshot was discarded because the Host lease did not release");
    }
    if let Err(error) =
        finalize_backup_snapshot(&profile_relative, &event_store_relative, &staging, &lease).await
    {
        cleanup_staging(&staging, &output_parent);
        return Err(error).context("failed to finalize the Host backup snapshot");
    }

    fs::rename(&staging, &output).with_context(|| {
        format!(
            "failed to publish backup {} from {}",
            output.display(),
            staging.display()
        )
    })?;
    println!("host/backup.created: {}", output.display());
    Ok(())
}

async fn capture_backup_snapshot(
    data_dir: &Path,
    event_store_path: &Path,
    event_store_relative: &Path,
    staging: &Path,
    lease: &plurora_service::DevelopmentHostLease,
) -> Result<()> {
    let payload = staging.join(BACKUP_PAYLOAD);
    fs::create_dir(&payload)?;
    restrict_directory_permissions(&payload)?;
    lease.ensure_active()?;
    copy_data_tree(data_dir, &payload, event_store_path)?;
    lease.ensure_active()?;

    let event_backup_path = payload.join(event_store_relative);
    if let Some(parent) = event_backup_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let source_store = Arc::new(SqliteEventStore::open(event_store_path)?);
    source_store.backup_to(&event_backup_path).await?;
    lease.ensure_active()?;
    Ok(())
}

async fn finalize_backup_snapshot(
    profile_relative: &Path,
    event_store_relative: &Path,
    staging: &Path,
    lease: &plurora_service::DevelopmentHostLease,
) -> Result<()> {
    let payload = staging.join(BACKUP_PAYLOAD);
    let event_backup_path = payload.join(event_store_relative);
    // The source snapshot contains the temporary exclusive lease. Append its
    // release to the snapshot before publishing so a restore is immediately usable.
    let snapshot_store = Arc::new(SqliteEventStore::open(&event_backup_path)?);
    plurora_service::release_development_host_lease(snapshot_store.clone(), lease).await?;
    snapshot_store.verify_integrity().await?;

    let files = inventory_payload(&payload)?;
    let profile_path = path_to_portable_string(profile_relative)?;
    let event_store_path = path_to_portable_string(event_store_relative)?;
    anyhow::ensure!(
        files.iter().any(|file| file.path == profile_path),
        "backup profile was not copied into the payload"
    );
    anyhow::ensure!(
        files.iter().any(|file| file.path == event_store_path),
        "backup event store was not copied into the payload"
    );
    let manifest = HostBackupManifest {
        format_version: BACKUP_FORMAT_VERSION,
        created_at_ms: Utc::now().timestamp_millis(),
        profile_path,
        event_store_path,
        files,
    };
    let manifest_path = staging.join(BACKUP_MANIFEST);
    let mut manifest_file = fs::File::create(&manifest_path)?;
    serde_json::to_writer_pretty(&mut manifest_file, &manifest)?;
    manifest_file.write_all(b"\n")?;
    manifest_file.sync_all()?;
    Ok(())
}

pub(crate) async fn restore(backup: PathBuf, data_dir: PathBuf) -> Result<()> {
    let backup = backup
        .canonicalize()
        .with_context(|| format!("failed to resolve backup directory {}", backup.display()))?;
    anyhow::ensure!(backup.is_dir(), "Host backup is not a directory");
    anyhow::ensure!(
        !data_dir.exists(),
        "restore data directory already exists; restore only targets a new path"
    );
    let (data_dir, data_parent) = new_output_path(&data_dir)?;
    let manifest: HostBackupManifest = serde_json::from_str(
        &fs::read_to_string(backup.join(BACKUP_MANIFEST))
            .context("failed to read backup manifest")?,
    )
    .context("failed to parse backup manifest")?;
    anyhow::ensure!(
        manifest.format_version == BACKUP_FORMAT_VERSION,
        "unsupported Host backup format version {}",
        manifest.format_version
    );
    validate_relative_path(&manifest.profile_path)?;
    validate_relative_path(&manifest.event_store_path)?;
    anyhow::ensure!(!manifest.files.is_empty(), "Host backup contains no files");

    let staging = data_parent.join(format!(
        ".plurora-host-restore-{}-{}",
        data_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("data"),
        Uuid::new_v4().simple()
    ));
    fs::create_dir(&staging).with_context(|| {
        format!(
            "failed to create restore staging directory {}",
            staging.display()
        )
    })?;
    restrict_directory_permissions(&staging)?;

    let result = restore_into_staging(&backup, &staging, &manifest).await;
    if let Err(error) = result {
        cleanup_staging(&staging, &data_parent);
        return Err(error);
    }
    fs::rename(&staging, &data_dir).with_context(|| {
        format!(
            "failed to publish restored data directory {}",
            data_dir.display()
        )
    })?;
    println!("host/backup.restored: {}", data_dir.display());
    Ok(())
}

async fn restore_into_staging(
    backup: &Path,
    staging: &Path,
    manifest: &HostBackupManifest,
) -> Result<()> {
    let payload = backup.join(BACKUP_PAYLOAD);
    ensure_regular_directory(&payload).context("Host backup payload is missing or unsafe")?;
    let profile_relative = validate_relative_path(&manifest.profile_path)?;
    let event_store_relative = validate_relative_path(&manifest.event_store_path)?;
    let manifest_paths = manifest
        .files
        .iter()
        .map(|file| validate_relative_path(&file.path))
        .collect::<Result<Vec<_>>>()?;
    anyhow::ensure!(
        manifest_paths.iter().any(|path| path == &profile_relative),
        "backup manifest does not list the Host profile"
    );
    anyhow::ensure!(
        manifest_paths
            .iter()
            .any(|path| path == &event_store_relative),
        "backup manifest does not list the SQLite event store"
    );

    let mut seen = HashSet::new();
    for (file, relative) in manifest.files.iter().zip(manifest_paths) {
        anyhow::ensure!(seen.insert(relative.clone()), "duplicate backup file path");
        let source = regular_file_beneath(&payload, &relative)
            .with_context(|| format!("backup file is missing or unsafe: {}", file.path))?;
        let metadata = fs::symlink_metadata(&source)?;
        anyhow::ensure!(metadata.len() == file.size, "backup file size mismatch");
        anyhow::ensure!(
            sha256_file(&source)? == file.sha256,
            "backup checksum mismatch"
        );

        let destination = staging.join(&relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&source, &destination)?;
        anyhow::ensure!(
            sha256_file(&destination)? == file.sha256,
            "restored checksum mismatch"
        );
    }

    let profile_path = regular_file_beneath(staging, &profile_relative)?;
    let profile: HostProfile = serde_yaml::from_str(&fs::read_to_string(&profile_path)?)?;
    let configured = match profile.event_store {
        HostEventStoreProfile::Sqlite { path } if path.is_relative() => path,
        _ => anyhow::bail!("backup profile is not portable SQLite configuration"),
    };
    let configured = normalize_path(&profile_path.parent().unwrap_or(staging).join(configured));
    let expected = normalize_path(&staging.join(&event_store_relative));
    anyhow::ensure!(
        configured == expected,
        "backup profile does not reference the snapshotted event store"
    );
    let expected = regular_file_beneath(staging, &event_store_relative)?;
    let store = SqliteEventStore::open(&expected)?;
    store.verify_integrity().await?;
    Ok(())
}

fn copy_data_tree(source: &Path, destination: &Path, event_store: &Path) -> Result<()> {
    let event_wal = PathBuf::from(format!("{}-wal", event_store.display()));
    let event_shm = PathBuf::from(format!("{}-shm", event_store.display()));
    let mut pending = vec![(source.to_path_buf(), destination.to_path_buf())];
    while let Some((current_source, current_destination)) = pending.pop() {
        let mut entries = fs::read_dir(&current_source)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let source_path = entry.path();
            let relative = source_path.strip_prefix(source)?;
            if relative.components().next().is_some_and(
                |component| matches!(component, Component::Normal(name) if name == "cache"),
            ) {
                continue;
            }
            if source_path == event_store || source_path == event_wal || source_path == event_shm {
                continue;
            }
            let metadata = fs::symlink_metadata(&source_path)?;
            anyhow::ensure!(
                !metadata.file_type().is_symlink(),
                "Host backup refuses symbolic links inside the data directory"
            );
            let destination_path = current_destination.join(entry.file_name());
            if metadata.is_dir() {
                fs::create_dir(&destination_path)?;
                pending.push((source_path, destination_path));
            } else if metadata.is_file() {
                fs::copy(&source_path, &destination_path)?;
            } else {
                anyhow::bail!("Host backup encountered an unsupported filesystem entry");
            }
        }
    }
    Ok(())
}

fn inventory_payload(payload: &Path) -> Result<Vec<HostBackupFile>> {
    let mut paths = Vec::new();
    let mut pending = vec![payload.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let mut entries = fs::read_dir(&directory)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            anyhow::ensure!(
                !metadata.file_type().is_symlink(),
                "backup payload contains a symlink"
            );
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                paths.push((path, metadata.len()));
            }
        }
    }
    paths.sort_by(|left, right| left.0.cmp(&right.0));
    paths
        .into_iter()
        .map(|(path, size)| {
            Ok(HostBackupFile {
                path: path_to_portable_string(path.strip_prefix(payload)?)?,
                size,
                sha256: sha256_file(&path)?,
            })
        })
        .collect()
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn canonicalize_from_current_dir(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.canonicalize()?)
    } else {
        Ok(std::env::current_dir()?.join(path).canonicalize()?)
    }
}

fn new_output_path(path: &Path) -> Result<(PathBuf, PathBuf)> {
    anyhow::ensure!(!path.exists(), "output path already exists");
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let parent = absolute
        .parent()
        .ok_or_else(|| anyhow::anyhow!("output path has no parent directory"))?;
    fs::create_dir_all(parent)?;
    let parent = parent.canonicalize()?;
    let name = absolute
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("output path has no file name"))?;
    Ok((parent.join(name), parent))
}

fn portable_relative_path(root: &Path, path: &Path) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)?;
    anyhow::ensure!(metadata.is_file(), "path is not a regular file");
    anyhow::ensure!(
        !metadata.file_type().is_symlink(),
        "symbolic links are not portable"
    );
    let relative = path.strip_prefix(root)?;
    validate_relative_path(&path_to_portable_string(relative)?)
}

fn validate_relative_path(raw: &str) -> Result<PathBuf> {
    anyhow::ensure!(!raw.is_empty(), "backup path is empty");
    let path = PathBuf::from(raw.replace('/', std::path::MAIN_SEPARATOR_STR));
    anyhow::ensure!(!path.is_absolute(), "backup path must be relative");
    anyhow::ensure!(
        path.components()
            .all(|component| matches!(component, Component::Normal(_))),
        "backup path contains an unsafe component"
    );
    Ok(path)
}

fn path_to_portable_string(path: &Path) -> Result<String> {
    let parts = path
        .components()
        .map(|component| match component {
            Component::Normal(value) => value
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| anyhow::anyhow!("backup path is not valid UTF-8")),
            _ => Err(anyhow::anyhow!("backup path contains an unsafe component")),
        })
        .collect::<Result<Vec<_>>>()?;
    anyhow::ensure!(!parts.is_empty(), "backup path is empty");
    Ok(parts.join("/"))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn ensure_regular_directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    anyhow::ensure!(
        !metadata.file_type().is_symlink() && metadata.is_dir(),
        "path is not a regular directory"
    );
    Ok(())
}

fn regular_file_beneath(root: &Path, relative: &Path) -> Result<PathBuf> {
    ensure_regular_directory(root)?;
    let mut current = root.to_path_buf();
    let mut components = relative.components().peekable();
    anyhow::ensure!(components.peek().is_some(), "backup path is empty");
    while let Some(component) = components.next() {
        let Component::Normal(component) = component else {
            anyhow::bail!("backup path contains an unsafe component");
        };
        current.push(component);
        let metadata = fs::symlink_metadata(&current)?;
        anyhow::ensure!(
            !metadata.file_type().is_symlink(),
            "backup path traverses a symbolic link"
        );
        if components.peek().is_some() {
            anyhow::ensure!(metadata.is_dir(), "backup path parent is not a directory");
        } else {
            anyhow::ensure!(metadata.is_file(), "backup entry is not a regular file");
        }
    }
    Ok(current)
}

fn cleanup_staging(staging: &Path, expected_parent: &Path) {
    if staging.parent() == Some(expected_parent)
        && staging
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(".plurora-host-"))
    {
        if let Err(error) = fs::remove_dir_all(staging) {
            eprintln!(
                "warning: failed to remove incomplete Host backup staging {}: {error}",
                staging.display()
            );
        }
    }
}

#[cfg(unix)]
fn restrict_directory_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(windows)]
fn restrict_directory_permissions(path: &Path) -> Result<()> {
    use std::process::Command;

    let whoami = Command::new("whoami")
        .args(["/user", "/fo", "csv", "/nh"])
        .output()
        .context("failed to query the current Windows user SID")?;
    anyhow::ensure!(
        whoami.status.success(),
        "whoami could not query the user SID"
    );
    let output = String::from_utf8_lossy(&whoami.stdout);
    let sid_start = output
        .find("S-")
        .ok_or_else(|| anyhow::anyhow!("whoami returned no Windows user SID"))?;
    let sid = output[sid_start..]
        .chars()
        .take_while(|character| {
            character.is_ascii_digit() || *character == '-' || *character == 'S'
        })
        .collect::<String>();
    anyhow::ensure!(
        sid.starts_with("S-1-"),
        "whoami returned an invalid user SID"
    );

    let status = Command::new("icacls")
        .arg(path)
        .arg("/inheritance:r")
        .arg("/grant:r")
        .arg(format!("*{sid}:(OI)(CI)F"))
        .arg("/Q")
        .status()
        .context("failed to restrict the Host data directory ACL")?;
    anyhow::ensure!(
        status.success(),
        "icacls could not restrict the directory ACL"
    );
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn restrict_directory_permissions(_path: &Path) -> Result<()> {
    anyhow::bail!("private Host data directories are unsupported on this platform")
}

#[cfg(test)]
async fn create_test_backup(root: &Path) -> Result<PathBuf> {
    let data = root.join("source");
    fs::create_dir_all(data.join("profiles"))?;
    fs::create_dir_all(data.join("projects/example"))?;
    fs::create_dir_all(data.join("cache"))?;
    fs::write(
        data.join("profiles/host.yaml"),
        "event_store:\n  kind: sqlite\n  path: events.sqlite3\n",
    )?;
    fs::write(data.join("projects/example/project.yaml"), "id: example\n")?;
    fs::write(data.join("cache/transient"), "skip")?;

    let event_path = data.join("profiles/events.sqlite3");
    let store = SqliteEventStore::open(&event_path)?;
    use plurora_runtime::EventStore;
    store
        .append(plurora_core::EventEnvelope::new(
            "backup-event".to_string(),
            plurora_core::SessionId::from("backup-session"),
            0,
            plurora_core::PackageId::from("test/backup"),
            "test/backup.created",
            serde_json::json!({"ok": true}),
        ))
        .await?;

    let backup_path = root.join("backup");
    backup(
        data,
        root.join("source/profiles/host.yaml"),
        backup_path.clone(),
    )
    .await?;
    Ok(backup_path)
}

#[cfg(test)]
fn write_test_manifest(path: &Path, manifest: &HostBackupManifest) -> Result<()> {
    let mut contents = serde_json::to_string_pretty(manifest)?;
    contents.push('\n');
    fs::write(path, contents)?;
    Ok(())
}

#[cfg(all(test, windows))]
fn assert_test_directory_has_no_inherited_aces(path: &Path) -> Result<()> {
    use std::process::Command;

    let output = Command::new("icacls")
        .arg(path)
        .output()
        .context("failed to inspect the test directory ACL")?;
    anyhow::ensure!(output.status.success(), "icacls could not inspect the ACL");
    let output = String::from_utf8_lossy(&output.stdout);
    anyhow::ensure!(
        !output.contains("(I)"),
        "private test directory retained inherited ACL entries"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use plurora_core::SessionId;
    use plurora_runtime::EventStore;

    #[tokio::test]
    async fn backup_and_restore_preserve_data_and_verify_sqlite() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let backup_path = create_test_backup(temp.path()).await?;
        assert!(!backup_path.join("data/cache/transient").exists());
        #[cfg(windows)]
        assert_test_directory_has_no_inherited_aces(&backup_path)?;

        let restored = temp.path().join("restored");
        restore(backup_path, restored.clone()).await?;
        #[cfg(windows)]
        assert_test_directory_has_no_inherited_aces(&restored)?;
        assert_eq!(
            fs::read_to_string(restored.join("projects/example/project.yaml"))?,
            "id: example\n"
        );
        let restored_store = SqliteEventStore::open(restored.join("profiles/events.sqlite3"))?;
        restored_store.verify_integrity().await?;
        assert_eq!(
            restored_store
                .list_session(&SessionId::from("backup-session"))
                .await?
                .len(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn restore_rejects_manifest_without_event_store_entry() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let backup_path = create_test_backup(temp.path()).await?;
        let manifest_path = backup_path.join(BACKUP_MANIFEST);
        let mut manifest: HostBackupManifest =
            serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
        manifest
            .files
            .retain(|file| file.path != manifest.event_store_path);
        write_test_manifest(&manifest_path, &manifest)?;

        let restored = temp.path().join("restored");
        let error = restore(backup_path, restored.clone()).await.unwrap_err();
        assert!(format!("{error:#}").contains("does not list the SQLite event store"));
        assert!(!restored.exists());
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn restore_rejects_payload_intermediate_symlink() -> Result<()> {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir()?;
        let backup_path = create_test_backup(temp.path()).await?;
        let profiles = backup_path.join("data/profiles");
        let external_profiles = temp.path().join("external-profiles");
        fs::rename(&profiles, &external_profiles)?;
        symlink(&external_profiles, &profiles)?;

        let restored = temp.path().join("restored");
        let error = restore(backup_path, restored.clone()).await.unwrap_err();
        assert!(format!("{error:#}").contains("symbolic link"));
        assert!(!restored.exists());
        Ok(())
    }

    #[test]
    fn backup_paths_reject_parent_components() {
        assert!(validate_relative_path("../escape").is_err());
        assert!(validate_relative_path("profiles/../escape").is_err());
        assert!(validate_relative_path("profiles/host.yaml").is_ok());
    }
}
