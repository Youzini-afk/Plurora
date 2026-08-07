use super::{case, ConformanceCase};

macro_rules! c {
    ($id:expr, [$($tag:expr),*], $func:path) => {
        case($id, &[$($tag),*], || Box::pin($func()))
    };
}

pub(super) fn network_cases() -> Vec<ConformanceCase> {
    vec![
        // --- network ---
        c!(
            "network.no_permission_denied",
            ["network", "outbound"],
            crate::conformance::network::no_network_permission_denied
        ),
        c!(
            "network.allowlisted_host_method_allowed",
            ["network", "outbound"],
            crate::conformance::network::allowlisted_host_method_allowed
        ),
        c!(
            "network.host_method_mismatch_denied",
            ["network", "outbound"],
            crate::conformance::network::host_method_mismatch_denied
        ),
        c!(
            "network.official_no_network_bypass",
            ["network", "outbound"],
            crate::conformance::network::official_no_network_bypass
        ),
        c!(
            "network.audit_no_raw_secrets",
            ["network", "outbound"],
            crate::conformance::network::audit_no_raw_secrets
        ),
        c!(
            "network.policy_pure_function",
            ["network", "outbound"],
            crate::conformance::network::network_policy_pure_function
        ),
    ]
}

pub(super) fn outbound_cases() -> Vec<ConformanceCase> {
    vec![
        // --- outbound ---
        c!(
            "outbound.no_permission_executor_not_called",
            ["outbound", "network"],
            crate::conformance::network::outbound_no_permission_executor_not_called
        ),
        c!(
            "outbound.policy_executor_mismatch_denied",
            ["outbound", "network"],
            crate::conformance::network::outbound_policy_executor_mismatch_denied
        ),
        c!(
            "outbound.allowlisted_fake_executor",
            ["outbound", "network"],
            crate::conformance::network::outbound_allowlisted_fake_executor
        ),
        c!(
            "outbound.raw_body_not_audited",
            ["outbound", "network"],
            crate::conformance::network::outbound_raw_body_not_audited
        ),
        c!(
            "outbound.secret_refs_only",
            ["outbound", "network", "secret"],
            crate::conformance::network::outbound_secret_refs_only
        ),
        c!(
            "outbound.host_mismatch_redirect_denied",
            ["outbound", "network"],
            crate::conformance::network::outbound_host_mismatch_redirect_denied
        ),
        c!(
            "outbound.model_provider_shape_fake_executor",
            ["outbound", "network"],
            crate::conformance::network::outbound_model_provider_shape_fake_executor
        ),
    ]
}

pub(super) fn live_http_outbound_cases() -> Vec<ConformanceCase> {
    vec![
        // --- live http outbound ---
        c!(
            "outbound.live_http_default_disabled",
            ["outbound", "network", "live"],
            crate::conformance::network::outbound_live_http_default_disabled
        ),
        c!(
            "outbound.live_http_rejects_insecure_url",
            ["outbound", "network", "live"],
            crate::conformance::network::outbound_live_http_rejects_insecure_url
        ),
        c!(
            "outbound.live_http_redacted_shape",
            ["outbound", "network", "live"],
            crate::conformance::network::outbound_live_http_redacted_shape
        ),
    ]
}

pub(super) fn kernel_v1_outbound_execute_cases() -> Vec<ConformanceCase> {
    vec![
        // --- kernel.v1.outbound.execute ---
        c!(
            "outbound.execute_package_allowed",
            ["outbound", "network"],
            crate::conformance::network::outbound_execute_package_allowed
        ),
        c!(
            "outbound.execute_spoofed_package_id_rejected",
            ["outbound", "network"],
            crate::conformance::network::outbound_execute_spoofed_package_id_rejected
        ),
        c!(
            "outbound.execute_no_permission_denied",
            ["outbound", "network"],
            crate::conformance::network::outbound_execute_no_permission_denied
        ),
        c!(
            "outbound.execute_no_raw_secret_in_response",
            ["outbound", "network", "secret"],
            crate::conformance::network::outbound_execute_no_raw_secret_in_response
        ),
    ]
}

pub(super) fn y1_outbound_execute_profile_conformance_cases() -> Vec<ConformanceCase> {
    vec![
        // --- Y1: outbound execute profile conformance ---
        c!(
            "outbound_execute.profile_default_deny_all",
            ["outbound", "network", "profile"],
            crate::conformance::network::outbound_execute_profile_default_deny_all
        ),
        c!(
            "outbound_execute.profile_fake_executor_works",
            ["outbound", "network", "profile"],
            crate::conformance::network::outbound_execute_profile_fake_executor_works
        ),
        c!(
            "outbound_execute.profile_live_disabled_returns_deny",
            ["outbound", "network", "profile"],
            crate::conformance::network::outbound_execute_profile_live_disabled_returns_deny
        ),
        c!(
            "outbound_websocket.profile_default_deny_all",
            ["outbound", "network", "websocket"],
            crate::conformance::network::outbound_websocket_profile_default_deny_all
        ),
        c!(
            "outbound_websocket.profile_fake_executor_works",
            ["outbound", "network", "websocket"],
            crate::conformance::network::outbound_websocket_profile_fake_executor_works
        ),
        c!(
            "outbound_websocket.profile_live_disabled_returns_deny",
            ["outbound", "network", "websocket"],
            crate::conformance::network::outbound_websocket_profile_live_disabled_returns_deny
        ),
        c!(
            "outbound_websocket.secret_ref_undeclared_fails",
            ["outbound", "network", "websocket", "secret", "manifest"],
            crate::conformance::network::outbound_websocket_secret_ref_undeclared_fails
        ),
        c!(
            "outbound_websocket.capability_namespace_enforced",
            ["outbound", "network", "websocket", "permission"],
            crate::conformance::network::outbound_websocket_capability_namespace_enforced
        ),
        c!(
            "outbound_websocket.wss_only_default",
            ["outbound", "network", "websocket"],
            crate::conformance::network::outbound_websocket_wss_only_default
        ),
        c!(
            "outbound_websocket.idle_timeout_emits_error_and_completed",
            ["outbound", "network", "websocket", "audit"],
            crate::conformance::network::outbound_websocket_idle_timeout_emits_error_and_completed
        ),
        c!(
            "outbound_websocket.max_total_bytes_inbound_terminates",
            ["outbound", "network", "websocket", "audit"],
            crate::conformance::network::outbound_websocket_max_total_bytes_inbound_terminates
        ),
        c!(
            "outbound_websocket.max_concurrent_connections_enforced",
            ["outbound", "network", "websocket"],
            crate::conformance::network::outbound_websocket_max_concurrent_connections_enforced
        ),
        c!(
            "outbound_websocket.cancel_via_capability_cancel",
            ["outbound", "network", "websocket"],
            crate::conformance::network::outbound_websocket_cancel_via_capability_cancel
        ),
        c!(
            "outbound.execute_completed_audit_emitted",
            ["outbound", "network", "audit"],
            crate::conformance::network::outbound_execute_completed_audit_emitted
        ),
        c!(
            "outbound.consistency_failure_emits_receipt",
            ["outbound", "network", "audit", "receipt"],
            crate::conformance::network::outbound_consistency_failure_emits_receipt
        ),
        c!(
            "outbound.receipt_replays_without_executor",
            ["outbound", "receipt", "replay"],
            crate::conformance::network::outbound_receipt_replays_without_executor
        ),
        c!(
            "outbound.execute_correlation_id_propagates",
            ["outbound", "network", "audit"],
            crate::conformance::network::outbound_execute_correlation_id_propagates
        ),
        c!(
            "outbound.stream_completed_audit_emitted",
            ["outbound", "network", "stream", "audit"],
            crate::conformance::network::outbound_stream_completed_audit_emitted
        ),
        c!(
            "outbound.websocket_completed_audit_emitted",
            ["outbound", "network", "websocket", "audit"],
            crate::conformance::network::outbound_websocket_completed_audit_emitted
        ),
    ]
}

pub(super) fn y2_manifest_permissions_secret_refs_conformance_cases() -> Vec<ConformanceCase> {
    vec![
        // --- Y2: manifest permissions.secret_refs conformance ---
        c!(
            "outbound_execute.secret_ref_undeclared_fails",
            ["outbound", "network", "secret", "manifest"],
            crate::conformance::network::outbound_execute_secret_ref_undeclared_fails
        ),
        c!(
            "outbound_execute.secret_ref_declared_resolves",
            ["outbound", "network", "secret", "manifest"],
            crate::conformance::network::outbound_execute_secret_ref_declared_resolves
        ),
    ]
}

pub(super) fn y3_kernel_v1_outbound_stream_conformance_cases() -> Vec<ConformanceCase> {
    vec![
        // --- Y3: kernel.v1.outbound.stream conformance ---
        c!(
            "outbound_stream.profile_default_deny_all",
            ["outbound", "network", "stream", "profile"],
            crate::conformance::network::outbound_stream_profile_default_deny_all
        ),
        c!(
            "outbound_stream.fake_executor_emits_canned_frames",
            ["outbound", "network", "stream"],
            crate::conformance::network::outbound_stream_fake_executor_emits_canned_frames
        ),
        c!(
            "outbound_stream.secret_ref_undeclared_fails",
            ["outbound", "network", "stream", "secret", "manifest"],
            crate::conformance::network::outbound_stream_secret_ref_undeclared_fails
        ),
        c!(
            "outbound_stream.secret_ref_declared_resolves",
            ["outbound", "network", "stream", "secret", "manifest"],
            crate::conformance::network::outbound_stream_secret_ref_declared_resolves
        ),
        c!(
            "outbound_stream.capability_namespace_enforced",
            ["outbound", "network", "stream", "permission"],
            crate::conformance::network::outbound_stream_capability_namespace_enforced
        ),
        c!(
            "outbound_stream.https_only",
            ["outbound", "network", "stream", "profile"],
            crate::conformance::network::outbound_stream_https_only
        ),
        c!(
            "subprocess.reverse_kernel_call_dispatched",
            ["subprocess", "outbound", "network"],
            crate::conformance::network::subprocess_reverse_kernel_call_dispatched
        ),
        c!(
            "subprocess.reverse_kernel_call_principal_locked",
            ["subprocess", "outbound", "network"],
            crate::conformance::network::subprocess_reverse_kernel_call_principal_locked
        ),
        c!(
            "subprocess.reverse_stream_chunks_piped",
            ["subprocess", "outbound", "network", "stream"],
            crate::conformance::network::subprocess_reverse_stream_chunks_piped
        ),
        c!(
            "sse_parser.basic_smoke",
            ["outbound", "network", "stream", "sse"],
            crate::conformance::network::sse_parser_basic_smoke
        ),
        c!(
            "sse_parser.partial_chunks",
            ["outbound", "network", "stream", "sse"],
            crate::conformance::network::sse_parser_partial_chunks
        ),
    ]
}

pub(super) fn streaming_cases() -> Vec<ConformanceCase> {
    vec![
        // --- streaming ---
        c!(
            "stream.normal_lifecycle",
            ["stream"],
            crate::conformance::streaming::stream_normal_lifecycle
        ),
        c!(
            "stream.cancel_blocks_chunks",
            ["stream"],
            crate::conformance::streaming::stream_cancel_blocks_chunks
        ),
        c!(
            "stream.timeout_blocks_chunks",
            ["stream"],
            crate::conformance::streaming::stream_timeout_blocks_chunks
        ),
        c!(
            "stream.error_terminal",
            ["stream"],
            crate::conformance::streaming::stream_error_terminal
        ),
        c!(
            "stream.non_streaming_rejected",
            ["stream"],
            crate::conformance::streaming::stream_non_streaming_rejected
        ),
        c!(
            "stream.no_model_agent_methods",
            ["stream"],
            crate::conformance::streaming::stream_no_model_agent_methods
        ),
        c!(
            "stream.protocol_dispatch",
            ["stream", "protocol"],
            crate::conformance::streaming::stream_protocol_dispatch
        ),
    ]
}

pub(super) fn live_model_cases() -> Vec<ConformanceCase> {
    vec![
        // --- live model ---
        c!(
            "outbound.secret_headers_parsed",
            ["outbound", "network", "live", "secret"],
            crate::conformance::live_model::outbound_secret_headers_parsed
        ),
        c!(
            "outbound.live_loopback_secret_injection",
            ["outbound", "network", "live", "secret"],
            crate::conformance::live_model::outbound_live_loopback_secret_injection
        ),
        c!(
            "stream.sse_normalize_deepseek_canary",
            ["stream", "live"],
            crate::conformance::live_model::stream_sse_normalize_deepseek_canary
        ),
        c!(
            "live_model.default_disabled_when_env_unset",
            ["live", "outbound", "network"],
            crate::conformance::live_model::live_model_default_disabled_when_env_unset
        ),
        c!(
            "live_model.smoke_skipped_in_default_run",
            ["live", "outbound", "network"],
            crate::conformance::live_model::live_model_smoke_skipped_in_default_run
        ),
        c!(
            "outbound.live_deepseek_opt_in",
            ["outbound", "network", "live"],
            crate::conformance::live_model::outbound_live_deepseek_opt_in
        ),
        c!(
            "canary.deepseek_profile_shape",
            ["live", "outbound"],
            crate::conformance::live_model::canary_deepseek_profile_shape
        ),
    ]
}

pub(super) fn live_model_providers_cases() -> Vec<ConformanceCase> {
    vec![
        // --- live model providers ---
        c!(
            "outbound.openai_chat_loopback",
            ["outbound", "network", "live"],
            crate::conformance::live_model::openai_chat_loopback
        ),
        c!(
            "outbound.openai_responses_loopback",
            ["outbound", "network", "live"],
            crate::conformance::live_model::openai_responses_loopback
        ),
        c!(
            "outbound.anthropic_messages_loopback",
            ["outbound", "network", "live"],
            crate::conformance::live_model::anthropic_messages_loopback
        ),
        c!(
            "outbound.gemini_generate_content_loopback",
            ["outbound", "network", "live"],
            crate::conformance::live_model::gemini_generate_content_loopback
        ),
        c!(
            "outbound.missing_secret_fails_closed",
            ["outbound", "network", "live", "secret"],
            crate::conformance::live_model::missing_secret_fails_closed
        ),
        c!(
            "outbound.provider_normalize_request_alignment",
            ["outbound", "network", "live"],
            crate::conformance::live_model::provider_normalize_request_alignment
        ),
        c!(
            "outbound.no_raw_secret_leak_all_providers",
            ["outbound", "network", "live", "secret"],
            crate::conformance::live_model::no_raw_secret_leak_all_providers
        ),
        c!(
            "outbound.static_headers_safe_allowlist",
            ["outbound", "network", "live"],
            crate::conformance::live_model::static_headers_safe_allowlist
        ),
        c!(
            "outbound.static_headers_block_secrets",
            ["outbound", "network", "live", "secret"],
            crate::conformance::live_model::static_headers_block_secrets
        ),
    ]
}

pub(super) fn live_model_quirks_cases() -> Vec<ConformanceCase> {
    vec![
        // --- live model quirks ---
        c!(
            "outbound.openrouter_loopback_headers",
            ["outbound", "network", "live"],
            crate::conformance::live_model::openrouter_loopback_headers
        ),
        c!(
            "outbound.xai_loopback",
            ["outbound", "network", "live"],
            crate::conformance::live_model::xai_loopback
        ),
        c!(
            "outbound.fireworks_loopback",
            ["outbound", "network", "live"],
            crate::conformance::live_model::fireworks_loopback
        ),
        c!(
            "stream.deepseek_reasoning_stream",
            ["stream", "live"],
            crate::conformance::live_model::deepseek_reasoning_stream
        ),
        c!(
            "stream.openrouter_midstream_error",
            ["stream", "live"],
            crate::conformance::live_model::openrouter_midstream_error
        ),
        c!(
            "outbound.provider_quirk_fixtures_no_secrets",
            ["outbound", "network", "live", "secret"],
            crate::conformance::live_model::provider_quirk_fixtures_no_secrets
        ),
        c!(
            "outbound.static_headers_openrouter_safe",
            ["outbound", "network", "live"],
            crate::conformance::live_model::static_headers_openrouter_safe
        ),
    ]
}
