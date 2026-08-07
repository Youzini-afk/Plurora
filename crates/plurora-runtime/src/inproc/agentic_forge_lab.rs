//! Handler for `official/agentic-forge-lab` capabilities.
//!
//! Phase A of Agentic Forge Beta: package-owned agent run lifecycle,
//! working state, plan graph contract. Deterministic, no-network,
//! no real model inference.
//!
//! Phase B: branch-aware scratch branch / candidate / compare / promote proof.
//! Candidates never mutate target branch; promote only produces proposal drafts.
//! Stale target branch detection (revision mismatch) blocks promote.
//!
//! Phase C: inference-backed agent run with deterministic fallback.
//! Model output can only produce candidate/proposal seeds, never escalate
//! privileges or auto-promote. Cloud adapter plan returns needs_host_policy,
//! no network performed. Replay mismatches are flagged, never silently passed.

use serde_json::Value;

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/agentic-forge-lab";

// ---------------------------------------------------------------------------
// Lifecycle states
// ---------------------------------------------------------------------------

const LIFECYCLE_STATES: &[&str] = &[
    "created",
    "prepared",
    "running",
    "paused",
    "waiting_for_approval",
    "completed",
    "failed",
    "cancelled",
    "archived",
];

fn is_valid_lifecycle_state(s: &str) -> bool {
    LIFECYCLE_STATES.contains(&s)
}

// ---------------------------------------------------------------------------
// Candidate states
// ---------------------------------------------------------------------------

const CANDIDATE_STATES: &[&str] = &[
    "draft",
    "ready",
    "comparing",
    "promoting",
    "promoted",
    "rejected",
    "archived",
    "failed",
];

#[cfg(test)]
fn is_valid_candidate_state(s: &str) -> bool {
    CANDIDATE_STATES.contains(&s)
}

// ---------------------------------------------------------------------------
// Plan node kinds (Phase C: explicit coverage)
// ---------------------------------------------------------------------------

const PLAN_NODE_KINDS: &[&str] = &[
    "observe",
    "infer",
    "tool_call",
    "inspect",
    "branch_op",
    "compare",
    "propose",
    "wait",
];

// ---------------------------------------------------------------------------
// Inference provider kinds
// ---------------------------------------------------------------------------

const PROVIDER_KINDS: &[&str] = &[
    "deterministic",
    "recorded",
    "cloud_adapter_plan",
    "local_fake",
];

// ---------------------------------------------------------------------------
// Inference failure taxonomy
// ---------------------------------------------------------------------------

const INFERENCE_FAILURE_KINDS: &[&str] = &[
    "rate_limit",
    "quota",
    "timeout",
    "auth",
    "network_denied",
    "invalid_output",
    "malformed_output",
    "replay_mismatch",
    "policy_reject",
];

// ---------------------------------------------------------------------------
// Allowed inference output actions (Phase C safety boundary)
// ---------------------------------------------------------------------------

const ALLOWED_INFERENCE_ACTIONS: &[&str] = &[
    "candidate_seed",
    "proposal_seed",
    "observation",
    "needs_repair",
];

const FORBIDDEN_INFERENCE_ACTIONS: &[&str] = &[
    "privilege_escalation",
    "auto_promote",
    "secret_request",
    "target_branch_write",
    "unknown_action",
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
    } else if id.ends_with("/start_run") {
        Some(start_run(request))
    } else if id.ends_with("/inspect_run") {
        Some(inspect_run(request))
    } else if id.ends_with("/cancel_run") {
        Some(cancel_run(request))
    } else if id.ends_with("/summarize_run") {
        Some(summarize_run(request))
    } else if id.ends_with("/export_plan_graph") {
        Some(export_plan_graph(request))
    } else if id.ends_with("/create_candidate") {
        Some(create_candidate(request))
    } else if id.ends_with("/compare_candidate") {
        Some(compare_candidate(request))
    } else if id.ends_with("/draft_promote_proposal") {
        Some(draft_promote_proposal(request))
    } else if id.ends_with("/archive_candidate") {
        Some(archive_candidate(request))
    } else if id.ends_with("/explain_branch_policy") {
        Some(explain_branch_policy(request))
    } else if id.ends_with("/run_inference_node") {
        Some(run_inference_node(request))
    } else if id.ends_with("/replay_inference_node") {
        Some(replay_inference_node(request))
    } else if id.ends_with("/validate_inference_output") {
        Some(validate_inference_output(request))
    } else if id.ends_with("/explain_inference_failure") {
        Some(explain_inference_failure(request))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Capability implementations
// ---------------------------------------------------------------------------

fn describe_contract(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "agentic_forge_contract",
        "package_id": request.provider_package_id,
        "package_kind": "ordinary",
        "capabilities": [
            {"id": "official/agentic-forge-lab/describe_contract", "purpose": "describe the agentic forge package contract"},
            {"id": "official/agentic-forge-lab/start_run", "purpose": "start a deterministic agent run with plan graph and working state"},
            {"id": "official/agentic-forge-lab/inspect_run", "purpose": "inspect an existing run's working state and lifecycle"},
            {"id": "official/agentic-forge-lab/cancel_run", "purpose": "cancel a running or paused run"},
            {"id": "official/agentic-forge-lab/summarize_run", "purpose": "produce an observability summary of a run"},
            {"id": "official/agentic-forge-lab/export_plan_graph", "purpose": "export the plan graph artifact of a run"},
            {"id": "official/agentic-forge-lab/create_candidate", "purpose": "create a branch-aware candidate from a scratch branch, never write target"},
            {"id": "official/agentic-forge-lab/compare_candidate", "purpose": "compare scratch vs target branch diff summary with stale detection"},
            {"id": "official/agentic-forge-lab/draft_promote_proposal", "purpose": "draft a promote proposal without direct target mutation; stale target blocked"},
            {"id": "official/agentic-forge-lab/archive_candidate", "purpose": "archive a candidate without modifying target branch"},
            {"id": "official/agentic-forge-lab/explain_branch_policy", "purpose": "explain the scratch/target branch policy and promote constraints"},
            {"id": "official/agentic-forge-lab/run_inference_node", "purpose": "run an inference node with deterministic/recorded/cloud_adapter_plan provider; produces candidate/proposal seeds only"},
            {"id": "official/agentic-forge-lab/replay_inference_node", "purpose": "replay a recorded inference output; mismatch flagged, never silently passed"},
            {"id": "official/agentic-forge-lab/validate_inference_output", "purpose": "validate inference output action allowlist; reject privilege_escalation/auto_promote/secret_request/target_branch_write/unknown_action"},
            {"id": "official/agentic-forge-lab/explain_inference_failure", "purpose": "explain inference failure taxonomy with recovery hints"},
        ],
        "lifecycle_states": LIFECYCLE_STATES,
        "candidate_states": CANDIDATE_STATES,
        "plan_node_kinds": PLAN_NODE_KINDS,
        "provider_kinds": PROVIDER_KINDS,
        "inference_failure_taxonomy": INFERENCE_FAILURE_KINDS,
        "allowed_inference_actions": ALLOWED_INFERENCE_ACTIONS,
        "plan_graph_fields": [
            "nodes", "edges", "status", "revision", "input_refs",
            "output_refs", "approval_policy", "retry_policy", "deterministic_mode"
        ],
        "working_state_fields": [
            "run_id", "owner_package", "target_branch_ref", "scratch_branch_ref",
            "current_objective", "local_context_refs", "plan_graph_ref",
            "candidate_refs", "tool_observation_refs", "inference_trace_refs",
            "policy_state"
        ],
        "candidate_fields": [
            "candidate_id", "run_id", "target_branch_ref", "scratch_branch_ref",
            "changed_asset_refs", "projection_refs", "diff_summary",
            "inspection_refs", "confidence", "uncertainty", "provenance", "status"
        ],
        "branch_policy": {
            "default_scratch_intent": "explore_without_mutating_target",
            "promote_requires_proposal": true,
            "stale_target_blocks_promote": true,
            "target_revision_must_match": true
        },
        "run_constraints": {
            "max_steps_supported": true,
            "deadline_ms_supported": true,
            "budget_required_for_run": false,
            "budget_diagnosed_if_missing": true,
            "cancellation_consistent": true
        },
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn start_run(request: &InprocInvocation) -> anyhow::Result<Value> {
    // Check for raw-secret-like content in input
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "agentic_forge_run_rejected",
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

    let run_id = format!("run_{}", deterministic_id(&request.input));
    let objective = request
        .input
        .get("objective")
        .and_then(Value::as_str)
        .unwrap_or("deterministic agentic forge run");

    let plan_graph = build_plan_graph(&run_id, objective);
    let working_state = build_working_state(&run_id, &request.provider_package_id, &request.input);

    Ok(serde_json::json!({
        "kind": "agentic_forge_run_started",
        "run_id": run_id,
        "lifecycle_state": "prepared",
        "plan_graph": plan_graph,
        "working_state": working_state,
        "scratch_branch_policy": {
            "intent": "explore_without_mutating_target",
            "scratch_branch_ref": working_state["scratch_branch_ref"],
            "target_branch_ref": working_state["target_branch_ref"],
            "target_revision": 1,
            "promote_requires_proposal": true,
            "stale_target_blocks_promote": true
        },
        "trace_events": [
            {
                "event_type": "run_created",
                "run_id": run_id,
                "timestamp": 0,
                "payload": {"step": "plan_only", "status": "deterministic"}
            },
            {
                "event_type": "run_prepared",
                "run_id": run_id,
                "timestamp": 1,
                "payload": {"objective": objective, "plan_revision": 1}
            }
        ],
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn inspect_run(request: &InprocInvocation) -> anyhow::Result<Value> {
    let run_id = request
        .input
        .get("run_id")
        .and_then(Value::as_str)
        .unwrap_or("run_unknown");

    let state = request
        .input
        .get("lifecycle_state")
        .and_then(Value::as_str)
        .filter(|s| is_valid_lifecycle_state(s))
        .unwrap_or("running");

    Ok(serde_json::json!({
        "kind": "agentic_forge_run_inspection",
        "run_id": run_id,
        "lifecycle_state": state,
        "working_state": build_working_state(&run_id, &request.provider_package_id, &request.input),
        "plan_graph_ref": format!("plan_graph:{}", run_id),
        "candidate_refs": request.input.get("candidate_refs").cloned().unwrap_or(serde_json::json!([])),
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn cancel_run(request: &InprocInvocation) -> anyhow::Result<Value> {
    let run_id = request
        .input
        .get("run_id")
        .and_then(Value::as_str)
        .unwrap_or("run_unknown");

    Ok(serde_json::json!({
        "kind": "agentic_forge_run_cancelled",
        "run_id": run_id,
        "previous_state": request.input.get("lifecycle_state").and_then(Value::as_str).unwrap_or("running"),
        "lifecycle_state": "cancelled",
        "trace_events": [
            {
                "event_type": "run_cancelled",
                "run_id": run_id,
                "timestamp": 0,
                "payload": {"reason": "user_cancelled", "status": "deterministic"}
            }
        ],
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn summarize_run(request: &InprocInvocation) -> anyhow::Result<Value> {
    let run_id = request
        .input
        .get("run_id")
        .and_then(Value::as_str)
        .unwrap_or("run_unknown");
    let event_count = request
        .input
        .get("trace_events")
        .and_then(Value::as_array)
        .map(|a| a.len())
        .unwrap_or(0);
    let node_count = request
        .input
        .get("plan_graph")
        .and_then(|pg| pg.get("nodes"))
        .and_then(Value::as_array)
        .map(|a| a.len())
        .unwrap_or(3);
    let candidate_count = request
        .input
        .get("candidate_refs")
        .and_then(Value::as_array)
        .map(|a| a.len())
        .unwrap_or(0);

    Ok(serde_json::json!({
        "kind": "agentic_forge_run_summary",
        "run_id": run_id,
        "lifecycle_state": request.input.get("lifecycle_state").and_then(Value::as_str).unwrap_or("completed"),
        "trace_event_count": event_count,
        "plan_node_count": node_count,
        "candidate_count": candidate_count,
        "tool_observation_count": 0,
        "inference_trace_count": 0,
        "inference_performed": false,
        "network_performed": false,
        "summary": format!("Run {run_id}: {event_count} trace events, {node_count} plan nodes, {candidate_count} candidates, no inference, no network"),
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn export_plan_graph(request: &InprocInvocation) -> anyhow::Result<Value> {
    let run_id = request
        .input
        .get("run_id")
        .and_then(Value::as_str)
        .unwrap_or("run_unknown");
    let objective = request
        .input
        .get("objective")
        .and_then(Value::as_str)
        .unwrap_or("deterministic agentic forge run");

    let plan_graph = build_plan_graph(run_id, objective);

    Ok(serde_json::json!({
        "kind": "agentic_forge_plan_graph",
        "run_id": run_id,
        "plan_graph": plan_graph,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

// ---------------------------------------------------------------------------
// Phase B capabilities: candidate / compare / promote / archive
// ---------------------------------------------------------------------------

fn create_candidate(request: &InprocInvocation) -> anyhow::Result<Value> {
    // Raw-secret check
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "agentic_forge_candidate_rejected",
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

    let run_id = request
        .input
        .get("run_id")
        .and_then(Value::as_str)
        .unwrap_or("run_unknown");
    let target_branch = request
        .input
        .get("target_branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:target:default");
    let scratch_branch = request
        .input
        .get("scratch_branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:scratch:default");

    let candidate_id = format!("cand_{}", deterministic_id(&request.input));

    Ok(serde_json::json!({
        "kind": "agentic_forge_candidate_created",
        "candidate": {
            "candidate_id": candidate_id,
            "run_id": run_id,
            "target_branch_ref": target_branch,
            "scratch_branch_ref": scratch_branch,
            "changed_asset_refs": request.input.get("changed_asset_refs").cloned().unwrap_or(serde_json::json!([])),
            "projection_refs": request.input.get("projection_refs").cloned().unwrap_or(serde_json::json!([])),
            "diff_summary": request.input.get("diff_summary").and_then(Value::as_str).unwrap_or("deterministic diff: no real changes"),
            "inspection_refs": request.input.get("inspection_refs").cloned().unwrap_or(serde_json::json!([])),
            "confidence": request.input.get("confidence").and_then(Value::as_f64).unwrap_or(0.5),
            "uncertainty": request.input.get("uncertainty").and_then(Value::as_f64).unwrap_or(0.5),
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            },
            "status": "draft",
            "target_revision": request.input.get("target_revision").and_then(Value::as_u64).unwrap_or(1)
        },
        "target_branch_unchanged": true,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn compare_candidate(request: &InprocInvocation) -> anyhow::Result<Value> {
    let candidate_id = request
        .input
        .get("candidate_id")
        .and_then(Value::as_str)
        .unwrap_or("cand_unknown");
    let target_branch = request
        .input
        .get("target_branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:target:default");
    let scratch_branch = request
        .input
        .get("scratch_branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:scratch:default");

    // Determine staleness: compare target_revision from candidate vs current_target_revision
    let candidate_revision = request
        .input
        .get("target_revision")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    let current_target_revision = request
        .input
        .get("current_target_revision")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    let is_stale = candidate_revision != current_target_revision;

    Ok(serde_json::json!({
        "kind": "agentic_forge_candidate_comparison",
        "candidate_id": candidate_id,
        "target_branch_ref": target_branch,
        "scratch_branch_ref": scratch_branch,
        "diff_summary": request.input.get("diff_summary").and_then(Value::as_str).unwrap_or("deterministic diff: no real changes"),
        "affected_assets": request.input.get("changed_asset_refs").cloned().unwrap_or(serde_json::json!([])),
        "affected_projections": request.input.get("projection_refs").cloned().unwrap_or(serde_json::json!([])),
        "lineage_impact": {
            "target_branch_modified": false,
            "scratch_branch_source": scratch_branch,
            "requires_rebase": is_stale,
        },
        "stale": is_stale,
        "candidate_target_revision": candidate_revision,
        "current_target_revision": current_target_revision,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn draft_promote_proposal(request: &InprocInvocation) -> anyhow::Result<Value> {
    // Raw-secret check
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "agentic_forge_promote_rejected",
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

    let candidate_id = request
        .input
        .get("candidate_id")
        .and_then(Value::as_str)
        .unwrap_or("cand_unknown");
    let run_id = request
        .input
        .get("run_id")
        .and_then(Value::as_str)
        .unwrap_or("run_unknown");
    let target_branch = request
        .input
        .get("target_branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:target:default");
    let scratch_branch = request
        .input
        .get("scratch_branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:scratch:default");

    // Stale target check: revision mismatch blocks promote
    let candidate_revision = request
        .input
        .get("target_revision")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    let current_target_revision = request
        .input
        .get("current_target_revision")
        .and_then(Value::as_u64)
        .unwrap_or(1);

    if candidate_revision != current_target_revision {
        return Ok(serde_json::json!({
            "kind": "agentic_forge_promote_blocked",
            "reason": "stale_target_branch",
            "candidate_id": candidate_id,
            "candidate_target_revision": candidate_revision,
            "current_target_revision": current_target_revision,
            "target_branch_unchanged": true,
            "inference_performed": false,
            "network_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    // Produce a proposal draft — package-owned ops, not kernel.v1.proposal.create call
    let changed_assets = request
        .input
        .get("changed_asset_refs")
        .cloned()
        .unwrap_or(serde_json::json!([]));

    Ok(serde_json::json!({
        "kind": "agentic_forge_promote_proposal_draft",
        "candidate_id": candidate_id,
        "run_id": run_id,
        "proposal_draft": {
            "requires_user_approval": true,
            "operations": [
                {
                    "op": "asset.put",
                    "payload": {
                        "ref": changed_assets,
                        "source_branch": scratch_branch,
                        "target_branch": target_branch
                    }
                }
            ],
            "required_permissions": [],
            "expected_effects": [
                "candidate assets promoted to target branch via proposal approval"
            ],
            "source_candidate": candidate_id,
            "source_run": run_id,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        },
        "target_branch_unchanged": true,
        "direct_mutation": false,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn archive_candidate(request: &InprocInvocation) -> anyhow::Result<Value> {
    let candidate_id = request
        .input
        .get("candidate_id")
        .and_then(Value::as_str)
        .unwrap_or("cand_unknown");

    Ok(serde_json::json!({
        "kind": "agentic_forge_candidate_archived",
        "candidate_id": candidate_id,
        "previous_status": request.input.get("status").and_then(Value::as_str).unwrap_or("draft"),
        "status": "archived",
        "target_branch_unchanged": true,
        "summary": format!("Candidate {} archived; target branch not modified", candidate_id),
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn explain_branch_policy(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "agentic_forge_branch_policy",
        "policy": {
            "scratch_branch_intent": "explore_without_mutating_target",
            "promote_requires_proposal": true,
            "stale_target_blocks_promote": true,
            "target_revision_must_match": true,
            "reject_leaves_target_unchanged": true,
            "archive_does_not_modify_target": true
        },
        "explanation": "Agents explore in scratch branches. Candidates are compared against target branches. Promote produces a proposal draft that must be approved through the proposal lifecycle; it never directly mutates the target branch. If the target branch has advanced since the candidate was created (revision mismatch), promote is blocked as stale. Archiving or rejecting a candidate leaves the target branch unchanged.",
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

// ---------------------------------------------------------------------------
// Phase C capabilities: inference node / replay / validation / failure
// ---------------------------------------------------------------------------

fn run_inference_node(request: &InprocInvocation) -> anyhow::Result<Value> {
    // Raw-secret check
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "agentic_forge_inference_rejected",
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

    let run_id = request
        .input
        .get("run_id")
        .and_then(Value::as_str)
        .unwrap_or("run_unknown");
    let node_id = request
        .input
        .get("node_id")
        .and_then(Value::as_str)
        .unwrap_or("node_infer_default");
    let provider_kind = request
        .input
        .get("provider_kind")
        .and_then(Value::as_str)
        .unwrap_or("deterministic");

    // Validate provider_kind
    if !PROVIDER_KINDS.contains(&provider_kind) {
        return Ok(serde_json::json!({
            "kind": "agentic_forge_inference_rejected",
            "reason": "invalid_provider_kind",
            "provider_kind": provider_kind,
            "allowed_kinds": PROVIDER_KINDS,
            "inference_performed": false,
            "network_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    // cloud_adapter_plan: return plan/needs_host_policy, no network
    if provider_kind == "cloud_adapter_plan" {
        return Ok(serde_json::json!({
            "kind": "agentic_forge_inference_node_plan",
            "run_id": run_id,
            "node_id": node_id,
            "provider_kind": provider_kind,
            "node_result": {
                "status": "needs_host_policy",
                "description": "cloud adapter requires host-managed network policy and outbound execution; no network performed by package"
            },
            "inference_trace": {
                "provider_kind": provider_kind,
                "model_performed": false,
                "network_performed": false,
                "output_action": "observation"
            },
            "inference_performed": false,
            "network_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    // deterministic / recorded / local_fake: produce candidate_seed or proposal_seed
    let objective = request
        .input
        .get("objective")
        .and_then(Value::as_str)
        .unwrap_or("deterministic inference");

    let output_action = if objective.contains("proposal") {
        "proposal_seed"
    } else {
        "candidate_seed"
    };

    let model_performed = provider_kind == "local_fake";

    Ok(serde_json::json!({
        "kind": "agentic_forge_inference_node_result",
        "run_id": run_id,
        "node_id": node_id,
        "provider_kind": provider_kind,
        "node_result": {
            "status": "completed",
            "output_action": output_action,
            "content_hint": format!("deterministic {} from {}", output_action, provider_kind),
            "target_branch_unchanged": true,
            "direct_mutation": false
        },
        "inference_trace": {
            "provider_kind": provider_kind,
            "model_performed": model_performed,
            "network_performed": false,
            "output_action": output_action,
            "fingerprint": format!("fp_{}", deterministic_id(&request.input))
        },
        "inference_performed": model_performed,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn replay_inference_node(request: &InprocInvocation) -> anyhow::Result<Value> {
    // Raw-secret check
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "agentic_forge_replay_rejected",
            "redaction_state": "unsafe_blocked",
            "reason": "input contains raw-secret-like content",
            "inference_performed": false,
            "network_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    let run_id = request
        .input
        .get("run_id")
        .and_then(Value::as_str)
        .unwrap_or("run_unknown");
    let node_id = request
        .input
        .get("node_id")
        .and_then(Value::as_str)
        .unwrap_or("node_infer_default");

    let recorded_fingerprint = request
        .input
        .get("expected_fingerprint")
        .and_then(Value::as_str)
        .unwrap_or("");
    let computed_fingerprint = format!("fp_{}", deterministic_id(&request.input));

    if recorded_fingerprint == computed_fingerprint {
        Ok(serde_json::json!({
            "kind": "agentic_forge_replay_ok",
            "run_id": run_id,
            "node_id": node_id,
            "fingerprint_match": true,
            "fingerprint": recorded_fingerprint,
            "inference_performed": false,
            "network_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }))
    } else {
        Ok(serde_json::json!({
            "kind": "agentic_forge_replay_mismatch",
            "run_id": run_id,
            "node_id": node_id,
            "fingerprint_match": false,
            "expected_fingerprint": recorded_fingerprint,
            "actual_fingerprint": computed_fingerprint,
            "inference_performed": false,
            "network_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }))
    }
}

fn validate_inference_output(request: &InprocInvocation) -> anyhow::Result<Value> {
    let action = request
        .input
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("unknown_action");

    let is_allowed = ALLOWED_INFERENCE_ACTIONS.contains(&action);
    let is_forbidden = FORBIDDEN_INFERENCE_ACTIONS.contains(&action);

    let validation_result = if is_forbidden || !is_allowed {
        "rejected"
    } else {
        "accepted"
    };

    let reason = if is_forbidden {
        format!("action '{}' is in the forbidden list; model output cannot escalate privileges, auto-promote, request secrets, write target branches, or execute unknown actions", action)
    } else if !is_allowed {
        format!("action '{}' is not in the allowed list; only candidate_seed, proposal_seed, observation, needs_repair are permitted", action)
    } else {
        "action is permitted".to_string()
    };

    Ok(serde_json::json!({
        "kind": "agentic_forge_inference_validation",
        "action": action,
        "validation_result": validation_result,
        "allowed": is_allowed && !is_forbidden,
        "reason": reason,
        "allowed_actions": ALLOWED_INFERENCE_ACTIONS,
        "forbidden_actions": FORBIDDEN_INFERENCE_ACTIONS,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn explain_inference_failure(request: &InprocInvocation) -> anyhow::Result<Value> {
    let failure_kind = request
        .input
        .get("failure_kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown");

    let (is_known, recovery_hint) = match failure_kind {
        "rate_limit" => (true, "reduce request frequency or implement backoff; consider recorded replay for deterministic re-runs"),
        "quota" => (true, "check usage limits; switch to deterministic or recorded provider for quota-free runs"),
        "timeout" => (true, "increase timeout budget or use recorded replay to avoid network dependency"),
        "auth" => (true, "verify secret_ref resolves correctly; do not embed raw credentials; check provider identity"),
        "network_denied" => (true, "network access was denied by policy; use deterministic or local_fake provider; cloud_adapter_plan only returns plan shape"),
        "invalid_output" => (true, "model output failed validation; run validate_inference_output to check; repair with needs_repair action"),
        "malformed_output" => (true, "model output could not be parsed; treat as node_failed; generate repair proposal"),
        "replay_mismatch" => (true, "recorded output fingerprint does not match expected; re-run with correct recorded output or update expected fingerprint"),
        "policy_reject" => (true, "inference output action was rejected by policy; only candidate_seed/proposal_seed/observation/needs_repair are allowed; model output cannot escalate or auto-promote"),
        _ => (false, "unknown failure kind; consult inference_failure_taxonomy for valid kinds"),
    };

    Ok(serde_json::json!({
        "kind": "agentic_forge_inference_failure_explanation",
        "failure_kind": failure_kind,
        "is_known": is_known,
        "recovery_hint": recovery_hint,
        "taxonomy": INFERENCE_FAILURE_KINDS,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

// ---------------------------------------------------------------------------
// Shape builders
// ---------------------------------------------------------------------------

fn deterministic_id(input: &Value) -> String {
    // Simple deterministic ID from input content, no random
    let objective = input
        .get("objective")
        .and_then(Value::as_str)
        .unwrap_or("default");
    let len = objective.len();
    format!("{:04x}", len.wrapping_mul(31).wrapping_add(0xaf))
}

fn build_plan_graph(run_id: &str, objective: &str) -> Value {
    serde_json::json!({
        "nodes": [
            {
                "node_id": format!("{run_id}_node_observe"),
                "kind": "observe",
                "label": "Observe context",
                "status": "pending"
            },
            {
                "node_id": format!("{run_id}_node_plan"),
                "kind": "tool_call",
                "label": objective,
                "status": "pending"
            },
            {
                "node_id": format!("{run_id}_node_propose"),
                "kind": "propose",
                "label": "Produce candidate",
                "status": "pending"
            }
        ],
        "edges": [
            {
                "from_node_id": format!("{run_id}_node_observe"),
                "to_node_id": format!("{run_id}_node_plan"),
                "kind": "sequential"
            },
            {
                "from_node_id": format!("{run_id}_node_plan"),
                "to_node_id": format!("{run_id}_node_propose"),
                "kind": "sequential"
            }
        ],
        "status": "prepared",
        "revision": 1,
        "input_refs": [],
        "output_refs": [],
        "approval_policy": "fork_then_approve",
        "retry_policy": {"max_retries": 0, "backoff": "none"},
        "deterministic_mode": true
    })
}

fn build_working_state(run_id: &str, owner_package: &str, input: &Value) -> Value {
    serde_json::json!({
        "run_id": run_id,
        "owner_package": owner_package,
        "target_branch_ref": input.get("target_branch_ref").and_then(Value::as_str).unwrap_or("branch:target:default"),
        "scratch_branch_ref": input.get("scratch_branch_ref").and_then(Value::as_str).unwrap_or("branch:scratch:default"),
        "current_objective": input.get("objective").and_then(Value::as_str).unwrap_or("deterministic agentic forge run"),
        "local_context_refs": input.get("local_context_refs").cloned().unwrap_or(serde_json::json!([])),
        "plan_graph_ref": format!("plan_graph:{}", run_id),
        "candidate_refs": input.get("candidate_refs").cloned().unwrap_or(serde_json::json!([])),
        "tool_observation_refs": input.get("tool_observation_refs").cloned().unwrap_or(serde_json::json!([])),
        "inference_trace_refs": input.get("inference_trace_refs").cloned().unwrap_or(serde_json::json!([])),
        "policy_state": {
            "approval_policy": "fork_then_approve",
            "retry_budget_remaining": 0,
            "deterministic_mode": true
        }
    })
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
        let req = make_request("official/agentic-forge-lab/describe_contract", json!({}));
        assert!(try_handle(&req).is_some());
    }

    #[test]
    fn try_handle_rejects_wrong_package() {
        let req = InprocInvocation {
            capability_id: "official/agentic-forge-lab/describe_contract".to_string(),
            provider_package_id: "official/other".to_string(),
            session_id: None,
            input: json!({}),
        };
        assert!(try_handle(&req).is_none());
    }

    #[test]
    fn describe_contract_returns_lifecycle_states() {
        let req = make_request("official/agentic-forge-lab/describe_contract", json!({}));
        let result = try_handle(&req).unwrap().unwrap();
        let states = result["lifecycle_states"].as_array().unwrap();
        assert_eq!(states.len(), 9);
        assert_eq!(states[0], json!("created"));
        assert_eq!(states[8], json!("archived"));
    }

    #[test]
    fn start_run_returns_plan_graph_and_working_state() {
        let req = make_request(
            "official/agentic-forge-lab/start_run",
            json!({"objective": "test run"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_run_started"));
        assert_eq!(result["lifecycle_state"], json!("prepared"));
        assert!(result["plan_graph"]["nodes"].is_array());
        assert!(result["working_state"]["run_id"].is_string());
        assert_eq!(result["plan_graph"]["deterministic_mode"], json!(true));
    }

    #[test]
    fn start_run_blocks_raw_secret() {
        let req = make_request(
            "official/agentic-forge-lab/start_run",
            json!({"objective": "test", "api_key": "RawSecretExample1234567890abcdefABCDEF123456"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_run_rejected"));
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn start_run_blocks_bearer_secret() {
        let req = make_request(
            "official/agentic-forge-lab/start_run",
            json!({"objective": "test", "token": "Bearer abc123"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn start_run_accepts_secret_ref() {
        let req = make_request(
            "official/agentic-forge-lab/start_run",
            json!({"objective": "test", "api_key": "secret_ref:env:MY_KEY"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_run_started"));
        assert_ne!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn cancel_run_sets_cancelled_state() {
        let req = make_request(
            "official/agentic-forge-lab/cancel_run",
            json!({"run_id": "run_test", "lifecycle_state": "running"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_run_cancelled"));
        assert_eq!(result["lifecycle_state"], json!("cancelled"));
    }

    #[test]
    fn summarize_run_returns_observability() {
        let req = make_request(
            "official/agentic-forge-lab/summarize_run",
            json!({"run_id": "run_test", "trace_events": [{"a": 1}, {"b": 2}]}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_run_summary"));
        assert_eq!(result["trace_event_count"], json!(2));
    }

    #[test]
    fn export_plan_graph_returns_nodes_edges() {
        let req = make_request(
            "official/agentic-forge-lab/export_plan_graph",
            json!({"run_id": "run_test"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_plan_graph"));
        assert!(result["plan_graph"]["nodes"].as_array().unwrap().len() >= 1);
        assert!(result["plan_graph"]["approval_policy"].is_string());
    }

    #[test]
    fn no_kernel_agent_namespace_in_output() {
        let req = make_request(
            "official/agentic-forge-lab/start_run",
            json!({"objective": "test"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        let output_str = serde_json::to_string(&result).unwrap();
        assert!(!output_str.contains("kernel.v1.agent"));
        assert!(!output_str.contains("kernel.v1.model"));
        assert!(!output_str.contains("kernel.v1.prompt"));
        assert!(!output_str.contains("kernel.v1.memory"));
        assert!(!output_str.contains("kernel.v1.turn"));
    }

    #[test]
    fn all_lifecycle_states_valid() {
        for state in LIFECYCLE_STATES {
            assert!(is_valid_lifecycle_state(state));
        }
        assert!(!is_valid_lifecycle_state("unknown_state"));
    }

    #[test]
    fn contains_raw_secret_detects_sk_prefix() {
        assert!(safety::contains_raw_secret(
            &json!({"api_key": "RawSecretExample1234567890abcdefABCDEF123456"})
        ));
        assert!(safety::contains_raw_secret(&json!({"token": "Bearer xyz"})));
        assert!(!safety::contains_raw_secret(
            &json!({"api_key": "secret_ref:env:MY_KEY"})
        ));
        assert!(!safety::contains_raw_secret(
            &json!({"api_key": "secret-ref:env:MY_KEY"})
        ));
        assert!(!safety::contains_raw_secret(
            &json!({"api_key": "host:env:MY_KEY"})
        ));
        assert!(!safety::contains_raw_secret(
            &json!({"objective": "safe text"})
        ));
    }

    // -----------------------------------------------------------------------
    // Phase B unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn create_candidate_returns_branch_aware_candidate() {
        let req = make_request(
            "official/agentic-forge-lab/create_candidate",
            json!({
                "run_id": "run_test",
                "target_branch_ref": "branch:target:main",
                "scratch_branch_ref": "branch:scratch:s1",
                "target_revision": 1,
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_candidate_created"));
        assert_eq!(result["target_branch_unchanged"], json!(true));
        let cand = &result["candidate"];
        assert!(cand["candidate_id"].is_string());
        assert_eq!(cand["run_id"], json!("run_test"));
        assert_eq!(cand["target_branch_ref"], json!("branch:target:main"));
        assert_eq!(cand["scratch_branch_ref"], json!("branch:scratch:s1"));
        assert!(cand["changed_asset_refs"].is_array());
        assert!(cand["projection_refs"].is_array());
        assert!(cand["diff_summary"].is_string());
        assert!(cand["inspection_refs"].is_array());
        assert!(cand["confidence"].is_number());
        assert!(cand["uncertainty"].is_number());
        assert!(cand["provenance"]["package_id"].is_string());
        assert_eq!(cand["status"], json!("draft"));
    }

    #[test]
    fn compare_candidate_reports_diff_and_stale() {
        // Matching revisions → stale=false
        let req = make_request(
            "official/agentic-forge-lab/compare_candidate",
            json!({
                "candidate_id": "cand_test",
                "target_revision": 1,
                "current_target_revision": 1,
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_candidate_comparison"));
        assert_eq!(result["stale"], json!(false));

        // Mismatched revisions → stale=true
        let req_stale = make_request(
            "official/agentic-forge-lab/compare_candidate",
            json!({
                "candidate_id": "cand_test",
                "target_revision": 1,
                "current_target_revision": 3,
            }),
        );
        let result_stale = try_handle(&req_stale).unwrap().unwrap();
        assert_eq!(result_stale["stale"], json!(true));
        assert_eq!(result_stale["candidate_target_revision"], json!(1));
        assert_eq!(result_stale["current_target_revision"], json!(3));
    }

    #[test]
    fn draft_promote_proposal_returns_proposal_draft() {
        let req = make_request(
            "official/agentic-forge-lab/draft_promote_proposal",
            json!({
                "candidate_id": "cand_test",
                "run_id": "run_test",
                "target_revision": 1,
                "current_target_revision": 1,
                "target_branch_ref": "branch:target:main",
                "scratch_branch_ref": "branch:scratch:s1",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["kind"],
            json!("agentic_forge_promote_proposal_draft")
        );
        assert_eq!(result["target_branch_unchanged"], json!(true));
        assert_eq!(result["direct_mutation"], json!(false));
        assert_eq!(
            result["proposal_draft"]["requires_user_approval"],
            json!(true)
        );
        assert!(result["proposal_draft"]["operations"].is_array());
    }

    #[test]
    fn draft_promote_blocked_on_stale_target() {
        let req = make_request(
            "official/agentic-forge-lab/draft_promote_proposal",
            json!({
                "candidate_id": "cand_test",
                "run_id": "run_test",
                "target_revision": 1,
                "current_target_revision": 2,
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_promote_blocked"));
        assert_eq!(result["reason"], json!("stale_target_branch"));
        assert_eq!(result["target_branch_unchanged"], json!(true));
    }

    #[test]
    fn archive_candidate_sets_archived_status() {
        let req = make_request(
            "official/agentic-forge-lab/archive_candidate",
            json!({
                "candidate_id": "cand_test",
                "status": "draft",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_candidate_archived"));
        assert_eq!(result["status"], json!("archived"));
        assert_eq!(result["target_branch_unchanged"], json!(true));
    }

    #[test]
    fn explain_branch_policy_returns_policy() {
        let req = make_request(
            "official/agentic-forge-lab/explain_branch_policy",
            json!({}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_branch_policy"));
        assert_eq!(result["policy"]["promote_requires_proposal"], json!(true));
        assert_eq!(result["policy"]["stale_target_blocks_promote"], json!(true));
    }

    #[test]
    fn start_run_includes_scratch_branch_policy() {
        let req = make_request(
            "official/agentic-forge-lab/start_run",
            json!({"objective": "branch test"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["scratch_branch_policy"]["intent"],
            json!("explore_without_mutating_target")
        );
        assert_eq!(
            result["scratch_branch_policy"]["promote_requires_proposal"],
            json!(true)
        );
        assert_eq!(
            result["scratch_branch_policy"]["stale_target_blocks_promote"],
            json!(true)
        );
    }

    #[test]
    fn create_candidate_blocks_raw_secret() {
        let req = make_request(
            "official/agentic-forge-lab/create_candidate",
            json!({
                "run_id": "run_test",
                "api_key": "RawSecretExample1234567890abcdefABCDEF123456",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_candidate_rejected"));
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn draft_promote_blocks_raw_secret() {
        let req = make_request(
            "official/agentic-forge-lab/draft_promote_proposal",
            json!({
                "candidate_id": "cand_test",
                "run_id": "run_test",
                "token": "Bearer abc123",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_promote_rejected"));
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn all_candidate_states_valid() {
        for state in CANDIDATE_STATES {
            assert!(is_valid_candidate_state(state));
        }
        assert!(!is_valid_candidate_state("unknown_state"));
    }

    #[test]
    fn no_kernel_namespace_in_phase_b_outputs() {
        for cap in &[
            "official/agentic-forge-lab/create_candidate",
            "official/agentic-forge-lab/compare_candidate",
            "official/agentic-forge-lab/draft_promote_proposal",
            "official/agentic-forge-lab/archive_candidate",
            "official/agentic-forge-lab/explain_branch_policy",
        ] {
            let req = make_request(
                cap,
                json!({"run_id": "run_ns", "candidate_id": "cand_ns", "target_revision": 1, "current_target_revision": 1}),
            );
            let result = try_handle(&req).unwrap().unwrap();
            let output_str = serde_json::to_string(&result).unwrap();
            assert!(
                !output_str.contains("kernel.v1.agent"),
                "{cap} must not contain kernel.v1.agent"
            );
            assert!(
                !output_str.contains("kernel.v1.model"),
                "{cap} must not contain kernel.v1.model"
            );
            assert!(
                !output_str.contains("kernel.v1.prompt"),
                "{cap} must not contain kernel.v1.prompt"
            );
            assert!(
                !output_str.contains("kernel.v1.memory"),
                "{cap} must not contain kernel.v1.memory"
            );
            assert!(
                !output_str.contains("kernel.v1.turn"),
                "{cap} must not contain kernel.v1.turn"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Phase C unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn run_inference_node_deterministic_produces_candidate_seed() {
        let req = make_request(
            "official/agentic-forge-lab/run_inference_node",
            json!({
                "run_id": "run_inf",
                "node_id": "node_infer_1",
                "provider_kind": "deterministic",
                "objective": "analyze composition",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_inference_node_result"));
        assert_eq!(
            result["node_result"]["output_action"],
            json!("candidate_seed")
        );
        assert_eq!(
            result["node_result"]["target_branch_unchanged"],
            json!(true)
        );
        assert_eq!(result["node_result"]["direct_mutation"], json!(false));
        assert_eq!(result["inference_trace"]["network_performed"], json!(false));
        assert_eq!(result["network_performed"], json!(false));
    }

    #[test]
    fn run_inference_node_objective_with_proposal_produces_proposal_seed() {
        let req = make_request(
            "official/agentic-forge-lab/run_inference_node",
            json!({
                "run_id": "run_inf",
                "node_id": "node_infer_2",
                "provider_kind": "deterministic",
                "objective": "draft proposal for changes",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["node_result"]["output_action"],
            json!("proposal_seed")
        );
    }

    #[test]
    fn run_inference_node_cloud_adapter_returns_needs_host_policy() {
        let req = make_request(
            "official/agentic-forge-lab/run_inference_node",
            json!({
                "run_id": "run_cloud",
                "node_id": "node_infer_cloud",
                "provider_kind": "cloud_adapter_plan",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_inference_node_plan"));
        assert_eq!(result["node_result"]["status"], json!("needs_host_policy"));
        assert_eq!(result["network_performed"], json!(false));
        assert_eq!(result["inference_performed"], json!(false));
    }

    #[test]
    fn run_inference_node_rejects_invalid_provider() {
        let req = make_request(
            "official/agentic-forge-lab/run_inference_node",
            json!({
                "run_id": "run_bad",
                "provider_kind": "cloud_real",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_inference_rejected"));
        assert_eq!(result["reason"], json!("invalid_provider_kind"));
    }

    #[test]
    fn run_inference_node_blocks_raw_secret() {
        let req = make_request(
            "official/agentic-forge-lab/run_inference_node",
            json!({
                "run_id": "run_inf",
                "api_key": "RawSecretExample1234567890abcdefABCDEF123456",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_inference_rejected"));
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn replay_inference_node_match_ok() {
        // Compute the expected fingerprint first from an identical input
        let input = json!({
            "run_id": "run_replay",
            "node_id": "node_infer_1",
        });
        let expected_fp = format!("fp_{}", deterministic_id(&input));
        let req = make_request(
            "official/agentic-forge-lab/replay_inference_node",
            json!({
                "run_id": "run_replay",
                "node_id": "node_infer_1",
                "expected_fingerprint": expected_fp,
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_replay_ok"));
        assert_eq!(result["fingerprint_match"], json!(true));
    }

    #[test]
    fn replay_inference_node_mismatch_flagged() {
        let req = make_request(
            "official/agentic-forge-lab/replay_inference_node",
            json!({
                "run_id": "run_replay",
                "node_id": "node_infer_1",
                "expected_fingerprint": "fp_WRONG",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("agentic_forge_replay_mismatch"));
        assert_eq!(result["fingerprint_match"], json!(false));
        assert_ne!(result["expected_fingerprint"], result["actual_fingerprint"]);
    }

    #[test]
    fn validate_inference_output_accepts_allowed_actions() {
        for action in ALLOWED_INFERENCE_ACTIONS {
            let req = make_request(
                "official/agentic-forge-lab/validate_inference_output",
                json!({
                    "action": action,
                }),
            );
            let result = try_handle(&req).unwrap().unwrap();
            assert_eq!(
                result["validation_result"],
                json!("accepted"),
                "action {} should be accepted",
                action
            );
            assert_eq!(result["allowed"], json!(true));
        }
    }

    #[test]
    fn validate_inference_output_rejects_forbidden_actions() {
        for action in FORBIDDEN_INFERENCE_ACTIONS {
            let req = make_request(
                "official/agentic-forge-lab/validate_inference_output",
                json!({
                    "action": action,
                }),
            );
            let result = try_handle(&req).unwrap().unwrap();
            assert_eq!(
                result["validation_result"],
                json!("rejected"),
                "action {} should be rejected",
                action
            );
            assert_eq!(result["allowed"], json!(false));
        }
    }

    #[test]
    fn validate_inference_output_rejects_unknown_action() {
        let req = make_request(
            "official/agentic-forge-lab/validate_inference_output",
            json!({
                "action": "arbitrary_exec",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["validation_result"], json!("rejected"));
        assert_eq!(result["allowed"], json!(false));
    }

    #[test]
    fn explain_inference_failure_returns_recovery_hints() {
        for kind in INFERENCE_FAILURE_KINDS {
            let req = make_request(
                "official/agentic-forge-lab/explain_inference_failure",
                json!({
                    "failure_kind": kind,
                }),
            );
            let result = try_handle(&req).unwrap().unwrap();
            assert_eq!(
                result["kind"],
                json!("agentic_forge_inference_failure_explanation")
            );
            assert_eq!(result["is_known"], json!(true));
            assert!(
                result["recovery_hint"].as_str().unwrap().len() > 0,
                "failure {} should have recovery hint",
                kind
            );
        }
    }

    #[test]
    fn explain_inference_failure_unknown_kind() {
        let req = make_request(
            "official/agentic-forge-lab/explain_inference_failure",
            json!({
                "failure_kind": "unknown_error",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["is_known"], json!(false));
    }

    #[test]
    fn no_kernel_namespace_in_phase_c_outputs() {
        for cap in &[
            "official/agentic-forge-lab/run_inference_node",
            "official/agentic-forge-lab/replay_inference_node",
            "official/agentic-forge-lab/validate_inference_output",
            "official/agentic-forge-lab/explain_inference_failure",
        ] {
            let req = make_request(
                cap,
                json!({"run_id": "run_ns", "node_id": "n1", "provider_kind": "deterministic", "failure_kind": "timeout", "action": "candidate_seed", "expected_fingerprint": "fp_test"}),
            );
            let result = try_handle(&req).unwrap().unwrap();
            let output_str = serde_json::to_string(&result).unwrap();
            assert!(
                !output_str.contains("kernel.v1.agent"),
                "{cap} must not contain kernel.v1.agent"
            );
            assert!(
                !output_str.contains("kernel.v1.model"),
                "{cap} must not contain kernel.v1.model"
            );
            assert!(
                !output_str.contains("kernel.v1.prompt"),
                "{cap} must not contain kernel.v1.prompt"
            );
            assert!(
                !output_str.contains("kernel.v1.memory"),
                "{cap} must not contain kernel.v1.memory"
            );
            assert!(
                !output_str.contains("kernel.v1.turn"),
                "{cap} must not contain kernel.v1.turn"
            );
        }
    }

    #[test]
    fn describe_contract_includes_phase_c_fields() {
        let req = make_request("official/agentic-forge-lab/describe_contract", json!({}));
        let result = try_handle(&req).unwrap().unwrap();
        assert!(
            result["plan_node_kinds"].is_array(),
            "describe_contract must have plan_node_kinds"
        );
        assert!(
            result["provider_kinds"].is_array(),
            "describe_contract must have provider_kinds"
        );
        assert!(
            result["inference_failure_taxonomy"].is_array(),
            "describe_contract must have inference_failure_taxonomy"
        );
        assert!(
            result["allowed_inference_actions"].is_array(),
            "describe_contract must have allowed_inference_actions"
        );
        assert_eq!(
            result["capabilities"].as_array().unwrap().len(),
            15,
            "describe_contract must list 15 capabilities"
        );
    }

    #[test]
    fn local_fake_provider_sets_inference_performed() {
        let req = make_request(
            "official/agentic-forge-lab/run_inference_node",
            json!({
                "run_id": "run_local",
                "node_id": "node_infer_local",
                "provider_kind": "local_fake",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["inference_performed"], json!(true));
        assert_eq!(result["inference_trace"]["model_performed"], json!(true));
        assert_eq!(result["network_performed"], json!(false));
    }
}
