//! Handler for `official/experience-observability-lab` capabilities.
//!
//! Experience Beta 3 — Experience Observability (backend/package part).
//!
//! Package-owned experience observability: session health, package health,
//! agent run health, proposal causal chain, failure breadcrumbs,
//! cost/latency summary, and guardrail/audit summary.
//!
//! Deterministic, no-network, no real model inference. Produces
//! protocol-visible observability shapes from package/protocol refs,
//! not from SQLite or runtime internals.
//!
//! No `kernel.v1.observability.*`, `kernel.v1.experience.*`,
//! `kernel.v1.world.*`, `kernel.v1.scene.*`, `kernel.v1.turn.*`,
//! `kernel.v1.chat.*`, `kernel.v1.memory.*`, `kernel.v1.agent.*`,
//! `kernel.v1.model.*`, `kernel.v1.prompt.*`, or `kernel.v1.director.*`
//! namespace references.
//!
//! State terminology: session_health, package_health, agent_run_health,
//! proposal_causal_chain, failure_breadcrumbs, cost_latency_summary,
//! guardrail_audit_summary — not chat/message/prompt/world/scene/turn/memory.

use serde_json::Value;

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/experience-observability-lab";

// ---------------------------------------------------------------------------
// Session health statuses
// ---------------------------------------------------------------------------

const SESSION_HEALTH_STATUSES: &[&str] =
    &["healthy", "degraded", "failed", "recovering", "unknown"];

// ---------------------------------------------------------------------------
// Package health statuses
// ---------------------------------------------------------------------------

const PACKAGE_HEALTH_STATUSES: &[&str] = &["loaded", "degraded", "unloaded", "failed", "unknown"];

// ---------------------------------------------------------------------------
// Agent run health statuses
// ---------------------------------------------------------------------------

const AGENT_RUN_HEALTH_STATUSES: &[&str] = &[
    "idle",
    "running",
    "completed",
    "cancelled",
    "timeout",
    "failed",
    "unknown",
];

// ---------------------------------------------------------------------------
// Failure breadcrumb kinds
// ---------------------------------------------------------------------------

const FAILURE_BREADCRUMB_KINDS: &[&str] = &[
    "capability_invocation_failed",
    "proposal_rejected",
    "proposal_apply_failed",
    "checkpoint_corrupt",
    "checkpoint_missing",
    "agent_run_cancelled",
    "agent_run_timeout",
    "agent_run_failed",
    "inference_error",
    "outbound_denied",
    "raw_secret_blocked",
    "constraint_violation",
    "unknown",
];

// ---------------------------------------------------------------------------
// Guardrail kinds
// ---------------------------------------------------------------------------

const GUARDRAIL_KINDS: &[&str] = &[
    "raw_secret_blocked",
    "outbound_denied",
    "permission_denied",
    "schema_validation_failed",
    "kernel_namespace_blocked",
    "proposal_rejected_by_policy",
    "budget_exceeded",
    "deadline_exceeded",
    "replay_mismatch_flagged",
    "privilege_escalation_rejected",
];

// ---------------------------------------------------------------------------
// Causal chain step kinds
// ---------------------------------------------------------------------------

const CAUSAL_STEP_KINDS: &[&str] = &[
    "player_action",
    "state_delta",
    "checkpoint",
    "agent_run_start",
    "plan_node",
    "inference_node",
    "candidate_created",
    "proposal_drafted",
    "proposal_approved",
    "proposal_applied",
    "projection_rebuild",
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
    if id.ends_with("/describe_observability") {
        Some(describe_observability(request))
    } else if id.ends_with("/summarize_session_health") {
        Some(summarize_session_health(request))
    } else if id.ends_with("/summarize_package_health") {
        Some(summarize_package_health(request))
    } else if id.ends_with("/summarize_agent_run_health") {
        Some(summarize_agent_run_health(request))
    } else if id.ends_with("/trace_proposal_causality") {
        Some(trace_proposal_causality(request))
    } else if id.ends_with("/summarize_cost_latency") {
        Some(summarize_cost_latency(request))
    } else if id.ends_with("/list_failure_breadcrumbs") {
        Some(list_failure_breadcrumbs(request))
    } else if id.ends_with("/summarize_guardrails") {
        Some(summarize_guardrails(request))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Capability implementations
// ---------------------------------------------------------------------------

fn describe_observability(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "experience_observability_contract",
        "package_id": request.provider_package_id,
        "package_kind": "ordinary",
        "capabilities": [
            {"id": "official/experience-observability-lab/describe_observability", "purpose": "describe the observability package contract"},
            {"id": "official/experience-observability-lab/summarize_session_health", "purpose": "summarize the health of a session from protocol-visible refs"},
            {"id": "official/experience-observability-lab/summarize_package_health", "purpose": "summarize the health of packages in a session from protocol-visible refs"},
            {"id": "official/experience-observability-lab/summarize_agent_run_health", "purpose": "summarize the health of agent runs from protocol-visible refs"},
            {"id": "official/experience-observability-lab/trace_proposal_causality", "purpose": "trace causal chain from proposal to contributing events"},
            {"id": "official/experience-observability-lab/summarize_cost_latency", "purpose": "summarize cost and latency from outbound audit refs"},
            {"id": "official/experience-observability-lab/list_failure_breadcrumbs", "purpose": "list failure breadcrumbs from protocol-visible event refs"},
            {"id": "official/experience-observability-lab/summarize_guardrails", "purpose": "summarize guardrail/audit findings from protocol-visible refs"},
        ],
        "surfaces": {
            "forge_panel": "official/experience-observability-lab/forge-panel",
            "assistant_action": "official/experience-observability-lab/assistant-action",
            "home_card": "official/experience-observability-lab/home-card",
        },
        "session_health_statuses": SESSION_HEALTH_STATUSES,
        "package_health_statuses": PACKAGE_HEALTH_STATUSES,
        "agent_run_health_statuses": AGENT_RUN_HEALTH_STATUSES,
        "failure_breadcrumb_kinds": FAILURE_BREADCRUMB_KINDS,
        "guardrail_kinds": GUARDRAIL_KINDS,
        "causal_step_kinds": CAUSAL_STEP_KINDS,
        "output_shapes": {
            "session_health": ["session_id", "status", "event_count", "package_count", "proposal_count", "asset_count", "last_event_ref", "failure_count"],
            "package_health": ["package_id", "status", "capability_count", "error_count", "lifecycle_events"],
            "agent_run_health": ["run_id", "status", "plan_node_count", "candidate_count", "inference_count", "duration_hint_ms"],
            "proposal_causal_chain": ["proposal_id", "chain", "chain[].step", "chain[].ref", "chain[].content_address", "chain[].description"],
            "failure_breadcrumbs": ["session_id", "breadcrumbs", "breadcrumbs[].kind", "breadcrumbs[].ref", "breadcrumbs[].sequence", "breadcrumbs[].description"],
            "cost_latency_summary": ["total_invocations", "outbound_request_count", "total_duration_hint_ms", "cost_refs", "latency_by_capability"],
            "guardrail_audit_summary": ["total_guardrails", "guardrails", "guardrails[].kind", "guardrails[].ref", "guardrails[].description"],
        },
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn summarize_session_health(request: &InprocInvocation) -> anyhow::Result<Value> {
    // Raw-secret check
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "experience_observability_rejected",
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
        .unwrap_or("session:default");

    // Deterministic: derive health from input refs, not from SQLite
    let event_count = request
        .input
        .get("event_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let package_count = request
        .input
        .get("package_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let proposal_count = request
        .input
        .get("proposal_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let asset_count = request
        .input
        .get("asset_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let failure_count = request
        .input
        .get("failure_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    let status = request
        .input
        .get("status")
        .and_then(Value::as_str)
        .filter(|s| SESSION_HEALTH_STATUSES.contains(s))
        .unwrap_or_else(|| {
            if failure_count > 0 {
                "degraded"
            } else if event_count == 0 {
                "unknown"
            } else {
                "healthy"
            }
        });

    let last_event_ref = request
        .input
        .get("last_event_ref")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("event:{}:latest", session_id));

    Ok(serde_json::json!({
        "kind": "session_health",
        "package_id": request.provider_package_id,
        "session_id": session_id,
        "status": status,
        "event_count": event_count,
        "package_count": package_count,
        "proposal_count": proposal_count,
        "asset_count": asset_count,
        "last_event_ref": last_event_ref,
        "failure_count": failure_count,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn summarize_package_health(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "experience_observability_rejected",
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

    let package_id = request
        .input
        .get("package_id")
        .and_then(Value::as_str)
        .unwrap_or("official/unknown");

    let capability_count = request
        .input
        .get("capability_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let error_count = request
        .input
        .get("error_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    let status = request
        .input
        .get("status")
        .and_then(Value::as_str)
        .filter(|s| PACKAGE_HEALTH_STATUSES.contains(s))
        .unwrap_or_else(|| {
            if error_count > 0 {
                "degraded"
            } else if capability_count > 0 {
                "loaded"
            } else {
                "unknown"
            }
        });

    let lifecycle_events = request
        .input
        .get("lifecycle_events")
        .cloned()
        .unwrap_or(serde_json::json!([]));

    Ok(serde_json::json!({
        "kind": "package_health",
        "provider_package_id": request.provider_package_id,
        "package_id": package_id,
        "status": status,
        "capability_count": capability_count,
        "error_count": error_count,
        "lifecycle_events": lifecycle_events,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn summarize_agent_run_health(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "experience_observability_rejected",
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
        .unwrap_or("run:unknown");

    let plan_node_count = request
        .input
        .get("plan_node_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let candidate_count = request
        .input
        .get("candidate_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let inference_count = request
        .input
        .get("inference_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let duration_hint_ms = request
        .input
        .get("duration_hint_ms")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    let status = request
        .input
        .get("status")
        .and_then(Value::as_str)
        .filter(|s| AGENT_RUN_HEALTH_STATUSES.contains(s))
        .unwrap_or("unknown");

    Ok(serde_json::json!({
        "kind": "agent_run_health",
        "provider_package_id": request.provider_package_id,
        "run_id": run_id,
        "status": status,
        "plan_node_count": plan_node_count,
        "candidate_count": candidate_count,
        "inference_count": inference_count,
        "duration_hint_ms": duration_hint_ms,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn trace_proposal_causality(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "experience_observability_rejected",
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

    let proposal_id = request
        .input
        .get("proposal_id")
        .and_then(Value::as_str)
        .unwrap_or("proposal:unknown");

    // Build deterministic causal chain from input refs
    let player_action_ref = request
        .input
        .get("player_action_ref")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("action:default:1"));
    let checkpoint_ref = request
        .input
        .get("checkpoint_ref")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .unwrap_or_else(|| "checkpoint:default:1".to_string());
    let run_ref = request
        .input
        .get("run_ref")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .unwrap_or_else(|| "run:forge:default".to_string());
    let candidate_ref = request
        .input
        .get("candidate_ref")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .unwrap_or_else(|| "cand:forge:default".to_string());

    let chain = serde_json::json!([
        {
            "step": "player_action",
            "ref": player_action_ref,
            "content_address": crate::runtime::content_address(&format!("action:{}", player_action_ref)),
            "description": "Player action that triggered the change request"
        },
        {
            "step": "state_delta",
            "ref": request.input.get("state_delta_ref").and_then(Value::as_str).unwrap_or("asset:state_delta:default:1"),
            "content_address": crate::runtime::content_address("delta:asset:state_delta:default:1"),
            "description": "State delta produced by the player action"
        },
        {
            "step": "checkpoint",
            "ref": checkpoint_ref,
            "content_address": crate::runtime::content_address(&format!("checkpoint:{}", checkpoint_ref)),
            "description": "Board checkpoint at the time of change request"
        },
        {
            "step": "agent_run_start",
            "ref": run_ref,
            "content_address": crate::runtime::content_address(&format!("run:{}", run_ref)),
            "description": "Agent run started to evaluate the change"
        },
        {
            "step": "candidate_created",
            "ref": candidate_ref,
            "content_address": crate::runtime::content_address(&format!("cand:{}", candidate_ref)),
            "description": "Candidate produced by agent run on scratch branch"
        },
        {
            "step": "proposal_drafted",
            "ref": proposal_id,
            "content_address": crate::runtime::content_address(&format!("proposal:{}", proposal_id)),
            "description": "Promote proposal drafted from candidate"
        },
    ]);

    Ok(serde_json::json!({
        "kind": "proposal_causal_chain",
        "provider_package_id": request.provider_package_id,
        "proposal_id": proposal_id,
        "chain": chain,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn summarize_cost_latency(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "experience_observability_rejected",
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
        .unwrap_or("session:default");

    let total_invocations = request
        .input
        .get("total_invocations")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let outbound_request_count = request
        .input
        .get("outbound_request_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_duration_hint_ms = request
        .input
        .get("total_duration_hint_ms")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    // Cost refs are protocol-visible outbound audit refs (not raw cost data)
    let cost_refs = request
        .input
        .get("cost_refs")
        .cloned()
        .unwrap_or(serde_json::json!([]));

    // Latency by capability (from input or empty)
    let latency_by_capability = request
        .input
        .get("latency_by_capability")
        .cloned()
        .unwrap_or(serde_json::json!([]));

    Ok(serde_json::json!({
        "kind": "cost_latency_summary",
        "provider_package_id": request.provider_package_id,
        "session_id": session_id,
        "total_invocations": total_invocations,
        "outbound_request_count": outbound_request_count,
        "total_duration_hint_ms": total_duration_hint_ms,
        "cost_refs": cost_refs,
        "latency_by_capability": latency_by_capability,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn list_failure_breadcrumbs(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "experience_observability_rejected",
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
        .unwrap_or("session:default");

    // Breadcrumbs from input (protocol-visible event refs), not from SQLite
    let breadcrumbs = request
        .input
        .get("breadcrumbs")
        .cloned()
        .unwrap_or_else(|| {
            // Produce deterministic sample breadcrumbs from input hints
            let failure_kind = request
                .input
                .get("failure_kind")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let sequence = request
                .input
                .get("sequence")
                .and_then(Value::as_u64)
                .unwrap_or(1);

            serde_json::json!([
                {
                    "kind": failure_kind,
                    "ref": format!("event:{}:{}", session_id, sequence),
                    "sequence": sequence,
                    "description": format!("Failure breadcrumb: {} at sequence {}", failure_kind, sequence),
                }
            ])
        });

    Ok(serde_json::json!({
        "kind": "failure_breadcrumbs",
        "provider_package_id": request.provider_package_id,
        "session_id": session_id,
        "breadcrumbs": breadcrumbs,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn summarize_guardrails(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(serde_json::json!({
            "kind": "experience_observability_rejected",
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
        .unwrap_or("session:default");

    // Guardrails from input (protocol-visible audit refs), not from runtime internals
    let guardrails = request
        .input
        .get("guardrails")
        .cloned()
        .unwrap_or_else(|| {
            // Produce deterministic sample guardrails from input hints
            let guardrail_kind = request
                .input
                .get("guardrail_kind")
                .and_then(Value::as_str)
                .unwrap_or("raw_secret_blocked");
            let sequence = request
                .input
                .get("sequence")
                .and_then(Value::as_u64)
                .unwrap_or(1);

            serde_json::json!([
                {
                    "kind": guardrail_kind,
                    "ref": format!("audit:{}:{}", session_id, sequence),
                    "description": format!("Guardrail triggered: {} at sequence {}", guardrail_kind, sequence),
                }
            ])
        });

    let total_guardrails = guardrails.as_array().map(|a| a.len()).unwrap_or(0);

    Ok(serde_json::json!({
        "kind": "guardrail_audit_summary",
        "provider_package_id": request.provider_package_id,
        "session_id": session_id,
        "total_guardrails": total_guardrails,
        "guardrails": guardrails,
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
            "official/experience-observability-lab/describe_observability",
            json!({}),
        );
        assert!(try_handle(&req).is_some());
    }

    #[test]
    fn try_handle_rejects_wrong_package() {
        let req = InprocInvocation {
            capability_id: "official/experience-observability-lab/describe_observability"
                .to_string(),
            provider_package_id: "official/other".to_string(),
            session_id: None,
            input: json!({}),
        };
        assert!(try_handle(&req).is_none());
    }

    #[test]
    fn describe_observability_has_all_surfaces() {
        let req = make_request(
            "official/experience-observability-lab/describe_observability",
            json!({}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        let surfaces = result["surfaces"].as_object().unwrap();
        assert!(surfaces.contains_key("forge_panel"));
        assert!(surfaces.contains_key("assistant_action"));
        assert!(surfaces.contains_key("home_card"));
    }

    #[test]
    fn describe_observability_lists_8_capabilities() {
        let req = make_request(
            "official/experience-observability-lab/describe_observability",
            json!({}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["capabilities"]
                .as_array()
                .map(|a| a.len())
                .unwrap_or(0),
            8,
            "must list 8 capabilities"
        );
    }

    #[test]
    fn session_health_default_is_healthy() {
        let req = make_request(
            "official/experience-observability-lab/summarize_session_health",
            json!({"session_id": "s1", "event_count": 10}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("session_health"));
        assert_eq!(result["status"], json!("healthy"));
    }

    #[test]
    fn session_health_with_failures_is_degraded() {
        let req = make_request(
            "official/experience-observability-lab/summarize_session_health",
            json!({"session_id": "s2", "event_count": 10, "failure_count": 2}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["status"], json!("degraded"));
    }

    #[test]
    fn raw_secret_blocked() {
        let req = make_request(
            "official/experience-observability-lab/summarize_session_health",
            json!({"session_id": "s3", "api_key": "RawSecretExample1234567890abcdefABCDEF123456"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("experience_observability_rejected"));
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn no_forbidden_namespace_in_describe() {
        let req = make_request(
            "official/experience-observability-lab/describe_observability",
            json!({}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        let output_str = serde_json::to_string(&result).unwrap();
        for token in &[
            "kernel.v1.observability.",
            "kernel.v1.experience.",
            "kernel.v1.world.",
            "kernel.v1.scene.",
            "kernel.v1.turn.",
            "kernel.v1.chat.",
            "kernel.v1.memory.",
            "kernel.v1.agent.",
            "kernel.v1.model.",
            "kernel.v1.prompt.",
            "kernel.v1.director.",
        ] {
            assert!(!output_str.contains(token), "must not contain {}", token);
        }
    }

    #[test]
    fn proposal_causality_chain_has_content_address() {
        let req = make_request(
            "official/experience-observability-lab/trace_proposal_causality",
            json!({"proposal_id": "p1"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("proposal_causal_chain"));
        let chain = result["chain"].as_array().unwrap();
        for (i, step) in chain.iter().enumerate() {
            assert!(
                step["content_address"].is_string(),
                "chain step {} must have content_address",
                i
            );
        }
    }
}
