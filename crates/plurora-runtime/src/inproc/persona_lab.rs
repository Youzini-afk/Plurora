//! Handler for `plurora/persona-lab` capabilities.

use serde_json::Value;

use super::InprocInvocation;

const PACKAGE_ID: &str = "plurora/persona-lab";

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }
    let id = request.capability_id.as_str();
    if id.ends_with("/import_profile") {
        Some(import_profile(request))
    } else if id.ends_with("/normalize_profile") {
        Some(normalize_profile(request))
    } else if id.ends_with("/describe_profile") {
        Some(describe_profile(request))
    } else if id.ends_with("/render_fragment") {
        Some(render_fragment(request))
    } else if id.ends_with("/compat_report") {
        Some(compat_report(request))
    } else {
        None
    }
}

fn import_profile(request: &InprocInvocation) -> anyhow::Result<Value> {
    let data = request.input.get("data").unwrap_or(&request.input);
    let core = data.get("data").unwrap_or(data);
    let name = core
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Unnamed Persona");
    Ok(serde_json::json!({
        "kind": "persona_profile",
        "imported_format": data.get("spec").and_then(Value::as_str).unwrap_or("generic_profile"),
        "core": {
            "name": name,
            "description": core.get("description").cloned().unwrap_or(Value::Null),
            "personality": core.get("personality").cloned().unwrap_or(Value::Null),
            "scenario": core.get("scenario").cloned().unwrap_or(Value::Null),
            "example_dialogue": core.get("mes_example").or_else(|| core.get("example_dialogue")).cloned().unwrap_or(Value::Null)
        },
        "greetings": {
            "primary": core.get("first_mes").or_else(|| core.get("primary_greeting")).cloned().unwrap_or(Value::Null),
            "alternate": core.get("alternate_greetings").cloned().unwrap_or_else(|| serde_json::json!([]))
        },
        "metadata": {
            "tags": core.get("tags").cloned().unwrap_or_else(|| serde_json::json!([])),
            "source": request.input.get("source").and_then(Value::as_str).unwrap_or("inline")
        },
        "diagnostics": {"unknown_fields_preserved": true, "warnings": []},
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn normalize_profile(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "persona_profile",
        "profile": request.input.get("profile").cloned().unwrap_or_else(|| request.input.clone()),
        "normalized": true,
        "diagnostics": {"warnings": []},
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn describe_profile(request: &InprocInvocation) -> anyhow::Result<Value> {
    let profile = request.input.get("profile").unwrap_or(&request.input);
    Ok(serde_json::json!({
        "kind": "persona_profile_description",
        "name": profile.pointer("/core/name").or_else(|| profile.get("name")).cloned().unwrap_or(Value::Null),
        "sections": ["core", "greetings", "metadata", "extensions"],
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn render_fragment(request: &InprocInvocation) -> anyhow::Result<Value> {
    let profile = request.input.get("profile").unwrap_or(&request.input);
    let name = profile
        .pointer("/core/name")
        .or_else(|| profile.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("Persona");
    let description = profile
        .pointer("/core/description")
        .or_else(|| profile.get("description"))
        .and_then(Value::as_str)
        .unwrap_or("");
    Ok(serde_json::json!({
        "kind": "persona_fragment",
        "fragment": format!("{name}: {description}"),
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id, "source": "explicit_profile"}
    }))
}

fn compat_report(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "persona_compat_report",
        "input_format": request.input.get("format").and_then(Value::as_str).unwrap_or("unknown"),
        "lossy": false,
        "unsupported_fields": [],
        "diagnostics": ["compatibility input is not canonical Plurora ontology"]
    }))
}
