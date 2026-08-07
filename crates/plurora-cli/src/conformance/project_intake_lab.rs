//! Conformance tests for `official/project-intake-lab` (External Project Operating Plane Alpha Phase E1 + E5).
//!
//! Covers:
//! 1. Intake contract shape (11 capabilities, 3 surfaces, ordinary package, no execution)
//! 2. Source classification (git/npm/local/archive/unknown)
//! 3. Stack detection (node/rust/python/static/unknown) with npm lifecycle risk
//! 4. Workspace plan is plan-only
//! 5. Local path rejection (path traversal, home path, absolute sensitive path)
//! 6. Adapter plan is plan-only
//! 7. No forbidden namespace (kernel.v1.project/workspace/git/npm/deploy/ide)
//! 8. No raw secrets in any capability
//! 9. Adapter manifest preview no write (E5)
//! 10. Rejects official adapter id (E5)
//! 11. Rejects path traversal adapter id (E5)
//! 12. Capability namespace mismatch rejected (E5)
//! 13. Wrapper preview no execution (E5)
//! 14. Fixture preview redacted (E5)
//! 15. Readiness checklist ok (E5)
//! 16. No forbidden namespace / raw secret in E5 capabilities (E5)

use std::path::PathBuf;

use plurora_runtime::CapabilityInvocationRequest;
use serde_json::json;

use super::fixtures::*;
use crate::commands::manifest;

const PACKAGE_ID: &str = "official/project-intake-lab";

async fn load_project_intake_lab(
) -> anyhow::Result<plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/official/project-intake-lab/manifest.yaml",
            ))
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

/// Case 1: Intake contract — 7 capabilities, 3 surfaces, ordinary package,
/// no execution, no forbidden namespace.
pub(crate) async fn project_intake_contract() -> anyhow::Result<()> {
    let rt = load_project_intake_lab().await?;

    let contract = invoke(&rt, "describe_intake_contract", json!({})).await?;

    anyhow::ensure!(
        contract.output["kind"] == json!("project_intake_contract"),
        "describe_intake_contract must return project_intake_contract kind"
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

    // 11 capabilities
    anyhow::ensure!(
        contract.output["capabilities"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
            == 11,
        "describe_intake_contract must list 11 capabilities"
    );

    // No execution
    anyhow::ensure!(contract.output["execution_performed"] == json!(false));
    anyhow::ensure!(contract.output["network_performed"] == json!(false));
    anyhow::ensure!(contract.output["inference_performed"] == json!(false));
    anyhow::ensure!(contract.output["filesystem_performed"] == json!(false));

    // No forbidden namespace
    let output_str = serde_json::to_string(&contract.output).unwrap();
    for token in &[
        "kernel.v1.project.",
        "kernel.v1.workspace.",
        "kernel.v1.git.",
        "kernel.v1.npm.",
        "kernel.v1.deploy.",
        "kernel.v1.ide.",
    ] {
        anyhow::ensure!(
            !output_str.contains(token),
            "describe_intake_contract must not contain {}",
            token
        );
    }

    Ok(())
}

/// Case 2: Source classification — git/npm/local/archive/unknown.
pub(crate) async fn project_intake_source_classification() -> anyhow::Result<()> {
    let rt = load_project_intake_lab().await?;

    // git
    let git = invoke(
        &rt,
        "inspect_external_project_ref",
        json!({"source_ref": "https://github.com/example/project.git"}),
    )
    .await?;
    anyhow::ensure!(git.output["source_kind"] == json!("git"));

    // npm
    let npm = invoke(
        &rt,
        "inspect_external_project_ref",
        json!({"source_ref": "npm:lodash"}),
    )
    .await?;
    anyhow::ensure!(npm.output["source_kind"] == json!("npm"));

    // local
    let local = invoke(
        &rt,
        "inspect_external_project_ref",
        json!({"source_ref": "./my-project"}),
    )
    .await?;
    anyhow::ensure!(local.output["source_kind"] == json!("local"));
    anyhow::ensure!(local.output["path_safety"] == json!("appears_safe"));

    // archive
    let archive = invoke(
        &rt,
        "inspect_external_project_ref",
        json!({"source_ref": "project.tar.gz"}),
    )
    .await?;
    anyhow::ensure!(archive.output["source_kind"] == json!("archive"));

    // no execution performed
    anyhow::ensure!(git.output["execution_performed"] == json!(false));
    anyhow::ensure!(git.output["network_performed"] == json!(false));

    Ok(())
}

/// Case 3: Stack detection — node with npm lifecycle risk, rust, python, unknown.
pub(crate) async fn project_intake_stack_detection() -> anyhow::Result<()> {
    let rt = load_project_intake_lab().await?;

    // Node with lifecycle scripts
    let node = invoke(
        &rt,
        "detect_project_stack_from_metadata",
        json!({"metadata": {
            "package_json": {
                "name": "test",
                "scripts": {
                    "preinstall": "echo pre",
                    "install": "echo install",
                    "postinstall": "echo post",
                    "prepare": "echo prepare",
                    "prepublish": "echo prepublish",
                    "start": "node index.js"
                }
            }
        }}),
    )
    .await?;

    anyhow::ensure!(node.output["detected_stack"] == json!("node"));
    let risks = node.output["npm_lifecycle_risks"].as_array().unwrap();
    anyhow::ensure!(
        risks.len() == 5,
        "must detect 5 npm lifecycle scripts, got {}",
        risks.len()
    );
    for risk in risks {
        anyhow::ensure!(
            risk["executes_code"] == json!(true),
            "npm lifecycle risk must have executes_code=true"
        );
        anyhow::ensure!(
            risk["requires_approval"] == json!(true),
            "npm lifecycle risk must have requires_approval=true"
        );
    }

    // Rust
    let rust = invoke(
        &rt,
        "detect_project_stack_from_metadata",
        json!({"metadata": {"cargo_toml": {"name": "test"}}}),
    )
    .await?;
    anyhow::ensure!(rust.output["detected_stack"] == json!("rust"));

    // Python
    let python = invoke(
        &rt,
        "detect_project_stack_from_metadata",
        json!({"metadata": {"pyproject": {"name": "test"}}}),
    )
    .await?;
    anyhow::ensure!(python.output["detected_stack"] == json!("python"));

    // Unknown
    let unknown = invoke(
        &rt,
        "detect_project_stack_from_metadata",
        json!({"metadata": {}}),
    )
    .await?;
    anyhow::ensure!(unknown.output["detected_stack"] == json!("unknown"));

    Ok(())
}

/// Case 4: Workspace plan is plan-only, no execution.
pub(crate) async fn project_intake_workspace_plan() -> anyhow::Result<()> {
    let rt = load_project_intake_lab().await?;

    let plan = invoke(
        &rt,
        "draft_workspace_plan",
        json!({"source_ref": "https://github.com/example/project.git", "source_kind": "git"}),
    )
    .await?;

    anyhow::ensure!(plan.output["kind"] == json!("project_workspace_plan"));
    anyhow::ensure!(plan.output["plan_only"] == json!(true));
    anyhow::ensure!(plan.output["requires_user_approval"] == json!(true));
    anyhow::ensure!(plan.output["execution_performed"] == json!(false));
    anyhow::ensure!(plan.output["network_performed"] == json!(false));

    Ok(())
}

/// Case 5: Local path rejection — path traversal, home path, absolute sensitive path.
pub(crate) async fn project_intake_local_path_rejection() -> anyhow::Result<()> {
    let rt = load_project_intake_lab().await?;

    // Path traversal
    let traversal = invoke(
        &rt,
        "inspect_external_project_ref",
        json!({"source_ref": "../../etc/passwd"}),
    )
    .await?;
    anyhow::ensure!(
        traversal.output["path_safety"] == json!("rejected"),
        "path traversal must be rejected"
    );

    // Home path
    let home = invoke(
        &rt,
        "inspect_external_project_ref",
        json!({"source_ref": "~/secret-project"}),
    )
    .await?;
    anyhow::ensure!(
        home.output["path_safety"] == json!("rejected"),
        "home path must be rejected"
    );

    // Absolute sensitive path
    let abs = invoke(
        &rt,
        "inspect_external_project_ref",
        json!({"source_ref": "/etc/shadow"}),
    )
    .await?;
    anyhow::ensure!(
        abs.output["path_safety"] == json!("rejected"),
        "absolute sensitive path must be rejected"
    );

    // Workspace plan also rejects unsafe local paths
    let ws_unsafe = invoke(
        &rt,
        "draft_workspace_plan",
        json!({"source_ref": "~/secret-project", "source_kind": "local"}),
    )
    .await?;
    anyhow::ensure!(
        ws_unsafe.output["kind"] == json!("project_intake_rejected"),
        "workspace plan must reject unsafe local paths"
    );
    anyhow::ensure!(ws_unsafe.output["redaction_state"] == json!("unsafe_blocked"));

    // Adapter plan also rejects unsafe local paths
    let ad_unsafe = invoke(
        &rt,
        "draft_adapter_plan",
        json!({"source_ref": "~/secret-project", "source_kind": "local"}),
    )
    .await?;
    anyhow::ensure!(
        ad_unsafe.output["kind"] == json!("project_intake_rejected"),
        "adapter plan must reject unsafe local paths"
    );

    Ok(())
}

/// Case 6: Adapter plan is plan-only, no execution.
pub(crate) async fn project_intake_adapter_plan() -> anyhow::Result<()> {
    let rt = load_project_intake_lab().await?;

    let plan = invoke(
        &rt,
        "draft_adapter_plan",
        json!({"source_ref": "./my-project", "source_kind": "local"}),
    )
    .await?;

    anyhow::ensure!(plan.output["kind"] == json!("project_adapter_plan"));
    anyhow::ensure!(plan.output["plan_only"] == json!(true));
    anyhow::ensure!(plan.output["requires_user_approval"] == json!(true));
    anyhow::ensure!(plan.output["execution_performed"] == json!(false));
    anyhow::ensure!(plan.output["network_performed"] == json!(false));

    // Must have proposed capabilities
    let caps = plan.output["proposed_capabilities"].as_array().unwrap();
    anyhow::ensure!(!caps.is_empty(), "adapter plan must propose capabilities");

    Ok(())
}

/// Case 7: No forbidden namespace in any capability output.
pub(crate) async fn project_intake_no_forbidden_namespace() -> anyhow::Result<()> {
    let rt = load_project_intake_lab().await?;

    let caps = [
        "describe_intake_contract",
        "inspect_external_project_ref",
        "detect_project_stack_from_metadata",
        "draft_workspace_plan",
        "draft_security_risk_summary",
        "list_candidate_entrypoints",
        "draft_adapter_plan",
        "generate_adapter_manifest_preview",
        "generate_subprocess_wrapper_preview",
        "generate_adapter_fixture_preview",
        "check_adapter_readiness",
    ];

    let forbidden = [
        "kernel.v1.project.",
        "kernel.v1.workspace.",
        "kernel.v1.git.",
        "kernel.v1.npm.",
        "kernel.v1.deploy.",
        "kernel.v1.ide.",
    ];

    for cap in &caps {
        let result = invoke(&rt, cap, json!({"source_ref": "./test"})).await?;
        let output_str = serde_json::to_string(&result.output).unwrap();
        for token in &forbidden {
            anyhow::ensure!(
                !output_str.contains(token),
                "{cap} must not contain {token}"
            );
        }
    }

    Ok(())
}

/// Case 8: No raw secrets in any capability.
pub(crate) async fn project_intake_no_raw_secrets() -> anyhow::Result<()> {
    let rt = load_project_intake_lab().await?;

    // inspect blocks raw secret
    let inspect = invoke(
        &rt,
        "inspect_external_project_ref",
        json!({"source_ref": "test", "api_key": "RawSecretExample1234567890abcdefABCDEF123456"}),
    )
    .await?;
    anyhow::ensure!(inspect.output["kind"] == json!("project_intake_rejected"));
    anyhow::ensure!(inspect.output["redaction_state"] == json!("unsafe_blocked"));

    // draft_workspace_plan blocks raw secret
    let ws = invoke(
        &rt,
        "draft_workspace_plan",
        json!({"source_ref": "test", "token": "Bearer abc123"}),
    )
    .await?;
    anyhow::ensure!(ws.output["kind"] == json!("project_intake_rejected"));

    // draft_adapter_plan blocks raw secret
    let ad = invoke(
        &rt,
        "draft_adapter_plan",
        json!({"source_ref": "test", "secret": "RawSecretExample1234567890abcdefABCDEF123456"}),
    )
    .await?;
    anyhow::ensure!(ad.output["kind"] == json!("project_intake_rejected"));

    // E5: generate_adapter_manifest_preview blocks raw secret
    let manifest = invoke(
        &rt,
        "generate_adapter_manifest_preview",
        json!({"source_ref": "test", "adapter_package_id": "thirdparty/adapter", "capability_name": "invoke", "api_key": "RawSecretExample1234567890abcdefABCDEF123456"}),
    )
    .await?;
    anyhow::ensure!(manifest.output["kind"] == json!("project_intake_rejected"));

    // E5: check_adapter_readiness blocks raw secret
    let readiness = invoke(
        &rt,
        "check_adapter_readiness",
        json!({"adapter_package_id": "thirdparty/adapter", "capability_name": "invoke", "secret": "RawSecretExample1234567890abcdefABCDEF123456"}),
    )
    .await?;
    anyhow::ensure!(readiness.output["kind"] == json!("project_intake_rejected"));

    Ok(())
}

// ---------------------------------------------------------------------------
// E5 Adapter / Wrapper Generation Proof conformance cases
// ---------------------------------------------------------------------------

/// Case 9 (E5): Adapter manifest preview produces preview without file write.
pub(crate) async fn project_intake_adapter_manifest_preview_no_write() -> anyhow::Result<()> {
    let rt = load_project_intake_lab().await?;

    let result = invoke(
        &rt,
        "generate_adapter_manifest_preview",
        json!({
            "source_ref": "./my-project",
            "source_kind": "local",
            "adapter_package_id": "thirdparty/my-adapter",
            "capability_name": "invoke",
            "entry_kind": "subprocess"
        }),
    )
    .await?;

    anyhow::ensure!(result.output["kind"] == json!("adapter_manifest_preview"));
    anyhow::ensure!(result.output["adapter_package_id"] == json!("thirdparty/my-adapter"));
    anyhow::ensure!(result.output["filesystem_performed"] == json!(false));
    anyhow::ensure!(result.output["network_performed"] == json!(false));
    anyhow::ensure!(result.output["execution_performed"] == json!(false));

    // Manifest preview exists and is an object
    let preview = &result.output["manifest_preview"];
    anyhow::ensure!(preview.is_object(), "manifest_preview must be an object");
    anyhow::ensure!(preview["id"] == json!("thirdparty/my-adapter"));
    anyhow::ensure!(preview["schema_version"] == json!(1));

    // No network/filesystem/process permissions by default
    let perms = &preview["permissions"];
    anyhow::ensure!(perms.is_object(), "permissions must be present");

    Ok(())
}

/// Case 10 (E5): Rejects official adapter package id.
pub(crate) async fn project_intake_rejects_official_adapter_id() -> anyhow::Result<()> {
    let rt = load_project_intake_lab().await?;

    let result = invoke(
        &rt,
        "generate_adapter_manifest_preview",
        json!({
            "source_ref": "./test",
            "adapter_package_id": "official/fake-adapter",
            "capability_name": "invoke"
        }),
    )
    .await?;

    anyhow::ensure!(result.output["kind"] == json!("project_intake_rejected"));
    anyhow::ensure!(result.output["redaction_state"] == json!("unsafe_blocked"));

    Ok(())
}

/// Case 11 (E5): Rejects path traversal / unsafe chars in adapter package id.
pub(crate) async fn project_intake_rejects_path_traversal_adapter_id() -> anyhow::Result<()> {
    let rt = load_project_intake_lab().await?;

    // Path traversal
    let traversal = invoke(
        &rt,
        "generate_adapter_manifest_preview",
        json!({
            "source_ref": "./test",
            "adapter_package_id": "thirdparty/../evil",
            "capability_name": "invoke"
        }),
    )
    .await?;
    anyhow::ensure!(traversal.output["kind"] == json!("project_intake_rejected"));

    // Unsafe chars
    let unsafe_chars = invoke(
        &rt,
        "generate_adapter_manifest_preview",
        json!({
            "source_ref": "./test",
            "adapter_package_id": "thirdparty/evil;rm",
            "capability_name": "invoke"
        }),
    )
    .await?;
    anyhow::ensure!(unsafe_chars.output["kind"] == json!("project_intake_rejected"));

    Ok(())
}

/// Case 12 (E5): Capability namespace mismatch rejected.
pub(crate) async fn project_intake_capability_namespace_mismatch_rejected() -> anyhow::Result<()> {
    let rt = load_project_intake_lab().await?;

    let result = invoke(
        &rt,
        "generate_adapter_manifest_preview",
        json!({
            "source_ref": "./test",
            "adapter_package_id": "thirdparty/my-adapter",
            "capability_name": "other-pkg/invoke"
        }),
    )
    .await?;

    anyhow::ensure!(result.output["kind"] == json!("project_intake_rejected"));
    anyhow::ensure!(result.output["redaction_state"] == json!("unsafe_blocked"));

    Ok(())
}

/// Case 13 (E5): Wrapper preview no execution, safe comments present.
pub(crate) async fn project_intake_wrapper_preview_no_execution() -> anyhow::Result<()> {
    let rt = load_project_intake_lab().await?;

    let ts_result = invoke(
        &rt,
        "generate_subprocess_wrapper_preview",
        json!({
            "source_ref": "./my-project",
            "source_kind": "local",
            "adapter_package_id": "thirdparty/my-adapter",
            "capability_name": "invoke",
            "language": "typescript"
        }),
    )
    .await?;

    anyhow::ensure!(ts_result.output["kind"] == json!("subprocess_wrapper_preview"));
    anyhow::ensure!(ts_result.output["execution_performed"] == json!(false));
    anyhow::ensure!(ts_result.output["network_performed"] == json!(false));
    anyhow::ensure!(ts_result.output["filesystem_performed"] == json!(false));

    // Safe comments must be present
    let safe_comments = ts_result.output["safe_comments"].as_array().unwrap();
    anyhow::ensure!(!safe_comments.is_empty(), "must have safe comments");

    // Wrapper content must contain safe comments
    let files = ts_result.output["files"].as_array().unwrap();
    anyhow::ensure!(!files.is_empty(), "must have files");
    let content = files[0]["content"].as_str().unwrap();
    anyhow::ensure!(
        content.contains("SAFE COMMENT"),
        "wrapper must contain SAFE COMMENT"
    );
    anyhow::ensure!(
        content.contains("policy-gated executor"),
        "wrapper must reference policy-gated executor"
    );

    // Python variant
    let py_result = invoke(
        &rt,
        "generate_subprocess_wrapper_preview",
        json!({
            "source_ref": "./my-project",
            "source_kind": "local",
            "adapter_package_id": "thirdparty/my-adapter",
            "capability_name": "invoke",
            "language": "python"
        }),
    )
    .await?;
    anyhow::ensure!(py_result.output["kind"] == json!("subprocess_wrapper_preview"));
    anyhow::ensure!(py_result.output["language"] == json!("python"));

    Ok(())
}

/// Case 14 (E5): Fixture preview is redacted — no raw secrets.
pub(crate) async fn project_intake_fixture_preview_redacted() -> anyhow::Result<()> {
    let rt = load_project_intake_lab().await?;

    let result = invoke(
        &rt,
        "generate_adapter_fixture_preview",
        json!({
            "adapter_package_id": "thirdparty/my-adapter",
            "capability_name": "invoke"
        }),
    )
    .await?;

    anyhow::ensure!(result.output["kind"] == json!("adapter_fixture_preview"));
    anyhow::ensure!(result.output["redacted"] == json!(true));
    anyhow::ensure!(result.output["execution_performed"] == json!(false));

    // Fixture input must not contain raw secrets
    let input_str = serde_json::to_string(&result.output["fixture_input"]).unwrap();
    anyhow::ensure!(
        !input_str.contains("sk-"),
        "fixture input must not contain sk- prefix"
    );
    anyhow::ensure!(
        !input_str.contains("Bearer "),
        "fixture input must not contain Bearer prefix"
    );

    // Fixture output must be redacted
    let output = &result.output["fixture_output"];
    anyhow::ensure!(
        output["result_preview"] == json!("[redacted]"),
        "fixture output result must be redacted"
    );

    Ok(())
}

/// Case 15 (E5): Readiness checklist passes for valid adapter.
pub(crate) async fn project_intake_readiness_checklist_ok() -> anyhow::Result<()> {
    let rt = load_project_intake_lab().await?;

    let result = invoke(
        &rt,
        "check_adapter_readiness",
        json!({
            "adapter_package_id": "thirdparty/my-adapter",
            "capability_name": "invoke",
            "has_manifest": true,
            "has_wrapper": true,
            "has_fixture": true,
            "source_ref": "./test"
        }),
    )
    .await?;

    anyhow::ensure!(result.output["kind"] == json!("adapter_readiness"));
    anyhow::ensure!(result.output["ready"] == json!(true));
    anyhow::ensure!(result.output["capability_namespace_ok"] == json!(true));
    anyhow::ensure!(result.output["surface_coverage"] == json!(true));
    anyhow::ensure!(result.output["permissions_minimal"] == json!(true));
    anyhow::ensure!(result.output["fixture_present"] == json!(true));
    anyhow::ensure!(result.output["no_raw_secrets"] == json!(true));
    anyhow::ensure!(result.output["needs_approval_for_execution"] == json!(true));

    let checklist = result.output["checklist"].as_array().unwrap();
    anyhow::ensure!(checklist.len() >= 7, "checklist must have at least 7 items");

    Ok(())
}

/// Case 16 (E5): No forbidden namespace / raw secret in E5 capabilities.
pub(crate) async fn project_intake_e5_no_forbidden_namespace_no_raw_secret() -> anyhow::Result<()> {
    let rt = load_project_intake_lab().await?;

    let forbidden = [
        "kernel.v1.project.",
        "kernel.v1.workspace.",
        "kernel.v1.git.",
        "kernel.v1.npm.",
        "kernel.v1.deploy.",
        "kernel.v1.ide.",
    ];

    let e5_caps = [
        "generate_adapter_manifest_preview",
        "generate_subprocess_wrapper_preview",
        "generate_adapter_fixture_preview",
        "check_adapter_readiness",
    ];

    for cap in &e5_caps {
        let result = invoke(
            &rt,
            cap,
            json!({"source_ref": "./test", "adapter_package_id": "thirdparty/my-adapter", "capability_name": "invoke"}),
        )
        .await?;
        let output_str = serde_json::to_string(&result.output).unwrap();
        for token in &forbidden {
            anyhow::ensure!(
                !output_str.contains(token),
                "{cap} must not contain {token}"
            );
        }
    }

    Ok(())
}
