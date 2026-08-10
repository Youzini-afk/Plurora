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
const HOST_DATA_DIRECTORIES: &[&str] = &[
    "objects",
    "installations",
    "workspaces",
    "runtime",
    "profiles",
    "store",
    "keys",
];
const HOST_DATA_FILES: &[&str] = &["secrets.dat", "secret-store.key"];

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

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum BackupWorkspaceOwnership {
    Managed,
    LinkedLocal,
}

#[derive(Debug, Deserialize)]
struct BackupWorkspaceRecord {
    ownership: BackupWorkspaceOwnership,
}

pub(crate) async fn backup(
    data_dir: PathBuf,
    profile_path: PathBuf,
    output: PathBuf,
) -> Result<()> {
    ensure_real_directory(&data_dir).with_context(|| {
        format!(
            "Host data directory must be a real directory: {}",
            data_dir.display()
        )
    })?;
    let data_dir = data_dir
        .canonicalize()
        .with_context(|| format!("failed to resolve data directory {}", data_dir.display()))?;

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
    let configured_event_store_path = resolve_profile_path(&profile_path, configured_event_path);
    let event_store_metadata = fs::symlink_metadata(&configured_event_store_path)
        .context("failed to inspect the profile SQLite event store")?;
    reject_link_like(
        &event_store_metadata,
        "profile SQLite event store cannot be a link or reparse point",
    )?;
    anyhow::ensure!(
        event_store_metadata.is_file(),
        "profile SQLite event store is not a regular file"
    );
    let event_store_path = configured_event_store_path
        .canonicalize()
        .context("failed to resolve the profile SQLite event store")?;
    let event_store_relative = portable_relative_path(&data_dir, &event_store_path)
        .context("profile SQLite event store must be inside the data directory")?;
    anyhow::ensure!(
        profile_relative != event_store_relative,
        "Host profile and SQLite event store must be different files"
    );

    let requested_output = absolute_normalized_path(&output)?;
    anyhow::ensure!(
        !requested_output.starts_with(&data_dir),
        "backup output must be outside the Host data directory"
    );
    let (output, output_parent) = new_output_path(&output)?;
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
        &profile_relative,
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
    profile_relative: &Path,
    staging: &Path,
    lease: &plurora_service::DevelopmentHostLease,
) -> Result<()> {
    let payload = staging.join(BACKUP_PAYLOAD);
    fs::create_dir(&payload)?;
    restrict_directory_permissions(&payload)?;
    lease.ensure_active()?;
    copy_data_tree(
        data_dir,
        &payload,
        event_store_path,
        event_store_relative,
        profile_relative,
    )?;
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
    let file_paths = files
        .iter()
        .map(|file| validate_relative_path(&file.path))
        .collect::<Result<Vec<_>>>()?;
    validate_workspace_source_ownership(&payload, &file_paths)?;
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
    ensure_real_directory(&backup)
        .with_context(|| format!("Host backup must be a real directory: {}", backup.display()))?;
    let backup = backup
        .canonicalize()
        .with_context(|| format!("failed to resolve backup directory {}", backup.display()))?;
    let requested_data_dir = absolute_normalized_path(&data_dir)?;
    anyhow::ensure!(
        !requested_data_dir.starts_with(&backup) && !backup.starts_with(&requested_data_dir),
        "restore data directory and Host backup must be separate trees"
    );
    let (data_dir, data_parent) = new_output_path(&data_dir)?;
    anyhow::ensure!(
        !data_dir.starts_with(&backup) && !backup.starts_with(&data_dir),
        "restore data directory and Host backup must be separate trees"
    );
    let manifest_path = regular_file_beneath(&backup, Path::new(BACKUP_MANIFEST))
        .context("failed to open backup manifest safely")?;
    let manifest: HostBackupManifest = serde_json::from_reader(
        fs::File::open(&manifest_path).context("failed to read backup manifest")?,
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
    ensure_restore_capacity(&data_parent, &manifest)?;

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
    anyhow::ensure!(
        profile_relative != event_store_relative,
        "backup profile and SQLite event store must be different files"
    );
    let manifest_paths = manifest
        .files
        .iter()
        .map(|file| validate_relative_path(&file.path))
        .collect::<Result<Vec<_>>>()?;
    anyhow::ensure!(
        manifest_paths.iter().all(|path| current_host_data_path(
            path,
            &profile_relative,
            &event_store_relative
        )),
        "backup contains a file outside the current Host data layout"
    );
    validate_workspace_source_ownership(&payload, &manifest_paths)?;
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
        let destination = staging.join(&relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        copy_file_verified(&source, &destination, file.size, &file.sha256)?;
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

fn copy_data_tree(
    source: &Path,
    destination: &Path,
    event_store: &Path,
    event_store_relative: &Path,
    profile_relative: &Path,
) -> Result<()> {
    let event_store_handle = same_file::Handle::from_path(event_store)?;
    let event_journal = PathBuf::from(format!("{}-journal", event_store.display()));
    let event_wal = PathBuf::from(format!("{}-wal", event_store.display()));
    let event_shm = PathBuf::from(format!("{}-shm", event_store.display()));
    let source_handle = validated_directory_handle(source, source)?;
    let mut pending = vec![(
        source.to_path_buf(),
        destination.to_path_buf(),
        source_handle,
    )];
    while let Some((current_source, current_destination, directory_handle)) = pending.pop() {
        anyhow::ensure!(
            validated_directory_handle(source, &current_source)? == directory_handle,
            "Host data directory changed during backup"
        );
        let mut entries = fs::read_dir(&current_source)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let source_path = entry.path();
            let relative = source_path.strip_prefix(source)?;
            if source_path == event_store
                || source_path == event_journal
                || source_path == event_wal
                || source_path == event_shm
            {
                continue;
            }
            let metadata = fs::symlink_metadata(&source_path)?;
            let directory_allowed = current_host_data_directory(relative, profile_relative);
            let file_allowed =
                current_host_data_path(relative, profile_relative, event_store_relative);
            if !directory_allowed && !file_allowed {
                continue;
            }
            reject_link_like(
                &metadata,
                "Host backup refuses links inside the data directory",
            )?;
            let destination_path = current_destination.join(entry.file_name());
            if metadata.is_dir() {
                anyhow::ensure!(
                    directory_allowed,
                    "Host data file path is unexpectedly a directory"
                );
                fs::create_dir(&destination_path)?;
                let handle = validated_directory_handle(source, &source_path)?;
                pending.push((source_path, destination_path, handle));
            } else if metadata.is_file() {
                anyhow::ensure!(file_allowed, "Host data directory path is not a directory");
                if same_file::Handle::from_path(&source_path)? == event_store_handle {
                    continue;
                }
                copy_file_stable(&source_path, &destination_path, &metadata)?;
            } else {
                anyhow::bail!("Host backup encountered an unsupported filesystem entry");
            }
        }
        anyhow::ensure!(
            validated_directory_handle(source, &current_source)? == directory_handle,
            "Host data directory changed during backup"
        );
    }
    Ok(())
}

fn current_host_data_directory(relative: &Path, profile_relative: &Path) -> bool {
    relative
        .components()
        .next()
        .is_some_and(|component| {
            matches!(component, Component::Normal(name) if HOST_DATA_DIRECTORIES.iter().any(|allowed| name == *allowed))
        })
        || profile_relative.starts_with(relative)
}

fn current_host_data_path(
    relative: &Path,
    profile_relative: &Path,
    event_store_relative: &Path,
) -> bool {
    if relative == profile_relative || relative == event_store_relative {
        return true;
    }
    let mut components = relative.components();
    let Some(Component::Normal(first)) = components.next() else {
        return false;
    };
    if components.next().is_some() {
        HOST_DATA_DIRECTORIES
            .iter()
            .any(|allowed| first == *allowed)
    } else {
        HOST_DATA_FILES.iter().any(|allowed| first == *allowed)
    }
}

fn validate_workspace_source_ownership(payload: &Path, files: &[PathBuf]) -> Result<()> {
    let workspace_roots = files
        .iter()
        .filter_map(|path| workspace_root_for_source_file(path))
        .collect::<HashSet<_>>();
    for workspace_root in workspace_roots {
        let record_relative = workspace_root.join("workspace.json");
        anyhow::ensure!(
            files.iter().any(|path| path == &record_relative),
            "Workspace source is missing its ownership record"
        );
        let record_path = regular_file_beneath(payload, &record_relative)
            .context("Workspace ownership record is missing or unsafe")?;
        let record: BackupWorkspaceRecord = serde_json::from_reader(fs::File::open(record_path)?)
            .context("Workspace ownership record is malformed")?;
        anyhow::ensure!(
            record.ownership == BackupWorkspaceOwnership::Managed,
            "linked-local Workspace source cannot be stored in a Host backup"
        );
    }
    Ok(())
}

fn workspace_root_for_source_file(path: &Path) -> Option<PathBuf> {
    let mut components = path.components();
    let Component::Normal(workspaces) = components.next()? else {
        return None;
    };
    if workspaces != "workspaces" {
        return None;
    }
    let Component::Normal(workspace_id) = components.next()? else {
        return None;
    };
    let Component::Normal(source) = components.next()? else {
        return None;
    };
    if source != "source" {
        return None;
    }
    Some(PathBuf::from(workspaces).join(workspace_id))
}

fn validated_directory_handle(root: &Path, directory: &Path) -> Result<same_file::Handle> {
    let metadata = fs::symlink_metadata(directory)?;
    reject_link_like(&metadata, "Host backup refuses linked directories")?;
    anyhow::ensure!(metadata.is_dir(), "Host backup path is not a directory");
    let canonical = fs::canonicalize(directory)?;
    anyhow::ensure!(
        canonical.starts_with(root),
        "Host backup directory escaped the data root"
    );
    Ok(same_file::Handle::from_path(directory)?)
}

fn copy_file_stable(source: &Path, destination: &Path, inspected: &fs::Metadata) -> Result<()> {
    reject_link_like(inspected, "Host backup refuses linked files")?;
    anyhow::ensure!(
        inspected.is_file(),
        "Host backup source is not a regular file"
    );
    let input = fs::File::open(source)?;
    let opened_handle = same_file::Handle::from_file(input.try_clone()?)?;
    let opened = input.metadata()?;
    anyhow::ensure!(
        same_file_identity(inspected, &opened)
            && same_file::Handle::from_path(source)? == opened_handle,
        "Host backup source changed while it was being opened"
    );
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let copied = std::io::copy(&mut input.take(opened.len().saturating_add(1)), &mut output)?;
    anyhow::ensure!(
        copied == opened.len(),
        "Host backup source size changed during copy"
    );
    output.flush()?;
    output.sync_all()?;
    fs::set_permissions(destination, inspected.permissions())?;
    let after = fs::symlink_metadata(source)?;
    reject_link_like(&after, "Host backup source became a link during copy")?;
    anyhow::ensure!(
        same_file_identity(inspected, &after)
            && same_file::Handle::from_path(source)? == opened_handle
            && after.len() == copied,
        "Host backup source changed during copy"
    );
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
                !metadata.file_type().is_symlink() && !is_reparse_point(&metadata),
                "backup payload contains a link or reparse point"
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

fn copy_file_verified(
    source: &Path,
    destination: &Path,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<()> {
    let inspected = fs::symlink_metadata(source)?;
    reject_link_like(&inspected, "backup file is linked or redirected")?;
    anyhow::ensure!(
        inspected.is_file() && inspected.len() == expected_size,
        "backup file size mismatch"
    );
    let mut input = fs::File::open(source)?;
    let opened_handle = same_file::Handle::from_file(input.try_clone()?)?;
    let opened = input.metadata()?;
    anyhow::ensure!(
        opened.is_file()
            && opened.len() == expected_size
            && same_file_identity(&inspected, &opened)
            && same_file::Handle::from_path(source)? == opened_handle,
        "backup file changed while it was being opened"
    );

    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let mut hasher = Sha256::new();
    let mut copied = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let remaining = expected_size
            .checked_sub(copied)
            .ok_or_else(|| anyhow::anyhow!("backup file exceeded its declared size"))?;
        let limit = remaining.saturating_add(1).min(buffer.len() as u64) as usize;
        let read = input.read(&mut buffer[..limit])?;
        if read == 0 {
            break;
        }
        copied = copied
            .checked_add(read as u64)
            .ok_or_else(|| anyhow::anyhow!("backup file size overflow"))?;
        anyhow::ensure!(
            copied <= expected_size,
            "backup file exceeded its declared size"
        );
        hasher.update(&buffer[..read]);
        output.write_all(&buffer[..read])?;
    }
    anyhow::ensure!(copied == expected_size, "backup file size mismatch");
    anyhow::ensure!(
        format!("{:x}", hasher.finalize()) == expected_sha256,
        "backup checksum mismatch"
    );
    output.flush()?;
    output.sync_all()?;
    fs::set_permissions(destination, inspected.permissions())?;

    let after = fs::symlink_metadata(source)?;
    reject_link_like(&after, "backup file became linked or redirected")?;
    anyhow::ensure!(
        same_file_identity(&inspected, &after)
            && same_file::Handle::from_path(source)? == opened_handle
            && after.len() == copied,
        "backup file changed during restore"
    );
    Ok(())
}

fn ensure_restore_capacity(parent: &Path, manifest: &HostBackupManifest) -> Result<()> {
    let required = manifest.files.iter().try_fold(0_u64, |total, file| {
        total
            .checked_add(file.size)
            .ok_or_else(|| anyhow::anyhow!("backup declared size overflow"))
    })?;
    let available = fs2::available_space(parent)
        .context("failed to query available storage for Host restore")?;
    anyhow::ensure!(
        required <= available,
        "Host backup requires more storage than is available at the restore destination"
    );
    Ok(())
}

fn ensure_real_directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    reject_link_like(&metadata, "path is a link or reparse point")?;
    anyhow::ensure!(metadata.is_dir(), "path is not a directory");
    Ok(())
}

fn reject_link_like(metadata: &fs::Metadata, message: &str) -> Result<()> {
    anyhow::ensure!(
        !metadata.file_type().is_symlink() && !is_reparse_point(metadata),
        "{message}"
    );
    Ok(())
}

#[cfg(unix)]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.created().ok() == right.created().ok()
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn canonicalize_from_current_dir(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let metadata = fs::symlink_metadata(&absolute)?;
    reject_link_like(&metadata, "Host profile cannot be a link or reparse point")?;
    anyhow::ensure!(metadata.is_file(), "Host profile is not a regular file");
    Ok(absolute.canonicalize()?)
}

fn absolute_normalized_path(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(normalize_path(&absolute))
}

fn new_output_path(path: &Path) -> Result<(PathBuf, PathBuf)> {
    match fs::symlink_metadata(path) {
        Ok(_) => anyhow::bail!("output path already exists"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
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
        !metadata.file_type().is_symlink() && !is_reparse_point(&metadata),
        "links and reparse points are not portable"
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
        !metadata.file_type().is_symlink() && !is_reparse_point(&metadata) && metadata.is_dir(),
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
            !metadata.file_type().is_symlink() && !is_reparse_point(&metadata),
            "backup path traverses a link or reparse point"
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
        let safe_to_remove = fs::symlink_metadata(staging).is_ok_and(|metadata| {
            metadata.is_dir() && !metadata.file_type().is_symlink() && !is_reparse_point(&metadata)
        });
        if !safe_to_remove {
            eprintln!(
                "warning: incomplete Host backup staging is not a real directory: {}",
                staging.display()
            );
            return;
        }
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
    fs::create_dir_all(data.join("objects/sha256"))?;
    fs::create_dir_all(data.join("installations/installation-1/state"))?;
    fs::create_dir_all(data.join("workspaces/managed-1/source"))?;
    fs::create_dir_all(data.join("workspaces/linked-1"))?;
    fs::create_dir_all(data.join("runtime"))?;
    fs::create_dir_all(data.join("store/package-1"))?;
    fs::create_dir_all(data.join("keys"))?;
    fs::create_dir_all(data.join("cache"))?;
    fs::create_dir_all(data.join("unowned-layout"))?;
    let linked_source = root.join("linked-source");
    fs::create_dir(&linked_source)?;
    fs::write(linked_source.join("owned-by-user.txt"), "keep")?;
    fs::write(
        data.join("profiles/host.yaml"),
        "event_store:\n  kind: sqlite\n  path: ../runtime/installations.sqlite3\n",
    )?;
    fs::write(data.join("objects/sha256/work"), "immutable-work")?;
    fs::write(
        data.join("installations/installation-1/installation.json"),
        "{\"installation_id\":\"installation-1\"}\n",
    )?;
    fs::write(
        data.join("installations/installation-1/assembly.lock.json"),
        "{}\n",
    )?;
    fs::write(
        data.join("installations/installation-1/state/save.bin"),
        "durable-state",
    )?;
    fs::write(
        data.join("workspaces/managed-1/workspace.json"),
        "{\"ownership\":\"managed\"}\n",
    )?;
    fs::write(
        data.join("workspaces/managed-1/source/main.txt"),
        "managed-source",
    )?;
    fs::write(
        data.join("workspaces/linked-1/workspace.json"),
        format!(
            "{{\"ownership\":\"linked_local\",\"source_locator\":{}}}\n",
            serde_json::to_string(&linked_source.to_string_lossy())?
        ),
    )?;
    fs::write(
        data.join("store/package-1/manifest.yaml"),
        "id: test/item\n",
    )?;
    fs::write(data.join("keys/trusted.asc"), "public-key")?;
    fs::write(data.join("secrets.dat"), "encrypted-secret")?;
    fs::write(data.join("secret-store.key"), "encrypted-key")?;
    fs::write(data.join("cache/transient"), "skip")?;
    fs::write(data.join("unowned-layout/retired-data"), "skip")?;

    let event_path = data.join("runtime/installations.sqlite3");
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
        assert!(!backup_path.join("data/unowned-layout").exists());
        assert!(backup_path
            .join("data/installations/installation-1/installation.json")
            .is_file());
        assert!(backup_path.join("data/objects/sha256/work").is_file());
        assert!(backup_path
            .join("data/workspaces/managed-1/source/main.txt")
            .is_file());
        assert!(!backup_path.join("data/workspaces/linked-1/source").exists());
        #[cfg(windows)]
        assert_test_directory_has_no_inherited_aces(&backup_path)?;

        let restored = temp.path().join("restored");
        restore(backup_path, restored.clone()).await?;
        #[cfg(windows)]
        assert_test_directory_has_no_inherited_aces(&restored)?;
        assert_eq!(
            fs::read_to_string(restored.join("installations/installation-1/installation.json"))?,
            "{\"installation_id\":\"installation-1\"}\n"
        );
        assert_eq!(
            fs::read_to_string(restored.join("objects/sha256/work"))?,
            "immutable-work"
        );
        assert_eq!(
            fs::read_to_string(restored.join("workspaces/managed-1/source/main.txt"))?,
            "managed-source"
        );
        assert!(restored
            .join("workspaces/linked-1/workspace.json")
            .is_file());
        assert!(!restored.join("workspaces/linked-1/source").exists());
        assert_eq!(
            fs::read_to_string(temp.path().join("linked-source/owned-by-user.txt"))?,
            "keep"
        );
        assert!(!restored.join("unowned-layout").exists());
        let restored_store =
            SqliteEventStore::open(restored.join("runtime/installations.sqlite3"))?;
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
        assert!(format!("{error:#}").contains("link or reparse point"));
        assert!(!restored.exists());
        Ok(())
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn restore_rejects_payload_intermediate_reparse_point() -> Result<()> {
        use std::os::windows::fs::symlink_dir;

        let temp = tempfile::tempdir()?;
        let backup_path = create_test_backup(temp.path()).await?;
        let profiles = backup_path.join("data/profiles");
        let external_profiles = temp.path().join("external-profiles");
        fs::rename(&profiles, &external_profiles)?;
        if symlink_dir(&external_profiles, &profiles).is_err() {
            fs::rename(&external_profiles, &profiles)?;
            return Ok(());
        }

        let restored = temp.path().join("restored");
        let error = restore(backup_path, restored.clone()).await.unwrap_err();
        assert!(format!("{error:#}").contains("link or reparse point"));
        assert!(!restored.exists());
        Ok(())
    }

    #[tokio::test]
    async fn restore_rejects_files_outside_current_host_layout() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let backup_path = create_test_backup(temp.path()).await?;
        let extra_path = backup_path.join("data/unowned-layout/entry");
        fs::create_dir_all(extra_path.parent().unwrap())?;
        fs::write(&extra_path, "not-current-host-data")?;
        let manifest_path = backup_path.join(BACKUP_MANIFEST);
        let mut manifest: HostBackupManifest =
            serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
        manifest.files.push(HostBackupFile {
            path: "unowned-layout/entry".to_string(),
            size: fs::metadata(&extra_path)?.len(),
            sha256: sha256_file(&extra_path)?,
        });
        write_test_manifest(&manifest_path, &manifest)?;

        let restored = temp.path().join("restored");
        let error = restore(backup_path, restored.clone()).await.unwrap_err();
        assert!(format!("{error:#}").contains("outside the current Host data layout"));
        assert!(!restored.exists());
        Ok(())
    }

    #[tokio::test]
    async fn restore_rejects_host_owned_source_for_linked_local_workspace() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let backup_path = create_test_backup(temp.path()).await?;
        let source_path = backup_path.join("data/workspaces/linked-1/source/copied.txt");
        fs::create_dir_all(source_path.parent().unwrap())?;
        fs::write(&source_path, "must-not-become-host-owned")?;
        let manifest_path = backup_path.join(BACKUP_MANIFEST);
        let mut manifest: HostBackupManifest =
            serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
        manifest.files.push(HostBackupFile {
            path: "workspaces/linked-1/source/copied.txt".to_string(),
            size: fs::metadata(&source_path)?.len(),
            sha256: sha256_file(&source_path)?,
        });
        write_test_manifest(&manifest_path, &manifest)?;

        let restored = temp.path().join("restored");
        let error = restore(backup_path, restored.clone()).await.unwrap_err();
        assert!(format!("{error:#}").contains("linked-local Workspace source"));
        assert!(!restored.exists());
        assert_eq!(
            fs::read_to_string(temp.path().join("linked-source/owned-by-user.txt"))?,
            "keep"
        );
        Ok(())
    }

    #[tokio::test]
    async fn restore_rejects_destination_inside_backup_tree() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let backup_path = create_test_backup(temp.path()).await?;
        let restored = backup_path.join("restored");
        let error = restore(backup_path.clone(), restored.clone())
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("must be separate trees"));
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
