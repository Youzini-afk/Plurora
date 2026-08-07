use std::path::{Path, PathBuf};
use std::sync::Arc;

use plurora_core::WorldBundleArchive;
use plurora_runtime::{
    audit_world_bundle_archive, replay_world_bundle_archive, verify_world_bundle_archive,
    FilesystemObjectStore, Runtime, RuntimeConfig, SqliteEventStore, WorldBundleAuditReport,
    WorldBundleReplayResult,
};

const MAX_WORLD_BUNDLE_ARCHIVE_BYTES: u64 = 1024 * 1024 * 1024;

pub(crate) async fn verify(path: PathBuf, json: bool) -> anyhow::Result<()> {
    let archive = read_archive(&path).await?;
    verify_world_bundle_archive(&archive)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "valid": true,
                "bundle_digest": archive.bundle_descriptor.digest,
                "world_id": archive.manifest.world_id,
                "head_digest": archive.manifest.world_head.digest,
            }))?
        );
    } else {
        println!(
            "World Bundle verified: {} ({})",
            archive.manifest.world_id, archive.bundle_descriptor.digest
        );
    }
    Ok(())
}

pub(crate) async fn audit(path: PathBuf, json: bool) -> anyhow::Result<()> {
    let report = audit_file(&path).await?;
    emit(&report, json)
}

pub(crate) async fn replay(path: PathBuf, json: bool) -> anyhow::Result<()> {
    let result = replay_file(&path).await?;
    emit(&result, json)
}

pub(crate) async fn import(path: PathBuf, data_dir: PathBuf, json: bool) -> anyhow::Result<()> {
    let archive = read_archive(&path).await?;
    prepare_fresh_data_dir(&data_dir).await?;
    let lock_path = data_dir.join(".world-bundle-import.lock");
    let lock_file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .await
        .map_err(|error| {
            anyhow::anyhow!(
                "failed to acquire World Bundle import lock '{}': {error}",
                lock_path.display()
            )
        })?;
    let import_result = async {
        let store = Arc::new(SqliteEventStore::open(
            data_dir.join("world-bundle-events.sqlite3"),
        )?);
        let mut config = RuntimeConfig::default();
        config.object_store = Arc::new(FilesystemObjectStore::new(data_dir.join("objects")));
        let runtime = Runtime::new(store, config);
        runtime.import_world_bundle(&archive).await
    }
    .await;
    drop(lock_file);
    let _ = tokio::fs::remove_file(&lock_path).await;
    let result = import_result?;
    emit(&result, json)
}

pub(crate) async fn audit_file(path: &Path) -> anyhow::Result<WorldBundleAuditReport> {
    let archive = read_archive(path).await?;
    audit_world_bundle_archive(&archive)
}

pub(crate) async fn replay_file(path: &Path) -> anyhow::Result<WorldBundleReplayResult> {
    let archive = read_archive(path).await?;
    replay_world_bundle_archive(&archive)
}

pub(crate) async fn read_archive(path: &Path) -> anyhow::Result<WorldBundleArchive> {
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|error| anyhow::anyhow!("failed to inspect '{}': {error}", path.display()))?;
    anyhow::ensure!(metadata.is_file(), "'{}' is not a file", path.display());
    anyhow::ensure!(
        metadata.len() <= MAX_WORLD_BUNDLE_ARCHIVE_BYTES,
        "World Bundle archive exceeds the CLI size limit"
    );
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|error| anyhow::anyhow!("failed to read '{}': {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| anyhow::anyhow!("invalid World Bundle '{}': {error}", path.display()))
}

async fn prepare_fresh_data_dir(data_dir: &Path) -> anyhow::Result<()> {
    match tokio::fs::metadata(data_dir).await {
        Ok(metadata) => {
            anyhow::ensure!(
                metadata.is_dir(),
                "World Bundle data path '{}' is not a directory",
                data_dir.display()
            );
            let mut entries = tokio::fs::read_dir(data_dir).await?;
            anyhow::ensure!(
                entries.next_entry().await?.is_none(),
                "World Bundle import data directory '{}' must be empty",
                data_dir.display()
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            tokio::fs::create_dir_all(data_dir).await?;
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn emit<T: serde::Serialize>(value: &T, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        let value = serde_json::to_value(value)?;
        for key in [
            "world_id",
            "bundle_digest",
            "head_digest",
            "object_count",
            "event_count",
            "effect_receipt_count",
            "objects_imported",
            "events_imported",
        ] {
            if let Some(item) = value.get(key) {
                println!("{key}: {}", display_value(item));
            }
        }
    }
    Ok(())
}

fn display_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plurora_runtime::WorldBundleImportResult;

    #[test]
    fn import_result_is_json_serializable() -> anyhow::Result<()> {
        let value = WorldBundleImportResult {
            bundle_digest:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            world_id: "example/world".to_string(),
            head_digest: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                .to_string(),
            objects_imported: 4,
            events_imported: 2,
            sessions_imported: 1,
        };
        assert_eq!(serde_json::to_value(value)?["events_imported"], 2);
        Ok(())
    }

    #[tokio::test]
    async fn import_requires_an_empty_data_directory() -> anyhow::Result<()> {
        let temp = tempfile::TempDir::new()?;
        tokio::fs::write(temp.path().join("existing.txt"), b"occupied").await?;
        let error = prepare_fresh_data_dir(temp.path())
            .await
            .expect_err("non-empty directory must fail");
        assert!(error.to_string().contains("must be empty"));
        Ok(())
    }
}
