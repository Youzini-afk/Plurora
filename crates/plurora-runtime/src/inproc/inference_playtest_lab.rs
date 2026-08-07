//! Handler for `official/inference-playtest-lab` capabilities.
//!
//! Ygg-native inference proposal vertical slice. Proves inference is not
//! "prompt -> text response", but ordinary package participation in
//! session / branch / proposal / inspection / fork creative runtime.
//!
//! This package does NOT call kernel.v1.proposal.create directly.
//! It produces proposal drafts that must go through the existing
//! proposal lifecycle (create → approve/reject → apply).

use serde_json::Value;

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/inference-playtest-lab";

// Secret-looking field names that must be rejected
const SECRET_FIELD_NAMES: &[&str] = &[
    "api_key",
    "apiKey",
    "secret",
    "password",
    "token",
    "credential",
];

// ---------------------------------------------------------------------------
// Top-level dispatch
// ---------------------------------------------------------------------------

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }
    let id = request.capability_id.as_str();
    if id.ends_with("/draft_proposal") {
        Some(draft_proposal(request))
    } else if id.ends_with("/inspect_proposal") {
        Some(inspect_proposal(request))
    } else if id.ends_with("/branch_plan") {
        Some(branch_plan(request))
    } else if id.ends_with("/explain_flow") {
        Some(explain_flow(request))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// draft_proposal
// ---------------------------------------------------------------------------

fn draft_proposal(request: &InprocInvocation) -> anyhow::Result<Value> {
    let input = &request.input;

    // Reject raw secret-looking fields
    if looks_like_raw_secret_field(input) {
        return Ok(serde_json::json!({
            "kind": "inference_playtest_error",
            "error_kind": "secret_rejected",
            "message": "raw secret fields are not accepted; inference playtest lab does not require secrets",
            "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
        }));
    }

    let session_id = input
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown_session");

    let branch_id = input.get("branch_id").and_then(Value::as_str);

    let inference_result = input
        .get("inference_result")
        .cloned()
        .unwrap_or(Value::Null);

    let intent = input
        .get("intent")
        .and_then(Value::as_str)
        .unwrap_or("unspecified");

    let asset_content = input
        .get("asset_content")
        .cloned()
        .unwrap_or(serde_json::json!({
            "inference_derived": true,
            "operation_kind": inference_result.get("operation_kind").and_then(Value::as_str).unwrap_or("generate"),
            "output_payload": inference_result.get("output_payload").cloned().unwrap_or(Value::Null),
        }));

    // Build proposal draft — NOT calling kernel.v1.proposal.create
    // The caller (conformance/host) must feed this into kernel.v1.proposal.create
    let operations = vec![serde_json::json!({
        "op": "asset.put",
        "payload": {
            "mime": "application/json",
            "content": serde_json::to_string(&asset_content).unwrap_or_else(|_| "{}".to_string()),
        },
    })];

    let source_inference = serde_json::json!({
        "package_id": "official/inference-local-lab",
        "operation_kind": inference_result.get("operation_kind").and_then(Value::as_str).unwrap_or("generate"),
        "operation_id": inference_result.get("operation_id").and_then(Value::as_str).unwrap_or("unknown"),
        "inference_result_kind": inference_result.get("kind").and_then(Value::as_str).unwrap_or("unknown"),
        "transport_performed": inference_result.get("transport_performed").and_then(Value::as_str).unwrap_or("none"),
        "network_performed": inference_result.get("network_performed").and_then(Value::as_bool).unwrap_or(false),
    });

    let expected_effects = serde_json::json!({
        "summary": "write inference-derived artifact as asset",
        "inference_derived": true,
        "asset_kind": "inference_artifact",
        "source_inference": source_inference,
    });

    let required_permissions = vec!["assets.write".to_string()];

    Ok(serde_json::json!({
        "kind": "inference_playtest_proposal_draft",
        "session_id": session_id,
        "branch_id": branch_id,
        "operations": operations,
        "expected_effects": expected_effects,
        "required_permissions": required_permissions,
        "inspection": {
            "risk": "low",
            "operations_summary": ["asset.put: inference-derived JSON artifact"],
            "permissions_required": required_permissions,
        },
        "requires_user_approval": true,
        "source_inference": source_inference,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id,
            "intent": intent,
        },
    }))
}

// ---------------------------------------------------------------------------
// inspect_proposal
// ---------------------------------------------------------------------------

fn inspect_proposal(request: &InprocInvocation) -> anyhow::Result<Value> {
    let input = &request.input;

    // Reject raw secret-looking fields
    if looks_like_raw_secret_field(input) {
        return Ok(serde_json::json!({
            "kind": "inference_playtest_error",
            "error_kind": "secret_rejected",
            "message": "raw secret fields are not accepted",
            "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
        }));
    }

    let proposal = input.get("proposal").cloned().unwrap_or(Value::Null);
    let proposal_id = proposal
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");

    let status = proposal
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");

    let operations = proposal
        .get("operations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let required_permissions = proposal
        .get("required_permissions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let source_inference = proposal
        .get("source_inference")
        .cloned()
        .or_else(|| {
            proposal
                .get("expected_effects")
                .and_then(|effects| effects.get("source_inference"))
                .cloned()
        })
        .unwrap_or(Value::Null);

    // Determine risk from operations
    let risk = if operations.iter().any(|op| {
        op.get("op")
            .and_then(Value::as_str)
            .map(|s| s != "asset.put" && s != "projection.rebuild")
            .unwrap_or(false)
    }) {
        "medium"
    } else {
        "low"
    };

    let operations_summary: Vec<Value> = operations
        .iter()
        .map(|op| {
            let op_name = op.get("op").and_then(Value::as_str).unwrap_or("unknown");
            serde_json::json!(format!("{}: inference-derived artifact", op_name))
        })
        .collect();

    Ok(serde_json::json!({
        "kind": "inference_playtest_inspection",
        "proposal_id": proposal_id,
        "status": status,
        "risk": risk,
        "operations_summary": operations_summary,
        "permissions": required_permissions,
        "provenance": {
            "source_inference": source_inference,
            "inspected_by": request.provider_package_id,
            "capability_id": request.capability_id,
        },
    }))
}

// ---------------------------------------------------------------------------
// branch_plan
// ---------------------------------------------------------------------------

fn branch_plan(request: &InprocInvocation) -> anyhow::Result<Value> {
    let input = &request.input;

    // Reject raw secret-looking fields
    if looks_like_raw_secret_field(input) {
        return Ok(serde_json::json!({
            "kind": "inference_playtest_error",
            "error_kind": "secret_rejected",
            "message": "raw secret fields are not accepted",
            "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
        }));
    }

    let session_id = input
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown_session");

    let proposal_id = input
        .get("proposal_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown_proposal");

    let source_inference = input
        .get("source_inference")
        .cloned()
        .unwrap_or(Value::Null);

    // Return suggested fork metadata — does NOT directly fork
    Ok(serde_json::json!({
        "kind": "inference_playtest_branch_plan",
        "session_id": session_id,
        "fork_metadata": {
            "reason": "inference-derived proposal applied, preserving pre-inference state",
            "proposal_id": proposal_id,
            "source_inference": source_inference,
        },
        "recommended_fork_sequence": 0,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id,
        },
    }))
}

// ---------------------------------------------------------------------------
// explain_flow
// ---------------------------------------------------------------------------

fn explain_flow(request: &InprocInvocation) -> anyhow::Result<Value> {
    // Reject raw secret-looking fields
    if looks_like_raw_secret_field(&request.input) {
        return Ok(serde_json::json!({
            "kind": "inference_playtest_error",
            "error_kind": "secret_rejected",
            "message": "raw secret fields are not accepted",
            "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
        }));
    }

    Ok(serde_json::json!({
        "kind": "inference_playtest_flow_explanation",
        "flow": [
            {"step": 1, "name": "session", "description": "Open a Plurora session — the creative workspace"},
            {"step": 2, "name": "inference", "description": "Invoke an inference capability (e.g. inference-local-lab/invoke) to produce an inference_result"},
            {"step": 3, "name": "proposal", "description": "Call inference-playtest-lab/draft_proposal with the inference_result to produce a proposal_draft"},
            {"step": 4, "name": "inspect", "description": "Call inference-playtest-lab/inspect_proposal to review risk, operations, permissions, and provenance"},
            {"step": 5, "name": "approve_or_reject", "description": "Use kernel.v1.proposal.approve or kernel.v1.proposal.reject on the created proposal"},
            {"step": 6, "name": "apply", "description": "If approved, use kernel.v1.proposal.apply to execute the operations (e.g. asset.put)"},
            {"step": 7, "name": "fork", "description": "Use kernel.v1.session.fork or inference-playtest-lab/branch_plan to create a branch preserving pre-inference state"},
        ],
        "key_properties": [
            "Inference result flows into a proposal draft, not directly into assets",
            "Proposal requires user approval before apply",
            "Rejected proposals cannot be applied",
            "Branch metadata records proposal and inference provenance",
            "No content-shape methods are added to the kernel protocol",
            "Proposal apply uses existing asset.put/projection.rebuild operations",
        ],
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check if input contains raw secret-looking fields.
fn looks_like_raw_secret_field(input: &Value) -> bool {
    if let Some(obj) = input.as_object() {
        for key in obj.keys() {
            if SECRET_FIELD_NAMES.contains(&key.as_str()) {
                if let Some(val) = obj.get(key) {
                    if let Some(s) = val.as_str() {
                        if looks_like_raw_secret_value(s) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Heuristic: value looks like a raw API key / secret.
fn looks_like_raw_secret_value(s: &str) -> bool {
    let s = s.trim();
    if s.starts_with("secret_ref:")
        || s.starts_with("secretRef:")
        || s.starts_with("secret-ref:")
        || s.starts_with("host:")
    {
        return false;
    }
    // Long alphanumeric strings likely to be raw keys
    if s.len() >= 20
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return true;
    }
    // Common API key prefixes
    if s.starts_with("sk-") || s.starts_with("sk_") || s.starts_with("key-") {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(cap_id: &str, input: Value) -> InprocInvocation {
        InprocInvocation {
            capability_id: cap_id.to_string(),
            provider_package_id: PACKAGE_ID.to_string(),
            session_id: None,
            input,
        }
    }

    #[test]
    fn draft_proposal_basic() {
        let req = make_request(
            "official/inference-playtest-lab/draft_proposal",
            serde_json::json!({
                "session_id": "ses_test",
                "inference_result": {
                    "kind": "inference_local_invoke_result",
                    "operation_id": "op_test",
                    "operation_kind": "generate",
                    "output_payload": {"text": "deterministic output"},
                    "transport_performed": "in_memory_fake",
                    "network_performed": false,
                },
                "intent": "create inference artifact",
            }),
        );
        let result = draft_proposal(&req).unwrap();
        assert_eq!(result["kind"], "inference_playtest_proposal_draft");
        assert_eq!(result["session_id"], "ses_test");
        assert_eq!(result["requires_user_approval"], true);
        // Must have asset.put operation
        let ops = result["operations"].as_array().unwrap();
        assert!(
            ops.iter().any(|op| op["op"] == "asset.put"),
            "must have asset.put operation"
        );
        // Must have source_inference provenance
        assert_eq!(
            result["source_inference"]["package_id"],
            "official/inference-local-lab"
        );
        assert_eq!(result["source_inference"]["network_performed"], false);
        // Must NOT contain chat/message/prompt fields
        let output_str = serde_json::to_string(&result).unwrap();
        assert!(!output_str.contains("\"messages\""), "no messages field");
        assert!(!output_str.contains("\"prompt\""), "no prompt field");
        assert!(!output_str.contains("\"chat\""), "no chat field");
    }

    #[test]
    fn draft_proposal_rejects_raw_secret() {
        let req = make_request(
            "official/inference-playtest-lab/draft_proposal",
            serde_json::json!({
                "session_id": "ses_test",
                "api_key": "rawSecretPlaceholder1234567890ABCDEF",
            }),
        );
        let result = draft_proposal(&req).unwrap();
        assert_eq!(result["error_kind"], "secret_rejected");
    }

    #[test]
    fn inspect_proposal_basic() {
        let req = make_request(
            "official/inference-playtest-lab/inspect_proposal",
            serde_json::json!({
                "proposal": {
                    "id": "prp_test",
                    "status": "created",
                    "operations": [{"op": "asset.put", "payload": {"content": "{}"}}],
                    "required_permissions": ["assets.write"],
                    "source_inference": {"package_id": "official/inference-local-lab"},
                },
            }),
        );
        let result = inspect_proposal(&req).unwrap();
        assert_eq!(result["kind"], "inference_playtest_inspection");
        assert_eq!(result["proposal_id"], "prp_test");
        assert_eq!(result["risk"], "low");
        assert!(
            result["provenance"]["source_inference"]["package_id"]
                == "official/inference-local-lab"
        );
    }

    #[test]
    fn branch_plan_basic() {
        let req = make_request(
            "official/inference-playtest-lab/branch_plan",
            serde_json::json!({
                "session_id": "ses_test",
                "proposal_id": "prp_test",
                "source_inference": {"package_id": "official/inference-local-lab"},
            }),
        );
        let result = branch_plan(&req).unwrap();
        assert_eq!(result["kind"], "inference_playtest_branch_plan");
        assert_eq!(result["fork_metadata"]["proposal_id"], "prp_test");
        assert_eq!(
            result["fork_metadata"]["source_inference"]["package_id"],
            "official/inference-local-lab"
        );
    }

    #[test]
    fn explain_flow_basic() {
        let req = make_request(
            "official/inference-playtest-lab/explain_flow",
            serde_json::json!({}),
        );
        let result = explain_flow(&req).unwrap();
        assert_eq!(result["kind"], "inference_playtest_flow_explanation");
        let flow = result["flow"].as_array().unwrap();
        assert!(flow.len() >= 7, "flow must have at least 7 steps");
        // Verify key step names
        let step_names: Vec<&str> = flow
            .iter()
            .map(|s| s["name"].as_str().unwrap_or_default())
            .collect();
        assert!(step_names.contains(&"session"), "must have session step");
        assert!(
            step_names.contains(&"inference"),
            "must have inference step"
        );
        assert!(step_names.contains(&"proposal"), "must have proposal step");
        assert!(step_names.contains(&"inspect"), "must have inspect step");
        assert!(
            step_names.contains(&"approve_or_reject"),
            "must have approve_or_reject step"
        );
        assert!(step_names.contains(&"apply"), "must have apply step");
        assert!(step_names.contains(&"fork"), "must have fork step");
    }
}
