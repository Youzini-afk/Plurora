use plurora_core::{
    CapabilityPermissions, EntryDescriptor, PackageContributions, PackageEntry, PermissionSet,
    SandboxPolicy,
};
use plurora_runtime::CapabilityInvocationRequest;
use serde_json::json;

use super::fixtures::*;

/// Proves that a non-official inproc package with a `/preview` capability suffix
/// does NOT receive `asset_preview` output from the shared common handlers.
///
/// The common handlers are restricted to `plurora/` packages; a third-party
/// package routed through the same `plurora-foundation` inproc entry should
/// have its capabilities go unhandled rather than silently served by the
/// generic fallback.
pub(crate) async fn non_first_party_preview_rejected() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    // Register a third-party Package that shares the plurora-foundation inproc entry
    // but whose package ID is outside the plurora/ namespace.
    runtime
        .load_package(plurora_core::PackageManifest {
            schema_version: 1,
            id: "thirdparty/asset-hijack".to_string(),
            version: "0.1.0".to_string(),
            display_name: None,
            description: None,
            author: None,
            license: None,
            entry: EntryDescriptor::v1(PackageEntry::RustInproc {
                crate_ref: "plurora-foundation".to_string(),
                symbol: "register".to_string(),
                abi_version: 1,
            }),
            provides: vec![plurora_core::CapabilityDescriptor {
                id: "thirdparty/asset-hijack/preview".to_string(),
                version: "0.1.0".to_string(),
                input_schema: serde_json::Value::Null,
                output_schema: serde_json::Value::Null,
                streaming: false,
                side_effects: Vec::new(),
                description: None,
            }],
            consumes: Vec::new(),
            requires: Vec::new(),
            contributes: PackageContributions::default(),
            permissions: PermissionSet::default(),
            sandbox_policy: SandboxPolicy::default(),
        })
        .await?;
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("thirdparty/asset-hijack/preview".to_string()),
            caller_package_id: None,
            provider_package_id: Some("thirdparty/asset-hijack".to_string()),
            version: None,
            session_id: None,
            input: json!({"asset_id": "test", "content": "should not preview"}),
        })
        .await;
    anyhow::ensure!(
        result.is_err(),
        "third-party Package with /preview suffix should not succeed (no asset_preview fallback)"
    );
    Ok(())
}

/// Proves that an unknown/unimplemented inproc capability from an first-party Package
/// returns an explicit error instead of a generic permissive success.
///
/// Before the package-aware fix, unhandled capabilities fell through to
/// `common::fallback` which returned `{"ok": true}`. This test ensures that
/// gap is closed: unknown capabilities must fail loudly.
pub(crate) async fn unknown_inproc_capability_errors() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    // Use an first-party Package that goes through plurora-foundation inproc entry
    runtime
        .load_package(plurora_core::PackageManifest {
            schema_version: 1,
            id: "plurora/test-unknown-cap".to_string(),
            version: "0.1.0".to_string(),
            display_name: None,
            description: None,
            author: None,
            license: None,
            entry: EntryDescriptor::v1(PackageEntry::RustInproc {
                crate_ref: "plurora-foundation".to_string(),
                symbol: "register".to_string(),
                abi_version: 1,
            }),
            provides: vec![plurora_core::CapabilityDescriptor {
                id: "plurora/test-unknown-cap/nonexistent_action".to_string(),
                version: "0.1.0".to_string(),
                input_schema: serde_json::Value::Null,
                output_schema: serde_json::Value::Null,
                streaming: false,
                side_effects: Vec::new(),
                description: None,
            }],
            consumes: Vec::new(),
            requires: Vec::new(),
            contributes: PackageContributions::default(),
            permissions: PermissionSet::default(),
            sandbox_policy: SandboxPolicy::default(),
        })
        .await?;
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/test-unknown-cap/nonexistent_action".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/test-unknown-cap".to_string()),
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await;
    anyhow::ensure!(
        result.is_err(),
        "unknown inproc capability should return an error, not generic success"
    );
    let err_msg = result.unwrap_err().to_string();
    anyhow::ensure!(
        err_msg.contains("no handler for inproc capability"),
        "error message should mention missing handler, got: {err_msg}"
    );
    Ok(())
}

pub(crate) async fn inproc_bindings_init_receives_manifest_bindings() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(echo_package(
            "example/inproc-model-provider",
            "model/live_call",
        ))
        .await?;
    runtime
        .load_package(plurora_core::PackageManifest {
            schema_version: 1,
            id: "example/inproc-bindings".to_string(),
            version: "0.1.0".to_string(),
            display_name: None,
            description: None,
            author: None,
            license: None,
            entry: EntryDescriptor::v1(PackageEntry::RustInproc {
                crate_ref: "example-bindings-inproc".to_string(),
                symbol: "register".to_string(),
                abi_version: 1,
            }),
            provides: vec![plurora_core::CapabilityDescriptor {
                id: "example/inproc-bindings/bindings".to_string(),
                version: "0.1.0".to_string(),
                input_schema: serde_json::Value::Null,
                output_schema: serde_json::Value::Null,
                streaming: false,
                side_effects: Vec::new(),
                description: None,
            }],
            consumes: Vec::new(),
            requires: Vec::new(),
            contributes: PackageContributions::default(),
            permissions: PermissionSet {
                capabilities: CapabilityPermissions {
                    invoke: vec!["model/live_call".to_string()],
                },
                ..PermissionSet::default()
            },
            sandbox_policy: SandboxPolicy::default(),
        })
        .await?;

    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("example/inproc-bindings/bindings".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await?;
    let handle_id = result
        .output
        .get("modelLiveCall")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing modelLiveCall binding: {:?}", result.output))?;
    let handles = runtime
        .handles()
        .list_for(&"example/inproc-bindings".to_string())
        .await;
    anyhow::ensure!(
        handles.iter().any(|handle| serde_json::to_value(handle.id)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .as_deref()
            == Some(handle_id)
            && handle.cap_type == "model/live_call"),
        "inproc binding handle did not match manifest declaration"
    );
    Ok(())
}
