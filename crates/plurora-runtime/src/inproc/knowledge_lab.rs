//! Handler for `official/knowledge-lab` capabilities.

use serde_json::Value;

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/knowledge-lab";

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }
    let id = request.capability_id.as_str();
    if id.ends_with("/import_collection") {
        Some(import_collection(request))
    } else if id.ends_with("/normalize_entries") {
        Some(normalize_entries(request))
    } else if id.ends_with("/match_entries") {
        Some(match_entries(request))
    } else if id.ends_with("/injection_plan") {
        Some(injection_plan(request))
    } else if id.ends_with("/compat_report") {
        Some(compat_report(request))
    } else {
        None
    }
}

fn import_collection(request: &InprocInvocation) -> anyhow::Result<Value> {
    let data = request.input.get("data").unwrap_or(&request.input);
    let entries_value = data
        .get("entries")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let entries: Vec<Value> = if let Some(array) = entries_value.as_array() {
        array.clone()
    } else if let Some(object) = entries_value.as_object() {
        object.values().cloned().collect()
    } else {
        Vec::new()
    };
    Ok(serde_json::json!({
        "kind": "knowledge_collection",
        "name": data.get("name").and_then(Value::as_str).unwrap_or("Knowledge Collection"),
        "entries": entries,
        "entry_count": entries.len(),
        "diagnostics": {"compatibility_input": request.input.get("format").cloned().unwrap_or(Value::Null), "warnings": []},
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn normalize_entries(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "knowledge_collection",
        "entries": request.input.get("entries").cloned().unwrap_or_else(|| serde_json::json!([])),
        "normalized": true,
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn match_entries(request: &InprocInvocation) -> anyhow::Result<Value> {
    let query = request
        .input
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    let entries = request
        .input
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut matches = Vec::new();
    for entry in entries {
        let keys = entry
            .get("key")
            .or_else(|| entry.get("keys"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let hit = keys.iter().any(|key| {
            key.as_str()
                .map(|key| query.contains(&key.to_lowercase()))
                .unwrap_or(false)
        });
        if hit
            || entry
                .get("constant")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            matches.push(
                serde_json::json!({"entry": entry, "reason": if hit {"keyword"} else {"constant"}}),
            );
        }
    }
    Ok(serde_json::json!({
        "kind": "knowledge_match_result",
        "query": request.input.get("query").cloned().unwrap_or(Value::Null),
        "matches": matches,
        "trace": {"algorithm": "deterministic_keyword_contains", "case_sensitive": false},
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn injection_plan(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "knowledge_injection_plan",
        "matches": request.input.get("matches").cloned().unwrap_or_else(|| serde_json::json!([])),
        "plan_only": true,
        "requires_user_approval": request.input.get("requires_user_approval").and_then(Value::as_bool).unwrap_or(true),
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn compat_report(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "knowledge_compat_report",
        "input_format": request.input.get("format").and_then(Value::as_str).unwrap_or("unknown"),
        "lossy": false,
        "diagnostics": ["worldbook-like inputs are compatibility formats, not canonical ontology"]
    }))
}
