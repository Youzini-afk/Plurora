//! Handler for `official/memory-lab` capabilities.
//!
//! Experience Beta 4 — Memory / Knowledge Package Alpha.
//!
//! Package-owned long-term memory and knowledge: memory record, retrieval
//! trace, proposal-gated update, correction, forget/redaction, branch-aware
//! view, and provenance.
//!
//! Deterministic, no-network, no real model inference. Produces
//! package-owned memory shapes: memory_record, retrieval_trace,
//! branch_view, correction, redaction_plan, provenance.
//!
//! Proposal-gated update: draft_memory_update only produces a
//! proposal/update draft; it does not directly modify persistent state.
//! Forget/redaction produces a redaction plan, not a direct deletion.
//!
//! No `kernel.v1.memory.*`, `kernel.v1.experience.*`, `kernel.v1.world.*`,
//! `kernel.v1.scene.*`, `kernel.v1.turn.*`, `kernel.v1.chat.*`,
//! `kernel.v1.agent.*`, `kernel.v1.model.*`, `kernel.v1.prompt.*`,
//! or `kernel.v1.director.*` namespace references.
//!
//! State terminology: memory_record, retrieval_trace, branch_view,
//! correction, redaction_plan, provenance — not
//! chat/message/prompt/world/scene/turn/character/director.

use serde_json::Value;

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/memory-lab";

// ---------------------------------------------------------------------------
// Memory record kinds
// ---------------------------------------------------------------------------

const MEMORY_RECORD_KINDS: &[&str] = &[
    "fact",
    "preference",
    "observation",
    "correction",
    "summary",
    "context",
];

// ---------------------------------------------------------------------------
// Retrieval match algorithms
// ---------------------------------------------------------------------------

const RETRIEVAL_ALGORITHMS: &[&str] = &[
    "deterministic_keyword_contains",
    "deterministic_key_exact",
    "branch_aware_filter",
];

// ---------------------------------------------------------------------------
// Update draft kinds
// ---------------------------------------------------------------------------

const UPDATE_DRAFT_KINDS: &[&str] = &[
    "add_record",
    "modify_record",
    "correct_record",
    "forget_record",
    "merge_records",
];

// ---------------------------------------------------------------------------
// Redaction plan statuses
// ---------------------------------------------------------------------------

const REDACTION_PLAN_STATUSES: &[&str] = &[
    "draft",
    "pending_approval",
    "approved",
    "applied",
    "rejected",
    "cancelled",
];

// ---------------------------------------------------------------------------
// Branch view scopes
// ---------------------------------------------------------------------------

const BRANCH_VIEW_SCOPES: &[&str] = &[
    "current_branch",
    "all_branches",
    "specified_branch",
    "branch_diff",
];

// ---------------------------------------------------------------------------
// Provenance step kinds
// ---------------------------------------------------------------------------

const PROVENANCE_STEP_KINDS: &[&str] = &[
    "record_created",
    "record_retrieved",
    "update_drafted",
    "correction_applied",
    "redaction_planned",
    "branch_viewed",
    "provenance_traced",
];

// ---------------------------------------------------------------------------
// Raw-secret detection (delegated to shared safety module)
// ---------------------------------------------------------------------------

use super::safety;

fn rejected_output(request: &InprocInvocation) -> Value {
    serde_json::json!({
        "kind": "memory_lab_rejected",
        "redaction_state": "unsafe_blocked",
        "reason": "input contains raw-secret-like content; use secret_ref references instead",
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    })
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }
    let id = request.capability_id.as_str();
    if id.ends_with("/describe_memory_contract") {
        Some(describe_memory_contract(request))
    } else if id.ends_with("/record_memory") {
        Some(record_memory(request))
    } else if id.ends_with("/retrieve_memory") {
        Some(retrieve_memory(request))
    } else if id.ends_with("/trace_retrieval") {
        Some(trace_retrieval(request))
    } else if id.ends_with("/draft_memory_update") {
        Some(draft_memory_update(request))
    } else if id.ends_with("/apply_memory_correction") {
        Some(apply_memory_correction(request))
    } else if id.ends_with("/draft_forget_redaction") {
        Some(draft_forget_redaction(request))
    } else if id.ends_with("/branch_memory_view") {
        Some(branch_memory_view(request))
    } else if id.ends_with("/explain_memory_provenance") {
        Some(explain_memory_provenance(request))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Capability implementations
// ---------------------------------------------------------------------------

fn describe_memory_contract(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "memory_lab_contract",
        "package_id": request.provider_package_id,
        "package_kind": "ordinary",
        "capabilities": [
            {"id": "official/memory-lab/describe_memory_contract", "purpose": "describe the memory lab package contract"},
            {"id": "official/memory-lab/record_memory", "purpose": "record a package-owned memory entry with content_address and provenance"},
            {"id": "official/memory-lab/retrieve_memory", "purpose": "retrieve memory entries matching a query, deterministic, no embedding/network"},
            {"id": "official/memory-lab/trace_retrieval", "purpose": "produce a retrieval trace showing how entries were matched"},
            {"id": "official/memory-lab/draft_memory_update", "purpose": "draft a proposal-gated memory update (add/modify/merge), no direct state mutation"},
            {"id": "official/memory-lab/apply_memory_correction", "purpose": "produce a correction shape for an existing memory record (proposal-gated)"},
            {"id": "official/memory-lab/draft_forget_redaction", "purpose": "draft a redaction plan for forgetting memory records, no direct deletion"},
            {"id": "official/memory-lab/branch_memory_view", "purpose": "produce a branch-aware view of memory records scoped by branch"},
            {"id": "official/memory-lab/explain_memory_provenance", "purpose": "explain provenance chain of a memory record with content_address per step"},
        ],
        "surfaces": {
            "forge_panel": "official/memory-lab/forge-panel",
            "assistant_action": "official/memory-lab/assistant-action",
            "home_card": "official/memory-lab/home-card",
        },
        "memory_record_kinds": MEMORY_RECORD_KINDS,
        "retrieval_algorithms": RETRIEVAL_ALGORITHMS,
        "update_draft_kinds": UPDATE_DRAFT_KINDS,
        "redaction_plan_statuses": REDACTION_PLAN_STATUSES,
        "branch_view_scopes": BRANCH_VIEW_SCOPES,
        "provenance_step_kinds": PROVENANCE_STEP_KINDS,
        "output_shapes": {
            "memory_record": ["record_id", "kind", "key", "content", "content_address", "branch_ref", "provenance", "disclosure"],
            "retrieval_trace": ["query", "algorithm", "matches", "match_count", "trace_details", "provenance"],
            "update_draft": ["draft_id", "kind", "requires_user_approval", "target_record_ref", "proposed_content", "content_address", "provenance"],
            "correction": ["correction_id", "original_record_ref", "corrected_content", "content_address", "requires_user_approval", "provenance"],
            "redaction_plan": ["plan_id", "status", "target_record_refs", "redaction_scope", "requires_user_approval", "provenance"],
            "branch_view": ["scope", "branch_ref", "records", "record_count", "provenance"],
            "provenance": ["record_id", "chain", "chain[].step", "chain[].ref", "chain[].content_address", "chain[].description"],
        },
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn record_memory(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let kind = request
        .input
        .get("kind")
        .and_then(Value::as_str)
        .filter(|k| MEMORY_RECORD_KINDS.contains(k))
        .unwrap_or("fact");

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

    let branch_ref = request
        .input
        .get("branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:main");

    let disclosure = request
        .input
        .get("disclosure")
        .and_then(Value::as_str)
        .unwrap_or("unspecified");

    let record_id = format!("mem:{}:{}", key, crate::runtime::content_address(content));
    let content_address = crate::runtime::content_address(&format!("{}:{}", key, content));

    let source_refs = request
        .input
        .get("source_refs")
        .cloned()
        .unwrap_or(serde_json::json!([]));

    let knowledge_refs = request
        .input
        .get("knowledge_refs")
        .cloned()
        .unwrap_or(serde_json::json!([]));

    Ok(serde_json::json!({
        "kind": "memory_record",
        "record_id": record_id,
        "record_kind": kind,
        "key": key,
        "content": content,
        "content_address": content_address,
        "branch_ref": branch_ref,
        "disclosure": disclosure,
        "source_refs": source_refs,
        "knowledge_refs": knowledge_refs,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn retrieve_memory(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let query = request
        .input
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_lowercase();

    let records = request
        .input
        .get("records")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let branch_ref = request.input.get("branch_ref").and_then(Value::as_str);

    let mut matches = Vec::new();
    for record in &records {
        // Branch filtering: only include records matching the specified branch (if given)
        if let Some(branch) = branch_ref {
            let record_branch = record
                .get("branch_ref")
                .and_then(Value::as_str)
                .unwrap_or("branch:main");
            if record_branch != branch {
                continue;
            }
        }

        let keys = record
            .get("key")
            .and_then(Value::as_str)
            .map(|k| vec![k.to_string()])
            .or_else(|| {
                record.get("keys").and_then(Value::as_array).map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
            })
            .unwrap_or_default();

        let hit = keys
            .iter()
            .any(|k| query.contains(&k.to_lowercase()) || k.to_lowercase().contains(&query));

        if hit
            || record
                .get("constant")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            matches.push(serde_json::json!({
                "record": record,
                "reason": if hit { "keyword" } else { "constant" },
            }));
        }
    }

    Ok(serde_json::json!({
        "kind": "retrieval_result",
        "query": request.input.get("query").cloned().unwrap_or(Value::Null),
        "algorithm": "deterministic_keyword_contains",
        "branch_ref": request.input.get("branch_ref").cloned().unwrap_or(Value::Null),
        "matches": matches,
        "match_count": matches.len(),
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn trace_retrieval(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let query = request
        .input
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let algorithm = request
        .input
        .get("algorithm")
        .and_then(Value::as_str)
        .filter(|a| RETRIEVAL_ALGORITHMS.contains(a))
        .unwrap_or("deterministic_keyword_contains");

    let records = request
        .input
        .get("records")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let trace_steps = vec![
        serde_json::json!({
            "step": "query_received",
            "detail": format!("query='{}' algorithm={}", query, algorithm),
        }),
        serde_json::json!({
            "step": "records_scanned",
            "detail": format!("{} records evaluated", records.len()),
        }),
        serde_json::json!({
            "step": "keyword_match",
            "detail": "case-insensitive substring match on record key/content",
        }),
        serde_json::json!({
            "step": "branch_filter",
            "detail": "branch-aware filtering applied if branch_ref specified",
        }),
    ];

    Ok(serde_json::json!({
        "kind": "retrieval_trace",
        "query": request.input.get("query").cloned().unwrap_or(Value::Null),
        "algorithm": algorithm,
        "trace": trace_steps,
        "record_count": records.len(),
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn draft_memory_update(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    // Proposal-gated: only produces a draft, does NOT modify any persistent state
    let draft_kind = request
        .input
        .get("update_kind")
        .and_then(Value::as_str)
        .filter(|k| UPDATE_DRAFT_KINDS.contains(k))
        .unwrap_or("add_record");

    let target_record_ref = request
        .input
        .get("target_record_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    let proposed_content = request
        .input
        .get("proposed_content")
        .cloned()
        .unwrap_or(Value::Null);

    let key = request
        .input
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or("memory:update");

    let branch_ref = request
        .input
        .get("branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:main");

    let draft_id = format!(
        "draft:{}:{}",
        key,
        crate::runtime::content_address(&format!("{:?}", proposed_content))
    );
    let content_address =
        crate::runtime::content_address(&format!("draft:{}:{}", key, proposed_content));

    Ok(serde_json::json!({
        "kind": "memory_update_draft",
        "draft_id": draft_id,
        "update_kind": draft_kind,
        "requires_user_approval": true,
        "target_record_ref": if target_record_ref.is_empty() { Value::Null } else { serde_json::json!(target_record_ref) },
        "key": key,
        "proposed_content": proposed_content,
        "branch_ref": branch_ref,
        "content_address": content_address,
        "disclosure": request.input.get("disclosure").and_then(Value::as_str).unwrap_or("unspecified"),
        "plan_only": true,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn apply_memory_correction(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    // Correction produces a proposal-gated shape, does not directly modify state
    let original_record_ref = request
        .input
        .get("original_record_ref")
        .and_then(Value::as_str)
        .unwrap_or("mem:unknown");

    let corrected_content = request
        .input
        .get("corrected_content")
        .cloned()
        .unwrap_or(Value::Null);

    let correction_reason = request
        .input
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("user_correction");

    let correction_id = format!(
        "correction:{}:{}",
        original_record_ref,
        crate::runtime::content_address(&format!("{:?}", corrected_content))
    );
    let content_address = crate::runtime::content_address(&format!(
        "correction:{}:{:?}",
        original_record_ref, corrected_content
    ));

    Ok(serde_json::json!({
        "kind": "memory_correction",
        "correction_id": correction_id,
        "original_record_ref": original_record_ref,
        "corrected_content": corrected_content,
        "reason": correction_reason,
        "content_address": content_address,
        "requires_user_approval": true,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn draft_forget_redaction(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    // Forget/redaction produces a redaction plan, does NOT directly delete
    let target_record_refs = request
        .input
        .get("target_record_refs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_else(|| {
            // Also support single target_record_ref
            match request
                .input
                .get("target_record_ref")
                .and_then(Value::as_str)
            {
                Some(r) => vec![serde_json::json!(r)],
                None => vec![],
            }
        });

    let redaction_scope = request
        .input
        .get("redaction_scope")
        .and_then(Value::as_str)
        .unwrap_or("record_only");

    let reason = request
        .input
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("user_requested_forget");

    let branch_ref = request
        .input
        .get("branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:main");

    let plan_id = format!(
        "redaction:{}:{}",
        crate::runtime::content_address(&format!("{:?}", target_record_refs)),
        reason
    );

    Ok(serde_json::json!({
        "kind": "memory_redaction_plan",
        "plan_id": plan_id,
        "status": "draft",
        "target_record_refs": target_record_refs,
        "redaction_scope": redaction_scope,
        "reason": reason,
        "branch_ref": branch_ref,
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

fn branch_memory_view(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let scope = request
        .input
        .get("scope")
        .and_then(Value::as_str)
        .filter(|s| BRANCH_VIEW_SCOPES.contains(s))
        .unwrap_or("current_branch");

    let branch_ref = request
        .input
        .get("branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:main");

    let records = request
        .input
        .get("records")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    // Filter records by branch scope
    let filtered: Vec<Value> = match scope {
        "current_branch" | "specified_branch" => records
            .into_iter()
            .filter(|r| {
                r.get("branch_ref")
                    .and_then(Value::as_str)
                    .unwrap_or("branch:main")
                    == branch_ref
            })
            .collect(),
        "all_branches" => records,
        "branch_diff" => {
            // Return records grouped by branch
            records
        }
        _ => records,
    };

    Ok(serde_json::json!({
        "kind": "memory_branch_view",
        "scope": scope,
        "branch_ref": branch_ref,
        "records": filtered,
        "record_count": filtered.len(),
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn explain_memory_provenance(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let record_id = request
        .input
        .get("record_id")
        .and_then(Value::as_str)
        .unwrap_or("mem:unknown");

    let correction_ref = request
        .input
        .get("correction_ref")
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    let redaction_ref = request
        .input
        .get("redaction_ref")
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    let mut chain = vec![
        serde_json::json!({
            "step": "record_created",
            "ref": record_id,
            "content_address": crate::runtime::content_address(&format!("record:{}", record_id)),
            "description": "Memory record initially created"
        }),
        serde_json::json!({
            "step": "record_retrieved",
            "ref": format!("retrieval:{}", record_id),
            "content_address": crate::runtime::content_address(&format!("retrieval:{}", record_id)),
            "description": "Memory record retrieved by query"
        }),
    ];

    if let Some(ref correction) = correction_ref {
        chain.push(serde_json::json!({
            "step": "correction_applied",
            "ref": correction,
            "content_address": crate::runtime::content_address(&format!("correction:{}", correction)),
            "description": "User correction applied to memory record"
        }));
    }

    if let Some(ref redaction) = redaction_ref {
        chain.push(serde_json::json!({
            "step": "redaction_planned",
            "ref": redaction,
            "content_address": crate::runtime::content_address(&format!("redaction:{}", redaction)),
            "description": "Redaction plan created for memory record"
        }));
    }

    Ok(serde_json::json!({
        "kind": "memory_provenance",
        "record_id": record_id,
        "chain": chain,
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
        let req = make_request("official/memory-lab/describe_memory_contract", json!({}));
        assert!(try_handle(&req).is_some());
    }

    #[test]
    fn try_handle_rejects_wrong_package() {
        let req = InprocInvocation {
            capability_id: "official/memory-lab/describe_memory_contract".to_string(),
            provider_package_id: "official/other".to_string(),
            session_id: None,
            input: json!({}),
        };
        assert!(try_handle(&req).is_none());
    }

    #[test]
    fn describe_contract_has_all_surfaces() {
        let req = make_request("official/memory-lab/describe_memory_contract", json!({}));
        let result = try_handle(&req).unwrap().unwrap();
        let surfaces = result["surfaces"].as_object().unwrap();
        assert!(surfaces.contains_key("forge_panel"));
        assert!(surfaces.contains_key("assistant_action"));
        assert!(surfaces.contains_key("home_card"));
    }

    #[test]
    fn describe_contract_lists_9_capabilities() {
        let req = make_request("official/memory-lab/describe_memory_contract", json!({}));
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["capabilities"]
                .as_array()
                .map(|a| a.len())
                .unwrap_or(0),
            9,
            "must list 9 capabilities"
        );
    }

    #[test]
    fn record_memory_produces_content_address() {
        let req = make_request(
            "official/memory-lab/record_memory",
            json!({"key": "test_key", "content": "test content", "kind": "fact"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("memory_record"));
        assert!(result["content_address"].is_string());
        assert_eq!(result["record_kind"], json!("fact"));
    }

    #[test]
    fn retrieve_memory_keyword_match() {
        let req = make_request(
            "official/memory-lab/retrieve_memory",
            json!({
                "query": "dragon",
                "records": [
                    {"key": "dragon_type", "content": "fire", "branch_ref": "branch:main"},
                    {"key": "location", "content": "castle", "branch_ref": "branch:main"},
                ]
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("retrieval_result"));
        assert_eq!(result["match_count"], json!(1));
    }

    #[test]
    fn draft_update_is_proposal_only() {
        let req = make_request(
            "official/memory-lab/draft_memory_update",
            json!({"key": "update1", "update_kind": "add_record"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("memory_update_draft"));
        assert_eq!(result["requires_user_approval"], json!(true));
        assert_eq!(result["plan_only"], json!(true));
    }

    #[test]
    fn forget_produces_redaction_plan_not_deletion() {
        let req = make_request(
            "official/memory-lab/draft_forget_redaction",
            json!({"target_record_ref": "mem:test:abc123", "reason": "user_request"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("memory_redaction_plan"));
        assert_eq!(result["status"], json!("draft"));
        assert_eq!(result["requires_user_approval"], json!(true));
        assert_eq!(result["plan_only"], json!(true));
    }

    #[test]
    fn raw_secret_blocked() {
        let req = make_request(
            "official/memory-lab/record_memory",
            json!({"key": "test", "api_key": "RawSecretExample1234567890abcdefABCDEF123456"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("memory_lab_rejected"));
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn no_forbidden_namespace_in_contract() {
        let req = make_request("official/memory-lab/describe_memory_contract", json!({}));
        let result = try_handle(&req).unwrap().unwrap();
        let output_str = serde_json::to_string(&result).unwrap();
        for token in &[
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
        ] {
            assert!(!output_str.contains(token), "must not contain {}", token);
        }
    }

    #[test]
    fn branch_view_filters_by_branch() {
        let req = make_request(
            "official/memory-lab/branch_memory_view",
            json!({
                "scope": "current_branch",
                "branch_ref": "branch:feature1",
                "records": [
                    {"key": "a", "branch_ref": "branch:main"},
                    {"key": "b", "branch_ref": "branch:feature1"},
                ]
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("memory_branch_view"));
        assert_eq!(result["record_count"], json!(1));
    }

    #[test]
    fn provenance_chain_has_content_address() {
        let req = make_request(
            "official/memory-lab/explain_memory_provenance",
            json!({"record_id": "mem:test:abc"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("memory_provenance"));
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
