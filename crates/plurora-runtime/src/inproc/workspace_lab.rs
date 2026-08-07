//! Handler for `official/workspace-lab` capabilities.
//!
//! External Project Operating Plane Alpha Phase E2 + E3 —
//! Workspace Action Policy Boundary + Managed Workspace Deterministic Proof.
//!
//! Workspace action policy boundary for external project workspaces.
//! No real execution, no shell, no clone, no install, no run.
//! Deny-by-default fake executor; all dangerous actions require approval.
//!
//! E3 adds deterministic fixture managed workspace capabilities:
//! - create_fixture_workspace: generate workspace descriptor from input/fixtures
//! - inspect_workspace: return fixture descriptor (no filesystem)
//! - read_workspace_metadata: return deterministic fixture metadata
//! - plan_workspace_run: generate run plan (requires_approval, executor_invoked=false)
//! - record_fixture_process_result: record caller-provided fixture result (no real process)
//! - discover_workspace_entrypoints: generate entrypoint candidates from metadata
//! - draft_workspace_patch: generate patch proposal (no file writes)
//!
//! Capabilities:
//! - describe_workspace_contract: describe the workspace lab contract
//! - draft_workspace_creation: draft a plan-only workspace creation proposal
//! - explain_required_permissions: explain permissions required for workspace actions
//! - request_workspace_action: request a workspace action (deny-by-default, fake executor)
//! - summarize_workspace_audit: produce deterministic redacted audit summary
//! - create_fixture_workspace: generate deterministic fixture workspace descriptor
//! - inspect_workspace: return fixture workspace descriptor (no filesystem read)
//! - read_workspace_metadata: return deterministic fixture metadata (no filesystem read)
//! - plan_workspace_run: generate run plan (all actions require_approval, executor_invoked=false)
//! - record_fixture_process_result: record caller-provided fixture result shape (no real process)
//! - discover_workspace_entrypoints: generate entrypoint candidates from metadata/scripts/stack
//! - draft_workspace_patch: generate patch proposal shape (no file writes, requires_approval=true)
//!
//! Safety:
//! - Raw secret blocking (delegated to shared safety module)
//! - No kernel.v1.project/workspace/git/npm/deploy namespace references in outputs
//! - No filesystem reads, no shell, no outbound, no execution
//! - Deny-by-default: executor_invoked=false, execution_performed=false
//! - Approval tokens are not honored in Alpha; proposal_required=true always
//! - Patch target_files validated/redacted; raw secret blocking in patch content
//! - Unsafe local paths still rejected
//! - No forbidden kernel namespace outputs
//! - No new kernel workspace/project protocol

use serde_json::Value;

use super::safety;
use super::InprocInvocation;

const PACKAGE_ID: &str = "official/workspace-lab";

// ---------------------------------------------------------------------------
// Action taxonomy
// ---------------------------------------------------------------------------

const ACTION_TAXONOMY: &[(&str, &str, bool, bool, bool, bool)] = &[
    // (action, risk_level, requires_approval, executes_code, network_required, filesystem_write_required)
    ("clone_project", "high", true, false, true, true),
    ("read_metadata", "low", false, false, false, false),
    ("install_dependencies", "critical", true, true, true, true),
    ("run_command", "critical", true, true, false, true),
    ("run_tests", "high", true, true, false, true),
    ("stop_process", "medium", true, false, false, false),
    ("read_logs", "low", false, false, false, false),
    ("discover_entrypoints", "low", false, false, false, false),
    ("write_patch", "high", true, false, false, true),
    ("deploy_plan", "critical", true, true, true, true),
];

const VALID_ACTIONS: &[&str] = &[
    "clone_project",
    "read_metadata",
    "install_dependencies",
    "run_command",
    "run_tests",
    "stop_process",
    "read_logs",
    "discover_entrypoints",
    "write_patch",
    "deploy_plan",
];

// ---------------------------------------------------------------------------
// Forbidden namespace tokens (must not appear in outputs)
// ---------------------------------------------------------------------------

/// Kernel namespace tokens that must not appear in outputs.
const FORBIDDEN_NAMESPACE_TOKENS: &[&str] = &[
    "kernel.v1.project.",
    "kernel.v1.workspace.",
    "kernel.v1.git.",
    "kernel.v1.npm.",
    "kernel.v1.deploy.",
    "kernel.v1.ide.",
];

/// Check whether a string contains forbidden kernel namespace tokens.
/// Used in tests to verify outputs do not leak kernel namespaces.
#[allow(dead_code)]
fn contains_forbidden_namespace(s: &str) -> bool {
    FORBIDDEN_NAMESPACE_TOKENS.iter().any(|t| s.contains(t))
}

// ---------------------------------------------------------------------------
// Unsafe local path patterns
// ---------------------------------------------------------------------------

/// Check whether a path string looks like an unsafe local path.
fn is_unsafe_local_path(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Path traversal
    if trimmed.contains("..") {
        return true;
    }
    // Home path
    if trimmed.starts_with('~') {
        return true;
    }
    // Absolute sensitive paths
    let lower = trimmed.to_lowercase();
    if lower.starts_with("/etc/")
        || lower.starts_with("/root/")
        || lower.starts_with("/var/")
        || lower.starts_with("/proc/")
        || lower.starts_with("/sys/")
        || lower.starts_with("/dev/")
    {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Stack detection helper (deterministic, metadata-only)
// ---------------------------------------------------------------------------

/// Detect project stack from metadata hints.
fn detect_stack(stack_hint: &str, metadata: &Value) -> &'static str {
    if !stack_hint.is_empty() {
        match stack_hint {
            "node" | "npm" => return "node",
            "rust" | "cargo" => return "rust",
            "python" | "pip" => return "python",
            "static" => return "static",
            _ => {}
        }
    }
    // Fallback: inspect metadata for stack clues
    if let Some(obj) = metadata.as_object() {
        if obj.contains_key("package_json") || obj.contains_key("npm_scripts") {
            return "node";
        }
        if obj.contains_key("cargo_toml") {
            return "rust";
        }
        if obj.contains_key("pyproject_toml") || obj.contains_key("requirements_txt") {
            return "python";
        }
    }
    "unknown"
}

// ---------------------------------------------------------------------------
// Fixture workspace scripts (deterministic, metadata-only)
// ---------------------------------------------------------------------------

/// Generate deterministic fixture scripts based on stack hint.
fn fixture_scripts_for_stack(stack: &str) -> Vec<Value> {
    match stack {
        "node" => vec![
            serde_json::json!({"name": "install", "command": "npm install", "lifecycle": "preinstall", "executes_code": true, "requires_approval": true}),
            serde_json::json!({"name": "build", "command": "npm run build", "lifecycle": "postinstall", "executes_code": true, "requires_approval": true}),
            serde_json::json!({"name": "test", "command": "npm test", "lifecycle": "test", "executes_code": true, "requires_approval": true}),
            serde_json::json!({"name": "start", "command": "npm start", "lifecycle": "start", "executes_code": true, "requires_approval": true}),
        ],
        "rust" => vec![
            serde_json::json!({"name": "build", "command": "cargo build", "lifecycle": "build", "executes_code": true, "requires_approval": true}),
            serde_json::json!({"name": "test", "command": "cargo test", "lifecycle": "test", "executes_code": true, "requires_approval": true}),
        ],
        "python" => vec![
            serde_json::json!({"name": "install", "command": "pip install -r requirements.txt", "lifecycle": "install", "executes_code": true, "requires_approval": true}),
            serde_json::json!({"name": "test", "command": "pytest", "lifecycle": "test", "executes_code": true, "requires_approval": true}),
        ],
        _ => vec![],
    }
}

// ---------------------------------------------------------------------------
// Rejected output for raw-secret input
// ---------------------------------------------------------------------------

fn rejected_output(request: &InprocInvocation) -> Value {
    serde_json::json!({
        "kind": "workspace_lab_rejected",
        "redaction_state": "unsafe_blocked",
        "reason": "input contains raw-secret-like content; use secret_ref references instead",
        "executor_invoked": false,
        "execution_performed": false,
        "inference_performed": false,
        "network_performed": false,
        "filesystem_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    })
}

// ---------------------------------------------------------------------------
// Provenance helper
// ---------------------------------------------------------------------------

fn provenance(request: &InprocInvocation) -> Value {
    serde_json::json!({
        "package_id": request.provider_package_id,
        "capability_id": request.capability_id
    })
}

// ---------------------------------------------------------------------------
// Lookup action taxonomy entry
// ---------------------------------------------------------------------------

fn lookup_action(action: &str) -> Option<(&'static str, &'static str, bool, bool, bool, bool)> {
    ACTION_TAXONOMY
        .iter()
        .find(|(a, _, _, _, _, _)| *a == action)
        .map(|(a, risk, appr, code, net, fs)| (*a, *risk, *appr, *code, *net, *fs))
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }
    let id = request.capability_id.as_str();
    if id.ends_with("/describe_workspace_contract") {
        Some(describe_workspace_contract(request))
    } else if id.ends_with("/draft_workspace_creation") {
        Some(draft_workspace_creation(request))
    } else if id.ends_with("/explain_required_permissions") {
        Some(explain_required_permissions(request))
    } else if id.ends_with("/request_workspace_action") {
        Some(request_workspace_action(request))
    } else if id.ends_with("/summarize_workspace_audit") {
        Some(summarize_workspace_audit(request))
    } else if id.ends_with("/create_fixture_workspace") {
        Some(create_fixture_workspace(request))
    } else if id.ends_with("/inspect_workspace") {
        Some(inspect_workspace(request))
    } else if id.ends_with("/read_workspace_metadata") {
        Some(read_workspace_metadata(request))
    } else if id.ends_with("/plan_workspace_run") {
        Some(plan_workspace_run(request))
    } else if id.ends_with("/record_fixture_process_result") {
        Some(record_fixture_process_result(request))
    } else if id.ends_with("/discover_workspace_entrypoints") {
        Some(discover_workspace_entrypoints(request))
    } else if id.ends_with("/draft_workspace_patch") {
        Some(draft_workspace_patch(request))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Capability implementations — E2 originals
// ---------------------------------------------------------------------------

fn describe_workspace_contract(request: &InprocInvocation) -> anyhow::Result<Value> {
    let action_entries: Vec<Value> = ACTION_TAXONOMY
        .iter()
        .map(|(action, risk, appr, code, net, fs)| {
            serde_json::json!({
                "action": action,
                "risk_level": risk,
                "requires_approval": appr,
                "executes_code": code,
                "network_required": net,
                "filesystem_write_required": fs,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "kind": "workspace_lab_contract",
        "package_id": request.provider_package_id,
        "package_kind": "ordinary",
        "capabilities": [
            {"id": "official/workspace-lab/describe_workspace_contract", "purpose": "describe the workspace lab contract and action taxonomy"},
            {"id": "official/workspace-lab/draft_workspace_creation", "purpose": "draft a plan-only workspace creation proposal, no direct workspace creation"},
            {"id": "official/workspace-lab/explain_required_permissions", "purpose": "explain permissions required for workspace actions"},
            {"id": "official/workspace-lab/request_workspace_action", "purpose": "request a workspace action with deny-by-default policy; no real execution"},
            {"id": "official/workspace-lab/summarize_workspace_audit", "purpose": "produce deterministic redacted audit summary; no raw env/logs/commands/secrets"},
            {"id": "official/workspace-lab/create_fixture_workspace", "purpose": "generate deterministic fixture workspace descriptor; no real creation, no filesystem"},
            {"id": "official/workspace-lab/inspect_workspace", "purpose": "return fixture workspace descriptor; no filesystem read"},
            {"id": "official/workspace-lab/read_workspace_metadata", "purpose": "return deterministic fixture metadata; no filesystem read"},
            {"id": "official/workspace-lab/plan_workspace_run", "purpose": "generate run plan; all actions require_approval, executor_invoked=false"},
            {"id": "official/workspace-lab/record_fixture_process_result", "purpose": "record caller-provided fixture result shape; no real process"},
            {"id": "official/workspace-lab/discover_workspace_entrypoints", "purpose": "generate entrypoint candidates from metadata/scripts/stack_hint"},
            {"id": "official/workspace-lab/draft_workspace_patch", "purpose": "generate patch proposal shape; no file writes, requires_approval=true"},
        ],
        "surfaces": {
            "forge_panel": "official/workspace-lab/forge-panel",
            "assistant_action": "official/workspace-lab/assistant-action",
            "home_card": "official/workspace-lab/home-card",
        },
        "action_taxonomy": action_entries,
        "policy_defaults": {
            "default_decision": "denied_by_default",
            "requires_approval": true,
            "executor_invoked": false,
            "execution_performed": false,
            "proposal_required": true,
            "approval_token_honored": false,
        },
        "managed_workspace_defaults": {
            "managed_workspace_kind": "fixture",
            "workspace_created_in_host": false,
            "execution_performed": false,
            "filesystem_performed": false,
            "network_performed": false,
            "real_creation_requires": ["approval", "policy", "executor"],
        },
        "audit_shape": {
            "fields": ["workspace_ref", "action", "policy_decision", "executor_invoked", "execution_performed", "proposal_required", "audit_preview"],
            "redaction": "no raw env, no raw logs, no raw commands, no raw secrets",
        },
        "inference_performed": false,
        "network_performed": false,
        "execution_performed": false,
        "filesystem_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn draft_workspace_creation(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let workspace_ref = request
        .input
        .get("workspace_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    let source_ref = request
        .input
        .get("source_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    // Collect required actions and their risk levels
    let mut proposed_actions: Vec<Value> = Vec::new();
    let mut risk_notes: Vec<Value> = Vec::new();

    // Typical workspace creation needs: clone + read_metadata + install_dependencies + discover_entrypoints
    for action in &[
        "clone_project",
        "read_metadata",
        "install_dependencies",
        "discover_entrypoints",
    ] {
        if let Some((_, risk, appr, code, net, fs)) = lookup_action(action) {
            proposed_actions.push(serde_json::json!({
                "action": action,
                "risk_level": risk,
                "requires_approval": appr,
                "executes_code": code,
                "network_required": net,
                "filesystem_write_required": fs,
            }));
        }
    }

    risk_notes.push(serde_json::json!({
        "kind": "deny_by_default",
        "note": "all workspace actions are denied by default; each requires explicit approval via proposal"
    }));
    risk_notes.push(serde_json::json!({
        "kind": "no_execution",
        "note": "Alpha phase does not execute any workspace action; executor_invoked is always false"
    }));
    if !source_ref.is_empty() {
        risk_notes.push(serde_json::json!({
            "kind": "source_ref_present",
            "note": "source ref provided; clone or fetch may be required which needs network and policy approval"
        }));
    }

    Ok(serde_json::json!({
        "kind": "workspace_creation_draft",
        "plan_only": true,
        "requires_user_approval": true,
        "workspace_ref": if workspace_ref.is_empty() { Value::Null } else { serde_json::json!(workspace_ref) },
        "source_ref": if source_ref.is_empty() { Value::Null } else { serde_json::json!(source_ref) },
        "proposed_actions": proposed_actions,
        "risk_notes": risk_notes,
        "executor_invoked": false,
        "execution_performed": false,
        "inference_performed": false,
        "network_performed": false,
        "filesystem_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn explain_required_permissions(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let action = request
        .input
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("");

    let mut explanations: Vec<Value> = Vec::new();

    if action.is_empty() {
        // Explain all actions
        for (act, risk, appr, code, net, fs) in ACTION_TAXONOMY {
            explanations.push(serde_json::json!({
                "action": act,
                "risk_level": risk,
                "requires_approval": appr,
                "executes_code": code,
                "network_required": net,
                "filesystem_write_required": fs,
                "explanation": format_permission_explanation(act, *risk, *appr, *code, *net, *fs),
            }));
        }
    } else if let Some((_, risk, appr, code, net, fs)) = lookup_action(action) {
        explanations.push(serde_json::json!({
            "action": action,
            "risk_level": risk,
            "requires_approval": appr,
            "executes_code": code,
            "network_required": net,
            "filesystem_write_required": fs,
            "explanation": format_permission_explanation(action, risk, appr, code, net, fs),
        }));
    } else {
        return Ok(serde_json::json!({
            "kind": "workspace_lab_rejected",
            "redaction_state": "action_unknown",
            "reason": format!("action '{}' is not in the action taxonomy; request denied", action),
            "executor_invoked": false,
            "execution_performed": false,
            "inference_performed": false,
            "network_performed": false,
            "filesystem_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    Ok(serde_json::json!({
        "kind": "workspace_permission_explanation",
        "explanations": explanations,
        "policy_defaults": {
            "default_decision": "denied_by_default",
            "requires_approval": true,
            "approval_token_honored": false,
        },
        "inference_performed": false,
        "network_performed": false,
        "execution_performed": false,
        "filesystem_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn request_workspace_action(request: &InprocInvocation) -> anyhow::Result<Value> {
    // Extract known workspace-lab fields before raw-secret scanning.
    // `approval_token` contains "token" which the shared safety module
    // would flag; we need to handle it specially here.
    let action = request
        .input
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("");

    let workspace_ref = request
        .input
        .get("workspace_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    let source_ref = request
        .input
        .get("source_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    let approval_token = request.input.get("approval_token").and_then(Value::as_str);

    let policy = request
        .input
        .get("policy")
        .and_then(Value::as_str)
        .unwrap_or("");

    // Raw-secret blocking: scan input excluding known workspace-lab fields
    // that the safety module would false-positive on (approval_token contains "token").
    let input_for_scan = {
        let mut obj = request.input.as_object().cloned().unwrap_or_default();
        obj.remove("approval_token"); // remove before scanning; token is a protocol field, not a secret
        serde_json::Value::Object(obj)
    };

    if safety::contains_raw_secret(&input_for_scan) {
        return Ok(serde_json::json!({
            "kind": "workspace_lab_rejected",
            "redaction_state": "unsafe_blocked",
            "reason": "input contains raw-secret-like content; use secret_ref references instead",
            "executor_invoked": false,
            "execution_performed": false,
            "proposal_required": true,
            "inference_performed": false,
            "network_performed": false,
            "filesystem_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    // Validate action is in taxonomy
    if !VALID_ACTIONS.contains(&action) {
        return Ok(serde_json::json!({
            "kind": "workspace_action_rejected",
            "policy_decision": "denied",
            "reason": format!("action '{}' is not in the action taxonomy; fail-closed", action),
            "executor_invoked": false,
            "execution_performed": false,
            "proposal_required": true,
            "audit_preview": {
                "action": if action.is_empty() { Value::Null } else { serde_json::json!(action) },
                "workspace_ref": if workspace_ref.is_empty() { Value::Null } else { serde_json::json!(workspace_ref) },
                "policy_decision": "denied",
                "denial_reason": "unknown_action",
            },
            "inference_performed": false,
            "network_performed": false,
            "filesystem_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    let (_, risk, requires_approval, executes_code, network_required, fs_write) =
        lookup_action(action).unwrap();

    // Policy/action mismatch: if policy claims "allow" but the action taxonomy
    // requires approval or the policy value is invalid, fail closed.
    let valid_policies = ["", "deny", "require_approval", "propose"];
    let policy_mismatch =
        !valid_policies.contains(&policy) || (policy == "allow" && requires_approval);

    if policy_mismatch {
        return Ok(serde_json::json!({
            "kind": "workspace_action_rejected",
            "policy_decision": "denied",
            "reason": "policy/action mismatch: policy must not claim 'allow' for approval-required actions; fail-closed",
            "executor_invoked": false,
            "execution_performed": false,
            "proposal_required": true,
            "audit_preview": {
                "action": action,
                "workspace_ref": if workspace_ref.is_empty() { Value::Null } else { serde_json::json!(workspace_ref) },
                "policy_decision": "denied",
                "denial_reason": "policy_action_mismatch",
            },
            "inference_performed": false,
            "network_performed": false,
            "filesystem_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    // Approval tokens are not honored in Alpha phase
    let approval_token_rejected = approval_token.is_some();

    // Build audit preview (no raw env/logs/commands/secrets)
    let mut audit_preview = serde_json::json!({
        "action": action,
        "workspace_ref": if workspace_ref.is_empty() { Value::Null } else { serde_json::json!(workspace_ref) },
        "policy_decision": "denied_by_default",
        "risk_level": risk,
        "requires_approval": requires_approval,
        "executes_code": executes_code,
        "network_required": network_required,
        "filesystem_write_required": fs_write,
        "approval_token_present": approval_token.is_some(),
        "approval_token_honored": false,
    });

    if !source_ref.is_empty() {
        audit_preview["source_ref_provided"] = serde_json::json!(true);
    }

    // Deny-by-default: always output denied, no execution
    Ok(serde_json::json!({
        "kind": "workspace_action_denied_by_default",
        "policy_decision": "denied_by_default",
        "executor_invoked": false,
        "execution_performed": false,
        "proposal_required": true,
        "audit_preview": audit_preview,
        "risk_level": risk,
        "requires_approval": requires_approval,
        "executes_code": executes_code,
        "network_required": network_required,
        "filesystem_write_required": fs_write,
        "approval_token_rejected": approval_token_rejected,
        "approval_token_honored": false,
        "inference_performed": false,
        "network_performed": false,
        "filesystem_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn summarize_workspace_audit(request: &InprocInvocation) -> anyhow::Result<Value> {
    // Check top-level fields (excluding action_history) for raw secrets.
    // action_history entries are individually redacted below.
    let top_level_for_scan = {
        let mut obj = request.input.as_object().cloned().unwrap_or_default();
        obj.remove("action_history"); // handled per-entry below
        serde_json::Value::Object(obj)
    };
    if safety::contains_raw_secret(&top_level_for_scan) {
        return Ok(rejected_output(request));
    }

    let workspace_ref = request
        .input
        .get("workspace_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    let action_history = request
        .input
        .get("action_history")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    // Produce deterministic redacted audit summary
    // Never include raw env, logs, commands, or secrets
    let mut action_summaries: Vec<Value> = Vec::new();
    let mut total_actions = 0u64;
    let mut denied_count = 0u64;
    let mut approved_count = 0u64;
    let mut pending_count = 0u64;

    for entry in &action_history {
        if safety::contains_raw_secret(entry) {
            // Skip entries containing raw secrets — redact them
            action_summaries.push(serde_json::json!({
                "redacted": true,
                "redaction_reason": "entry contained raw-secret-like content",
            }));
            denied_count += 1;
            total_actions += 1;
            continue;
        }

        let action = entry
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let decision = entry
            .get("policy_decision")
            .and_then(Value::as_str)
            .unwrap_or("unknown");

        // Build redacted summary — only safe fields
        let mut summary = serde_json::json!({
            "action": if VALID_ACTIONS.contains(&action) { action } else { "redacted_unknown" },
            "policy_decision": decision,
            "executor_invoked": entry.get("executor_invoked").and_then(Value::as_bool).unwrap_or(false),
            "execution_performed": entry.get("execution_performed").and_then(Value::as_bool).unwrap_or(false),
        });

        // Include risk_level from taxonomy if known
        if let Some((_, risk, _, _, _, _)) = lookup_action(action) {
            summary["risk_level"] = serde_json::json!(risk);
        }

        // Strip any raw fields that might have leaked in
        for key in &[
            "raw_command",
            "raw_env",
            "raw_log",
            "raw_output",
            "secret",
            "api_key",
            "token",
            "password",
        ] {
            if let Some(obj) = summary.as_object_mut() {
                obj.remove(*key);
            }
        }

        action_summaries.push(summary);
        total_actions += 1;
        match decision {
            "denied" | "denied_by_default" => denied_count += 1,
            "approved" | "allowed" => approved_count += 1,
            "pending" | "proposed" => pending_count += 1,
            _ => {}
        }
    }

    Ok(serde_json::json!({
        "kind": "workspace_audit_summary",
        "workspace_ref": if workspace_ref.is_empty() { Value::Null } else { serde_json::json!(workspace_ref) },
        "total_actions": total_actions,
        "denied_count": denied_count,
        "approved_count": approved_count,
        "pending_count": pending_count,
        "action_summaries": action_summaries,
        "redaction_applied": true,
        "redaction_policy": "no raw env, no raw logs, no raw commands, no raw secrets",
        "inference_performed": false,
        "network_performed": false,
        "execution_performed": false,
        "filesystem_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

// ---------------------------------------------------------------------------
// Capability implementations — E3 Managed Workspace Deterministic Proof
// ---------------------------------------------------------------------------

/// create_fixture_workspace: Generate a deterministic fixture workspace descriptor.
///
/// The descriptor includes workspace_ref, source_ref, source_kind, detected_stack,
/// metadata, scripts, entrypoints, risk_summary, process_state, log_refs, and patch_plan.
/// All derived deterministically from inputs or fixture defaults.
/// No real creation, no filesystem access, no execution, no network.
fn create_fixture_workspace(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let workspace_ref = request
        .input
        .get("workspace_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    let source_ref = request
        .input
        .get("source_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    let stack_hint = request
        .input
        .get("stack_hint")
        .and_then(Value::as_str)
        .unwrap_or("");

    let metadata = request
        .input
        .get("metadata")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    // Reject unsafe local paths in source_ref
    if !source_ref.is_empty() && is_unsafe_local_path(source_ref) {
        return Ok(serde_json::json!({
            "kind": "workspace_lab_rejected",
            "redaction_state": "unsafe_path_blocked",
            "reason": "source_ref is an unsafe local path; workspace creation requires safe paths",
            "executor_invoked": false,
            "execution_performed": false,
            "inference_performed": false,
            "network_performed": false,
            "filesystem_performed": false,
            "provenance": provenance(request),
        }));
    }

    // Classify source kind
    let source_kind = classify_source_kind(source_ref);

    // Detect stack
    let detected_stack = detect_stack(stack_hint, &metadata);

    // Generate deterministic fixture scripts
    let scripts = fixture_scripts_for_stack(detected_stack);

    // Generate deterministic entrypoints
    let entrypoints = entrypoint_candidates_for_stack(detected_stack);

    // Build risk summary
    let risk_summary = build_risk_summary(source_kind, detected_stack, &scripts);

    // Build process_state (fixture: no real process)
    let process_state = serde_json::json!({
        "status": "none",
        "executor_invoked": false,
        "execution_performed": false,
        "pid": Value::Null,
        "started_at": Value::Null,
        "exited_at": Value::Null,
        "exit_code": Value::Null,
    });

    // Build log_refs (fixture: empty, no real logs)
    let log_refs: Vec<Value> = Vec::new();

    // Build patch_plan (fixture: empty, no patches)
    let patch_plan: Vec<Value> = Vec::new();

    let ws_ref = if workspace_ref.is_empty() {
        // Generate deterministic workspace_ref from source_ref
        if source_ref.is_empty() {
            "ws-fixture-default".to_string()
        } else {
            format!("ws-fixture-{}", &source_ref.len().to_string())
        }
    } else {
        workspace_ref.to_string()
    };

    Ok(serde_json::json!({
        "kind": "fixture_workspace_descriptor",
        "managed_workspace_kind": "fixture",
        "workspace_ref": ws_ref,
        "source_ref": if source_ref.is_empty() { Value::Null } else { serde_json::json!(source_ref) },
        "source_kind": source_kind,
        "detected_stack": detected_stack,
        "metadata": metadata,
        "scripts": scripts,
        "entrypoints": entrypoints,
        "risk_summary": risk_summary,
        "process_state": process_state,
        "log_refs": log_refs,
        "patch_plan": patch_plan,
        "workspace_created_in_host": false,
        "execution_performed": false,
        "filesystem_performed": false,
        "network_performed": false,
        "inference_performed": false,
        "real_creation_requires": ["approval", "policy", "executor"],
        "provenance": provenance(request),
    }))
}

/// inspect_workspace: Return fixture workspace descriptor from input.
/// No filesystem read; returns what caller provides or a deterministic fixture.
fn inspect_workspace(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let workspace_ref = request
        .input
        .get("workspace_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    // If caller provides a workspace_descriptor, echo it back with safety fields;
    // otherwise return a minimal fixture descriptor.
    let descriptor = request.input.get("workspace_descriptor").cloned();

    match descriptor {
        Some(desc) => {
            // Validate: no raw secrets, no unsafe paths
            if safety::contains_raw_secret(&desc) {
                return Ok(rejected_output(request));
            }
            Ok(serde_json::json!({
                "kind": "workspace_inspection",
                "workspace_ref": if workspace_ref.is_empty() { Value::Null } else { serde_json::json!(workspace_ref) },
                "descriptor": desc,
                "filesystem_performed": false,
                "execution_performed": false,
                "network_performed": false,
                "inference_performed": false,
                "provenance": provenance(request),
            }))
        }
        None => {
            // Return minimal fixture
            Ok(serde_json::json!({
                "kind": "workspace_inspection",
                "workspace_ref": if workspace_ref.is_empty() { Value::Null } else { serde_json::json!(workspace_ref) },
                "descriptor": {
                    "managed_workspace_kind": "fixture",
                    "status": "fixture_only",
                    "source_kind": "unknown",
                    "detected_stack": "unknown",
                },
                "filesystem_performed": false,
                "execution_performed": false,
                "network_performed": false,
                "inference_performed": false,
                "provenance": provenance(request),
            }))
        }
    }
}

/// read_workspace_metadata: Return deterministic fixture metadata.
/// No filesystem read; returns caller-provided or fixture default.
fn read_workspace_metadata(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let workspace_ref = request
        .input
        .get("workspace_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    // If caller provides metadata, echo it; otherwise return fixture
    let metadata = request.input.get("metadata").cloned();

    Ok(serde_json::json!({
        "kind": "workspace_metadata",
        "workspace_ref": if workspace_ref.is_empty() { Value::Null } else { serde_json::json!(workspace_ref) },
        "metadata": metadata.unwrap_or_else(|| serde_json::json!({
            "fixture": true,
            "source_kind": "unknown",
            "detected_stack": "unknown",
        })),
        "filesystem_performed": false,
        "execution_performed": false,
        "network_performed": false,
        "inference_performed": false,
        "provenance": provenance(request),
    }))
}

/// plan_workspace_run: Generate a run plan based on scripts/entrypoints.
/// All run/install/test actions require_approval; executor_invoked=false.
fn plan_workspace_run(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let workspace_ref = request
        .input
        .get("workspace_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    let scripts = request
        .input
        .get("scripts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let entrypoints = request
        .input
        .get("entrypoints")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    // Build run plan steps from scripts and entrypoints
    let mut run_steps: Vec<Value> = Vec::new();

    for script in &scripts {
        let name = script
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let executes_code = script
            .get("executes_code")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        run_steps.push(serde_json::json!({
            "step_kind": "run_script",
            "script_name": name,
            "requires_approval": true,
            "executes_code": executes_code,
            "executor_invoked": false,
            "network_required": true,
            "filesystem_write_required": true,
        }));
    }

    for entry in &entrypoints {
        let name = entry
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let executes_code = entry
            .get("executes_code")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        run_steps.push(serde_json::json!({
            "step_kind": "run_entrypoint",
            "entrypoint_name": name,
            "requires_approval": true,
            "executes_code": executes_code,
            "executor_invoked": false,
            "network_required": true,
            "filesystem_write_required": true,
        }));
    }

    // If no scripts/entrypoints provided, add a placeholder install step
    if run_steps.is_empty() {
        run_steps.push(serde_json::json!({
            "step_kind": "install",
            "script_name": "install",
            "requires_approval": true,
            "executes_code": true,
            "executor_invoked": false,
            "network_required": true,
            "filesystem_write_required": true,
        }));
    }

    Ok(serde_json::json!({
        "kind": "workspace_run_plan",
        "workspace_ref": if workspace_ref.is_empty() { Value::Null } else { serde_json::json!(workspace_ref) },
        "plan_only": true,
        "requires_user_approval": true,
        "executor_invoked": false,
        "execution_performed": false,
        "filesystem_performed": false,
        "network_performed": false,
        "inference_performed": false,
        "run_steps": run_steps,
        "risk_note": "all run/install/test steps require approval and a host executor; Alpha phase does not execute",
        "provenance": provenance(request),
    }))
}

/// record_fixture_process_result: Record caller-provided fixture result shape.
/// No real process is spawned. The result shape is recorded with redaction.
fn record_fixture_process_result(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let workspace_ref = request
        .input
        .get("workspace_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    let process_ref = request
        .input
        .get("process_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    // Build a redacted result record from caller-provided fixture data
    let exit_code = request.input.get("exit_code").and_then(Value::as_i64);

    let stdout_preview_len = request
        .input
        .get("stdout_preview")
        .and_then(Value::as_str)
        .map(|s| s.len())
        .unwrap_or(0);

    let stderr_preview_len = request
        .input
        .get("stderr_preview")
        .and_then(Value::as_str)
        .map(|s| s.len())
        .unwrap_or(0);

    // Build redacted result — no raw stdout/stderr content
    let mut result_record = serde_json::json!({
        "kind": "fixture_process_result_record",
        "workspace_ref": if workspace_ref.is_empty() { Value::Null } else { serde_json::json!(workspace_ref) },
        "process_ref": if process_ref.is_empty() { Value::Null } else { serde_json::json!(process_ref) },
        "exit_code": exit_code,
        "stdout_preview_length": stdout_preview_len,
        "stderr_preview_length": stderr_preview_len,
        "real_process_spawned": false,
        "execution_performed": false,
        "filesystem_performed": false,
        "network_performed": false,
        "inference_performed": false,
        "provenance": provenance(request),
    });

    // Include only safe/redacted fields from caller input
    if let Some(duration_ms) = request.input.get("duration_ms").and_then(Value::as_i64) {
        result_record["duration_ms"] = serde_json::json!(duration_ms);
    }
    if let Some(status) = request.input.get("status").and_then(Value::as_str) {
        // Only allow safe status values
        let safe_statuses = ["success", "failure", "timeout", "cancelled", "error"];
        if safe_statuses.contains(&status) {
            result_record["status"] = serde_json::json!(status);
        }
    }

    // Strip any raw fields
    for key in &[
        "raw_stdout",
        "raw_stderr",
        "raw_command",
        "raw_env",
        "secret",
        "api_key",
        "token",
        "password",
    ] {
        if let Some(obj) = result_record.as_object_mut() {
            obj.remove(*key);
        }
    }

    Ok(result_record)
}

/// discover_workspace_entrypoints: Generate entrypoint candidates from
/// metadata, scripts, and stack_hint. Deterministic, no filesystem scan.
fn discover_workspace_entrypoints(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let workspace_ref = request
        .input
        .get("workspace_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    let stack_hint = request
        .input
        .get("stack_hint")
        .and_then(Value::as_str)
        .unwrap_or("");

    let metadata = request
        .input
        .get("metadata")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let scripts = request
        .input
        .get("scripts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    // Detect stack
    let detected_stack = detect_stack(stack_hint, &metadata);

    // Start with stack-based entrypoint candidates
    let mut candidates = entrypoint_candidates_for_stack(detected_stack);

    // Add entrypoints from caller-provided scripts
    for script in &scripts {
        let name = script
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let executes_code = script
            .get("executes_code")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        // Don't duplicate entries already in stack-based candidates
        let already_present = candidates
            .iter()
            .any(|c| c.get("name").and_then(Value::as_str) == Some(name));
        if !already_present {
            candidates.push(serde_json::json!({
                "name": name,
                "kind": "script",
                "requires_approval": true,
                "executes_code": executes_code,
                "source": "caller_provided",
            }));
        }
    }

    // Check metadata for additional entrypoint hints
    if let Some(obj) = metadata.as_object() {
        if let Some(bin) = obj.get("bin").and_then(Value::as_object) {
            for (bin_name, _bin_path) in bin {
                let already_present = candidates
                    .iter()
                    .any(|c| c.get("name").and_then(Value::as_str) == Some(bin_name.as_str()));
                if !already_present {
                    candidates.push(serde_json::json!({
                        "name": bin_name,
                        "kind": "binary",
                        "requires_approval": true,
                        "executes_code": true,
                        "source": "metadata_bin",
                    }));
                }
            }
        }
        if let Some(scripts_map) = obj.get("npm_scripts").and_then(Value::as_object) {
            for (script_name, _script_val) in scripts_map {
                let already_present = candidates
                    .iter()
                    .any(|c| c.get("name").and_then(Value::as_str) == Some(script_name.as_str()));
                if !already_present {
                    candidates.push(serde_json::json!({
                        "name": script_name,
                        "kind": "npm_script",
                        "requires_approval": true,
                        "executes_code": true,
                        "source": "metadata_npm_scripts",
                    }));
                }
            }
        }
    }

    Ok(serde_json::json!({
        "kind": "workspace_entrypoint_candidates",
        "workspace_ref": if workspace_ref.is_empty() { Value::Null } else { serde_json::json!(workspace_ref) },
        "detected_stack": detected_stack,
        "candidates": candidates,
        "execution_performed": false,
        "filesystem_performed": false,
        "network_performed": false,
        "inference_performed": false,
        "provenance": provenance(request),
    }))
}

/// draft_workspace_patch: Generate patch proposal shape.
/// No file writes; requires_approval=true; target_files validated/redacted;
/// raw secret blocking in patch content.
fn draft_workspace_patch(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let workspace_ref = request
        .input
        .get("workspace_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    // Validate target_files
    let target_files = request
        .input
        .get("target_files")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut validated_files: Vec<Value> = Vec::new();
    let mut rejected_files: Vec<Value> = Vec::new();

    for file_entry in &target_files {
        let path = file_entry.as_str().unwrap_or("");
        if is_unsafe_local_path(path) {
            rejected_files.push(serde_json::json!({
                "path": path,
                "rejection_reason": "unsafe_local_path",
            }));
        } else if path.is_empty() {
            rejected_files.push(serde_json::json!({
                "path": "",
                "rejection_reason": "empty_path",
            }));
        } else {
            validated_files.push(serde_json::json!(path));
        }
    }

    // Build patch proposal shape
    let patch_description = request
        .input
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("");

    let patch_kind = request
        .input
        .get("patch_kind")
        .and_then(Value::as_str)
        .unwrap_or("modification");

    Ok(serde_json::json!({
        "kind": "workspace_patch_proposal",
        "workspace_ref": if workspace_ref.is_empty() { Value::Null } else { serde_json::json!(workspace_ref) },
        "plan_only": true,
        "requires_user_approval": true,
        "executor_invoked": false,
        "execution_performed": false,
        "filesystem_performed": false,
        "network_performed": false,
        "inference_performed": false,
        "patch_kind": patch_kind,
        "description": if patch_description.is_empty() { Value::Null } else { serde_json::json!(patch_description) },
        "target_files": validated_files,
        "rejected_files": rejected_files,
        "file_write_performed": false,
        "risk_note": "patch is proposal-only; real file writes require approval, policy, and host executor",
        "provenance": provenance(request),
    }))
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Classify source reference kind.
fn classify_source_kind(source_ref: &str) -> &'static str {
    if source_ref.is_empty() {
        return "unknown";
    }
    let lower = source_ref.to_lowercase();
    // npm patterns first (before generic URL check, since npm URLs also start with https://)
    if lower.starts_with("npm:")
        || lower.contains("npmjs.com")
        || lower.contains("registry.npmjs.org")
    {
        return "npm";
    }
    if lower.starts_with("https://") || lower.starts_with("http://") || lower.ends_with(".git") {
        return "git";
    }
    if lower.starts_with("/")
        || lower.starts_with("./")
        || lower.starts_with("../")
        || lower.starts_with("~/")
    {
        return "local";
    }
    if lower.ends_with(".tar.gz") || lower.ends_with(".zip") || lower.ends_with(".tgz") {
        return "archive";
    }
    "unknown"
}

/// Generate deterministic entrypoint candidates based on stack.
fn entrypoint_candidates_for_stack(stack: &str) -> Vec<Value> {
    match stack {
        "node" => vec![
            serde_json::json!({"name": "npm start", "kind": "npm_script", "requires_approval": true, "executes_code": true, "source": "stack_hint"}),
            serde_json::json!({"name": "npm test", "kind": "npm_script", "requires_approval": true, "executes_code": true, "source": "stack_hint"}),
            serde_json::json!({"name": "npm run build", "kind": "npm_script", "requires_approval": true, "executes_code": true, "source": "stack_hint"}),
        ],
        "rust" => vec![
            serde_json::json!({"name": "cargo run", "kind": "cargo_command", "requires_approval": true, "executes_code": true, "source": "stack_hint"}),
            serde_json::json!({"name": "cargo test", "kind": "cargo_command", "requires_approval": true, "executes_code": true, "source": "stack_hint"}),
        ],
        "python" => vec![
            serde_json::json!({"name": "python -m pytest", "kind": "python_command", "requires_approval": true, "executes_code": true, "source": "stack_hint"}),
            serde_json::json!({"name": "python -m venv", "kind": "python_command", "requires_approval": true, "executes_code": true, "source": "stack_hint"}),
        ],
        _ => vec![],
    }
}

/// Build risk summary from source kind, detected stack, and scripts.
fn build_risk_summary(source_kind: &str, detected_stack: &str, scripts: &[Value]) -> Value {
    let executes_code_count = scripts
        .iter()
        .filter(|s| {
            s.get("executes_code")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .count();

    let overall_risk = if executes_code_count > 0 || source_kind == "local" {
        "high"
    } else if source_kind == "git" || source_kind == "npm" {
        "medium"
    } else {
        "low"
    };

    let mut risk_factors: Vec<Value> = Vec::new();
    if executes_code_count > 0 {
        risk_factors.push(serde_json::json!({
            "factor": "lifecycle_scripts_execute_code",
            "count": executes_code_count,
            "severity": "critical",
        }));
    }
    if source_kind == "local" {
        risk_factors.push(serde_json::json!({
            "factor": "local_source_ref",
            "severity": "high",
        }));
    }
    if source_kind == "git" {
        risk_factors.push(serde_json::json!({
            "factor": "git_source_ref",
            "severity": "medium",
        }));
    }
    if detected_stack == "node" {
        risk_factors.push(serde_json::json!({
            "factor": "npm_lifecycle_scripts",
            "severity": "high",
            "detail": "npm install runs preinstall/install/postinstall/prepare scripts",
        }));
    }

    serde_json::json!({
        "overall_risk": overall_risk,
        "risk_factors": risk_factors,
        "recommendation": "do not auto-execute; require approval and host executor for all code-executing actions",
    })
}

// ---------------------------------------------------------------------------
// Permission explanation formatting
// ---------------------------------------------------------------------------

fn format_permission_explanation(
    action: &str,
    risk: &str,
    appr: bool,
    code: bool,
    net: bool,
    fs: bool,
) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if appr {
        parts.push("requires explicit user approval");
    }
    if code {
        parts.push("executes arbitrary code");
    }
    if net {
        parts.push("requires network access");
    }
    if fs {
        parts.push("writes to filesystem");
    }
    let detail = if parts.is_empty() {
        "read-only, no side effects".to_string()
    } else {
        parts.join("; ")
    };
    format!("action '{}' has {} risk and {}", action, risk, detail)
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
            "official/workspace-lab/describe_workspace_contract",
            json!({}),
        );
        assert!(try_handle(&req).is_some());
    }

    #[test]
    fn try_handle_rejects_wrong_package() {
        let req = InprocInvocation {
            capability_id: "official/workspace-lab/describe_workspace_contract".to_string(),
            provider_package_id: "official/other".to_string(),
            session_id: None,
            input: json!({}),
        };
        assert!(try_handle(&req).is_none());
    }

    #[test]
    fn describe_contract_has_all_surfaces() {
        let req = make_request(
            "official/workspace-lab/describe_workspace_contract",
            json!({}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        let surfaces = result["surfaces"].as_object().unwrap();
        assert!(surfaces.contains_key("forge_panel"));
        assert!(surfaces.contains_key("assistant_action"));
        assert!(surfaces.contains_key("home_card"));
    }

    #[test]
    fn describe_contract_lists_12_capabilities() {
        let req = make_request(
            "official/workspace-lab/describe_workspace_contract",
            json!({}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["capabilities"]
                .as_array()
                .map(|a| a.len())
                .unwrap_or(0),
            12,
            "must list 12 capabilities (5 original + 7 E3)"
        );
    }

    #[test]
    fn describe_contract_lists_10_action_taxonomy_entries() {
        let req = make_request(
            "official/workspace-lab/describe_workspace_contract",
            json!({}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["action_taxonomy"]
                .as_array()
                .map(|a| a.len())
                .unwrap_or(0),
            10,
            "must list 10 action taxonomy entries"
        );
    }

    #[test]
    fn describe_contract_deny_by_default() {
        let req = make_request(
            "official/workspace-lab/describe_workspace_contract",
            json!({}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["policy_defaults"]["default_decision"],
            json!("denied_by_default")
        );
        assert_eq!(result["policy_defaults"]["executor_invoked"], json!(false));
        assert_eq!(
            result["policy_defaults"]["execution_performed"],
            json!(false)
        );
    }

    #[test]
    fn describe_contract_managed_workspace_defaults() {
        let req = make_request(
            "official/workspace-lab/describe_workspace_contract",
            json!({}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["managed_workspace_defaults"]["managed_workspace_kind"],
            json!("fixture")
        );
        assert_eq!(
            result["managed_workspace_defaults"]["workspace_created_in_host"],
            json!(false)
        );
    }

    #[test]
    fn draft_creation_is_plan_only() {
        let req = make_request(
            "official/workspace-lab/draft_workspace_creation",
            json!({"workspace_ref": "ws-001", "source_ref": "https://example.com/project.git"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_creation_draft"));
        assert_eq!(result["plan_only"], json!(true));
        assert_eq!(result["requires_user_approval"], json!(true));
        assert_eq!(result["executor_invoked"], json!(false));
        assert_eq!(result["execution_performed"], json!(false));
    }

    #[test]
    fn request_action_denied_by_default() {
        let req = make_request(
            "official/workspace-lab/request_workspace_action",
            json!({"action": "clone_project", "workspace_ref": "ws-001"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_action_denied_by_default"));
        assert_eq!(result["policy_decision"], json!("denied_by_default"));
        assert_eq!(result["executor_invoked"], json!(false));
        assert_eq!(result["execution_performed"], json!(false));
        assert_eq!(result["proposal_required"], json!(true));
    }

    #[test]
    fn request_action_unknown_action_denied() {
        let req = make_request(
            "official/workspace-lab/request_workspace_action",
            json!({"action": "hack_the_planet", "workspace_ref": "ws-001"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_action_rejected"));
        assert_eq!(result["policy_decision"], json!("denied"));
    }

    #[test]
    fn request_action_policy_mismatch_fail_closed() {
        let req = make_request(
            "official/workspace-lab/request_workspace_action",
            json!({"action": "install_dependencies", "workspace_ref": "ws-001", "policy": "allow"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_action_rejected"));
        assert_eq!(result["policy_decision"], json!("denied"));
    }

    #[test]
    fn request_action_approval_token_not_honored() {
        let req = make_request(
            "official/workspace-lab/request_workspace_action",
            json!({"action": "clone_project", "workspace_ref": "ws-001", "approval_token": "fake-token-12345"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["policy_decision"], json!("denied_by_default"));
        assert_eq!(result["executor_invoked"], json!(false));
        assert_eq!(result["execution_performed"], json!(false));
        assert_eq!(result["approval_token_rejected"], json!(true));
        assert_eq!(result["approval_token_honored"], json!(false));
    }

    #[test]
    fn raw_secret_blocked() {
        let req = make_request(
            "official/workspace-lab/request_workspace_action",
            json!({"action": "clone_project", "workspace_ref": "ws-001", "api_key": "RawSecretExample1234567890abcdefABCDEF123456"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_lab_rejected"));
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn audit_summary_redacted() {
        let req = make_request(
            "official/workspace-lab/summarize_workspace_audit",
            json!({
                "workspace_ref": "ws-001",
                "action_history": [
                    {"action": "clone_project", "policy_decision": "denied_by_default", "executor_invoked": false, "execution_performed": false},
                    {"action": "read_metadata", "policy_decision": "approved", "executor_invoked": false, "execution_performed": false},
                    {"action": "run_command", "policy_decision": "pending", "executor_invoked": false, "execution_performed": false, "raw_command": "rm -rf /"},
                ]
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_audit_summary"));
        assert_eq!(result["total_actions"], json!(3));
        assert_eq!(result["denied_count"], json!(1));
        assert_eq!(result["approved_count"], json!(1));
        assert_eq!(result["pending_count"], json!(1));
        assert_eq!(result["redaction_applied"], json!(true));

        // Verify no raw_command leaked
        let output_str = serde_json::to_string(&result).unwrap();
        assert!(!output_str.contains("rm -rf"));
        assert!(!output_str.contains("raw_command"));
        assert!(!output_str.contains("raw_env"));
        assert!(!output_str.contains("raw_log"));
    }

    #[test]
    fn audit_summary_raw_secret_redacted() {
        let req = make_request(
            "official/workspace-lab/summarize_workspace_audit",
            json!({
                "workspace_ref": "ws-001",
                "action_history": [
                    {"action": "clone_project", "policy_decision": "denied_by_default", "executor_invoked": false, "execution_performed": false},
                    {"action": "run_command", "policy_decision": "approved", "executor_invoked": false, "execution_performed": false, "secret": "RawSecretExample1234567890abcdefABCDEF123456"},
                ]
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        // The entry with secret should be redacted
        let summaries = result["action_summaries"].as_array().unwrap();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[1]["redacted"], json!(true));
        assert_eq!(result["denied_count"], json!(2));
    }

    #[test]
    fn no_forbidden_namespace_in_contract() {
        let req = make_request(
            "official/workspace-lab/describe_workspace_contract",
            json!({}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        let output_str = serde_json::to_string(&result).unwrap();
        assert!(
            !contains_forbidden_namespace(&output_str),
            "must not contain forbidden namespace tokens"
        );
    }

    #[test]
    fn no_forbidden_namespace_all_capabilities() {
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
            let req = make_request(
                &format!("official/workspace-lab/{}", cap),
                json!({"workspace_ref": "ws-001", "action": "read_metadata"}),
            );
            let result = try_handle(&req).unwrap().unwrap();
            let output_str = serde_json::to_string(&result).unwrap();
            assert!(
                !contains_forbidden_namespace(&output_str),
                "{} must not contain forbidden namespace tokens",
                cap
            );
        }
    }

    #[test]
    fn no_execution_performed() {
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
            let req = make_request(
                &format!("official/workspace-lab/{}", cap),
                json!({"workspace_ref": "ws-001", "action": "clone_project"}),
            );
            let result = try_handle(&req).unwrap().unwrap();
            assert_eq!(
                result["execution_performed"],
                json!(false),
                "{} must have execution_performed=false",
                cap
            );
            assert_eq!(
                result["network_performed"],
                json!(false),
                "{} must have network_performed=false",
                cap
            );
            assert_eq!(
                result["inference_performed"],
                json!(false),
                "{} must have inference_performed=false",
                cap
            );
            assert_eq!(
                result["filesystem_performed"],
                json!(false),
                "{} must have filesystem_performed=false",
                cap
            );
        }
    }

    #[test]
    fn action_taxonomy_complete() {
        assert_eq!(ACTION_TAXONOMY.len(), 10);
        assert_eq!(VALID_ACTIONS.len(), 10);
        for (action, _, _, _, _, _) in ACTION_TAXONOMY {
            assert!(
                VALID_ACTIONS.contains(action),
                "action {} must be in VALID_ACTIONS",
                action
            );
        }
    }

    #[test]
    fn action_taxonomy_risk_annotations() {
        // clone_project: high risk, requires approval, no code exec, network + fs write
        let (_, risk, appr, code, net, fs) = lookup_action("clone_project").unwrap();
        assert_eq!(risk, "high");
        assert!(appr);
        assert!(!code);
        assert!(net);
        assert!(fs);

        // read_metadata: low risk, no approval, no code, no network, no fs write
        let (_, risk, appr, code, net, fs) = lookup_action("read_metadata").unwrap();
        assert_eq!(risk, "low");
        assert!(!appr);
        assert!(!code);
        assert!(!net);
        assert!(!fs);

        // install_dependencies: critical, requires approval, executes code
        let (_, risk, appr, code, net, fs) = lookup_action("install_dependencies").unwrap();
        assert_eq!(risk, "critical");
        assert!(appr);
        assert!(code);
        assert!(net);
        assert!(fs);

        // run_command: critical, executes code
        let (_, risk, appr, code, _, _) = lookup_action("run_command").unwrap();
        assert_eq!(risk, "critical");
        assert!(appr);
        assert!(code);

        // deploy_plan: critical, executes code, network + fs write
        let (_, risk, appr, code, net, fs) = lookup_action("deploy_plan").unwrap();
        assert_eq!(risk, "critical");
        assert!(appr);
        assert!(code);
        assert!(net);
        assert!(fs);
    }

    #[test]
    fn explain_permissions_for_single_action() {
        let req = make_request(
            "official/workspace-lab/explain_required_permissions",
            json!({"action": "clone_project"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_permission_explanation"));
        let explanations = result["explanations"].as_array().unwrap();
        assert_eq!(explanations.len(), 1);
        assert_eq!(explanations[0]["action"], json!("clone_project"));
    }

    #[test]
    fn explain_permissions_for_unknown_action() {
        let req = make_request(
            "official/workspace-lab/explain_required_permissions",
            json!({"action": "teleport"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_lab_rejected"));
        assert_eq!(result["redaction_state"], json!("action_unknown"));
    }

    #[test]
    fn explain_permissions_all_when_no_action() {
        let req = make_request(
            "official/workspace-lab/explain_required_permissions",
            json!({}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_permission_explanation"));
        let explanations = result["explanations"].as_array().unwrap();
        assert_eq!(explanations.len(), 10);
    }

    // -----------------------------------------------------------------------
    // E3 tests
    // -----------------------------------------------------------------------

    #[test]
    fn create_fixture_workspace_basic() {
        let req = make_request(
            "official/workspace-lab/create_fixture_workspace",
            json!({"workspace_ref": "ws-fixture-1", "source_ref": "https://example.com/project.git", "stack_hint": "node"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("fixture_workspace_descriptor"));
        assert_eq!(result["managed_workspace_kind"], json!("fixture"));
        assert_eq!(result["workspace_ref"], json!("ws-fixture-1"));
        assert_eq!(result["source_kind"], json!("git"));
        assert_eq!(result["detected_stack"], json!("node"));
        assert_eq!(result["workspace_created_in_host"], json!(false));
        assert_eq!(result["execution_performed"], json!(false));
        assert_eq!(result["filesystem_performed"], json!(false));
        assert_eq!(result["network_performed"], json!(false));
    }

    #[test]
    fn create_fixture_workspace_no_execution() {
        let req = make_request(
            "official/workspace-lab/create_fixture_workspace",
            json!({"workspace_ref": "ws-fixture-1", "source_ref": "https://example.com/project.git", "stack_hint": "node"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["execution_performed"], json!(false));
        assert_eq!(result["filesystem_performed"], json!(false));
        assert_eq!(result["network_performed"], json!(false));
        assert_eq!(result["inference_performed"], json!(false));
        assert_eq!(result["workspace_created_in_host"], json!(false));
        assert_eq!(result["process_state"]["executor_invoked"], json!(false));
        assert_eq!(result["process_state"]["execution_performed"], json!(false));
    }

    #[test]
    fn create_fixture_workspace_real_creation_requires() {
        let req = make_request(
            "official/workspace-lab/create_fixture_workspace",
            json!({"workspace_ref": "ws-fixture-1"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        let requires = result["real_creation_requires"].as_array().unwrap();
        assert!(requires.iter().any(|v| v == "approval"));
        assert!(requires.iter().any(|v| v == "policy"));
        assert!(requires.iter().any(|v| v == "executor"));
    }

    #[test]
    fn create_fixture_workspace_unsafe_path_rejected() {
        let req = make_request(
            "official/workspace-lab/create_fixture_workspace",
            json!({"workspace_ref": "ws-fixture-1", "source_ref": "../../../etc/passwd"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_lab_rejected"));
        assert_eq!(result["redaction_state"], json!("unsafe_path_blocked"));
    }

    #[test]
    fn create_fixture_workspace_raw_secret_blocked() {
        let req = make_request(
            "official/workspace-lab/create_fixture_workspace",
            json!({"workspace_ref": "ws-fixture-1", "api_key": "RawSecretExample1234567890abcdefABCDEF123456"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_lab_rejected"));
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn inspect_workspace_returns_fixture() {
        let req = make_request(
            "official/workspace-lab/inspect_workspace",
            json!({"workspace_ref": "ws-1"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_inspection"));
        assert_eq!(result["filesystem_performed"], json!(false));
        assert_eq!(result["execution_performed"], json!(false));
    }

    #[test]
    fn inspect_workspace_no_filesystem() {
        let req = make_request(
            "official/workspace-lab/inspect_workspace",
            json!({"workspace_ref": "ws-1", "workspace_descriptor": {"managed_workspace_kind": "fixture", "source_kind": "git"}}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_inspection"));
        assert_eq!(result["filesystem_performed"], json!(false));
    }

    #[test]
    fn read_workspace_metadata_returns_fixture() {
        let req = make_request(
            "official/workspace-lab/read_workspace_metadata",
            json!({"workspace_ref": "ws-1", "metadata": {"version": "1.0.0", "stack": "node"}}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_metadata"));
        assert_eq!(result["metadata"]["version"], json!("1.0.0"));
        assert_eq!(result["filesystem_performed"], json!(false));
    }

    #[test]
    fn read_workspace_metadata_no_filesystem() {
        let req = make_request(
            "official/workspace-lab/read_workspace_metadata",
            json!({"workspace_ref": "ws-1"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_metadata"));
        assert_eq!(result["metadata"]["fixture"], json!(true));
        assert_eq!(result["filesystem_performed"], json!(false));
    }

    #[test]
    fn plan_workspace_run_requires_approval() {
        let req = make_request(
            "official/workspace-lab/plan_workspace_run",
            json!({"workspace_ref": "ws-1", "scripts": [{"name": "build", "executes_code": true}]}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_run_plan"));
        assert_eq!(result["plan_only"], json!(true));
        assert_eq!(result["requires_user_approval"], json!(true));
        assert_eq!(result["executor_invoked"], json!(false));
        assert_eq!(result["execution_performed"], json!(false));
        let steps = result["run_steps"].as_array().unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0]["requires_approval"], json!(true));
        assert_eq!(steps[0]["executor_invoked"], json!(false));
    }

    #[test]
    fn plan_workspace_run_all_steps_require_approval() {
        let req = make_request(
            "official/workspace-lab/plan_workspace_run",
            json!({"workspace_ref": "ws-1", "scripts": [{"name": "install", "executes_code": true}, {"name": "test", "executes_code": true}], "entrypoints": [{"name": "main", "executes_code": true}]}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        let steps = result["run_steps"].as_array().unwrap();
        assert!(steps.iter().all(|s| s["requires_approval"] == json!(true)));
        assert!(steps.iter().all(|s| s["executor_invoked"] == json!(false)));
    }

    #[test]
    fn record_fixture_process_result_no_real_process() {
        let req = make_request(
            "official/workspace-lab/record_fixture_process_result",
            json!({"workspace_ref": "ws-1", "process_ref": "proc-1", "exit_code": 0, "duration_ms": 1500, "status": "success"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("fixture_process_result_record"));
        assert_eq!(result["real_process_spawned"], json!(false));
        assert_eq!(result["execution_performed"], json!(false));
        assert_eq!(result["exit_code"], json!(0));
        assert_eq!(result["duration_ms"], json!(1500));
        assert_eq!(result["status"], json!("success"));
    }

    #[test]
    fn record_fixture_process_result_redacted() {
        let req = make_request(
            "official/workspace-lab/record_fixture_process_result",
            json!({"workspace_ref": "ws-1", "process_ref": "proc-1", "exit_code": 1, "raw_stdout": "sensitive output", "raw_stderr": "error output", "raw_command": "rm -rf /"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        let output_str = serde_json::to_string(&result).unwrap();
        // Raw fields must be stripped
        assert!(!output_str.contains("raw_stdout"));
        assert!(!output_str.contains("raw_stderr"));
        assert!(!output_str.contains("raw_command"));
        assert!(!output_str.contains("sensitive output"));
    }

    #[test]
    fn discover_workspace_entrypoints_deterministic() {
        let req = make_request(
            "official/workspace-lab/discover_workspace_entrypoints",
            json!({"workspace_ref": "ws-1", "stack_hint": "node"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_entrypoint_candidates"));
        assert_eq!(result["detected_stack"], json!("node"));
        let candidates = result["candidates"].as_array().unwrap();
        assert!(
            !candidates.is_empty(),
            "node stack should produce entrypoint candidates"
        );
        // All node candidates should require_approval
        assert!(candidates
            .iter()
            .all(|c| c["requires_approval"] == json!(true)));
        assert_eq!(result["execution_performed"], json!(false));
        assert_eq!(result["filesystem_performed"], json!(false));
    }

    #[test]
    fn discover_workspace_entrypoints_with_scripts() {
        let req = make_request(
            "official/workspace-lab/discover_workspace_entrypoints",
            json!({"workspace_ref": "ws-1", "stack_hint": "node", "scripts": [{"name": "custom-script", "executes_code": true}]}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        let candidates = result["candidates"].as_array().unwrap();
        let has_custom = candidates
            .iter()
            .any(|c| c["name"] == json!("custom-script"));
        assert!(
            has_custom,
            "should include caller-provided script entrypoint"
        );
    }

    #[test]
    fn draft_workspace_patch_proposal_only() {
        let req = make_request(
            "official/workspace-lab/draft_workspace_patch",
            json!({"workspace_ref": "ws-1", "target_files": ["src/main.rs", "Cargo.toml"], "description": "fix typo", "patch_kind": "modification"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_patch_proposal"));
        assert_eq!(result["plan_only"], json!(true));
        assert_eq!(result["requires_user_approval"], json!(true));
        assert_eq!(result["executor_invoked"], json!(false));
        assert_eq!(result["filesystem_performed"], json!(false));
        assert_eq!(result["file_write_performed"], json!(false));
        let target_files = result["target_files"].as_array().unwrap();
        assert_eq!(target_files.len(), 2);
    }

    #[test]
    fn draft_workspace_patch_no_write() {
        let req = make_request(
            "official/workspace-lab/draft_workspace_patch",
            json!({"workspace_ref": "ws-1", "target_files": ["src/main.rs"]}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["file_write_performed"], json!(false));
        assert_eq!(result["filesystem_performed"], json!(false));
        assert_eq!(result["execution_performed"], json!(false));
    }

    #[test]
    fn draft_workspace_patch_unsafe_path_rejected() {
        let req = make_request(
            "official/workspace-lab/draft_workspace_patch",
            json!({"workspace_ref": "ws-1", "target_files": ["../../../etc/passwd", "src/safe.rs"]}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_patch_proposal"));
        let rejected = result["rejected_files"].as_array().unwrap();
        assert_eq!(rejected.len(), 1);
        assert_eq!(rejected[0]["rejection_reason"], json!("unsafe_local_path"));
        let validated = result["target_files"].as_array().unwrap();
        assert_eq!(validated.len(), 1);
    }

    #[test]
    fn draft_workspace_patch_raw_secret_blocked() {
        let req = make_request(
            "official/workspace-lab/draft_workspace_patch",
            json!({"workspace_ref": "ws-1", "target_files": ["src/main.rs"], "api_key": "RawSecretExample1234567890abcdefABCDEF123456"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("workspace_lab_rejected"));
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn create_fixture_workspace_stack_detection() {
        // Test various stack hints
        for (hint, expected) in &[
            ("node", "node"),
            ("npm", "node"),
            ("rust", "rust"),
            ("cargo", "rust"),
            ("python", "python"),
            ("pip", "python"),
            ("static", "static"),
            ("unknown", "unknown"),
        ] {
            let req = make_request(
                "official/workspace-lab/create_fixture_workspace",
                json!({"workspace_ref": "ws-1", "stack_hint": hint}),
            );
            let result = try_handle(&req).unwrap().unwrap();
            assert_eq!(
                result["detected_stack"],
                json!(*expected),
                "stack_hint {} should detect {}",
                hint,
                expected
            );
        }
    }

    #[test]
    fn classify_source_kind_various() {
        assert_eq!(
            classify_source_kind("https://github.com/user/repo.git"),
            "git"
        );
        assert_eq!(classify_source_kind("http://example.com/repo"), "git");
        assert_eq!(classify_source_kind("git@github.com:user/repo.git"), "git");
        assert_eq!(classify_source_kind("npm:express"), "npm");
        assert_eq!(
            classify_source_kind("https://registry.npmjs.org/express"),
            "npm"
        );
        assert_eq!(classify_source_kind("./local/path"), "local");
        assert_eq!(classify_source_kind("/absolute/path"), "local");
        assert_eq!(classify_source_kind("archive.tar.gz"), "archive");
        assert_eq!(classify_source_kind("project.zip"), "archive");
        assert_eq!(classify_source_kind("something"), "unknown");
        assert_eq!(classify_source_kind(""), "unknown");
    }

    #[test]
    fn is_unsafe_local_path_various() {
        assert!(is_unsafe_local_path("../../../etc/passwd"));
        assert!(is_unsafe_local_path("~/secret"));
        assert!(is_unsafe_local_path("/etc/shadow"));
        assert!(is_unsafe_local_path("/root/.ssh"));
        assert!(is_unsafe_local_path("/var/log"));
        assert!(is_unsafe_local_path("/proc/self"));
        assert!(!is_unsafe_local_path("src/main.rs"));
        assert!(!is_unsafe_local_path("./local/path"));
        assert!(!is_unsafe_local_path(""));
    }
}
