//! Conformance tests for `official/memory-lab` (Experience Beta 4).
//!
//! Covers:
//! 1. Memory contract shape (surfaces, capabilities, output shapes, ordinary package)
//! 2. Record memory produces memory_record with content_address
//! 3. Retrieve memory returns keyword matches, branch-aware, no inference
//! 4. Trace retrieval produces deterministic retrieval trace
//! 5. Draft memory update is proposal-only, no direct state mutation
//! 6. Apply memory correction is proposal-gated, requires approval
//! 7. Draft forget/redaction produces redaction plan, not deletion
//! 8. Branch-aware memory view filters by branch
//! 9. No forbidden namespace (kernel.v1.memory.* etc.)
//! 10. No raw secrets in any capability

use std::path::PathBuf;

use plurora_runtime::CapabilityInvocationRequest;
use serde_json::json;

use super::fixtures::*;
use crate::commands::manifest;

const PACKAGE_ID: &str = "official/memory-lab";

async fn load_memory_lab(
) -> anyhow::Result<plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from("packages/official/memory-lab/manifest.yaml"))
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

/// Case 1: Memory contract — 9 capabilities, 3 surfaces, ordinary package,
/// no forbidden namespace, output shapes defined.
pub(crate) async fn memory_lab_contract() -> anyhow::Result<()> {
    let rt = load_memory_lab().await?;

    let contract = invoke(&rt, "describe_memory_contract", json!({})).await?;

    anyhow::ensure!(
        contract.output["kind"] == json!("memory_lab_contract"),
        "describe_memory_contract must return memory_lab_contract kind"
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
        "describe_memory_contract must list 9 capabilities"
    );

    // Output shapes defined
    anyhow::ensure!(
        contract.output["output_shapes"].is_object(),
        "must have output_shapes"
    );
    anyhow::ensure!(
        contract.output["output_shapes"]["memory_record"].is_array(),
        "output_shapes must have memory_record"
    );
    anyhow::ensure!(
        contract.output["output_shapes"]["retrieval_trace"].is_array(),
        "output_shapes must have retrieval_trace"
    );
    anyhow::ensure!(
        contract.output["output_shapes"]["redaction_plan"].is_array(),
        "output_shapes must have redaction_plan"
    );

    // No inference / no network
    anyhow::ensure!(contract.output["inference_performed"] == json!(false));
    anyhow::ensure!(contract.output["network_performed"] == json!(false));

    Ok(())
}

/// Case 2: Record memory — produces memory_record with content_address.
pub(crate) async fn memory_lab_record_memory() -> anyhow::Result<()> {
    let rt = load_memory_lab().await?;

    let record = invoke(
        &rt,
        "record_memory",
        json!({
            "kind": "fact",
            "key": "player_name",
            "content": "The player's name is Alice",
            "branch_ref": "branch:main",
        }),
    )
    .await?;

    anyhow::ensure!(record.output["kind"] == json!("memory_record"));
    anyhow::ensure!(record.output["record_kind"] == json!("fact"));
    anyhow::ensure!(record.output["key"] == json!("player_name"));
    anyhow::ensure!(
        record.output["content_address"].is_string(),
        "must have content_address"
    );
    anyhow::ensure!(record.output["branch_ref"] == json!("branch:main"));
    anyhow::ensure!(record.output["inference_performed"] == json!(false));
    anyhow::ensure!(record.output["network_performed"] == json!(false));

    Ok(())
}

/// Case 3: Retrieve memory — keyword match, branch-aware, no inference.
pub(crate) async fn memory_lab_retrieve_memory() -> anyhow::Result<()> {
    let rt = load_memory_lab().await?;

    let result = invoke(
        &rt,
        "retrieve_memory",
        json!({
            "query": "dragon",
            "branch_ref": "branch:main",
            "records": [
                {"key": "dragon_type", "content": "fire dragon", "branch_ref": "branch:main"},
                {"key": "elf_name", "content": "Legolas", "branch_ref": "branch:main"},
                {"key": "dragon_habitat", "content": "volcano", "branch_ref": "branch:feature1"},
            ]
        }),
    )
    .await?;

    anyhow::ensure!(result.output["kind"] == json!("retrieval_result"));
    anyhow::ensure!(result.output["algorithm"] == json!("deterministic_keyword_contains"));
    // Only "dragon_type" should match (same branch + keyword)
    anyhow::ensure!(result.output["match_count"] == json!(1));
    anyhow::ensure!(result.output["inference_performed"] == json!(false));

    Ok(())
}

/// Case 4: Trace retrieval — deterministic retrieval trace.
pub(crate) async fn memory_lab_trace_retrieval() -> anyhow::Result<()> {
    let rt = load_memory_lab().await?;

    let trace = invoke(
        &rt,
        "trace_retrieval",
        json!({
            "query": "dragon",
            "algorithm": "deterministic_keyword_contains",
        }),
    )
    .await?;

    anyhow::ensure!(trace.output["kind"] == json!("retrieval_trace"));
    anyhow::ensure!(trace.output["algorithm"] == json!("deterministic_keyword_contains"));
    anyhow::ensure!(trace.output["trace"].is_array(), "must have trace array");
    anyhow::ensure!(
        trace.output["trace"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
            > 0,
        "trace must have steps"
    );
    anyhow::ensure!(trace.output["inference_performed"] == json!(false));

    Ok(())
}

/// Case 5: Draft memory update is proposal-only — no direct state mutation.
pub(crate) async fn memory_lab_draft_update() -> anyhow::Result<()> {
    let rt = load_memory_lab().await?;

    let draft = invoke(
        &rt,
        "draft_memory_update",
        json!({
            "update_kind": "add_record",
            "key": "world_setting",
            "proposed_content": {"name": "Dark Forest", "type": "region"},
        }),
    )
    .await?;

    anyhow::ensure!(draft.output["kind"] == json!("memory_update_draft"));
    anyhow::ensure!(draft.output["requires_user_approval"] == json!(true));
    anyhow::ensure!(draft.output["plan_only"] == json!(true));
    anyhow::ensure!(
        draft.output["content_address"].is_string(),
        "draft must have content_address"
    );
    anyhow::ensure!(draft.output["inference_performed"] == json!(false));

    Ok(())
}

/// Case 6: Apply memory correction is proposal-gated.
pub(crate) async fn memory_lab_correction() -> anyhow::Result<()> {
    let rt = load_memory_lab().await?;

    let correction = invoke(
        &rt,
        "apply_memory_correction",
        json!({
            "original_record_ref": "mem:player_name:abc123",
            "corrected_content": {"name": "Bob", "type": "corrected"},
            "reason": "user_correction",
        }),
    )
    .await?;

    anyhow::ensure!(correction.output["kind"] == json!("memory_correction"));
    anyhow::ensure!(correction.output["requires_user_approval"] == json!(true));
    anyhow::ensure!(
        correction.output["content_address"].is_string(),
        "correction must have content_address"
    );
    anyhow::ensure!(correction.output["inference_performed"] == json!(false));

    Ok(())
}

/// Case 7: Forget/redaction produces redaction plan, not deletion.
pub(crate) async fn memory_lab_forget_redaction() -> anyhow::Result<()> {
    let rt = load_memory_lab().await?;

    let redaction = invoke(
        &rt,
        "draft_forget_redaction",
        json!({
            "target_record_ref": "mem:player_name:abc123",
            "reason": "user_requested_forget",
            "redaction_scope": "record_only",
        }),
    )
    .await?;

    anyhow::ensure!(redaction.output["kind"] == json!("memory_redaction_plan"));
    anyhow::ensure!(redaction.output["status"] == json!("draft"));
    anyhow::ensure!(redaction.output["requires_user_approval"] == json!(true));
    anyhow::ensure!(redaction.output["plan_only"] == json!(true));
    anyhow::ensure!(redaction.output["inference_performed"] == json!(false));
    // Not a deletion — it's a plan
    anyhow::ensure!(!serde_json::to_string(&redaction.output)?.contains("deleted"));

    Ok(())
}

/// Case 8: Branch-aware memory view filters by branch.
pub(crate) async fn memory_lab_branch_view() -> anyhow::Result<()> {
    let rt = load_memory_lab().await?;

    let view = invoke(
        &rt,
        "branch_memory_view",
        json!({
            "scope": "current_branch",
            "branch_ref": "branch:feature1",
            "records": [
                {"key": "a", "branch_ref": "branch:main"},
                {"key": "b", "branch_ref": "branch:feature1"},
                {"key": "c", "branch_ref": "branch:feature1"},
            ]
        }),
    )
    .await?;

    anyhow::ensure!(view.output["kind"] == json!("memory_branch_view"));
    anyhow::ensure!(view.output["scope"] == json!("current_branch"));
    // Only records from branch:feature1
    anyhow::ensure!(view.output["record_count"] == json!(2));
    anyhow::ensure!(view.output["inference_performed"] == json!(false));

    Ok(())
}

/// Case 9: No forbidden namespace in any output.
/// kernel.v1.memory.* is forbidden — memory is package-owned.
pub(crate) async fn memory_lab_no_forbidden_namespace() -> anyhow::Result<()> {
    let rt = load_memory_lab().await?;

    let caps = [
        "describe_memory_contract",
        "record_memory",
        "retrieve_memory",
        "trace_retrieval",
        "draft_memory_update",
        "apply_memory_correction",
        "draft_forget_redaction",
        "branch_memory_view",
        "explain_memory_provenance",
    ];

    let forbidden = [
        "kernel.v1.memory.",
        "kernel.v1.experience.",
        "kernel.v1.world.",
        "kernel.v1.scene.",
        "kernel.v1.turn.",
        "kernel.v1.chat.",
        "kernel.v1.agent.",
        "kernel.v1.model.",
        "kernel.v1.prompt.",
        "kernel.v1.director.",
    ];

    for cap in &caps {
        let result = invoke(&rt, cap, json!({"key": "ns_test"})).await?;
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

/// Case 10: No raw secrets in any capability output or input processing.
pub(crate) async fn memory_lab_no_raw_secrets() -> anyhow::Result<()> {
    let rt = load_memory_lab().await?;

    // record_memory blocks raw secret
    let record = invoke(
        &rt,
        "record_memory",
        json!({
            "key": "test",
            "api_key": "RawSecretExample1234567890abcdefABCDEF123456",
        }),
    )
    .await?;
    anyhow::ensure!(record.output["kind"] == json!("memory_lab_rejected"));
    anyhow::ensure!(record.output["redaction_state"] == json!("unsafe_blocked"));

    // draft_memory_update blocks raw secret
    let update = invoke(
        &rt,
        "draft_memory_update",
        json!({
            "key": "test",
            "token": "Bearer abc123",
        }),
    )
    .await?;
    anyhow::ensure!(update.output["kind"] == json!("memory_lab_rejected"));

    // draft_forget_redaction blocks raw secret
    let redaction = invoke(
        &rt,
        "draft_forget_redaction",
        json!({
            "target_record_ref": "mem:test",
            "secret": "RawSecretExample1234567890abcdefABCDEF123456",
        }),
    )
    .await?;
    anyhow::ensure!(redaction.output["kind"] == json!("memory_lab_rejected"));

    Ok(())
}
