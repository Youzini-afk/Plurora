//! Handler for `official/playable-seed` capabilities.

use serde_json::Value;

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/playable-seed";

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }
    let id = request.capability_id.as_str();
    if id.ends_with("/create") {
        Some(create(request))
    } else if id.ends_with("/launch") {
        Some(launch(request))
    } else if id.ends_with("/describe_state") {
        Some(describe_state(request))
    } else if id.ends_with("/render_payload") {
        Some(render_payload(request))
    } else if id.ends_with("/propose_change") {
        Some(propose_change(request))
    } else {
        None
    }
}

fn create(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "playable_seed",
        "title": request.input.get("title").and_then(Value::as_str).unwrap_or("Playable Seed"),
        "state": request.input.get("state").cloned().unwrap_or_else(|| serde_json::json!({"steps": [], "note": "reference playable seed"})),
    }))
}

fn launch(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "playable_seed_launch",
        "title": request.input.get("title").and_then(Value::as_str).unwrap_or("Playable Seed"),
        "render_capability_id": "official/playable-seed/render_payload",
        "forge_panel_id": "official/playable-seed/forge-panel",
    }))
}

fn describe_state(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "playable_seed_state",
        "state": request.input.get("state").cloned().unwrap_or_else(|| serde_json::json!({})),
        "editable_through": "proposal",
    }))
}

fn render_payload(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "playable_seed_render_payload",
        "blocks": request.input.get("blocks").cloned().unwrap_or_else(|| serde_json::json!([{"type": "text", "text": "Playable Seed is running through package surfaces."}])),
    }))
}

fn propose_change(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "playable_seed_change_proposal",
        "requires_user_approval": true,
        "recommended_operations": ["asset.put", "projection.rebuild"],
        "proposal": request.input,
    }))
}
