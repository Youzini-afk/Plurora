use plurora_runtime::{
    AppendEventRequest, CapabilityInvocationRequest, OpenSessionRequest, ProtocolContext,
    ProtocolError,
};
use serde_json::json;

use super::fixtures::*;

pub(crate) async fn structured_permission_error() -> anyhow::Result<()> {
    let error = ProtocolError::from_anyhow(anyhow::anyhow!(
        "package 'example/nope' is not allowed to read events"
    ));
    anyhow::ensure!(
        error.code == "runtime/error/permission_denied",
        "wrong error code: {}",
        error.code
    );
    Ok(())
}

pub(crate) async fn permission_grant_revoke_audit() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    runtime
        .load_package(event_package("example/grant-reader", true, true))
        .await?;
    runtime
        .append_event(AppendEventRequest {
            session_id: session.id.clone(),
            writer_package_id: "example/grant-reader".to_string(),
            kind: "example/grant-reader/event".to_string(),
            payload: json!({"ok": true}),
            metadata: json!({}),
        })
        .await?;
    let human = json!({"kind": "human", "user_id": "user/conformance"});
    let human_context = ProtocolContext {
        principal: serde_json::from_value(human.clone())?,
        transport: "conformance".to_string(),
        authority: None,
        host_operation: None,
        session_id: None,
        correlation_id: None,
        parent_invocation_id: None,
    };
    let denied = runtime
        .call_protocol(
            &human_context,
            "journal.list",
            json!({"session_id": session.id}),
        )
        .await;
    anyhow::ensure!(denied.is_err(), "human read should require grant");
    let grant = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "authority.grant.create",
            json!({"principal": human, "permission": "events.read", "scope": session.id, "reason": "conformance"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let grant_id = grant["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("grant missing id"))?
        .to_string();
    let allowed = runtime
        .call_protocol(
            &human_context,
            "journal.list",
            json!({"session_id": session.id}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        allowed
            .as_array()
            .map(|items| !items.is_empty())
            .unwrap_or(false),
        "grant did not allow event read"
    );
    runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "authority.grant.revoke",
            json!({"grant_id": grant_id}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let audit = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "authority.decision.list",
            json!({}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        audit.as_array().map(|items| items.len()).unwrap_or(0) >= 2,
        "permission audit missing grant/revoke events"
    );
    Ok(())
}

pub(crate) async fn assistant_capability_grant() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(echo_package(
            "example/assistant-target",
            "example/assistant-target/echo",
        ))
        .await?;
    let assistant = json!({"kind": "assistant", "assistant_id": "assistant/conformance", "delegated_user_id": "user/conformance"});
    let assistant_context = ProtocolContext {
        principal: serde_json::from_value(assistant.clone())?,
        transport: "conformance".to_string(),
        authority: None,
        host_operation: None,
        session_id: None,
        correlation_id: None,
        parent_invocation_id: None,
    };
    let denied = runtime
        .call_protocol(
            &assistant_context,
            "capability.invoke",
            json!({"capability_id": "example/assistant-target/echo", "input": {"ok": true}}),
        )
        .await;
    anyhow::ensure!(denied.is_err(), "assistant invoke should require grant");
    runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "authority.grant.create",
            json!({"principal": assistant, "permission": "capabilities.invoke", "scope": "example/assistant-target"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let result = runtime
        .call_protocol(
            &assistant_context,
            "capability.invoke",
            json!({"capability_id": "example/assistant-target/echo", "input": {"ok": true}}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        result["output"] == json!({"ok": true}),
        "assistant grant did not permit invoke"
    );
    Ok(())
}

pub(crate) async fn principal_cannot_self_assert_writer() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    runtime
        .load_package(event_package("example/actual", true, true))
        .await?;
    let event = runtime
        .append_event_with_context(
            &ProtocolContext::package("example/actual", "conformance"),
            AppendEventRequest {
                session_id: session.id,
                writer_package_id: "example/spoofed".to_string(),
                kind: "example/actual/event".to_string(),
                payload: json!({}),
                metadata: json!({}),
            },
        )
        .await?;
    anyhow::ensure!(
        event.writer_package_id == "example/actual",
        "writer spoof was accepted"
    );
    Ok(())
}

pub(crate) async fn principal_cannot_self_assert_capability_caller() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(echo_package("example/echo", "example/echo/echo"))
        .await?;
    runtime
        .load_package(event_package("example/actual", false, false))
        .await?;
    let denied = runtime
        .invoke_capability_with_context(
            &ProtocolContext::package("example/actual", "conformance"),
            CapabilityInvocationRequest {
                handle: None,
                capability_id: Some("example/echo/echo".to_string()),
                caller_package_id: None,
                provider_package_id: None,
                version: None,
                session_id: None,
                input: json!({}),
            },
        )
        .await;
    anyhow::ensure!(
        denied.is_err(),
        "caller self-assertion bypassed invoke permission"
    );
    Ok(())
}
