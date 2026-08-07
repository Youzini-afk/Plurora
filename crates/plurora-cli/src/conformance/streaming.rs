use plurora_core::{
    CapabilityDescriptor, EntryDescriptor, PackageContributions, PackageEntry, PackageManifest,
    PermissionSet, RedactionState, SandboxPolicy, StreamFrameType, StreamInvocationState,
    EVENT_STREAM_CANCELLED, EVENT_STREAM_CHUNK, EVENT_STREAM_ENDED, EVENT_STREAM_ERROR,
    EVENT_STREAM_STARTED, EVENT_STREAM_TIMEOUT,
};
use plurora_runtime::{EventStore, OpenSessionRequest, ProtocolContext};
use serde_json::json;
use serde_json::Value;

use super::fixtures::*;

/// Helper: create a streaming echo package manifest.
fn streaming_echo_package(id: &str, capability_id: &str, streaming: bool) -> PackageManifest {
    PackageManifest {
        schema_version: 1,
        id: id.to_string(),
        version: "0.1.0".to_string(),
        display_name: None,
        description: None,
        author: None,
        license: None,
        entry: EntryDescriptor::v1(PackageEntry::RustInproc {
            crate_ref: "example-echo-rust-inproc".to_string(),
            symbol: "register".to_string(),
            abi_version: 1,
        }),
        provides: vec![CapabilityDescriptor {
            id: capability_id.to_string(),
            version: "0.1.0".to_string(),
            input_schema: Value::Null,
            output_schema: Value::Null,
            streaming,
            side_effects: Vec::new(),
            description: None,
        }],
        consumes: Vec::new(),
        requires: Vec::new(),
        contributes: PackageContributions::default(),
        permissions: PermissionSet::default(),
        sandbox_policy: SandboxPolicy::default(),
    }
}

/// Test: streaming normal lifecycle emits ordered frames/events.
pub(crate) async fn stream_normal_lifecycle() -> anyhow::Result<()> {
    let (store, runtime) = runtime();
    runtime
        .load_package(streaming_echo_package(
            "example/stream",
            "example/stream/echo",
            true,
        ))
        .await?;
    let session = runtime.open_session(OpenSessionRequest::default()).await?;

    // Start streaming
    let (start_frame, record) = runtime
        .stream_capability_start(
            &session.id,
            &"example/stream/echo".to_string(),
            None,
            None,
            json!({}),
        )
        .await?;
    assert_eq!(start_frame.frame_type, StreamFrameType::Start);
    assert_eq!(record.state, StreamInvocationState::Active);

    // Append chunks
    let chunk1 = runtime
        .stream_capability_chunk(
            &session.id,
            &record.invocation_id,
            json!({"n": 1}),
            RedactionState::NotCaptured,
        )
        .await?;
    assert_eq!(chunk1.frame_type, StreamFrameType::Chunk);
    assert_eq!(chunk1.sequence, 1);

    let chunk2 = runtime
        .stream_capability_chunk(
            &session.id,
            &record.invocation_id,
            json!({"n": 2}),
            RedactionState::NotCaptured,
        )
        .await?;
    assert_eq!(chunk2.sequence, 2);

    // End
    let end = runtime
        .stream_capability_end(&session.id, &record.invocation_id)
        .await?;
    assert_eq!(end.frame_type, StreamFrameType::End);

    // Verify events emitted
    let events = store.list_session(&session.id).await?;
    let started = events.iter().find(|e| e.kind == EVENT_STREAM_STARTED);
    let chunk_events: Vec<_> = events
        .iter()
        .filter(|e| e.kind == EVENT_STREAM_CHUNK)
        .collect();
    let ended = events.iter().find(|e| e.kind == EVENT_STREAM_ENDED);
    assert!(started.is_some(), "missing capability/stream.started");
    assert_eq!(
        chunk_events.len(),
        2,
        "expected 2 capability/stream.chunk events"
    );
    assert!(ended.is_some(), "missing capability/stream.ended");

    Ok(())
}

/// Test: cancel marks invocation cancelled and blocks further chunks.
pub(crate) async fn stream_cancel_blocks_chunks() -> anyhow::Result<()> {
    let (store, runtime) = runtime();
    runtime
        .load_package(streaming_echo_package(
            "example/stream",
            "example/stream/echo",
            true,
        ))
        .await?;
    let session = runtime.open_session(OpenSessionRequest::default()).await?;

    let (_, record) = runtime
        .stream_capability_start(
            &session.id,
            &"example/stream/echo".to_string(),
            None,
            None,
            json!({}),
        )
        .await?;

    // Cancel
    let cancel = runtime
        .stream_capability_cancel(&session.id, &record.invocation_id)
        .await?;
    assert_eq!(cancel.frame_type, StreamFrameType::Cancelled);

    // Chunk after cancel should fail
    let result = runtime
        .stream_capability_chunk(
            &session.id,
            &record.invocation_id,
            json!({}),
            RedactionState::NotCaptured,
        )
        .await;
    assert!(result.is_err());

    // Verify event
    let events = store.list_session(&session.id).await?;
    let cancelled = events.iter().find(|e| e.kind == EVENT_STREAM_CANCELLED);
    assert!(cancelled.is_some(), "missing capability/stream.cancelled");

    Ok(())
}

/// Test: timeout marks invocation timeout and blocks further chunks.
pub(crate) async fn stream_timeout_blocks_chunks() -> anyhow::Result<()> {
    let (store, runtime) = runtime();
    runtime
        .load_package(streaming_echo_package(
            "example/stream",
            "example/stream/echo",
            true,
        ))
        .await?;
    let session = runtime.open_session(OpenSessionRequest::default()).await?;

    let (_, record) = runtime
        .stream_capability_start(
            &session.id,
            &"example/stream/echo".to_string(),
            None,
            None,
            json!({}),
        )
        .await?;

    // Timeout
    let timeout = runtime
        .stream_capability_timeout(&session.id, &record.invocation_id)
        .await?;
    assert_eq!(timeout.frame_type, StreamFrameType::Timeout);

    // Chunk after timeout should fail
    let result = runtime
        .stream_capability_chunk(
            &session.id,
            &record.invocation_id,
            json!({}),
            RedactionState::NotCaptured,
        )
        .await;
    assert!(result.is_err());

    // Verify event
    let events = store.list_session(&session.id).await?;
    let timeout_evt = events.iter().find(|e| e.kind == EVENT_STREAM_TIMEOUT);
    assert!(timeout_evt.is_some(), "missing capability/stream.timeout");

    Ok(())
}

/// Test: error terminal frame works.
pub(crate) async fn stream_error_terminal() -> anyhow::Result<()> {
    let (store, runtime) = runtime();
    runtime
        .load_package(streaming_echo_package(
            "example/stream",
            "example/stream/echo",
            true,
        ))
        .await?;
    let session = runtime.open_session(OpenSessionRequest::default()).await?;

    let (_, record) = runtime
        .stream_capability_start(
            &session.id,
            &"example/stream/echo".to_string(),
            None,
            None,
            json!({}),
        )
        .await?;

    // Error
    let error = runtime
        .stream_capability_error(&session.id, &record.invocation_id, "test error")
        .await?;
    assert_eq!(error.frame_type, StreamFrameType::Error);

    // Chunk after error should fail
    let result = runtime
        .stream_capability_chunk(
            &session.id,
            &record.invocation_id,
            json!({}),
            RedactionState::NotCaptured,
        )
        .await;
    assert!(result.is_err());

    // Verify event
    let events = store.list_session(&session.id).await?;
    let error_evt = events.iter().find(|e| e.kind == EVENT_STREAM_ERROR);
    assert!(error_evt.is_some(), "missing capability/stream.error");

    Ok(())
}

/// Test: non-streaming capability cannot be streamed.
pub(crate) async fn stream_non_streaming_rejected() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    // Load a non-streaming capability (streaming=false)
    runtime
        .load_package(streaming_echo_package(
            "example/nonstream",
            "example/nonstream/echo",
            false,
        ))
        .await?;
    let session = runtime.open_session(OpenSessionRequest::default()).await?;

    let result = runtime
        .stream_capability_start(
            &session.id,
            &"example/nonstream/echo".to_string(),
            None,
            None,
            json!({}),
        )
        .await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("not a streaming capability"),
        "expected streaming=false rejection, got: {}",
        err_msg
    );

    Ok(())
}

/// Test: no model/agent methods added to protocol.
pub(crate) async fn stream_no_model_agent_methods() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let value = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.info",
            json!({}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let methods = value
        .get("methods")
        .and_then(|m| m.as_array())
        .ok_or_else(|| anyhow::anyhow!("host.info missing methods"))?;
    for method in methods {
        let id = method.get("id").and_then(|v| v.as_str()).unwrap_or("");
        anyhow::ensure!(
            !id.contains("model"),
            "found model method in protocol: {}",
            id
        );
        anyhow::ensure!(
            !id.contains("agent"),
            "found agent method in protocol: {}",
            id
        );
    }
    Ok(())
}

/// Test: capability.stream and capability.cancel are dispatchable through protocol.
pub(crate) async fn stream_protocol_dispatch() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(streaming_echo_package(
            "example/stream",
            "example/stream/echo",
            true,
        ))
        .await?;
    let session = runtime.open_session(OpenSessionRequest::default()).await?;

    // Stream via protocol
    let value = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "capability.stream",
            json!({
                "capability_id": "example/stream/echo",
                "session_id": session.id,
            }),
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.message))?;

    let invocation_id = value
        .get("invocation")
        .and_then(|v| v.get("invocation_id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("stream response missing invocation.invocation_id"))?;
    let frame_type = value
        .get("frame")
        .and_then(|v| v.get("frame_type"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("stream response missing frame.frame_type"))?;
    assert_eq!(frame_type, "start");

    // Cancel via protocol
    let cancel_value = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "capability.cancel",
            json!({
                "invocation_id": invocation_id,
                "session_id": session.id,
            }),
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.message))?;

    let cancel_frame_type = cancel_value
        .get("frame_type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("cancel response missing frame_type"))?;
    assert_eq!(cancel_frame_type, "cancelled");

    Ok(())
}
