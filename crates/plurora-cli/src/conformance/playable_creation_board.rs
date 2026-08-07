//! Conformance tests for `plurora/playable-creation-board` (Experience Beta 1 + Beta 2).
//!
//! Covers:
//! 1. Surface discovery / launch shape
//! 2. Player actions produce state changes (state_delta_asset_ref / projection_ref / sequence / provenance)
//! 3. Checkpoint / recovery shape (aligned with experience-runtime-lab)
//! 4. request_change produces structured agent objective (not chat messages)
//! 5. bind_agent_run produces scoped agentic-forge binding
//! 6. Candidate / proposal do not mutate target branch
//! 7. Reject / approve / fork minimal proof
//! 8. Third-party replacement: no official priority
//! 9. No forbidden namespace (platform.experience/world/scene/character/turn/chat/memory/agent/model/prompt/director)
//! 10. No raw secrets in any capability output
//! 11. content_address is stable and deterministic (Beta 2)
//! 12. create_checkpoint includes content_address and Beta 2 metadata (Beta 2)
//! 13. explain_provenance includes content_address per chain step and provenance_graph (Beta 2)
//! 14. preview_state_diff produces branch-aware diff preview (Beta 2)
//! 15. describe_asset_provenance returns provenance graph with disclosure (Beta 2)
//! 16. Beta 2 capabilities block raw secrets (Beta 2)

use std::path::PathBuf;

use plurora_runtime::CapabilityInvocationRequest;
use serde_json::json;

use super::fixtures::*;
use crate::commands::manifest;

const PACKAGE_ID: &str = "plurora/playable-creation-board";

async fn load_playable_creation_board(
) -> anyhow::Result<plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/playable-creation-board/manifest.yaml",
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

/// Case 1: Surface discovery / launch shape.
/// describe_contract returns 4 surfaces, 11 capabilities, ordinary package.
pub(crate) async fn playable_board_describe_contract() -> anyhow::Result<()> {
    let runtime = load_playable_creation_board().await?;

    let contract = invoke(&runtime, "describe_contract", json!({})).await?;

    anyhow::ensure!(
        contract.output["kind"] == json!("playable_creation_board_contract"),
        "describe_contract must return playable_creation_board_contract kind"
    );
    anyhow::ensure!(
        contract.output["package_kind"] == json!("ordinary"),
        "must be ordinary package"
    );

    // 4 surfaces
    let surfaces = contract.output["surfaces"].as_object().unwrap();
    anyhow::ensure!(
        surfaces.contains_key("experience_entry"),
        "must have experience_entry surface"
    );
    anyhow::ensure!(
        surfaces.contains_key("play_renderer"),
        "must have play_renderer surface"
    );
    anyhow::ensure!(
        surfaces.contains_key("forge_panel"),
        "must have forge_panel surface"
    );
    anyhow::ensure!(
        surfaces.contains_key("assistant_action"),
        "must have assistant_action surface"
    );

    // 11 capabilities (Beta 1: 11, Beta 2: +2 = 13, Beta 3: +1 = 14)
    anyhow::ensure!(
        contract.output["capabilities"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
            == 14,
        "describe_contract must list 14 capabilities"
    );

    // No inference / no network
    anyhow::ensure!(contract.output["inference_performed"] == json!(false));
    anyhow::ensure!(contract.output["network_performed"] == json!(false));

    Ok(())
}

/// Case 2: Launch and player actions produce state_delta_asset_ref / projection_ref / sequence / provenance.
pub(crate) async fn playable_board_launch_and_player_actions() -> anyhow::Result<()> {
    let runtime = load_playable_creation_board().await?;

    // Launch
    let launch = invoke(
        &runtime,
        "launch",
        json!({
            "board_id": "board:conf",
            "title": "Conformance Board",
        }),
    )
    .await?;
    anyhow::ensure!(launch.output["kind"] == json!("playable_creation_board_launched"));
    anyhow::ensure!(launch.output["lifecycle_state"] == json!("created"));
    anyhow::ensure!(launch.output["board_id"] == json!("board:conf"));

    // Record 3 player actions
    for seq in 1..=3 {
        let action = invoke(
            &runtime,
            "record_player_action",
            json!({
                "board_id": "board:conf",
                "action_kind": "place_marker",
                "sequence": seq,
                "payload": {"marker_id": format!("m{}", seq)},
            }),
        )
        .await?;

        anyhow::ensure!(action.output["kind"] == json!("playable_creation_board_action_recorded"));
        anyhow::ensure!(action.output["sequence"] == json!(seq));
        anyhow::ensure!(
            action.output["state_delta_asset_ref"].is_string(),
            "must have state_delta_asset_ref"
        );
        anyhow::ensure!(
            action.output["projection_ref"].is_string(),
            "must have projection_ref"
        );
        anyhow::ensure!(action.output["provenance"]["package_id"].is_string());
    }

    Ok(())
}

/// Case 3: Checkpoint / recovery shape aligned with experience-runtime-lab.
pub(crate) async fn playable_board_checkpoint_recovery() -> anyhow::Result<()> {
    let runtime = load_playable_creation_board().await?;

    // Create checkpoint
    let checkpoint = invoke(
        &runtime,
        "create_checkpoint",
        json!({
            "board_id": "board:conf",
            "state_snapshot": {"markers": 3},
            "asset_refs": ["asset:board:conf"],
            "sequence": 1,
        }),
    )
    .await?;
    anyhow::ensure!(checkpoint.output["kind"] == json!("playable_creation_board_checkpoint"));
    anyhow::ensure!(checkpoint.output["format"] == json!("snapshot"));
    anyhow::ensure!(checkpoint.output["inference_performed"] == json!(false));

    // Inspect checkpoint
    let inspect = invoke(
        &runtime,
        "inspect_checkpoint",
        json!({
            "checkpoint_id": checkpoint.output["checkpoint_id"],
            "board_id": "board:conf",
            "state_snapshot": {"markers": 3},
            "format": "snapshot",
            "sequence": 1,
        }),
    )
    .await?;
    anyhow::ensure!(inspect.output["valid"] == json!(true));

    // Draft recovery with checkpoint
    let recovery = invoke(
        &runtime,
        "draft_recovery",
        json!({
            "board_id": "board:conf",
            "failure_kind": "constraint_violation",
            "last_checkpoint_ref": checkpoint.output["checkpoint_id"],
        }),
    )
    .await?;
    anyhow::ensure!(recovery.output["kind"] == json!("playable_creation_board_recovery_plan"));
    anyhow::ensure!(recovery.output["recommended_strategy"] == json!("restore_last_checkpoint"));
    anyhow::ensure!(recovery.output["plan"]["checkpoint_available"] == json!(true));

    // Draft recovery without checkpoint
    let recovery_no_cp = invoke(
        &runtime,
        "draft_recovery",
        json!({
            "board_id": "board:conf",
            "failure_kind": "checkpoint_missing",
        }),
    )
    .await?;
    anyhow::ensure!(recovery_no_cp.output["recommended_strategy"] == json!("restart_session"));

    Ok(())
}

/// Case 4: request_change produces structured agent objective, allowed_change_kinds, risk/budget,
/// and bindable refs — NOT chat messages or conversation.
pub(crate) async fn playable_board_request_change_no_chat() -> anyhow::Result<()> {
    let runtime = load_playable_creation_board().await?;

    let change = invoke(
        &runtime,
        "request_change",
        json!({
            "board_id": "board:conf",
            "objective": "add a new grid module",
            "change_kind": "add_module",
        }),
    )
    .await?;

    anyhow::ensure!(change.output["kind"] == json!("playable_creation_board_change_request"));
    anyhow::ensure!(change.output["objective"] == json!("add a new grid module"));
    anyhow::ensure!(
        change.output["allowed_change_kinds"].is_array(),
        "must have allowed_change_kinds"
    );
    anyhow::ensure!(change.output["risk"].is_string(), "must have risk");
    anyhow::ensure!(change.output["budget"].is_number(), "must have budget");
    anyhow::ensure!(
        change.output["bindable_refs"].is_object(),
        "must have bindable_refs"
    );
    anyhow::ensure!(
        change.output["agent_run_binding"].is_object(),
        "must have agent_run_binding"
    );
    anyhow::ensure!(change.output["requires_user_approval"] == json!(true));

    // Must NOT contain chat/message/conversation terminology
    let output_str = serde_json::to_string(&change.output).unwrap();
    anyhow::ensure!(
        !output_str.contains("assistant_message"),
        "no assistant_message"
    );
    anyhow::ensure!(!output_str.contains("conversation"), "no conversation");
    anyhow::ensure!(
        !output_str.contains("prompt_transcript"),
        "no prompt_transcript"
    );
    anyhow::ensure!(!output_str.contains("chat_message"), "no chat_message");

    Ok(())
}

/// Case 5: bind_agent_run produces scoped agentic-forge binding.
pub(crate) async fn playable_board_bind_agent_run_scoped() -> anyhow::Result<()> {
    let runtime = load_playable_creation_board().await?;

    let binding = invoke(
        &runtime,
        "bind_agent_run",
        json!({
            "board_id": "board:conf",
            "agent_package_id": "plurora/agentic-forge-lab",
        }),
    )
    .await?;

    anyhow::ensure!(binding.output["kind"] == json!("playable_creation_board_agent_run_binding"));
    anyhow::ensure!(binding.output["scoped_to_branch"] == json!(true));
    anyhow::ensure!(binding.output["forge_panel_binding"]["branch_aware"] == json!(true));
    anyhow::ensure!(
        binding.output["assist_binding"]["approval_policy"] == json!("fork_then_approve")
    );
    anyhow::ensure!(binding.output["inference_performed"] == json!(false));
    anyhow::ensure!(binding.output["network_performed"] == json!(false));

    Ok(())
}

/// Case 6: Candidate / proposal do not mutate target branch.
/// Uses agentic-forge-lab alongside playable-creation-board.
pub(crate) async fn playable_board_candidate_proposal_no_target_mutation() -> anyhow::Result<()> {
    let runtime = load_playable_creation_board().await?;
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/agentic-forge-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    // Start run
    let start_run = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/agentic-forge-lab/start_run".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/agentic-forge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"objective": "modify board module"}),
        })
        .await?;
    let run_id = start_run.output["run_id"].as_str().unwrap_or("run_unknown");

    // Create candidate
    let cand = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/agentic-forge-lab/create_candidate".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/agentic-forge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "run_id": run_id,
                "target_branch_ref": "branch:target:default",
                "scratch_branch_ref": "branch:scratch:default",
                "target_revision": 1,
            }),
        })
        .await?;
    anyhow::ensure!(
        cand.output["target_branch_unchanged"] == json!(true),
        "candidate must not change target"
    );

    // Draft promote proposal
    let promote = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/agentic-forge-lab/draft_promote_proposal".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/agentic-forge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "candidate_id": cand.output["candidate"]["candidate_id"],
                "run_id": run_id,
                "target_revision": 1,
                "current_target_revision": 1,
                "target_branch_ref": "branch:target:default",
                "scratch_branch_ref": "branch:scratch:default",
            }),
        })
        .await?;
    anyhow::ensure!(
        promote.output["target_branch_unchanged"] == json!(true),
        "proposal must not change target"
    );
    anyhow::ensure!(
        promote.output["direct_mutation"] == json!(false),
        "no direct mutation"
    );

    Ok(())
}

/// Case 7: Reject / approve / fork minimal proof via proposal lifecycle.
/// Reject leaves target unchanged; approve requires user approval.
pub(crate) async fn playable_board_reject_approve_fork_proof() -> anyhow::Result<()> {
    let runtime = load_playable_creation_board().await?;
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/agentic-forge-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    // Start run + create candidate + draft promote
    let start_run = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/agentic-forge-lab/start_run".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/agentic-forge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"objective": "test reject/approve"}),
        })
        .await?;
    let run_id = start_run.output["run_id"].as_str().unwrap_or("run_unknown");

    let cand = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/agentic-forge-lab/create_candidate".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/agentic-forge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"run_id": run_id, "target_revision": 1}),
        })
        .await?;

    let cand_id = cand.output["candidate"]["candidate_id"]
        .as_str()
        .unwrap_or("cand_unknown");

    let promote = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/agentic-forge-lab/draft_promote_proposal".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/agentic-forge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "candidate_id": cand_id,
                "run_id": run_id,
                "target_revision": 1,
                "current_target_revision": 1,
            }),
        })
        .await?;

    // Proposal draft requires user approval
    anyhow::ensure!(
        promote.output["proposal_draft"]["requires_user_approval"] == json!(true),
        "proposal must require user approval"
    );

    // Archive candidate = reject proof (target unchanged)
    let archive = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/agentic-forge-lab/archive_candidate".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/agentic-forge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"candidate_id": cand_id}),
        })
        .await?;
    anyhow::ensure!(
        archive.output["target_branch_unchanged"] == json!(true),
        "archive must not change target"
    );

    Ok(())
}

/// Case 8: Third-party replacement: no official priority.
/// bind_agent_run accepts thirdparty/agentic-forge as forge provider.
pub(crate) async fn playable_board_thirdparty_no_publisher_priority() -> anyhow::Result<()> {
    let runtime = load_playable_creation_board().await?;

    // Bind with third-party forge
    let binding = invoke(
        &runtime,
        "bind_agent_run",
        json!({
            "board_id": "board:conf",
            "agent_package_id": "thirdparty/agentic-forge",
            "run_capabilities": [
                "thirdparty/agentic-forge/start_run",
                "thirdparty/agentic-forge/create_candidate",
                "thirdparty/agentic-forge/draft_promote_proposal"
            ],
        }),
    )
    .await?;

    anyhow::ensure!(binding.output["kind"] == json!("playable_creation_board_agent_run_binding"));
    anyhow::ensure!(binding.output["agent_package_id"] == json!("thirdparty/agentic-forge"));
    // No publisher_priority field
    let output_str = serde_json::to_string(&binding.output).unwrap();
    anyhow::ensure!(
        !output_str.contains("publisher_priority"),
        "no publisher_priority field"
    );
    Ok(())
}

/// Case 9: No forbidden namespace in any output.
pub(crate) async fn playable_board_no_forbidden_namespace() -> anyhow::Result<()> {
    let runtime = load_playable_creation_board().await?;

    let caps = [
        "describe_contract",
        "launch",
        "project_state",
        "render_payload",
        "record_player_action",
        "request_change",
        "create_checkpoint",
        "inspect_checkpoint",
        "draft_recovery",
        "bind_agent_run",
        "explain_provenance",
        "summarize_experience_health",
    ];

    let forbidden = [
        "platform.experience.",
        "platform.world.",
        "platform.scene.",
        "platform.character.",
        "platform.turn.",
        "platform.chat.",
        "platform.memory.",
        "platform.agent.",
        "platform.model.",
        "platform.prompt.",
        "platform.director.",
    ];

    for cap in &caps {
        let result = invoke(&runtime, cap, json!({"board_id": "board:ns"})).await?;
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
pub(crate) async fn playable_board_no_raw_secrets() -> anyhow::Result<()> {
    let runtime = load_playable_creation_board().await?;

    // record_player_action blocks raw secret
    let action = invoke(
        &runtime,
        "record_player_action",
        json!({
            "board_id": "board:secret",
            "api_key": "RawSecretExample1234567890abcdefABCDEF123456",
        }),
    )
    .await?;
    anyhow::ensure!(action.output["kind"] == json!("playable_creation_board_action_rejected"));
    anyhow::ensure!(action.output["redaction_state"] == json!("unsafe_blocked"));

    // create_checkpoint blocks raw secret
    let checkpoint = invoke(
        &runtime,
        "create_checkpoint",
        json!({
            "board_id": "board:secret",
            "token": "Bearer abc123",
        }),
    )
    .await?;
    anyhow::ensure!(
        checkpoint.output["kind"] == json!("playable_creation_board_checkpoint_rejected")
    );

    // request_change blocks raw secret
    let change = invoke(
        &runtime,
        "request_change",
        json!({
            "board_id": "board:secret",
            "objective": "test",
            "api_key": "RawSecretExample1234567890abcdefABCDEF",
        }),
    )
    .await?;
    anyhow::ensure!(change.output["kind"] == json!("playable_creation_board_change_rejected"));

    // bind_agent_run blocks raw secret
    let binding = invoke(
        &runtime,
        "bind_agent_run",
        json!({
            "board_id": "board:secret",
            "token": "Bearer xyz",
        }),
    )
    .await?;
    anyhow::ensure!(binding.output["kind"] == json!("playable_creation_board_binding_rejected"));

    Ok(())
}

// ---------------------------------------------------------------------------
// Experience Beta 2 — State + Asset Pipeline Alpha conformance cases
// ---------------------------------------------------------------------------

/// Case 11: content_address is stable and deterministic in record_player_action.
pub(crate) async fn playable_board_content_address_stable() -> anyhow::Result<()> {
    let runtime = load_playable_creation_board().await?;

    // Same payload twice should produce same content_address
    let action1 = invoke(
        &runtime,
        "record_player_action",
        json!({
            "board_id": "board:b2",
            "action_kind": "place_marker",
            "sequence": 1,
            "payload": {"marker_id": "stable_test"},
        }),
    )
    .await?;
    let action2 = invoke(
        &runtime,
        "record_player_action",
        json!({
            "board_id": "board:b2",
            "action_kind": "place_marker",
            "sequence": 1,
            "payload": {"marker_id": "stable_test"},
        }),
    )
    .await?;

    anyhow::ensure!(
        action1.output["content_address"] == action2.output["content_address"],
        "content_address must be deterministic"
    );
    anyhow::ensure!(
        action1.output["content_address"]
            .as_str()
            .map(|s| s.starts_with("sha256:"))
            .unwrap_or(false),
        "content_address must use sha256 scheme"
    );
    anyhow::ensure!(
        action1.output["state_snapshot_asset_ref"].is_string(),
        "must have state_snapshot_asset_ref"
    );

    Ok(())
}

/// Case 12: create_checkpoint includes content_address and Beta 2 metadata.
pub(crate) async fn playable_board_checkpoint_metadata() -> anyhow::Result<()> {
    let runtime = load_playable_creation_board().await?;

    let checkpoint = invoke(
        &runtime,
        "create_checkpoint",
        json!({
            "board_id": "board:b2",
            "state_snapshot": {"markers": 3},
            "asset_refs": ["asset:board:b2"],
            "sequence": 1,
            "disclosure": "checkpoint_debug",
        }),
    )
    .await?;

    anyhow::ensure!(checkpoint.output["kind"] == json!("playable_creation_board_checkpoint"));
    anyhow::ensure!(
        checkpoint.output["content_address"]
            .as_str()
            .map(|s| s.starts_with("sha256:"))
            .unwrap_or(false),
        "checkpoint must have sha256 content_address"
    );
    anyhow::ensure!(
        checkpoint.output["state_snapshot_asset_ref"].is_string(),
        "checkpoint must have state_snapshot_asset_ref"
    );
    anyhow::ensure!(
        checkpoint.output["disclosure"] == json!("checkpoint_debug"),
        "checkpoint must have disclosure"
    );
    anyhow::ensure!(
        checkpoint.output["provenance"]["content_address"].is_string(),
        "checkpoint provenance must have content_address"
    );

    Ok(())
}

/// Case 13: explain_provenance includes content_address per chain step and provenance graph.
pub(crate) async fn playable_board_provenance_graph() -> anyhow::Result<()> {
    let runtime = load_playable_creation_board().await?;

    let provenance = invoke(
        &runtime,
        "explain_provenance",
        json!({
            "board_id": "board:b2",
            "action_id": "action:board:b2:1",
            "sequence": 1,
        }),
    )
    .await?;

    anyhow::ensure!(provenance.output["kind"] == json!("playable_creation_board_provenance_chain"));

    // Every chain step must have content_address
    let chain = provenance.output["chain"].as_array().unwrap();
    for (i, step) in chain.iter().enumerate() {
        anyhow::ensure!(
            step["content_address"].is_string(),
            "chain step {} must have content_address",
            i
        );
    }

    // Must have provenance_graph with nodes and edges
    anyhow::ensure!(
        provenance.output["provenance_graph"].is_object(),
        "must have provenance_graph"
    );
    anyhow::ensure!(
        provenance.output["provenance_graph"]["nodes"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false),
        "provenance_graph must have nodes"
    );
    anyhow::ensure!(
        provenance.output["provenance_graph"]["edges"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false),
        "provenance_graph must have edges"
    );

    Ok(())
}

/// Case 14: preview_state_diff produces branch-aware diff preview.
pub(crate) async fn playable_board_state_diff_preview() -> anyhow::Result<()> {
    let runtime = load_playable_creation_board().await?;

    let diff = invoke(
        &runtime,
        "preview_state_diff",
        json!({
            "board_id": "board:b2",
            "before_snapshot_ref": "asset:state_snapshot:board:b2:1",
            "after_snapshot_ref": "asset:state_snapshot:board:b2:2",
            "branch_ref": "branch:target:default",
        }),
    )
    .await?;

    anyhow::ensure!(diff.output["kind"] == json!("playable_creation_board_state_diff_preview"));
    anyhow::ensure!(diff.output["branch_aware"] == json!(true));
    anyhow::ensure!(
        diff.output["before_content_address"].is_string(),
        "must have before_content_address"
    );
    anyhow::ensure!(
        diff.output["after_content_address"].is_string(),
        "must have after_content_address"
    );
    anyhow::ensure!(diff.output["diff_summary"].is_string());
    anyhow::ensure!(diff.output["inference_performed"] == json!(false));
    anyhow::ensure!(diff.output["network_performed"] == json!(false));

    Ok(())
}

/// Case 15: describe_asset_provenance returns provenance graph with disclosure.
pub(crate) async fn playable_board_describe_asset_provenance() -> anyhow::Result<()> {
    let runtime = load_playable_creation_board().await?;

    let desc = invoke(
        &runtime,
        "describe_asset_provenance",
        json!({
            "board_id": "board:b2",
            "asset_ref": "asset:state_delta:board:b2:1",
            "disclosure": "debug",
        }),
    )
    .await?;

    anyhow::ensure!(desc.output["kind"] == json!("playable_creation_board_asset_provenance"));
    anyhow::ensure!(
        desc.output["content_address"].is_string(),
        "must have content_address"
    );
    anyhow::ensure!(
        desc.output["provenance_graph"].is_object(),
        "must have provenance_graph"
    );
    anyhow::ensure!(
        desc.output["provenance_graph"]["disclosure"] == json!("debug"),
        "provenance_graph must have disclosure"
    );
    anyhow::ensure!(desc.output["inference_performed"] == json!(false));

    Ok(())
}

/// Case 16: preview_state_diff blocks raw secrets.
pub(crate) async fn playable_board_beta2_no_raw_secrets() -> anyhow::Result<()> {
    let runtime = load_playable_creation_board().await?;

    // preview_state_diff blocks raw secret
    let diff = invoke(
        &runtime,
        "preview_state_diff",
        json!({
            "board_id": "board:secret",
            "api_key": "RawSecretExample1234567890abcdefABCDEF123456",
        }),
    )
    .await?;
    anyhow::ensure!(diff.output["kind"] == json!("playable_creation_board_state_diff_rejected"));

    // describe_asset_provenance blocks raw secret
    let desc = invoke(
        &runtime,
        "describe_asset_provenance",
        json!({
            "board_id": "board:secret",
            "token": "Bearer abc123",
        }),
    )
    .await?;
    anyhow::ensure!(
        desc.output["kind"] == json!("playable_creation_board_provenance_describe_rejected")
    );

    Ok(())
}
