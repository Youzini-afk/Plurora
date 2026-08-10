use plurora_core::{
    CapHandle, CapHandleId, EffectTerminalStatus, HandleLease, HandleProvenance, HandleScope,
    EVENT_CAPABILITY_COMPLETED, EVENT_CAPABILITY_FAILED, EVENT_CAPABILITY_INVOKED,
};
use plurora_runtime::{
    AppendEventRequest, CapabilityInvocationRequest, EventStore, OpenSessionRequest,
    ProtocolContext, RuntimeConfig,
};
use serde_json::json;

use super::fixtures::*;

pub(crate) async fn session_open() -> anyhow::Result<()> {
    let (store, runtime) = runtime();
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    let events = store.list_session(&session.id).await?;
    anyhow::ensure!(events.len() == 1, "expected one session-open event");
    Ok(())
}

pub(crate) async fn event_append_authorized() -> anyhow::Result<()> {
    let (store, runtime) = runtime();
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    runtime
        .load_package(event_package("example/echo", true, true))
        .await?;
    runtime
        .append_event(AppendEventRequest {
            session_id: session.id.clone(),
            writer_package_id: "example/echo".to_string(),
            kind: "example/echo/conformance.event".to_string(),
            payload: json!({"conformance": true}),
            metadata: json!({}),
        })
        .await?;
    anyhow::ensure!(
        store.list_session(&session.id).await?.len() == 2,
        "expected append event"
    );
    Ok(())
}

pub(crate) async fn event_append_without_permission_denied() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    runtime
        .load_package(event_package("example/noappend", true, false))
        .await?;
    let denied = runtime
        .append_event(AppendEventRequest {
            session_id: session.id,
            writer_package_id: "example/noappend".to_string(),
            kind: "example/noappend/event".to_string(),
            payload: json!({}),
            metadata: json!({}),
        })
        .await;
    anyhow::ensure!(
        denied.is_err(),
        "append without permission unexpectedly succeeded"
    );
    Ok(())
}

pub(crate) async fn platform_event_namespace_denied() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    runtime
        .load_package(event_package("example/writer", true, true))
        .await?;
    let denied = runtime
        .append_event(AppendEventRequest {
            session_id: session.id,
            writer_package_id: "example/writer".to_string(),
            kind: plurora_core::EVENT_SESSION_OPENED.to_string(),
            payload: json!({}),
            metadata: json!({}),
        })
        .await;
    anyhow::ensure!(denied.is_err(), "package wrote a platform-owned event");
    Ok(())
}

pub(crate) async fn event_read_without_permission_denied() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    runtime
        .load_package(event_package("example/noread", false, false))
        .await?;
    let denied = runtime
        .list_events_for(&session.id, Some(&"example/noread".to_string()))
        .await;
    anyhow::ensure!(
        denied.is_err(),
        "event read without permission unexpectedly succeeded"
    );
    Ok(())
}

pub(crate) async fn closed_session_rejects_append() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    runtime
        .load_package(event_package("example/writer", true, true))
        .await?;
    runtime.close_session(session.id.clone()).await?;
    let denied = runtime
        .append_event(AppendEventRequest {
            session_id: session.id,
            writer_package_id: "example/writer".to_string(),
            kind: "example/writer/event".to_string(),
            payload: json!({}),
            metadata: json!({}),
        })
        .await;
    anyhow::ensure!(
        denied.is_err(),
        "append to closed session unexpectedly succeeded"
    );
    Ok(())
}

pub(crate) async fn event_range_replay() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    runtime
        .load_package(event_package("example/range", true, true))
        .await?;
    for idx in 0..3 {
        runtime
            .append_event(AppendEventRequest {
                session_id: session.id.clone(),
                writer_package_id: "example/range".to_string(),
                kind: "example/range/event".to_string(),
                payload: json!({"idx": idx}),
                metadata: json!({}),
            })
            .await?;
    }
    let value = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "journal.list",
            json!({"session_id": session.id, "after_sequence": 1, "limit": 2, "kind_prefix": "example/range"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let events = value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("event list did not return array"))?;
    anyhow::ensure!(
        events.len() == 2,
        "expected two ranged events, got {}",
        events.len()
    );
    anyhow::ensure!(
        events[0]["sequence"] == json!(2),
        "range did not resume after sequence"
    );
    Ok(())
}

pub(crate) async fn capability_invoke() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(echo_package(
            "example/echo-rust-inproc",
            "example/echo-rust-inproc/echo",
        ))
        .await?;
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("example/echo-rust-inproc/echo".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({"ok": true}),
        })
        .await?;
    anyhow::ensure!(result.output == json!({"ok": true}), "echo output mismatch");
    Ok(())
}

pub(crate) async fn capability_handle_invoke() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(echo_package("example/handle", "example/handle/echo"))
        .await?;
    let handle = mint_test_handle(&runtime, "example/handle", "example/handle/echo", None).await;
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: Some(handle),
            capability_id: None,
            caller_package_id: Some("example/handle".to_string()),
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({"via": "handle"}),
        })
        .await?;
    anyhow::ensure!(
        result.output == json!({"via": "handle"}),
        "handle invoke mismatch"
    );
    Ok(())
}

pub(crate) async fn capability_handle_attenuate_invoke() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(echo_package("example/attenuate", "example/attenuate/echo"))
        .await?;
    let parent = mint_test_handle(
        &runtime,
        "example/attenuate",
        "example/attenuate/echo",
        None,
    )
    .await;
    let response = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "authority.handle.attenuate",
            json!({"parent_handle": parent, "constraints": {"max_bytes": 1024}}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let child: CapHandle = serde_json::from_value(response["handle"].clone())?;
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: Some(child.id),
            capability_id: None,
            caller_package_id: Some("example/attenuate".to_string()),
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({"via": "attenuated"}),
        })
        .await?;
    anyhow::ensure!(
        result.output == json!({"via": "attenuated"}),
        "attenuated invoke mismatch"
    );
    Ok(())
}

pub(crate) async fn capability_handle_revoke_blocks_invoke() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(echo_package("example/revoke", "example/revoke/echo"))
        .await?;
    let handle = mint_test_handle(&runtime, "example/revoke", "example/revoke/echo", None).await;
    runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "authority.handle.revoke",
            json!({"handle": handle}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let denied = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: Some(handle),
            capability_id: None,
            caller_package_id: Some("example/revoke".to_string()),
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({"blocked": true}),
        })
        .await;
    anyhow::ensure!(denied.is_err(), "revoked handle should fail");
    Ok(())
}

pub(crate) async fn capability_auto_mint_legacy_invoke() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(echo_package("example/legacy", "example/legacy/echo"))
        .await?;
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("example/legacy/echo".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({"legacy": true}),
        })
        .await?;
    anyhow::ensure!(
        result.output == json!({"legacy": true}),
        "legacy auto-mint invoke mismatch"
    );
    Ok(())
}

pub(crate) async fn capability_invoke_events_completed() -> anyhow::Result<()> {
    let (store, runtime) = runtime();
    runtime
        .load_package(echo_package("example/events", "example/events/echo"))
        .await?;
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("example/events/echo".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({"events": true}),
        })
        .await?;
    let invoked = store.list_kind_prefix(EVENT_CAPABILITY_INVOKED).await?;
    let completed = store.list_kind_prefix(EVENT_CAPABILITY_COMPLETED).await?;
    anyhow::ensure!(
        !invoked.is_empty() && !completed.is_empty(),
        "missing capability lifecycle events"
    );
    anyhow::ensure!(
        invoked[0].payload["correlation_id"] == completed[0].payload["correlation_id"],
        "correlation_id mismatch"
    );
    let duration_ms = completed[0].payload["duration_ms"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("duration missing"))?;
    anyhow::ensure!(
        (1..60_000).contains(&duration_ms),
        "duration should be non-zero and reasonable"
    );
    anyhow::ensure!(
        result.correlation_id.to_string()
            == completed[0].payload["correlation_id"]
                .as_str()
                .unwrap_or_default(),
        "result correlation mismatch"
    );
    let receipt = result
        .receipt
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("capability result receipt missing"))?;
    anyhow::ensure!(
        completed[0].payload["receipt"]["digest"] == json!(receipt.digest),
        "capability event receipt mismatch"
    );
    let replay = runtime.replay_effect_receipt(&receipt.digest).await?;
    anyhow::ensure!(
        replay.receipt.effect_kind == "capability.invoke"
            && replay.receipt.status == EffectTerminalStatus::Succeeded
            && replay.outputs == vec![json!({"events": true})],
        "capability success receipt is incomplete"
    );
    Ok(())
}

pub(crate) async fn capability_invoke_events_failed() -> anyhow::Result<()> {
    let (store, runtime) = runtime();
    let denied = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("example/missing/echo".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await;
    anyhow::ensure!(denied.is_err(), "missing capability should fail");
    let failed = store.list_kind_prefix(EVENT_CAPABILITY_FAILED).await?;
    anyhow::ensure!(!failed.is_empty(), "missing capability failed event");
    let duration_ms = failed[0].payload["duration_ms"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("failed duration missing"))?;
    anyhow::ensure!(
        (1..60_000).contains(&duration_ms),
        "failed duration should be non-zero and reasonable"
    );
    let receipt_digest = failed[0].payload["receipt"]["digest"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("failed capability receipt missing"))?;
    let replay = runtime.replay_effect_receipt(receipt_digest).await?;
    anyhow::ensure!(
        replay.receipt.effect_kind == "capability.invoke"
            && replay.receipt.status == EffectTerminalStatus::Failed
            && replay.outputs.is_empty(),
        "capability failed receipt is incomplete"
    );
    Ok(())
}

async fn mint_test_handle(
    runtime: &plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>,
    holder: &str,
    capability_id: &str,
    max_invocations: Option<u32>,
) -> CapHandleId {
    let handle = CapHandle {
        id: CapHandleId::new(),
        cap_type: capability_id.to_string(),
        cap_version: "0.1.0".to_string(),
        scope: HandleScope {
            holder_package_id: holder.to_string(),
            session_id: None,
        },
        constraints: json!({}),
        lease: HandleLease {
            expires_at: None,
            max_invocations,
            invocations_used: 0,
        },
        provenance: HandleProvenance {
            granted_at: chrono::Utc::now(),
            granted_by_package_id: plurora_core::PLATFORM_RUNTIME_ID.to_string(),
            via_method: "package_load".to_string(),
        },
        parent: None,
        revoked: false,
    };
    runtime.handles().mint(handle).await
}

pub(crate) async fn ambiguous_provider_denied() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(echo_package("example/provider-a", "example/shared/echo"))
        .await?;
    runtime
        .load_package(echo_package("example/provider-b", "example/shared/echo"))
        .await?;
    let denied = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("example/shared/echo".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await;
    anyhow::ensure!(denied.is_err(), "ambiguous route unexpectedly succeeded");
    Ok(())
}

pub(crate) async fn explicit_provider_selected() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(echo_package(
            "example/provider-a",
            "example/shared/selected",
        ))
        .await?;
    runtime
        .load_package(echo_package(
            "example/provider-b",
            "example/shared/selected",
        ))
        .await?;
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("example/shared/selected".to_string()),
            caller_package_id: None,
            provider_package_id: Some("example/provider-b".to_string()),
            version: Some("^0.1".to_string()),
            session_id: None,
            input: json!({"selected": true}),
        })
        .await?;
    anyhow::ensure!(
        result.provider_package_id == "example/provider-b",
        "explicit provider was ignored"
    );
    Ok(())
}

pub(crate) async fn unload_removes_capability() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(echo_package("example/temp", "example/temp/echo"))
        .await?;
    runtime.unload_package(&"example/temp".to_string()).await?;
    let denied = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("example/temp/echo".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await;
    anyhow::ensure!(denied.is_err(), "unloaded capability remained invokable");
    Ok(())
}

pub(crate) async fn first_party_no_privilege() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(echo_package("plurora/echo", "example/shared/echo"))
        .await?;
    runtime
        .load_package(echo_package("thirdparty/echo", "example/shared/echo"))
        .await?;
    let denied = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("example/shared/echo".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await;
    anyhow::ensure!(
        denied.is_err(),
        "first-party-looking package won ambiguous route"
    );
    Ok(())
}

pub(crate) async fn capability_schema_rejects_invalid() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(schema_echo_package(
            "example/schema-echo",
            "example/schema-echo/echo",
            json!({"type": "object", "required": ["ok"]}),
            json!({"type": "object", "required": ["ok"]}),
        ))
        .await?;
    let denied = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("example/schema-echo/echo".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await;
    anyhow::ensure!(
        denied.is_err(),
        "invalid capability input unexpectedly passed"
    );
    Ok(())
}

pub(crate) async fn event_schema_rejects_invalid() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    runtime.load_package(event_schema_package()).await?;
    let denied = runtime
        .append_event(AppendEventRequest {
            session_id: session.id,
            writer_package_id: "example/schema-writer".to_string(),
            kind: "example/schema-writer/event.checked".to_string(),
            payload: json!({}),
            metadata: json!({}),
        })
        .await;
    anyhow::ensure!(denied.is_err(), "invalid event payload unexpectedly passed");
    Ok(())
}

pub(crate) async fn host_diagnostics() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(echo_package("example/diag", "example/diag/echo"))
        .await?;
    let diagnostics = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.diagnostics",
            json!({}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        diagnostics["package_count"] == json!(1),
        "diagnostics package count mismatch"
    );
    Ok(())
}

pub(crate) async fn host_profile_autoload() -> anyhow::Result<()> {
    use crate::commands::host;
    use plurora_runtime::{InMemoryEventStore, Runtime};
    use std::path::PathBuf;
    use std::sync::Arc;

    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
    host::load_host_profile(runtime.clone(), PathBuf::from("profiles/forge-alpha.yaml")).await?;
    let packages = runtime.list_packages().await;
    anyhow::ensure!(
        packages
            .iter()
            .any(|package| package.id == "example/echo-rust-inproc"),
        "profile did not autoload rust package"
    );
    anyhow::ensure!(
        packages
            .iter()
            .any(|package| package.id == "example/echo-subprocess-python"),
        "profile did not autoload subprocess package"
    );
    Ok(())
}

pub(crate) async fn asset_put_get_list() -> anyhow::Result<()> {
    let (store, runtime) = runtime();
    let sentinel = format!("phase4-large-content-sentinel:{}", "x".repeat(1024 * 1024));
    let record_value = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "object.put",
            json!({"mime": "text/plain", "content": sentinel.clone(), "metadata": {"purpose": "conformance"}}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let asset = &record_value["asset"];
    let asset_id = asset["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("asset put returned no id"))?;
    let digest = asset["hash"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("asset put returned no digest"))?;
    anyhow::ensure!(
        digest.starts_with("sha256:") && digest.len() == 71,
        "asset digest is not canonical sha256"
    );
    anyhow::ensure!(
        asset["descriptor"]["digest"] == json!(digest)
            && asset["descriptor"]["size_bytes"] == json!(sentinel.len())
            && record_value["descriptor"] == asset["descriptor"],
        "asset descriptor does not match legacy record fields"
    );
    let get_value = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "object.get",
            json!({"asset_id": asset_id}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        get_value["content"] == json!(sentinel)
            && get_value["record"] == *asset
            && get_value.as_object().is_some_and(|value| value.len() == 2),
        "asset get did not preserve the original record/content contract"
    );
    let list_value = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "object.list",
            json!({}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        list_value.as_array().map(|items| items.len()).unwrap_or(0) == 1,
        "asset list missing record"
    );
    let events = store.list_all().await?;
    let event = events
        .iter()
        .find(|event| event.kind == plurora_core::EVENT_ASSET_PUT)
        .ok_or_else(|| anyhow::anyhow!("asset put event missing"))?;
    anyhow::ensure!(
        event.metadata.get("content").is_none()
            && event.metadata["content_included"] == json!(false)
            && event.metadata["artifact_digest"] == json!(digest),
        "asset event persisted inline content or omitted its object reference"
    );
    anyhow::ensure!(
        !serde_json::to_string(event)?.contains(&sentinel),
        "asset content leaked into the journal event"
    );
    Ok(())
}

pub(crate) async fn session_fork_branch() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    let branch_value = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "context.fork",
            json!({"parent_session_id": session.id, "forked_from_sequence": 0, "metadata": {"why": "try"}}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        branch_value["parent_session_id"] == json!(session.id),
        "branch parent mismatch"
    );
    let branches = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "context.branch.list",
            json!({"session_id": session.id}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        branches.as_array().map(|items| items.len()).unwrap_or(0) == 1,
        "branch list missing fork"
    );
    Ok(())
}

pub(crate) async fn projection_rebuild() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    runtime
        .load_package(event_package("example/projection", true, true))
        .await?;
    runtime
        .append_event(AppendEventRequest {
            session_id: session.id.clone(),
            writer_package_id: "example/projection".to_string(),
            kind: "example/projection/event".to_string(),
            payload: json!({"ok": true}),
            metadata: json!({}),
        })
        .await?;
    runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "projection.register",
            json!({"id": "example/projection/state", "session_id": session.id, "source_kind_prefix": "example/projection", "state": {}}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let rebuilt = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "projection.rebuild",
            json!({"projection_id": "example/projection/state"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        rebuilt["state"]["event_count"] == json!(1),
        "projection event count mismatch"
    );
    Ok(())
}
