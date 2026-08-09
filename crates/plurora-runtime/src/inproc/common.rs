//! Generic/foundation capability handlers shared across first-party Packages.
//!
//! These handlers match on `(provider_package_id, local_capability_name)` pairs,
//! requiring the capability_id to be under the provider_package_id namespace.
//! Only `plurora/` packages are served by these handlers; third-party Packages
//! are rejected to prevent accidental fallback behavior.

use serde_json::Value;

use super::InprocInvocation;

/// Extract the local capability name when `capability_id` is under
/// `provider_package_id` namespace.
///
/// Returns `None` when `capability_id` does not start with
/// `"<provider_package_id>/"`.
///
/// Example: provider `plurora/asset-lab`, capability `plurora/asset-lab/preview`
///          => local name `preview`.
fn extract_local_name<'a>(capability_id: &'a str, provider_package_id: &str) -> Option<&'a str> {
    if !capability_id.starts_with(provider_package_id) {
        return None;
    }
    let rest = capability_id.get(provider_package_id.len()..)?;
    if !rest.starts_with('/') {
        return None;
    }
    Some(&rest[1..])
}

pub fn try_handle(request: &mut InprocInvocation) -> Option<anyhow::Result<Value>> {
    // Only serve plurora/ packages through the shared handlers.
    if !request.provider_package_id.starts_with("plurora/") {
        return None;
    }

    let local_name = extract_local_name(&request.capability_id, &request.provider_package_id)?;

    match local_name {
        "echo" => Some(Ok(request.input.clone())),
        "fail" => Some(Err(anyhow::anyhow!(
            "first-party Package-lab requested failure"
        ))),
        "describe" => Some(describe(request)),
        "validate" => Some(validate()),
        "sample" => Some(sample(request)),
        "summarize" => Some(summarize(request)),
        "preview" => Some(preview(request)),
        "diff" => Some(diff(request)),
        "export" => Some(export(request)),
        "import_plan" => Some(import_plan(request)),
        "rebuild_plan" => Some(rebuild_plan(request)),
        "explain_source_events" => Some(explain_source_events(request)),
        "explain" => Some(explain(request)),
        "suggest" => Some(suggest()),
        "draft_branch_change" => Some(draft_branch_change(request)),
        "create_seed" => Some(create_seed(request)),
        "project" => Some(project(request)),
        "content_address" => Some(content_address(request)),
        "provenance_graph" => Some(provenance_graph(request)),
        "state_snapshot" => Some(state_snapshot(request)),
        _ => None,
    }
}

/// Returns an error for unhandled/unknown inproc capabilities.
///
/// Replaces the former permissive `fallback` that returned generic `{"ok": true}`
/// success for any unrecognized capability. Unknown capabilities must now fail
/// loudly so that callers receive a clear error instead of a misleading success.
pub fn unhandled_capability(request: &InprocInvocation) -> anyhow::Result<Value> {
    Err(anyhow::anyhow!(
        "no handler for inproc capability '{}' in package '{}'",
        request.capability_id,
        request.provider_package_id,
    ))
}

fn describe(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "package_id": request.provider_package_id,
        "capability_id": request.capability_id,
        "input_keys": request.input.as_object().map(|object| object.keys().cloned().collect::<Vec<_>>()).unwrap_or_default(),
    }))
}

fn validate() -> anyhow::Result<Value> {
    Ok(serde_json::json!({"valid": true, "diagnostics": []}))
}

fn sample(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({"sample": request.input.get("schema").cloned().unwrap_or(Value::Null)}))
}

fn summarize(request: &InprocInvocation) -> anyhow::Result<Value> {
    let count = request
        .input
        .get("events")
        .and_then(Value::as_array)
        .map(|events| events.len())
        .unwrap_or(0);
    Ok(serde_json::json!({"event_count": count}))
}

fn preview(request: &InprocInvocation) -> anyhow::Result<Value> {
    let content = request
        .input
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default();
    Ok(serde_json::json!({
        "kind": "asset_preview",
        "asset_id": request.input.get("asset_id").cloned().unwrap_or(Value::Null),
        "mime": request.input.get("mime").and_then(Value::as_str).unwrap_or("application/octet-stream"),
        "content_length": content.len(),
        "preview": content.chars().take(120).collect::<String>(),
    }))
}

fn diff(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "asset_diff",
        "from": request.input.get("from").cloned().unwrap_or(Value::Null),
        "to": request.input.get("to").cloned().unwrap_or(Value::Null),
        "requires_proposal": true,
    }))
}

fn export(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "asset_export",
        "asset_id": request.input.get("asset_id").cloned().unwrap_or(Value::Null),
        "format": request.input.get("format").and_then(Value::as_str).unwrap_or("json"),
        "content": request.input.get("content").cloned().unwrap_or(Value::Null),
    }))
}

fn import_plan(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "asset_import_plan",
        "requires_user_approval": true,
        "recommended_operation": "asset.put",
        "mime": request.input.get("mime").and_then(Value::as_str).unwrap_or("application/json"),
        "metadata": request.input.get("metadata").cloned().unwrap_or_else(|| serde_json::json!({})),
    }))
}

fn rebuild_plan(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "projection_rebuild_plan",
        "projection_id": request.input.get("projection_id").cloned().unwrap_or(Value::Null),
        "requires_user_approval": true,
        "recommended_operation": "projection.rebuild",
        "source_kind_prefix": request.input.get("source_kind_prefix").cloned().unwrap_or(Value::Null),
    }))
}

fn explain_source_events(request: &InprocInvocation) -> anyhow::Result<Value> {
    let event_count = request
        .input
        .get("events")
        .and_then(Value::as_array)
        .map(|events| events.len())
        .unwrap_or(0);
    Ok(serde_json::json!({
        "kind": "projection_source_events",
        "projection_id": request.input.get("projection_id").cloned().unwrap_or(Value::Null),
        "event_count": event_count,
        "source_kind_prefix": request.input.get("source_kind_prefix").cloned().unwrap_or(Value::Null),
    }))
}

fn explain(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "assistant_explanation",
        "summary": "Assistant package can explain protocol-visible context without private kernel access.",
        "context_keys": request.input.as_object().map(|object| object.keys().cloned().collect::<Vec<_>>()).unwrap_or_default(),
    }))
}

fn suggest() -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "assistant_suggestions",
        "suggestions": ["inspect events", "fork before changing", "invoke package capability through public protocol"],
    }))
}

fn draft_branch_change(request: &mut InprocInvocation) -> anyhow::Result<Value> {
    Ok(Value::Object(serde_json::Map::from_iter([
        (
            "kind".to_string(),
            Value::String("assistant_proposal".to_string()),
        ),
        ("requires_user_approval".to_string(), Value::Bool(true)),
        (
            "recommended_operation".to_string(),
            Value::String("context.fork".to_string()),
        ),
        ("proposal".to_string(), std::mem::take(&mut request.input)),
    ])))
}

fn create_seed(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "blank_experience_seed",
        "title": request.input.get("title").and_then(Value::as_str).unwrap_or("Blank Experience"),
        "seed": request.input,
    }))
}

fn project(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "blank_experience_projection",
        "state": request.input,
    }))
}

/// content_address: returns content-address shape and convention metadata.
/// No network, no inference. Used by asset-lab and other packages needing
/// stable content-addressed asset metadata.
fn content_address(request: &InprocInvocation) -> anyhow::Result<Value> {
    let content = request
        .input
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let scheme = request
        .input
        .get("scheme")
        .and_then(Value::as_str)
        .unwrap_or("sha256");
    let ca = match scheme {
        "sha256" => crate::runtime::content_address(content),
        "fnv1a64" => crate::runtime::legacy_content_address(content),
        other => anyhow::bail!("unsupported content address scheme '{other}'"),
    };
    Ok(serde_json::json!({
        "kind": "asset_content_address",
        "package_id": request.provider_package_id,
        "content_address": ca,
        "scheme": scheme,
        "size_bytes": content.len(),
        "metadata_convention": {
            "content_address": ca,
            "content_address_scheme": scheme,
            "provenance": {
                "origin_package_id": request.input.get("origin_package_id").and_then(Value::as_str).unwrap_or(request.provider_package_id.as_str()),
            },
            "disclosure": request.input.get("disclosure").and_then(Value::as_str).unwrap_or("unspecified"),
            "source_refs": request.input.get("source_refs").cloned().unwrap_or(serde_json::json!([])),
            "derived_refs": request.input.get("derived_refs").cloned().unwrap_or(serde_json::json!([])),
            "branch_ref": request.input.get("branch_ref").cloned().unwrap_or(Value::Null),
            "state_snapshot_ref": request.input.get("state_snapshot_ref").cloned().unwrap_or(Value::Null),
            "projection_ref": request.input.get("projection_ref").cloned().unwrap_or(Value::Null),
            "proposal_ref": request.input.get("proposal_ref").cloned().unwrap_or(Value::Null),
            "inference_ref": request.input.get("inference_ref").cloned().unwrap_or(Value::Null),
            "large_output_policy": request.input.get("large_output_policy").and_then(Value::as_str).unwrap_or("object_ref"),
        },
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id,
        }
    }))
}

/// provenance_graph: returns asset provenance graph shape with source/derived/ref slots.
/// No network, no inference. Used by asset-lab and other packages needing
/// asset lineage metadata.
fn provenance_graph(request: &InprocInvocation) -> anyhow::Result<Value> {
    let asset_id = request
        .input
        .get("asset_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    Ok(serde_json::json!({
        "kind": "asset_provenance_graph",
        "package_id": request.provider_package_id,
        "asset_id": if asset_id.is_empty() { Value::Null } else { serde_json::json!(asset_id) },
        "nodes": request.input.get("nodes").cloned().unwrap_or(serde_json::json!([])),
        "edges": request.input.get("edges").cloned().unwrap_or(serde_json::json!([])),
        "source_refs": request.input.get("source_refs").cloned().unwrap_or(serde_json::json!([])),
        "derived_refs": request.input.get("derived_refs").cloned().unwrap_or(serde_json::json!([])),
        "disclosure": request.input.get("disclosure").and_then(Value::as_str).unwrap_or("unspecified"),
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id,
        }
    }))
}

/// state_snapshot: returns state snapshot asset convention and branch-aware diff preview shape.
/// No network, no inference. Used by projection-lab and other packages needing
/// state snapshot metadata.
fn state_snapshot(request: &InprocInvocation) -> anyhow::Result<Value> {
    let projection_id = request
        .input
        .get("projection_id")
        .cloned()
        .unwrap_or(Value::Null);
    let branch_ref = request
        .input
        .get("branch_ref")
        .cloned()
        .unwrap_or(Value::Null);
    Ok(serde_json::json!({
        "kind": "projection_state_snapshot",
        "package_id": request.provider_package_id,
        "projection_id": projection_id,
        "state_snapshot_convention": {
            "snapshot_asset_mime": "application/vnd.plurora.state-snapshot+json",
            "snapshot_metadata_fields": [
                "content_address", "provenance", "disclosure",
                "source_refs", "derived_refs", "branch_ref",
                "state_snapshot_ref", "projection_ref",
                "proposal_ref", "inference_ref", "large_output_policy"
            ],
        },
        "diff_preview_shape": {
            "branch_aware": true,
            "requires_before_ref": true,
            "requires_after_ref": true,
            "output_fields": ["diff_summary", "changed_asset_refs", "branch_ref", "sequence"],
        },
        "branch_ref": branch_ref,
        "inference_performed": false,
        "network_performed": false,
        "provenance": {
            "package_id": request.provider_package_id,
            "capability_id": request.capability_id,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(provider: &str, cap: &str) -> InprocInvocation {
        InprocInvocation {
            capability_id: cap.to_string(),
            provider_package_id: provider.to_string(),
            session_id: None,
            input: serde_json::json!({}),
        }
    }

    #[test]
    fn extract_local_name_matches_namespace() {
        assert_eq!(
            extract_local_name("plurora/asset-lab/preview", "plurora/asset-lab"),
            Some("preview")
        );
        assert_eq!(
            extract_local_name("plurora/package-lab/echo", "plurora/package-lab"),
            Some("echo")
        );
    }

    #[test]
    fn extract_local_name_rejects_wrong_namespace() {
        assert_eq!(
            extract_local_name("thirdparty/pkg/preview", "plurora/asset-lab"),
            None
        );
        assert_eq!(
            extract_local_name("plurora/asset-lab/preview", "plurora/other"),
            None
        );
    }

    #[test]
    fn extract_local_name_rejects_partial_prefix() {
        // "plurora/asset" is a prefix of "plurora/asset-lab" but not a valid namespace
        assert_eq!(
            extract_local_name("plurora/asset-lab/preview", "plurora/asset"),
            None
        );
    }

    #[test]
    fn try_handle_first_party_preview() {
        let mut request = make_request("plurora/asset-lab", "plurora/asset-lab/preview");
        let result = try_handle(&mut request);
        assert!(
            result.is_some(),
            "first-party Package preview should be handled"
        );
        let output = result.unwrap().unwrap();
        assert_eq!(output["kind"], "asset_preview");
    }

    #[test]
    fn draft_branch_change_preserves_nested_input_without_reserializing() {
        let mut request = make_request(
            "plurora/assistant-lab",
            "plurora/assistant-lab/draft_branch_change",
        );
        request.input = serde_json::json!({
            "seed": {
                "kind": "blank_experience_seed",
                "seed": { "title": "Blank Loop", "intent": "prove substrate" }
            },
            "change": "try a first branch"
        });

        let output = try_handle(&mut request).unwrap().unwrap();

        assert_eq!(output["proposal"]["change"], "try a first branch");
        assert_eq!(output["proposal"]["seed"]["kind"], "blank_experience_seed");
        assert_eq!(output["requires_user_approval"], true);
    }

    #[test]
    fn try_handle_rejects_non_first_party() {
        let mut request = make_request("thirdparty/pkg", "thirdparty/pkg/preview");
        assert!(
            try_handle(&mut request).is_none(),
            "third-party Package should not be handled by common"
        );
    }

    #[test]
    fn try_handle_rejects_wrong_namespace() {
        let mut request = make_request("plurora/other", "plurora/asset-lab/preview");
        assert!(
            try_handle(&mut request).is_none(),
            "wrong namespace should not be handled"
        );
    }

    #[test]
    fn try_handle_unknown_local_name_returns_none() {
        let mut request = make_request("plurora/package-lab", "plurora/package-lab/nonexistent");
        assert!(
            try_handle(&mut request).is_none(),
            "unknown local name should return None"
        );
    }

    #[test]
    fn unhandled_capability_returns_error() {
        let request = make_request("plurora/package-lab", "plurora/package-lab/unknown");
        let result = unhandled_capability(&request);
        assert!(
            result.is_err(),
            "unhandled capability should return an error"
        );
    }
}
