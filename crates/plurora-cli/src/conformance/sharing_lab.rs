//! Conformance tests for `official/sharing-lab` (Experience Beta 6).
//!
//! Covers:
//! 1. Sharing contract shape (9 capabilities, 3 surfaces, ordinary package, red lines)
//! 2. Export composition bundle produces bundle with lockfile and disclosure
//! 3. Import composition bundle validates shape and compatibility
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

const PACKAGE_ID: &str = "official/sharing-lab";

async fn load_sharing_lab(
) -> anyhow::Result<plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from("packages/official/sharing-lab/manifest.yaml"))
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
/// red lines (no marketplace, no billing, no signing network, no kernel.v1.sharing).
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
        contract.output["output_shapes"]["composition_bundle"].is_array(),
        "output_shapes must have composition_bundle"
    );
    anyhow::ensure!(
        contract.output["output_shapes"]["package_set_lockfile"].is_array(),
        "output_shapes must have package_set_lockfile"
    );

    // Red lines
    anyhow::ensure!(contract.output["red_lines"]["no_marketplace"] == json!(true));
    anyhow::ensure!(contract.output["red_lines"]["no_billing"] == json!(true));
    anyhow::ensure!(contract.output["red_lines"]["no_signing_network"] == json!(true));
    anyhow::ensure!(contract.output["red_lines"]["no_kernel_sharing"] == json!(true));
    anyhow::ensure!(contract.output["red_lines"]["no_raw_secrets"] == json!(true));

    // No inference / no network
    anyhow::ensure!(contract.output["inference_performed"] == json!(false));
    anyhow::ensure!(contract.output["network_performed"] == json!(false));

    Ok(())
}

/// Case 2: Export composition bundle — produces bundle with lockfile, AI disclosure, no marketplace fields.
pub(crate) async fn sharing_export_bundle() -> anyhow::Result<()> {
    let rt = load_sharing_lab().await?;

    let export = invoke(
        &rt,
        "export_composition_bundle",
        json!({
            "composition_id": "test-composition",
            "packages": [
                {"package_id": "official/playable-seed", "version": "0.1.0"},
                {"package_id": "official/memory-lab", "version": "0.1.0"},
            ],
            "composition_manifest": {
                "title": "Test Composition",
                "required_capabilities": ["official/playable-seed/launch"],
            }
        }),
    )
    .await?;

    anyhow::ensure!(export.output["kind"] == json!("composition_bundle"));
    anyhow::ensure!(export.output["bundle_id"].is_string());
    anyhow::ensure!(export.output["format_version"] == json!("1"));
    anyhow::ensure!(export.output["package_set_lockfile"].is_object());
    anyhow::ensure!(export.output["ai_disclosure"].is_object());
    anyhow::ensure!(export.output["no_marketplace_fields"] == json!(true));
    anyhow::ensure!(export.output["no_billing_fields"] == json!(true));
    anyhow::ensure!(export.output["no_signing_network_fields"] == json!(true));
    anyhow::ensure!(export.output["inference_performed"] == json!(false));

    Ok(())
}

/// Case 3: Import composition bundle — validates shape, compatibility status, plan-only.
pub(crate) async fn sharing_import_bundle() -> anyhow::Result<()> {
    let rt = load_sharing_lab().await?;

    // Compatible import
    let import_ok = invoke(
        &rt,
        "import_composition_bundle",
        json!({
            "bundle_id": "bundle:test:abc",
            "format_version": "1",
            "packages": [{"package_id": "official/playable-seed", "version": "0.1.0"}],
            "missing_packages": [],
        }),
    )
    .await?;

    anyhow::ensure!(import_ok.output["kind"] == json!("composition_bundle_import"));
    anyhow::ensure!(import_ok.output["compatibility_status"] == json!("compatible"));
    anyhow::ensure!(import_ok.output["requires_user_approval"] == json!(true));
    anyhow::ensure!(import_ok.output["plan_only"] == json!(true));
    anyhow::ensure!(import_ok.output["no_raw_secrets"] == json!(true));

    // Incompatible import (missing packages)
    let import_missing = invoke(
        &rt,
        "import_composition_bundle",
        json!({
            "bundle_id": "bundle:test:abc",
            "format_version": "1",
            "packages": [],
            "missing_packages": [{"package_id": "official/missing-pkg", "version": "0.1.0"}],
        }),
    )
    .await?;

    anyhow::ensure!(
        import_missing.output["compatibility_status"] == json!("minor_incompatibility")
    );

    // Format mismatch (migration required)
    let import_migrate = invoke(
        &rt,
        "import_composition_bundle",
        json!({
            "bundle_id": "bundle:test:old",
            "format_version": "0",
            "packages": [],
            "missing_packages": [],
        }),
    )
    .await?;

    anyhow::ensure!(import_migrate.output["compatibility_status"] == json!("migration_required"));

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
                {"package_id": "official/playable-seed", "version": "0.1.0"},
                {"package_id": "official/memory-lab", "version": "0.1.0"},
                {"package_id": "official/agentic-forge-lab", "version": "0.2.0"},
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
            "source_ref": "bundle:composition:v1",
            "target_ref": "bundle:composition:v2",
            "source_packages": [
                {"package_id": "official/playable-seed", "version": "0.1.0"},
                {"package_id": "official/old-deprecated-pkg", "version": "0.1.0"},
            ],
            "target_packages": [
                {"package_id": "official/playable-seed", "version": "0.2.0"},
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

    // Export with raw secret should be rejected
    let export_secret = invoke(
        &rt,
        "export_composition_bundle",
        json!({
            "composition_id": "test",
            "api_key": "RawSecretExample1234567890abcdefABCDEF123456",
        }),
    )
    .await?;
    anyhow::ensure!(export_secret.output["kind"] == json!("sharing_lab_rejected"));
    anyhow::ensure!(export_secret.output["redaction_state"] == json!("unsafe_blocked"));

    // Export with marketplace field should be rejected
    let export_marketplace = invoke(
        &rt,
        "export_composition_bundle",
        json!({
            "composition_id": "test",
            "marketplace_category": "games",
        }),
    )
    .await?;
    anyhow::ensure!(export_marketplace.output["kind"] == json!("sharing_lab_rejected"));

    // Import with billing field should be rejected
    let import_billing = invoke(
        &rt,
        "import_composition_bundle",
        json!({
            "bundle_id": "test",
            "billing_token": "bt-12345",
        }),
    )
    .await?;
    anyhow::ensure!(import_billing.output["kind"] == json!("sharing_lab_rejected"));

    // Contract output must not contain kernel.v1.sharing/marketplace/billing namespace
    let contract = invoke(&rt, "describe_sharing_contract", json!({})).await?;
    let output_str = serde_json::to_string(&contract.output).unwrap();
    for token in &[
        "kernel.v1.sharing.",
        "kernel.v1.marketplace.",
        "kernel.v1.billing.",
        "kernel.v1.distribution.",
    ] {
        anyhow::ensure!(
            !output_str.contains(token),
            "contract must not contain {token}"
        );
    }

    Ok(())
}
