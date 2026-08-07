use plurora_core::{
    CapabilityDescriptor, CapabilityPermissions, EntryDescriptor, PackageContributions,
    PackageEntry, PackageManifest, PermissionSet, SandboxPolicy,
};
use plurora_runtime::{CapabilityInvocationRequest, ProtocolContext};
use serde_json::json;

use super::fixtures::runtime;

pub(crate) async fn package_audit_report() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime.load_package(audit_provider_package()).await?;
    runtime.load_package(audit_caller_package()).await?;

    runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("example/audit-provider/a".to_string()),
            caller_package_id: Some("example/audit-caller".to_string()),
            provider_package_id: Some("example/audit-provider".to_string()),
            version: None,
            session_id: None,
            input: json!({"ok": true}),
        })
        .await?;

    let report_value = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.package.audit",
            json!({"package_id": "example/audit-caller"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!("{:?}", error))?;

    let used = report_value
        .pointer("/used/capabilities_invoked/example~1audit-provider~1a")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    anyhow::ensure!(used == 1, "audit report did not count used capability A");

    let unused = report_value
        .pointer("/unused/capabilities_unused")
        .and_then(|value| value.as_array())
        .ok_or_else(|| anyhow::anyhow!("audit report missing unused capabilities"))?;
    anyhow::ensure!(
        unused
            .iter()
            .any(|value| value == "example/audit-provider/b"),
        "capability B should be unused"
    );
    anyhow::ensure!(
        unused
            .iter()
            .any(|value| value == "example/audit-provider/c"),
        "capability C should be unused"
    );

    let suggestions = report_value
        .pointer("/suggestions")
        .and_then(|value| value.as_array())
        .ok_or_else(|| anyhow::anyhow!("audit report missing suggestions"))?;
    anyhow::ensure!(
        suggestions
            .iter()
            .any(|value| value.get("target") == Some(&json!("example/audit-provider/b"))),
        "missing suggestion for capability B"
    );
    anyhow::ensure!(
        suggestions
            .iter()
            .any(|value| value.get("target") == Some(&json!("example/audit-provider/c"))),
        "missing suggestion for capability C"
    );

    Ok(())
}

fn audit_provider_package() -> PackageManifest {
    let id = "example/audit-provider";
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
        provides: ["a", "b", "c"]
            .into_iter()
            .map(|suffix| CapabilityDescriptor {
                id: format!("{id}/{suffix}"),
                version: "0.1.0".to_string(),
                input_schema: serde_json::Value::Null,
                output_schema: serde_json::Value::Null,
                streaming: false,
                side_effects: Vec::new(),
                description: None,
            })
            .collect(),
        consumes: Vec::new(),
        requires: Vec::new(),
        contributes: PackageContributions::default(),
        permissions: PermissionSet::default(),
        sandbox_policy: SandboxPolicy::default(),
    }
}

fn audit_caller_package() -> PackageManifest {
    PackageManifest {
        schema_version: 1,
        id: "example/audit-caller".to_string(),
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
        provides: Vec::new(),
        consumes: Vec::new(),
        requires: Vec::new(),
        contributes: PackageContributions::default(),
        permissions: PermissionSet {
            capabilities: CapabilityPermissions {
                invoke: vec![
                    "example/audit-provider/a".to_string(),
                    "example/audit-provider/b".to_string(),
                    "example/audit-provider/c".to_string(),
                ],
            },
            ..PermissionSet::default()
        },
        sandbox_policy: SandboxPolicy::default(),
    }
}
