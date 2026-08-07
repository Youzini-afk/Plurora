//! Handler for `official/text-transform-lab` capabilities.

use serde_json::Value;

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/text-transform-lab";

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }
    let id = request.capability_id.as_str();
    if id.ends_with("/import_rules") {
        Some(import_rules(request))
    } else if id.ends_with("/validate_rules") {
        Some(validate_rules(request))
    } else if id.ends_with("/apply_preview") {
        Some(apply_preview(request))
    } else if id.ends_with("/explain_pipeline") {
        Some(explain_pipeline(request))
    } else if id.ends_with("/compat_report") {
        Some(compat_report(request))
    } else {
        None
    }
}

fn import_rules(request: &InprocInvocation) -> anyhow::Result<Value> {
    let rules = request
        .input
        .get("rules")
        .cloned()
        .unwrap_or_else(|| request.input.clone());
    let count = rules.as_array().map(|rules| rules.len()).unwrap_or(0);
    Ok(serde_json::json!({
        "kind": "text_transform_profile",
        "rules": rules,
        "rule_count": count,
        "diagnostics": {"warnings": []},
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn validate_rules(request: &InprocInvocation) -> anyhow::Result<Value> {
    let rules = request
        .input
        .get("rules")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let diagnostics: Vec<Value> = rules
        .iter()
        .enumerate()
        .filter_map(|(index, rule)| {
            if rule.get("find").or_else(|| rule.get("findRegex")).is_none() {
                Some(serde_json::json!({"index": index, "severity": "warning", "message": "missing find pattern"}))
            } else {
                None
            }
        })
        .collect();
    Ok(
        serde_json::json!({"kind": "text_transform_validation", "valid": diagnostics.is_empty(), "diagnostics": diagnostics}),
    )
}

fn apply_preview(request: &InprocInvocation) -> anyhow::Result<Value> {
    let mut output = request
        .input
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let mut trace = Vec::new();
    if let Some(rules) = request.input.get("rules").and_then(Value::as_array) {
        for rule in rules {
            if rule
                .get("disabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                trace.push(serde_json::json!({"rule": rule.get("id").or_else(|| rule.get("scriptName")).cloned().unwrap_or(Value::Null), "applied": false, "reason": "disabled"}));
                continue;
            }
            let pattern = rule
                .get("find")
                .or_else(|| rule.get("findRegex"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let replacement = rule
                .get("replace")
                .or_else(|| rule.get("replaceString"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let simple_pattern = pattern
                .trim_start_matches('/')
                .split('/')
                .next()
                .unwrap_or(pattern);
            let before = output.clone();
            output = output.replace(simple_pattern, replacement);
            trace.push(serde_json::json!({"rule": rule.get("id").or_else(|| rule.get("scriptName")).cloned().unwrap_or(Value::Null), "applied": before != output, "pattern": simple_pattern}));
        }
    }
    Ok(serde_json::json!({
        "kind": "text_transform_preview",
        "input": request.input.get("text").cloned().unwrap_or(Value::Null),
        "output": output,
        "trace": trace,
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn explain_pipeline(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "text_transform_pipeline",
        "rules": request.input.get("rules").cloned().unwrap_or_else(|| serde_json::json!([])),
        "execution": "deterministic_ordered_preview",
        "safety": "preview_only_no_mutation",
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn compat_report(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "text_transform_compat_report",
        "input_format": request.input.get("format").and_then(Value::as_str).unwrap_or("unknown"),
        "diagnostics": ["regex-like compatibility rules are imported into generic transform profiles"]
    }))
}
