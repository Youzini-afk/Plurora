//! Handler for `official/playable-creation-board` capabilities.
//!
//! Experience Beta 1 — First Real Playable Vertical Slice.
//!
//! Package-owned playable creation board with board/module/constraint/marker
//! state. Play→record_player_action→request_change→agentic forge
//! candidate/proposal→inspect/approve/reject→fork/compare→recovery.
//!
//! Deterministic, no-network, no real model inference.
//!
//! No `kernel.v1.experience.*`, `kernel.v1.world.*`, `kernel.v1.scene.*`,
//! `kernel.v1.character.*`, `kernel.v1.turn.*`, `kernel.v1.chat.*`,
//! `kernel.v1.memory.*`, `kernel.v1.agent.*`, `kernel.v1.model.*`,
//! `kernel.v1.prompt.*`, or `kernel.v1.director.*` namespace references.
//!
//! State terminology: board, module, constraint, marker — not
//! world/scene/character/chat/message/turn/memory/prompt/director.

use serde_json::Value;

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/playable-creation-board";

// ---------------------------------------------------------------------------
// Board lifecycle states
// ---------------------------------------------------------------------------

const BOARD_LIFECYCLE_STATES: &[&str] = &[
    "created",
    "running",
    "paused",
    "checkpointed",
    "recovering",
    "recovered",
    "failed",
    "completed",
    "archived",
];

// ---------------------------------------------------------------------------
// Player action kinds
// ---------------------------------------------------------------------------

const PLAYER_ACTION_KINDS: &[&str] = &[
    "place_marker",
    "move_marker",
    "remove_marker",
    "set_constraint",
    "modify_module",
    "request_change",
];

// ---------------------------------------------------------------------------
// Change kinds (for request_change)
// ---------------------------------------------------------------------------

const ALLOWED_CHANGE_KINDS: &[&str] = &[
    "add_module",
    "remove_module",
    "modify_module",
    "add_constraint",
    "remove_constraint",
    "modify_constraint",
    "add_marker",
    "remove_marker",
    "move_marker",
];

// ---------------------------------------------------------------------------
// Checkpoint formats
// ---------------------------------------------------------------------------

const CHECKPOINT_FORMATS: &[&str] = &["snapshot", "incremental", "delta"];

// ---------------------------------------------------------------------------
// Recovery strategies
// ---------------------------------------------------------------------------

const RECOVERY_STRATEGIES: &[&str] = &[
    "restore_last_checkpoint",
    "replay_from_checkpoint",
    "restart_session",
    "manual_intervention",
    "discard_and_reset",
];

// ---------------------------------------------------------------------------
// Failure kinds
// ---------------------------------------------------------------------------

const FAILURE_KINDS: &[&str] = &[
    "state_corruption",
    "checkpoint_missing",
    "checkpoint_corrupt",
    "constraint_violation",
    "module_failure",
    "resource_exhausted",
    "unknown",
];

// ---------------------------------------------------------------------------
// Risk levels for request_change
// ---------------------------------------------------------------------------

const RISK_LEVELS: &[&str] = &["low", "medium", "high", "critical"];

// ---------------------------------------------------------------------------
// Raw-secret detection (delegated to shared safety module)
// ---------------------------------------------------------------------------

use super::safety;

// ---------------------------------------------------------------------------
// Deterministic ID helper
// ---------------------------------------------------------------------------

fn deterministic_id(input: &Value) -> String {
    let objective = input
        .get("objective")
        .and_then(Value::as_str)
        .unwrap_or("default");
    let len = objective.len();
    format!("{:04x}", len.wrapping_mul(31).wrapping_add(0xcb))
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }
    let id = request.capability_id.as_str();
    if id.ends_with("/describe_contract") {
        Some(describe_contract(request))
    } else if id.ends_with("/launch") {
        Some(launch(request))
    } else if id.ends_with("/project_state") {
        Some(project_state(request))
    } else if id.ends_with("/render_payload") {
        Some(render_payload(request))
    } else if id.ends_with("/record_player_action") {
        Some(record_player_action(request))
    } else if id.ends_with("/request_change") {
        Some(request_change(request))
    } else if id.ends_with("/create_checkpoint") {
        Some(create_checkpoint(request))
    } else if id.ends_with("/inspect_checkpoint") {
        Some(inspect_checkpoint(request))
    } else if id.ends_with("/draft_recovery") {
        Some(draft_recovery(request))
    } else if id.ends_with("/bind_agent_run") {
        Some(bind_agent_run(request))
    } else if id.ends_with("/explain_provenance") {
        Some(explain_provenance(request))
    } else if id.ends_with("/preview_state_diff") {
        Some(preview_state_diff(request))
    } else if id.ends_with("/describe_asset_provenance") {
        Some(describe_asset_provenance(request))
    } else if id.ends_with("/summarize_experience_health") {
        Some(summarize_experience_health(request))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Capability implementations
// ---------------------------------------------------------------------------

fn describe_contract(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "playable_creation_board_contract",
        "package_id": request.provider_package_id,
        "package_kind": "ordinary",
        "capabilities": [
            {"id": "official/playable-creation-board/describe_contract", "purpose": "describe the playable creation board package contract"},
            {"id": "official/playable-creation-board/launch", "purpose": "launch a playable creation board with board/module/constraint/marker state"},
            {"id": "official/playable-creation-board/project_state", "purpose": "project the current board state as a package-owned opaque state snapshot"},
            {"id": "official/playable-creation-board/render_payload", "purpose": "produce a protocol-visible render payload for the board"},
            {"id": "official/playable-creation-board/record_player_action", "purpose": "record a player action producing content_address/state_snapshot_asset_ref/projection_ref/sequence/provenance"},
            {"id": "official/playable-creation-board/request_change", "purpose": "produce a structured agent objective, allowed_change_kinds, risk/budget, and bindable refs for agentic forge"},
            {"id": "official/playable-creation-board/create_checkpoint", "purpose": "create a deterministic board checkpoint asset with content_address and state_snapshot_asset_ref"},
            {"id": "official/playable-creation-board/inspect_checkpoint", "purpose": "inspect a board checkpoint's shape and validity"},
            {"id": "official/playable-creation-board/draft_recovery", "purpose": "draft a recovery plan for a failed board session"},
            {"id": "official/playable-creation-board/bind_agent_run", "purpose": "bind an agentic forge run to the board session with scoped branch binding"},
            {"id": "official/playable-creation-board/explain_provenance", "purpose": "explain causal chain with content_address and provenance graph fields"},
            {"id": "official/playable-creation-board/preview_state_diff", "purpose": "preview branch-aware state diff between snapshots"},
            {"id": "official/playable-creation-board/describe_asset_provenance", "purpose": "describe asset provenance graph with source/derived/disclosure metadata"},
            {"id": "official/playable-creation-board/summarize_experience_health", "purpose": "summarize board experience health with observability refs"},
        ],
        "surfaces": {
            "experience_entry": "official/playable-creation-board/entry",
            "play_renderer": "official/playable-creation-board/play-renderer",
            "forge_panel": "official/playable-creation-board/forge-panel",
            "assistant_action": "official/playable-creation-board/assistant-action",
        },
        "lifecycle_states": BOARD_LIFECYCLE_STATES,
        "player_action_kinds": PLAYER_ACTION_KINDS,
        "allowed_change_kinds": ALLOWED_CHANGE_KINDS,
        "checkpoint_formats": CHECKPOINT_FORMATS,
        "recovery_strategies": RECOVERY_STRATEGIES,
        "failure_kinds": FAILURE_KINDS,
        "risk_levels": RISK_LEVELS,
        "board_state_fields": [
            "board_id", "modules", "constraints", "markers",
            "sequence", "lifecycle_state", "asset_refs",
            "branch_ref", "provenance"
        ],
        "player_action_output_fields": [
            "action_id", "action_kind", "board_id",
            "state_delta_asset_ref", "projection_ref",
            "sequence", "provenance",
            "content_address", "state_snapshot_asset_ref",
            "disclosure"
        ],
        "request_change_output_fields": [
            "objective", "allowed_change_kinds", "risk",
            "budget", "bindable_refs", "memory_refs",
            "agent_run_binding_ref", "provenance",
            "content_address", "disclosure"
        ],
        "checkpoint_fields": [
            "checkpoint_id", "board_id", "format", "state_snapshot",
            "asset_refs", "branch_ref", "sequence",
            "inference_performed", "network_performed", "provenance",
            "content_address", "state_snapshot_asset_ref",
            "disclosure"
        ],
        "recovery_fields": [
            "failure_kind", "failure_detail", "last_checkpoint_ref",
            "recovery_strategy", "recovery_plan", "inference_performed",
            "network_performed", "provenance"
        ],
        "forge_binding_fields": [
            "board_id", "surface_id", "inspect_capabilities",
            "proposal_capabilities", "branch_aware",
            "inference_performed", "network_performed", "provenance"
        ],
        "provenance_chain_fields": [
            "player_action_event", "state_delta_asset",
            "checkpoint", "agent_run", "candidate",
            "proposal", "projection_rebuild"
        ],
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn launch(request: &InprocInvocation) -> anyhow::Result<Value> {
    let board_id = request
        .input
        .get("board_id")
        .and_then(Value::as_str)
        .unwrap_or("board:default");
    let title = request
        .input
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Playable Creation Board");
    let modules = request
        .input
        .get("modules")
        .cloned()
        .unwrap_or(serde_json::json!([]));
    let constraints = request
        .input
        .get("constraints")
        .cloned()
        .unwrap_or(serde_json::json!([]));
    let markers = request
        .input
        .get("markers")
        .cloned()
        .unwrap_or(serde_json::json!([]));

    Ok(serde_json::json!({
        "kind": "playable_creation_board_launched",
        "package_id": request.provider_package_id,
        "board_id": board_id,
        "title": title,
        "lifecycle_state": "created",
        "initial_state": {
            "modules": modules,
            "constraints": constraints,
            "markers": markers,
        },
        "render_capability_id": "official/playable-creation-board/render_payload",
        "forge_panel_id": "official/playable-creation-board/forge-panel",
        "assistant_action_id": "official/playable-creation-board/assistant-action",
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn project_state(request: &InprocInvocation) -> anyhow::Result<Value> {
    let board_id = request
        .input
        .get("board_id")
        .and_then(Value::as_str)
        .unwrap_or("board:default");

    Ok(serde_json::json!({
        "kind": "playable_creation_board_state",
        "package_id": request.provider_package_id,
        "board_id": board_id,
        "state": request.input.get("state").cloned().unwrap_or_else(|| serde_json::json!({
            "modules": [],
            "constraints": [],
            "markers": [],
            "sequence": 0,
        })),
        "lifecycle_state": request.input
            .get("lifecycle_state")
            .and_then(Value::as_str)
            .filter(|s| BOARD_LIFECYCLE_STATES.contains(s))
            .unwrap_or("running"),
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn render_payload(request: &InprocInvocation) -> anyhow::Result<Value> {
    let board_id = request
        .input
        .get("board_id")
        .and_then(Value::as_str)
        .unwrap_or("board:default");

    Ok(serde_json::json!({
        "kind": "playable_creation_board_render_payload",
        "package_id": request.provider_package_id,
        "board_id": board_id,
        "blocks": request.input
            .get("blocks")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([
                {"type": "board_summary", "board_id": board_id},
                {"type": "modules", "text": "No modules yet"},
                {"type": "constraints", "text": "No constraints yet"},
                {"type": "markers", "text": "No markers placed"},
            ])),
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn record_player_action(request: &InprocInvocation) -> anyhow::Result<Value> {
    // Raw-secret check
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "playable_creation_board_action_rejected",
            "redaction_state": "unsafe_blocked",
            "reason": "input contains raw-secret-like content; use secret_ref references instead",
            "inference_performed": false,
            "network_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    let board_id = request
        .input
        .get("board_id")
        .and_then(Value::as_str)
        .unwrap_or("board:default");
    let action_kind = request
        .input
        .get("action_kind")
        .and_then(Value::as_str)
        .filter(|k| PLAYER_ACTION_KINDS.contains(k))
        .unwrap_or("place_marker");
    let sequence = request
        .input
        .get("sequence")
        .and_then(Value::as_u64)
        .unwrap_or(1);

    let action_id = format!("action:{}:{}", board_id, sequence);
    let state_delta_asset_ref = format!("asset:state_delta:{}:{}", board_id, sequence);
    let projection_ref = format!("projection:board:{}:{}", board_id, sequence);

    // Beta 2: deterministic content address for the action payload
    let payload_str = serde_json::to_string(
        &request
            .input
            .get("payload")
            .cloned()
            .unwrap_or(serde_json::json!({})),
    )
    .unwrap_or_default();
    let ca = crate::runtime::content_address(&payload_str);
    let state_snapshot_asset_ref = format!("asset:state_snapshot:{}:{}", board_id, sequence);
    let disclosure = request
        .input
        .get("disclosure")
        .and_then(Value::as_str)
        .unwrap_or("player_action");

    Ok(serde_json::json!({
        "kind": "playable_creation_board_action_recorded",
        "package_id": request.provider_package_id,
        "board_id": board_id,
        "action_id": action_id,
        "action_kind": action_kind,
        "state_delta_asset_ref": state_delta_asset_ref,
        "projection_ref": projection_ref,
        "sequence": sequence,
        "payload": request.input.get("payload").cloned().unwrap_or(serde_json::json!({})),
        "content_address": ca,
        "state_snapshot_asset_ref": state_snapshot_asset_ref,
        "disclosure": disclosure,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id,
            "content_address": ca,
            "source_refs": [state_delta_asset_ref],
            "derived_refs": [],
        }
    }))
}

fn request_change(request: &InprocInvocation) -> anyhow::Result<Value> {
    // Raw-secret check
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "playable_creation_board_change_rejected",
            "redaction_state": "unsafe_blocked",
            "reason": "input contains raw-secret-like content; use secret_ref references instead",
            "inference_performed": false,
            "network_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    let board_id = request
        .input
        .get("board_id")
        .and_then(Value::as_str)
        .unwrap_or("board:default");
    let objective = request
        .input
        .get("objective")
        .and_then(Value::as_str)
        .unwrap_or("modify board state according to player request");
    let change_kind = request
        .input
        .get("change_kind")
        .and_then(Value::as_str)
        .filter(|k| ALLOWED_CHANGE_KINDS.contains(k))
        .unwrap_or("modify_module");

    let risk = request
        .input
        .get("risk")
        .and_then(Value::as_str)
        .filter(|r| RISK_LEVELS.contains(r))
        .unwrap_or("medium");

    let budget = request
        .input
        .get("budget")
        .and_then(Value::as_u64)
        .unwrap_or(100);

    let target_branch_ref = request
        .input
        .get("target_branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:target:default");
    let scratch_branch_ref = request
        .input
        .get("scratch_branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:scratch:default");

    let agent_run_binding_ref = format!(
        "binding:forge:{}:{}",
        board_id,
        deterministic_id(&request.input)
    );

    Ok(serde_json::json!({
        "kind": "playable_creation_board_change_request",
        "package_id": request.provider_package_id,
        "board_id": board_id,
        "objective": objective,
        "allowed_change_kinds": ALLOWED_CHANGE_KINDS,
        "requested_change_kind": change_kind,
        "risk": risk,
        "budget": budget,
        "bindable_refs": {
            "board_state_ref": format!("asset:board_state:{}", board_id),
            "target_branch_ref": target_branch_ref,
            "scratch_branch_ref": scratch_branch_ref,
            "agent_run_binding_ref": agent_run_binding_ref,
        },
        "memory_refs": {
            "memory_package_id": request.input
                .get("memory_package_id")
                .and_then(Value::as_str)
                .unwrap_or("official/memory-lab"),
            "retrieve_context_plan": {
                "capability_id": "official/memory-lab/retrieve_memory",
                "optional": true,
                "description": "Optional memory context retrieval for board change planning; board does not depend on memory-lab to operate",
            },
            "knowledge_refs": request.input.get("knowledge_refs").cloned().unwrap_or(serde_json::json!([])),
        },
        "agent_run_binding": {
            "binding_ref": agent_run_binding_ref,
            "forge_package_id": request.input
                .get("forge_package_id")
                .and_then(Value::as_str)
                .unwrap_or("official/agentic-forge-lab"),
            "run_capabilities": [
                "official/agentic-forge-lab/start_run",
                "official/agentic-forge-lab/create_candidate",
                "official/agentic-forge-lab/draft_promote_proposal"
            ],
            "scoped_to_branch": true,
        },
        "requires_user_approval": true,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn create_checkpoint(request: &InprocInvocation) -> anyhow::Result<Value> {
    // Raw-secret check
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "playable_creation_board_checkpoint_rejected",
            "redaction_state": "unsafe_blocked",
            "reason": "input contains raw-secret-like content; use secret_ref references instead",
            "inference_performed": false,
            "network_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    let board_id = request
        .input
        .get("board_id")
        .and_then(Value::as_str)
        .unwrap_or("board:default");
    let format = request
        .input
        .get("format")
        .and_then(Value::as_str)
        .filter(|f| CHECKPOINT_FORMATS.contains(f))
        .unwrap_or("snapshot");
    let branch_ref = request
        .input
        .get("branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:default");
    let sequence = request
        .input
        .get("sequence")
        .and_then(Value::as_u64)
        .unwrap_or(1);

    let state_snapshot = request
        .input
        .get("state_snapshot")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let asset_refs = request
        .input
        .get("asset_refs")
        .cloned()
        .unwrap_or(serde_json::json!([]));

    let checkpoint_id = format!("checkpoint:{}:{}", board_id, sequence);

    // Beta 2: deterministic content address for the checkpoint
    let checkpoint_content = serde_json::to_string(&serde_json::json!({
        "board_id": board_id,
        "sequence": sequence,
        "format": format,
        "state_snapshot": state_snapshot,
    }))
    .unwrap_or_default();
    let ca = crate::runtime::content_address(&checkpoint_content);
    let state_snapshot_asset_ref = format!("asset:state_snapshot:{}:{}", board_id, sequence);
    let disclosure = request
        .input
        .get("disclosure")
        .and_then(Value::as_str)
        .unwrap_or("checkpoint");

    Ok(serde_json::json!({
        "kind": "playable_creation_board_checkpoint",
        "package_id": request.provider_package_id,
        "board_id": board_id,
        "checkpoint_id": checkpoint_id,
        "format": format,
        "state_snapshot": state_snapshot,
        "asset_refs": asset_refs,
        "branch_ref": branch_ref,
        "sequence": sequence,
        "content_address": ca,
        "state_snapshot_asset_ref": state_snapshot_asset_ref,
        "disclosure": disclosure,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id,
            "content_address": ca,
            "source_refs": [format!("asset:board_state:{}", board_id)],
            "derived_refs": [state_snapshot_asset_ref.clone()],
            "branch_ref": branch_ref,
        }
    }))
}

fn inspect_checkpoint(request: &InprocInvocation) -> anyhow::Result<Value> {
    let checkpoint_id = request
        .input
        .get("checkpoint_id")
        .and_then(Value::as_str)
        .unwrap_or("checkpoint_unknown");
    let board_id = request
        .input
        .get("board_id")
        .and_then(Value::as_str)
        .unwrap_or("board:default");
    let format = request
        .input
        .get("format")
        .and_then(Value::as_str)
        .filter(|f| CHECKPOINT_FORMATS.contains(f))
        .unwrap_or("snapshot");
    let sequence = request
        .input
        .get("sequence")
        .and_then(Value::as_u64)
        .unwrap_or(1);

    let has_state = request.input.get("state_snapshot").is_some();
    let asset_count = request
        .input
        .get("asset_refs")
        .and_then(Value::as_array)
        .map(|a| a.len())
        .unwrap_or(0);

    let mut errors: Vec<String> = Vec::new();
    if checkpoint_id.is_empty() {
        errors.push("checkpoint_id must be non-empty".to_string());
    }
    if board_id.is_empty() {
        errors.push("board_id must be non-empty".to_string());
    }
    if !has_state {
        errors.push("state_snapshot must be present".to_string());
    }
    if !CHECKPOINT_FORMATS.contains(&format) {
        errors.push(format!("invalid checkpoint format: {}", format));
    }
    if sequence < 1 {
        errors.push("sequence must be >= 1".to_string());
    }

    let valid = errors.is_empty();

    Ok(serde_json::json!({
        "kind": "playable_creation_board_checkpoint_inspection",
        "checkpoint_id": checkpoint_id,
        "board_id": board_id,
        "valid": valid,
        "errors": errors,
        "format": format,
        "sequence": sequence,
        "asset_count": asset_count,
        "summary": if valid {
            format!("Board checkpoint {} valid (format={}, sequence={}, assets={})", checkpoint_id, format, sequence, asset_count)
        } else {
            format!("Board checkpoint {} invalid: {}", checkpoint_id, errors.join("; "))
        },
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn draft_recovery(request: &InprocInvocation) -> anyhow::Result<Value> {
    // Raw-secret check
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "playable_creation_board_recovery_rejected",
            "redaction_state": "unsafe_blocked",
            "reason": "input contains raw-secret-like content; use secret_ref references instead",
            "inference_performed": false,
            "network_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    let board_id = request
        .input
        .get("board_id")
        .and_then(Value::as_str)
        .unwrap_or("board:default");
    let failure_kind = request
        .input
        .get("failure_kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let last_checkpoint_ref = request
        .input
        .get("last_checkpoint_ref")
        .and_then(Value::as_str);

    let checkpoint_available = last_checkpoint_ref.is_some();

    let recommended_strategy = if checkpoint_available {
        "restore_last_checkpoint"
    } else {
        "restart_session"
    };

    let recovery_strategy = request
        .input
        .get("recovery_strategy")
        .and_then(Value::as_str)
        .filter(|s| RECOVERY_STRATEGIES.contains(s))
        .unwrap_or(recommended_strategy);

    let (steps, requires_user_approval) = match recovery_strategy {
        "restore_last_checkpoint" => (
            vec![
                "locate last board checkpoint asset".to_string(),
                "restore board state from checkpoint".to_string(),
                "resume from checkpoint sequence".to_string(),
            ],
            false,
        ),
        "replay_from_checkpoint" => (
            vec![
                "locate last board checkpoint".to_string(),
                "restore board state".to_string(),
                "replay player actions after checkpoint".to_string(),
                "verify board state consistency".to_string(),
            ],
            true,
        ),
        "restart_session" => (
            vec![
                "create new board session".to_string(),
                "re-initialize board from descriptor".to_string(),
                "notify user of restart".to_string(),
            ],
            true,
        ),
        "manual_intervention" => (
            vec![
                "pause board".to_string(),
                "present failure breadcrumbs to user".to_string(),
                "await user action".to_string(),
            ],
            true,
        ),
        "discard_and_reset" => (
            vec![
                "discard current board state".to_string(),
                "reset to initial board descriptor".to_string(),
                "archive failed session".to_string(),
            ],
            true,
        ),
        _ => (vec!["unknown strategy".to_string()], true),
    };

    Ok(serde_json::json!({
        "kind": "playable_creation_board_recovery_plan",
        "package_id": request.provider_package_id,
        "board_id": board_id,
        "failure_kind": failure_kind,
        "recommended_strategy": recommended_strategy,
        "recovery_strategy": recovery_strategy,
        "available_strategies": RECOVERY_STRATEGIES,
        "plan": {
            "steps": steps,
            "requires_user_approval": requires_user_approval,
            "checkpoint_available": checkpoint_available,
            "affected_asset_refs": request.input.get("asset_refs").cloned().unwrap_or(serde_json::json!([])),
        },
        "failure_kinds": FAILURE_KINDS,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn bind_agent_run(request: &InprocInvocation) -> anyhow::Result<Value> {
    // Raw-secret check
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "playable_creation_board_binding_rejected",
            "redaction_state": "unsafe_blocked",
            "reason": "input contains raw-secret-like content; use secret_ref references instead",
            "inference_performed": false,
            "network_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    let board_id = request
        .input
        .get("board_id")
        .and_then(Value::as_str)
        .unwrap_or("board:default");
    let agent_package_id = request
        .input
        .get("agent_package_id")
        .and_then(Value::as_str)
        .unwrap_or("official/agentic-forge-lab");
    let target_branch_ref = request
        .input
        .get("target_branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:target:default");
    let scratch_branch_ref = request
        .input
        .get("scratch_branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:scratch:default");

    let run_capabilities =
        request
            .input
            .get("run_capabilities")
            .cloned()
            .unwrap_or(serde_json::json!([
                "official/agentic-forge-lab/start_run",
                "official/agentic-forge-lab/create_candidate",
                "official/agentic-forge-lab/draft_promote_proposal"
            ]));

    Ok(serde_json::json!({
        "kind": "playable_creation_board_agent_run_binding",
        "package_id": request.provider_package_id,
        "board_id": board_id,
        "agent_package_id": agent_package_id,
        "run_capabilities": run_capabilities,
        "scoped_to_branch": true,
        "target_branch_ref": target_branch_ref,
        "scratch_branch_ref": scratch_branch_ref,
        "forge_panel_binding": {
            "surface_id": "official/playable-creation-board/forge-panel",
            "inspect_capabilities": [
                "official/playable-creation-board/describe_contract",
                "official/playable-creation-board/inspect_checkpoint",
                "official/playable-creation-board/explain_provenance"
            ],
            "proposal_capabilities": [
                "official/playable-creation-board/request_change",
                "official/playable-creation-board/draft_recovery"
            ],
            "branch_aware": true,
        },
        "assist_binding": {
            "surface_id": "official/playable-creation-board/assistant-action",
            "action_capabilities": [
                "official/playable-creation-board/request_change",
                "official/playable-creation-board/draft_recovery",
                "official/playable-creation-board/bind_agent_run"
            ],
            "approval_policy": "fork_then_approve",
        },
        "play_subscription": {
            "surface_id": "official/playable-creation-board/play-renderer",
            "subscription_type": "board_state_change",
        },
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn explain_provenance(request: &InprocInvocation) -> anyhow::Result<Value> {
    let board_id = request
        .input
        .get("board_id")
        .and_then(Value::as_str)
        .unwrap_or("board:default");
    let action_id = request
        .input
        .get("action_id")
        .and_then(Value::as_str)
        .unwrap_or("action:default:1");

    let checkpoint_ref = request
        .input
        .get("checkpoint_ref")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("checkpoint:{}:1", board_id));
    let agent_run_ref = request
        .input
        .get("agent_run_ref")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("run:forge:{}", board_id));
    let candidate_ref = request
        .input
        .get("candidate_ref")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("cand:forge:{}", board_id));
    let proposal_ref = request
        .input
        .get("proposal_ref")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .unwrap_or("proposal:default".to_string());

    let sequence = request
        .input
        .get("sequence")
        .and_then(Value::as_u64)
        .unwrap_or(1);

    // Compute content addresses for the chain artifacts
    let state_delta_ref = format!("asset:state_delta:{}:{}", board_id, sequence);
    let ca_action = crate::runtime::content_address(&format!("action:{}", action_id));
    let ca_delta = crate::runtime::content_address(&format!("delta:{}", state_delta_ref));

    // Build a causal chain: player_action → state_delta → checkpoint → agent_run → candidate → proposal → projection_rebuild
    let chain = serde_json::json!([
        {
            "step": "player_action_event",
            "ref": action_id,
            "content_address": ca_action,
            "description": "Player action recorded on the board"
        },
        {
            "step": "state_delta_asset",
            "ref": state_delta_ref,
            "content_address": ca_delta,
            "description": "State delta asset produced by the player action"
        },
        {
            "step": "checkpoint",
            "ref": checkpoint_ref,
            "content_address": crate::runtime::content_address(&format!("checkpoint:{}", checkpoint_ref)),
            "description": "Board checkpoint containing the accumulated state"
        },
        {
            "step": "agent_run",
            "ref": agent_run_ref,
            "content_address": crate::runtime::content_address(&format!("run:{}", agent_run_ref)),
            "description": "Agentic forge run started to evaluate requested change"
        },
        {
            "step": "candidate",
            "ref": candidate_ref,
            "content_address": crate::runtime::content_address(&format!("cand:{}", candidate_ref)),
            "description": "Candidate produced by agent run on scratch branch"
        },
        {
            "step": "proposal",
            "ref": proposal_ref,
            "content_address": crate::runtime::content_address(&format!("proposal:{}", proposal_ref)),
            "description": "Promote proposal drafted from candidate"
        },
        {
            "step": "projection_rebuild",
            "ref": format!("projection:board:{}:latest", board_id),
            "content_address": crate::runtime::content_address(&format!("projection:board:{}:latest", board_id)),
            "description": "Projection rebuild after proposal approval"
        }
    ]);

    // Build provenance graph (Beta 2)
    let provenance_graph = serde_json::json!({
        "nodes": [
            {"id": action_id, "kind": "player_action", "content_address": ca_action},
            {"id": state_delta_ref, "kind": "state_delta_asset", "content_address": ca_delta},
            {"id": checkpoint_ref, "kind": "checkpoint"},
            {"id": agent_run_ref, "kind": "agent_run"},
            {"id": candidate_ref, "kind": "candidate"},
            {"id": proposal_ref, "kind": "proposal"},
        ],
        "edges": [
            {"from": action_id, "to": state_delta_ref, "relation": "produces"},
            {"from": state_delta_ref, "to": checkpoint_ref, "relation": "accumulates_into"},
            {"from": checkpoint_ref, "to": agent_run_ref, "relation": "triggers"},
            {"from": agent_run_ref, "to": candidate_ref, "relation": "produces"},
            {"from": candidate_ref, "to": proposal_ref, "relation": "drafted_into"},
        ],
        "disclosure": request.input.get("disclosure").and_then(Value::as_str).unwrap_or("provenance_chain"),
    });

    Ok(serde_json::json!({
        "kind": "playable_creation_board_provenance_chain",
        "package_id": request.provider_package_id,
        "board_id": board_id,
        "action_id": action_id,
        "chain": chain,
        "provenance_graph": provenance_graph,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

// ---------------------------------------------------------------------------
// Beta 2 capabilities: preview_state_diff, describe_asset_provenance
// ---------------------------------------------------------------------------

fn preview_state_diff(request: &InprocInvocation) -> anyhow::Result<Value> {
    // Raw-secret check
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "playable_creation_board_state_diff_rejected",
            "redaction_state": "unsafe_blocked",
            "reason": "input contains raw-secret-like content; use secret_ref references instead",
            "inference_performed": false,
            "network_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    let board_id = request
        .input
        .get("board_id")
        .and_then(Value::as_str)
        .unwrap_or("board:default");
    let before_ref = request
        .input
        .get("before_snapshot_ref")
        .and_then(Value::as_str)
        .unwrap_or("");
    let after_ref = request
        .input
        .get("after_snapshot_ref")
        .and_then(Value::as_str)
        .unwrap_or("");
    let branch_ref = request
        .input
        .get("branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:default");

    // Deterministic diff summary
    let before_ca = crate::runtime::content_address(before_ref);
    let after_ca = crate::runtime::content_address(after_ref);
    let has_diff = before_ca != after_ca;

    Ok(serde_json::json!({
        "kind": "playable_creation_board_state_diff_preview",
        "package_id": request.provider_package_id,
        "board_id": board_id,
        "branch_ref": branch_ref,
        "before_snapshot_ref": if before_ref.is_empty() { Value::Null } else { serde_json::json!(before_ref) },
        "after_snapshot_ref": if after_ref.is_empty() { Value::Null } else { serde_json::json!(after_ref) },
        "before_content_address": before_ca,
        "after_content_address": after_ca,
        "diff_summary": if has_diff {
            format!("State changed between {} and {}", before_ref, after_ref)
        } else {
            "No state change detected".to_string()
        },
        "changed_asset_refs": request.input.get("changed_asset_refs").cloned().unwrap_or(serde_json::json!([])),
        "sequence": request.input.get("sequence").and_then(Value::as_u64).unwrap_or(1),
        "branch_aware": true,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id,
        }
    }))
}

fn describe_asset_provenance(request: &InprocInvocation) -> anyhow::Result<Value> {
    // Raw-secret check
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "playable_creation_board_provenance_describe_rejected",
            "redaction_state": "unsafe_blocked",
            "reason": "input contains raw-secret-like content; use secret_ref references instead",
            "inference_performed": false,
            "network_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    let board_id = request
        .input
        .get("board_id")
        .and_then(Value::as_str)
        .unwrap_or("board:default");
    let asset_ref = request
        .input
        .get("asset_ref")
        .and_then(Value::as_str)
        .unwrap_or("");
    let disclosure = request
        .input
        .get("disclosure")
        .and_then(Value::as_str)
        .unwrap_or("provenance_describe");

    let ca = crate::runtime::content_address(asset_ref);

    Ok(serde_json::json!({
        "kind": "playable_creation_board_asset_provenance",
        "package_id": request.provider_package_id,
        "board_id": board_id,
        "asset_ref": if asset_ref.is_empty() { Value::Null } else { serde_json::json!(asset_ref) },
        "content_address": ca,
        "disclosure": disclosure,
        "provenance_graph": {
            "asset_ref": if asset_ref.is_empty() { Value::Null } else { serde_json::json!(asset_ref) },
            "content_address": ca,
            "source_refs": request.input.get("source_refs").cloned().unwrap_or(serde_json::json!([])),
            "derived_refs": request.input.get("derived_refs").cloned().unwrap_or(serde_json::json!([])),
            "branch_ref": request.input.get("branch_ref").cloned().unwrap_or(Value::Null),
            "proposal_ref": request.input.get("proposal_ref").cloned().unwrap_or(Value::Null),
            "inference_ref": request.input.get("inference_ref").cloned().unwrap_or(Value::Null),
            "disclosure": disclosure,
            "large_output_policy": request.input.get("large_output_policy").and_then(Value::as_str).unwrap_or("object_ref"),
        },
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id,
        }
    }))
}

// ---------------------------------------------------------------------------
// Beta 3 capability: summarize_experience_health (observability linkage)
// ---------------------------------------------------------------------------

fn summarize_experience_health(request: &InprocInvocation) -> anyhow::Result<Value> {
    // Raw-secret check
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "playable_creation_board_health_rejected",
            "redaction_state": "unsafe_blocked",
            "reason": "input contains raw-secret-like content; use secret_ref references instead",
            "inference_performed": false,
            "network_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    let board_id = request
        .input
        .get("board_id")
        .and_then(Value::as_str)
        .unwrap_or("board:default");

    let sequence = request
        .input
        .get("sequence")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let checkpoint_count = request
        .input
        .get("checkpoint_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let proposal_count = request
        .input
        .get("proposal_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let failure_count = request
        .input
        .get("failure_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    let status = if failure_count > 0 {
        "degraded"
    } else if sequence == 0 {
        "created"
    } else {
        "running"
    };

    // Link to observability package (ref, not invoke)
    let observability_ref = serde_json::json!({
        "package_id": "official/experience-observability-lab",
        "session_health_capability": "official/experience-observability-lab/summarize_session_health",
        "failure_breadcrumbs_capability": "official/experience-observability-lab/list_failure_breadcrumbs",
    });

    Ok(serde_json::json!({
        "kind": "playable_creation_board_experience_health",
        "package_id": request.provider_package_id,
        "board_id": board_id,
        "status": status,
        "sequence": sequence,
        "checkpoint_count": checkpoint_count,
        "proposal_count": proposal_count,
        "failure_count": failure_count,
        "observability_refs": observability_ref,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id,
        }
    }))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_request(cap: &str, input: Value) -> InprocInvocation {
        InprocInvocation {
            capability_id: cap.to_string(),
            provider_package_id: PACKAGE_ID.to_string(),
            session_id: None,
            input,
        }
    }

    #[test]
    fn try_handle_matches_package_id() {
        let req = make_request(
            "official/playable-creation-board/describe_contract",
            json!({}),
        );
        assert!(try_handle(&req).is_some());
    }

    #[test]
    fn try_handle_rejects_wrong_package() {
        let req = InprocInvocation {
            capability_id: "official/playable-creation-board/describe_contract".to_string(),
            provider_package_id: "official/other".to_string(),
            session_id: None,
            input: json!({}),
        };
        assert!(try_handle(&req).is_none());
    }

    #[test]
    fn describe_contract_has_all_surfaces() {
        let req = make_request(
            "official/playable-creation-board/describe_contract",
            json!({}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        let surfaces = result["surfaces"].as_object().unwrap();
        assert!(surfaces.contains_key("experience_entry"));
        assert!(surfaces.contains_key("play_renderer"));
        assert!(surfaces.contains_key("forge_panel"));
        assert!(surfaces.contains_key("assistant_action"));
    }

    #[test]
    fn describe_contract_lists_14_capabilities() {
        let req = make_request(
            "official/playable-creation-board/describe_contract",
            json!({}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["capabilities"]
                .as_array()
                .map(|a| a.len())
                .unwrap_or(0),
            14,
            "must list 14 capabilities"
        );
    }

    #[test]
    fn launch_returns_board_id() {
        let req = make_request(
            "official/playable-creation-board/launch",
            json!({
                "board_id": "board:test",
                "title": "Test Board",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("playable_creation_board_launched"));
        assert_eq!(result["board_id"], json!("board:test"));
        assert_eq!(result["lifecycle_state"], json!("created"));
    }

    #[test]
    fn record_player_action_returns_state_delta() {
        let req = make_request(
            "official/playable-creation-board/record_player_action",
            json!({
                "board_id": "board:test",
                "action_kind": "place_marker",
                "sequence": 1,
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["kind"],
            json!("playable_creation_board_action_recorded")
        );
        assert_eq!(result["action_kind"], json!("place_marker"));
        assert!(result["state_delta_asset_ref"].is_string());
        assert!(result["projection_ref"].is_string());
        assert_eq!(result["sequence"], json!(1));
    }

    #[test]
    fn request_change_returns_agent_objective() {
        let req = make_request(
            "official/playable-creation-board/request_change",
            json!({
                "board_id": "board:test",
                "objective": "add a new module to the board",
                "change_kind": "add_module",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["kind"],
            json!("playable_creation_board_change_request")
        );
        assert_eq!(result["objective"], json!("add a new module to the board"));
        assert!(result["allowed_change_kinds"].is_array());
        assert!(result["risk"].is_string());
        assert!(result["budget"].is_number());
        assert!(result["bindable_refs"].is_object());
        assert!(result["agent_run_binding"].is_object());
        assert_eq!(result["requires_user_approval"], json!(true));
    }

    #[test]
    fn create_checkpoint_returns_deterministic() {
        let req = make_request(
            "official/playable-creation-board/create_checkpoint",
            json!({"board_id": "board:test", "state_snapshot": {"modules": 3}, "sequence": 2}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("playable_creation_board_checkpoint"));
        assert_eq!(result["board_id"], json!("board:test"));
        assert_eq!(result["format"], json!("snapshot"));
        assert_eq!(result["sequence"], json!(2));
    }

    #[test]
    fn draft_recovery_returns_plan() {
        let req = make_request(
            "official/playable-creation-board/draft_recovery",
            json!({"board_id": "board:test", "failure_kind": "constraint_violation", "last_checkpoint_ref": "cp:1"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["kind"],
            json!("playable_creation_board_recovery_plan")
        );
        assert_eq!(
            result["recommended_strategy"],
            json!("restore_last_checkpoint")
        );
    }

    #[test]
    fn bind_agent_run_returns_scoped_binding() {
        let req = make_request(
            "official/playable-creation-board/bind_agent_run",
            json!({"board_id": "board:test", "agent_package_id": "official/agentic-forge-lab"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["kind"],
            json!("playable_creation_board_agent_run_binding")
        );
        assert_eq!(result["scoped_to_branch"], json!(true));
        assert_eq!(result["forge_panel_binding"]["branch_aware"], json!(true));
        assert_eq!(
            result["assist_binding"]["approval_policy"],
            json!("fork_then_approve")
        );
    }

    #[test]
    fn explain_provenance_returns_causal_chain() {
        let req = make_request(
            "official/playable-creation-board/explain_provenance",
            json!({"board_id": "board:test", "action_id": "action:board:test:1"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["kind"],
            json!("playable_creation_board_provenance_chain")
        );
        let chain = result["chain"].as_array().unwrap();
        assert_eq!(chain.len(), 7);
        assert_eq!(chain[0]["step"], json!("player_action_event"));
        assert_eq!(chain[6]["step"], json!("projection_rebuild"));
    }

    #[test]
    fn create_checkpoint_blocks_raw_secret() {
        let req = make_request(
            "official/playable-creation-board/create_checkpoint",
            json!({"board_id": "test", "api_key": "RawSecretExample1234567890abcdefABCDEF123456"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["kind"],
            json!("playable_creation_board_checkpoint_rejected")
        );
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn record_player_action_blocks_raw_secret() {
        let req = make_request(
            "official/playable-creation-board/record_player_action",
            json!({"board_id": "test", "token": "Bearer abc123"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["kind"],
            json!("playable_creation_board_action_rejected")
        );
    }

    #[test]
    fn no_forbidden_kernel_namespace_in_output() {
        let caps = [
            "official/playable-creation-board/describe_contract",
            "official/playable-creation-board/launch",
            "official/playable-creation-board/project_state",
            "official/playable-creation-board/render_payload",
            "official/playable-creation-board/record_player_action",
            "official/playable-creation-board/request_change",
            "official/playable-creation-board/create_checkpoint",
            "official/playable-creation-board/inspect_checkpoint",
            "official/playable-creation-board/draft_recovery",
            "official/playable-creation-board/bind_agent_run",
            "official/playable-creation-board/explain_provenance",
        ];
        let forbidden = [
            "kernel.v1.experience.",
            "kernel.v1.world.",
            "kernel.v1.scene.",
            "kernel.v1.character.",
            "kernel.v1.turn.",
            "kernel.v1.chat.",
            "kernel.v1.memory.",
            "kernel.v1.agent.",
            "kernel.v1.model.",
            "kernel.v1.prompt.",
            "kernel.v1.director.",
        ];
        for cap in &caps {
            let req = make_request(cap, json!({"board_id": "test"}));
            let result = try_handle(&req).unwrap().unwrap();
            let output_str = serde_json::to_string(&result).unwrap();
            for token in &forbidden {
                assert!(
                    !output_str.contains(token),
                    "{} must not contain {}",
                    cap,
                    token
                );
            }
        }
    }

    #[test]
    fn contains_raw_secret_detects_known_patterns() {
        assert!(safety::contains_raw_secret(
            &json!({"api_key": "RawSecretExample1234567890abcdefABCDEF123456"})
        ));
        assert!(safety::contains_raw_secret(&json!({"token": "Bearer xyz"})));
        assert!(!safety::contains_raw_secret(
            &json!({"api_key": "secret_ref:env:MY_KEY"})
        ));
        assert!(!safety::contains_raw_secret(
            &json!({"objective": "safe text"})
        ));
    }

    #[test]
    fn no_chat_or_conversation_terminology_in_output() {
        let req = make_request(
            "official/playable-creation-board/request_change",
            json!({
                "board_id": "board:test",
                "objective": "add module",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        let output_str = serde_json::to_string(&result).unwrap();
        assert!(!output_str.contains("assistant_message"));
        assert!(!output_str.contains("conversation"));
        assert!(!output_str.contains("prompt_transcript"));
        assert!(!output_str.contains("chat_message"));
    }

    // -----------------------------------------------------------------------
    // Beta 2 unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn record_player_action_includes_content_address() {
        let req = make_request(
            "official/playable-creation-board/record_player_action",
            json!({
                "board_id": "board:b2",
                "action_kind": "place_marker",
                "sequence": 1,
                "payload": {"marker_id": "m1"},
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["kind"],
            json!("playable_creation_board_action_recorded")
        );
        assert!(
            result["content_address"].is_string(),
            "must have content_address"
        );
        assert!(result["content_address"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
        assert!(
            result["state_snapshot_asset_ref"].is_string(),
            "must have state_snapshot_asset_ref"
        );
        assert!(result["disclosure"].is_string(), "must have disclosure");
    }

    #[test]
    fn create_checkpoint_includes_content_address() {
        let req = make_request(
            "official/playable-creation-board/create_checkpoint",
            json!({
                "board_id": "board:b2",
                "state_snapshot": {"markers": 1},
                "sequence": 1,
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("playable_creation_board_checkpoint"));
        assert!(
            result["content_address"].is_string(),
            "must have content_address"
        );
        assert!(result["content_address"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
        assert!(
            result["state_snapshot_asset_ref"].is_string(),
            "must have state_snapshot_asset_ref"
        );
        assert!(result["disclosure"].is_string(), "must have disclosure");
        assert!(result["provenance"]["content_address"].is_string());
    }

    #[test]
    fn explain_provenance_includes_content_address_and_graph() {
        let req = make_request(
            "official/playable-creation-board/explain_provenance",
            json!({
                "board_id": "board:b2",
                "action_id": "action:board:b2:1",
                "sequence": 1,
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["kind"],
            json!("playable_creation_board_provenance_chain")
        );
        let chain = result["chain"].as_array().unwrap();
        // Each chain step should now have content_address
        for step in chain {
            assert!(
                step["content_address"].is_string(),
                "chain step must have content_address"
            );
        }
        // Must have provenance_graph
        assert!(
            result["provenance_graph"].is_object(),
            "must have provenance_graph"
        );
        assert!(result["provenance_graph"]["nodes"].is_array());
        assert!(result["provenance_graph"]["edges"].is_array());
    }

    #[test]
    fn preview_state_diff_returns_branch_aware_shape() {
        let req = make_request(
            "official/playable-creation-board/preview_state_diff",
            json!({
                "board_id": "board:b2",
                "before_snapshot_ref": "asset:state_snapshot:board:b2:1",
                "after_snapshot_ref": "asset:state_snapshot:board:b2:2",
                "branch_ref": "branch:target:default",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["kind"],
            json!("playable_creation_board_state_diff_preview")
        );
        assert_eq!(result["branch_aware"], json!(true));
        assert!(result["before_content_address"].is_string());
        assert!(result["after_content_address"].is_string());
        assert!(result["diff_summary"].is_string());
        assert_eq!(result["inference_performed"], json!(false));
    }

    #[test]
    fn describe_asset_provenance_returns_graph() {
        let req = make_request(
            "official/playable-creation-board/describe_asset_provenance",
            json!({
                "board_id": "board:b2",
                "asset_ref": "asset:state_delta:board:b2:1",
                "disclosure": "debug",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["kind"],
            json!("playable_creation_board_asset_provenance")
        );
        assert!(result["content_address"].is_string());
        assert!(result["provenance_graph"].is_object());
        assert!(result["provenance_graph"]["disclosure"].is_string());
        assert_eq!(result["inference_performed"], json!(false));
    }

    #[test]
    fn content_address_deterministic() {
        let content = "deterministic test content for beta 2";
        let ca1 = crate::runtime::content_address(content);
        let ca2 = crate::runtime::content_address(content);
        assert_eq!(ca1, ca2, "content address must be deterministic");
        assert!(ca1.starts_with("sha256:"));
    }

    #[test]
    fn content_address_different_for_different_content() {
        let ca1 = crate::runtime::content_address("content a");
        let ca2 = crate::runtime::content_address("content b");
        assert_ne!(
            ca1, ca2,
            "different content must produce different addresses"
        );
    }

    #[test]
    fn preview_state_diff_blocks_raw_secret() {
        let req = make_request(
            "official/playable-creation-board/preview_state_diff",
            json!({
                "board_id": "board:b2",
                "api_key": "RawSecretExample1234567890abcdefABCDEF123456",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["kind"],
            json!("playable_creation_board_state_diff_rejected")
        );
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn describe_asset_provenance_blocks_raw_secret() {
        let req = make_request(
            "official/playable-creation-board/describe_asset_provenance",
            json!({
                "board_id": "board:b2",
                "token": "Bearer abc123",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["kind"],
            json!("playable_creation_board_provenance_describe_rejected")
        );
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn no_forbidden_kernel_namespace_in_beta2_output() {
        let beta2_caps = [
            "official/playable-creation-board/preview_state_diff",
            "official/playable-creation-board/describe_asset_provenance",
        ];
        let forbidden = [
            "kernel.v1.experience.",
            "kernel.v1.world.",
            "kernel.v1.scene.",
            "kernel.v1.character.",
            "kernel.v1.turn.",
            "kernel.v1.chat.",
            "kernel.v1.memory.",
            "kernel.v1.agent.",
            "kernel.v1.model.",
            "kernel.v1.prompt.",
            "kernel.v1.director.",
            "kernel.v1.state.",
        ];
        for cap in &beta2_caps {
            let req = make_request(cap, json!({"board_id": "test"}));
            let result = try_handle(&req).unwrap().unwrap();
            let output_str = serde_json::to_string(&result).unwrap();
            for token in &forbidden {
                assert!(
                    !output_str.contains(token),
                    "{} must not contain {}",
                    cap,
                    token
                );
            }
        }
    }
}
