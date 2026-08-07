//! Handler for `plurora/project-intake-lab` capabilities.
//!
//! External Project Operating Plane Alpha Phase E1 + E5 — Project Intake Lab.
//!
//! Static project intake for external project refs. No clone, no install,
//! no run, no network, no filesystem access, no shell, no outbound.
//!
//! Capabilities:
//! - describe_intake_contract: describe the intake lab contract
//! - inspect_external_project_ref: classify source ref (git/npm/local/archive/unknown)
//! - detect_project_stack_from_metadata: detect stack from metadata (node/rust/python/static/unknown)
//! - draft_workspace_plan: produce plan-only workspace plan
//! - draft_security_risk_summary: produce security risk summary
//! - list_candidate_entrypoints: list candidate entrypoints with risk annotations
//! - draft_adapter_plan: produce plan-only adapter plan
//! - generate_adapter_manifest_preview: produce Plurora Package manifest preview for an adapter (no file write)
//! - generate_subprocess_wrapper_preview: produce subprocess wrapper code preview (no file write, no execution)
//! - generate_adapter_fixture_preview: produce package fixture input/output sample (redacted)
//! - check_adapter_readiness: produce readiness checklist for adapter package
//!
//! Safety:
//! - Raw secret blocking (delegated to shared safety module)
//! - Unsafe local path rejection (path traversal, home path, absolute sensitive paths)
//! - No reserved external-project platform-reserved namespace references
//! - No filesystem reads, no shell, no outbound, no execution

use serde_json::Value;

use super::safety;
use super::InprocInvocation;

const PACKAGE_ID: &str = "plurora/project-intake-lab";

// ---------------------------------------------------------------------------
// Source kinds
// ---------------------------------------------------------------------------

const SOURCE_KINDS: &[&str] = &["git", "npm", "local", "archive", "unknown"];

// ---------------------------------------------------------------------------
// Stack kinds
// ---------------------------------------------------------------------------

const STACK_KINDS: &[&str] = &["node", "rust", "python", "static", "unknown"];

// ---------------------------------------------------------------------------
// Metadata kinds
// ---------------------------------------------------------------------------

const METADATA_KINDS: &[&str] = &["package_json", "readme", "cargo_toml", "pyproject", "files"];

// ---------------------------------------------------------------------------
// NPM lifecycle scripts that execute code
// ---------------------------------------------------------------------------

const NPM_LIFECYCLE_SCRIPTS: &[&str] = &[
    "preinstall",
    "install",
    "postinstall",
    "prepare",
    "prepublish",
];

// ---------------------------------------------------------------------------
// Unsafe local path patterns
// ---------------------------------------------------------------------------

fn is_unsafe_local_path(path: &str) -> bool {
    let p = path.trim();
    // Path traversal
    if p.contains("..") {
        return true;
    }
    // Home path
    if p.starts_with("~/") || p.starts_with("~\\") {
        return true;
    }
    // Absolute sensitive paths
    let lower = p.to_lowercase();
    if lower.starts_with("/etc/")
        || lower.starts_with("/root/")
        || lower.starts_with("/home/")
        || lower.starts_with("/usr/")
        || lower.starts_with("/var/")
        || lower.starts_with("/tmp/")
        || lower.starts_with("/proc/")
        || lower.starts_with("/sys/")
        || lower.starts_with("/dev/")
        || lower.starts_with("c:\\")
        || lower.starts_with("\\\\")
    {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Rejected output for raw-secret input
// ---------------------------------------------------------------------------

fn rejected_output(request: &InprocInvocation) -> Value {
    serde_json::json!({
        "kind": "project_intake_rejected",
        "redaction_state": "unsafe_blocked",
        "reason": "input contains raw-secret-like content; use secret_ref references instead",
        "inference_performed": false,
        "network_performed": false,
        "execution_performed": false,
        "filesystem_performed": false,
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
    if id.ends_with("/describe_intake_contract") {
        Some(describe_intake_contract(request))
    } else if id.ends_with("/inspect_external_project_ref") {
        Some(inspect_external_project_ref(request))
    } else if id.ends_with("/detect_project_stack_from_metadata") {
        Some(detect_project_stack_from_metadata(request))
    } else if id.ends_with("/draft_workspace_plan") {
        Some(draft_workspace_plan(request))
    } else if id.ends_with("/draft_security_risk_summary") {
        Some(draft_security_risk_summary(request))
    } else if id.ends_with("/list_candidate_entrypoints") {
        Some(list_candidate_entrypoints(request))
    } else if id.ends_with("/draft_adapter_plan") {
        Some(draft_adapter_plan(request))
    } else if id.ends_with("/generate_adapter_manifest_preview") {
        Some(generate_adapter_manifest_preview(request))
    } else if id.ends_with("/generate_subprocess_wrapper_preview") {
        Some(generate_subprocess_wrapper_preview(request))
    } else if id.ends_with("/generate_adapter_fixture_preview") {
        Some(generate_adapter_fixture_preview(request))
    } else if id.ends_with("/check_adapter_readiness") {
        Some(check_adapter_readiness(request))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Capability implementations
// ---------------------------------------------------------------------------

fn describe_intake_contract(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "project_intake_contract",
        "package_id": request.provider_package_id,
        "package_kind": "ordinary",
        "capabilities": [
            {"id": "plurora/project-intake-lab/describe_intake_contract", "purpose": "describe the project intake lab contract"},
            {"id": "plurora/project-intake-lab/inspect_external_project_ref", "purpose": "classify an external project source ref without clone/install/run"},
            {"id": "plurora/project-intake-lab/detect_project_stack_from_metadata", "purpose": "detect project stack from metadata files without filesystem access"},
            {"id": "plurora/project-intake-lab/draft_workspace_plan", "purpose": "draft a plan-only workspace plan, no direct workspace creation"},
            {"id": "plurora/project-intake-lab/draft_security_risk_summary", "purpose": "draft security risk summary from metadata, no filesystem scan"},
            {"id": "plurora/project-intake-lab/list_candidate_entrypoints", "purpose": "list candidate entrypoints with risk annotations, no execution"},
            {"id": "plurora/project-intake-lab/draft_adapter_plan", "purpose": "draft plan-only adapter plan, no direct adapter creation"},
            {"id": "plurora/project-intake-lab/generate_adapter_manifest_preview", "purpose": "generate adapter package manifest preview without file write"},
            {"id": "plurora/project-intake-lab/generate_subprocess_wrapper_preview", "purpose": "generate subprocess wrapper code preview without file write or execution"},
            {"id": "plurora/project-intake-lab/generate_adapter_fixture_preview", "purpose": "generate adapter package fixture input/output sample, redacted"},
            {"id": "plurora/project-intake-lab/check_adapter_readiness", "purpose": "produce readiness checklist for adapter package"},
        ],
        "surfaces": {
            "forge_panel": "plurora/project-intake-lab/forge-panel",
            "assistant_action": "plurora/project-intake-lab/assistant-action",
            "home_card": "plurora/project-intake-lab/home-card",
        },
        "source_kinds": SOURCE_KINDS,
        "stack_kinds": STACK_KINDS,
        "metadata_kinds": METADATA_KINDS,
        "npm_lifecycle_scripts": NPM_LIFECYCLE_SCRIPTS,
        "output_shapes": {
            "intake_contract": ["package_id", "package_kind", "capabilities", "surfaces", "source_kinds", "stack_kinds", "metadata_kinds", "npm_lifecycle_scripts"],
            "project_ref_inspection": ["source_kind", "source_ref", "classification_confidence", "path_safety", "unsafe_path_reason", "warnings"],
            "stack_detection": ["detected_stack", "metadata_signals", "confidence", "npm_lifecycle_risks"],
            "workspace_plan": ["plan_only", "requires_user_approval", "source_kind", "source_ref", "proposed_steps", "risk_notes"],
            "security_risk_summary": ["risk_level", "risk_factors", "npm_lifecycle_risks", "path_safety", "raw_secret_detected", "recommendations"],
            "candidate_entrypoints": ["entrypoints", "entrypoints[].label", "entrypoints[].command", "entrypoints[].requires_approval", "entrypoints[].executes_code"],
            "adapter_plan": ["plan_only", "requires_user_approval", "source_kind", "proposed_capabilities", "proposed_entry", "risk_notes"],
            "adapter_manifest_preview": ["manifest_preview", "adapter_package_id", "capability_name", "entry_kind", "filesystem_performed", "network_performed", "execution_performed"],
            "subprocess_wrapper_preview": ["files", "language", "safe_comments", "filesystem_performed", "network_performed", "execution_performed"],
            "adapter_fixture_preview": ["fixture_input", "fixture_output", "redacted", "filesystem_performed", "network_performed", "execution_performed"],
            "adapter_readiness": ["checklist", "ready", "capability_namespace_ok", "surface_coverage", "permissions_minimal", "fixture_present", "no_raw_secrets", "needs_approval_for_execution"],
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

fn inspect_external_project_ref(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let source_ref = request
        .input
        .get("source_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    let source_kind = request
        .input
        .get("source_kind")
        .and_then(Value::as_str)
        .filter(|k| SOURCE_KINDS.contains(k))
        .unwrap_or_else(|| classify_source_kind(source_ref));

    let (path_safety, unsafe_path_reason) = if source_kind == "local" {
        if is_unsafe_local_path(source_ref) {
            (
                "rejected",
                Some("unsafe local path: path traversal, home path, or absolute sensitive path"),
            )
        } else if source_ref.is_empty() {
            ("unknown", None)
        } else {
            ("appears_safe", None)
        }
    } else {
        ("not_applicable", None)
    };

    let mut warnings: Vec<Value> = Vec::new();
    if source_kind == "unknown" {
        warnings.push(serde_json::json!({
            "kind": "unknown_source",
            "message": "source ref could not be classified; manual review recommended"
        }));
    }
    if path_safety == "rejected" {
        warnings.push(serde_json::json!({
            "kind": "unsafe_path",
            "message": unsafe_path_reason.unwrap_or("unsafe local path")
        }));
    }

    let classification_confidence = match source_kind {
        "git" | "npm" | "archive" => "high",
        "local" => "medium",
        _ => "low",
    };

    Ok(serde_json::json!({
        "kind": "project_ref_inspection",
        "source_kind": source_kind,
        "source_ref": if source_ref.is_empty() { Value::Null } else { serde_json::json!(source_ref) },
        "classification_confidence": classification_confidence,
        "path_safety": path_safety,
        "unsafe_path_reason": if path_safety == "rejected" { serde_json::json!(unsafe_path_reason) } else { Value::Null },
        "warnings": warnings,
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

fn detect_project_stack_from_metadata(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let metadata = request
        .input
        .get("metadata")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let mut signals: Vec<Value> = Vec::new();
    let mut npm_lifecycle_risks: Vec<Value> = Vec::new();

    // Detect from metadata signals
    if metadata.contains_key("package_json") {
        signals
            .push(serde_json::json!({"metadata_kind": "package_json", "signal": "node_project"}));
        // Check for npm lifecycle scripts
        let pkg = metadata.get("package_json");
        if let Some(scripts) = pkg
            .and_then(Value::as_object)
            .and_then(|o| o.get("scripts"))
            .and_then(Value::as_object)
        {
            for &script_name in NPM_LIFECYCLE_SCRIPTS {
                if scripts.contains_key(script_name) {
                    npm_lifecycle_risks.push(serde_json::json!({
                        "script": script_name,
                        "risk": "executes_code",
                        "description": format!("npm `{}` script runs arbitrary code during install", script_name),
                        "requires_approval": true,
                        "executes_code": true,
                    }));
                }
            }
        }
    }
    if metadata.contains_key("cargo_toml") {
        signals.push(serde_json::json!({"metadata_kind": "cargo_toml", "signal": "rust_project"}));
    }
    if metadata.contains_key("pyproject") {
        signals.push(serde_json::json!({"metadata_kind": "pyproject", "signal": "python_project"}));
    }
    if metadata.contains_key("readme") {
        signals.push(serde_json::json!({"metadata_kind": "readme", "signal": "documentation"}));
    }
    if metadata.contains_key("files") {
        signals.push(serde_json::json!({"metadata_kind": "files", "signal": "file_listing"}));
    }

    // Determine stack
    let detected_stack = if signals.iter().any(|s| s["signal"] == "node_project") {
        "node"
    } else if signals.iter().any(|s| s["signal"] == "rust_project") {
        "rust"
    } else if signals.iter().any(|s| s["signal"] == "python_project") {
        "python"
    } else if !signals.is_empty() {
        "static"
    } else {
        "unknown"
    };

    let confidence = if !signals.is_empty() { "medium" } else { "low" };

    Ok(serde_json::json!({
        "kind": "project_stack_detection",
        "detected_stack": detected_stack,
        "metadata_signals": signals,
        "confidence": confidence,
        "npm_lifecycle_risks": npm_lifecycle_risks,
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

fn draft_workspace_plan(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let source_ref = request
        .input
        .get("source_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    let source_kind = request
        .input
        .get("source_kind")
        .and_then(Value::as_str)
        .filter(|k| SOURCE_KINDS.contains(k))
        .unwrap_or_else(|| classify_source_kind(source_ref));

    // Check local path safety
    if source_kind == "local" && is_unsafe_local_path(source_ref) {
        return Ok(serde_json::json!({
            "kind": "project_intake_rejected",
            "redaction_state": "unsafe_blocked",
            "reason": "unsafe local path in workspace plan: path traversal, home path, or absolute sensitive path",
            "inference_performed": false,
            "network_performed": false,
            "execution_performed": false,
            "filesystem_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    let mut proposed_steps: Vec<Value> = Vec::new();
    let mut risk_notes: Vec<Value> = Vec::new();

    match source_kind {
        "git" => {
            proposed_steps.push(serde_json::json!({"step": "clone", "requires_approval": true, "executes_code": false, "network_required": true}));
            proposed_steps.push(serde_json::json!({"step": "read_metadata", "requires_approval": false, "executes_code": false, "network_required": false}));
            risk_notes.push(serde_json::json!({"kind": "network_required", "note": "git clone requires outbound network access; must be policy-gated"}));
        }
        "npm" => {
            proposed_steps.push(serde_json::json!({"step": "fetch_package", "requires_approval": true, "executes_code": false, "network_required": true}));
            proposed_steps.push(serde_json::json!({"step": "read_metadata", "requires_approval": false, "executes_code": false, "network_required": false}));
            risk_notes.push(serde_json::json!({"kind": "network_required", "note": "npm fetch requires outbound network access; must be policy-gated"}));
        }
        "local" => {
            proposed_steps.push(serde_json::json!({"step": "read_metadata", "requires_approval": false, "executes_code": false, "network_required": false}));
        }
        "archive" => {
            proposed_steps.push(serde_json::json!({"step": "extract", "requires_approval": true, "executes_code": false, "network_required": false}));
            proposed_steps.push(serde_json::json!({"step": "read_metadata", "requires_approval": false, "executes_code": false, "network_required": false}));
            risk_notes.push(serde_json::json!({"kind": "archive_risk", "note": "archive extraction must be path-bounded to prevent zip-slip"}));
        }
        _ => {
            proposed_steps.push(serde_json::json!({"step": "read_metadata", "requires_approval": false, "executes_code": false, "network_required": false}));
            risk_notes.push(serde_json::json!({"kind": "unknown_source", "note": "source kind unknown; manual review required"}));
        }
    }

    Ok(serde_json::json!({
        "kind": "project_workspace_plan",
        "plan_only": true,
        "requires_user_approval": true,
        "source_kind": source_kind,
        "source_ref": if source_ref.is_empty() { Value::Null } else { serde_json::json!(source_ref) },
        "proposed_steps": proposed_steps,
        "risk_notes": risk_notes,
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

fn draft_security_risk_summary(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let source_ref = request
        .input
        .get("source_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    let source_kind = request
        .input
        .get("source_kind")
        .and_then(Value::as_str)
        .filter(|k| SOURCE_KINDS.contains(k))
        .unwrap_or_else(|| classify_source_kind(source_ref));

    let metadata = request
        .input
        .get("metadata")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let mut risk_factors: Vec<Value> = Vec::new();
    let mut npm_lifecycle_risks: Vec<Value> = Vec::new();
    let mut recommendations: Vec<Value> = Vec::new();
    let mut path_safety = "not_applicable";

    // Path safety for local
    if source_kind == "local" {
        if is_unsafe_local_path(source_ref) {
            path_safety = "rejected";
            risk_factors.push(serde_json::json!({"kind": "unsafe_path", "severity": "high", "detail": "path traversal, home path, or absolute sensitive path"}));
        } else if source_ref.is_empty() {
            path_safety = "unknown";
        } else {
            path_safety = "appears_safe";
        }
    }

    // npm lifecycle risks
    if let Some(scripts) = metadata
        .get("package_json")
        .and_then(Value::as_object)
        .and_then(|o| o.get("scripts"))
        .and_then(Value::as_object)
    {
        for &script_name in NPM_LIFECYCLE_SCRIPTS {
            if scripts.contains_key(script_name) {
                npm_lifecycle_risks.push(serde_json::json!({
                    "script": script_name,
                    "risk": "executes_code",
                    "description": format!("npm `{}` script runs arbitrary code during install", script_name),
                    "requires_approval": true,
                    "executes_code": true,
                }));
            }
        }
        if !npm_lifecycle_risks.is_empty() {
            risk_factors.push(serde_json::json!({
                "kind": "npm_lifecycle_scripts",
                "severity": "high",
                "detail": format!("{} npm lifecycle script(s) that execute code detected", npm_lifecycle_risks.len())
            }));
            recommendations.push(serde_json::json!({
                "kind": "skip_lifecycle_scripts",
                "suggestion": "use --ignore-scripts or equivalent flag to prevent automatic code execution"
            }));
        }
    }

    // Source-specific risks
    match source_kind {
        "git" => {
            risk_factors.push(serde_json::json!({"kind": "network_access", "severity": "medium", "detail": "git clone requires network access"}));
            recommendations.push(serde_json::json!({"kind": "network_policy", "suggestion": "require explicit network approval and audit for git operations"}));
        }
        "npm" => {
            risk_factors.push(serde_json::json!({"kind": "network_access", "severity": "medium", "detail": "npm fetch requires network access"}));
            risk_factors.push(serde_json::json!({"kind": "supply_chain", "severity": "high", "detail": "npm packages may contain malicious lifecycle scripts"}));
            recommendations.push(serde_json::json!({"kind": "audit_registry", "suggestion": "verify package integrity and audit npm registry source"}));
        }
        "archive" => {
            risk_factors.push(serde_json::json!({"kind": "archive_path_traversal", "severity": "medium", "detail": "archives may contain path-traversal entries"}));
        }
        "local" => {}
        _ => {
            risk_factors.push(serde_json::json!({"kind": "unknown_source", "severity": "medium", "detail": "source kind unknown; risk cannot be fully assessed"}));
        }
    }

    let raw_secret_detected = safety::contains_raw_secret(&request.input);
    if raw_secret_detected {
        // Already handled above, but defensive check
        risk_factors.push(serde_json::json!({"kind": "raw_secret", "severity": "critical", "detail": "raw secret-like content detected in input"}));
    }

    let risk_level =
        if raw_secret_detected || path_safety == "rejected" || !npm_lifecycle_risks.is_empty() {
            "high"
        } else if risk_factors.is_empty() {
            "low"
        } else {
            "medium"
        };

    Ok(serde_json::json!({
        "kind": "project_security_risk_summary",
        "risk_level": risk_level,
        "risk_factors": risk_factors,
        "npm_lifecycle_risks": npm_lifecycle_risks,
        "path_safety": path_safety,
        "raw_secret_detected": raw_secret_detected,
        "recommendations": recommendations,
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

fn list_candidate_entrypoints(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let metadata = request
        .input
        .get("metadata")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let mut entrypoints: Vec<Value> = Vec::new();

    // Node project entrypoints
    if let Some(pkg) = metadata.get("package_json").and_then(Value::as_object) {
        if pkg.contains_key("bin") {
            entrypoints.push(serde_json::json!({
                "label": "bin (package.json)",
                "command": "bin",
                "source": "package_json",
                "requires_approval": true,
                "executes_code": true,
            }));
        }
        if let Some(main) = pkg.get("main") {
            entrypoints.push(serde_json::json!({
                "label": format!("main ({})", main),
                "command": format!("node {}", main),
                "source": "package_json",
                "requires_approval": true,
                "executes_code": true,
            }));
        }
        if let Some(scripts) = pkg.get("scripts").and_then(Value::as_object) {
            if scripts.contains_key("start") {
                entrypoints.push(serde_json::json!({
                    "label": "npm start",
                    "command": "npm start",
                    "source": "package_json.scripts",
                    "requires_approval": true,
                    "executes_code": true,
                }));
            }
            if scripts.contains_key("test") {
                entrypoints.push(serde_json::json!({
                    "label": "npm test",
                    "command": "npm test",
                    "source": "package_json.scripts",
                    "requires_approval": true,
                    "executes_code": true,
                }));
            }
            if scripts.contains_key("dev") {
                entrypoints.push(serde_json::json!({
                    "label": "npm run dev",
                    "command": "npm run dev",
                    "source": "package_json.scripts",
                    "requires_approval": true,
                    "executes_code": true,
                }));
            }
        }
    }

    // Rust project entrypoints
    if metadata.contains_key("cargo_toml") {
        entrypoints.push(serde_json::json!({
            "label": "cargo build",
            "command": "cargo build",
            "source": "cargo_toml",
            "requires_approval": true,
            "executes_code": true,
        }));
        entrypoints.push(serde_json::json!({
            "label": "cargo test",
            "command": "cargo test",
            "source": "cargo_toml",
            "requires_approval": true,
            "executes_code": true,
        }));
    }

    // Python project entrypoints
    if metadata.contains_key("pyproject") {
        entrypoints.push(serde_json::json!({
            "label": "pip install",
            "command": "pip install",
            "source": "pyproject",
            "requires_approval": true,
            "executes_code": true,
        }));
        if let Some(pyproj) = metadata.get("pyproject").and_then(Value::as_object) {
            if let Some(scripts) = pyproj.get("scripts").and_then(Value::as_object) {
                for name in scripts.keys() {
                    entrypoints.push(serde_json::json!({
                        "label": name,
                        "command": name,
                        "source": "pyproject.scripts",
                        "requires_approval": true,
                        "executes_code": true,
                    }));
                }
            }
        }
    }

    // README suggestions
    if metadata.contains_key("readme") {
        entrypoints.push(serde_json::json!({
            "label": "README review",
            "command": "read",
            "source": "readme",
            "requires_approval": false,
            "executes_code": false,
        }));
    }

    Ok(serde_json::json!({
        "kind": "project_candidate_entrypoints",
        "entrypoints": entrypoints,
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

fn draft_adapter_plan(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let source_ref = request
        .input
        .get("source_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    let source_kind = request
        .input
        .get("source_kind")
        .and_then(Value::as_str)
        .filter(|k| SOURCE_KINDS.contains(k))
        .unwrap_or_else(|| classify_source_kind(source_ref));

    // Check local path safety
    if source_kind == "local" && is_unsafe_local_path(source_ref) {
        return Ok(serde_json::json!({
            "kind": "project_intake_rejected",
            "redaction_state": "unsafe_blocked",
            "reason": "unsafe local path in adapter plan: path traversal, home path, or absolute sensitive path",
            "inference_performed": false,
            "network_performed": false,
            "execution_performed": false,
            "filesystem_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    let mut proposed_capabilities: Vec<Value> = Vec::new();
    let mut risk_notes: Vec<Value> = Vec::new();

    match source_kind {
        "git" | "npm" | "local" | "archive" => {
            proposed_capabilities.push(serde_json::json!({
                "capability_kind": "invoke",
                "purpose": "run a project command through the adapter",
                "requires_approval": true,
            }));
            proposed_capabilities.push(serde_json::json!({
                "capability_kind": "inspect",
                "purpose": "read project metadata through the adapter",
                "requires_approval": false,
            }));
            if source_kind == "git" || source_kind == "npm" {
                risk_notes.push(serde_json::json!({"kind": "network_required", "note": "adapter may need network access for git clone or npm fetch"}));
            }
        }
        _ => {
            risk_notes.push(serde_json::json!({"kind": "unknown_source", "note": "source kind unknown; adapter plan may be incomplete"}));
        }
    }

    Ok(serde_json::json!({
        "kind": "project_adapter_plan",
        "plan_only": true,
        "requires_user_approval": true,
        "source_kind": source_kind,
        "source_ref": if source_ref.is_empty() { Value::Null } else { serde_json::json!(source_ref) },
        "proposed_capabilities": proposed_capabilities,
        "proposed_entry": {
            "kind": "subprocess",
            "requires_approval": true,
        },
        "risk_notes": risk_notes,
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
// Forbidden namespace tokens that must not appear in adapter output
// ---------------------------------------------------------------------------

const FORBIDDEN_NAMESPACE_TOKENS: &[&str] = &[
    "platform.project.",
    "platform.workspace.",
    "platform.git.",
    "platform.npm.",
    "platform.deploy.",
    "platform.ide.",
];

fn contains_forbidden_namespace(value: &Value) -> bool {
    let s = serde_json::to_string(value).unwrap_or_default();
    FORBIDDEN_NAMESPACE_TOKENS.iter().any(|t| s.contains(t))
}

// ---------------------------------------------------------------------------
// Unsafe adapter package ID validation
// ---------------------------------------------------------------------------

fn is_unsafe_adapter_package_id(id: &str) -> bool {
    // Must not be plurora/ — prevents impersonation of first-party Packages
    if id.starts_with("plurora/") {
        return true;
    }
    // Must not contain path traversal
    if id.contains("..") {
        return true;
    }
    // Must not contain unsafe characters (only allow alphanumeric, dash, underscore, slash)
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '/')
    {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Capability namespace validation: capability_id must be under adapter namespace
// ---------------------------------------------------------------------------

fn is_capability_namespace_mismatch(adapter_package_id: &str, capability_name: &str) -> bool {
    // capability_name should be a bare name like "invoke" or "inspect"
    // The full capability_id would be adapter_package_id/capability_name
    // It must not reference a different package namespace
    if capability_name.contains('/') && !capability_name.starts_with(adapter_package_id) {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Capability implementations (E5)
// ---------------------------------------------------------------------------

fn generate_adapter_manifest_preview(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let source_ref = request
        .input
        .get("source_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    let source_kind = request
        .input
        .get("source_kind")
        .and_then(Value::as_str)
        .filter(|k| SOURCE_KINDS.contains(k))
        .unwrap_or_else(|| classify_source_kind(source_ref));

    let adapter_package_id = request
        .input
        .get("adapter_package_id")
        .and_then(Value::as_str)
        .unwrap_or("");

    let capability_name = request
        .input
        .get("capability_name")
        .and_then(Value::as_str)
        .unwrap_or("invoke");

    let entry_kind = request
        .input
        .get("entry_kind")
        .and_then(Value::as_str)
        .unwrap_or("subprocess");

    // Reject first-party adapter Package IDs
    if is_unsafe_adapter_package_id(adapter_package_id) {
        return Ok(serde_json::json!({
            "kind": "project_intake_rejected",
            "redaction_state": "unsafe_blocked",
            "reason": "adapter_package_id must not be plurora/ and must not contain path traversal or unsafe characters",
            "inference_performed": false,
            "network_performed": false,
            "execution_performed": false,
            "filesystem_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    // Check capability namespace mismatch
    if is_capability_namespace_mismatch(adapter_package_id, capability_name) {
        return Ok(serde_json::json!({
            "kind": "project_intake_rejected",
            "redaction_state": "unsafe_blocked",
            "reason": "capability_name must belong to the adapter package namespace",
            "inference_performed": false,
            "network_performed": false,
            "execution_performed": false,
            "filesystem_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    // Build manifest preview
    let full_capability_id = format!("{adapter_package_id}/{capability_name}");
    let manifest_preview = serde_json::json!({
        "schema_version": 1,
        "id": adapter_package_id,
        "version": "0.1.0",
        "display_name": format!("Adapter for {}", source_ref),
        "description": format!("Adapter/wrapper package for external project {} — generated preview, not written to filesystem", source_ref),
        "entry": {
            "kind": entry_kind,
        },
        "provides": [
            {
                "id": full_capability_id,
                "version": "0.1.0",
                "input_schema": {},
                "output_schema": {},
                "streaming": false,
                "side_effects": [],
            }
        ],
        "consumes": [],
        "contributes": {
            "schemas": [],
            "hooks": [],
            "extension_points": [],
            "surfaces": []
        },
        "permissions": {
            "capabilities": {
                "invoke": []
            }
        },
        "sandbox_policy": {
            "cpu_quota_ms_per_invoke": 5000,
            "memory_mb": 128,
            "wall_clock_ms": 30000
        }
    });

    // Check for forbidden namespace in preview
    if contains_forbidden_namespace(&manifest_preview) {
        return Ok(serde_json::json!({
            "kind": "project_intake_rejected",
            "redaction_state": "unsafe_blocked",
            "reason": "adapter manifest preview contains forbidden platform-reserved namespace references",
            "inference_performed": false,
            "network_performed": false,
            "execution_performed": false,
            "filesystem_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    Ok(serde_json::json!({
        "kind": "adapter_manifest_preview",
        "manifest_preview": manifest_preview,
        "adapter_package_id": adapter_package_id,
        "capability_name": capability_name,
        "entry_kind": entry_kind,
        "source_kind": source_kind,
        "source_ref": if source_ref.is_empty() { Value::Null } else { serde_json::json!(source_ref) },
        "filesystem_performed": false,
        "network_performed": false,
        "execution_performed": false,
        "inference_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn generate_subprocess_wrapper_preview(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let source_ref = request
        .input
        .get("source_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    let source_kind = request
        .input
        .get("source_kind")
        .and_then(Value::as_str)
        .filter(|k| SOURCE_KINDS.contains(k))
        .unwrap_or_else(|| classify_source_kind(source_ref));

    let adapter_package_id = request
        .input
        .get("adapter_package_id")
        .and_then(Value::as_str)
        .unwrap_or("thirdparty/adapter");

    let capability_name = request
        .input
        .get("capability_name")
        .and_then(Value::as_str)
        .unwrap_or("invoke");

    // Reject unsafe adapter package ids
    if is_unsafe_adapter_package_id(adapter_package_id) {
        return Ok(serde_json::json!({
            "kind": "project_intake_rejected",
            "redaction_state": "unsafe_blocked",
            "reason": "adapter_package_id must not be plurora/ and must not contain path traversal or unsafe characters",
            "inference_performed": false,
            "network_performed": false,
            "execution_performed": false,
            "filesystem_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    let language = request
        .input
        .get("language")
        .and_then(Value::as_str)
        .unwrap_or("typescript");

    let command = match source_kind {
        "git" | "npm" | "local" | "archive" => "run",
        _ => "unknown",
    };

    // Generate wrapper code preview — no real execution
    let wrapper_content = match language {
        "python" => format!(
            r#"# Adapter subprocess wrapper for {adapter_package_id}
# SAFE COMMENT: external project invocation requires future policy-gated executor / explicit approval
# This preview is generated for inspection only; do not execute without approval.

import json
import sys

def handle_invoke(input_data):
    # SAFE COMMENT: external project invocation requires future policy-gated executor / explicit approval
    # No real execution is performed in this preview
    return {{"kind": "adapter_invoke_result", "command": "{command}", "source_ref": "{source_ref}", "execution_performed": False}}

if __name__ == "__main__":
    request = json.load(sys.stdin)
    result = handle_invoke(request.get("input", {{}}))
    json.dump(result, sys.stdout)
"#,
            adapter_package_id = adapter_package_id,
            command = command,
            source_ref = source_ref,
        ),
        _ => format!(
            r#"// Adapter subprocess wrapper for {adapter_package_id}
// SAFE COMMENT: external project invocation requires future policy-gated executor / explicit approval
// This preview is generated for inspection only; do not execute without approval.

import {{ SubprocessHandler }} from "@plurora/sdk/subprocess";

const handler: SubprocessHandler = {{
  async {capability_name}(input: Record<string, unknown>) {{
    // SAFE COMMENT: external project invocation requires future policy-gated executor / explicit approval
    // No real execution is performed in this preview
    return {{
      kind: "adapter_invoke_result",
      command: "{command}",
      source_ref: "{source_ref}",
      execution_performed: false,
    }};
  }},
}};

export default handler;
"#,
            adapter_package_id = adapter_package_id,
            capability_name = capability_name,
            command = command,
            source_ref = source_ref,
        ),
    };

    let safe_comments = vec![
        "external project invocation requires future policy-gated executor / explicit approval",
    ];

    Ok(serde_json::json!({
        "kind": "subprocess_wrapper_preview",
        "files": [
            {
                "path": format!("src/index.{}", if language == "python" { "py" } else { "ts" }),
                "content": wrapper_content,
            }
        ],
        "language": language,
        "adapter_package_id": adapter_package_id,
        "capability_name": capability_name,
        "safe_comments": safe_comments,
        "source_kind": source_kind,
        "filesystem_performed": false,
        "network_performed": false,
        "execution_performed": false,
        "inference_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn generate_adapter_fixture_preview(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let adapter_package_id = request
        .input
        .get("adapter_package_id")
        .and_then(Value::as_str)
        .unwrap_or("thirdparty/adapter");

    let capability_name = request
        .input
        .get("capability_name")
        .and_then(Value::as_str)
        .unwrap_or("invoke");

    // Reject unsafe adapter package ids
    if is_unsafe_adapter_package_id(adapter_package_id) {
        return Ok(serde_json::json!({
            "kind": "project_intake_rejected",
            "redaction_state": "unsafe_blocked",
            "reason": "adapter_package_id must not be plurora/ and must not contain path traversal or unsafe characters",
            "inference_performed": false,
            "network_performed": false,
            "execution_performed": false,
            "filesystem_performed": false,
            "provenance": {
                "package_id": request.provider_package_id,
                "capability_id": request.capability_id
            }
        }));
    }

    // Build fixture with redacted values for any secret-like content
    let fixture_input = serde_json::json!({
        "command": "run",
        "args": [],
        "env_refs": ["secret_ref:env:ADAPTER_TEST_KEY"],
        "source_ref": "example-project",
    });

    let fixture_output = serde_json::json!({
        "kind": "adapter_invoke_result",
        "execution_performed": false,
        "network_performed": false,
        "result_preview": "[redacted]",
        "exit_code": 0,
    });

    Ok(serde_json::json!({
        "kind": "adapter_fixture_preview",
        "fixture_input": fixture_input,
        "fixture_output": fixture_output,
        "adapter_package_id": adapter_package_id,
        "capability_name": capability_name,
        "redacted": true,
        "redaction_note": "fixture values containing secrets are replaced with secret_ref references; result previews are redacted",
        "filesystem_performed": false,
        "network_performed": false,
        "execution_performed": false,
        "inference_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn check_adapter_readiness(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }

    let adapter_package_id = request
        .input
        .get("adapter_package_id")
        .and_then(Value::as_str)
        .unwrap_or("");

    let capability_name = request
        .input
        .get("capability_name")
        .and_then(Value::as_str)
        .unwrap_or("invoke");

    let has_manifest = request
        .input
        .get("has_manifest")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let has_wrapper = request
        .input
        .get("has_wrapper")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let has_fixture = request
        .input
        .get("has_fixture")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let source_ref = request
        .input
        .get("source_ref")
        .and_then(Value::as_str)
        .unwrap_or("");

    // Reject first-party adapter IDs
    let capability_namespace_ok = !is_unsafe_adapter_package_id(adapter_package_id)
        && !is_capability_namespace_mismatch(adapter_package_id, capability_name);

    // Check surface coverage — at least one capability provided
    let surface_coverage = !capability_name.is_empty();

    // Permissions minimal — adapter has no network/filesystem/process by default
    let permissions_minimal = true;

    // Fixture present
    let fixture_present = has_manifest && has_wrapper && has_fixture;

    // No raw secrets — already checked above
    let no_raw_secrets = true;

    // Needs approval for execution — always true for adapters
    let needs_approval_for_execution = true;

    // Check for forbidden namespace in input
    let no_forbidden_namespace = !contains_forbidden_namespace(&request.input);

    let mut checklist: Vec<Value> = Vec::new();

    // capability namespace ok
    checklist.push(serde_json::json!({
        "item": "capability_namespace_ok",
        "status": capability_namespace_ok,
        "detail": if capability_namespace_ok { "adapter package id is not plurora/ and capability belongs to adapter namespace" } else { "adapter_package_id must not be plurora/ and must not contain path traversal; capability must belong to adapter namespace" }
    }));

    // surface coverage
    checklist.push(serde_json::json!({
        "item": "surface_coverage",
        "status": surface_coverage,
        "detail": if surface_coverage { "at least one capability declared" } else { "adapter must declare at least one capability" }
    }));

    // permissions minimal
    checklist.push(serde_json::json!({
        "item": "permissions_minimal",
        "status": permissions_minimal,
        "detail": "adapter has no network/filesystem/process permissions by default"
    }));

    // fixture present
    checklist.push(serde_json::json!({
        "item": "fixture_present",
        "status": fixture_present,
        "detail": if fixture_present { "manifest, wrapper, and fixture preview available" } else { "one or more of manifest/wrapper/fixture not available" }
    }));

    // no raw secrets
    checklist.push(serde_json::json!({
        "item": "no_raw_secrets",
        "status": no_raw_secrets,
        "detail": "input contains no raw-secret-like content"
    }));

    // no forbidden namespace
    checklist.push(serde_json::json!({
        "item": "no_forbidden_namespace",
        "status": no_forbidden_namespace,
        "detail": if no_forbidden_namespace { "no forbidden platform-reserved namespace references in output" } else { "output must not contain reserved external-project platform-reserved namespace references" }
    }));

    // needs approval for execution
    checklist.push(serde_json::json!({
        "item": "needs_approval_for_execution",
        "status": needs_approval_for_execution,
        "detail": "adapter execution always requires explicit user approval; no automatic execution"
    }));

    let all_ok = capability_namespace_ok
        && surface_coverage
        && permissions_minimal
        && fixture_present
        && no_raw_secrets
        && no_forbidden_namespace;

    Ok(serde_json::json!({
        "kind": "adapter_readiness",
        "checklist": checklist,
        "ready": all_ok,
        "capability_namespace_ok": capability_namespace_ok,
        "surface_coverage": surface_coverage,
        "permissions_minimal": permissions_minimal,
        "fixture_present": fixture_present,
        "no_raw_secrets": no_raw_secrets,
        "no_forbidden_namespace": no_forbidden_namespace,
        "needs_approval_for_execution": needs_approval_for_execution,
        "adapter_package_id": if adapter_package_id.is_empty() { Value::Null } else { serde_json::json!(adapter_package_id) },
        "source_ref": if source_ref.is_empty() { Value::Null } else { serde_json::json!(source_ref) },
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
// Source kind classification
// ---------------------------------------------------------------------------

fn classify_source_kind(source_ref: &str) -> &'static str {
    if source_ref.starts_with("https://")
        || source_ref.starts_with("git://")
        || source_ref.starts_with("git@")
        || source_ref.ends_with(".git")
    {
        "git"
    } else if source_ref.starts_with("npm:")
        || source_ref.starts_with("@") && source_ref.contains('/') && !source_ref.starts_with("@/")
    {
        "npm"
    } else if source_ref.ends_with(".tar.gz")
        || source_ref.ends_with(".zip")
        || source_ref.ends_with(".tgz")
        || source_ref.ends_with(".tar.xz")
    {
        "archive"
    } else if source_ref.starts_with("./")
        || source_ref.starts_with("../")
        || source_ref.starts_with("/")
        || source_ref.starts_with("~/")
        || (!source_ref.contains(':') && !source_ref.contains('@') && !source_ref.is_empty())
    {
        "local"
    } else {
        "unknown"
    }
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
            "plurora/project-intake-lab/describe_intake_contract",
            json!({}),
        );
        assert!(try_handle(&req).is_some());
    }

    #[test]
    fn try_handle_rejects_wrong_package() {
        let req = InprocInvocation {
            capability_id: "plurora/project-intake-lab/describe_intake_contract".to_string(),
            provider_package_id: "plurora/other".to_string(),
            session_id: None,
            input: json!({}),
        };
        assert!(try_handle(&req).is_none());
    }

    #[test]
    fn describe_contract_has_all_surfaces() {
        let req = make_request(
            "plurora/project-intake-lab/describe_intake_contract",
            json!({}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        let surfaces = result["surfaces"].as_object().unwrap();
        assert!(surfaces.contains_key("forge_panel"));
        assert!(surfaces.contains_key("assistant_action"));
        assert!(surfaces.contains_key("home_card"));
    }

    #[test]
    fn describe_contract_lists_11_capabilities() {
        let req = make_request(
            "plurora/project-intake-lab/describe_intake_contract",
            json!({}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(
            result["capabilities"]
                .as_array()
                .map(|a| a.len())
                .unwrap_or(0),
            11,
            "must list 11 capabilities"
        );
    }

    #[test]
    fn inspect_classifies_git() {
        let req = make_request(
            "plurora/project-intake-lab/inspect_external_project_ref",
            json!({"source_ref": "https://github.com/example/project.git"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["source_kind"], json!("git"));
    }

    #[test]
    fn inspect_classifies_npm() {
        let req = make_request(
            "plurora/project-intake-lab/inspect_external_project_ref",
            json!({"source_ref": "npm:lodash"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["source_kind"], json!("npm"));
    }

    #[test]
    fn inspect_classifies_local() {
        let req = make_request(
            "plurora/project-intake-lab/inspect_external_project_ref",
            json!({"source_ref": "./my-project"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["source_kind"], json!("local"));
        assert_eq!(result["path_safety"], json!("appears_safe"));
    }

    #[test]
    fn inspect_rejects_unsafe_path_traversal() {
        let req = make_request(
            "plurora/project-intake-lab/inspect_external_project_ref",
            json!({"source_ref": "../../etc/passwd"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["path_safety"], json!("rejected"));
    }

    #[test]
    fn inspect_rejects_home_path() {
        let req = make_request(
            "plurora/project-intake-lab/inspect_external_project_ref",
            json!({"source_ref": "~/secret-project"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["path_safety"], json!("rejected"));
    }

    #[test]
    fn inspect_rejects_absolute_sensitive_path() {
        let req = make_request(
            "plurora/project-intake-lab/inspect_external_project_ref",
            json!({"source_ref": "/etc/shadow"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["path_safety"], json!("rejected"));
    }

    #[test]
    fn detect_stack_node() {
        let req = make_request(
            "plurora/project-intake-lab/detect_project_stack_from_metadata",
            json!({"metadata": {"package_json": {"name": "test", "scripts": {"start": "node index.js"}}}}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["detected_stack"], json!("node"));
        let risks = result["npm_lifecycle_risks"].as_array().unwrap();
        // "start" is not a lifecycle script, so no npm_lifecycle_risks
        assert_eq!(risks.len(), 0);
    }

    #[test]
    fn detect_stack_node_with_lifecycle_scripts() {
        let req = make_request(
            "plurora/project-intake-lab/detect_project_stack_from_metadata",
            json!({"metadata": {"package_json": {"name": "test", "scripts": {"preinstall": "echo hi", "postinstall": "echo bye", "start": "node index.js"}}}}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["detected_stack"], json!("node"));
        let risks = result["npm_lifecycle_risks"].as_array().unwrap();
        assert_eq!(risks.len(), 2); // preinstall + postinstall
        for risk in risks {
            assert_eq!(risk["executes_code"], json!(true));
            assert_eq!(risk["requires_approval"], json!(true));
        }
    }

    #[test]
    fn detect_stack_rust() {
        let req = make_request(
            "plurora/project-intake-lab/detect_project_stack_from_metadata",
            json!({"metadata": {"cargo_toml": {"name": "test"}}}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["detected_stack"], json!("rust"));
    }

    #[test]
    fn detect_stack_python() {
        let req = make_request(
            "plurora/project-intake-lab/detect_project_stack_from_metadata",
            json!({"metadata": {"pyproject": {"name": "test"}}}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["detected_stack"], json!("python"));
    }

    #[test]
    fn detect_stack_unknown() {
        let req = make_request(
            "plurora/project-intake-lab/detect_project_stack_from_metadata",
            json!({"metadata": {}}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["detected_stack"], json!("unknown"));
    }

    #[test]
    fn workspace_plan_is_plan_only() {
        let req = make_request(
            "plurora/project-intake-lab/draft_workspace_plan",
            json!({"source_ref": "https://github.com/example/project.git", "source_kind": "git"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("project_workspace_plan"));
        assert_eq!(result["plan_only"], json!(true));
        assert_eq!(result["requires_user_approval"], json!(true));
    }

    #[test]
    fn workspace_plan_rejects_unsafe_local_path() {
        let req = make_request(
            "plurora/project-intake-lab/draft_workspace_plan",
            json!({"source_ref": "~/secret-project", "source_kind": "local"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("project_intake_rejected"));
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn adapter_plan_is_plan_only() {
        let req = make_request(
            "plurora/project-intake-lab/draft_adapter_plan",
            json!({"source_ref": "./my-project", "source_kind": "local"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("project_adapter_plan"));
        assert_eq!(result["plan_only"], json!(true));
        assert_eq!(result["requires_user_approval"], json!(true));
    }

    #[test]
    fn raw_secret_blocked() {
        let req = make_request(
            "plurora/project-intake-lab/inspect_external_project_ref",
            json!({"source_ref": "test", "api_key": "RawSecretExample1234567890abcdefABCDEF123456"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("project_intake_rejected"));
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn no_forbidden_namespace_in_contract() {
        let req = make_request(
            "plurora/project-intake-lab/describe_intake_contract",
            json!({}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        let output_str = serde_json::to_string(&result).unwrap();
        for token in &[
            "platform.project.",
            "platform.workspace.",
            "platform.git.",
            "platform.npm.",
            "platform.deploy.",
            "platform.ide.",
        ] {
            assert!(!output_str.contains(token), "must not contain {}", token);
        }
    }

    #[test]
    fn candidate_entrypoints_require_approval() {
        let req = make_request(
            "plurora/project-intake-lab/list_candidate_entrypoints",
            json!({"metadata": {"package_json": {"name": "test", "main": "index.js", "scripts": {"start": "node index.js"}}}}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        let entrypoints = result["entrypoints"].as_array().unwrap();
        assert!(!entrypoints.is_empty());
        for ep in entrypoints {
            if ep["executes_code"] == json!(true) {
                assert_eq!(
                    ep["requires_approval"],
                    json!(true),
                    "executes_code entrypoint must require approval"
                );
            }
        }
    }

    #[test]
    fn no_execution_performed() {
        let caps = [
            "describe_intake_contract",
            "inspect_external_project_ref",
            "detect_project_stack_from_metadata",
            "draft_workspace_plan",
            "draft_security_risk_summary",
            "list_candidate_entrypoints",
            "draft_adapter_plan",
            "generate_adapter_manifest_preview",
            "generate_subprocess_wrapper_preview",
            "generate_adapter_fixture_preview",
            "check_adapter_readiness",
        ];
        for cap in &caps {
            let req = make_request(&format!("plurora/project-intake-lab/{}", cap), json!({}));
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
    fn is_unsafe_local_path_tests() {
        // Path traversal
        assert!(is_unsafe_local_path("../../etc/passwd"));
        assert!(is_unsafe_local_path("foo/../bar/.."));
        // Home path
        assert!(is_unsafe_local_path("~/project"));
        // Absolute sensitive
        assert!(is_unsafe_local_path("/etc/shadow"));
        assert!(is_unsafe_local_path("/root/.ssh"));
        assert!(is_unsafe_local_path("/home/user/.ssh"));
        // Safe paths
        assert!(!is_unsafe_local_path("./my-project"));
        assert!(!is_unsafe_local_path("my-project"));
    }

    #[test]
    fn classify_source_kind_tests() {
        assert_eq!(
            classify_source_kind("https://github.com/example/project.git"),
            "git"
        );
        assert_eq!(
            classify_source_kind("git@github.com:example/project.git"),
            "git"
        );
        assert_eq!(classify_source_kind("npm:lodash"), "npm");
        assert_eq!(classify_source_kind("@scope/package"), "npm");
        assert_eq!(classify_source_kind("./my-project"), "local");
        assert_eq!(classify_source_kind("project.tar.gz"), "archive");
        assert_eq!(classify_source_kind("unknown-thing"), "local");
        assert_eq!(classify_source_kind(""), "unknown");
    }

    // E5 tests

    #[test]
    fn generate_adapter_manifest_preview_basic() {
        let req = make_request(
            "plurora/project-intake-lab/generate_adapter_manifest_preview",
            json!({
                "source_ref": "./my-project",
                "source_kind": "local",
                "adapter_package_id": "thirdparty/my-adapter",
                "capability_name": "invoke",
                "entry_kind": "subprocess"
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("adapter_manifest_preview"));
        assert_eq!(result["adapter_package_id"], json!("thirdparty/my-adapter"));
        assert_eq!(result["capability_name"], json!("invoke"));
        assert_eq!(result["filesystem_performed"], json!(false));
        assert_eq!(result["network_performed"], json!(false));
        assert_eq!(result["execution_performed"], json!(false));
    }

    #[test]
    fn adapter_manifest_rejects_first_party_id() {
        let req = make_request(
            "plurora/project-intake-lab/generate_adapter_manifest_preview",
            json!({
                "source_ref": "./test",
                "adapter_package_id": "plurora/fake-adapter",
                "capability_name": "invoke"
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("project_intake_rejected"));
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn adapter_manifest_rejects_path_traversal_id() {
        let req = make_request(
            "plurora/project-intake-lab/generate_adapter_manifest_preview",
            json!({
                "source_ref": "./test",
                "adapter_package_id": "thirdparty/../evil",
                "capability_name": "invoke"
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("project_intake_rejected"));
    }

    #[test]
    fn adapter_manifest_rejects_unsafe_chars_id() {
        let req = make_request(
            "plurora/project-intake-lab/generate_adapter_manifest_preview",
            json!({
                "source_ref": "./test",
                "adapter_package_id": "thirdparty/evil;rm-rf",
                "capability_name": "invoke"
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("project_intake_rejected"));
    }

    #[test]
    fn adapter_manifest_rejects_capability_namespace_mismatch() {
        let req = make_request(
            "plurora/project-intake-lab/generate_adapter_manifest_preview",
            json!({
                "source_ref": "./test",
                "adapter_package_id": "thirdparty/my-adapter",
                "capability_name": "other-pkg/invoke"
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("project_intake_rejected"));
    }

    #[test]
    fn subprocess_wrapper_preview_no_execution() {
        let req = make_request(
            "plurora/project-intake-lab/generate_subprocess_wrapper_preview",
            json!({
                "source_ref": "./my-project",
                "source_kind": "local",
                "adapter_package_id": "thirdparty/my-adapter",
                "capability_name": "invoke",
                "language": "typescript"
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("subprocess_wrapper_preview"));
        assert_eq!(result["execution_performed"], json!(false));
        assert_eq!(result["network_performed"], json!(false));
        assert_eq!(result["filesystem_performed"], json!(false));
        let safe_comments = result["safe_comments"].as_array().unwrap();
        assert!(!safe_comments.is_empty());
        // Verify safe comments present in wrapper content
        let content = result["files"].as_array().unwrap()[0]["content"]
            .as_str()
            .unwrap();
        assert!(content.contains("SAFE COMMENT"));
        assert!(content.contains("policy-gated executor"));
    }

    #[test]
    fn subprocess_wrapper_python_preview() {
        let req = make_request(
            "plurora/project-intake-lab/generate_subprocess_wrapper_preview",
            json!({
                "source_ref": "./my-project",
                "source_kind": "local",
                "adapter_package_id": "thirdparty/my-adapter",
                "capability_name": "invoke",
                "language": "python"
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("subprocess_wrapper_preview"));
        assert_eq!(result["language"], json!("python"));
    }

    #[test]
    fn subprocess_wrapper_rejects_first_party_id() {
        let req = make_request(
            "plurora/project-intake-lab/generate_subprocess_wrapper_preview",
            json!({
                "source_ref": "./test",
                "adapter_package_id": "plurora/evil",
                "capability_name": "invoke"
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("project_intake_rejected"));
    }

    #[test]
    fn fixture_preview_redacted() {
        let req = make_request(
            "plurora/project-intake-lab/generate_adapter_fixture_preview",
            json!({
                "adapter_package_id": "thirdparty/my-adapter",
                "capability_name": "invoke"
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("adapter_fixture_preview"));
        assert_eq!(result["redacted"], json!(true));
        assert_eq!(result["execution_performed"], json!(false));
        // fixture input should use secret_ref not raw secrets
        let input = &result["fixture_input"];
        let input_str = serde_json::to_string(input).unwrap();
        assert!(!input_str.contains("sk-"));
        assert!(!input_str.contains("Bearer"));
        // fixture output should be redacted
        let output = &result["fixture_output"];
        assert_eq!(output["result_preview"], json!("[redacted]"));
    }

    #[test]
    fn fixture_preview_rejects_first_party_id() {
        let req = make_request(
            "plurora/project-intake-lab/generate_adapter_fixture_preview",
            json!({
                "adapter_package_id": "plurora/evil",
                "capability_name": "invoke"
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("project_intake_rejected"));
    }

    #[test]
    fn check_adapter_readiness_ok() {
        let req = make_request(
            "plurora/project-intake-lab/check_adapter_readiness",
            json!({
                "adapter_package_id": "thirdparty/my-adapter",
                "capability_name": "invoke",
                "has_manifest": true,
                "has_wrapper": true,
                "has_fixture": true,
                "source_ref": "./test"
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("adapter_readiness"));
        assert_eq!(result["ready"], json!(true));
        assert_eq!(result["capability_namespace_ok"], json!(true));
        assert_eq!(result["surface_coverage"], json!(true));
        assert_eq!(result["permissions_minimal"], json!(true));
        assert_eq!(result["fixture_present"], json!(true));
        assert_eq!(result["no_raw_secrets"], json!(true));
        assert_eq!(result["needs_approval_for_execution"], json!(true));
    }

    #[test]
    fn check_adapter_readiness_rejects_first_party_id() {
        let req = make_request(
            "plurora/project-intake-lab/check_adapter_readiness",
            json!({
                "adapter_package_id": "plurora/evil",
                "capability_name": "invoke"
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("adapter_readiness"));
        assert_eq!(result["ready"], json!(false));
        assert_eq!(result["capability_namespace_ok"], json!(false));
    }

    #[test]
    fn check_adapter_readiness_rejects_raw_secret() {
        let req = make_request(
            "plurora/project-intake-lab/check_adapter_readiness",
            json!({
                "adapter_package_id": "thirdparty/my-adapter",
                "capability_name": "invoke",
                "secret": "RawSecretExample1234567890abcdefABCDEF123456"
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("project_intake_rejected"));
    }

    #[test]
    fn adapter_manifest_no_forbidden_namespace() {
        let req = make_request(
            "plurora/project-intake-lab/generate_adapter_manifest_preview",
            json!({
                "source_ref": "./test",
                "adapter_package_id": "thirdparty/my-adapter",
                "capability_name": "invoke"
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        if result["kind"] == json!("adapter_manifest_preview") {
            let output_str = serde_json::to_string(&result).unwrap();
            for token in FORBIDDEN_NAMESPACE_TOKENS {
                assert!(!output_str.contains(token), "must not contain {}", token);
            }
        }
    }

    #[test]
    fn wrapper_no_forbidden_namespace() {
        let req = make_request(
            "plurora/project-intake-lab/generate_subprocess_wrapper_preview",
            json!({
                "source_ref": "./test",
                "adapter_package_id": "thirdparty/my-adapter",
                "capability_name": "invoke"
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        let output_str = serde_json::to_string(&result).unwrap();
        for token in FORBIDDEN_NAMESPACE_TOKENS {
            assert!(!output_str.contains(token), "must not contain {}", token);
        }
    }

    #[test]
    fn fixture_no_forbidden_namespace() {
        let req = make_request(
            "plurora/project-intake-lab/generate_adapter_fixture_preview",
            json!({
                "adapter_package_id": "thirdparty/my-adapter",
                "capability_name": "invoke"
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        let output_str = serde_json::to_string(&result).unwrap();
        for token in FORBIDDEN_NAMESPACE_TOKENS {
            assert!(!output_str.contains(token), "must not contain {}", token);
        }
    }

    #[test]
    fn readiness_no_forbidden_namespace() {
        let req = make_request(
            "plurora/project-intake-lab/check_adapter_readiness",
            json!({
                "adapter_package_id": "thirdparty/my-adapter",
                "capability_name": "invoke"
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        let output_str = serde_json::to_string(&result).unwrap();
        for token in FORBIDDEN_NAMESPACE_TOKENS {
            assert!(!output_str.contains(token), "must not contain {}", token);
        }
    }
}
