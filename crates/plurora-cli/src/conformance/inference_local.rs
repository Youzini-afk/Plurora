//! Conformance tests for `official/inference-local-lab`.
//!
//! Proves that the inference capability seam does not depend on HTTP,
//! bearer tokens, JSON provider schemas, or network access.

use std::path::PathBuf;

use plurora_runtime::CapabilityInvocationRequest;
use serde_json::json;

use super::fixtures::*;
use crate::commands::manifest;

pub(crate) async fn inference_local_lab_describe_capabilities() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/official/inference-local-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    let caps = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-local-lab/describe_capabilities".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-local-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await?;
    anyhow::ensure!(
        caps.output["kind"] == json!("inference_local_capabilities"),
        "inference-local-lab describe_capabilities wrong kind"
    );
    // transport_kinds must include in_memory and local_process
    let transports = caps.output["transport_kinds"]
        .as_array()
        .expect("transport_kinds must be array");
    anyhow::ensure!(
        transports.iter().any(|t| t == "in_memory"),
        "transport_kinds must include in_memory"
    );
    anyhow::ensure!(
        transports.iter().any(|t| t == "local_process"),
        "transport_kinds must include local_process"
    );
    // Does not require network or secrets
    anyhow::ensure!(
        caps.output["network_required"] == json!(false),
        "network_required must be false"
    );
    anyhow::ensure!(
        caps.output["secrets_required"] == json!(false),
        "secrets_required must be false"
    );
    // Streaming supported
    anyhow::ensure!(
        caps.output["streaming_supported"] == json!(true),
        "streaming_supported must be true"
    );
    // operation_kinds must include generate/classify/transform
    let ops = caps.output["operation_kinds"]
        .as_array()
        .expect("operation_kinds must be array");
    anyhow::ensure!(
        ops.iter().any(|o| o == "generate"),
        "operation_kinds must include generate"
    );
    anyhow::ensure!(
        ops.iter().any(|o| o == "classify"),
        "operation_kinds must include classify"
    );
    anyhow::ensure!(
        ops.iter().any(|o| o == "transform"),
        "operation_kinds must include transform"
    );
    Ok(())
}

pub(crate) async fn inference_local_lab_invoke() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/official/inference-local-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    // invoke non-HTTP succeeds with no URL/header/status/messages fields, network_performed=false
    let inv = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-local-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-local-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "operation_id": "op_test_001",
                "operation_kind": "generate",
                "input_payload": {"text": "hello"},
                "transport_kind": "in_memory"
            }),
        })
        .await?;
    anyhow::ensure!(
        inv.output["kind"] == json!("inference_local_invoke_result"),
        "invoke wrong kind"
    );
    anyhow::ensure!(
        inv.output["network_performed"] == json!(false),
        "invoke must not perform network"
    );
    anyhow::ensure!(
        inv.output["transport_performed"] == json!("in_memory_fake"),
        "invoke transport_performed must be in_memory_fake"
    );
    anyhow::ensure!(
        inv.output["normalized_error"].is_null(),
        "invoke should not have error"
    );
    // No URL/header/status/messages fields in output
    let output_str = serde_json::to_string(&inv.output).unwrap();
    anyhow::ensure!(
        !output_str.contains("\"url\""),
        "invoke output should not contain url field"
    );
    anyhow::ensure!(
        !output_str.contains("\"header\"") && !output_str.contains("\"headers\""),
        "invoke output should not contain header fields"
    );
    anyhow::ensure!(
        !output_str.contains("\"status_code\"") && !output_str.contains("\"statusCode\""),
        "invoke output should not contain status code fields"
    );
    anyhow::ensure!(
        !output_str.contains("\"messages\""),
        "invoke output should not contain messages field"
    );

    // invoke classify
    let inv_classify = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-local-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-local-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "operation_id": "op_test_002",
                "operation_kind": "classify",
                "transport_kind": "local_process"
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_classify.output["output_payload"]["label"].is_string(),
        "classify must return label"
    );
    anyhow::ensure!(
        inv_classify.output["network_performed"] == json!(false),
        "classify must not perform network"
    );

    // invoke transform
    let inv_transform = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-local-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-local-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "operation_id": "op_test_003",
                "operation_kind": "transform",
                "input_payload": {"data": "test"}
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_transform.output["output_payload"]["transformed"].is_string(),
        "transform must return transformed output"
    );
    anyhow::ensure!(
        inv_transform.output["network_performed"] == json!(false),
        "transform must not perform network"
    );

    Ok(())
}

pub(crate) async fn inference_local_lab_invoke_rejects_http() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/official/inference-local-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    // invoke rejects http transport
    let inv_http = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-local-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-local-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "operation_kind": "generate",
                "transport_kind": "http"
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_http.output["normalized_error"]["error_kind"] == json!("transport_rejected"),
        "http transport must be rejected"
    );
    anyhow::ensure!(
        inv_http.output["network_performed"] == json!(false),
        "http rejected invoke must not perform network"
    );

    // invoke rejects HTTP-shaped fields: url
    let inv_url = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-local-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-local-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "operation_kind": "generate",
                "url": "https://api.example.com/v1/chat"
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_url.output["normalized_error"]["error_kind"] == json!("http_field_rejected"),
        "url field must be rejected as http_field_rejected"
    );

    // invoke rejects HTTP-shaped fields: headers
    let inv_headers = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-local-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-local-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "operation_kind": "generate",
                "headers": {"x-test-auth": "rawSecretPlaceholder1234567890ABCDEF"}
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_headers.output["normalized_error"]["error_kind"] == json!("http_field_rejected"),
        "headers field must be rejected as http_field_rejected"
    );

    // invoke rejects HTTP-shaped fields: status_code
    let inv_status = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-local-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-local-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "operation_kind": "generate",
                "status_code": 200
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_status.output["normalized_error"]["error_kind"] == json!("http_field_rejected"),
        "status_code field must be rejected as http_field_rejected"
    );

    // invoke rejects messages-shaped fields: messages
    let inv_messages = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-local-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-local-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "operation_kind": "generate",
                "messages": [{"role": "user", "content": "hello"}]
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_messages.output["normalized_error"]["error_kind"] == json!("messages_field_rejected"),
        "messages field must be rejected as messages_field_rejected"
    );

    // invoke rejects messages-shaped fields: system
    let inv_system = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-local-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-local-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "operation_kind": "generate",
                "system": "You are a helpful assistant"
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_system.output["normalized_error"]["error_kind"] == json!("messages_field_rejected"),
        "system field must be rejected as messages_field_rejected"
    );

    // invoke rejects raw secret-looking fields
    let inv_secret = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-local-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-local-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "operation_kind": "generate",
                "api_key": "rawSecretPlaceholder1234567890ABCDEF"
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_secret.output["normalized_error"]["error_kind"] == json!("secret_rejected"),
        "raw api_key must be rejected as secret_rejected"
    );

    Ok(())
}

pub(crate) async fn inference_local_lab_stream() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/official/inference-local-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    let stream_result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-local-lab/stream".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-local-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "invocation_id": "inv_stream_conf_001",
                "operation_kind": "generate",
                "transport_kind": "in_memory"
            }),
        })
        .await?;
    anyhow::ensure!(
        stream_result.output["kind"] == json!("inference_local_stream_result"),
        "stream wrong kind"
    );
    anyhow::ensure!(
        stream_result.output["network_performed"] == json!(false),
        "stream must not perform network"
    );
    anyhow::ensure!(
        stream_result.output["inference_performed"] == json!(false),
        "stream must not claim inference"
    );

    // Frames must have start/chunk/progress/end
    let frames = stream_result.output["frames"]
        .as_array()
        .expect("stream must have frames");
    let kinds: Vec<&str> = frames
        .iter()
        .map(|f| f["kind"].as_str().unwrap_or_default())
        .collect();
    anyhow::ensure!(kinds.first() == Some(&"start"), "first frame must be start");
    anyhow::ensure!(kinds.last() == Some(&"end"), "last frame must be end");
    anyhow::ensure!(kinds.contains(&"chunk"), "must have chunk frames");
    anyhow::ensure!(kinds.contains(&"progress"), "must have progress frame");
    anyhow::ensure!(
        stream_result.output["terminal_frame_consistent"] == json!(true),
        "terminal_frame_consistent must be true"
    );

    // Every frame must have invocation_id, sequence, redaction_state, metadata
    for (i, frame) in frames.iter().enumerate() {
        anyhow::ensure!(
            frame["invocation_id"].is_string(),
            "frame {} missing invocation_id",
            i
        );
        anyhow::ensure!(
            frame["sequence"].is_number(),
            "frame {} missing sequence",
            i
        );
        anyhow::ensure!(
            frame["redaction_state"] == json!("clean"),
            "frame {} must be clean",
            i
        );
        anyhow::ensure!(
            frame["metadata"]["provider_type"] == json!("fake_local_deterministic"),
            "frame {} wrong metadata.provider_type",
            i
        );
    }

    // No URL/header/status/provider_schema in frames
    let frames_str = serde_json::to_string(&frames).unwrap();
    anyhow::ensure!(
        !frames_str.contains("\"url\""),
        "frames should not contain url"
    );
    anyhow::ensure!(
        !frames_str.contains("\"header\"") && !frames_str.contains("\"headers\""),
        "frames should not contain header fields"
    );
    anyhow::ensure!(
        !frames_str.contains("\"status_code\"") && !frames_str.contains("\"statusCode\""),
        "frames should not contain status code fields"
    );

    Ok(())
}

pub(crate) async fn inference_local_lab_explain_error() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/official/inference-local-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    // Cover local/resource error classes
    let local_errors = [
        "local_process_failed",
        "local_resource_exhausted",
        "local_model_not_loaded",
        "local_inference_error",
        "timeout",
        "cancelled",
    ];
    for error_kind in &local_errors {
        let result = runtime
            .invoke_capability(CapabilityInvocationRequest {
                handle: None,
                capability_id: Some("official/inference-local-lab/explain_error".to_string()),
                caller_package_id: None,
                provider_package_id: Some("official/inference-local-lab".to_string()),
                version: None,
                session_id: None,
                input: json!({"error_kind": error_kind}),
            })
            .await?;
        anyhow::ensure!(
            result.output["kind"] == json!("inference_local_error_explanation"),
            "explain_error wrong kind for {}",
            error_kind
        );
        anyhow::ensure!(
            result.output["explanation"].is_string(),
            "explain_error missing explanation for {}",
            error_kind
        );
        let error_class = result.output["error_class"].as_str().unwrap_or_default();
        anyhow::ensure!(
            error_class == "local" || error_class == "resource",
            "explain_error error_class must be local or resource for {}, got {}",
            error_kind,
            error_class
        );
        anyhow::ensure!(
            result.output["network_performed"] == json!(false),
            "explain_error must not perform network for {}",
            error_kind
        );
        anyhow::ensure!(
            result.output["inference_performed"] == json!(false),
            "explain_error must not claim inference for {}",
            error_kind
        );
    }

    Ok(())
}
