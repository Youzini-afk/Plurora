//! Handler for `official/experience-runtime-lab` capabilities.
//!
//! Experience Beta 0 — Thin Experience Runtime Contract.
//!
//! Package-owned experience descriptor, state projection, checkpoint,
//! recovery, and Play/Forge/Assist surface bindings. Deterministic,
//! no-network, no real model inference.
//!
//! No `kernel.v1.experience.*`, `kernel.v1.world.*`, `kernel.v1.turn.*`,
//! `kernel.v1.chat.*`, or `kernel.v1.memory.*` namespace references.

use serde_json::Value;

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/experience-runtime-lab";

// ---------------------------------------------------------------------------
// Lifecycle states
// ---------------------------------------------------------------------------

const LIFECYCLE_STATES: &[&str] = &[
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
    "capability_failure",
    "session_expired",
    "resource_exhausted",
    "package_error",
    "unknown",
];

// ---------------------------------------------------------------------------
// Raw-secret detection (delegated to shared safety module)
// ---------------------------------------------------------------------------

use super::safety;

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
    } else if id.ends_with("/create_checkpoint") {
        Some(create_checkpoint(request))
    } else if id.ends_with("/inspect_checkpoint") {
        Some(inspect_checkpoint(request))
    } else if id.ends_with("/draft_recovery") {
        Some(draft_recovery(request))
    } else if id.ends_with("/bind_agent_run") {
        Some(bind_agent_run(request))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Capability implementations
// ---------------------------------------------------------------------------

fn describe_contract(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "experience_runtime_contract",
        "package_id": request.provider_package_id,
        "package_kind": "ordinary",
        "capabilities": [
            {"id": "official/experience-runtime-lab/describe_contract", "purpose": "describe the experience runtime package contract"},
            {"id": "official/experience-runtime-lab/create_checkpoint", "purpose": "create a deterministic experience checkpoint asset"},
            {"id": "official/experience-runtime-lab/inspect_checkpoint", "purpose": "inspect a checkpoint's shape and validity"},
            {"id": "official/experience-runtime-lab/draft_recovery", "purpose": "draft a recovery plan for a failed experience session"},
            {"id": "official/experience-runtime-lab/bind_agent_run", "purpose": "bind an agentic forge run to an experience session"},
        ],
        "surfaces": {
            "experience_entry": "official/experience-runtime-lab/entry",
            "play_renderer": "official/experience-runtime-lab/play",
            "forge_panel": "official/experience-runtime-lab/forge",
            "assistant_action": "official/experience-runtime-lab/assist",
        },
        "lifecycle_states": LIFECYCLE_STATES,
        "checkpoint_formats": CHECKPOINT_FORMATS,
        "recovery_strategies": RECOVERY_STRATEGIES,
        "failure_kinds": FAILURE_KINDS,
        "checkpoint_fields": [
            "checkpoint_id", "session_id", "format", "state_snapshot",
            "asset_refs", "branch_ref", "sequence",
            "inference_performed", "network_performed", "provenance"
        ],
        "recovery_fields": [
            "failure_kind", "failure_detail", "last_checkpoint_ref",
            "recovery_strategy", "recovery_plan", "inference_performed",
            "network_performed", "provenance"
        ],
        "forge_binding_fields": [
            "session_id", "surface_id", "inspect_capabilities",
            "proposal_capabilities", "branch_aware",
            "inference_performed", "network_performed", "provenance"
        ],
        "assist_binding_fields": [
            "session_id", "surface_id", "action_capabilities",
            "approval_policy", "inference_performed",
            "network_performed", "provenance"
        ],
        "play_subscription_fields": [
            "session_id", "surface_id", "subscription_type",
            "filter", "inference_performed",
            "network_performed", "provenance"
        ],
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
            "kind": "experience_checkpoint_rejected",
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

    let session_id = request
        .input
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("session_default");
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

    let checkpoint_id = format!("checkpoint:{}:{}", session_id, sequence);

    Ok(serde_json::json!({
        "kind": "experience_checkpoint",
        "package_id": request.provider_package_id,
        "session_id": session_id,
        "checkpoint_id": checkpoint_id,
        "format": format,
        "state_snapshot": state_snapshot,
        "asset_refs": asset_refs,
        "branch_ref": branch_ref,
        "sequence": sequence,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn inspect_checkpoint(request: &InprocInvocation) -> anyhow::Result<Value> {
    let checkpoint_id = request
        .input
        .get("checkpoint_id")
        .and_then(Value::as_str)
        .unwrap_or("checkpoint_unknown");
    let session_id = request
        .input
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("session_default");
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
    if session_id.is_empty() {
        errors.push("session_id must be non-empty".to_string());
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
        "kind": "experience_checkpoint_inspection",
        "checkpoint_id": checkpoint_id,
        "session_id": session_id,
        "valid": valid,
        "errors": errors,
        "format": format,
        "sequence": sequence,
        "asset_count": asset_count,
        "summary": if valid {
            format!("Checkpoint {} valid (format={}, sequence={}, assets={})", checkpoint_id, format, sequence, asset_count)
        } else {
            format!("Checkpoint {} invalid: {}", checkpoint_id, errors.join("; "))
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
            "kind": "experience_recovery_rejected",
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

    let session_id = request
        .input
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("session_default");
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
                "locate last checkpoint asset".to_string(),
                "restore state from checkpoint".to_string(),
                "resume from checkpoint sequence".to_string(),
            ],
            false,
        ),
        "replay_from_checkpoint" => (
            vec![
                "locate last checkpoint".to_string(),
                "restore state".to_string(),
                "replay events after checkpoint".to_string(),
                "verify replay consistency".to_string(),
            ],
            true,
        ),
        "restart_session" => (
            vec![
                "create new session".to_string(),
                "re-initialize state from descriptor".to_string(),
                "notify user of restart".to_string(),
            ],
            true,
        ),
        "manual_intervention" => (
            vec![
                "pause experience".to_string(),
                "present failure breadcrumbs to user".to_string(),
                "await user action".to_string(),
            ],
            true,
        ),
        "discard_and_reset" => (
            vec![
                "discard current state".to_string(),
                "reset to initial descriptor state".to_string(),
                "archive failed session".to_string(),
            ],
            true,
        ),
        _ => (vec!["unknown strategy".to_string()], true),
    };

    Ok(serde_json::json!({
        "kind": "experience_recovery_plan",
        "package_id": request.provider_package_id,
        "session_id": session_id,
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
            "kind": "experience_agent_run_binding_rejected",
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

    let session_id = request
        .input
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("session_default");
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
        "kind": "experience_agent_run_binding",
        "package_id": request.provider_package_id,
        "session_id": session_id,
        "agent_package_id": agent_package_id,
        "run_capabilities": run_capabilities,
        "scoped_to_branch": true,
        "target_branch_ref": target_branch_ref,
        "scratch_branch_ref": scratch_branch_ref,
        "forge_panel_binding": {
            "surface_id": "official/experience-runtime-lab/forge",
            "inspect_capabilities": [
                "official/experience-runtime-lab/describe_contract",
                "official/experience-runtime-lab/inspect_checkpoint"
            ],
            "proposal_capabilities": [
                "official/experience-runtime-lab/draft_recovery"
            ],
            "branch_aware": true,
        },
        "assist_binding": {
            "surface_id": "official/experience-runtime-lab/assist",
            "action_capabilities": [
                "official/experience-runtime-lab/draft_recovery",
                "official/experience-runtime-lab/bind_agent_run"
            ],
            "approval_policy": "fork_then_approve",
        },
        "play_subscription": {
            "surface_id": "official/experience-runtime-lab/play",
            "subscription_type": "state_change",
        },
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
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
            "official/experience-runtime-lab/describe_contract",
            json!({}),
        );
        assert!(try_handle(&req).is_some());
    }

    #[test]
    fn try_handle_rejects_wrong_package() {
        let req = InprocInvocation {
            capability_id: "official/experience-runtime-lab/describe_contract".to_string(),
            provider_package_id: "official/other".to_string(),
            session_id: None,
            input: json!({}),
        };
        assert!(try_handle(&req).is_none());
    }

    #[test]
    fn describe_contract_returns_lifecycle_states() {
        let req = make_request(
            "official/experience-runtime-lab/describe_contract",
            json!({}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        let states = result["lifecycle_states"].as_array().unwrap();
        assert_eq!(states.len(), 9);
        assert_eq!(states[0], json!("created"));
        assert_eq!(states[8], json!("archived"));
    }

    #[test]
    fn describe_contract_has_surfaces() {
        let req = make_request(
            "official/experience-runtime-lab/describe_contract",
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
    fn create_checkpoint_returns_deterministic() {
        let req = make_request(
            "official/experience-runtime-lab/create_checkpoint",
            json!({"session_id": "session_test", "state_snapshot": {"health": 100}, "asset_refs": ["asset:module:seed"]}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("experience_checkpoint"));
        assert_eq!(result["session_id"], json!("session_test"));
        assert_eq!(result["format"], json!("snapshot"));
        assert_eq!(result["inference_performed"], json!(false));
    }

    #[test]
    fn create_checkpoint_blocks_raw_secret() {
        let req = make_request(
            "official/experience-runtime-lab/create_checkpoint",
            json!({"session_id": "test", "api_key": "RawSecretExample1234567890abcdefABCDEF123456"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("experience_checkpoint_rejected"));
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn inspect_checkpoint_valid() {
        let req = make_request(
            "official/experience-runtime-lab/inspect_checkpoint",
            json!({"checkpoint_id": "cp:1", "session_id": "s:1", "state_snapshot": {"x": 1}, "format": "snapshot", "sequence": 1}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("experience_checkpoint_inspection"));
        assert_eq!(result["valid"], json!(true));
    }

    #[test]
    fn draft_recovery_returns_plan() {
        let req = make_request(
            "official/experience-runtime-lab/draft_recovery",
            json!({"session_id": "session_test", "failure_kind": "state_corruption", "last_checkpoint_ref": "cp:1"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("experience_recovery_plan"));
        assert_eq!(
            result["recommended_strategy"],
            json!("restore_last_checkpoint")
        );
        assert_eq!(result["inference_performed"], json!(false));
    }

    #[test]
    fn draft_recovery_no_checkpoint_recommends_restart() {
        let req = make_request(
            "official/experience-runtime-lab/draft_recovery",
            json!({"session_id": "session_test", "failure_kind": "checkpoint_missing"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["recommended_strategy"], json!("restart_session"));
    }

    #[test]
    fn bind_agent_run_returns_binding() {
        let req = make_request(
            "official/experience-runtime-lab/bind_agent_run",
            json!({"session_id": "session_test", "agent_package_id": "official/agentic-forge-lab"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("experience_agent_run_binding"));
        assert_eq!(result["scoped_to_branch"], json!(true));
        assert_eq!(result["inference_performed"], json!(false));
    }

    #[test]
    fn no_kernel_experience_namespace_in_output() {
        let caps = [
            "official/experience-runtime-lab/describe_contract",
            "official/experience-runtime-lab/create_checkpoint",
            "official/experience-runtime-lab/inspect_checkpoint",
            "official/experience-runtime-lab/draft_recovery",
            "official/experience-runtime-lab/bind_agent_run",
        ];
        for cap in &caps {
            let req = make_request(cap, json!({"session_id": "test"}));
            let result = try_handle(&req).unwrap().unwrap();
            let output_str = serde_json::to_string(&result).unwrap();
            assert!(
                !output_str.contains("kernel.v1.experience."),
                "{} must not contain kernel.v1.experience.",
                cap
            );
            assert!(
                !output_str.contains("kernel.v1.world."),
                "{} must not contain kernel.v1.world.",
                cap
            );
            assert!(
                !output_str.contains("kernel.v1.turn."),
                "{} must not contain kernel.v1.turn.",
                cap
            );
            assert!(
                !output_str.contains("kernel.v1.chat."),
                "{} must not contain kernel.v1.chat.",
                cap
            );
            assert!(
                !output_str.contains("kernel.v1.memory."),
                "{} must not contain kernel.v1.memory.",
                cap
            );
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
}
