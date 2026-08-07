//! Handler for `official/sharing-lab` capabilities.
//!
//! Experience Beta 6 — Sharing / Distribution Alpha.
//!
//! Package-owned sharing and distribution: composition bundle export/import,
//! branch/session bundle manifest, package-set lockfile, compatibility/migration
//! report, AI disclosure metadata bundle, read-only shared session manifest,
//! async fork sharing plan.
//!
//! Deterministic, no-network, no marketplace, no signing network, no billing.
//! All outputs are local/file-level proofs.
//!
//! No `kernel.v1.sharing.*`, `kernel.v1.marketplace.*`, `kernel.v1.billing.*`,
//! `kernel.v1.distribution.*` namespace references.
//!
//! Red lines:
//! - No marketplace, package signing network, dependency resolver economy,
//!   hosted billing.
//! - No `kernel.v1.sharing.*` / `kernel.v1.marketplace.*`.
//! - No raw secrets; `secret_ref` is reference-only, never resolved.
//! - No public network or remote service required.

use serde_json::Value;

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/sharing-lab";

// ---------------------------------------------------------------------------
// Bundle format versions
// ---------------------------------------------------------------------------

const BUNDLE_FORMAT_VERSION: &str = "1";

// ---------------------------------------------------------------------------
// Sharing contract kinds
// ---------------------------------------------------------------------------

const SHARING_CONTRACT_KINDS: &[&str] = &[
    "composition_bundle",
    "branch_session_bundle",
    "package_set_lockfile",
    "compatibility_report",
    "ai_disclosure_bundle",
    "read_only_share_manifest",
    "async_fork_share_plan",
];

// ---------------------------------------------------------------------------
// Compatibility status kinds
// ---------------------------------------------------------------------------

const COMPAT_STATUS_KINDS: &[&str] = &[
    "compatible",
    "minor_incompatibility",
    "major_incompatibility",
    "migration_required",
    "unsupported",
];

// ---------------------------------------------------------------------------
// AI disclosure kinds
// ---------------------------------------------------------------------------

const AI_DISCLOSURE_KINDS: &[&str] = &[
    "ai_generated",
    "ai_assisted",
    "human_created",
    "ai Reviewed",
    "mixed",
    "undisclosed",
];

// ---------------------------------------------------------------------------
// Async fork share status
// ---------------------------------------------------------------------------

const ASYNC_FORK_STATUSES: &[&str] = &[
    "draft",
    "pending_acceptance",
    "accepted",
    "rejected",
    "expired",
    "cancelled",
];

// ---------------------------------------------------------------------------
// Raw-secret detection (delegated to shared safety module)
// ---------------------------------------------------------------------------

use super::safety;

/// Check for forbidden marketplace/billing fields that must not appear.
fn contains_forbidden_marketplace_fields(value: &Value) -> bool {
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                let key_lower = key.to_lowercase();
                // Forbidden fields: marketplace, billing, signing network
                if key_lower.contains("marketplace")
                    || key_lower.contains("billing")
                    || key_lower.contains("signing_network")
                    || key_lower.contains("payment")
                    || key_lower.contains("subscription")
                    || key_lower.contains("license_key")
                {
                    return true;
                }
                if contains_forbidden_marketplace_fields(val) {
                    return true;
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                if contains_forbidden_marketplace_fields(item) {
                    return true;
                }
            }
        }
        _ => {}
    }
    false
}

fn rejected_output(request: &InprocInvocation, reason: &str) -> Value {
    serde_json::json!({
        "kind": "sharing_lab_rejected",
        "redaction_state": "unsafe_blocked",
        "reason": reason,
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
    if id.ends_with("/describe_sharing_contract") {
        Some(describe_sharing_contract(request))
    } else if id.ends_with("/export_composition_bundle") {
        Some(export_composition_bundle(request))
    } else if id.ends_with("/import_composition_bundle") {
        Some(import_composition_bundle(request))
    } else if id.ends_with("/create_branch_session_bundle") {
        Some(create_branch_session_bundle(request))
    } else if id.ends_with("/create_package_set_lockfile") {
        Some(create_package_set_lockfile(request))
    } else if id.ends_with("/compatibility_report") {
        Some(compatibility_report(request))
    } else if id.ends_with("/ai_disclosure_bundle") {
        Some(ai_disclosure_bundle(request))
    } else if id.ends_with("/read_only_share_manifest") {
        Some(read_only_share_manifest(request))
    } else if id.ends_with("/async_fork_share_plan") {
        Some(async_fork_share_plan(request))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Capability implementations
// ---------------------------------------------------------------------------

fn describe_sharing_contract(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "sharing_lab_contract",
        "package_id": request.provider_package_id,
        "package_kind": "ordinary",
        "capabilities": [
            {"id": "official/sharing-lab/describe_sharing_contract", "purpose": "describe the sharing lab package contract"},
            {"id": "official/sharing-lab/export_composition_bundle", "purpose": "export a composition as a self-contained bundle with manifest, lockfile, and disclosure metadata"},
            {"id": "official/sharing-lab/import_composition_bundle", "purpose": "import a composition bundle, validating shape, compatibility, and no-raw-secrets constraints"},
            {"id": "official/sharing-lab/create_branch_session_bundle", "purpose": "create a branch/session bundle manifest for sharing a specific session state"},
            {"id": "official/sharing-lab/create_package_set_lockfile", "purpose": "create a package-set lockfile pinning exact package versions and content addresses"},
            {"id": "official/sharing-lab/compatibility_report", "purpose": "produce a compatibility/migration report between two bundle versions or package sets"},
            {"id": "official/sharing-lab/ai_disclosure_bundle", "purpose": "produce AI disclosure metadata bundle for composition or session content"},
            {"id": "official/sharing-lab/read_only_share_manifest", "purpose": "create a read-only shared session manifest proof — local/file-level, no remote service"},
            {"id": "official/sharing-lab/async_fork_share_plan", "purpose": "create an async fork sharing plan — local proof for deferred/async session fork sharing"},
        ],
        "surfaces": {
            "forge_panel": "official/sharing-lab/forge-panel",
            "assistant_action": "official/sharing-lab/assistant-action",
            "home_card": "official/sharing-lab/home-card",
        },
        "sharing_contract_kinds": SHARING_CONTRACT_KINDS,
        "compat_status_kinds": COMPAT_STATUS_KINDS,
        "ai_disclosure_kinds": AI_DISCLOSURE_KINDS,
        "async_fork_statuses": ASYNC_FORK_STATUSES,
        "output_shapes": {
            "composition_bundle": ["bundle_id", "format_version", "composition_manifest", "package_set_lockfile", "compatibility_report", "ai_disclosure", "created_at"],
            "branch_session_bundle": ["bundle_id", "session_id", "branch_ref", "sequence", "content_address", "ai_disclosure"],
            "package_set_lockfile": ["lockfile_id", "packages", "packages[].package_id", "packages[].version", "packages[].content_address"],
            "compatibility_report": ["report_id", "source_ref", "target_ref", "status", "incompatibilities", "migration_steps"],
            "ai_disclosure_bundle": ["disclosure_id", "items", "items[].content_ref", "items[].disclosure_kind", "items[].description"],
            "read_only_share_manifest": ["manifest_id", "session_ref", "sequence", "readonly", "expires_at"],
            "async_fork_share_plan": ["plan_id", "source_session", "target_session", "status", "fork_intent"],
        },
        "red_lines": {
            "no_marketplace": true,
            "no_signing_network": true,
            "no_billing": true,
            "no_kernel_sharing": true,
            "no_kernel_marketplace": true,
            "no_raw_secrets": true,
            "no_remote_service_required": true,
        },
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn export_composition_bundle(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(
            request,
            "input contains raw-secret-like content; use secret_ref references instead",
        ));
    }
    if contains_forbidden_marketplace_fields(&request.input) {
        return Ok(rejected_output(
            request,
            "input contains forbidden marketplace/billing/signing fields",
        ));
    }

    let composition_id = request
        .input
        .get("composition_id")
        .and_then(Value::as_str)
        .unwrap_or("composition:default");

    let packages = request
        .input
        .get("packages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let composition_manifest = request
        .input
        .get("composition_manifest")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let bundle_id = format!(
        "bundle:{}:{}",
        composition_id,
        crate::runtime::content_address(&format!("{:?}", packages))
    );

    let lockfile_id = format!(
        "lockfile:{}",
        crate::runtime::content_address(&format!("{:?}", packages))
    );

    let package_entries: Vec<Value> = packages
        .iter()
        .map(|p| {
            let pid = p
                .get("package_id")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let version = p.get("version").and_then(Value::as_str).unwrap_or("0.0.0");
            serde_json::json!({
                "package_id": pid,
                "version": version,
                "content_address": crate::runtime::content_address(&format!("{}:{}", pid, version)),
            })
        })
        .collect();

    Ok(serde_json::json!({
        "kind": "composition_bundle",
        "bundle_id": bundle_id,
        "format_version": BUNDLE_FORMAT_VERSION,
        "composition_id": composition_id,
        "composition_manifest": composition_manifest,
        "package_set_lockfile": {
            "lockfile_id": lockfile_id,
            "format_version": BUNDLE_FORMAT_VERSION,
            "packages": package_entries,
            "content_address": crate::runtime::content_address(&format!("lockfile:{}", composition_id)),
        },
        "ai_disclosure": {
            "disclosure_id": format!("disclosure:{}", bundle_id),
            "items": [{
                "content_ref": composition_id,
                "disclosure_kind": "mixed",
                "description": "Composition bundle with AI-generated and human-created content"
            }],
            "content_address": crate::runtime::content_address(&format!("disclosure:{}", bundle_id)),
        },
        "no_marketplace_fields": true,
        "no_billing_fields": true,
        "no_signing_network_fields": true,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn import_composition_bundle(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(
            request,
            "bundle contains raw-secret-like content; use secret_ref references instead",
        ));
    }
    if contains_forbidden_marketplace_fields(&request.input) {
        return Ok(rejected_output(
            request,
            "bundle contains forbidden marketplace/billing/signing fields",
        ));
    }

    let bundle_id = request
        .input
        .get("bundle_id")
        .and_then(Value::as_str)
        .unwrap_or("bundle:unknown");

    let format_version = request
        .input
        .get("format_version")
        .and_then(Value::as_str)
        .unwrap_or(BUNDLE_FORMAT_VERSION);

    let composition_manifest = request
        .input
        .get("composition_manifest")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let packages = request
        .input
        .get("packages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let missing_packages: Vec<Value> = request
        .input
        .get("missing_packages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let has_missing = !missing_packages.is_empty();
    let has_incompatible_format = format_version != BUNDLE_FORMAT_VERSION;

    let (status, diagnostics): (&str, Vec<Value>) = if has_incompatible_format {
        (
            "migration_required",
            vec![serde_json::json!({
                "kind": "format_version_mismatch",
                "expected": BUNDLE_FORMAT_VERSION,
                "found": format_version,
                "action": "migrate bundle format before import"
            })],
        )
    } else if has_missing {
        (
            "minor_incompatibility",
            vec![serde_json::json!({
                "kind": "missing_packages",
                "count": missing_packages.len(),
                "action": "install missing packages or adjust composition"
            })],
        )
    } else {
        ("compatible", vec![])
    };

    Ok(serde_json::json!({
        "kind": "composition_bundle_import",
        "bundle_id": bundle_id,
        "format_version": format_version,
        "composition_manifest": composition_manifest,
        "packages": packages,
        "missing_packages": missing_packages,
        "compatibility_status": status,
        "diagnostics": diagnostics,
        "requires_user_approval": true,
        "plan_only": true,
        "no_marketplace_fields": true,
        "no_billing_fields": true,
        "no_raw_secrets": true,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn create_branch_session_bundle(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(
            request,
            "input contains raw-secret-like content; use secret_ref references instead",
        ));
    }

    let session_id = request
        .input
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("session:default");

    let branch_ref = request
        .input
        .get("branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:main");

    let sequence = request
        .input
        .get("sequence")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    let bundle_id = format!(
        "branch-bundle:{}:{}:{}",
        session_id,
        branch_ref,
        crate::runtime::content_address(&format!("{}:{}:{}", session_id, branch_ref, sequence))
    );

    Ok(serde_json::json!({
        "kind": "branch_session_bundle",
        "bundle_id": bundle_id,
        "format_version": BUNDLE_FORMAT_VERSION,
        "session_id": session_id,
        "branch_ref": branch_ref,
        "sequence": sequence,
        "content_address": crate::runtime::content_address(&format!("{}:{}:{}", session_id, branch_ref, sequence)),
        "ai_disclosure": {
            "disclosure_id": format!("disclosure:{}", bundle_id),
            "items": [{
                "content_ref": format!("{}:{}", session_id, branch_ref),
                "disclosure_kind": "mixed",
                "description": "Branch/session bundle with session state and event history"
            }],
            "content_address": crate::runtime::content_address(&format!("disclosure:{}", bundle_id)),
        },
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

fn create_package_set_lockfile(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(
            request,
            "input contains raw-secret-like content; use secret_ref references instead",
        ));
    }

    let packages = request
        .input
        .get("packages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let package_entries: Vec<Value> = packages
        .iter()
        .map(|p| {
            let pid = p
                .get("package_id")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let version = p.get("version").and_then(Value::as_str).unwrap_or("0.0.0");
            let content_address = crate::runtime::content_address(&format!("{}:{}", pid, version));
            serde_json::json!({
                "package_id": pid,
                "version": version,
                "content_address": content_address,
            })
        })
        .collect();

    let lockfile_content = format!("{:?}", package_entries);
    let lockfile_id = format!(
        "lockfile:{}",
        crate::runtime::content_address(&lockfile_content)
    );

    Ok(serde_json::json!({
        "kind": "package_set_lockfile",
        "lockfile_id": lockfile_id,
        "format_version": BUNDLE_FORMAT_VERSION,
        "packages": package_entries,
        "content_address": crate::runtime::content_address(&lockfile_content),
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn compatibility_report(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(
            request,
            "input contains raw-secret-like content; use secret_ref references instead",
        ));
    }

    let source_ref = request
        .input
        .get("source_ref")
        .and_then(Value::as_str)
        .unwrap_or("bundle:source:unknown");

    let target_ref = request
        .input
        .get("target_ref")
        .and_then(Value::as_str)
        .unwrap_or("bundle:target:unknown");

    let source_packages = request
        .input
        .get("source_packages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let target_packages = request
        .input
        .get("target_packages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    // Deterministic comparison: find packages only in source or only in target,
    // or with version mismatches
    let mut incompatibilities = Vec::new();

    let source_ids: Vec<String> = source_packages
        .iter()
        .filter_map(|p| {
            p.get("package_id")
                .and_then(Value::as_str)
                .map(String::from)
        })
        .collect();

    let target_ids: Vec<String> = target_packages
        .iter()
        .filter_map(|p| {
            p.get("package_id")
                .and_then(Value::as_str)
                .map(String::from)
        })
        .collect();

    for sid in &source_ids {
        if !target_ids.contains(sid) {
            incompatibilities.push(serde_json::json!({
                "package_id": sid,
                "kind": "missing_in_target",
                "severity": "major",
            }));
        }
    }

    for tid in &target_ids {
        if !source_ids.contains(tid) {
            incompatibilities.push(serde_json::json!({
                "package_id": tid,
                "kind": "added_in_target",
                "severity": "minor",
            }));
        }
    }

    // Version mismatches
    for sp in &source_packages {
        let sp_id = sp.get("package_id").and_then(Value::as_str).unwrap_or("");
        let sp_ver = sp.get("version").and_then(Value::as_str).unwrap_or("");
        for tp in &target_packages {
            let tp_id = tp.get("package_id").and_then(Value::as_str).unwrap_or("");
            let tp_ver = tp.get("version").and_then(Value::as_str).unwrap_or("");
            if sp_id == tp_id && sp_ver != tp_ver && !sp_id.is_empty() {
                incompatibilities.push(serde_json::json!({
                    "package_id": sp_id,
                    "kind": "version_mismatch",
                    "severity": "minor",
                    "source_version": sp_ver,
                    "target_version": tp_ver,
                }));
            }
        }
    }

    let status = if incompatibilities.iter().any(|i| i["severity"] == "major") {
        "major_incompatibility"
    } else if !incompatibilities.is_empty() {
        "minor_incompatibility"
    } else {
        "compatible"
    };

    let migration_steps: Vec<Value> = incompatibilities
        .iter()
        .filter(|i| i["severity"] == "major")
        .map(|i| {
            serde_json::json!({
                "action": "install_or_replace",
                "package_id": i["package_id"],
                "description": format!("Package {} needs to be installed or replaced in target", i["package_id"]),
            })
        })
        .collect();

    let report_id = format!(
        "compat-report:{}:{}",
        source_ref,
        crate::runtime::content_address(&format!("{:?}", incompatibilities))
    );

    Ok(serde_json::json!({
        "kind": "compatibility_report",
        "report_id": report_id,
        "source_ref": source_ref,
        "target_ref": target_ref,
        "status": status,
        "incompatibilities": incompatibilities,
        "migration_steps": migration_steps,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn ai_disclosure_bundle(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(
            request,
            "input contains raw-secret-like content; use secret_ref references instead",
        ));
    }

    let content_refs = request
        .input
        .get("content_refs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let default_kind = request
        .input
        .get("default_disclosure_kind")
        .and_then(Value::as_str)
        .filter(|k| AI_DISCLOSURE_KINDS.contains(k))
        .unwrap_or("mixed");

    let items: Vec<Value> = content_refs
        .iter()
        .map(|cr| {
            let content_ref = cr.as_str().unwrap_or("unknown");
            let kind = cr
                .get("disclosure_kind")
                .and_then(Value::as_str)
                .filter(|k| AI_DISCLOSURE_KINDS.contains(k))
                .unwrap_or(default_kind);
            let description = cr.get("description").and_then(Value::as_str).unwrap_or("");
            serde_json::json!({
                "content_ref": content_ref,
                "disclosure_kind": kind,
                "description": if description.is_empty() {
                    format!("AI disclosure for {}", content_ref)
                } else {
                    description.to_string()
                },
            })
        })
        .collect();

    let disclosure_id = format!(
        "ai-disclosure:{}",
        crate::runtime::content_address(&format!("{:?}", items))
    );

    Ok(serde_json::json!({
        "kind": "ai_disclosure_bundle",
        "disclosure_id": disclosure_id,
        "items": items,
        "content_address": crate::runtime::content_address(&format!("{:?}", items)),
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn read_only_share_manifest(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(
            request,
            "input contains raw-secret-like content; use secret_ref references instead",
        ));
    }

    let session_ref = request
        .input
        .get("session_ref")
        .and_then(Value::as_str)
        .unwrap_or("session:default");

    let sequence = request
        .input
        .get("sequence")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    let branch_ref = request
        .input
        .get("branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:main");

    let manifest_id = format!(
        "readonly-share:{}:{}",
        session_ref,
        crate::runtime::content_address(&format!("{}:{}:{}", session_ref, branch_ref, sequence))
    );

    Ok(serde_json::json!({
        "kind": "read_only_share_manifest",
        "manifest_id": manifest_id,
        "format_version": BUNDLE_FORMAT_VERSION,
        "session_ref": session_ref,
        "branch_ref": branch_ref,
        "sequence": sequence,
        "readonly": true,
        "share_scope": "local_file",
        "no_remote_service": true,
        "content_address": crate::runtime::content_address(&format!("readonly:{}:{}", session_ref, sequence)),
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id
        }
    }))
}

fn async_fork_share_plan(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(
            request,
            "input contains raw-secret-like content; use secret_ref references instead",
        ));
    }

    let source_session = request
        .input
        .get("source_session")
        .and_then(Value::as_str)
        .unwrap_or("session:source");

    let target_session = request
        .input
        .get("target_session")
        .and_then(Value::as_str)
        .unwrap_or("session:target");

    let fork_intent = request
        .input
        .get("fork_intent")
        .and_then(Value::as_str)
        .unwrap_or("explore_alternative");

    let branch_ref = request
        .input
        .get("branch_ref")
        .and_then(Value::as_str)
        .unwrap_or("branch:share-fork");

    let plan_id = format!(
        "async-fork-plan:{}:{}:{}",
        source_session,
        target_session,
        crate::runtime::content_address(&format!("{}:{}", fork_intent, branch_ref))
    );

    Ok(serde_json::json!({
        "kind": "async_fork_share_plan",
        "plan_id": plan_id,
        "format_version": BUNDLE_FORMAT_VERSION,
        "source_session": source_session,
        "target_session": target_session,
        "fork_intent": fork_intent,
        "branch_ref": branch_ref,
        "status": "draft",
        "share_scope": "local_file",
        "no_remote_service": true,
        "requires_user_approval": true,
        "plan_only": true,
        "content_address": crate::runtime::content_address(&format!("{}:{}:{}", source_session, target_session, fork_intent)),
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
        let req = make_request("official/sharing-lab/describe_sharing_contract", json!({}));
        assert!(try_handle(&req).is_some());
    }

    #[test]
    fn try_handle_rejects_wrong_package() {
        let req = InprocInvocation {
            capability_id: "official/sharing-lab/describe_sharing_contract".to_string(),
            provider_package_id: "official/other".to_string(),
            session_id: None,
            input: json!({}),
        };
        assert!(try_handle(&req).is_none());
    }

    #[test]
    fn describe_contract_has_all_surfaces() {
        let req = make_request("official/sharing-lab/describe_sharing_contract", json!({}));
        let result = try_handle(&req).unwrap().unwrap();
        let surfaces = result["surfaces"].as_object().unwrap();
        assert!(surfaces.contains_key("forge_panel"));
        assert!(surfaces.contains_key("assistant_action"));
        assert!(surfaces.contains_key("home_card"));
    }

    #[test]
    fn describe_contract_lists_9_capabilities() {
        let req = make_request("official/sharing-lab/describe_sharing_contract", json!({}));
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
    fn describe_contract_has_red_lines() {
        let req = make_request("official/sharing-lab/describe_sharing_contract", json!({}));
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["red_lines"]["no_marketplace"], json!(true));
        assert_eq!(result["red_lines"]["no_billing"], json!(true));
        assert_eq!(result["red_lines"]["no_signing_network"], json!(true));
        assert_eq!(result["red_lines"]["no_kernel_sharing"], json!(true));
        assert_eq!(result["red_lines"]["no_raw_secrets"], json!(true));
    }

    #[test]
    fn export_bundle_produces_composition_bundle() {
        let req = make_request(
            "official/sharing-lab/export_composition_bundle",
            json!({
                "composition_id": "test-comp",
                "packages": [
                    {"package_id": "official/playable-seed", "version": "0.1.0"},
                    {"package_id": "official/memory-lab", "version": "0.1.0"},
                ]
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("composition_bundle"));
        assert!(result["bundle_id"].is_string());
        assert!(result["package_set_lockfile"].is_object());
        assert!(result["ai_disclosure"].is_object());
        assert_eq!(result["no_marketplace_fields"], json!(true));
        assert_eq!(result["no_billing_fields"], json!(true));
    }

    #[test]
    fn export_bundle_blocks_raw_secret() {
        let req = make_request(
            "official/sharing-lab/export_composition_bundle",
            json!({"composition_id": "test", "api_key": "RawSecretExample1234567890abcdefABCDEF123456"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("sharing_lab_rejected"));
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn export_bundle_blocks_marketplace_fields() {
        let req = make_request(
            "official/sharing-lab/export_composition_bundle",
            json!({"composition_id": "test", "marketplace_id": "mp-123"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("sharing_lab_rejected"));
    }

    #[test]
    fn import_bundle_validates_shape() {
        let req = make_request(
            "official/sharing-lab/import_composition_bundle",
            json!({
                "bundle_id": "bundle:test:abc",
                "format_version": "1",
                "packages": [{"package_id": "official/playable-seed", "version": "0.1.0"}],
                "missing_packages": [{"package_id": "official/missing-pkg", "version": "0.1.0"}],
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("composition_bundle_import"));
        assert_eq!(
            result["compatibility_status"],
            json!("minor_incompatibility")
        );
        assert_eq!(result["requires_user_approval"], json!(true));
    }

    #[test]
    fn import_bundle_blocks_raw_secret() {
        let req = make_request(
            "official/sharing-lab/import_composition_bundle",
            json!({"bundle_id": "test", "token": "RawSecretExample1234567890abcdefABCDEF123456"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("sharing_lab_rejected"));
    }

    #[test]
    fn branch_session_bundle_produces_shape() {
        let req = make_request(
            "official/sharing-lab/create_branch_session_bundle",
            json!({"session_id": "sess:1", "branch_ref": "branch:main", "sequence": 42}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("branch_session_bundle"));
        assert_eq!(result["session_id"], json!("sess:1"));
        assert_eq!(result["sequence"], json!(42));
        assert!(result["content_address"].is_string());
    }

    #[test]
    fn package_set_lockfile_pins_versions() {
        let req = make_request(
            "official/sharing-lab/create_package_set_lockfile",
            json!({
                "packages": [
                    {"package_id": "official/playable-seed", "version": "0.1.0"},
                    {"package_id": "official/memory-lab", "version": "0.1.0"},
                ]
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("package_set_lockfile"));
        assert!(result["lockfile_id"].is_string());
        let packages = result["packages"].as_array().unwrap();
        assert_eq!(packages.len(), 2);
        for p in packages {
            assert!(p["content_address"].is_string());
        }
    }

    #[test]
    fn compatibility_report_detects_incompatibility() {
        let req = make_request(
            "official/sharing-lab/compatibility_report",
            json!({
                "source_ref": "bundle:v1",
                "target_ref": "bundle:v2",
                "source_packages": [
                    {"package_id": "official/playable-seed", "version": "0.1.0"},
                    {"package_id": "official/old-pkg", "version": "0.1.0"},
                ],
                "target_packages": [
                    {"package_id": "official/playable-seed", "version": "0.2.0"},
                ],
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("compatibility_report"));
        assert_eq!(result["status"], json!("major_incompatibility"));
        let incompat = result["incompatibilities"].as_array().unwrap();
        assert!(!incompat.is_empty());
    }

    #[test]
    fn ai_disclosure_bundle_produces_items() {
        let req = make_request(
            "official/sharing-lab/ai_disclosure_bundle",
            json!({
                "content_refs": ["asset:1", "asset:2"],
                "default_disclosure_kind": "ai_generated",
            }),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("ai_disclosure_bundle"));
        let items = result["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["disclosure_kind"], json!("ai_generated"));
    }

    #[test]
    fn read_only_share_manifest_is_local() {
        let req = make_request(
            "official/sharing-lab/read_only_share_manifest",
            json!({"session_ref": "sess:1", "sequence": 10}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("read_only_share_manifest"));
        assert_eq!(result["readonly"], json!(true));
        assert_eq!(result["share_scope"], json!("local_file"));
        assert_eq!(result["no_remote_service"], json!(true));
    }

    #[test]
    fn async_fork_share_plan_is_local() {
        let req = make_request(
            "official/sharing-lab/async_fork_share_plan",
            json!({"source_session": "sess:1", "target_session": "sess:2", "fork_intent": "explore"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("async_fork_share_plan"));
        assert_eq!(result["status"], json!("draft"));
        assert_eq!(result["share_scope"], json!("local_file"));
        assert_eq!(result["requires_user_approval"], json!(true));
        assert_eq!(result["plan_only"], json!(true));
    }

    #[test]
    fn no_forbidden_namespace_in_any_output() {
        let caps = [
            "describe_sharing_contract",
            "export_composition_bundle",
            "import_composition_bundle",
            "create_branch_session_bundle",
            "create_package_set_lockfile",
            "compatibility_report",
            "ai_disclosure_bundle",
            "read_only_share_manifest",
            "async_fork_share_plan",
        ];

        let forbidden = [
            "kernel.v1.sharing.",
            "kernel.v1.marketplace.",
            "kernel.v1.billing.",
            "kernel.v1.distribution.",
            "kernel.v1.experience.",
            "kernel.v1.world.",
            "kernel.v1.agent.",
            "kernel.v1.model.",
        ];

        for cap in &caps {
            let req = make_request(
                &format!("official/sharing-lab/{}", cap),
                json!({"test": "ns_check"}),
            );
            let result = try_handle(&req).unwrap().unwrap();
            let output_str = serde_json::to_string(&result).unwrap();
            for token in &forbidden {
                assert!(
                    !output_str.contains(token),
                    "{cap} must not contain {token}"
                );
            }
        }
    }
}
