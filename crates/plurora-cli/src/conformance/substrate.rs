use std::sync::Arc;

use plurora_runtime::{
    FilesystemObjectStore, OpenSessionRequest, Runtime, RuntimeConfig, SqliteEventStore,
};
use serde_json::json;

pub(crate) async fn sqlite_rehydrate() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("events.db");
    let object_root = directory.path().join("objects");
    let store = Arc::new(SqliteEventStore::open(&path)?);
    let mut config = RuntimeConfig::default();
    config.object_store = Arc::new(FilesystemObjectStore::new(&object_root));
    let runtime = Runtime::new(store.clone(), config);
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    let asset = runtime
        .put_asset(plurora_runtime::runtime::AssetPutRequest {
            origin_package_id: None,
            mime: "text/plain".to_string(),
            content: "durable".to_string(),
            metadata: json!({"phase": "A"}),
        })
        .await?;
    let branch = runtime
        .fork_session(session.id.clone(), 0, json!({"durable": true}))
        .await?;
    runtime
        .projection_register(plurora_runtime::runtime::ProjectionDefinition {
            id: "example/durable/projection".to_string(),
            session_id: session.id.clone(),
            source_kind_prefix: Some("context/".to_string()),
            state: json!({}),
        })
        .await?;
    runtime
        .projection_rebuild("example/durable/projection")
        .await?;
    drop(runtime);
    drop(store);

    let reopened = Arc::new(SqliteEventStore::open(&path)?);
    let mut reopened_config = RuntimeConfig::default();
    reopened_config.object_store = Arc::new(FilesystemObjectStore::new(&object_root));
    let hydrated = Runtime::new(reopened, reopened_config);
    hydrated.hydrate_substrate_from_events().await?;
    let hydrated_asset = hydrated.get_asset(&asset.id).await?;
    anyhow::ensure!(
        hydrated_asset.content == "durable",
        "asset did not rehydrate"
    );
    anyhow::ensure!(
        hydrated_asset
            .record
            .descriptor
            .as_ref()
            .map(|descriptor| descriptor.digest.as_str())
            == Some(asset.hash.as_str()),
        "rehydrated asset descriptor changed"
    );
    anyhow::ensure!(
        hydrated
            .list_branches(&session.id)
            .await
            .iter()
            .any(|item| item.id == branch.id),
        "branch did not rehydrate"
    );
    let projection = hydrated
        .projection_get("example/durable/projection")
        .await?;
    anyhow::ensure!(
        projection.state["event_count"].as_u64().unwrap_or(0) >= 1,
        "projection did not rehydrate"
    );
    Ok(())
}
