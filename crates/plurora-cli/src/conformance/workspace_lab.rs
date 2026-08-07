//! Conformance tests for `official/workspace-lab`
//! (External Project Operating Plane Alpha Phase E2 + E3).
//!
//! E2 covers:
//! 1. Contract shape (5→12 capabilities, 3 surfaces, ordinary package, deny-by-default, 10 action taxonomy)
//! 2. Action taxonomy deny-by-default (all dangerous actions denied, no execution)
//! 3. Policy mismatch fail-closed (policy "allow" for approval-required action → denied)
//! 4. Raw secret blocked (raw API key / Bearer token in input → rejected)
//! 5. Audit redacted (no raw env/logs/commands/secrets in audit summary)
//! 6. No forbidden namespace (no platform.project/workspace/git/npm/deploy/ide in any output)
//! 7. No execution performed (executor_invoked=false, execution_performed=false always)
//!
//! E3 covers:
//! 8. Fixture workspace creation no execution (managed_workspace_kind=fixture, no real creation)
//! 9. Inspect/read metadata no filesystem (no disk reads)
//! 10. Run plan requires approval (all run/install/test steps require_approval, executor_invoked=false)
//! 11. Fixture process result redacted/no real process (real_process_spawned=false, raw fields stripped)
//! 12. Entrypoint discovery deterministic (stack-based candidates, no filesystem scan)
//! 13. Patch draft proposal only/no write (plan_only=true, file_write_performed=false, unsafe path rejected)
//! 14. Raw secret blocked/no forbidden namespace across E3 capabilities

use std::path::PathBuf;

use plurora_runtime::CapabilityInvocationRequest;
use serde_json::json;

use super::fixtures::*;
use crate::commands::manifest;

const PACKAGE_ID: &str = "official/workspace-lab";

async fn load_workspace_lab(
) -> anyhow::Result<plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/official/workspace-lab/manifest.yaml",
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

/// Case 1: Contract shape — 12 capabilities, 3 surfaces, ordinary package,
/// deny-by-default, 10 action taxonomy, managed workspace defaults,
/// no forbidden namespace, no execution.
pub(crate) async fn workspace_lab_contract() -> anyhow::Result<()> {
    let rt = load_workspace_lab().await?;

    let contract = invoke(&rt, "describe_workspace_contract", json!({})).await?;

    anyhow::ensure!(
        contract.output["kind"] == json!("workspace_lab_contract"),
        "describe_workspace_contract must return workspace_lab_contract kind"
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

    // 12 capabilities (5 original + 7 E3)
    anyhow::ensure!(
        contract.output["capabilities"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
            == 12,
        "describe_workspace_contract must list 12 capabilities (5 E2 + 7 E3)"
    );

    // 10 action taxonomy entries
    anyhow::ensure!(
        contract.output["action_taxonomy"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
            == 10,
        "describe_workspace_contract must list 10 action taxonomy entries"
    );

    // Deny-by-default
    anyhow::ensure!(
        contract.output["policy_defaults"]["default_decision"] == json!("denied_by_default")
    );
    anyhow::ensure!(contract.output["policy_defaults"]["executor_invoked"] == json!(false));
    anyhow::ensure!(contract.output["policy_defaults"]["execution_performed"] == json!(false));
    anyhow::ensure!(contract.output["policy_defaults"]["proposal_required"] == json!(true));

    // Managed workspace defaults (E3)
    anyhow::ensure!(
        contract.output["managed_workspace_defaults"]["managed_workspace_kind"] == json!("fixture")
    );
    anyhow::ensure!(
        contract.output["managed_workspace_defaults"]["workspace_created_in_host"] == json!(false)
    );
    anyhow::ensure!(
        contract.output["managed_workspace_defaults"]["execution_performed"] == json!(false)
    );
    anyhow::ensure!(
        contract.output["managed_workspace_defaults"]["filesystem_performed"] == json!(false)
    );
    anyhow::ensure!(
        contract.output["managed_workspace_defaults"]["network_performed"] == json!(false)
    );

    // No execution
    anyhow::ensure!(contract.output["execution_performed"] == json!(false));
    anyhow::ensure!(contract.output["network_performed"] == json!(false));
    anyhow::ensure!(contract.output["inference_performed"] == json!(false));
    anyhow::ensure!(contract.output["filesystem_performed"] == json!(false));

    // No forbidden namespace
    let output_str = serde_json::to_string(&contract.output).unwrap();
    for token in &[
        "platform.project.",
        "platform.workspace.",
        "platform.git.",
        "platform.npm.",
        "platform.deploy.",
        "platform.ide.",
    ] {
        anyhow::ensure!(
            !output_str.contains(token),
            "describe_workspace_contract must not contain {}",
            token
        );
    }

    Ok(())
}

/// Case 2: Action taxonomy deny-by-default — all dangerous actions denied,
/// no execution, proposal_required=true, approval_token not honored.
pub(crate) async fn workspace_lab_action_deny_default() -> anyhow::Result<()> {
    let rt = load_workspace_lab().await?;

    // Test dangerous actions: clone_project, install_dependencies, run_command, deploy_plan
    for action in &[
        "clone_project",
        "install_dependencies",
        "run_command",
        "deploy_plan",
    ] {
        let result = invoke(
            &rt,
            "request_workspace_action",
            json!({"action": action, "workspace_ref": "ws-test"}),
        )
        .await?;
        anyhow::ensure!(
            result.output["policy_decision"] == json!("denied_by_default"),
            "action {} must be denied_by_default",
            action
        );
        anyhow::ensure!(
            result.output["executor_invoked"] == json!(false),
            "action {} must have executor_invoked=false",
            action
        );
        anyhow::ensure!(
            result.output["execution_performed"] == json!(false),
            "action {} must have execution_performed=false",
            action
        );
        anyhow::ensure!(
            result.output["proposal_required"] == json!(true),
            "action {} must have proposal_required=true",
            action
        );
    }

    // Approval token not honored
    let with_token = invoke(
        &rt,
        "request_workspace_action",
        json!({"action": "clone_project", "workspace_ref": "ws-test", "approval_token": "pretend-token"}),
    )
    .await?;
    anyhow::ensure!(
        with_token.output["policy_decision"] == json!("denied_by_default"),
        "approval_token must not bypass deny-by-default"
    );
    anyhow::ensure!(
        with_token.output["approval_token_rejected"] == json!(true),
        "approval_token must be rejected"
    );
    anyhow::ensure!(
        with_token.output["approval_token_honored"] == json!(false),
        "approval_token must not be honored"
    );

    Ok(())
}

/// Case 3: Policy/action mismatch fail-closed — policy "allow" for
/// approval-required actions → denied; unknown policy → denied.
pub(crate) async fn workspace_lab_policy_mismatch() -> anyhow::Result<()> {
    let rt = load_workspace_lab().await?;

    // policy "allow" with approval-required action → fail closed
    let mismatch = invoke(
        &rt,
        "request_workspace_action",
        json!({"action": "install_dependencies", "workspace_ref": "ws-test", "policy": "allow"}),
    )
    .await?;
    anyhow::ensure!(
        mismatch.output["kind"] == json!("workspace_action_rejected"),
        "policy/action mismatch must produce workspace_action_rejected"
    );
    anyhow::ensure!(
        mismatch.output["policy_decision"] == json!("denied"),
        "policy/action mismatch must produce policy_decision=denied"
    );
    anyhow::ensure!(
        mismatch.output["executor_invoked"] == json!(false),
        "policy mismatch must not invoke executor"
    );

    // unknown policy → fail closed
    let unknown = invoke(
        &rt,
        "request_workspace_action",
        json!({"action": "run_command", "workspace_ref": "ws-test", "policy": "auto_approve"}),
    )
    .await?;
    anyhow::ensure!(
        unknown.output["policy_decision"] == json!("denied"),
        "unknown policy must be denied"
    );

    // unknown action → fail closed
    let bad_action = invoke(
        &rt,
        "request_workspace_action",
        json!({"action": "rm_rf", "workspace_ref": "ws-test"}),
    )
    .await?;
    anyhow::ensure!(
        bad_action.output["kind"] == json!("workspace_action_rejected"),
        "unknown action must produce workspace_action_rejected"
    );
    anyhow::ensure!(
        bad_action.output["policy_decision"] == json!("denied"),
        "unknown action must produce policy_decision=denied"
    );

    Ok(())
}

/// Case 4: Raw secret blocked — raw API key / Bearer token in input → rejected.
pub(crate) async fn workspace_lab_raw_secret_blocked() -> anyhow::Result<()> {
    let rt = load_workspace_lab().await?;

    // request_workspace_action blocks raw secret
    let with_secret = invoke(
        &rt,
        "request_workspace_action",
        json!({"action": "clone_project", "workspace_ref": "ws-test", "api_key": "RawSecretExample1234567890abcdefABCDEF123456"}),
    )
    .await?;
    anyhow::ensure!(
        with_secret.output["kind"] == json!("workspace_lab_rejected"),
        "raw secret must produce workspace_lab_rejected"
    );
    anyhow::ensure!(
        with_secret.output["redaction_state"] == json!("unsafe_blocked"),
        "raw secret must produce redaction_state=unsafe_blocked"
    );

    // draft_workspace_creation blocks raw secret
    let draft_secret = invoke(
        &rt,
        "draft_workspace_creation",
        json!({"workspace_ref": "ws-test", "token": "Bearer abc123"}),
    )
    .await?;
    anyhow::ensure!(
        draft_secret.output["kind"] == json!("workspace_lab_rejected"),
        "raw secret in draft must produce workspace_lab_rejected"
    );

    // summarize_workspace_audit blocks raw secret in top-level input
    let audit_secret = invoke(
        &rt,
        "summarize_workspace_audit",
        json!({"workspace_ref": "ws-test", "secret": "RawSecretExample1234567890abcdefABCDEF123456"}),
    )
    .await?;
    anyhow::ensure!(
        audit_secret.output["kind"] == json!("workspace_lab_rejected"),
        "raw secret in audit must produce workspace_lab_rejected"
    );

    // E3: create_fixture_workspace blocks raw secret
    let fixture_secret = invoke(
        &rt,
        "create_fixture_workspace",
        json!({"workspace_ref": "ws-test", "api_key": "RawSecretExample1234567890abcdefABCDEF123456"}),
    )
    .await?;
    anyhow::ensure!(
        fixture_secret.output["kind"] == json!("workspace_lab_rejected"),
        "raw secret in create_fixture_workspace must produce workspace_lab_rejected"
    );

    // E3: draft_workspace_patch blocks raw secret
    let patch_secret = invoke(
        &rt,
        "draft_workspace_patch",
        json!({"workspace_ref": "ws-test", "target_files": ["src/main.rs"], "api_key": "RawSecretExample1234567890abcdefABCDEF123456"}),
    )
    .await?;
    anyhow::ensure!(
        patch_secret.output["kind"] == json!("workspace_lab_rejected"),
        "raw secret in draft_workspace_patch must produce workspace_lab_rejected"
    );

    Ok(())
}

/// Case 5: Audit redacted — no raw env/logs/commands/secrets in audit summary.
pub(crate) async fn workspace_lab_audit_redacted() -> anyhow::Result<()> {
    let rt = load_workspace_lab().await?;

    let audit = invoke(
        &rt,
        "summarize_workspace_audit",
        json!({
            "workspace_ref": "ws-audit",
            "action_history": [
                {"action": "clone_project", "policy_decision": "denied_by_default", "executor_invoked": false, "execution_performed": false},
                {"action": "read_metadata", "policy_decision": "approved", "executor_invoked": false, "execution_performed": false},
                {"action": "run_command", "policy_decision": "pending", "executor_invoked": false, "execution_performed": false, "raw_command": "rm -rf /", "raw_env": "SECRET=abc", "raw_log": "password=xyz"},
            ]
        }),
    )
    .await?;

    anyhow::ensure!(
        audit.output["kind"] == json!("workspace_audit_summary"),
        "must produce workspace_audit_summary"
    );
    anyhow::ensure!(
        audit.output["redaction_applied"] == json!(true),
        "must have redaction_applied=true"
    );

    // Verify no raw content leaked
    let output_str = serde_json::to_string(&audit.output).unwrap();
    anyhow::ensure!(
        !output_str.contains("rm -rf"),
        "audit must not contain raw commands"
    );
    anyhow::ensure!(
        !output_str.contains("raw_command"),
        "audit must not contain raw_command field"
    );
    anyhow::ensure!(
        !output_str.contains("raw_env"),
        "audit must not contain raw_env field"
    );
    anyhow::ensure!(
        !output_str.contains("raw_log"),
        "audit must not contain raw_log field"
    );
    anyhow::ensure!(
        !output_str.contains("SECRET=abc"),
        "audit must not contain raw env values"
    );
    anyhow::ensure!(
        !output_str.contains("password=xyz"),
        "audit must not contain raw log values"
    );

    // Verify counts
    anyhow::ensure!(
        audit.output["total_actions"] == json!(3),
        "must count 3 total actions"
    );

    Ok(())
}

/// Case 6: No forbidden namespace in any capability output.
pub(crate) async fn workspace_lab_no_forbidden_namespace() -> anyhow::Result<()> {
    let rt = load_workspace_lab().await?;

    let caps = [
        "describe_workspace_contract",
        "draft_workspace_creation",
        "explain_required_permissions",
        "request_workspace_action",
        "summarize_workspace_audit",
        "create_fixture_workspace",
        "inspect_workspace",
        "read_workspace_metadata",
        "plan_workspace_run",
        "record_fixture_process_result",
        "discover_workspace_entrypoints",
        "draft_workspace_patch",
    ];

    let forbidden = [
        "platform.project.",
        "platform.workspace.",
        "platform.git.",
        "platform.npm.",
        "platform.deploy.",
        "platform.ide.",
    ];

    for cap in &caps {
        let result = invoke(
            &rt,
            cap,
            json!({"workspace_ref": "ws-test", "action": "clone_project"}),
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

/// Case 7: No execution performed across all capabilities.
pub(crate) async fn workspace_lab_no_execution() -> anyhow::Result<()> {
    let rt = load_workspace_lab().await?;

    let caps = [
        "describe_workspace_contract",
        "draft_workspace_creation",
        "explain_required_permissions",
        "request_workspace_action",
        "summarize_workspace_audit",
        "create_fixture_workspace",
        "inspect_workspace",
        "read_workspace_metadata",
        "plan_workspace_run",
        "record_fixture_process_result",
        "discover_workspace_entrypoints",
        "draft_workspace_patch",
    ];

    for cap in &caps {
        let result = invoke(
            &rt,
            cap,
            json!({"workspace_ref": "ws-test", "action": "run_command"}),
        )
        .await?;
        anyhow::ensure!(
            result.output["execution_performed"] == json!(false),
            "{cap} must have execution_performed=false"
        );
        anyhow::ensure!(
            result.output["network_performed"] == json!(false),
            "{cap} must have network_performed=false"
        );
        anyhow::ensure!(
            result.output["inference_performed"] == json!(false),
            "{cap} must have inference_performed=false"
        );
        anyhow::ensure!(
            result.output["filesystem_performed"] == json!(false),
            "{cap} must have filesystem_performed=false"
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// E3 conformance cases
// ---------------------------------------------------------------------------

/// Case 8: Fixture workspace creation no execution.
/// create_fixture_workspace produces fixture descriptor with
/// managed_workspace_kind="fixture", execution_performed=false,
/// filesystem_performed=false, network_performed=false,
/// workspace_created_in_host=false.
pub(crate) async fn workspace_lab_fixture_workspace_creation() -> anyhow::Result<()> {
    let rt = load_workspace_lab().await?;

    let result = invoke(
        &rt,
        "create_fixture_workspace",
        json!({
            "workspace_ref": "ws-e3-1",
            "source_ref": "https://github.com/example/project.git",
            "stack_hint": "node",
            "metadata": {"package_json": true, "version": "1.0.0"}
        }),
    )
    .await?;

    anyhow::ensure!(
        result.output["kind"] == json!("fixture_workspace_descriptor"),
        "must produce fixture_workspace_descriptor"
    );
    anyhow::ensure!(
        result.output["managed_workspace_kind"] == json!("fixture"),
        "managed_workspace_kind must be fixture"
    );
    anyhow::ensure!(
        result.output["workspace_ref"] == json!("ws-e3-1"),
        "workspace_ref must match input"
    );
    anyhow::ensure!(
        result.output["source_kind"] == json!("git"),
        "source_kind must be git"
    );
    anyhow::ensure!(
        result.output["detected_stack"] == json!("node"),
        "detected_stack must be node"
    );
    anyhow::ensure!(
        result.output["workspace_created_in_host"] == json!(false),
        "workspace_created_in_host must be false"
    );
    anyhow::ensure!(
        result.output["execution_performed"] == json!(false),
        "execution_performed must be false"
    );
    anyhow::ensure!(
        result.output["filesystem_performed"] == json!(false),
        "filesystem_performed must be false"
    );
    anyhow::ensure!(
        result.output["network_performed"] == json!(false),
        "network_performed must be false"
    );
    anyhow::ensure!(
        result.output["inference_performed"] == json!(false),
        "inference_performed must be false"
    );

    // Real creation requires approval/policy/executor
    let requires = result.output["real_creation_requires"].as_array().unwrap();
    anyhow::ensure!(
        requires.iter().any(|v| v == "approval"),
        "real_creation_requires must include approval"
    );
    anyhow::ensure!(
        requires.iter().any(|v| v == "policy"),
        "real_creation_requires must include policy"
    );
    anyhow::ensure!(
        requires.iter().any(|v| v == "executor"),
        "real_creation_requires must include executor"
    );

    // Process state must show no execution
    anyhow::ensure!(
        result.output["process_state"]["executor_invoked"] == json!(false),
        "process_state.executor_invoked must be false"
    );
    anyhow::ensure!(
        result.output["process_state"]["execution_performed"] == json!(false),
        "process_state.execution_performed must be false"
    );

    Ok(())
}

/// Case 9: Inspect/read metadata no filesystem.
/// inspect_workspace and read_workspace_metadata never read disk.
pub(crate) async fn workspace_lab_inspect_read_no_filesystem() -> anyhow::Result<()> {
    let rt = load_workspace_lab().await?;

    // inspect_workspace — no filesystem
    let inspect = invoke(
        &rt,
        "inspect_workspace",
        json!({"workspace_ref": "ws-inspect"}),
    )
    .await?;
    anyhow::ensure!(
        inspect.output["kind"] == json!("workspace_inspection"),
        "must produce workspace_inspection"
    );
    anyhow::ensure!(
        inspect.output["filesystem_performed"] == json!(false),
        "inspect must have filesystem_performed=false"
    );
    anyhow::ensure!(
        inspect.output["execution_performed"] == json!(false),
        "inspect must have execution_performed=false"
    );

    // read_workspace_metadata — no filesystem
    let read_meta = invoke(
        &rt,
        "read_workspace_metadata",
        json!({"workspace_ref": "ws-meta", "metadata": {"stack": "rust", "version": "0.1.0"}}),
    )
    .await?;
    anyhow::ensure!(
        read_meta.output["kind"] == json!("workspace_metadata"),
        "must produce workspace_metadata"
    );
    anyhow::ensure!(
        read_meta.output["metadata"]["stack"] == json!("rust"),
        "metadata must echo input"
    );
    anyhow::ensure!(
        read_meta.output["filesystem_performed"] == json!(false),
        "read_metadata must have filesystem_performed=false"
    );
    anyhow::ensure!(
        read_meta.output["execution_performed"] == json!(false),
        "read_metadata must have execution_performed=false"
    );

    Ok(())
}

/// Case 10: Run plan requires approval.
/// plan_workspace_run generates plan where all run/install/test steps
/// have requires_approval=true, executor_invoked=false.
pub(crate) async fn workspace_lab_run_plan_requires_approval() -> anyhow::Result<()> {
    let rt = load_workspace_lab().await?;

    let plan = invoke(
        &rt,
        "plan_workspace_run",
        json!({
            "workspace_ref": "ws-run-plan",
            "scripts": [
                {"name": "install", "executes_code": true},
                {"name": "build", "executes_code": true},
                {"name": "test", "executes_code": true}
            ],
            "entrypoints": [
                {"name": "start", "executes_code": true}
            ]
        }),
    )
    .await?;

    anyhow::ensure!(
        plan.output["kind"] == json!("workspace_run_plan"),
        "must produce workspace_run_plan"
    );
    anyhow::ensure!(plan.output["plan_only"] == json!(true), "must be plan_only");
    anyhow::ensure!(
        plan.output["requires_user_approval"] == json!(true),
        "must require user approval"
    );
    anyhow::ensure!(
        plan.output["executor_invoked"] == json!(false),
        "executor_invoked must be false"
    );
    anyhow::ensure!(
        plan.output["execution_performed"] == json!(false),
        "execution_performed must be false"
    );

    // All run steps must require approval and executor_invoked=false
    let steps = plan.output["run_steps"].as_array().unwrap();
    anyhow::ensure!(
        steps.len() == 4,
        "must have 4 run steps (3 scripts + 1 entrypoint)"
    );
    for step in steps {
        anyhow::ensure!(
            step["requires_approval"] == json!(true),
            "all steps must require_approval=true"
        );
        anyhow::ensure!(
            step["executor_invoked"] == json!(false),
            "all steps must have executor_invoked=false"
        );
    }

    Ok(())
}

/// Case 11: Fixture process result redacted/no real process.
/// record_fixture_process_result records caller-provided shape;
/// real_process_spawned=false; raw fields stripped.
pub(crate) async fn workspace_lab_fixture_process_result_redacted() -> anyhow::Result<()> {
    let rt = load_workspace_lab().await?;

    let result = invoke(
        &rt,
        "record_fixture_process_result",
        json!({
            "workspace_ref": "ws-proc",
            "process_ref": "proc-1",
            "exit_code": 0,
            "duration_ms": 1500,
            "status": "success",
            "stdout_preview": "Hello, world!",
            "stderr_preview": "",
            "raw_stdout": "should be stripped",
            "raw_stderr": "should be stripped",
            "raw_command": "should be stripped"
        }),
    )
    .await?;

    anyhow::ensure!(
        result.output["kind"] == json!("fixture_process_result_record"),
        "must produce fixture_process_result_record"
    );
    anyhow::ensure!(
        result.output["real_process_spawned"] == json!(false),
        "real_process_spawned must be false"
    );
    anyhow::ensure!(
        result.output["execution_performed"] == json!(false),
        "execution_performed must be false"
    );
    anyhow::ensure!(
        result.output["exit_code"] == json!(0),
        "exit_code must be 0"
    );
    anyhow::ensure!(
        result.output["duration_ms"] == json!(1500),
        "duration_ms must be 1500"
    );
    anyhow::ensure!(
        result.output["status"] == json!("success"),
        "status must be success"
    );
    // stdout_preview_length based on input, not raw content
    anyhow::ensure!(
        result.output["stdout_preview_length"] == json!(13),
        "stdout_preview_length must be 13"
    );

    // Verify raw fields are stripped
    let output_str = serde_json::to_string(&result.output).unwrap();
    anyhow::ensure!(
        !output_str.contains("raw_stdout"),
        "must not contain raw_stdout"
    );
    anyhow::ensure!(
        !output_str.contains("raw_stderr"),
        "must not contain raw_stderr"
    );
    anyhow::ensure!(
        !output_str.contains("raw_command"),
        "must not contain raw_command"
    );
    anyhow::ensure!(
        !output_str.contains("should be stripped"),
        "must not contain raw content"
    );

    Ok(())
}

/// Case 12: Entrypoint discovery deterministic.
/// discover_workspace_entrypoints generates candidates from stack_hint
/// and metadata; no filesystem scan; all candidates require_approval.
pub(crate) async fn workspace_lab_entrypoint_discovery() -> anyhow::Result<()> {
    let rt = load_workspace_lab().await?;

    // Test with node stack
    let node_result = invoke(
        &rt,
        "discover_workspace_entrypoints",
        json!({
            "workspace_ref": "ws-entry",
            "stack_hint": "node",
            "scripts": [{"name": "custom-script", "executes_code": true}]
        }),
    )
    .await?;

    anyhow::ensure!(
        node_result.output["kind"] == json!("workspace_entrypoint_candidates"),
        "must produce workspace_entrypoint_candidates"
    );
    anyhow::ensure!(
        node_result.output["detected_stack"] == json!("node"),
        "detected_stack must be node"
    );
    anyhow::ensure!(
        node_result.output["execution_performed"] == json!(false),
        "must have execution_performed=false"
    );
    anyhow::ensure!(
        node_result.output["filesystem_performed"] == json!(false),
        "must have filesystem_performed=false"
    );

    let candidates = node_result.output["candidates"].as_array().unwrap();
    anyhow::ensure!(
        !candidates.is_empty(),
        "node stack must produce entrypoint candidates"
    );
    // All candidates require approval
    for candidate in candidates {
        anyhow::ensure!(
            candidate["requires_approval"] == json!(true),
            "all entrypoint candidates must require_approval=true"
        );
    }
    // Should include caller-provided script
    let has_custom = candidates
        .iter()
        .any(|c| c["name"] == json!("custom-script"));
    anyhow::ensure!(has_custom, "must include caller-provided script entrypoint");

    // Test with rust stack — deterministic output
    let rust_result = invoke(
        &rt,
        "discover_workspace_entrypoints",
        json!({"workspace_ref": "ws-entry", "stack_hint": "rust"}),
    )
    .await?;
    let rust_candidates = rust_result.output["candidates"].as_array().unwrap();
    let has_cargo = rust_candidates
        .iter()
        .any(|c| c["name"].as_str().unwrap_or("").contains("cargo"));
    anyhow::ensure!(
        has_cargo,
        "rust stack must produce cargo entrypoint candidates"
    );

    Ok(())
}

/// Case 13: Patch draft proposal only/no write.
/// draft_workspace_patch produces plan_only=true, file_write_performed=false;
/// unsafe local paths are rejected; no real file writes.
pub(crate) async fn workspace_lab_patch_draft_proposal() -> anyhow::Result<()> {
    let rt = load_workspace_lab().await?;

    let patch = invoke(
        &rt,
        "draft_workspace_patch",
        json!({
            "workspace_ref": "ws-patch",
            "target_files": ["src/main.rs", "Cargo.toml", "../../../etc/passwd"],
            "description": "fix a typo",
            "patch_kind": "modification"
        }),
    )
    .await?;

    anyhow::ensure!(
        patch.output["kind"] == json!("workspace_patch_proposal"),
        "must produce workspace_patch_proposal"
    );
    anyhow::ensure!(
        patch.output["plan_only"] == json!(true),
        "must be plan_only"
    );
    anyhow::ensure!(
        patch.output["requires_user_approval"] == json!(true),
        "must require user approval"
    );
    anyhow::ensure!(
        patch.output["executor_invoked"] == json!(false),
        "executor_invoked must be false"
    );
    anyhow::ensure!(
        patch.output["file_write_performed"] == json!(false),
        "file_write_performed must be false"
    );
    anyhow::ensure!(
        patch.output["filesystem_performed"] == json!(false),
        "filesystem_performed must be false"
    );
    anyhow::ensure!(
        patch.output["execution_performed"] == json!(false),
        "execution_performed must be false"
    );

    // Validated target files (only safe paths)
    let target_files = patch.output["target_files"].as_array().unwrap();
    anyhow::ensure!(
        target_files.len() == 2,
        "must have 2 validated target files (unsafe path excluded)"
    );

    // Rejected files (unsafe path)
    let rejected = patch.output["rejected_files"].as_array().unwrap();
    anyhow::ensure!(
        rejected.len() == 1,
        "must have 1 rejected file (unsafe path)"
    );
    anyhow::ensure!(
        rejected[0]["rejection_reason"] == json!("unsafe_local_path"),
        "rejected file must have unsafe_local_path reason"
    );

    Ok(())
}

/// Case 14: Raw secret blocked and no forbidden namespace across all E3 capabilities.
pub(crate) async fn workspace_lab_e3_raw_secret_no_forbidden_namespace() -> anyhow::Result<()> {
    let rt = load_workspace_lab().await?;

    let e3_caps = [
        "create_fixture_workspace",
        "inspect_workspace",
        "read_workspace_metadata",
        "plan_workspace_run",
        "record_fixture_process_result",
        "discover_workspace_entrypoints",
        "draft_workspace_patch",
    ];

    let forbidden = [
        "platform.project.",
        "platform.workspace.",
        "platform.git.",
        "platform.npm.",
        "platform.deploy.",
        "platform.ide.",
    ];

    // No forbidden namespace in any E3 output
    for cap in &e3_caps {
        let result = invoke(
            &rt,
            cap,
            json!({"workspace_ref": "ws-ns-test", "action": "read_metadata"}),
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

    // Raw secret blocked in create_fixture_workspace
    let fixture_secret = invoke(
        &rt,
        "create_fixture_workspace",
        json!({"workspace_ref": "ws-secret", "api_key": "RawSecretExample1234567890abcdefABCDEF123456"}),
    )
    .await?;
    anyhow::ensure!(
        fixture_secret.output["kind"] == json!("workspace_lab_rejected"),
        "raw secret in create_fixture_workspace must be blocked"
    );

    // Raw secret blocked in record_fixture_process_result
    let proc_secret = invoke(
        &rt,
        "record_fixture_process_result",
        json!({"workspace_ref": "ws-proc", "secret": "RawSecretExample1234567890abcdefABCDEF123456"}),
    )
    .await?;
    anyhow::ensure!(
        proc_secret.output["kind"] == json!("workspace_lab_rejected"),
        "raw secret in record_fixture_process_result must be blocked"
    );

    // Raw secret blocked in discover_workspace_entrypoints
    let entry_secret = invoke(
        &rt,
        "discover_workspace_entrypoints",
        json!({"workspace_ref": "ws-entry", "api_key": "RawSecretExample1234567890abcdefABCDEF123456"}),
    )
    .await?;
    anyhow::ensure!(
        entry_secret.output["kind"] == json!("workspace_lab_rejected"),
        "raw secret in discover_workspace_entrypoints must be blocked"
    );

    Ok(())
}
