use plurora_runtime::{AppendEventRequest, OpenSessionRequest};
use serde_json::json;

use super::fixtures::*;

pub(crate) async fn ordering_stable() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(hook_package(
            "example/hook-b",
            "journal/before_append",
            "observe",
            0,
        ))
        .await?;
    runtime
        .load_package(hook_package(
            "example/hook-a",
            "journal/before_append",
            "observe",
            0,
        ))
        .await?;
    let result = runtime
        .dispatch_extension("journal/before_append", json!({}))
        .await;
    let invoked: Vec<_> = result
        .invoked
        .iter()
        .map(|hook| hook.subscriber_package_id.as_str())
        .collect();
    anyhow::ensure!(
        invoked == vec!["example/hook-a", "example/hook-b"],
        "hook order not stable: {invoked:?}"
    );
    Ok(())
}

pub(crate) async fn veto_blocks_event_append() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    runtime
        .load_package(event_package("example/writer", true, true))
        .await?;
    runtime
        .load_package(hook_package(
            "example/veto",
            "journal/before_append",
            "veto",
            0,
        ))
        .await?;
    let denied = runtime
        .append_event(AppendEventRequest {
            session_id: session.id,
            writer_package_id: "example/writer".to_string(),
            kind: "example/writer/event".to_string(),
            payload: json!({}),
            metadata: json!({}),
        })
        .await;
    anyhow::ensure!(denied.is_err(), "veto hook did not block append");
    Ok(())
}

pub(crate) async fn metadata_mutation_allowed() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    runtime
        .load_package(event_package("example/writer", true, true))
        .await?;
    runtime
        .load_package(hook_package(
            "example/tracer",
            "journal/before_append",
            "metadata_trace",
            0,
        ))
        .await?;
    let event = runtime
        .append_event(AppendEventRequest {
            session_id: session.id,
            writer_package_id: "example/writer".to_string(),
            kind: "example/writer/event".to_string(),
            payload: json!({}),
            metadata: json!({}),
        })
        .await?;
    anyhow::ensure!(
        event.metadata["hook_trace"] == "example/tracer",
        "metadata trace missing"
    );
    Ok(())
}

pub(crate) async fn package_owned_handler() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    runtime
        .load_package(event_package("example/writer", true, true))
        .await?;
    runtime
        .load_package(hook_handler_package(
            "example/hook-owner",
            "journal/before_append",
            "example/hook-owner/trace",
        ))
        .await?;
    let event = runtime
        .append_event(AppendEventRequest {
            session_id: session.id,
            writer_package_id: "example/writer".to_string(),
            kind: "example/writer/event".to_string(),
            payload: json!({}),
            metadata: json!({}),
        })
        .await?;
    anyhow::ensure!(
        event.metadata.get("hook_trace") == Some(&json!("example/hook-owner")),
        "package-owned hook handler did not patch metadata"
    );
    Ok(())
}

pub(crate) async fn unload_removes_subscription() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    runtime
        .load_package(event_package("example/writer", true, true))
        .await?;
    runtime
        .load_package(hook_package(
            "example/veto",
            "journal/before_append",
            "veto",
            0,
        ))
        .await?;
    runtime.unload_package(&"example/veto".to_string()).await?;
    runtime
        .append_event(AppendEventRequest {
            session_id: session.id,
            writer_package_id: "example/writer".to_string(),
            kind: "example/writer/event".to_string(),
            payload: json!({}),
            metadata: json!({}),
        })
        .await?;
    Ok(())
}
