use std::path::PathBuf;

use plurora_runtime::{CapabilityInvocationRequest, ProtocolContext};
use serde_json::json;

use super::fixtures::*;
use crate::commands::manifest;

pub(crate) async fn foundation_packages() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    for manifest_path in [
        "packages/official/package-lab/manifest.yaml",
        "packages/official/schema-tools/manifest.yaml",
        "packages/official/event-tools/manifest.yaml",
    ] {
        runtime
            .load_package(manifest::read_manifest(PathBuf::from(manifest_path)).await?)
            .await?;
    }
    let echo = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/package-lab/echo".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({"official": "ordinary"}),
        })
        .await?;
    anyhow::ensure!(
        echo.output == json!({"official": "ordinary"}),
        "package-lab echo failed"
    );
    let schema = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/schema-tools/validate".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({"schema": {"type": "object"}, "value": {}}),
        })
        .await?;
    anyhow::ensure!(
        schema.output["valid"] == json!(true),
        "schema-tools validate failed"
    );
    let events = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/event-tools/summarize".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({"events": [{"kind": "x"}, {"kind": "y"}]}),
        })
        .await?;
    anyhow::ensure!(
        events.output["event_count"] == json!(2),
        "event-tools summarize failed"
    );
    let surfaces = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "kernel.v1.surface.contribution.list",
            json!({"slot": "forge_panel"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        surfaces.as_array().map(|items| items.len()).unwrap_or(0) >= 2,
        "official package surfaces missing"
    );
    Ok(())
}
