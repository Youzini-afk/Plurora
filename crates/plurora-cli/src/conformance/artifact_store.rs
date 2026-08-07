use std::sync::Arc;

use chrono::Utc;
use plurora_core::{new_id, AssetRecord, EventEnvelope, EVENT_ASSET_PUT, PLATFORM_RUNTIME_ID};
use plurora_runtime::{
    legacy_content_address, ArtifactCommitRequest, EventStore, FilesystemObjectStore,
    InMemoryEventStore, ObjectStore, ObjectStoreError, Runtime, RuntimeConfig,
};
use serde_json::json;
use tokio::io::AsyncReadExt;

pub(crate) async fn object_store_portability_integrity() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let source_store = Arc::new(FilesystemObjectStore::new(directory.path().join("host-a")));
    let target_store = Arc::new(FilesystemObjectStore::new(directory.path().join("host-b")));
    let bytes = b"portable artifact without package state".to_vec();

    let mut config = RuntimeConfig::default();
    config.object_store = source_store.clone();
    let runtime = Runtime::new(Arc::new(InMemoryEventStore::default()), config);
    let descriptor = runtime
        .commit_artifact(ArtifactCommitRequest {
            artifact_type_uri: "urn:vendor.example:artifact:unknown:v7".to_string(),
            media_type: "application/x-vendor-unknown".to_string(),
            bytes: bytes.clone().into(),
            references: vec!["urn:example:source:1".to_string()],
            annotations: Default::default(),
        })
        .await?;
    anyhow::ensure!(
        descriptor.digest.starts_with("sha256:") && descriptor.digest.len() == 71,
        "artifact digest is not canonical sha256"
    );
    anyhow::ensure!(
        descriptor.artifact_type_uri == "urn:vendor.example:artifact:unknown:v7",
        "unknown artifact type was not preserved"
    );

    let copied_bytes = runtime.read_artifact(&descriptor).await?;
    let target_info = target_store.put(copied_bytes).await?;
    anyhow::ensure!(
        target_info.digest == descriptor.digest,
        "identical bytes produced different digests across hosts"
    );
    anyhow::ensure!(
        source_store.has(&descriptor.digest).await?,
        "source host does not report the committed object"
    );
    anyhow::ensure!(
        matches!(
            source_store.has("fnv1a64:0000000000000000").await,
            Err(ObjectStoreError::UnsupportedDigestAlgorithm { .. })
        ),
        "legacy FNV address was accepted as canonical object identity"
    );
    let mut stream = target_store.stream(&descriptor.digest).await?;
    let mut streamed = Vec::new();
    stream.read_to_end(&mut streamed).await?;
    anyhow::ensure!(streamed == bytes, "portable object stream mismatch");

    let hex = descriptor
        .digest
        .strip_prefix("sha256:")
        .ok_or_else(|| anyhow::anyhow!("missing sha256 prefix"))?;
    let target_path = target_store.root().join("sha256").join(hex);
    tokio::fs::write(target_path, b"tampered").await?;
    anyhow::ensure!(
        matches!(
            target_store.verify(&descriptor.digest).await,
            Err(ObjectStoreError::Integrity { .. })
        ),
        "tampered object passed verification"
    );
    Ok(())
}

pub(crate) async fn asset_legacy_fnv_migration() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let object_root = directory.path().join("objects");
    let store = Arc::new(InMemoryEventStore::default());
    let content = "legacy inline asset";
    let asset_id = "ast_legacy_fixture".to_string();
    let event_id = new_id("evt");
    let legacy_hash = legacy_content_address(content);
    let record = AssetRecord {
        id: asset_id.clone(),
        origin_package_id: "example/legacy-package".to_string(),
        mime: "text/plain".to_string(),
        hash: legacy_hash.clone(),
        size_bytes: content.len() as u64,
        created_at: Utc::now(),
        metadata: json!({"legacy": true}),
        descriptor: None,
    };
    store
        .append(EventEnvelope {
            id: event_id.clone(),
            session_id: "legacy-session".to_string(),
            sequence: 0,
            writer_package_id: PLATFORM_RUNTIME_ID.to_string(),
            kind: EVENT_ASSET_PUT.to_string(),
            schema_version: 1,
            timestamp: Utc::now(),
            payload: serde_json::to_value(record)?,
            metadata: json!({"content": content}),
        })
        .await?;

    // Simulate an interrupted prior migration that committed bytes to CAS but
    // did not finish rebuilding the in-memory asset projection.
    let object_store = Arc::new(FilesystemObjectStore::new(&object_root));
    object_store.put(content.as_bytes().to_vec().into()).await?;
    let mut config = RuntimeConfig::default();
    config.object_store = object_store;
    let runtime = Runtime::new(store.clone(), config);
    let event_count = store.list_all().await?.len();
    runtime.hydrate_substrate_from_events().await?;
    let migrated = runtime.get_asset(&asset_id).await?;
    anyhow::ensure!(migrated.content == content, "legacy asset content changed");
    anyhow::ensure!(
        migrated.record.hash.starts_with("sha256:") && migrated.record.hash.len() == 71,
        "legacy asset did not migrate to sha256"
    );
    let descriptor = migrated
        .record
        .descriptor
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("migrated asset has no descriptor"))?;
    anyhow::ensure!(
        descriptor.annotations.get("legacy_asset_id") == Some(&json!(asset_id)),
        "legacy asset id was not preserved"
    );
    anyhow::ensure!(
        descriptor.annotations.get("legacy_hash") == Some(&json!(legacy_hash)),
        "legacy FNV digest was not preserved"
    );
    anyhow::ensure!(
        descriptor.annotations.get("legacy_event_id") == Some(&json!(event_id)),
        "legacy event reference was not preserved"
    );
    anyhow::ensure!(
        descriptor.annotations.get("legacy_event_sequence") == Some(&json!(0)),
        "legacy event sequence was not preserved"
    );
    anyhow::ensure!(
        descriptor.annotations.get("legacy_event_session_id") == Some(&json!("legacy-session")),
        "legacy event session was not preserved"
    );
    let migrated_digest = migrated.record.hash.clone();

    drop(runtime);
    let mut restarted_config = RuntimeConfig::default();
    restarted_config.object_store = Arc::new(FilesystemObjectStore::new(&object_root));
    let restarted = Runtime::new(store.clone(), restarted_config);
    restarted.hydrate_substrate_from_events().await?;
    let migrated_again = restarted.get_asset(&asset_id).await?;
    anyhow::ensure!(
        migrated_again.record.hash == migrated_digest,
        "legacy migration was not restart-safe and idempotent"
    );
    anyhow::ensure!(
        store.list_all().await?.len() == event_count,
        "legacy migration appended duplicate journal events"
    );
    Ok(())
}
