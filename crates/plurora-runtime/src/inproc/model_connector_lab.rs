//! Handler for `official/model-connector-lab` capabilities.

use serde_json::Value;

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/model-connector-lab";

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }
    let id = request.capability_id.as_str();
    if id.ends_with("/describe_families") {
        Some(describe_families(request))
    } else if id.ends_with("/mask_secret") {
        Some(mask_secret(request))
    } else if id.ends_with("/validate_profile") {
        Some(validate_profile(request))
    } else if id.ends_with("/discovery_plan") {
        Some(discovery_plan(request))
    } else if id.ends_with("/compat_report") {
        Some(compat_report(request))
    } else {
        None
    }
}

fn describe_families(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "model_provider_families",
        "verification_level": "static_declared",
        "families": [
            {"id": "openai", "auth": "bearer_secret_ref", "base_url_required": false, "live_discovery": "planned_only"},
            {"id": "openai-compatible", "auth": "bearer_secret_ref", "base_url_required": true, "live_discovery": "planned_only"},
            {"id": "anthropic", "auth": "x-api-key_secret_ref", "base_url_required": false, "live_discovery": "planned_only"},
            {"id": "google", "auth": "key_secret_ref", "base_url_required": false, "live_discovery": "planned_only"},
            {"id": "deepseek", "auth": "bearer_secret_ref", "base_url_required": false, "live_discovery": "planned_only"},
            {"id": "xai", "auth": "bearer_secret_ref", "base_url_required": false, "live_discovery": "planned_only"}
        ],
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn mask_secret(request: &InprocInvocation) -> anyhow::Result<Value> {
    let secret = request
        .input
        .get("secret")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let masked = if secret.is_empty() {
        "<secret:redacted>".to_string()
    } else {
        let suffix: String = secret
            .chars()
            .rev()
            .take(4)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        format!("<secret:...{suffix}>")
    };
    Ok(serde_json::json!({"kind": "model_secret_mask", "masked": masked, "raw_returned": false}))
}

fn validate_profile(request: &InprocInvocation) -> anyhow::Result<Value> {
    let family = request
        .input
        .get("provider_family")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let supported = [
        "openai",
        "openai-compatible",
        "anthropic",
        "google",
        "deepseek",
        "xai",
    ]
    .contains(&family);
    let base_url = request
        .input
        .get("base_url")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let secret_ref = request
        .input
        .get("secret_ref")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let has_raw_secret =
        request.input.get("api_key").is_some() || request.input.get("secret").is_some();
    let mut diagnostics = Vec::new();
    if !supported {
        diagnostics.push(
            serde_json::json!({"severity": "error", "message": "unsupported provider_family"}),
        );
    }
    if family == "openai-compatible"
        && !(base_url.starts_with("http://") || base_url.starts_with("https://"))
    {
        diagnostics.push(serde_json::json!({"severity": "error", "message": "openai-compatible requires http(s) base_url"}));
    }
    if secret_ref.is_empty() {
        diagnostics.push(serde_json::json!({"severity": "warning", "message": "missing secret_ref; credential usability is not verified in Alpha"}));
    }
    if has_raw_secret {
        diagnostics.push(serde_json::json!({"severity": "error", "message": "raw secrets are not accepted; use secret_ref"}));
    }
    let valid = !diagnostics
        .iter()
        .any(|d| d.get("severity").and_then(Value::as_str) == Some("error"));
    Ok(serde_json::json!({
        "kind": "model_connector_profile_validation",
        "valid": valid,
        "verification_level": "not_verified",
        "profile": {
            "provider_family": family,
            "base_url": if base_url.is_empty() {Value::Null} else {serde_json::json!(base_url)},
            "model_id": request.input.get("model_id").cloned().unwrap_or(Value::Null),
            "secret_ref": if secret_ref.is_empty() {Value::Null} else {serde_json::json!(secret_ref)},
        },
        "diagnostics": diagnostics,
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn discovery_plan(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "model_discovery_plan",
        "provider_family": request.input.get("provider_family").cloned().unwrap_or(Value::Null),
        "steps": ["validate profile structure", "resolve secret reference", "request network permission", "fetch model list", "normalize provider response"],
        "status": "planned",
        "network_performed": false,
        "verification_level": "not_verified",
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn compat_report(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "model_connector_compat_report",
        "provider_family": request.input.get("provider_family").cloned().unwrap_or(Value::Null),
        "status": "static_profile_only",
        "compatible_with": request.input.get("provider_family").cloned().unwrap_or(Value::Null),
        "network_performed": false,
        "diagnostics": ["Alpha compatibility is structural and unverified; no live provider call was made"]
    }))
}
