use std::path::PathBuf;

use plurora_runtime::{CapabilityInvocationRequest, ProtocolContext};
use serde_json::json;

use super::fixtures::*;
use crate::commands::manifest;

pub(crate) async fn foundation_packages() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    for manifest_path in [
        "packages/plurora/package-lab/manifest.yaml",
        "packages/plurora/schema-tools/manifest.yaml",
        "packages/plurora/event-tools/manifest.yaml",
    ] {
        runtime
            .load_package(manifest::read_manifest(PathBuf::from(manifest_path)).await?)
            .await?;
    }
    let echo = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/package-lab/echo".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({"publisher": "ordinary"}),
        })
        .await?;
    anyhow::ensure!(
        echo.output == json!({"publisher": "ordinary"}),
        "package-lab echo failed"
    );
    let schema = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/schema-tools/validate".to_string()),
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
            capability_id: Some("plurora/event-tools/summarize".to_string()),
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
            "shell.contribution.list",
            json!({"slot": "forge_panel"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        surfaces.as_array().map(|items| items.len()).unwrap_or(0) >= 2,
        "first-party Package surfaces missing"
    );
    Ok(())
}
