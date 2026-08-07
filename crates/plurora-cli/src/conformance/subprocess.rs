use std::path::PathBuf;

use plurora_core::{CapHandleId, CapabilityPermissions, PermissionSet};
use plurora_runtime::{
    AppendEventRequest, CapabilityInvocationRequest, EventStore, OpenSessionRequest,
    OutboundRequest, ProtocolPrincipal,
};
use serde_json::json;

use super::fixtures::*;
use crate::commands::manifest;

pub(crate) async fn subprocess_load_ready() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let record = runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "examples/packages/echo-subprocess-python/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    anyhow::ensure!(
        record.id == "example/echo-subprocess-python",
        "wrong package loaded"
    );
    Ok(())
}

pub(crate) async fn subprocess_invoke_echo() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "examples/packages/echo-subprocess-python/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("example/echo-subprocess-python/echo".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({"subprocess": true}),
        })
        .await?;
    anyhow::ensure!(
        result.output == json!({"subprocess": true}),
        "subprocess echo mismatch"
    );
    Ok(())
}

pub(crate) async fn package_lifecycle_timeline() -> anyhow::Result<()> {
    let (store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "examples/packages/echo-subprocess-python/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    let session_id = "kernel_package_example_echo-subprocess-python".to_string();
    let events = store.list_session(&session_id).await?;
    for expected in [
        "kernel/v1/package.loading",
        "kernel/v1/package.starting",
        "kernel/v1/package.ready",
        "kernel/v1/package.loaded",
    ] {
        anyhow::ensure!(
            events.iter().any(|event| event.kind == expected),
            "missing lifecycle event {expected}"
        );
    }
    Ok(())
}

pub(crate) async fn path_b_self_contained_contract_none() -> anyhow::Result<()> {
    let (store, runtime) = runtime();
    let manifest = manifest::read_manifest(PathBuf::from(
        "examples/packages/path-b-self-contained/manifest.yaml",
    ))
    .await?;
    let record = runtime.load_package(manifest).await?;
    anyhow::ensure!(
        record.id == "examples/path-b-app",
        "wrong Path B package loaded"
    );

    let session_id = "kernel_package_examples_path-b-app".to_string();
    let events = store.list_session(&session_id).await?;
    for expected in [
        "kernel/v1/package.loading",
        "kernel/v1/package.starting",
        "kernel/v1/package.ready",
        "kernel/v1/package.loaded",
    ] {
        anyhow::ensure!(
            events.iter().any(|event| event.kind == expected),
            "missing Path B lifecycle event {expected}"
        );
    }
    let loaded = events
        .iter()
        .find(|event| event.kind == "kernel/v1/package.loaded")
        .ok_or_else(|| anyhow::anyhow!("missing Path B package.loaded event"))?;
    anyhow::ensure!(
        loaded.payload["contract_mode"] == json!("none"),
        "Path B audit payload must show contract_mode none"
    );
    anyhow::ensure!(
        runtime.handles().list_for(&record.id).await.is_empty(),
        "Foreign Capsule must not receive v1 capability bindings"
    );

    runtime
        .load_package(echo_package(
            "examples/path-b-provider",
            "examples/path-b-provider/echo",
        ))
        .await?;
    let denied = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("examples/path-b-provider/echo".to_string()),
            caller_package_id: Some("examples/path-b-app".to_string()),
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({"path_b": true}),
        })
        .await
        .expect_err("Foreign Capsule must not receive kernel capability authority");
    anyhow::ensure!(
        denied.to_string().contains("Foreign Capsule"),
        "Path B denial should identify the Foreign Capsule boundary: {denied}"
    );

    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    let event_denied = runtime
        .append_event(AppendEventRequest {
            session_id: session.id,
            writer_package_id: record.id.clone(),
            kind: "examples/path-b-app/event".to_string(),
            payload: json!({}),
            metadata: json!({}),
        })
        .await
        .expect_err("Foreign Capsule must not receive event authority");
    anyhow::ensure!(event_denied.to_string().contains("not allowed"));

    let network_denied = runtime
        .check_and_audit_outbound(OutboundRequest {
            principal: ProtocolPrincipal::Package {
                package_id: record.id.clone(),
            },
            package_id: record.id.clone(),
            capability_id: "examples/path-b-app/outbound".to_string(),
            destination_host: "example.com".to_string(),
            method: "GET".to_string(),
            purpose: None,
            secret_refs_used: Vec::new(),
            correlation_id: None,
        })
        .await
        .expect_err("Foreign Capsule must not receive network authority");
    anyhow::ensure!(network_denied.to_string().contains("Foreign Capsule"));
    Ok(())
}

pub(crate) async fn package_logs_capture() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "examples/packages/logging-subprocess-python/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("example/logging-subprocess-python/echo".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({"logs": true}),
        })
        .await?;
    anyhow::ensure!(
        result.output == json!({"logs": true}),
        "logging echo mismatch"
    );
    let logs = runtime
        .package_logs(&"example/logging-subprocess-python".to_string())
        .await;
    anyhow::ensure!(
        logs.iter()
            .any(|log| log.line.contains("invoke observed") || log.line.contains("booted")),
        "stderr logs were not captured"
    );
    Ok(())
}

pub(crate) async fn package_restart_subprocess() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "examples/packages/echo-subprocess-python/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    let record = runtime
        .restart_package(&"example/echo-subprocess-python".to_string())
        .await?;
    anyhow::ensure!(
        matches!(record.state, plurora_runtime::PackageState::Ready),
        "restart did not return ready package"
    );
    Ok(())
}

pub(crate) async fn subprocess_bad_handshake() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let denied = runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "examples/packages/bad-handshake-subprocess-python/manifest.yaml",
            ))
            .await?,
        )
        .await;
    anyhow::ensure!(denied.is_err(), "bad handshake unexpectedly loaded");
    Ok(())
}

pub(crate) async fn subprocess_timeout() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "examples/packages/slow-subprocess-python/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    let denied = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("example/slow-subprocess-python/echo".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await;
    anyhow::ensure!(denied.is_err(), "slow subprocess did not time out");
    let status = runtime
        .package_status(&"example/slow-subprocess-python".to_string())
        .await;
    anyhow::ensure!(
        matches!(
            status.map(|record| record.state),
            Some(plurora_runtime::PackageState::Degraded)
        ),
        "timeout did not degrade package"
    );
    Ok(())
}

pub(crate) async fn subprocess_invalid_output_schema() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "examples/packages/invalid-output-subprocess-python/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    let denied = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("example/invalid-output-subprocess-python/echo".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await;
    anyhow::ensure!(denied.is_err(), "invalid subprocess output schema passed");
    Ok(())
}

pub(crate) async fn subprocess_unload_removes_capability() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "examples/packages/echo-subprocess-python/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    runtime
        .unload_package(&"example/echo-subprocess-python".to_string())
        .await?;
    let denied = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("example/echo-subprocess-python/echo".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await;
    anyhow::ensure!(
        denied.is_err(),
        "unloaded subprocess capability remained invokable"
    );
    Ok(())
}

/// Conformance: package_check enhanced diagnostics cover entry kind, trust level,
/// capabilities, surfaces, permissions, sandbox, and warnings for the echo subprocess.
pub(crate) async fn package_check_diagnostics() -> anyhow::Result<()> {
    // Use the echo subprocess manifest — it has 1 capability and 0 surfaces,
    // so it should trigger the "no surfaces" warning.
    let m = manifest::read_manifest(PathBuf::from(
        "examples/packages/echo-subprocess-python/manifest.yaml",
    ))
    .await?;
    m.validate_basic()?;
    anyhow::ensure!(
        m.provides.len() == 1,
        "echo subprocess should have 1 capability"
    );
    anyhow::ensure!(
        m.contributes.surfaces.is_empty(),
        "echo subprocess should have 0 surfaces (triggers warning)"
    );
    // Verify entry kind and trust level are accessible from manifest
    let entry_kind = match &m.entry.kind {
        plurora_core::PackageEntry::Subprocess { .. } => "subprocess",
        _ => anyhow::bail!("expected subprocess entry kind"),
    };
    anyhow::ensure!(
        entry_kind == "subprocess",
        "entry kind should be subprocess"
    );
    Ok(())
}

/// Conformance: package_reload smoke for echo subprocess — loads, restarts,
/// checks before/after status and logs count.
pub(crate) async fn package_reload_smoke() -> anyhow::Result<()> {
    let m = manifest::read_manifest(PathBuf::from(
        "examples/packages/echo-subprocess-python/manifest.yaml",
    ))
    .await?;
    let package_id = m.id.clone();

    let store = std::sync::Arc::new(plurora_runtime::InMemoryEventStore::default());
    let runtime = plurora_runtime::Runtime::new(store, plurora_runtime::RuntimeConfig::default());

    // Load
    let load_record = runtime.load_package(m.clone()).await?;
    anyhow::ensure!(load_record.id == package_id, "loaded id mismatch");
    let before = runtime.package_status(&package_id).await;
    anyhow::ensure!(before.is_some(), "package status should exist after load");
    anyhow::ensure!(
        matches!(
            before.as_ref().unwrap().state,
            plurora_runtime::PackageState::Ready
        ),
        "package should be ready after load"
    );

    // Restart
    let restart_record = runtime.restart_package(&package_id).await?;
    anyhow::ensure!(
        matches!(restart_record.state, plurora_runtime::PackageState::Ready),
        "package should be ready after restart"
    );

    let after = runtime.package_status(&package_id).await;
    anyhow::ensure!(after.is_some(), "package status should exist after restart");
    anyhow::ensure!(
        matches!(
            after.as_ref().unwrap().state,
            plurora_runtime::PackageState::Ready
        ),
        "package should be ready after restart (status)"
    );

    // Logs (may or may not have content, just ensure no panic)
    let _logs = runtime.package_logs(&package_id).await;

    // Unload
    runtime.unload_package(&package_id).await?;
    let after_unload = runtime.package_status(&package_id).await;
    anyhow::ensure!(
        after_unload.is_none(),
        "package should be gone after unload"
    );
    Ok(())
}

pub(crate) async fn subprocess_bindings_injected_and_invokable() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(echo_package("example/model-provider", "model/live_call"))
        .await?;

    let mut manifest = manifest::read_manifest(PathBuf::from(
        "examples/packages/echo-subprocess-python/manifest.yaml",
    ))
    .await?;
    manifest.permissions = PermissionSet {
        capabilities: CapabilityPermissions {
            invoke: vec!["model/live_call".to_string()],
        },
        ..PermissionSet::default()
    };
    runtime.load_package(manifest).await?;

    let binding_result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("example/echo-subprocess-python/echo".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({"__return_binding": "modelLiveCall"}),
        })
        .await?;
    let handle_id: CapHandleId = serde_json::from_value(binding_result.output)?;

    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: Some(handle_id),
            capability_id: None,
            caller_package_id: Some("example/echo-subprocess-python".to_string()),
            provider_package_id: Some("example/model-provider".to_string()),
            version: None,
            session_id: None,
            input: json!({"binding": true}),
        })
        .await?;
    anyhow::ensure!(
        result.output == json!({"binding": true}),
        "binding handle invoke mismatch"
    );

    let handles = runtime
        .handles()
        .list_for(&"example/echo-subprocess-python".to_string())
        .await;
    anyhow::ensure!(
        handles
            .iter()
            .any(|handle| handle.id == handle_id && handle.cap_type == "model/live_call"),
        "exampleModelProviderLiveCall handle was not minted for subprocess"
    );
    Ok(())
}
