//! Handler for `official/model-routing-lab` capabilities.

use serde_json::Value;

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/model-routing-lab";

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }
    let id = request.capability_id.as_str();
    if id.ends_with("/define_binding") {
        Some(define_binding(request))
    } else if id.ends_with("/resolve_binding") {
        Some(resolve_binding(request))
    } else if id.ends_with("/preview_routes") {
        Some(preview_routes(request))
    } else if id.ends_with("/params_normalize") {
        Some(params_normalize(request))
    } else if id.ends_with("/compat_report") {
        Some(compat_report(request))
    } else {
        None
    }
}

fn define_binding(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "model_route_binding",
        "consumer_slot": request.input.get("consumer_slot").cloned().unwrap_or(Value::Null),
        "scope": request.input.get("scope").and_then(Value::as_str).unwrap_or("session"),
        "bindings": request.input.get("bindings").cloned().unwrap_or_else(|| serde_json::json!([])),
        "requires_user_approval": true,
        "inference_performed": false,
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn resolve_binding(request: &InprocInvocation) -> anyhow::Result<Value> {
    let mut bindings = request
        .input
        .get("bindings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    bindings.sort_by(|a, b| {
        let ap = a.get("priority").and_then(Value::as_i64).unwrap_or(0);
        let bp = b.get("priority").and_then(Value::as_i64).unwrap_or(0);
        bp.cmp(&ap)
    });
    let selected = bindings.first().cloned().unwrap_or(Value::Null);
    let fallbacks: Vec<Value> = bindings.iter().skip(1).cloned().collect();
    Ok(serde_json::json!({
        "kind": "model_route_resolution",
        "consumer_slot": request.input.get("consumer_slot").cloned().unwrap_or(Value::Null),
        "selected": selected,
        "fallbacks": fallbacks,
        "deterministic": true,
        "inference_performed": false,
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn preview_routes(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "model_route_preview",
        "routes": request.input.get("bindings").cloned().unwrap_or_else(|| serde_json::json!([])),
        "status": "planned",
        "inference_performed": false,
        "diagnostics": ["routes are static plans; no model was invoked"]
    }))
}

fn params_normalize(request: &InprocInvocation) -> anyhow::Result<Value> {
    let params = request
        .input
        .get("params")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    Ok(serde_json::json!({
        "kind": "model_params_normalized",
        "params": {
            "temperature": params.get("temperature").cloned().unwrap_or_else(|| serde_json::json!(0.7)),
            "max_output_tokens": params.get("max_output_tokens").or_else(|| params.get("max_tokens")).cloned().unwrap_or_else(|| serde_json::json!(512)),
            "provider_options": params.get("provider_options").cloned().unwrap_or_else(|| serde_json::json!({}))
        },
        "provider_specific_namespaced": true,
        "inference_performed": false
    }))
}

fn compat_report(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "model_routing_compat_report",
        "consumer_slot": request.input.get("consumer_slot").cloned().unwrap_or(Value::Null),
        "status": "static_route_plan_only",
        "diagnostics": ["routing does not create a global model route and does not invoke inference"]
    }))
}
