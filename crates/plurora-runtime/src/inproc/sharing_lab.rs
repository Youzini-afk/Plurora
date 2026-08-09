//! Handler for `plurora/sharing-lab` capabilities.
//!
//! Experience Beta 6 — Sharing / Distribution Alpha.
//!
//! Package-owned sharing and distribution: Work bundle export/import,
//! branch/session bundle manifest, package-set lockfile, compatibility/migration
//! report, AI disclosure metadata bundle, read-only shared session manifest,
//! async fork sharing plan.
//!
//! Deterministic, no-network, no marketplace, no signing network, no billing.
//! All outputs are local/file-level proofs.
//!
//! No `platform.sharing.*`, `platform.marketplace.*`, `platform.billing.*`,
//! `platform.distribution.*` namespace references.
//!
//! Red lines:
//! - No marketplace, package signing network, dependency resolver economy,
//!   hosted billing.
//! - No `platform.sharing.*` / `platform.marketplace.*`.
//! - No raw secrets; `secret_ref` is reference-only, never resolved.
//! - No public network or remote service required.

use plurora_work::{
    validate_descriptor_type, ArtifactDescriptor, WorkId, ASSEMBLY_LOCK_TYPE_URI,
    ASSEMBLY_REVISION_TYPE_URI, CANONICAL_JSON_MEDIA_TYPE, WORK_REVISION_TYPE_URI,
};
use serde_json::Value;

use super::InprocInvocation;

const PACKAGE_ID: &str = "plurora/sharing-lab";

// ---------------------------------------------------------------------------
// Bundle format versions
// ---------------------------------------------------------------------------

const BUNDLE_FORMAT_VERSION: &str = "1";

// ---------------------------------------------------------------------------
// Sharing contract kinds
// ---------------------------------------------------------------------------

const SHARING_CONTRACT_KINDS: &[&str] = &[
    "work_bundle",
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
    "ai_reviewed",
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
                if matches!(
                    (key_lower.as_str(), val),
                    (
                        "no_marketplace_fields" | "no_billing_fields" | "no_signing_network_fields",
                        Value::Bool(true)
                    )
                ) {
                    continue;
                }
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

fn reject_unknown_fields(input: &Value, allowed: &[&str]) -> Result<(), String> {
    let object = input
        .as_object()
        .ok_or_else(|| "bundle input must be an object".to_string())?;
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("bundle input contains a field outside the Work bundle contract".to_string());
    }
    Ok(())
}

fn parse_typed_artifact(
    input: &Value,
    field: &str,
    expected_type_uri: &str,
    require_references: bool,
) -> Result<ArtifactDescriptor, String> {
    let descriptor_value = input
        .get(field)
        .cloned()
        .ok_or_else(|| format!("{field} is required"))?;
    reject_unknown_fields(
        &descriptor_value,
        &[
            "artifact_type_uri",
            "media_type",
            "digest",
            "size_bytes",
            "references",
            "annotations",
        ],
    )?;
    let descriptor = serde_json::from_value::<ArtifactDescriptor>(descriptor_value)
        .map_err(|_| format!("{field} must use the ArtifactDescriptor wire shape"))?;

    validate_descriptor_type(&descriptor, expected_type_uri)
        .map_err(|_| format!("{field} must be a valid {expected_type_uri} descriptor"))?;
    if descriptor.media_type != CANONICAL_JSON_MEDIA_TYPE {
        return Err(format!("{field} must reference canonical JSON"));
    }
    if descriptor.size_bytes == 0 {
        return Err(format!(
            "{field} must reference a non-empty canonical artifact"
        ));
    }
    if require_references && descriptor.references.is_empty() {
        return Err(format!(
            "{field} must carry its portable artifact references"
        ));
    }
    Ok(descriptor)
}

fn parse_work_bundle_identity(
    input: &Value,
) -> Result<
    (
        WorkId,
        ArtifactDescriptor,
        ArtifactDescriptor,
        ArtifactDescriptor,
    ),
    String,
> {
    let work_id = input
        .get("work_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "work_id is required".to_string())
        .and_then(|value| {
            WorkId::parse(value.to_string())
                .map_err(|_| "work_id does not satisfy the WorkId grammar".to_string())
        })?;
    let work_revision = parse_typed_artifact(input, "work_revision", WORK_REVISION_TYPE_URI, true)?;
    let assembly_revision = parse_typed_artifact(
        input,
        "assembly_revision",
        ASSEMBLY_REVISION_TYPE_URI,
        false,
    )?;
    let assembly_lock = parse_typed_artifact(input, "assembly_lock", ASSEMBLY_LOCK_TYPE_URI, true)?;

    if !work_revision
        .references
        .iter()
        .any(|reference| reference == &assembly_revision.digest)
        || !assembly_lock
            .references
            .iter()
            .any(|reference| reference == &assembly_revision.digest)
    {
        return Err(
            "work_revision and assembly_lock must both reference assembly_revision".to_string(),
        );
    }

    Ok((work_id, work_revision, assembly_revision, assembly_lock))
}

fn package_entries_for_export(input: &Value) -> Result<Vec<Value>, String> {
    let Some(packages) = input.get("packages") else {
        return Ok(Vec::new());
    };
    let packages = packages
        .as_array()
        .ok_or_else(|| "packages must be an array".to_string())?;
    packages
        .iter()
        .map(|package| {
            reject_unknown_fields(package, &["package_id", "version"])?;
            let package_id = package
                .get("package_id")
                .and_then(Value::as_str)
                .ok_or_else(|| "packages[].package_id is required".to_string())?;
            WorkId::parse(package_id.to_string())
                .map_err(|_| "packages[].package_id must use a safe namespaced id".to_string())?;
            let version = package
                .get("version")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "packages[].version is required".to_string())?;
            Ok(serde_json::json!({
                "package_id": package_id,
                "version": version,
                "content_address": crate::runtime::content_address(&format!("{package_id}:{version}")),
            }))
        })
        .collect()
}

fn package_entries_for_import(input: &Value) -> Result<Vec<Value>, String> {
    let lockfile = input
        .get("package_set_lockfile")
        .ok_or_else(|| "package_set_lockfile is required".to_string())?;
    reject_unknown_fields(
        lockfile,
        &[
            "lockfile_id",
            "format_version",
            "packages",
            "content_address",
        ],
    )?;
    let packages = lockfile
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| "package_set_lockfile.packages is required".to_string())?;
    for package in packages {
        reject_unknown_fields(package, &["package_id", "version", "content_address"])?;
        let package_id = package
            .get("package_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "package_set_lockfile.packages[].package_id is required".to_string())?;
        WorkId::parse(package_id.to_string()).map_err(|_| {
            "package_set_lockfile.packages[].package_id must use a safe namespaced id".to_string()
        })?;
        let version = package
            .get("version")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "package_set_lockfile.packages[].version is required".to_string())?;
        let expected = crate::runtime::content_address(&format!("{package_id}:{version}"));
        if package.get("content_address").and_then(Value::as_str) != Some(expected.as_str()) {
            return Err(
                "package_set_lockfile package content address does not match its identity"
                    .to_string(),
            );
        }
    }
    let packages = packages.clone();
    let canonical = serde_json::to_string(&packages)
        .map_err(|_| "package_set_lockfile cannot be encoded".to_string())?;
    let digest = crate::runtime::content_address(&canonical);
    let expected_lockfile_id = format!("lockfile:{digest}");
    if lockfile.get("lockfile_id").and_then(Value::as_str) != Some(expected_lockfile_id.as_str())
        || lockfile.get("format_version").and_then(Value::as_str) != Some(BUNDLE_FORMAT_VERSION)
        || lockfile.get("content_address").and_then(Value::as_str) != Some(digest.as_str())
    {
        return Err("package_set_lockfile identity does not match its packages".to_string());
    }
    Ok(packages)
}

fn ai_disclosure_for_export(work_revision: &ArtifactDescriptor) -> anyhow::Result<Value> {
    let items = serde_json::json!([{
        "content_ref": work_revision.digest,
        "disclosure_kind": "mixed",
        "description": "Work bundle with AI-generated and human-created content"
    }]);
    let digest = crate::runtime::content_address(&serde_json::to_string(&items)?);
    Ok(serde_json::json!({
        "disclosure_id": format!("ai-disclosure:{digest}"),
        "items": items,
        "content_address": digest,
    }))
}

fn ai_disclosure_for_import(
    input: &Value,
    work_revision: &ArtifactDescriptor,
) -> Result<Value, String> {
    let disclosure = input
        .get("ai_disclosure")
        .ok_or_else(|| "ai_disclosure is required".to_string())?;
    reject_unknown_fields(disclosure, &["disclosure_id", "items", "content_address"])?;
    let items = disclosure
        .get("items")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| "ai_disclosure.items must be a non-empty array".to_string())?;
    let mut covers_work_revision = false;
    for item in items {
        reject_unknown_fields(item, &["content_ref", "disclosure_kind", "description"])?;
        let content_ref = item
            .get("content_ref")
            .and_then(Value::as_str)
            .ok_or_else(|| "ai_disclosure.items[].content_ref is required".to_string())?;
        covers_work_revision |= content_ref == work_revision.digest;
        let kind = item
            .get("disclosure_kind")
            .and_then(Value::as_str)
            .ok_or_else(|| "ai_disclosure.items[].disclosure_kind is required".to_string())?;
        if !AI_DISCLOSURE_KINDS.contains(&kind) {
            return Err("ai_disclosure contains an unknown disclosure kind".to_string());
        }
        if !item.get("description").is_some_and(Value::is_string) {
            return Err("ai_disclosure.items[].description is required".to_string());
        }
    }
    if !covers_work_revision {
        return Err("ai_disclosure must cover work_revision".to_string());
    }
    let digest = crate::runtime::content_address(
        &serde_json::to_string(items)
            .map_err(|_| "ai_disclosure items cannot be encoded".to_string())?,
    );
    let expected_disclosure_id = format!("ai-disclosure:{digest}");
    if disclosure.get("disclosure_id").and_then(Value::as_str)
        != Some(expected_disclosure_id.as_str())
        || disclosure.get("content_address").and_then(Value::as_str) != Some(digest.as_str())
    {
        return Err("ai_disclosure identity does not match its items".to_string());
    }
    Ok(disclosure.clone())
}

fn work_bundle_id(
    work_id: &WorkId,
    work_revision: &ArtifactDescriptor,
    assembly_revision: &ArtifactDescriptor,
    assembly_lock: &ArtifactDescriptor,
    packages: &[Value],
    ai_disclosure: &Value,
) -> anyhow::Result<String> {
    let identity = serde_json::json!({
        "work_id": work_id.as_str(),
        "work_revision": work_revision,
        "assembly_revision": assembly_revision,
        "assembly_lock": assembly_lock,
        "packages": packages,
        "ai_disclosure": ai_disclosure,
    });
    Ok(format!(
        "work-bundle:{}:{}",
        work_id,
        crate::runtime::content_address(&serde_json::to_string(&identity)?)
    ))
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
    } else if id.ends_with("/export_work_bundle") {
        Some(export_work_bundle(request))
    } else if id.ends_with("/import_work_bundle") {
        Some(import_work_bundle(request))
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
            {"id": "plurora/sharing-lab/describe_sharing_contract", "purpose": "describe the sharing lab package contract"},
            {"id": "plurora/sharing-lab/export_work_bundle", "purpose": "export content-addressed WorkRevision, AssemblyRevision, and AssemblyLock roots with distribution and disclosure metadata"},
            {"id": "plurora/sharing-lab/import_work_bundle", "purpose": "validate a Work bundle and produce an approval-gated import plan without installing or running it"},
            {"id": "plurora/sharing-lab/create_branch_session_bundle", "purpose": "create a branch/session bundle manifest for sharing a specific session state"},
            {"id": "plurora/sharing-lab/create_package_set_lockfile", "purpose": "create a package-set lockfile pinning exact package versions and content addresses"},
            {"id": "plurora/sharing-lab/compatibility_report", "purpose": "produce a compatibility/migration report between two bundle versions or package sets"},
            {"id": "plurora/sharing-lab/ai_disclosure_bundle", "purpose": "produce AI disclosure metadata bundle for Work or session content"},
            {"id": "plurora/sharing-lab/read_only_share_manifest", "purpose": "create a read-only shared session manifest proof — local/file-level, no remote service"},
            {"id": "plurora/sharing-lab/async_fork_share_plan", "purpose": "create an async fork sharing plan — local proof for deferred/async session fork sharing"},
        ],
        "surfaces": {
            "forge_panel": "plurora/sharing-lab/forge-panel",
            "assistant_action": "plurora/sharing-lab/assistant-action",
            "home_card": "plurora/sharing-lab/home-card",
        },
        "sharing_contract_kinds": SHARING_CONTRACT_KINDS,
        "compat_status_kinds": COMPAT_STATUS_KINDS,
        "ai_disclosure_kinds": AI_DISCLOSURE_KINDS,
        "async_fork_statuses": ASYNC_FORK_STATUSES,
        "output_shapes": {
            "work_bundle": ["bundle_id", "format_version", "work_id", "work_revision", "assembly_revision", "assembly_lock", "package_set_lockfile", "ai_disclosure"],
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
            "no_platform_sharing": true,
            "no_platform_marketplace": true,
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

fn export_work_bundle(request: &InprocInvocation) -> anyhow::Result<Value> {
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
    if let Err(reason) = reject_unknown_fields(
        &request.input,
        &[
            "work_id",
            "work_revision",
            "assembly_revision",
            "assembly_lock",
            "packages",
        ],
    ) {
        return Ok(rejected_output(request, &reason));
    }

    let (work_id, work_revision, assembly_revision, assembly_lock) =
        match parse_work_bundle_identity(&request.input) {
            Ok(identity) => identity,
            Err(reason) => return Ok(rejected_output(request, &reason)),
        };

    let package_entries = match package_entries_for_export(&request.input) {
        Ok(entries) => entries,
        Err(reason) => return Ok(rejected_output(request, &reason)),
    };
    let package_set_content = serde_json::to_string(&package_entries)?;
    let package_set_digest = crate::runtime::content_address(&package_set_content);
    let lockfile_id = format!("lockfile:{package_set_digest}");
    let ai_disclosure = ai_disclosure_for_export(&work_revision)?;
    let bundle_id = work_bundle_id(
        &work_id,
        &work_revision,
        &assembly_revision,
        &assembly_lock,
        &package_entries,
        &ai_disclosure,
    )?;

    Ok(serde_json::json!({
        "kind": "work_bundle",
        "bundle_id": bundle_id,
        "format_version": BUNDLE_FORMAT_VERSION,
        "work_id": work_id,
        "work_revision": work_revision,
        "assembly_revision": assembly_revision,
        "assembly_lock": assembly_lock,
        "package_set_lockfile": {
            "lockfile_id": lockfile_id,
            "format_version": BUNDLE_FORMAT_VERSION,
            "packages": package_entries,
            "content_address": package_set_digest,
        },
        "ai_disclosure": ai_disclosure,
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

fn import_work_bundle(request: &InprocInvocation) -> anyhow::Result<Value> {
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
    if let Err(reason) = reject_unknown_fields(
        &request.input,
        &[
            "kind",
            "bundle_id",
            "format_version",
            "work_id",
            "work_revision",
            "assembly_revision",
            "assembly_lock",
            "package_set_lockfile",
            "ai_disclosure",
            "no_marketplace_fields",
            "no_billing_fields",
            "no_signing_network_fields",
            "inference_performed",
            "network_performed",
            "provenance",
            "missing_packages",
        ],
    ) {
        return Ok(rejected_output(request, &reason));
    }

    if request.input.get("kind").and_then(Value::as_str) != Some("work_bundle") {
        return Ok(rejected_output(request, "kind must identify a work_bundle"));
    }

    let bundle_id = request
        .input
        .get("bundle_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let Some(bundle_id) = bundle_id else {
        return Ok(rejected_output(request, "bundle_id is required"));
    };

    let format_version = request.input.get("format_version").and_then(Value::as_str);
    let Some(format_version) = format_version else {
        return Ok(rejected_output(request, "format_version is required"));
    };

    let (work_id, work_revision, assembly_revision, assembly_lock) =
        match parse_work_bundle_identity(&request.input) {
            Ok(identity) => identity,
            Err(reason) => return Ok(rejected_output(request, &reason)),
        };

    let packages = match package_entries_for_import(&request.input) {
        Ok(entries) => entries,
        Err(reason) => return Ok(rejected_output(request, &reason)),
    };
    let ai_disclosure = match ai_disclosure_for_import(&request.input, &work_revision) {
        Ok(disclosure) => disclosure,
        Err(reason) => return Ok(rejected_output(request, &reason)),
    };
    let expected_bundle_id = work_bundle_id(
        &work_id,
        &work_revision,
        &assembly_revision,
        &assembly_lock,
        &packages,
        &ai_disclosure,
    )?;
    if bundle_id != expected_bundle_id.as_str() {
        return Ok(rejected_output(
            request,
            "bundle_id does not match the Work bundle content",
        ));
    }

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
            "unsupported",
            vec![serde_json::json!({
                "kind": "format_version_mismatch",
                "expected": BUNDLE_FORMAT_VERSION,
                "found": format_version,
                "action": "provide a work bundle using the supported format"
            })],
        )
    } else if has_missing {
        (
            "minor_incompatibility",
            vec![serde_json::json!({
                "kind": "missing_packages",
                "count": missing_packages.len(),
                "action": "install missing packages or resolve the Assembly with available providers"
            })],
        )
    } else {
        ("compatible", vec![])
    };

    Ok(serde_json::json!({
        "kind": "work_bundle_import",
        "bundle_id": bundle_id,
        "format_version": format_version,
        "work_id": work_id,
        "work_revision": work_revision,
        "assembly_revision": assembly_revision,
        "assembly_lock": assembly_lock,
        "packages": packages,
        "ai_disclosure": ai_disclosure,
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

    fn digest(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn artifact_descriptor(type_uri: &str, byte: char, references: &[char]) -> Value {
        json!({
            "artifact_type_uri": type_uri,
            "media_type": CANONICAL_JSON_MEDIA_TYPE,
            "digest": digest(byte),
            "size_bytes": 512,
            "references": references.iter().copied().map(digest).collect::<Vec<_>>(),
            "annotations": {},
        })
    }

    fn valid_work_bundle_input() -> Value {
        json!({
            "work_id": "example/test-work",
            "work_revision": artifact_descriptor(WORK_REVISION_TYPE_URI, 'a', &['c']),
            "assembly_revision": artifact_descriptor(ASSEMBLY_REVISION_TYPE_URI, 'c', &['d']),
            "assembly_lock": artifact_descriptor(ASSEMBLY_LOCK_TYPE_URI, 'b', &['c', 'd']),
            "packages": [
                {"package_id": "plurora/playable-seed", "version": "0.1.0"},
                {"package_id": "plurora/memory-lab", "version": "0.1.0"},
            ],
        })
    }

    #[test]
    fn try_handle_matches_package_id() {
        let req = make_request("plurora/sharing-lab/describe_sharing_contract", json!({}));
        assert!(try_handle(&req).is_some());
    }

    #[test]
    fn try_handle_rejects_wrong_package() {
        let req = InprocInvocation {
            capability_id: "plurora/sharing-lab/describe_sharing_contract".to_string(),
            provider_package_id: "plurora/other".to_string(),
            session_id: None,
            input: json!({}),
        };
        assert!(try_handle(&req).is_none());
    }

    #[test]
    fn describe_contract_has_all_surfaces() {
        let req = make_request("plurora/sharing-lab/describe_sharing_contract", json!({}));
        let result = try_handle(&req).unwrap().unwrap();
        let surfaces = result["surfaces"].as_object().unwrap();
        assert!(surfaces.contains_key("forge_panel"));
        assert!(surfaces.contains_key("assistant_action"));
        assert!(surfaces.contains_key("home_card"));
    }

    #[test]
    fn describe_contract_lists_9_capabilities() {
        let req = make_request("plurora/sharing-lab/describe_sharing_contract", json!({}));
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
        let req = make_request("plurora/sharing-lab/describe_sharing_contract", json!({}));
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["red_lines"]["no_marketplace"], json!(true));
        assert_eq!(result["red_lines"]["no_billing"], json!(true));
        assert_eq!(result["red_lines"]["no_signing_network"], json!(true));
        assert_eq!(result["red_lines"]["no_platform_sharing"], json!(true));
        assert_eq!(result["red_lines"]["no_raw_secrets"], json!(true));
    }

    #[test]
    fn export_bundle_produces_work_bundle() {
        let req = make_request(
            "plurora/sharing-lab/export_work_bundle",
            valid_work_bundle_input(),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("work_bundle"));
        assert!(result["bundle_id"].is_string());
        assert_eq!(result["work_id"], json!("example/test-work"));
        assert_eq!(
            result["work_revision"]["artifact_type_uri"],
            json!(WORK_REVISION_TYPE_URI)
        );
        assert_eq!(
            result["assembly_revision"]["artifact_type_uri"],
            json!(ASSEMBLY_REVISION_TYPE_URI)
        );
        assert_eq!(
            result["assembly_lock"]["artifact_type_uri"],
            json!(ASSEMBLY_LOCK_TYPE_URI)
        );
        assert!(result["package_set_lockfile"].is_object());
        assert!(result["ai_disclosure"].is_object());
        assert_eq!(result["no_marketplace_fields"], json!(true));
        assert_eq!(result["no_billing_fields"], json!(true));
    }

    #[test]
    fn export_bundle_blocks_raw_secret() {
        let req = make_request(
            "plurora/sharing-lab/export_work_bundle",
            json!({"work_id": "example/test-work", "api_key": "RawSecretExample1234567890abcdefABCDEF123456"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("sharing_lab_rejected"));
        assert_eq!(result["redaction_state"], json!("unsafe_blocked"));
    }

    #[test]
    fn export_bundle_blocks_marketplace_fields() {
        let req = make_request(
            "plurora/sharing-lab/export_work_bundle",
            json!({"work_id": "example/test-work", "marketplace_id": "mp-123"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("sharing_lab_rejected"));
    }

    #[test]
    fn import_bundle_validates_shape() {
        let exported = export_work_bundle(&make_request(
            "plurora/sharing-lab/export_work_bundle",
            valid_work_bundle_input(),
        ))
        .unwrap();
        let mut input = exported;
        input.as_object_mut().unwrap().insert(
            "missing_packages".to_string(),
            json!([{"package_id": "plurora/missing-pkg", "version": "0.1.0"}]),
        );
        let req = make_request("plurora/sharing-lab/import_work_bundle", input);
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("work_bundle_import"));
        assert_eq!(
            result["compatibility_status"],
            json!("minor_incompatibility")
        );
        assert_eq!(result["requires_user_approval"], json!(true));
        assert_eq!(result["plan_only"], json!(true));
    }

    #[test]
    fn work_bundle_rejects_invalid_descriptor_shapes() {
        let mut cases = Vec::new();

        let mut wrong_type = valid_work_bundle_input();
        wrong_type["work_revision"]["artifact_type_uri"] = json!(ASSEMBLY_LOCK_TYPE_URI);
        cases.push(wrong_type);

        let mut bad_digest = valid_work_bundle_input();
        bad_digest["work_revision"]["digest"] = json!("sha256:bad");
        cases.push(bad_digest);

        let mut bad_size = valid_work_bundle_input();
        bad_size["work_revision"]["size_bytes"] = json!("512");
        cases.push(bad_size);

        let mut bad_reference = valid_work_bundle_input();
        bad_reference["assembly_lock"]["references"] = json!(["sha256:bad"]);
        cases.push(bad_reference);

        let mut unrelated_closures = valid_work_bundle_input();
        unrelated_closures["assembly_lock"]["references"] = json!([digest('e')]);
        cases.push(unrelated_closures);

        for input in cases {
            let result = try_handle(&make_request(
                "plurora/sharing-lab/export_work_bundle",
                input,
            ))
            .unwrap()
            .unwrap();
            assert_eq!(result["kind"], json!("sharing_lab_rejected"));
        }
    }

    #[test]
    fn import_bundle_recomputes_content_identity() {
        let mut exported = export_work_bundle(&make_request(
            "plurora/sharing-lab/export_work_bundle",
            valid_work_bundle_input(),
        ))
        .unwrap();
        exported["assembly_revision"]["size_bytes"] = json!(513);

        let result = import_work_bundle(&make_request(
            "plurora/sharing-lab/import_work_bundle",
            exported,
        ))
        .unwrap();

        assert_eq!(result["kind"], json!("sharing_lab_rejected"));
        assert!(result["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("bundle_id")));
    }

    #[test]
    fn import_bundle_recomputes_disclosure_identity() {
        let mut exported = export_work_bundle(&make_request(
            "plurora/sharing-lab/export_work_bundle",
            valid_work_bundle_input(),
        ))
        .unwrap();
        exported["ai_disclosure"]["items"][0]["description"] = json!("tampered");

        let result = import_work_bundle(&make_request(
            "plurora/sharing-lab/import_work_bundle",
            exported,
        ))
        .unwrap();

        assert_eq!(result["kind"], json!("sharing_lab_rejected"));
        assert!(result["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("ai_disclosure")));
    }

    #[test]
    fn import_bundle_blocks_raw_secret() {
        let req = make_request(
            "plurora/sharing-lab/import_work_bundle",
            json!({"bundle_id": "test", "token": "RawSecretExample1234567890abcdefABCDEF123456"}),
        );
        let result = try_handle(&req).unwrap().unwrap();
        assert_eq!(result["kind"], json!("sharing_lab_rejected"));
    }

    #[test]
    fn branch_session_bundle_produces_shape() {
        let req = make_request(
            "plurora/sharing-lab/create_branch_session_bundle",
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
            "plurora/sharing-lab/create_package_set_lockfile",
            json!({
                "packages": [
                    {"package_id": "plurora/playable-seed", "version": "0.1.0"},
                    {"package_id": "plurora/memory-lab", "version": "0.1.0"},
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
            "plurora/sharing-lab/compatibility_report",
            json!({
                "source_ref": "bundle:v1",
                "target_ref": "bundle:v2",
                "source_packages": [
                    {"package_id": "plurora/playable-seed", "version": "0.1.0"},
                    {"package_id": "plurora/old-pkg", "version": "0.1.0"},
                ],
                "target_packages": [
                    {"package_id": "plurora/playable-seed", "version": "0.2.0"},
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
            "plurora/sharing-lab/ai_disclosure_bundle",
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
            "plurora/sharing-lab/read_only_share_manifest",
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
            "plurora/sharing-lab/async_fork_share_plan",
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
            "export_work_bundle",
            "import_work_bundle",
            "create_branch_session_bundle",
            "create_package_set_lockfile",
            "compatibility_report",
            "ai_disclosure_bundle",
            "read_only_share_manifest",
            "async_fork_share_plan",
        ];

        let forbidden = [
            "platform.sharing.",
            "platform.marketplace.",
            "platform.billing.",
            "platform.distribution.",
            "platform.experience.",
            "platform.world.",
            "platform.agent.",
            "platform.model.",
        ];

        for cap in &caps {
            let req = make_request(
                &format!("plurora/sharing-lab/{}", cap),
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
