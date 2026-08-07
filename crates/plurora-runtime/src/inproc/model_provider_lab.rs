//! Handler for `official/model-provider-lab` capabilities.
//!
//! No-network, no-inference provider family metadata, profile validation,
//! request normalization, fake/local invoke, stream normalization, and error
//! explanation across eight families: openai, anthropic, gemini,
//! openai_compatible, openrouter, deepseek, xai, fireworks.
//!
//! M4 adds `invoke` for openai, anthropic, gemini.
//! M5 extends `invoke` to openai_compatible, openrouter, deepseek, xai, fireworks.
//! M6 adds `normalize_stream` — normalizes provider stream events to
//! StreamFrameEnvelope-like frames (start/chunk/progress/end/error/cancelled/timeout).
//! The invoke handler produces a fake/local result with an auditable
//! `outbound_request_shape` but performs no real network I/O.

use serde_json::Value;

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/model-provider-lab";

const SUPPORTED_FAMILIES: &[&str] = &[
    "openai",
    "anthropic",
    "gemini",
    "openai_compatible",
    "openrouter",
    "deepseek",
    "xai",
    "fireworks",
];

const INVOKE_FAMILIES: &[&str] = &[
    "openai",
    "anthropic",
    "gemini",
    "openai_compatible",
    "openrouter",
    "deepseek",
    "xai",
    "fireworks",
];

// ---------------------------------------------------------------------------
// Normalized request shape (shared between normalize_request and invoke)
// ---------------------------------------------------------------------------

struct NormalizedShape {
    endpoint: String,
    request_dialect: String,
    stream_family: String,
    headers: Value,
    body_shape: Value,
}

// ---------------------------------------------------------------------------
// Top-level dispatch
// ---------------------------------------------------------------------------

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }
    let id = request.capability_id.as_str();
    if id.ends_with("/list_supported_families") {
        Some(list_supported_families(request))
    } else if id.ends_with("/validate_profile") {
        Some(validate_profile(request))
    } else if id.ends_with("/normalize_request") {
        Some(normalize_request(request))
    } else if id.ends_with("/explain_error") {
        Some(explain_error(request))
    } else if id.ends_with("/echo") {
        Some(Ok(request.input.clone()))
    } else if id.ends_with("/invoke") {
        Some(invoke(request))
    } else if id.ends_with("/normalize_stream") {
        Some(normalize_stream(request))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// list_supported_families
// ---------------------------------------------------------------------------

fn list_supported_families(request: &InprocInvocation) -> anyhow::Result<Value> {
    let families = serde_json::json!([
        {
            "id": "openai",
            "request_dialect": "openai_responses",
            "stream_family": "semantic_sse",
            "default_base_url": "https://api.openai.com",
            "auth_scheme": "bearer_secret_ref",
            "tool_modes": ["functions", "built_in_tools"],
            "usage_modes": ["top_level", "final_chunk"],
            "network_performed": false
        },
        {
            "id": "anthropic",
            "request_dialect": "anthropic_messages",
            "stream_family": "semantic_sse",
            "default_base_url": "https://api.anthropic.com",
            "auth_scheme": "x-api-key_secret_ref",
            "tool_modes": ["tool_use"],
            "usage_modes": ["cumulative_delta"],
            "network_performed": false
        },
        {
            "id": "gemini",
            "request_dialect": "gemini_generate_content",
            "stream_family": "typed_chunk_stream",
            "default_base_url": "https://generativelanguage.googleapis.com",
            "auth_scheme": "x-goog-api-key_secret_ref",
            "tool_modes": ["functions", "code_execution"],
            "usage_modes": ["usage_metadata"],
            "network_performed": false
        },
        {
            "id": "openai_compatible",
            "request_dialect": "openai_chat",
            "stream_family": "delta_sse",
            "default_base_url": null,
            "auth_scheme": "bearer_secret_ref",
            "tool_modes": ["functions"],
            "usage_modes": ["final_chunk", "top_level"],
            "network_performed": false
        },
        {
            "id": "openrouter",
            "request_dialect": "stateless_responses",
            "stream_family": "semantic_sse",
            "default_base_url": "https://openrouter.ai/api/v1",
            "auth_scheme": "bearer_secret_ref",
            "tool_modes": ["functions", "web_search"],
            "usage_modes": ["top_level"],
            "network_performed": false
        },
        {
            "id": "deepseek",
            "request_dialect": "openai_chat",
            "stream_family": "delta_sse",
            "default_base_url": "https://api.deepseek.com",
            "auth_scheme": "bearer_secret_ref",
            "tool_modes": ["functions"],
            "usage_modes": ["top_level", "final_chunk"],
            "network_performed": false
        },
        {
            "id": "xai",
            "request_dialect": "openai_responses",
            "stream_family": "semantic_sse",
            "default_base_url": "https://api.x.ai",
            "auth_scheme": "bearer_secret_ref",
            "tool_modes": ["functions", "web_search"],
            "usage_modes": ["top_level"],
            "network_performed": false
        },
        {
            "id": "fireworks",
            "request_dialect": "openai_chat",
            "stream_family": "delta_sse",
            "default_base_url": "https://api.fireworks.ai/inference/v1",
            "auth_scheme": "bearer_secret_ref",
            "tool_modes": ["functions", "mcp"],
            "usage_modes": ["top_level", "final_chunk"],
            "network_performed": false
        }
    ]);
    Ok(serde_json::json!({
        "kind": "model_provider_families",
        "families": families,
        "network_performed": false,
        "inference_performed": false,
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

// ---------------------------------------------------------------------------
// validate_profile
// ---------------------------------------------------------------------------

fn validate_profile(request: &InprocInvocation) -> anyhow::Result<Value> {
    let profile = request.input.get("profile");
    let family = profile
        .and_then(|p| p.get("family"))
        .or_else(|| request.input.get("family"))
        .and_then(Value::as_str)
        .unwrap_or_default();

    let credential = profile
        .and_then(|p| p.get("credential"))
        .or_else(|| request.input.get("credential"))
        .and_then(Value::as_str)
        .unwrap_or_default();

    let base_url = profile
        .and_then(|p| p.get("baseUrl"))
        .or_else(|| request.input.get("base_url"))
        .and_then(Value::as_str)
        .unwrap_or_default();

    let headers = profile
        .and_then(|p| p.get("headers"))
        .or_else(|| request.input.get("headers"));

    let mut diagnostics = Vec::new();
    let mut network_required_hosts: Vec<String> = Vec::new();
    let mut secret_refs: Vec<String> = Vec::new();

    if !SUPPORTED_FAMILIES.contains(&family) {
        diagnostics.push(serde_json::json!({
            "severity": "error",
            "field": "family",
            "message": format!("unsupported provider family: '{}'", family)
        }));
    }

    let has_raw_secret = request.input.get("api_key").is_some()
        || request.input.get("secret").is_some()
        || looks_like_raw_secret(credential);

    if has_raw_secret {
        diagnostics.push(serde_json::json!({
            "severity": "error",
            "field": "credential",
            "message": "raw secrets are not accepted; use secret_ref: or host: reference"
        }));
    } else if !is_valid_secret_ref(credential) && !credential.is_empty() {
        diagnostics.push(serde_json::json!({
            "severity": "error",
            "field": "credential",
            "message": "credential must be a secret_ref: or host: reference"
        }));
    } else if !credential.is_empty() {
        secret_refs.push(credential.to_string());
    }

    if family == "openai_compatible" {
        if base_url.is_empty() {
            diagnostics.push(serde_json::json!({
                "severity": "error",
                "field": "base_url",
                "message": "openai_compatible requires a base_url"
            }));
        } else if !base_url.starts_with("https://") {
            diagnostics.push(serde_json::json!({
                "severity": "error",
                "field": "base_url",
                "message": "openai_compatible requires HTTPS base_url"
            }));
        }
    }

    if !base_url.is_empty() && !base_url.starts_with("https://") && !base_url.starts_with("http://")
    {
        diagnostics.push(serde_json::json!({
            "severity": "error",
            "field": "base_url",
            "message": "base_url must use HTTPS"
        }));
    }

    if family == "openrouter" {
        let has_referer = headers_value(headers, "HTTP-Referer")
            .or_else(|| headers_value(headers, "http-referer"))
            .is_some();
        if !has_referer {
            diagnostics.push(serde_json::json!({
                "severity": "warning",
                "field": "headers",
                "message": "OpenRouter recommends setting HTTP-Referer header for attribution"
            }));
        }
        let has_title = headers_value(headers, "X-OpenRouter-Title")
            .or_else(|| headers_value(headers, "x-openrouter-title"))
            .is_some();
        if !has_title {
            diagnostics.push(serde_json::json!({
                "severity": "info",
                "field": "headers",
                "message": "OpenRouter supports X-OpenRouter-Title header for request labeling"
            }));
        }
    }

    if family == "anthropic" {
        let has_version = headers_value(headers, "anthropic-version").is_some();
        if !has_version {
            diagnostics.push(serde_json::json!({
                "severity": "warning",
                "field": "headers",
                "message": "Anthropic requires the anthropic-version header (e.g. '2023-06-01')"
            }));
        }
    }

    if family == "gemini" {
        let has_api_key_header = headers_value(headers, "x-goog-api-key")
            .or_else(|| headers_value(headers, "X-Goog-Api-Key"))
            .is_some();
        if !has_api_key_header {
            diagnostics.push(serde_json::json!({
                "severity": "info",
                "field": "headers",
                "message": "Gemini uses x-goog-api-key header for authentication; the adapter will include it from the credential ref"
            }));
        }
    }

    if let Some(headers_obj) = headers.and_then(Value::as_object) {
        for (key, value) in headers_obj {
            if let Some(s) = value.as_str() {
                if looks_like_raw_secret(s) {
                    diagnostics.push(serde_json::json!({
                        "severity": "error",
                        "field": format!("headers.{}", key),
                        "message": format!("header '{}' contains a raw secret; use secret_ref: or host: reference", key)
                    }));
                }
            }
        }
    }

    if !base_url.is_empty() {
        if let Some(host) = extract_host(base_url) {
            network_required_hosts.push(host);
        }
    } else if let Some(default_url) = default_base_url_for_family(family) {
        if let Some(host) = extract_host(default_url) {
            network_required_hosts.push(host);
        }
    }

    let valid = !diagnostics
        .iter()
        .any(|d| d.get("severity").and_then(Value::as_str) == Some("error"));

    Ok(serde_json::json!({
        "kind": "model_provider_profile_validation",
        "valid": valid,
        "diagnostics": diagnostics,
        "network_required": network_required_hosts,
        "secret_refs": secret_refs,
        "network_performed": false,
        "inference_performed": false,
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

// ---------------------------------------------------------------------------
// normalize_request
// ---------------------------------------------------------------------------

fn normalize_request(request: &InprocInvocation) -> anyhow::Result<Value> {
    let params = extract_profile_params(request);
    let shape = build_normalized_shape(request, &params)?;

    Ok(serde_json::json!({
        "kind": "model_provider_normalized_request",
        "family": params.family,
        "method": "POST",
        "endpoint": shape.endpoint,
        "request_dialect": shape.request_dialect,
        "stream_family": shape.stream_family,
        "headers": shape.headers,
        "body_shape": shape.body_shape,
        "credential_ref": credential_ref_placeholder(&params.credential),
        "provider_options_namespaced": true,
        "network_performed": false,
        "inference_performed": false,
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

// ---------------------------------------------------------------------------
// invoke (M4+M5 — all eight families)
// ---------------------------------------------------------------------------

fn invoke(request: &InprocInvocation) -> anyhow::Result<Value> {
    let params = extract_profile_params(request);
    let profile = request.input.get("profile");

    // Reject raw credentials
    if looks_like_raw_secret(&params.credential)
        || request.input.get("api_key").is_some()
        || request.input.get("secret").is_some()
        || headers_contain_raw_secret(
            profile
                .and_then(|p| p.get("headers"))
                .or_else(|| request.input.get("headers")),
        )
    {
        return Ok(serde_json::json!({
            "kind": "model_provider_invoke_result",
            "family": params.family,
            "normalized_error": {
                "error_kind": "secret_unavailable",
                "message": "raw secrets are not accepted; use secret_ref: or host: reference",
                "retryable": false,
            },
            "network_performed": false,
            "inference_performed": false,
            "executor_kind": "fake_local",
            "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
        }));
    }

    // openai_compatible requires an explicit HTTPS base_url (no default)
    if params.family == "openai_compatible" && params.base_url.is_empty() {
        return Ok(serde_json::json!({
            "kind": "model_provider_invoke_result",
            "family": params.family,
            "normalized_error": {
                "error_kind": "bad_request",
                "message": "openai_compatible requires an explicit base_url for invoke",
                "retryable": false,
            },
            "network_performed": false,
            "inference_performed": false,
            "executor_kind": "fake_local",
            "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
        }));
    }

    if !params.base_url.starts_with("https://") {
        return Ok(serde_json::json!({
            "kind": "model_provider_invoke_result",
            "family": params.family,
            "normalized_error": {
                "error_kind": "network_denied",
                "message": "invoke requires an HTTPS base_url",
                "retryable": false,
            },
            "network_performed": false,
            "inference_performed": false,
            "executor_kind": "fake_local",
            "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
        }));
    }

    // Only openai, anthropic, gemini supported for M4
    if !INVOKE_FAMILIES.contains(&params.family.as_str()) {
        return Ok(serde_json::json!({
            "kind": "model_provider_invoke_result",
            "family": params.family,
            "normalized_error": {
                "error_kind": "bad_request",
                "message": format!("invoke not implemented for family '{}'; supported: openai, anthropic, gemini, openai_compatible, openrouter, deepseek, xai, fireworks", params.family),
                "retryable": false,
            },
            "network_performed": false,
            "inference_performed": false,
            "executor_kind": "fake_local",
            "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
        }));
    }

    let shape = build_normalized_shape(request, &params)?;

    // Build outbound_request_shape
    let (destination_host, path) = split_endpoint(&shape.endpoint);
    let secret_refs = if is_valid_secret_ref(&params.credential) {
        vec![params.credential.to_string()]
    } else {
        vec![]
    };

    let outbound_request_shape = serde_json::json!({
        "destination_host": destination_host,
        "method": "POST",
        "path": path,
        "secret_refs": secret_refs,
        "body_shape": shape.body_shape,
        "redaction_state": "redacted"
    });

    // Build fake response per family
    let fake_response = build_fake_response(&params, &shape);

    Ok(serde_json::json!({
        "kind": "model_provider_invoke_result",
        "family": params.family,
        "request_dialect": shape.request_dialect,
        "stream_family": shape.stream_family,
        "endpoint": shape.endpoint,
        "method": "POST",
        "outbound_request_shape": outbound_request_shape,
        "response": fake_response,
        "normalized_error": Value::Null,
        "network_performed": false,
        "inference_performed": false,
        "executor_kind": "fake_local",
        "live_call_supported": false,
        "manual_live_call_requires": [
            "public outbound package path",
            "secret_ref",
            "network allowlist",
            "redacted audit"
        ],
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

fn build_fake_response(params: &ProfileParams, shape: &NormalizedShape) -> Value {
    match params.family.as_str() {
        "openai" => build_openai_fake_response(params, shape),
        "anthropic" => build_anthropic_fake_response(params),
        "gemini" => build_gemini_fake_response(params),
        "openai_compatible" => build_openai_compatible_fake_response(params, shape),
        "openrouter" => build_openrouter_fake_response(params, shape),
        "deepseek" => build_deepseek_fake_response(params, shape),
        "xai" => build_xai_fake_response(params, shape),
        "fireworks" => build_fireworks_fake_response(params, shape),
        _ => serde_json::json!({}),
    }
}

fn build_openai_fake_response(params: &ProfileParams, shape: &NormalizedShape) -> Value {
    let is_responses = shape.request_dialect == "openai_responses";
    if is_responses {
        serde_json::json!({
            "id": "resp_fake_001",
            "object": "response",
            "model": params.model,
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {"type": "output_text", "text": "fake local response"}
                    ]
                }
            ],
            "stop_reason": "complete",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
                "total_tokens": 15
            },
            "provider_request_id": "req_fake_openai_001"
        })
    } else {
        serde_json::json!({
            "id": "chatcmpl-fake-001",
            "object": "chat.completion",
            "model": params.model,
            "choices": [
                {
                    "index": 0,
                    "message": {"role": "assistant", "content": "fake local response"},
                    "finish_reason": "stop"
                }
            ],
            "stop_reason": "stop",
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            },
            "provider_request_id": "req_fake_openai_001"
        })
    }
}

fn build_anthropic_fake_response(params: &ProfileParams) -> Value {
    serde_json::json!({
        "id": "msg_fake_001",
        "type": "message",
        "role": "assistant",
        "model": params.model,
        "content": [
            {"type": "text", "text": "fake local response"}
        ],
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": 10,
            "output_tokens": 5
        },
        "provider_request_id": "req_fake_anthropic_001"
    })
}

fn build_gemini_fake_response(_params: &ProfileParams) -> Value {
    serde_json::json!({
        "candidates": [
            {
                "content": {
                    "parts": [{"text": "fake local response"}],
                    "role": "model"
                },
                "finishReason": "STOP",
                "index": 0
            }
        ],
        "usageMetadata": {
            "promptTokenCount": 10,
            "candidatesTokenCount": 5,
            "totalTokenCount": 15
        },
        "provider_request_id": "req_fake_gemini_001"
    })
}

fn build_openai_compatible_fake_response(params: &ProfileParams, shape: &NormalizedShape) -> Value {
    // OpenAI-compatible always uses openai_chat dialect (no responses branch)
    let _ = shape; // openai_compatible only has chat dialect
    serde_json::json!({
        "id": "chatcmpl-fake-compat-001",
        "object": "chat.completion",
        "model": params.model,
        "choices": [
            {
                "index": 0,
                "message": {"role": "assistant", "content": "fake local response"},
                "finish_reason": "stop"
            }
        ],
        "stop_reason": "stop",
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        },
        "provider_request_id": "req_fake_openai_compatible_001"
    })
}

fn build_openrouter_fake_response(params: &ProfileParams, shape: &NormalizedShape) -> Value {
    if shape.request_dialect == "stateless_responses" {
        serde_json::json!({
            "id": "resp_fake_or_001",
            "object": "response",
            "model": params.model,
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {"type": "output_text", "text": "fake local response"}
                    ]
                }
            ],
            "stop_reason": "complete",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
                "total_tokens": 15
            },
            "provider_request_id": "req_fake_openrouter_001"
        })
    } else {
        serde_json::json!({
            "id": "gen-fake-or-001",
            "object": "chat.completion",
            "model": params.model,
            "choices": [
                {
                    "index": 0,
                    "message": {"role": "assistant", "content": "fake local response"},
                    "finish_reason": "stop"
                }
            ],
            "stop_reason": "stop",
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            },
            "provider_request_id": "req_fake_openrouter_001"
        })
    }
}

fn build_deepseek_fake_response(params: &ProfileParams, shape: &NormalizedShape) -> Value {
    // DeepSeek uses openai_chat dialect only (no responses branch)
    let _ = shape;
    serde_json::json!({
        "id": "chatcmpl-fake-ds-001",
        "object": "chat.completion",
        "model": params.model,
        "choices": [
            {
                "index": 0,
                "message": {"role": "assistant", "content": "fake local response"},
                "finish_reason": "stop",
                "reasoning_content": null
            }
        ],
        "stop_reason": "stop",
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15,
            "prompt_cache_hit_tokens": 0,
            "prompt_cache_miss_tokens": 10
        },
        "provider_request_id": "req_fake_deepseek_001"
    })
}

fn build_xai_fake_response(params: &ProfileParams, shape: &NormalizedShape) -> Value {
    if shape.request_dialect == "openai_responses" {
        serde_json::json!({
            "id": "resp_fake_xai_001",
            "object": "response",
            "model": params.model,
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {"type": "output_text", "text": "fake local response"}
                    ]
                }
            ],
            "stop_reason": "complete",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
                "total_tokens": 15
            },
            "provider_request_id": "req_fake_xai_001"
        })
    } else {
        serde_json::json!({
            "id": "chatcmpl-fake-xai-001",
            "object": "chat.completion",
            "model": params.model,
            "choices": [
                {
                    "index": 0,
                    "message": {"role": "assistant", "content": "fake local response"},
                    "finish_reason": "stop"
                }
            ],
            "stop_reason": "stop",
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            },
            "provider_request_id": "req_fake_xai_001"
        })
    }
}

fn build_fireworks_fake_response(params: &ProfileParams, shape: &NormalizedShape) -> Value {
    if shape.request_dialect == "fireworks_responses" {
        serde_json::json!({
            "id": "resp_fake_fw_001",
            "object": "response",
            "model": params.model,
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {"type": "output_text", "text": "fake local response"}
                    ]
                }
            ],
            "stop_reason": "complete",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
                "total_tokens": 15
            },
            "provider_request_id": "req_fake_fireworks_001"
        })
    } else {
        serde_json::json!({
            "id": "chatcmpl-fake-fw-001",
            "object": "chat.completion",
            "model": params.model,
            "choices": [
                {
                    "index": 0,
                    "message": {"role": "assistant", "content": "fake local response"},
                    "finish_reason": "stop"
                }
            ],
            "stop_reason": "stop",
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            },
            "provider_request_id": "req_fake_fireworks_001"
        })
    }
}

// ---------------------------------------------------------------------------
// normalize_stream (M6 — all eight families)
// ---------------------------------------------------------------------------

fn normalize_stream(request: &InprocInvocation) -> anyhow::Result<Value> {
    let family = request
        .input
        .get("family")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    if !SUPPORTED_FAMILIES.contains(&family.as_str()) {
        return Ok(serde_json::json!({
            "kind": "model_provider_stream_normalization",
            "family": family,
            "stream_family": "unknown",
            "frames": [],
            "terminal_frame_consistent": false,
            "diagnostics": [{"severity": "error", "field": "family", "message": format!("unsupported provider family: '{}'", family)}],
            "network_performed": false,
            "inference_performed": false,
            "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
        }));
    }

    let stream_family = default_stream_family_for_family(&family);
    let invocation_id = request
        .input
        .get("invocation_id")
        .and_then(Value::as_str)
        .unwrap_or("inv_stream_001")
        .to_string();

    // If sample_provider_events are provided, normalize them; otherwise produce fake samples
    let sample_events = request.input.get("sample_provider_events");
    let frames = if let Some(events) = sample_events.and_then(Value::as_array) {
        normalize_provider_events(&family, &stream_family, &invocation_id, events)
    } else {
        build_fake_stream_frames(&family, &stream_family, &invocation_id)
    };

    // Check terminal consistency: must have at least one start frame and one
    // terminal frame (end/error/cancelled/timeout), with start first.
    let has_start = frames.iter().any(|f| f["kind"] == "start");
    let terminal_kinds = ["end", "error", "cancelled", "timeout"];
    let has_terminal = frames
        .iter()
        .any(|f| terminal_kinds.contains(&f["kind"].as_str().unwrap_or_default()));
    let terminal_frame_consistent = has_start && has_terminal;

    // Check for raw secrets in input (diagnostics only, not rejection)
    let mut diagnostics = Vec::new();
    let credential = request
        .input
        .get("credential")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if looks_like_raw_secret(credential) {
        diagnostics.push(serde_json::json!({
            "severity": "warning",
            "field": "credential",
            "message": "raw credential detected; stream normalization does not echo it, but callers should use secret_ref"
        }));
    }

    Ok(serde_json::json!({
        "kind": "model_provider_stream_normalization",
        "family": family,
        "stream_family": stream_family,
        "frames": frames,
        "terminal_frame_consistent": terminal_frame_consistent,
        "diagnostics": diagnostics,
        "network_performed": false,
        "inference_performed": false,
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

/// Normalize provider-specific stream events into StreamFrameEnvelope-like frames.
///
/// Always emits a `start` frame as the first frame (before processing provider events)
/// since provider events typically don't include an explicit stream-open event
/// (except Anthropic's `message_start`).
fn normalize_provider_events(
    family: &str,
    stream_family: &str,
    invocation_id: &str,
    events: &[Value],
) -> Vec<Value> {
    let mut frames = Vec::new();
    let mut seq = 0u64;

    // Always emit a start frame unless the first provider event is already a start
    // (Anthropic message_start is detected as start below, but we still add our own
    // canonical start before processing any provider events).
    seq += 1;
    frames.push(serde_json::json!({
        "kind": "start",
        "invocation_id": invocation_id,
        "sequence": seq,
        "payload": {
            "provider_family": family,
            "stream_family": stream_family,
            "event_count": events.len(),
        },
        "metadata": {"provider_family": family, "stream_family": stream_family},
        "redaction_state": "redacted",
    }));

    for event in events {
        seq += 1;

        // L6: Handle SSE keep-alive comments (e.g. ": keep-alive" or ": ").
        // These are empty events used by some providers (DeepSeek, OpenRouter)
        // to prevent connection timeout. They produce a progress heartbeat.
        if let Some(s) = event.as_str() {
            if s.starts_with(':') {
                frames.push(serde_json::json!({
                    "kind": "progress",
                    "invocation_id": invocation_id,
                    "sequence": seq,
                    "payload": {"provider_event": "keep_alive_comment", "comment": s},
                    "metadata": {"provider_family": family, "stream_family": stream_family},
                    "redaction_state": "redacted",
                }));
                continue;
            }
        }

        // L6: Handle mid-stream error events (OpenRouter, DeepSeek).
        // Some providers return error objects after HTTP 200 in the SSE stream.
        if let Some(error_obj) = event.get("error") {
            let error_message = error_obj
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("provider mid-stream error");
            let error_code = error_obj
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            frames.push(serde_json::json!({
                "kind": "error",
                "invocation_id": invocation_id,
                "sequence": seq,
                "payload": {
                    "provider_event": "mid_stream_error",
                    "error_kind": error_code,
                    "message": error_message,
                },
                "metadata": {"provider_family": family, "stream_family": stream_family},
                "redaction_state": "redacted",
            }));
            continue;
        }

        // Try to extract text content and event type from provider-specific shapes
        let (frame_kind, payload) = match family {
            "openai" | "openai_compatible" => normalize_delta_sse_event(event, &mut seq),
            "deepseek" => normalize_deepseek_event(event, &mut seq),
            "fireworks" => normalize_fireworks_event(event, &mut seq),
            "anthropic" => normalize_semantic_sse_event(event, &mut seq),
            "gemini" => normalize_typed_chunk_event(event, &mut seq),
            "openrouter" => {
                // OpenRouter can use either delta_sse or responses; check event shape
                if event.get("choices").is_some() {
                    normalize_delta_sse_event(event, &mut seq)
                } else if event.get("type").is_some() {
                    normalize_semantic_sse_event(event, &mut seq)
                } else if event.get("candidates").is_some() {
                    normalize_typed_chunk_event(event, &mut seq)
                } else if event.get("error").is_some() {
                    // Already handled above, but as fallback
                    (
                        "error".to_string(),
                        serde_json::json!({"provider_event": "mid_stream_error"}),
                    )
                } else {
                    ("chunk".to_string(), event.clone())
                }
            }
            "xai" => {
                // xAI uses openai_chat/responses shapes with possible reasoning fields
                if event.get("choices").is_some() || event.get("event").is_some() {
                    normalize_xai_event(event, &mut seq)
                } else {
                    ("chunk".to_string(), event.clone())
                }
            }
            _ => ("chunk".to_string(), event.clone()),
        };

        frames.push(serde_json::json!({
            "kind": frame_kind,
            "invocation_id": invocation_id,
            "sequence": seq,
            "payload": payload,
            "metadata": {
                "provider_family": family,
                "stream_family": stream_family,
            },
            "redaction_state": "redacted",
        }));
    }

    // Ensure there is a terminal frame if the last provider event didn't produce one
    let last_kind = frames
        .last()
        .and_then(|f| f["kind"].as_str())
        .unwrap_or_default();
    let terminal_kinds = ["end", "error", "cancelled", "timeout"];
    if !terminal_kinds.contains(&last_kind) {
        seq += 1;
        frames.push(serde_json::json!({
            "kind": "end",
            "invocation_id": invocation_id,
            "sequence": seq,
            "payload": {"finish_reason": "stop", "auto_terminated": true},
            "metadata": {"provider_family": family, "stream_family": stream_family},
            "redaction_state": "redacted",
        }));
    }

    frames
}

/// Normalize an OpenAI-style delta SSE event.
fn normalize_delta_sse_event(event: &Value, seq: &mut u64) -> (String, Value) {
    // Check for [DONE] marker
    if event.as_str() == Some("[DONE]") {
        return (
            "end".to_string(),
            serde_json::json!({"finish_reason": "stop", "marker": "[DONE]"}),
        );
    }

    // Check for finish_reason in choices
    if let Some(choices) = event.get("choices").and_then(Value::as_array) {
        for choice in choices {
            if let Some(fr) = choice.get("finish_reason").and_then(Value::as_str) {
                if !fr.is_empty() {
                    return ("end".to_string(), serde_json::json!({"finish_reason": fr}));
                }
            }
            // Delta content
            if let Some(delta) = choice.get("delta") {
                if let Some(content) = delta.get("content").and_then(Value::as_str) {
                    if !content.is_empty() {
                        return (
                            "chunk".to_string(),
                            serde_json::json!({"text_delta": content}),
                        );
                    }
                }
            }
        }
    }

    // Check for Responses API event field
    if let Some(evt_type) = event.get("event").and_then(Value::as_str) {
        if evt_type.contains("completed") || evt_type.contains("done") {
            return (
                "end".to_string(),
                serde_json::json!({"finish_reason": "complete", "event": evt_type}),
            );
        }
        if let Some(delta) = event
            .get("data")
            .and_then(|d| d.get("delta"))
            .and_then(Value::as_str)
        {
            return (
                "chunk".to_string(),
                serde_json::json!({"text_delta": delta, "event": evt_type}),
            );
        }
    }

    // Usage in final chunk
    if event.get("usage").is_some() {
        *seq += 1;
        return (
            "progress".to_string(),
            serde_json::json!({"usage_present": true}),
        );
    }

    ("chunk".to_string(), event.clone())
}

// ---------------------------------------------------------------------------
// L6: Provider-specific event normalizers for DeepSeek / xAI / Fireworks
// ---------------------------------------------------------------------------

/// Normalize a DeepSeek-style delta SSE event with quirks:
/// - `reasoning_content` in delta (thinking/reasoning output)
/// - Final usage chunk with `prompt_cache_hit_tokens` / `prompt_cache_miss_tokens`
/// - Keep-alive comments handled upstream (before this function)
/// - Mid-stream errors handled upstream
fn normalize_deepseek_event(event: &Value, seq: &mut u64) -> (String, Value) {
    // Check for [DONE] marker
    if event.as_str() == Some("[DONE]") {
        return (
            "end".to_string(),
            serde_json::json!({"finish_reason": "stop", "marker": "[DONE]"}),
        );
    }

    // L6: DeepSeek final usage chunk (often after [DONE] or as a standalone chunk)
    // Check usage FIRST because DeepSeek may send finish_reason AND usage in
    // the same chunk — we need to produce both an end frame and a progress frame.
    if event.get("usage").is_some() {
        let usage = &event["usage"];
        let has_cache = usage.get("prompt_cache_hit_tokens").is_some()
            || usage.get("prompt_cache_miss_tokens").is_some();
        *seq += 1;
        // If there's also a finish_reason in choices, emit both an end and progress.
        // The caller (normalize_provider_events) will emit both frames.
        if let Some(choices) = event.get("choices").and_then(Value::as_array) {
            for choice in choices {
                if let Some(fr) = choice.get("finish_reason").and_then(Value::as_str) {
                    if !fr.is_empty() {
                        // Return a combined payload; the caller needs to split
                        // We return "end" with usage info, and the caller can
                        // insert a progress frame before the end frame.
                        // Actually, normalize_provider_events only uses one return.
                        // So we return "progress" with usage and let the end be
                        // auto-appended by the terminal-frame check.
                        // But that would lose the finish_reason. Instead, let's
                        // return "end" and include usage_present so the auto-
                        // terminal check can be adjusted.
                        // Simplest fix: return "progress" for the usage, and
                        // let the auto-terminal append handle the end.
                        return (
                            "progress".to_string(),
                            serde_json::json!({
                                "usage_present": true,
                                "finish_reason": fr,
                                "provider_quirk": if has_cache { Some("deepseek_cache_usage") } else { None },
                            }),
                        );
                    }
                }
            }
        }
        return (
            "progress".to_string(),
            serde_json::json!({
                "usage_present": true,
                "provider_quirk": if has_cache { Some("deepseek_cache_usage") } else { None },
            }),
        );
    }

    // Check for finish_reason in choices (without usage)
    if let Some(choices) = event.get("choices").and_then(Value::as_array) {
        for choice in choices {
            if let Some(fr) = choice.get("finish_reason").and_then(Value::as_str) {
                if !fr.is_empty() {
                    return ("end".to_string(), serde_json::json!({"finish_reason": fr}));
                }
            }
            // L6: DeepSeek reasoning_content — emitted in delta alongside content
            if let Some(delta) = choice.get("delta") {
                if let Some(reasoning) = delta.get("reasoning_content").and_then(Value::as_str) {
                    if !reasoning.is_empty() {
                        return (
                            "chunk".to_string(),
                            serde_json::json!({
                                "reasoning_delta": reasoning,
                                "provider_quirk": "deepseek_reasoning_content",
                            }),
                        );
                    }
                }
                // Regular content delta
                if let Some(content) = delta.get("content").and_then(Value::as_str) {
                    if !content.is_empty() {
                        return (
                            "chunk".to_string(),
                            serde_json::json!({"text_delta": content}),
                        );
                    }
                }
            }
        }
    }

    ("chunk".to_string(), event.clone())
}

/// Normalize an xAI-style event with quirks:
/// - `reasoning_content` in choices[].delta (Grok reasoning output)
/// - `max_completion_tokens` usage pattern
/// - May include cost/ticks/reasoning details in usage
fn normalize_xai_event(event: &Value, seq: &mut u64) -> (String, Value) {
    // Check for [DONE] marker
    if event.as_str() == Some("[DONE]") {
        return (
            "end".to_string(),
            serde_json::json!({"finish_reason": "stop", "marker": "[DONE]"}),
        );
    }

    // Check for Responses API event field (xAI uses same shape)
    if let Some(evt_type) = event.get("event").and_then(Value::as_str) {
        if evt_type.contains("completed") || evt_type.contains("done") {
            return (
                "end".to_string(),
                serde_json::json!({"finish_reason": "complete", "event": evt_type}),
            );
        }
        if let Some(delta) = event
            .get("data")
            .and_then(|d| d.get("delta"))
            .and_then(Value::as_str)
        {
            return (
                "chunk".to_string(),
                serde_json::json!({"text_delta": delta, "event": evt_type}),
            );
        }
    }

    // Check choices for content and reasoning
    if let Some(choices) = event.get("choices").and_then(Value::as_array) {
        for choice in choices {
            if let Some(fr) = choice.get("finish_reason").and_then(Value::as_str) {
                if !fr.is_empty() {
                    return ("end".to_string(), serde_json::json!({"finish_reason": fr}));
                }
            }
            if let Some(delta) = choice.get("delta") {
                // L6: xAI reasoning content
                if let Some(reasoning) = delta.get("reasoning_content").and_then(Value::as_str) {
                    if !reasoning.is_empty() {
                        return (
                            "chunk".to_string(),
                            serde_json::json!({
                                "reasoning_delta": reasoning,
                                "provider_quirk": "xai_reasoning_content",
                            }),
                        );
                    }
                }
                // Regular content delta
                if let Some(content) = delta.get("content").and_then(Value::as_str) {
                    if !content.is_empty() {
                        return (
                            "chunk".to_string(),
                            serde_json::json!({"text_delta": content}),
                        );
                    }
                }
            }
        }
    }

    // L6: xAI usage with reasoning/cost details
    if event.get("usage").is_some() {
        *seq += 1;
        let usage = &event["usage"];
        let has_reasoning =
            usage.get("reasoning_tokens").is_some() || usage.get("prompt_tokens_details").is_some();
        return (
            "progress".to_string(),
            serde_json::json!({
                "usage_present": true,
                "provider_quirk": if has_reasoning { Some("xai_reasoning_usage") } else { None },
            }),
        );
    }

    ("chunk".to_string(), event.clone())
}

/// Normalize a Fireworks-style event with quirks:
/// - Perf/latency metadata in stream chunks
/// - Prompt/usage metadata with `prompt_tokens_details` and timing
/// - Responses-style stream with session/MCP continuation
fn normalize_fireworks_event(event: &Value, seq: &mut u64) -> (String, Value) {
    // Check for [DONE] marker
    if event.as_str() == Some("[DONE]") {
        return (
            "end".to_string(),
            serde_json::json!({"finish_reason": "stop", "marker": "[DONE]"}),
        );
    }

    // Check for finish_reason in choices
    if let Some(choices) = event.get("choices").and_then(Value::as_array) {
        for choice in choices {
            if let Some(fr) = choice.get("finish_reason").and_then(Value::as_str) {
                if !fr.is_empty() {
                    return ("end".to_string(), serde_json::json!({"finish_reason": fr}));
                }
            }
            if let Some(delta) = choice.get("delta") {
                if let Some(content) = delta.get("content").and_then(Value::as_str) {
                    if !content.is_empty() {
                        return (
                            "chunk".to_string(),
                            serde_json::json!({"text_delta": content}),
                        );
                    }
                }
            }
        }
    }

    // L6: Fireworks usage with perf/timing metadata
    if event.get("usage").is_some() {
        *seq += 1;
        let usage = &event["usage"];
        let has_perf = usage.get("prompt_tokens_details").is_some()
            || usage.get("completion_tokens_details").is_some();
        return (
            "progress".to_string(),
            serde_json::json!({
                "usage_present": true,
                "provider_quirk": if has_perf { Some("fireworks_perf_usage") } else { None },
            }),
        );
    }

    // L6: Fireworks may include timing/latency metadata at top level
    if event.get("timing").is_some() || event.get("latency_ms").is_some() {
        *seq += 1;
        return (
            "progress".to_string(),
            serde_json::json!({
                "usage_present": false,
                "provider_quirk": "fireworks_latency_metadata",
            }),
        );
    }

    ("chunk".to_string(), event.clone())
}

/// Normalize an Anthropic-style semantic SSE event.
fn normalize_semantic_sse_event(event: &Value, _seq: &mut u64) -> (String, Value) {
    let event_type = event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();

    match event_type {
        "message_start" => {
            // Canonical start is already injected; this becomes a progress heartbeat
            (
                "progress".to_string(),
                serde_json::json!({"provider_event": "message_start"}),
            )
        }
        "content_block_start" => (
            "chunk".to_string(),
            serde_json::json!({"provider_event": "content_block_start"}),
        ),
        "content_block_delta" => {
            if let Some(delta) = event.get("delta") {
                if let Some(text) = delta.get("text").and_then(Value::as_str) {
                    return ("chunk".to_string(), serde_json::json!({"text_delta": text}));
                }
            }
            ("chunk".to_string(), event.clone())
        }
        "content_block_stop" => (
            "progress".to_string(),
            serde_json::json!({"provider_event": "content_block_stop"}),
        ),
        "message_delta" => {
            if let Some(delta) = event.get("delta") {
                if let Some(stop_reason) = delta.get("stop_reason").and_then(Value::as_str) {
                    return (
                        "end".to_string(),
                        serde_json::json!({"finish_reason": stop_reason}),
                    );
                }
            }
            if event.get("usage").is_some() {
                return (
                    "progress".to_string(),
                    serde_json::json!({"usage_present": true}),
                );
            }
            ("progress".to_string(), event.clone())
        }
        "message_stop" => (
            "end".to_string(),
            serde_json::json!({"provider_event": "message_stop"}),
        ),
        "ping" => (
            "progress".to_string(),
            serde_json::json!({"provider_event": "ping"}),
        ),
        "error" => (
            "error".to_string(),
            serde_json::json!({"provider_event": "error"}),
        ),
        _ => ("chunk".to_string(), event.clone()),
    }
}

/// Normalize a Gemini-style typed chunk stream event.
fn normalize_typed_chunk_event(event: &Value, _seq: &mut u64) -> (String, Value) {
    // Check for candidates with content
    if let Some(candidates) = event.get("candidates").and_then(Value::as_array) {
        for candidate in candidates {
            if let Some(content) = candidate.get("content") {
                if let Some(parts) = content.get("parts").and_then(Value::as_array) {
                    for part in parts {
                        if let Some(text) = part.get("text").and_then(Value::as_str) {
                            if !text.is_empty() {
                                return (
                                    "chunk".to_string(),
                                    serde_json::json!({"text_delta": text}),
                                );
                            }
                        }
                    }
                }
            }
            // Check finish reason
            if let Some(finish_reason) = candidate.get("finishReason").and_then(Value::as_str) {
                return (
                    "end".to_string(),
                    serde_json::json!({"finish_reason": finish_reason}),
                );
            }
        }
    }

    // Usage metadata
    if event.get("usageMetadata").is_some() {
        return (
            "progress".to_string(),
            serde_json::json!({"usage_present": true}),
        );
    }

    ("chunk".to_string(), event.clone())
}

/// Build fake stream frames for a provider family (default sample).
fn build_fake_stream_frames(family: &str, stream_family: &str, invocation_id: &str) -> Vec<Value> {
    let mut frames = Vec::new();
    let mut seq = 0u64;

    // Start frame
    seq += 1;
    frames.push(serde_json::json!({
        "kind": "start",
        "invocation_id": invocation_id,
        "sequence": seq,
        "payload": {
            "provider_family": family,
            "stream_family": stream_family,
            "model": "fake-model",
        },
        "metadata": {"provider_family": family, "stream_family": stream_family},
        "redaction_state": "redacted",
    }));

    // Provider-specific chunk frames
    match stream_family {
        "delta_sse" => {
            // OpenAI-style delta SSE: text deltas
            for (i, text) in ["fake", " local", " stream"].iter().enumerate() {
                seq += 1;
                frames.push(serde_json::json!({
                    "kind": "chunk",
                    "invocation_id": invocation_id,
                    "sequence": seq,
                    "payload": {"text_delta": text, "delta_index": i},
                    "metadata": {"provider_family": family, "stream_family": stream_family},
                    "redaction_state": "redacted",
                }));
            }
            // L6: Provider-specific quirks in fake delta_sse frames
            if family == "deepseek" {
                // DeepSeek reasoning_content frame
                seq += 1;
                frames.push(serde_json::json!({
                    "kind": "chunk",
                    "invocation_id": invocation_id,
                    "sequence": seq,
                    "payload": {"reasoning_delta": "fake reasoning", "provider_quirk": "deepseek_reasoning_content"},
                    "metadata": {"provider_family": family, "stream_family": stream_family},
                    "redaction_state": "redacted",
                }));
                // DeepSeek cache usage progress
                seq += 1;
                frames.push(serde_json::json!({
                    "kind": "progress",
                    "invocation_id": invocation_id,
                    "sequence": seq,
                    "payload": {"usage_present": true, "provider_quirk": "deepseek_cache_usage"},
                    "metadata": {"provider_family": family, "stream_family": stream_family},
                    "redaction_state": "redacted",
                }));
            } else if family == "fireworks" {
                // Fireworks perf usage
                seq += 1;
                frames.push(serde_json::json!({
                    "kind": "progress",
                    "invocation_id": invocation_id,
                    "sequence": seq,
                    "payload": {"usage_present": true, "provider_quirk": "fireworks_perf_usage"},
                    "metadata": {"provider_family": family, "stream_family": stream_family},
                    "redaction_state": "redacted",
                }));
            }
            // Finish reason chunk
            seq += 1;
            frames.push(serde_json::json!({
                "kind": "chunk",
                "invocation_id": invocation_id,
                "sequence": seq,
                "payload": {"finish_reason": "stop", "marker": "[DONE]"},
                "metadata": {"provider_family": family, "stream_family": stream_family},
                "redaction_state": "redacted",
            }));
        }
        "semantic_sse" => {
            // Anthropic-style: content_block_delta → message_delta
            seq += 1;
            frames.push(serde_json::json!({
                "kind": "chunk",
                "invocation_id": invocation_id,
                "sequence": seq,
                "payload": {"text_delta": "fake local stream", "provider_event": "content_block_delta"},
                "metadata": {"provider_family": family, "stream_family": stream_family},
                "redaction_state": "redacted",
            }));
            // L6: OpenRouter/xAI reasoning in semantic_sse
            if family == "xai" {
                seq += 1;
                frames.push(serde_json::json!({
                    "kind": "chunk",
                    "invocation_id": invocation_id,
                    "sequence": seq,
                    "payload": {"reasoning_delta": "fake reasoning", "provider_quirk": "xai_reasoning_content"},
                    "metadata": {"provider_family": family, "stream_family": stream_family},
                    "redaction_state": "redacted",
                }));
                // xAI reasoning usage progress
                seq += 1;
                frames.push(serde_json::json!({
                    "kind": "progress",
                    "invocation_id": invocation_id,
                    "sequence": seq,
                    "payload": {"usage_present": true, "provider_quirk": "xai_reasoning_usage"},
                    "metadata": {"provider_family": family, "stream_family": stream_family},
                    "redaction_state": "redacted",
                }));
            }
            // Usage/stop
            seq += 1;
            frames.push(serde_json::json!({
                "kind": "progress",
                "invocation_id": invocation_id,
                "sequence": seq,
                "payload": {"usage_present": true, "provider_event": "message_delta"},
                "metadata": {"provider_family": family, "stream_family": stream_family},
                "redaction_state": "redacted",
            }));
        }
        "typed_chunk_stream" => {
            // Gemini-style: candidates with text parts
            seq += 1;
            frames.push(serde_json::json!({
                "kind": "chunk",
                "invocation_id": invocation_id,
                "sequence": seq,
                "payload": {"text_delta": "fake local stream", "provider_event": "candidates"},
                "metadata": {"provider_family": family, "stream_family": stream_family},
                "redaction_state": "redacted",
            }));
        }
        _ => {
            // Unknown stream family: just emit one generic chunk
            seq += 1;
            frames.push(serde_json::json!({
                "kind": "chunk",
                "invocation_id": invocation_id,
                "sequence": seq,
                "payload": {"text_delta": "fake local stream"},
                "metadata": {"provider_family": family, "stream_family": stream_family},
                "redaction_state": "redacted",
            }));
        }
    }

    // End frame
    seq += 1;
    frames.push(serde_json::json!({
        "kind": "end",
        "invocation_id": invocation_id,
        "sequence": seq,
        "payload": {"finish_reason": "stop"},
        "metadata": {"provider_family": family, "stream_family": stream_family},
        "redaction_state": "redacted",
    }));

    frames
}

fn default_stream_family_for_family(family: &str) -> &'static str {
    match family {
        "openai" => "semantic_sse", // default (responses) dialect uses semantic_sse
        "anthropic" => "semantic_sse",
        "gemini" => "typed_chunk_stream",
        "openai_compatible" => "delta_sse",
        "openrouter" => "semantic_sse", // default (responses) dialect
        "deepseek" => "delta_sse",
        "xai" => "semantic_sse", // default (responses) dialect
        "fireworks" => "delta_sse",
        _ => "unknown",
    }
}

// ---------------------------------------------------------------------------
// explain_error
// ---------------------------------------------------------------------------

fn explain_error(request: &InprocInvocation) -> anyhow::Result<Value> {
    let status = request.input.get("status").and_then(Value::as_i64);
    let code = request
        .input
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let family = request
        .input
        .get("family")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let stage = request
        .input
        .get("stage")
        .and_then(Value::as_str)
        .unwrap_or("request");

    let code_lower = code.to_lowercase();

    let (kind, retryable) = if !code.is_empty() {
        map_provider_code(&code_lower)
    } else if let Some(s) = status {
        map_http_status(s)
    } else {
        ("unknown".to_string(), false)
    };

    Ok(serde_json::json!({
        "kind": "model_provider_error_explanation",
        "error_kind": kind,
        "retryable": retryable,
        "stage": stage,
        "provider_family": family,
        "provider_code": if code.is_empty() { Value::Null } else { serde_json::json!(code) },
        "http_status": status,
        "network_performed": false,
        "inference_performed": false,
        "provenance": {"package_id": request.provider_package_id, "capability_id": request.capability_id}
    }))
}

// ---------------------------------------------------------------------------
// Shared profile extraction + normalization
// ---------------------------------------------------------------------------

struct ProfileParams {
    family: String,
    model: String,
    credential: String,
    base_url: String,
    prefer_responses: bool,
    stream: bool,
}

fn extract_profile_params(request: &InprocInvocation) -> ProfileParams {
    let profile = request.input.get("profile");
    let family = profile
        .and_then(|p| p.get("family"))
        .or_else(|| request.input.get("family"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let model = profile
        .and_then(|p| p.get("model"))
        .or_else(|| request.input.get("model"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let credential = profile
        .and_then(|p| p.get("credential"))
        .or_else(|| request.input.get("credential"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let base_url = profile
        .and_then(|p| p.get("baseUrl"))
        .or_else(|| request.input.get("base_url"))
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .or_else(|| default_base_url_for_family(&family).map(|s| s.to_string()))
        .unwrap_or_default();

    let extra = request
        .input
        .get("extra")
        .or_else(|| profile.and_then(|p| p.get("extra")));
    let prefer_responses = extra
        .and_then(|e| e.get("preferResponses"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let stream = request
        .input
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    ProfileParams {
        family,
        model,
        credential,
        base_url,
        prefer_responses,
        stream,
    }
}

fn build_normalized_shape(
    request: &InprocInvocation,
    params: &ProfileParams,
) -> anyhow::Result<NormalizedShape> {
    let profile = request.input.get("profile");
    let extra = request
        .input
        .get("extra")
        .or_else(|| profile.and_then(|p| p.get("extra")));

    let (endpoint, request_dialect, stream_family, headers, body_shape) =
        match params.family.as_str() {
            "openai" => {
                let is_responses = params.prefer_responses;
                let dialect = if is_responses {
                    "openai_responses".to_string()
                } else {
                    "openai_chat".to_string()
                };
                let sf = if is_responses {
                    "semantic_sse".to_string()
                } else {
                    "delta_sse".to_string()
                };
                let ep = if is_responses {
                    format!("{}/v1/responses", params.base_url)
                } else {
                    format!("{}/v1/chat/completions", params.base_url)
                };
                let hdrs = credential_header("Authorization", "Bearer", &params.credential);
                let body = if is_responses {
                    serde_json::json!({
                        "model": params.model,
                        "input": request.input.get("messages"),
                        "stream": params.stream,
                        "max_output_tokens": request.input.get("max_tokens"),
                        "temperature": request.input.get("temperature"),
                        "tools": request.input.get("tools"),
                    })
                } else {
                    serde_json::json!({
                        "model": params.model,
                        "messages": request.input.get("messages"),
                        "stream": params.stream,
                        "max_tokens": request.input.get("max_tokens"),
                        "temperature": request.input.get("temperature"),
                        "tools": request.input.get("tools"),
                    })
                };
                (ep, dialect, sf, hdrs, body)
            }
            "anthropic" => {
                let ep = format!("{}/v1/messages", params.base_url);
                let mut hdrs = serde_json::json!({
                    "x-api-key": credential_ref_placeholder(&params.credential),
                    "anthropic-version": "2023-06-01",
                    "Content-Type": "application/json",
                });
                if let Some(ph) = profile
                    .and_then(|p| p.get("headers"))
                    .and_then(Value::as_object)
                {
                    for (k, v) in ph {
                        if k != "anthropic-version" && v.is_string() {
                            hdrs[k] = v.clone();
                        }
                    }
                }
                let body = serde_json::json!({
                    "model": params.model,
                    "messages": request.input.get("messages"),
                    "system": request.input.get("system"),
                    "stream": params.stream,
                    "max_tokens": request.input.get("max_tokens"),
                    "temperature": request.input.get("temperature"),
                    "tools": request.input.get("tools"),
                });
                (
                    ep,
                    "anthropic_messages".to_string(),
                    "semantic_sse".to_string(),
                    hdrs,
                    body,
                )
            }
            "gemini" => {
                let stream_suffix = if params.stream { "?alt=sse" } else { "" };
                let ep = format!(
                    "{}/v1beta/models/{}:generateContent{}",
                    params.base_url, params.model, stream_suffix
                );
                let hdrs = serde_json::json!({
                    "x-goog-api-key": credential_ref_placeholder(&params.credential),
                    "Content-Type": "application/json",
                });
                let body = serde_json::json!({
                    "contents": request.input.get("messages"),
                    "systemInstruction": request.input.get("systemInstruction"),
                    "generationConfig": {
                        "maxOutputTokens": request.input.get("max_tokens"),
                        "temperature": request.input.get("temperature"),
                    },
                });
                (
                    ep,
                    "gemini_generate_content".to_string(),
                    "typed_chunk_stream".to_string(),
                    hdrs,
                    body,
                )
            }
            "openai_compatible" => {
                let ep = format!("{}/chat/completions", params.base_url);
                let hdrs = credential_header("Authorization", "Bearer", &params.credential);
                let body = serde_json::json!({
                    "model": params.model,
                    "messages": request.input.get("messages"),
                    "stream": params.stream,
                    "max_tokens": request.input.get("max_tokens"),
                    "temperature": request.input.get("temperature"),
                    "tools": request.input.get("tools"),
                });
                (
                    ep,
                    "openai_chat".to_string(),
                    "delta_sse".to_string(),
                    hdrs,
                    body,
                )
            }
            "openrouter" => {
                let is_responses = params.prefer_responses;
                let dialect = if is_responses {
                    "stateless_responses".to_string()
                } else {
                    "openai_chat".to_string()
                };
                let sf = if is_responses {
                    "semantic_sse".to_string()
                } else {
                    "delta_sse".to_string()
                };
                let ep = if is_responses {
                    format!("{}/responses", params.base_url)
                } else {
                    format!("{}/chat/completions", params.base_url)
                };
                let hdrs = credential_header("Authorization", "Bearer", &params.credential);
                let body = if is_responses {
                    serde_json::json!({
                        "model": params.model,
                        "input": request.input.get("messages"),
                        "stream": params.stream,
                    })
                } else {
                    serde_json::json!({
                        "model": params.model,
                        "messages": request.input.get("messages"),
                        "stream": params.stream,
                        "max_tokens": request.input.get("max_tokens"),
                        "temperature": request.input.get("temperature"),
                        "tools": request.input.get("tools"),
                    })
                };
                (ep, dialect, sf, hdrs, body)
            }
            "deepseek" => {
                let ep = format!("{}/chat/completions", params.base_url);
                let hdrs = credential_header("Authorization", "Bearer", &params.credential);
                let mut body = serde_json::json!({
                    "model": params.model,
                    "messages": request.input.get("messages"),
                    "stream": params.stream,
                    "max_tokens": request.input.get("max_tokens"),
                    "temperature": request.input.get("temperature"),
                    "tools": request.input.get("tools"),
                });
                if let Some(effort) = extra.and_then(|e| e.get("reasoning_effort")) {
                    body["reasoning_effort"] = effort.clone();
                }
                (
                    ep,
                    "openai_chat".to_string(),
                    "delta_sse".to_string(),
                    hdrs,
                    body,
                )
            }
            "xai" => {
                let is_responses = params.prefer_responses;
                let dialect = if is_responses {
                    "openai_responses".to_string()
                } else {
                    "openai_chat".to_string()
                };
                let sf = if is_responses {
                    "semantic_sse".to_string()
                } else {
                    "delta_sse".to_string()
                };
                let ep = if is_responses {
                    format!("{}/v1/responses", params.base_url)
                } else {
                    format!("{}/v1/chat/completions", params.base_url)
                };
                let hdrs = credential_header("Authorization", "Bearer", &params.credential);
                let body = if is_responses {
                    serde_json::json!({
                        "model": params.model,
                        "input": request.input.get("messages"),
                        "stream": params.stream,
                        "max_output_tokens": request.input.get("max_tokens"),
                    })
                } else {
                    serde_json::json!({
                        "model": params.model,
                        "messages": request.input.get("messages"),
                        "stream": params.stream,
                        "max_completion_tokens": request.input.get("max_tokens"),
                        "temperature": request.input.get("temperature"),
                        "tools": request.input.get("tools"),
                    })
                };
                (ep, dialect, sf, hdrs, body)
            }
            "fireworks" => {
                let is_responses = params.prefer_responses;
                let dialect = if is_responses {
                    "fireworks_responses".to_string()
                } else {
                    "openai_chat".to_string()
                };
                let sf = if is_responses {
                    "semantic_sse".to_string()
                } else {
                    "delta_sse".to_string()
                };
                let ep = if is_responses {
                    format!("{}/responses", params.base_url)
                } else {
                    format!("{}/chat/completions", params.base_url)
                };
                let hdrs = credential_header("Authorization", "Bearer", &params.credential);
                let body = if is_responses {
                    serde_json::json!({
                        "model": params.model,
                        "input": request.input.get("messages"),
                        "stream": params.stream,
                    })
                } else {
                    serde_json::json!({
                        "model": params.model,
                        "messages": request.input.get("messages"),
                        "stream": params.stream,
                        "max_tokens": request.input.get("max_tokens"),
                        "temperature": request.input.get("temperature"),
                        "tools": request.input.get("tools"),
                    })
                };
                (ep, dialect, sf, hdrs, body)
            }
            _ => {
                // Return an empty shape for unsupported families;
                // callers (normalize_request, invoke) handle the error case.
                (
                    String::new(),
                    "unknown".to_string(),
                    "unknown".to_string(),
                    serde_json::json!({}),
                    serde_json::json!({}),
                )
            }
        };

    Ok(NormalizedShape {
        endpoint,
        request_dialect,
        stream_family,
        headers,
        body_shape,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_valid_secret_ref(s: &str) -> bool {
    for prefix in &["secret_ref:", "secretRef:", "secret-ref:"] {
        if s.starts_with(prefix) {
            let after = &s[prefix.len()..];
            return after.contains(':') && after.len() > 2;
        }
    }
    if s.starts_with("host:") {
        return s.len() > 5;
    }
    false
}

fn looks_like_raw_secret(value: &str) -> bool {
    if is_valid_secret_ref(value) {
        return false;
    }
    if value.starts_with("Bearer ") || value.starts_with("bearer ") {
        return true;
    }
    if value.starts_with("sk-") || value.starts_with("sk_") {
        return true;
    }
    if value.starts_with("key-") || value.starts_with("key_") {
        return true;
    }
    if value.starts_with("AIza") {
        return true;
    }
    if value.len() >= 32 {
        let alphanum: bool = value
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_');
        if alphanum {
            let has_upper = value.chars().any(|c| c.is_uppercase());
            let has_lower = value.chars().any(|c| c.is_lowercase());
            let has_digit = value.chars().any(|c| c.is_ascii_digit());
            if has_upper && has_lower && has_digit {
                return true;
            }
            if value.chars().all(|c| c.is_ascii_hexdigit()) && value.len() >= 32 {
                return true;
            }
        }
    }
    false
}

fn credential_header(auth_header: &str, scheme: &str, credential: &str) -> Value {
    let placeholder = credential_ref_placeholder(credential);
    serde_json::json!({
        auth_header: format!("{} {}", scheme, placeholder),
        "Content-Type": "application/json",
    })
}

fn credential_ref_placeholder(credential: &str) -> String {
    if is_valid_secret_ref(credential) {
        format!("<{}>", credential)
    } else {
        "<credential_ref>".to_string()
    }
}

fn headers_value<'a>(headers: Option<&'a Value>, key: &str) -> Option<&'a str> {
    headers
        .and_then(Value::as_object)
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_str)
}

fn headers_contain_raw_secret(headers: Option<&Value>) -> bool {
    headers
        .and_then(Value::as_object)
        .map(|obj| {
            obj.values()
                .any(|value| value.as_str().map(looks_like_raw_secret).unwrap_or(false))
        })
        .unwrap_or(false)
}

fn default_base_url_for_family(family: &str) -> Option<&'static str> {
    match family {
        "openai" => Some("https://api.openai.com"),
        "anthropic" => Some("https://api.anthropic.com"),
        "gemini" => Some("https://generativelanguage.googleapis.com"),
        "openrouter" => Some("https://openrouter.ai/api/v1"),
        "deepseek" => Some("https://api.deepseek.com"),
        "xai" => Some("https://api.x.ai"),
        "fireworks" => Some("https://api.fireworks.ai/inference/v1"),
        _ => None,
    }
}

fn extract_host(url: &str) -> Option<String> {
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host_part = stripped.split('/').next()?;
    let host = host_part.split(':').next()?;
    Some(host.to_string())
}

/// Split an endpoint URL into (destination_host, path).
fn split_endpoint(endpoint: &str) -> (String, String) {
    if let Some(stripped) = endpoint
        .strip_prefix("https://")
        .or_else(|| endpoint.strip_prefix("http://"))
    {
        if let Some(slash_pos) = stripped.find('/') {
            let host = stripped[..slash_pos]
                .split(':')
                .next()
                .unwrap_or(&stripped[..slash_pos])
                .to_string();
            let path = stripped[slash_pos..].to_string();
            return (host, path);
        }
        let host = stripped.split(':').next().unwrap_or(stripped).to_string();
        return (host, "/".to_string());
    }
    (endpoint.to_string(), "/".to_string())
}

fn map_provider_code(code_lower: &str) -> (String, bool) {
    if code_lower.contains("invalid_request") {
        return ("bad_request".to_string(), false);
    }
    if code_lower.contains("authentication_error") {
        return ("authentication".to_string(), false);
    }
    if code_lower.contains("permission_error") {
        return ("permission".to_string(), false);
    }
    if code_lower.contains("not_found_error") {
        return ("not_found".to_string(), false);
    }
    if code_lower.contains("rate_limit_error") {
        return ("rate_limit".to_string(), true);
    }
    if code_lower.contains("overloaded_error") || code_lower == "529" {
        return ("overloaded".to_string(), true);
    }
    if code_lower.contains("timeout_error") {
        return ("timeout".to_string(), true);
    }
    if code_lower.contains("api_error") {
        return ("upstream_malformed".to_string(), true);
    }
    if code_lower == "invalid_argument" {
        return ("bad_request".to_string(), false);
    }
    if code_lower == "permission_denied" {
        return ("permission".to_string(), false);
    }
    if code_lower == "resource_exhausted" {
        return ("rate_limit".to_string(), true);
    }
    if code_lower == "not_found" {
        return ("not_found".to_string(), false);
    }
    if code_lower == "unavailable" {
        return ("overloaded".to_string(), true);
    }
    if code_lower == "deadline_exceeded" {
        return ("timeout".to_string(), true);
    }
    if code_lower == "unauthenticated" {
        return ("authentication".to_string(), false);
    }
    if code_lower == "invalid_api_key" {
        return ("authentication".to_string(), false);
    }
    if code_lower == "model_not_found" {
        return ("not_found".to_string(), false);
    }
    if code_lower == "insufficient_quota" {
        return ("billing".to_string(), false);
    }
    if code_lower.contains("tool") && code_lower.contains("schema") {
        return ("tool_schema".to_string(), false);
    }
    if code_lower.contains("stream") {
        return ("stream_error".to_string(), true);
    }
    ("unknown".to_string(), false)
}

fn map_http_status(status: i64) -> (String, bool) {
    match status {
        400 => ("bad_request".to_string(), false),
        401 => ("authentication".to_string(), false),
        402 => ("billing".to_string(), false),
        403 => ("permission".to_string(), false),
        404 => ("not_found".to_string(), false),
        408 => ("timeout".to_string(), true),
        422 => ("tool_schema".to_string(), false),
        429 => ("rate_limit".to_string(), true),
        500 => ("upstream_malformed".to_string(), true),
        502 => ("overloaded".to_string(), true),
        503 => ("overloaded".to_string(), true),
        504 => ("timeout".to_string(), true),
        529 => ("overloaded".to_string(), true),
        _ => ("unknown".to_string(), false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_endpoint_extracts_host_and_path() {
        let (host, path) = split_endpoint("https://api.openai.com/v1/chat/completions");
        assert_eq!(host, "api.openai.com");
        assert_eq!(path, "/v1/chat/completions");

        let (host, path) = split_endpoint("https://api.anthropic.com/v1/messages");
        assert_eq!(host, "api.anthropic.com");
        assert_eq!(path, "/v1/messages");

        let (host, path) = split_endpoint("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent");
        assert_eq!(host, "generativelanguage.googleapis.com");
        assert_eq!(path, "/v1beta/models/gemini-2.0-flash:generateContent");
    }

    #[test]
    fn split_endpoint_handles_no_path() {
        let (host, path) = split_endpoint("https://api.example.com");
        assert_eq!(host, "api.example.com");
        assert_eq!(path, "/");
    }
}
