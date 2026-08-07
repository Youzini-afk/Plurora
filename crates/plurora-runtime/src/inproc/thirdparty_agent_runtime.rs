//! Handler for `thirdparty/agent-runtime` and `thirdparty/memory-lab` capabilities.
//!
//! Demonstrates that third-party packages can produce the same
//! deterministic, no-network, no-inference, proposal-gated output shapes
//! as the official labs without any kernel privilege or special routing.

use serde_json::Value;

use super::InprocInvocation;

const AGENT_RUNTIME_PACKAGE_ID: &str = "thirdparty/agent-runtime";
const MEMORY_LAB_PACKAGE_ID: &str = "thirdparty/memory-lab";

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    // Agent runtime handlers
    if request.provider_package_id == AGENT_RUNTIME_PACKAGE_ID {
        return try_handle_agent_runtime(request);
    }
    // Memory lab handlers
    if request.provider_package_id == MEMORY_LAB_PACKAGE_ID {
        return try_handle_memory_lab(request);
    }
    None
}

fn try_handle_agent_runtime(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    let id = request.capability_id.as_str();
    if id.ends_with("/run") {
        Some(run(request))
    } else if id.ends_with("/explain_run") {
        Some(explain_run(request))
    } else if id.ends_with("/draft_proposal") {
        Some(draft_proposal(request))
    } else if id.ends_with("/summarize_trace") {
        Some(summarize_trace(request))
    } else if id.ends_with("/echo") {
        Some(echo(request))
    } else {
        None
    }
}

fn try_handle_memory_lab(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    let id = request.capability_id.as_str();
    if id.ends_with("/describe_memory_contract") {
        Some(memory_lab_describe_contract(request))
    } else if id.ends_with("/record_memory") {
        Some(memory_lab_record_memory(request))
    } else if id.ends_with("/retrieve_memory") {
        Some(memory_lab_retrieve_memory(request))
    } else if id.ends_with("/trace_retrieval") {
        Some(memory_lab_trace_retrieval(request))
    } else if id.ends_with("/draft_memory_update") {
        Some(memory_lab_draft_update(request))
    } else if id.ends_with("/apply_memory_correction") {
        Some(memory_lab_apply_correction(request))
    } else if id.ends_with("/draft_forget_redaction") {
        Some(memory_lab_draft_forget_redaction(request))
    } else if id.ends_with("/branch_memory_view") {
        Some(memory_lab_branch_view(request))
    } else if id.ends_with("/explain_memory_provenance") {
        Some(memory_lab_explain_provenance(request))
    } else {
        None
    }
}

fn run(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "thirdparty_agent_run_plan",
        "inference_performed": false,
        "network_performed": false,
        "trace_events": [
            {
                "event_type": "capability_invoked",
                "capability_id": request.capability_id,
                "timestamp": 0,
                "payload": {"step": "plan_only", "status": "deterministic"}
            }
        ],
        "stream_frames": [
            {"frame_type": "start", "sequence": 0},
            {"frame_type": "chunk", "sequence": 1, "content": "community deterministic plan step"},
            {"frame_type": "progress", "sequence": 2, "percent": 100},
            {"frame_type": "end", "sequence": 3}
        ],
        "proposal_draft": {
            "kind": "thirdparty_agent_proposal_draft",
            "requires_user_approval": true,
            "recommended_operation": "kernel.v1.session.fork",
            "plan_summary": "deterministic no-inference agent run plan from community runtime"
        },
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn explain_run(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "thirdparty_agent_run_explanation",
        "summary": "Community agent run plan explanation: no inference or network was performed.",
        "inference_performed": false,
        "network_performed": false,
        "trace_event_count": request.input.get("trace_events").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0),
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn draft_proposal(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "thirdparty_agent_proposal",
        "requires_user_approval": true,
        "recommended_operation": "kernel.v1.session.fork",
        "proposal": request.input,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn summarize_trace(request: &InprocInvocation) -> anyhow::Result<Value> {
    let event_count = request
        .input
        .get("trace_events")
        .and_then(Value::as_array)
        .map(|a| a.len())
        .unwrap_or(0);
    Ok(serde_json::json!({
        "kind": "thirdparty_agent_trace_summary",
        "event_count": event_count,
        "inference_performed": false,
        "network_performed": false,
        "summary": format!("Community trace summary: {event_count} events, no inference, no network"),
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn echo(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "thirdparty_agent_echo",
        "input": request.input,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

// ---------------------------------------------------------------------------
// Third-party memory-lab handlers
// ---------------------------------------------------------------------------

fn memory_lab_describe_contract(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "memory_lab_contract",
        "package_id": request.provider_package_id,
        "package_kind": "ordinary",
        "capabilities": [
            {"id": "thirdparty/memory-lab/describe_memory_contract", "purpose": "describe the community memory lab contract"},
            {"id": "thirdparty/memory-lab/record_memory", "purpose": "record a package-owned memory entry"},
            {"id": "thirdparty/memory-lab/retrieve_memory", "purpose": "retrieve memory entries matching a query"},
            {"id": "thirdparty/memory-lab/trace_retrieval", "purpose": "produce a retrieval trace"},
            {"id": "thirdparty/memory-lab/draft_memory_update", "purpose": "draft a proposal-gated memory update"},
            {"id": "thirdparty/memory-lab/apply_memory_correction", "purpose": "produce a correction shape (proposal-gated)"},
            {"id": "thirdparty/memory-lab/draft_forget_redaction", "purpose": "draft a redaction plan for forgetting"},
            {"id": "thirdparty/memory-lab/branch_memory_view", "purpose": "produce a branch-aware memory view"},
            {"id": "thirdparty/memory-lab/explain_memory_provenance", "purpose": "explain provenance of a memory record"},
        ],
        "surfaces": {
            "forge_panel": "thirdparty/memory-lab/forge-panel",
            "assistant_action": "thirdparty/memory-lab/assistant-action",
            "home_card": "thirdparty/memory-lab/home-card",
        },
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn memory_lab_record_memory(request: &InprocInvocation) -> anyhow::Result<Value> {
    let key = request
        .input
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or("memory:default");
    let content = request
        .input
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("");
    let record_id = format!("mem:{}:{}", key, crate::runtime::content_address(content));
    Ok(serde_json::json!({
        "kind": "memory_record",
        "record_id": record_id,
        "record_kind": request.input.get("kind").and_then(Value::as_str).unwrap_or("fact"),
        "key": key,
        "content": content,
        "content_address": crate::runtime::content_address(&format!("{}:{}", key, content)),
        "branch_ref": request.input.get("branch_ref").and_then(Value::as_str).unwrap_or("branch:main"),
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn memory_lab_retrieve_memory(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "retrieval_result",
        "query": request.input.get("query").cloned().unwrap_or(Value::Null),
        "algorithm": "deterministic_keyword_contains",
        "matches": request.input.get("matches").cloned().unwrap_or(serde_json::json!([])),
        "match_count": request.input.get("matches").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0),
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn memory_lab_trace_retrieval(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "retrieval_trace",
        "query": request.input.get("query").cloned().unwrap_or(Value::Null),
        "algorithm": "deterministic_keyword_contains",
        "trace": [
            {"step": "query_received", "detail": "community deterministic retrieval"},
            {"step": "keyword_match", "detail": "case-insensitive substring match"}
        ],
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn memory_lab_draft_update(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "memory_update_draft",
        "draft_id": format!("draft:community:{}", crate::runtime::content_address(&format!("{:?}", request.input))),
        "update_kind": request.input.get("update_kind").and_then(Value::as_str).unwrap_or("add_record"),
        "requires_user_approval": true,
        "plan_only": true,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn memory_lab_apply_correction(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "memory_correction",
        "correction_id": format!("correction:community:{}", crate::runtime::content_address(&format!("{:?}", request.input))),
        "original_record_ref": request.input.get("original_record_ref").and_then(Value::as_str).unwrap_or("mem:unknown"),
        "requires_user_approval": true,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn memory_lab_draft_forget_redaction(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "memory_redaction_plan",
        "plan_id": format!("redaction:community:{}", crate::runtime::content_address(&format!("{:?}", request.input))),
        "status": "draft",
        "requires_user_approval": true,
        "plan_only": true,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn memory_lab_branch_view(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "memory_branch_view",
        "scope": request.input.get("scope").and_then(Value::as_str).unwrap_or("current_branch"),
        "branch_ref": request.input.get("branch_ref").and_then(Value::as_str).unwrap_or("branch:main"),
        "records": request.input.get("records").cloned().unwrap_or(serde_json::json!([])),
        "record_count": request.input.get("records").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0),
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn memory_lab_explain_provenance(request: &InprocInvocation) -> anyhow::Result<Value> {
    let record_id = request
        .input
        .get("record_id")
        .and_then(Value::as_str)
        .unwrap_or("mem:unknown");
    Ok(serde_json::json!({
        "kind": "memory_provenance",
        "record_id": record_id,
        "chain": [
            {
                "step": "record_created",
                "ref": record_id,
                "content_address": crate::runtime::content_address(&format!("record:{}", record_id)),
                "description": "Memory record created (community lab)"
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
