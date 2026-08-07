//! Handler for `official/context-lab` capabilities.

use serde_json::Value;

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/context-lab";

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }
    let id = request.capability_id.as_str();
    if id.ends_with("/assemble_preview") {
        Some(assemble_preview(request))
    } else if id.ends_with("/inspect_layers") {
        Some(inspect_layers(request))
    } else if id.ends_with("/budget_plan") {
        Some(budget_plan(request))
    } else if id.ends_with("/render_template") {
        Some(render_template(request))
    } else if id.ends_with("/explain_assembly") {
        Some(explain_assembly(request))
    } else {
        None
    }
}

fn assemble_preview(request: &InprocInvocation) -> anyhow::Result<Value> {
    let sources = request
        .input
        .get("sources")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let budget = request
        .input
        .get("budget")
        .and_then(Value::as_u64)
        .unwrap_or(4096);
    let mut used = 0_u64;
    let mut included = Vec::new();
    let mut omitted = Vec::new();
    for source in sources {
        let text = source
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let cost = text.len() as u64;
        if used + cost <= budget {
            used += cost;
            included.push(serde_json::json!({"source": source, "estimated_cost": cost, "reason": "fits_budget"}));
        } else {
            omitted.push(serde_json::json!({"source": source, "estimated_cost": cost, "reason": "budget_exceeded"}));
        }
    }
    Ok(serde_json::json!({
        "kind": "context_preview",
        "blocks": included,
        "omitted": omitted,
        "budget": {"limit": budget, "used": used, "unit": "chars_estimate"},
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn inspect_layers(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "context_layer_inspection",
        "layers": request.input.get("layers").cloned().unwrap_or_else(|| serde_json::json!([])),
        "diagnostics": ["layers are generic context blocks, not chat roles"],
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn budget_plan(request: &InprocInvocation) -> anyhow::Result<Value> {
    let requested = request
        .input
        .get("requested")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let limit = request
        .input
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(4096);
    Ok(serde_json::json!({
        "kind": "context_budget_plan",
        "requested": requested,
        "limit": limit,
        "fits": requested <= limit,
        "omitted_reason": if requested <= limit {Value::Null} else {serde_json::json!("budget_exceeded")},
    }))
}

fn render_template(request: &InprocInvocation) -> anyhow::Result<Value> {
    let mut rendered = request
        .input
        .get("template")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if let Some(vars) = request.input.get("variables").and_then(Value::as_object) {
        for (key, value) in vars {
            let replacement = value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| value.to_string());
            rendered = rendered.replace(&format!("{{{{{key}}}}}"), &replacement);
        }
    }
    Ok(serde_json::json!({
        "kind": "context_template_render",
        "rendered": rendered,
        "unresolved_policy": "leave_placeholder",
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn explain_assembly(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "context_assembly_explanation",
        "summary": "Context Lab assembles explicit source blocks under an explicit budget without model calls or chat ontology.",
        "input_keys": request.input.as_object().map(|object| object.keys().cloned().collect::<Vec<_>>()).unwrap_or_default(),
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}
