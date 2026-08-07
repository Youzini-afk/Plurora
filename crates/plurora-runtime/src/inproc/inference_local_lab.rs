//! Handler for `official/inference-local-lab` capabilities.
//!
//! Deterministic non-HTTP fake local inference provider proof.
//! Proves the inference capability seam does not depend on cloud APIs,
//! HTTP, bearer tokens, JSON provider schemas, or network access.
//!
//! This is NOT a local model platform. It is a seam proof that prevents
//! the abstraction from hardening into an HTTP proxy.

use serde_json::Value;

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/inference-local-lab";

// HTTP-shaped field names that must be rejected
const HTTP_SHAPED_FIELDS: &[&str] = &[
    "url",
    "header",
    "headers",
    "status_code",
    "statusCode",
    "status",
];

// Messages/chat-shaped field names that must be rejected
const MESSAGES_SHAPED_FIELDS: &[&str] = &["messages", "system", "user", "assistant"];

// ---------------------------------------------------------------------------
// Top-level dispatch
// ---------------------------------------------------------------------------

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }
    let id = request.capability_id.as_str();
    if id.ends_with("/describe_capabilities") {
        Some(describe_capabilities(request))
    } else if id.ends_with("/invoke") {
        Some(invoke(request))
    } else if id.ends_with("/stream") {
        Some(stream(request))
    } else if id.ends_with("/explain_error") {
        Some(explain_error(request))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// describe_capabilities
// ---------------------------------------------------------------------------

fn describe_capabilities(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "inference_local_capabilities",
        "transport_kinds": ["in_memory", "local_process"],
        "runtime_kind": "in_memory",
        "network_required": false,
        "secrets_required": false,
        "streaming_supported": true,
        "operation_kinds": ["generate", "classify", "transform"],
        "modalities": ["text"],
        "provider_type": "fake_local_deterministic",
        "http_supported": false,
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

// ---------------------------------------------------------------------------
// invoke
// ---------------------------------------------------------------------------

fn invoke(request: &InprocInvocation) -> anyhow::Result<Value> {
    let input = &request.input;

    // Reject transport_kind=http
    let transport_kind = input
        .get("transport_kind")
        .and_then(Value::as_str)
        .unwrap_or("in_memory");
    if transport_kind == "http" {
        return Ok(serde_json::json!({
            "kind": "inference_local_invoke_result",
            "operation_kind": input.get("operation_kind").and_then(Value::as_str).unwrap_or("generate"),
            "normalized_error": {
                "error_kind": "transport_rejected",
                "message": "http transport is not supported; use in_memory or local_process",
                "retryable": false,
            },
            "network_performed": false,
            "transport_performed": "none",
            "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
        }));
    }

    // Reject HTTP-shaped fields
    for field in HTTP_SHAPED_FIELDS {
        if input.get(*field).is_some() {
            return Ok(serde_json::json!({
                "kind": "inference_local_invoke_result",
                "operation_kind": input.get("operation_kind").and_then(Value::as_str).unwrap_or("generate"),
                "normalized_error": {
                    "error_kind": "http_field_rejected",
                    "message": format!("field '{}' is not accepted; this provider does not use HTTP-shaped payloads", field),
                    "retryable": false,
                },
                "network_performed": false,
                "transport_performed": "none",
                "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
            }));
        }
    }

    // Reject messages/chat-shaped fields
    for field in MESSAGES_SHAPED_FIELDS {
        if input.get(*field).is_some() {
            return Ok(serde_json::json!({
                "kind": "inference_local_invoke_result",
                "operation_kind": input.get("operation_kind").and_then(Value::as_str).unwrap_or("generate"),
                "normalized_error": {
                    "error_kind": "messages_field_rejected",
                    "message": format!("field '{}' is not accepted; this provider does not use chat/messages-shaped payloads", field),
                    "retryable": false,
                },
                "network_performed": false,
                "transport_performed": "none",
                "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
            }));
        }
    }

    // Reject raw secret-looking fields
    if looks_like_raw_secret_field(input) {
        return Ok(serde_json::json!({
            "kind": "inference_local_invoke_result",
            "operation_kind": input.get("operation_kind").and_then(Value::as_str).unwrap_or("generate"),
            "normalized_error": {
                "error_kind": "secret_rejected",
                "message": "raw secret fields are not accepted; this provider does not require secrets",
                "retryable": false,
            },
            "network_performed": false,
            "transport_performed": "none",
            "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
        }));
    }

    // Extract operation details
    let operation_id = input
        .get("operation_id")
        .and_then(Value::as_str)
        .unwrap_or("op_local_001");
    let operation_kind = input
        .get("operation_kind")
        .and_then(Value::as_str)
        .unwrap_or("generate");
    let input_payload = input.get("input_payload").cloned().unwrap_or(Value::Null);
    let _input_refs = input
        .get("input_refs")
        .cloned()
        .unwrap_or(serde_json::json!([]));

    // Produce deterministic output based on operation_kind
    let output_payload = deterministic_output(operation_kind, operation_id, &input_payload);

    Ok(serde_json::json!({
        "kind": "inference_local_invoke_result",
        "operation_id": operation_id,
        "operation_kind": operation_kind,
        "output_payload": output_payload,
        "output_refs": [],
        "transport_kind": transport_kind,
        "network_performed": false,
        "transport_performed": "in_memory_fake",
        "inference_performed": false,
        "executor_kind": "fake_local_deterministic",
        "normalized_error": Value::Null,
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

/// Deterministic output generator based on operation kind.
fn deterministic_output(operation_kind: &str, operation_id: &str, input_payload: &Value) -> Value {
    match operation_kind {
        "generate" => serde_json::json!({
            "text": format!("deterministic generate output for {}", operation_id),
            "tokens_estimated": 5,
            "finish_reason": "complete",
        }),
        "classify" => serde_json::json!({
            "label": "deterministic_class",
            "confidence": 0.95,
            "categories": ["class_a", "class_b"],
        }),
        "transform" => serde_json::json!({
            "transformed": format!("deterministic transform of {:?}", input_payload),
            "operations_applied": 1,
        }),
        _ => serde_json::json!({
            "text": format!("deterministic output for {} operation {}", operation_kind, operation_id),
            "finish_reason": "complete",
        }),
    }
}

// ---------------------------------------------------------------------------
// stream
// ---------------------------------------------------------------------------

fn stream(request: &InprocInvocation) -> anyhow::Result<Value> {
    let input = &request.input;

    let invocation_id = input
        .get("invocation_id")
        .and_then(Value::as_str)
        .unwrap_or("inv_local_stream_001")
        .to_string();

    let operation_kind = input
        .get("operation_kind")
        .and_then(Value::as_str)
        .unwrap_or("generate")
        .to_string();

    let transport_kind = input
        .get("transport_kind")
        .and_then(Value::as_str)
        .unwrap_or("in_memory");

    // Reject http transport for stream too
    if transport_kind == "http" {
        return Ok(serde_json::json!({
            "kind": "inference_local_stream_result",
            "frames": [],
            "terminal_frame_consistent": false,
            "diagnostics": [{"severity": "error", "field": "transport_kind", "message": "http transport is not supported for streaming"}],
            "network_performed": false,
            "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
        }));
    }

    // Reject HTTP/messages-shaped fields in stream too
    for field in HTTP_SHAPED_FIELDS
        .iter()
        .chain(MESSAGES_SHAPED_FIELDS.iter())
    {
        if input.get(*field).is_some() {
            return Ok(serde_json::json!({
                "kind": "inference_local_stream_result",
                "frames": [],
                "terminal_frame_consistent": false,
                "diagnostics": [{"severity": "error", "field": *field, "message": format!("field '{}' is not accepted in stream input", field)}],
                "network_performed": false,
                "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
            }));
        }
    }

    // Build deterministic stream frames: start -> chunk -> progress -> end
    let frames = vec![
        serde_json::json!({
            "kind": "start",
            "invocation_id": invocation_id,
            "sequence": 1,
            "payload": {
                "operation_kind": operation_kind,
                "transport_kind": transport_kind,
                "provider_type": "fake_local_deterministic",
            },
            "metadata": {"provider_type": "fake_local_deterministic"},
            "redaction_state": "clean",
        }),
        serde_json::json!({
            "kind": "chunk",
            "invocation_id": invocation_id,
            "sequence": 2,
            "payload": {"text_delta": "deterministic "},
            "metadata": {"provider_type": "fake_local_deterministic"},
            "redaction_state": "clean",
        }),
        serde_json::json!({
            "kind": "chunk",
            "invocation_id": invocation_id,
            "sequence": 3,
            "payload": {"text_delta": "local "},
            "metadata": {"provider_type": "fake_local_deterministic"},
            "redaction_state": "clean",
        }),
        serde_json::json!({
            "kind": "chunk",
            "invocation_id": invocation_id,
            "sequence": 4,
            "payload": {"text_delta": "inference "},
            "metadata": {"provider_type": "fake_local_deterministic"},
            "redaction_state": "clean",
        }),
        serde_json::json!({
            "kind": "chunk",
            "invocation_id": invocation_id,
            "sequence": 5,
            "payload": {"text_delta": "output"},
            "metadata": {"provider_type": "fake_local_deterministic"},
            "redaction_state": "clean",
        }),
        serde_json::json!({
            "kind": "progress",
            "invocation_id": invocation_id,
            "sequence": 6,
            "payload": {"tokens_estimated": 4, "progress_ratio": 1.0},
            "metadata": {"provider_type": "fake_local_deterministic"},
            "redaction_state": "clean",
        }),
        serde_json::json!({
            "kind": "end",
            "invocation_id": invocation_id,
            "sequence": 7,
            "payload": {"finish_reason": "complete", "auto_terminated": true},
            "metadata": {"provider_type": "fake_local_deterministic"},
            "redaction_state": "clean",
        }),
    ];

    Ok(serde_json::json!({
        "kind": "inference_local_stream_result",
        "frames": frames,
        "terminal_frame_consistent": true,
        "diagnostics": [],
        "network_performed": false,
        "inference_performed": false,
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

// ---------------------------------------------------------------------------
// explain_error
// ---------------------------------------------------------------------------

fn explain_error(request: &InprocInvocation) -> anyhow::Result<Value> {
    let error_kind = request
        .input
        .get("error_kind")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let (explanation, retryable, error_class) = match error_kind {
        "local_process_failed" => (
            "The local inference process encountered an unexpected failure.",
            false,
            "local",
        ),
        "local_resource_exhausted" => (
            "Local compute resources (CPU, memory) are insufficient for the requested operation.",
            true,
            "resource",
        ),
        "local_model_not_loaded" => (
            "No local model is loaded or available for inference.",
            false,
            "local",
        ),
        "local_inference_error" => (
            "The local inference engine produced an error during computation.",
            true,
            "local",
        ),
        "timeout" => (
            "The inference operation exceeded its deadline.",
            true,
            "resource",
        ),
        "cancelled" => (
            "The inference operation was cancelled by the caller.",
            false,
            "resource",
        ),
        "transport_rejected" => (
            "The requested transport kind is not supported by this local provider.",
            false,
            "local",
        ),
        "http_field_rejected" => (
            "HTTP-shaped fields are not accepted by this local provider.",
            false,
            "local",
        ),
        "messages_field_rejected" => (
            "Chat/messages-shaped fields are not accepted by this local provider.",
            false,
            "local",
        ),
        "secret_rejected" => (
            "Secret fields are not needed or accepted by this local provider.",
            false,
            "local",
        ),
        _ => ("Unknown local inference error.", false, "local"),
    };

    Ok(serde_json::json!({
        "kind": "inference_local_error_explanation",
        "error_kind": error_kind,
        "explanation": explanation,
        "retryable": retryable,
        "error_class": error_class,
        "network_performed": false,
        "inference_performed": false,
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check if input contains raw secret-looking fields.
fn looks_like_raw_secret_field(input: &Value) -> bool {
    let secret_field_names = [
        "api_key",
        "apiKey",
        "secret",
        "password",
        "token",
        "credential",
    ];
    if let Some(obj) = input.as_object() {
        for key in obj.keys() {
            if secret_field_names.contains(&key.as_str()) {
                if let Some(val) = obj.get(key) {
                    if let Some(s) = val.as_str() {
                        if looks_like_raw_secret_value(s) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Heuristic: value looks like a raw API key / secret.
fn looks_like_raw_secret_value(s: &str) -> bool {
    let s = s.trim();
    if s.starts_with("secret_ref:")
        || s.starts_with("secretRef:")
        || s.starts_with("secret-ref:")
        || s.starts_with("host:")
    {
        return false;
    }
    // Long alphanumeric strings likely to be raw keys
    if s.len() >= 20
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return true;
    }
    // Common API key prefixes
    if s.starts_with("sk-") || s.starts_with("sk_") || s.starts_with("key-") {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(cap_id: &str, input: Value) -> InprocInvocation {
        InprocInvocation {
            capability_id: cap_id.to_string(),
            provider_package_id: PACKAGE_ID.to_string(),
            session_id: None,
            input,
        }
    }

    #[test]
    fn describe_capabilities_returns_local_transports() {
        let req = make_request(
            "official/inference-local-lab/describe_capabilities",
            serde_json::json!({}),
        );
        let result = describe_capabilities(&req).unwrap();
        assert_eq!(result["kind"], "inference_local_capabilities");
        assert!(result["transport_kinds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t == "in_memory"));
        assert!(result["transport_kinds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t == "local_process"));
        assert_eq!(result["network_required"], false);
        assert_eq!(result["secrets_required"], false);
        assert_eq!(result["streaming_supported"], true);
        let ops = result["operation_kinds"].as_array().unwrap();
        assert!(ops.iter().any(|o| o == "generate"));
        assert!(ops.iter().any(|o| o == "classify"));
        assert!(ops.iter().any(|o| o == "transform"));
    }

    #[test]
    fn invoke_rejects_http_transport() {
        let req = make_request(
            "official/inference-local-lab/invoke",
            serde_json::json!({"transport_kind": "http", "operation_kind": "generate"}),
        );
        let result = invoke(&req).unwrap();
        assert_eq!(
            result["normalized_error"]["error_kind"],
            "transport_rejected"
        );
    }

    #[test]
    fn invoke_rejects_url_field() {
        let req = make_request(
            "official/inference-local-lab/invoke",
            serde_json::json!({"url": "https://api.example.com", "operation_kind": "generate"}),
        );
        let result = invoke(&req).unwrap();
        assert_eq!(
            result["normalized_error"]["error_kind"],
            "http_field_rejected"
        );
    }

    #[test]
    fn invoke_rejects_messages_field() {
        let req = make_request(
            "official/inference-local-lab/invoke",
            serde_json::json!({"messages": [{"role": "user", "content": "hi"}], "operation_kind": "generate"}),
        );
        let result = invoke(&req).unwrap();
        assert_eq!(
            result["normalized_error"]["error_kind"],
            "messages_field_rejected"
        );
    }

    #[test]
    fn invoke_succeeds_without_http_fields() {
        let req = make_request(
            "official/inference-local-lab/invoke",
            serde_json::json!({"operation_kind": "generate", "operation_id": "op_001"}),
        );
        let result = invoke(&req).unwrap();
        assert_eq!(result["kind"], "inference_local_invoke_result");
        assert_eq!(result["network_performed"], false);
        assert_eq!(result["transport_performed"], "in_memory_fake");
        assert!(result["normalized_error"].is_null());
    }

    #[test]
    fn invoke_rejects_raw_secret() {
        let req = make_request(
            "official/inference-local-lab/invoke",
            serde_json::json!({"operation_kind": "generate", "api_key": "rawSecretPlaceholder1234567890ABCDEF"}),
        );
        let result = invoke(&req).unwrap();
        assert_eq!(result["normalized_error"]["error_kind"], "secret_rejected");
    }

    #[test]
    fn stream_emits_deterministic_frames() {
        let req = make_request(
            "official/inference-local-lab/stream",
            serde_json::json!({"invocation_id": "inv_test", "operation_kind": "generate"}),
        );
        let result = stream(&req).unwrap();
        let frames = result["frames"].as_array().unwrap();
        let kinds: Vec<&str> = frames
            .iter()
            .map(|f| f["kind"].as_str().unwrap_or_default())
            .collect();
        assert_eq!(kinds[0], "start");
        assert!(kinds.contains(&"chunk"));
        assert!(kinds.contains(&"progress"));
        assert_eq!(kinds[kinds.len() - 1], "end");
        assert_eq!(result["terminal_frame_consistent"], true);
        assert_eq!(result["network_performed"], false);
    }

    #[test]
    fn explain_error_covers_local_classes() {
        for error_kind in &[
            "local_process_failed",
            "local_resource_exhausted",
            "local_model_not_loaded",
            "local_inference_error",
            "timeout",
            "cancelled",
        ] {
            let req = make_request(
                "official/inference-local-lab/explain_error",
                serde_json::json!({"error_kind": error_kind}),
            );
            let result = explain_error(&req).unwrap();
            assert_eq!(result["kind"], "inference_local_error_explanation");
            assert!(result["explanation"].is_string());
            assert!(result["error_class"] == "local" || result["error_class"] == "resource");
        }
    }
}
