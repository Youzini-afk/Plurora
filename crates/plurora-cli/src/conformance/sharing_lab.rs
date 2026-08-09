//! Conformance tests for `plurora/sharing-lab` (Experience Beta 6).
//!
//! Covers:
//! 1. Sharing contract shape (9 capabilities, 3 surfaces, ordinary package, red lines)
//! 2. Export Work bundle produces content-addressed WorkRevision / AssemblyLock references
//! 3. Import Work bundle validates descriptor shape and compatibility without effects
//! 4. Branch/session bundle manifest shape
//! 5. Package-set lockfile pins versions with content addresses
//! 6. Compatibility report detects incompatibilities
//! 7. AI disclosure bundle produces items with disclosure kinds
//! 8. Read-only shared session manifest is local/file-level, no remote
//! 9. Async fork share plan is local, draft, plan-only
//! 10. No marketplace/billing fields and no raw secrets in any capability

use std::path::PathBuf;

use plurora_runtime::CapabilityInvocationRequest;
use serde_json::json;

use super::fixtures::*;
use crate::commands::manifest;

const PACKAGE_ID: &str = "plurora/sharing-lab";
const WORK_REVISION_TYPE_URI: &str = "urn:plurora:work-revision:v1";
const ASSEMBLY_REVISION_TYPE_URI: &str = "urn:plurora:assembly-revision:v1";
const ASSEMBLY_LOCK_TYPE_URI: &str = "urn:plurora:assembly-lock:v1";

fn digest(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

fn artifact_descriptor(type_uri: &str, byte: char, references: &[char]) -> serde_json::Value {
    json!({
        "artifact_type_uri": type_uri,
        "media_type": "application/json",
        "digest": digest(byte),
        "size_bytes": 512,
        "references": references.iter().copied().map(digest).collect::<Vec<_>>(),
        "annotations": {},
    })
}

fn work_bundle_input() -> serde_json::Value {
    json!({
        "work_id": "example/test-work",
        "work_revision": artifact_descriptor(WORK_REVISION_TYPE_URI, 'a', &['c']),
        "assembly_revision": artifact_descriptor(ASSEMBLY_REVISION_TYPE_URI, 'c', &['d']),
        "assembly_lock": artifact_descriptor(ASSEMBLY_LOCK_TYPE_URI, 'b', &['c', 'd']),
        "packages": [
            {"package_id": "plurora/playable-seed", "version": "0.1.0"},
            {"package_id": "plurora/memory-lab", "version": "0.1.0"},
        ],
    })
}

async fn load_sharing_lab(
) -> anyhow::Result<plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from("packages/plurora/sharing-lab/manifest.yaml"))
                .await?,
        )
        .await?;
    Ok(runtime)
}

async fn invoke(
    runtime: &plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>,
    cap: &str,
    input: serde_json::Value,
) -> anyhow::Result<plurora_runtime::CapabilityInvocationResult> {
    runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{PACKAGE_ID}/{cap}")),
            caller_package_id: None,
            provider_package_id: Some(PACKAGE_ID.to_string()),
            version: None,
            session_id: None,
            input,
        })
        .await
        .map_err(Into::into)
}

/// Case 1: Sharing contract — 9 capabilities, 3 surfaces, ordinary package,
/// red lines (no marketplace, no billing, no signing network, no platform.sharing).
pub(crate) async fn sharing_contract() -> anyhow::Result<()> {
    let rt = load_sharing_lab().await?;

    let contract = invoke(&rt, "describe_sharing_contract", json!({})).await?;

    anyhow::ensure!(
        contract.output["kind"] == json!("sharing_lab_contract"),
        "describe_sharing_contract must return sharing_lab_contract kind"
    );
    anyhow::ensure!(
        contract.output["package_kind"] == json!("ordinary"),
        "must be ordinary package"
    );

    // 3 surfaces
    let surfaces = contract.output["surfaces"].as_object().unwrap();
    anyhow::ensure!(
        surfaces.contains_key("forge_panel"),
        "must have forge_panel"
    );
    anyhow::ensure!(
        surfaces.contains_key("assistant_action"),
        "must have assistant_action"
    );
    anyhow::ensure!(surfaces.contains_key("home_card"), "must have home_card");

    // 9 capabilities
    anyhow::ensure!(
        contract.output["capabilities"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
            == 9,
        "describe_sharing_contract must list 9 capabilities"
    );

    // Output shapes defined
    anyhow::ensure!(
        contract.output["output_shapes"].is_object(),
        "must have output_shapes"
    );
    anyhow::ensure!(
        contract.output["output_shapes"]["work_bundle"].is_array(),
        "output_shapes must have work_bundle"
    );
    anyhow::ensure!(
        contract.output["output_shapes"]["package_set_lockfile"].is_array(),
        "output_shapes must have package_set_lockfile"
    );

    // Red lines
    anyhow::ensure!(contract.output["red_lines"]["no_marketplace"] == json!(true));
    anyhow::ensure!(contract.output["red_lines"]["no_billing"] == json!(true));
    anyhow::ensure!(contract.output["red_lines"]["no_signing_network"] == json!(true));
    anyhow::ensure!(contract.output["red_lines"]["no_platform_sharing"] == json!(true));
    anyhow::ensure!(contract.output["red_lines"]["no_raw_secrets"] == json!(true));

    // No inference / no network
    anyhow::ensure!(contract.output["inference_performed"] == json!(false));
    anyhow::ensure!(contract.output["network_performed"] == json!(false));

    Ok(())
}

/// Case 2: Export Work bundle with typed content references and no marketplace fields.
pub(crate) async fn sharing_export_bundle() -> anyhow::Result<()> {
    let rt = load_sharing_lab().await?;

    let export = invoke(&rt, "export_work_bundle", work_bundle_input()).await?;

    anyhow::ensure!(export.output["kind"] == json!("work_bundle"));
    anyhow::ensure!(export.output["bundle_id"].is_string());
    anyhow::ensure!(export.output["format_version"] == json!("1"));
    anyhow::ensure!(export.output["work_id"] == json!("example/test-work"));
    anyhow::ensure!(
        export.output["work_revision"]["artifact_type_uri"] == json!(WORK_REVISION_TYPE_URI)
    );
    anyhow::ensure!(
        export.output["assembly_revision"]["artifact_type_uri"]
            == json!(ASSEMBLY_REVISION_TYPE_URI)
    );
    anyhow::ensure!(
        export.output["assembly_lock"]["artifact_type_uri"] == json!(ASSEMBLY_LOCK_TYPE_URI)
    );
    anyhow::ensure!(export.output["work_revision"]["size_bytes"] == json!(512));
    anyhow::ensure!(export.output["assembly_lock"]["references"].is_array());
    anyhow::ensure!(export.output["package_set_lockfile"].is_object());
    anyhow::ensure!(export.output["ai_disclosure"].is_object());
    anyhow::ensure!(export.output["no_marketplace_fields"] == json!(true));
    anyhow::ensure!(export.output["no_billing_fields"] == json!(true));
    anyhow::ensure!(export.output["no_signing_network_fields"] == json!(true));
    anyhow::ensure!(export.output["inference_performed"] == json!(false));

    Ok(())
}

/// Case 3: Import Work bundle — validates typed references and remains plan-only.
pub(crate) async fn sharing_import_bundle() -> anyhow::Result<()> {
    let rt = load_sharing_lab().await?;

    let exported = invoke(&rt, "export_work_bundle", work_bundle_input()).await?;
    let sharing_manifest =
        manifest::read_manifest(PathBuf::from("packages/plurora/sharing-lab/manifest.yaml"))
            .await?;
    let import_schema = &sharing_manifest
        .provides
        .iter()
        .find(|capability| capability.id == "plurora/sharing-lab/import_work_bundle")
        .ok_or_else(|| anyhow::anyhow!("sharing-lab import capability is not declared"))?
        .input_schema;
    let compiled_import_schema = jsonschema::JSONSchema::compile(import_schema)
        .map_err(|error| anyhow::anyhow!("compile import_work_bundle schema: {error}"))?;
    let schema_errors = compiled_import_schema
        .validate(&exported.output)
        .err()
        .map(|errors| errors.map(|error| error.to_string()).collect::<Vec<_>>())
        .unwrap_or_default();
    anyhow::ensure!(
        schema_errors.is_empty(),
        "export_work_bundle output must satisfy import_work_bundle input schema: {}",
        schema_errors.join("; ")
    );

    // Compatible import of the exact exported Work bundle.
    let import_ok = invoke(&rt, "import_work_bundle", exported.output.clone()).await?;

    anyhow::ensure!(import_ok.output["kind"] == json!("work_bundle_import"));
    anyhow::ensure!(import_ok.output["compatibility_status"] == json!("compatible"));
    anyhow::ensure!(import_ok.output["requires_user_approval"] == json!(true));
    anyhow::ensure!(import_ok.output["plan_only"] == json!(true));
    anyhow::ensure!(import_ok.output["no_raw_secrets"] == json!(true));

    // Incompatible import (missing packages).
    let mut missing_input = exported.output.clone();
    missing_input.as_object_mut().unwrap().insert(
        "missing_packages".to_string(),
        json!([{"package_id": "plurora/missing-pkg", "version": "0.1.0"}]),
    );
    let import_missing = invoke(&rt, "import_work_bundle", missing_input).await?;

    anyhow::ensure!(
        import_missing.output["compatibility_status"] == json!("minor_incompatibility")
    );

    // Another format is unsupported; this capability is not an old-format reader.
    let mut unsupported_input = exported.output;
    unsupported_input["format_version"] = json!("0");
    let import_migrate = invoke(&rt, "import_work_bundle", unsupported_input).await?;

    anyhow::ensure!(import_migrate.output["compatibility_status"] == json!("unsupported"));

    let fixture: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        "examples/bundles/playable-creation-board-work-bundle/bundle.json",
    )?)?;
    let imported_fixture = invoke(&rt, "import_work_bundle", fixture).await?;
    anyhow::ensure!(
        imported_fixture.output["compatibility_status"] == json!("compatible"),
        "the checked-in Work bundle fixture must retain valid content identities"
    );

    Ok(())
}

/// Case 4: Branch/session bundle manifest shape.
pub(crate) async fn sharing_branch_session_bundle() -> anyhow::Result<()> {
    let rt = load_sharing_lab().await?;

    let bundle = invoke(
        &rt,
        "create_branch_session_bundle",
        json!({
            "session_id": "sess:abc123",
            "branch_ref": "branch:feature1",
            "sequence": 100,
        }),
    )
    .await?;

    anyhow::ensure!(bundle.output["kind"] == json!("branch_session_bundle"));
    anyhow::ensure!(bundle.output["session_id"] == json!("sess:abc123"));
    anyhow::ensure!(bundle.output["branch_ref"] == json!("branch:feature1"));
    anyhow::ensure!(bundle.output["sequence"] == json!(100));
    anyhow::ensure!(bundle.output["content_address"].is_string());
    anyhow::ensure!(bundle.output["ai_disclosure"].is_object());
    anyhow::ensure!(bundle.output["requires_user_approval"] == json!(true));
    anyhow::ensure!(bundle.output["inference_performed"] == json!(false));

    Ok(())
}

/// Case 5: Package-set lockfile pins versions with content addresses.
pub(crate) async fn sharing_package_set_lockfile() -> anyhow::Result<()> {
    let rt = load_sharing_lab().await?;

    let lockfile = invoke(
        &rt,
        "create_package_set_lockfile",
        json!({
            "packages": [
                {"package_id": "plurora/playable-seed", "version": "0.1.0"},
                {"package_id": "plurora/memory-lab", "version": "0.1.0"},
                {"package_id": "plurora/agentic-forge-lab", "version": "0.2.0"},
            ]
        }),
    )
    .await?;

    anyhow::ensure!(lockfile.output["kind"] == json!("package_set_lockfile"));
    anyhow::ensure!(lockfile.output["lockfile_id"].is_string());
    let packages = lockfile.output["packages"].as_array().unwrap();
    anyhow::ensure!(packages.len() == 3, "must pin 3 packages");
    for p in packages {
        anyhow::ensure!(
            p["package_id"].is_string(),
            "each package must have package_id"
        );
        anyhow::ensure!(p["version"].is_string(), "each package must have version");
        anyhow::ensure!(
            p["content_address"].is_string(),
            "each package must have content_address"
        );
    }
    anyhow::ensure!(lockfile.output["content_address"].is_string());
    anyhow::ensure!(lockfile.output["inference_performed"] == json!(false));

    Ok(())
}

/// Case 6: Compatibility report detects incompatibilities.
pub(crate) async fn sharing_compatibility_report() -> anyhow::Result<()> {
    let rt = load_sharing_lab().await?;

    let report = invoke(
        &rt,
        "compatibility_report",
        json!({
            "source_ref": "bundle:work:v1",
            "target_ref": "bundle:work:v2",
            "source_packages": [
                {"package_id": "plurora/playable-seed", "version": "0.1.0"},
                {"package_id": "plurora/old-deprecated-pkg", "version": "0.1.0"},
            ],
            "target_packages": [
                {"package_id": "plurora/playable-seed", "version": "0.2.0"},
            ],
        }),
    )
    .await?;

    anyhow::ensure!(report.output["kind"] == json!("compatibility_report"));
    anyhow::ensure!(report.output["report_id"].is_string());
    // Should detect major incompatibility (old-deprecated-pkg missing in target)
    anyhow::ensure!(
        report.output["status"] == json!("major_incompatibility"),
        "should detect major incompatibility"
    );
    let incompat = report.output["incompatibilities"].as_array().unwrap();
    anyhow::ensure!(!incompat.is_empty(), "must have incompatibilities");
    anyhow::ensure!(report.output["inference_performed"] == json!(false));

    Ok(())
}

/// Case 7: AI disclosure bundle produces items with disclosure kinds.
pub(crate) async fn sharing_ai_disclosure_bundle() -> anyhow::Result<()> {
    let rt = load_sharing_lab().await?;

    let disclosure = invoke(
        &rt,
        "ai_disclosure_bundle",
        json!({
            "content_refs": [
                {"content_ref": "asset:board-state", "disclosure_kind": "ai_generated", "description": "Board state was AI-generated"},
                {"content_ref": "asset:player-action", "disclosure_kind": "human_created"},
            ],
            "default_disclosure_kind": "mixed",
        }),
    )
    .await?;

    anyhow::ensure!(disclosure.output["kind"] == json!("ai_disclosure_bundle"));
    anyhow::ensure!(disclosure.output["disclosure_id"].is_string());
    let items = disclosure.output["items"].as_array().unwrap();
    anyhow::ensure!(items.len() == 2, "must have 2 disclosure items");
    anyhow::ensure!(items[0]["disclosure_kind"] == json!("ai_generated"));
    anyhow::ensure!(items[1]["disclosure_kind"] == json!("human_created"));
    anyhow::ensure!(disclosure.output["content_address"].is_string());
    anyhow::ensure!(disclosure.output["inference_performed"] == json!(false));

    Ok(())
}

/// Case 8: Read-only shared session manifest — local/file-level, no remote service.
pub(crate) async fn sharing_read_only_manifest() -> anyhow::Result<()> {
    let rt = load_sharing_lab().await?;

    let manifest = invoke(
        &rt,
        "read_only_share_manifest",
        json!({
            "session_ref": "sess:shared-abc",
            "branch_ref": "branch:main",
            "sequence": 50,
        }),
    )
    .await?;

    anyhow::ensure!(manifest.output["kind"] == json!("read_only_share_manifest"));
    anyhow::ensure!(manifest.output["manifest_id"].is_string());
    anyhow::ensure!(manifest.output["readonly"] == json!(true));
    anyhow::ensure!(manifest.output["share_scope"] == json!("local_file"));
    anyhow::ensure!(manifest.output["no_remote_service"] == json!(true));
    anyhow::ensure!(manifest.output["session_ref"] == json!("sess:shared-abc"));
    anyhow::ensure!(manifest.output["sequence"] == json!(50));
    anyhow::ensure!(manifest.output["content_address"].is_string());
    anyhow::ensure!(manifest.output["inference_performed"] == json!(false));

    Ok(())
}

/// Case 9: Async fork share plan — local proof, draft, plan-only.
pub(crate) async fn sharing_async_fork_plan() -> anyhow::Result<()> {
    let rt = load_sharing_lab().await?;

    let plan = invoke(
        &rt,
        "async_fork_share_plan",
        json!({
            "source_session": "sess:original",
            "target_session": "sess:fork-target",
            "fork_intent": "explore_alternative",
            "branch_ref": "branch:share-fork-1",
        }),
    )
    .await?;

    anyhow::ensure!(plan.output["kind"] == json!("async_fork_share_plan"));
    anyhow::ensure!(plan.output["plan_id"].is_string());
    anyhow::ensure!(plan.output["source_session"] == json!("sess:original"));
    anyhow::ensure!(plan.output["target_session"] == json!("sess:fork-target"));
    anyhow::ensure!(plan.output["fork_intent"] == json!("explore_alternative"));
    anyhow::ensure!(plan.output["status"] == json!("draft"));
    anyhow::ensure!(plan.output["share_scope"] == json!("local_file"));
    anyhow::ensure!(plan.output["no_remote_service"] == json!(true));
    anyhow::ensure!(plan.output["requires_user_approval"] == json!(true));
    anyhow::ensure!(plan.output["plan_only"] == json!(true));
    anyhow::ensure!(plan.output["content_address"].is_string());
    anyhow::ensure!(plan.output["inference_performed"] == json!(false));

    Ok(())
}

/// Case 10: No marketplace/billing fields and no raw secrets in any capability.
pub(crate) async fn sharing_no_marketplace_no_raw_secrets() -> anyhow::Result<()> {
    let rt = load_sharing_lab().await?;

    // Export with raw secret should be rejected.
    let mut secret_input = work_bundle_input();
    secret_input.as_object_mut().unwrap().insert(
        "api_key".to_string(),
        json!("RawSecretExample1234567890abcdefABCDEF123456"),
    );
    let export_secret = invoke(&rt, "export_work_bundle", secret_input).await?;
    anyhow::ensure!(export_secret.output["kind"] == json!("sharing_lab_rejected"));
    anyhow::ensure!(export_secret.output["redaction_state"] == json!("unsafe_blocked"));

    // Export with marketplace field should be rejected.
    let mut marketplace_input = work_bundle_input();
    marketplace_input
        .as_object_mut()
        .unwrap()
        .insert("marketplace_category".to_string(), json!("games"));
    let export_marketplace = invoke(&rt, "export_work_bundle", marketplace_input).await?;
    anyhow::ensure!(export_marketplace.output["kind"] == json!("sharing_lab_rejected"));

    // Import with billing field should be rejected.
    let exported = invoke(&rt, "export_work_bundle", work_bundle_input()).await?;
    let mut billing_input = exported.output;
    billing_input
        .as_object_mut()
        .unwrap()
        .insert("billing_token".to_string(), json!("bt-12345"));
    let import_billing = invoke(&rt, "import_work_bundle", billing_input).await?;
    anyhow::ensure!(import_billing.output["kind"] == json!("sharing_lab_rejected"));

    // Descriptor type, digest, size, and references are executable wire checks.
    let mut invalid_inputs = Vec::new();
    let mut wrong_type = work_bundle_input();
    wrong_type["work_revision"]["artifact_type_uri"] = json!(ASSEMBLY_LOCK_TYPE_URI);
    invalid_inputs.push(wrong_type);

    let mut bad_digest = work_bundle_input();
    bad_digest["work_revision"]["digest"] = json!("sha256:bad");
    invalid_inputs.push(bad_digest);

    let mut bad_size = work_bundle_input();
    bad_size["assembly_lock"]["size_bytes"] = json!("512");
    invalid_inputs.push(bad_size);

    let mut bad_reference = work_bundle_input();
    bad_reference["assembly_lock"]["references"] = json!(["sha256:bad"]);
    invalid_inputs.push(bad_reference);

    for invalid_input in invalid_inputs {
        match invoke(&rt, "export_work_bundle", invalid_input).await {
            Ok(rejected) => {
                anyhow::ensure!(rejected.output["kind"] == json!("sharing_lab_rejected"));
            }
            Err(error) => {
                anyhow::ensure!(
                    error.to_string().contains("schema"),
                    "invalid Work descriptor failed outside the schema or structured rejection boundary: {error}"
                );
            }
        }
    }

    // Contract output must not contain platform.sharing/marketplace/billing namespace
    let contract = invoke(&rt, "describe_sharing_contract", json!({})).await?;
    let output_str = serde_json::to_string(&contract.output).unwrap();
    for token in &[
        "platform.sharing.",
        "platform.marketplace.",
        "platform.billing.",
        "platform.distribution.",
    ] {
        anyhow::ensure!(
            !output_str.contains(token),
            "contract must not contain {token}"
        );
    }

    Ok(())
}
