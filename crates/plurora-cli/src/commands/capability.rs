use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use plurora_runtime::{CapabilityInvocationRequest, InMemoryEventStore, Runtime, RuntimeConfig};

use super::manifest::read_manifest;

pub(crate) async fn capability_invoke(
    manifest_path: PathBuf,
    capability_id: String,
    input: String,
) -> Result<()> {
    let manifest = read_manifest(manifest_path).await?;
    let payload: serde_json::Value = serde_json::from_str(&input)?;
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store, RuntimeConfig::default());
    runtime.load_package(manifest).await?;
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(capability_id),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: payload,
        })
        .await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
