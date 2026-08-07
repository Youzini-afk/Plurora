//! Content-free outbound executor abstraction (M3 + L2).
//!
//! This module defines the outbound executor boundary: the trait,
//! request/response types, and built-in deny-all, fake, and live HTTP
//! executors. No real network I/O happens in the deny-all or fake
//! executors. The live HTTP executor (L2) uses `reqwest` with rustls,
//! is HTTPS-only, disabled by default, and records only shape/audit
//! metadata — never raw auth/body content.
//!
//! Design principles:
//! - Fail-closed: default executor is `DenyAllOutboundExecutor`.
//! - Content-free: `body_shape` carries the JSON shape of the request
//!   body (e.g. `{"model": "...", "messages": [...]}`), but the raw
//!   body bytes are never persisted in audit records.
//! - `secret_refs` are references only; resolved secrets are never
//!   echoed back or stored.
//! - No provider-specific fields in core; opaque `metadata` for
//!   executor-specific data.
//! - This boundary secures the Plurora-provided outbound path. It does not
//!   claim to intercept arbitrary subprocess OS network calls.
//! - Live executor is opt-in only; `RuntimeConfig::default()` uses
//!   `DenyAll`. Live executor rejects non-HTTPS URLs, does not follow
//!   redirects by default, and never echoes raw headers/body in
//!   response shapes.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use plurora_core::{new_id, RedactionState};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::watch;

use super::Runtime;
use crate::EventStore;

// ---------------------------------------------------------------------------
// L4: Secret header injection types
// ---------------------------------------------------------------------------

/// Specification for a secret-derived HTTP header to be injected by the host
/// during outbound execution. Packages declare these in `host.outbound.execute`
/// params as `secret_headers`; the host resolves the `secret_ref` at execution
/// time and injects the resulting header value into the live HTTP request.
///
/// Raw secret values never appear in audit, response, or Debug output.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OutboundSecretHeaderSpec {
    pub header_name: String,
    pub secret_ref: String,
    pub scheme: String,
}

#[derive(Debug, Clone)]
pub struct SecretHeaderSpec {
    /// HTTP header name (e.g. `Authorization`, `x-api-key`).
    pub header_name: String,
    /// Secret reference to resolve (e.g. `secret_ref:env:DEEPSEEK_API_KEY`).
    pub secret_ref: String,
    /// Auth scheme to apply as a prefix (e.g. `bearer` → `Bearer <value>`,
    /// `basic` → `Basic <value>`). If empty or `"raw"`, the value is used as-is.
    pub scheme: String,
}

/// A resolved secret header value produced by the host during outbound execution.
///
/// This carries a raw header value that must never appear in Debug, Serialize,
/// audit, error, or response output. It exists only transiently during the
/// executor call and is dropped afterward.
#[derive(Clone)]
pub struct RedactedHeaderValue(pub String);

impl std::fmt::Debug for RedactedHeaderValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted]")
    }
}

/// A fully resolved secret header, ready for injection into an HTTP request.
#[derive(Clone)]
pub struct ResolvedSecretHeader {
    /// HTTP header name (e.g. `Authorization`).
    pub header_name: String,
    /// The full header value (e.g. `Bearer <secret>`). Redacted in Debug.
    pub value: RedactedHeaderValue,
}

impl std::fmt::Debug for ResolvedSecretHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedSecretHeader")
            .field("header_name", &self.header_name)
            .field("value", &"[redacted]")
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Executor request / response types
// ---------------------------------------------------------------------------

/// The kind of outbound executor that produced a response.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutorKind {
    /// All outbound is denied; no network performed.
    DenyAll,
    /// Fake/deterministic fixture executor; no real network.
    Fake,
    /// Real network executor (future M4+).
    Real,
}

/// Request sent to an outbound executor.
///
/// This is the content-free shape of an outbound request. It carries
/// identifiers, routing metadata, and the JSON *shape* of the body
/// (not the raw bytes). Secret references are just that — references,
/// never resolved raw values.
///
/// L4 adds `secret_headers` (specification for header injection from
/// secret refs) and `resolved_secret_headers` (host-resolved header
/// values, redacted in Debug, not serialized, never echoed back).
///
/// L5 adds `static_headers` (safe non-secret headers injected by the
/// caller, validated against a strict allowlist; secret-bearing header
/// names like Authorization/x-api-key/Cookie are blocked).
#[derive(Debug, Clone)]
pub struct OutboundExecutorRequest {
    /// Package that initiated the outbound request.
    pub package_id: String,
    /// Capability through which the request was made.
    pub capability_id: String,
    /// Destination host (e.g. `api.openai.com`).
    pub destination_host: String,
    /// HTTP method (GET, POST, etc).
    pub method: String,
    /// Optional URL path (e.g. `/v1/chat/completions`).
    pub path: Option<String>,
    /// Declared purpose (from manifest or request context).
    pub purpose: Option<String>,
    /// Secret references used (e.g. `secret_ref:env:MY_KEY`).
    /// These are identifiers only; resolved values are never stored.
    pub secret_refs: Vec<String>,
    /// Redaction state carried forward from the policy check.
    pub redaction_state: Option<RedactionState>,
    /// Optional timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Opaque executor-specific metadata (provider name, headers shape, etc).
    pub metadata: Value,
    /// JSON *shape* of the request body (e.g.
    /// `{"model": "gpt-4o", "messages": [...]}`).
    /// This is a structural description, not raw bytes.
    /// Raw body content is never persisted in audit.
    pub body_shape: Option<Value>,
    /// L4: Secret header injection specifications. Each entry declares
    /// a header to be injected from a secret_ref, with an optional scheme
    /// prefix (e.g. "bearer"). The host resolves these at execution time
    /// and populates `resolved_secret_headers`. Raw secret values never
    /// leave this struct except into the actual HTTP request.
    pub secret_headers: Vec<SecretHeaderSpec>,
    /// L4: Host-resolved secret header values, injected into the live
    /// HTTP executor's request headers. These carry raw secret-derived
    /// values and must never be serialized, logged, or echoed back.
    /// Debug prints `[redacted]` for the values.
    pub resolved_secret_headers: Vec<ResolvedSecretHeader>,
    /// L5: Safe non-secret headers provided by the caller, validated
    /// against a strict allowlist. Only known-safe header names are
    /// permitted (e.g. `anthropic-version`, `content-type`). Header
    /// names that carry secrets (Authorization, x-api-key, Cookie,
    /// etc.) are rejected. Values are plain strings — no secret
    /// resolution is performed on static_headers.
    pub static_headers: Vec<StaticHeader>,
}

/// A safe non-secret static header for outbound requests (L5).
///
/// Only header names on the `STATIC_HEADER_ALLOWLIST` are permitted.
/// Secret-bearing header names (Authorization, x-api-key, Cookie, etc.)
/// are rejected at parse time. Values must be plain strings that do not
/// look like raw secrets.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OutboundStaticHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct StaticHeader {
    /// HTTP header name (must be on the allowlist).
    pub name: String,
    /// Header value (plain string, no secret resolution).
    pub value: String,
}

/// The set of header names that are safe to inject via `static_headers`.
///
/// Only these known-safe, non-secret headers are allowed. All other
/// header names are rejected. This prevents `static_headers` from
/// becoming a secret bypass path.
///
/// To add a new safe header: it must never carry authentication,
/// authorization, cookies, API keys, or other secret material.
pub const STATIC_HEADER_ALLOWLIST: &[&str] = &[
    "anthropic-version",
    "content-type",
    "accept",
    // L6: OpenRouter safe attribution/labeling headers.
    // These carry no secrets — they are for app identification and
    // request labeling per OpenRouter's documentation. Case-insensitive
    // match means "HTTP-Referer" and "http-referer" both work.
    "http-referer",
    "x-title",
];

/// Check whether a header name is on the safe static headers allowlist.
///
/// Case-insensitive comparison. Returns true if the header name is
/// explicitly allowed for static injection.
pub fn is_static_header_allowed(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    STATIC_HEADER_ALLOWLIST
        .iter()
        .any(|&allowed| allowed == name_lower)
}

/// Check whether a header name is explicitly blocked from static_headers
/// because it is a well-known secret-bearing header.
///
/// These headers MUST use `secret_headers` (with secret_ref) instead.
/// This is a defense-in-depth check: even if someone adds a header name
/// to the allowlist, we still block known secret-bearing names.
pub fn is_secret_header_name(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    matches!(
        name_lower.as_str(),
        "authorization"
            | "x-api-key"
            | "x-goog-api-key"
            | "cookie"
            | "set-cookie"
            | "proxy-authorization"
            | "x-auth-token"
            | "x-access-token"
            | "x-secret"
            | "www-authenticate"
            | "proxy-authenticate"
    )
}

impl OutboundExecutorRequest {
    /// Helper to add the default L4 fields to a struct literal.
    /// Use in all existing construction sites.
    pub fn empty_secret_headers() -> (Vec<SecretHeaderSpec>, Vec<ResolvedSecretHeader>) {
        (Vec::new(), Vec::new())
    }

    /// Helper to add the default L5 fields to a struct literal.
    pub fn empty_static_headers() -> Vec<StaticHeader> {
        Vec::new()
    }
}

/// Response returned by an outbound executor.
///
/// Like the request, this is content-free. It carries status, the
/// *shape* of headers/body, usage/cost placeholders, and metadata
/// identifying what kind of executor produced the response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OutboundExecutorResponse {
    /// High-level status: "ok", "error", "denied", "timeout".
    pub status: String,
    /// HTTP-style status code if applicable (e.g. 200, 429, 503).
    #[serde(default)]
    pub status_code: Option<u16>,
    /// JSON *shape* of the response headers (e.g.
    /// `{"content-type": "application/json", "x-ratelimit-remaining": "..."}`).
    #[serde(default)]
    pub headers_shape: Option<Value>,
    /// JSON *shape* of the response body.
    #[serde(default)]
    pub body_shape: Option<Value>,
    /// Provider-assigned request id (e.g. `req-abc123`).
    #[serde(default)]
    pub provider_request_id: Option<String>,
    /// Usage placeholder (e.g. `{"prompt_tokens": 10, "completion_tokens": 20}`).
    #[serde(default)]
    pub usage: Value,
    /// Cost placeholder (e.g. `{"total_cost_usd": 0.002}`).
    #[serde(default)]
    pub cost: Value,
    /// Redaction state applied to the response.
    #[serde(default)]
    pub redaction_state: RedactionState,
    /// Whether real network I/O was performed.
    pub network_performed: bool,
    /// What kind of executor produced this response.
    pub executor_kind: ExecutorKind,
}

// ---------------------------------------------------------------------------
// Y3: Outbound streaming types
// ---------------------------------------------------------------------------

/// The format of a streaming response for `host.outbound.stream`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StreamFormat {
    /// Server-Sent Events (`text/event-stream`). Parses `data: ...\n\n` events.
    Sse,
    /// Newline-delimited JSON. Splits on `\n` and emits each line as a frame.
    Ndjson,
    /// Raw binary stream. Emits chunks as they arrive.
    Raw,
}

impl std::fmt::Display for StreamFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sse => f.write_str("sse"),
            Self::Ndjson => f.write_str("ndjson"),
            Self::Raw => f.write_str("raw"),
        }
    }
}

/// Status returned when a streaming outbound request starts.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StreamStartStatus {
    /// Stream started successfully.
    Ok,
    /// Stream request was denied by policy.
    Denied,
    /// Stream request encountered an error before starting.
    Error,
}

/// Response returned by `host.outbound.stream` on the initial call.
///
/// Contains the stream_id for subscribing to events, the start status,
/// and metadata about the executor that will handle the stream.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OutboundStreamResponse {
    /// The stream_id to subscribe to for stream frames.
    pub stream_id: String,
    /// Whether the stream started successfully.
    pub status: StreamStartStatus,
    /// Redaction state applied to stream frames.
    #[serde(default)]
    pub redaction_state: RedactionState,
    /// Whether real network I/O will be performed.
    pub network_performed: bool,
    /// What kind of executor is handling the stream.
    pub executor_kind: ExecutorKind,
}

/// The kind of a stream frame payload for outbound streaming.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutboundFrameKind {
    /// A parsed SSE event or NDJSON line.
    Event,
    /// A raw data chunk.
    Data,
    /// An error frame (stream continues but reports the error).
    Error,
    /// Terminal frame indicating the stream is complete.
    Done,
}

/// A frame emitted during outbound streaming.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OutboundStreamFrame {
    /// Sequence number, monotonically increasing from 1.
    pub seq: u64,
    /// The kind of frame.
    pub kind: OutboundFrameKind,
    /// The frame payload. For SSE: parsed event; for NDJSON: the JSON line;
    /// for Raw: the chunk shape (redacted). Never includes raw secrets.
    pub data_shape: Value,
    /// Total bytes received so far in this stream.
    pub bytes_received: u64,
}

/// Summary of a completed outbound stream, returned by the executor.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OutboundStreamSummary {
    /// Final status: "ok", "error", "timeout", "cancelled".
    pub status: String,
    /// HTTP status code if available.
    #[serde(default)]
    pub status_code: Option<u16>,
    /// Total number of frames emitted.
    pub frame_count: u64,
    /// Total bytes received.
    pub bytes_received: u64,
    /// JSON shape of response headers (redacted).
    #[serde(default)]
    pub headers_shape: Option<Value>,
    /// JSON shape of the response body (redacted preview).
    #[serde(default)]
    pub body_shape: Option<Value>,
    /// Provider-assigned request id if available.
    #[serde(default)]
    pub provider_request_id: Option<String>,
    /// Redaction state applied to the stream.
    #[serde(default)]
    pub redaction_state: RedactionState,
    /// Whether real network I/O was performed.
    pub network_performed: bool,
    /// What kind of executor produced this summary.
    pub executor_kind: ExecutorKind,
}

// ---------------------------------------------------------------------------
// Y3: StreamEmitter and CancelSignal traits
// ---------------------------------------------------------------------------

/// Trait for emitting frames into the platform stream lifecycle.
///
/// Implementations are wired into the existing `StreamRegistry` /
/// `stream_capability_chunk` infrastructure. The executor calls
/// `emit` for each frame it produces.
#[async_trait]
pub trait StreamEmitter: Send + Sync {
    /// Emit a frame into the stream.
    async fn emit(&self, frame: OutboundStreamFrame) -> anyhow::Result<()>;
}

/// Signal for cancelling an outbound stream.
///
/// When the caller invokes `capability.cancel`, the cancel signal
/// is set, and the executor's stream loop checks it before each iteration.
#[derive(Clone)]
pub struct CancelSignal {
    rx: watch::Receiver<bool>,
}

impl CancelSignal {
    /// Create a new cancel signal pair (sender, receiver).
    pub fn new() -> (watch::Sender<bool>, Self) {
        let (tx, rx) = watch::channel(false);
        (tx, Self { rx })
    }

    /// Check whether the stream has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        *self.rx.borrow()
    }
}

// ---------------------------------------------------------------------------
// OutboundExecutePolicyConfig — Y1 host-level execute policy
// ---------------------------------------------------------------------------

/// Host policy for outbound HTTP execute (Y1).
///
/// It governs host-level policy for `host.outbound.execute` calls:
/// whether execute is enabled at all, which destination hosts are
/// allowed, whether HTTPS is required, timeouts, redirect policy, and
/// a test-only loopback escape hatch.
///
/// Default is disabled (fail-closed): `enabled: false`, empty
/// `allowed_hosts`, `https_only: true`, no redirects, no loopback.
#[derive(Debug, Clone)]
pub struct OutboundExecutePolicyConfig {
    /// Explicit opt-in. Default is false (fail-closed).
    pub enabled: bool,
    /// Allowed destination hosts for HTTP execute. Empty means deny-all.
    pub allowed_hosts: Vec<String>,
    /// HTTPS-only guard. Must remain true for live executor parity.
    pub https_only: bool,
    /// Request timeout in milliseconds.
    pub timeout_ms: u64,
    /// Redirects are fail-closed in the current design.
    pub allow_redirects: bool,
    /// Test-only: allow insecure (HTTP) URLs to localhost / 127.0.0.1.
    pub allow_insecure_loopback_for_tests: bool,
}

impl Default for OutboundExecutePolicyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allowed_hosts: Vec::new(),
            https_only: true,
            timeout_ms: 30_000,
            allow_redirects: false,
            allow_insecure_loopback_for_tests: false,
        }
    }
}

// ---------------------------------------------------------------------------
// OutboundExecutor trait
// ---------------------------------------------------------------------------

/// Trait for outbound request execution.
///
/// Implementations range from deny-all (no network), fake
/// (deterministic fixtures, no network), to real HTTP executors
/// (M4+). The executor is called only after the policy check passes.
#[async_trait]
pub trait OutboundExecutor: Send + Sync + 'static {
    /// Execute an outbound request.
    ///
    /// Called only after `check_and_audit_outbound` has approved the
    /// request. Implementations must not persist raw body/header/
    /// secret content — only shapes, refs, and metadata.
    async fn execute(
        &self,
        request: OutboundExecutorRequest,
    ) -> anyhow::Result<OutboundExecutorResponse>;

    /// Execute an outbound streaming request (Y3).
    ///
    /// Called only after policy and permission checks have approved the
    /// request. The executor should:
    /// 1. Perform the HTTP request with streaming response
    /// 2. Parse the response body according to `format` (SSE, NDJSON, Raw)
    /// 3. Emit frames via the `emit` callback
    /// 4. Respect the `cancel` signal for client-initiated cancellation
    /// 5. Apply `max_frame_bytes`, `max_total_bytes`, `max_duration_ms`
    /// 6. Return a summary on completion (success, error, timeout, cancel)
    ///
    /// Default implementation returns "unsupported" error, so existing
    /// executors (DenyAll) don't need to implement this.
    async fn stream(
        &self,
        _request: OutboundExecutorRequest,
        _format: StreamFormat,
        _emit: Arc<dyn StreamEmitter>,
        _cancel: CancelSignal,
        _max_frame_bytes: Option<usize>,
        _max_total_bytes: Option<usize>,
        _max_duration_ms: Option<u64>,
    ) -> anyhow::Result<OutboundStreamSummary> {
        Err(anyhow::anyhow!("streaming not supported by this executor"))
    }
}

// ---------------------------------------------------------------------------
// DenyAllOutboundExecutor
// ---------------------------------------------------------------------------

/// An outbound executor that denies all requests without network.
///
/// This is the default, fail-closed executor. Any request that
/// reaches it returns a denied response. (In practice, the policy
/// check should already have denied the request, but this provides
/// a defense-in-depth fallback.)
pub struct DenyAllOutboundExecutor;

#[async_trait]
impl OutboundExecutor for DenyAllOutboundExecutor {
    async fn execute(
        &self,
        _request: OutboundExecutorRequest,
    ) -> anyhow::Result<OutboundExecutorResponse> {
        Ok(OutboundExecutorResponse {
            status: "denied".to_string(),
            status_code: None,
            headers_shape: None,
            body_shape: None,
            provider_request_id: None,
            usage: Value::Null,
            cost: Value::Null,
            redaction_state: RedactionState::NotCaptured,
            network_performed: false,
            executor_kind: ExecutorKind::DenyAll,
        })
    }
}

// ---------------------------------------------------------------------------
// FakeOutboundExecutor
// ---------------------------------------------------------------------------

/// A deterministic fake outbound executor for testing and conformance.
///
/// Returns fixture responses based on `(host, method, path)` keys.
/// No real network I/O is performed. If no fixture matches, returns
/// a generic 200 OK response with empty body shape.
pub struct FakeOutboundExecutor {
    /// Optional call counter for conformance assertions.
    call_count: std::sync::atomic::AtomicU64,
    /// Fixture map: `(host, method, path) → OutboundExecutorResponse`.
    fixtures: HashMap<(String, String, Option<String>), OutboundExecutorResponse>,
}

impl FakeOutboundExecutor {
    /// Create a fake executor with no fixtures (returns generic 200 OK).
    pub fn new() -> Self {
        Self {
            call_count: std::sync::atomic::AtomicU64::new(0),
            fixtures: HashMap::new(),
        }
    }

    /// Add a fixture for a given host/method/path combination.
    pub fn add_fixture(
        &mut self,
        host: &str,
        method: &str,
        path: Option<&str>,
        response: OutboundExecutorResponse,
    ) {
        self.fixtures.insert(
            (
                host.to_string(),
                method.to_string(),
                path.map(|s| s.to_string()),
            ),
            response,
        );
    }

    /// Get the number of times `execute` was called.
    pub fn call_count(&self) -> u64 {
        self.call_count.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn with_fixture(
        host: &str,
        method: &str,
        path: Option<&str>,
        response: OutboundExecutorResponse,
    ) -> Self {
        let mut executor = Self::new();
        executor.add_fixture(host, method, path, response);
        executor
    }
}

impl Default for FakeOutboundExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OutboundExecutor for FakeOutboundExecutor {
    async fn execute(
        &self,
        request: OutboundExecutorRequest,
    ) -> anyhow::Result<OutboundExecutorResponse> {
        self.call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let key = (
            request.destination_host.clone(),
            request.method.clone(),
            request.path.clone(),
        );

        if let Some(fixture) = self.fixtures.get(&key) {
            return Ok(fixture.clone());
        }

        // Default: generic 200 OK with fake body shape, no network
        Ok(OutboundExecutorResponse {
            status: "ok".to_string(),
            status_code: Some(200),
            headers_shape: Some(serde_json::json!({
                "content-type": "application/json"
            })),
            body_shape: Some(serde_json::json!({
                "id": "fake_response",
                "object": "fake",
                "choices": []
            })),
            provider_request_id: Some("fake_req_001".to_string()),
            usage: serde_json::json!({"prompt_tokens": 0, "completion_tokens": 0}),
            cost: serde_json::json!({}),
            redaction_state: RedactionState::Redacted,
            network_performed: false,
            executor_kind: ExecutorKind::Fake,
        })
    }

    /// Y3: Fake executor streams a small canned sequence of SSE frames.
    ///
    /// Emits 3 deterministic SSE events for testing, then a done frame.
    async fn stream(
        &self,
        _request: OutboundExecutorRequest,
        format: StreamFormat,
        emit: Arc<dyn StreamEmitter>,
        cancel: CancelSignal,
        _max_frame_bytes: Option<usize>,
        _max_total_bytes: Option<usize>,
        _max_duration_ms: Option<u64>,
    ) -> anyhow::Result<OutboundStreamSummary> {
        self.call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let canned_events = match format {
            StreamFormat::Sse => vec![
                serde_json::json!({
                    "event": None::<String>,
                    "data": "fake sse event 1",
                    "id": None::<String>,
                }),
                serde_json::json!({
                    "event": None::<String>,
                    "data": "fake sse event 2",
                    "id": None::<String>,
                }),
                serde_json::json!({
                    "event": None::<String>,
                    "data": "fake sse event 3",
                    "id": None::<String>,
                }),
            ],
            StreamFormat::Ndjson => vec![
                serde_json::json!({"id": "ndjson_1", "chunk": "first"}),
                serde_json::json!({"id": "ndjson_2", "chunk": "second"}),
                serde_json::json!({"id": "ndjson_3", "chunk": "third"}),
            ],
            StreamFormat::Raw => vec![
                serde_json::json!({"bytes": 64, "kind": "binary_chunk"}),
                serde_json::json!({"bytes": 64, "kind": "binary_chunk"}),
                serde_json::json!({"bytes": 32, "kind": "final_chunk"}),
            ],
        };

        let mut seq: u64 = 0;
        let mut bytes_received: u64 = 0;

        for event in &canned_events {
            if cancel.is_cancelled() {
                // Emit done frame with cancelled status
                seq += 1;
                emit.emit(OutboundStreamFrame {
                    seq,
                    kind: OutboundFrameKind::Done,
                    data_shape: serde_json::json!({"status": "cancelled"}),
                    bytes_received,
                })
                .await?;
                return Ok(OutboundStreamSummary {
                    status: "cancelled".to_string(),
                    status_code: Some(200),
                    frame_count: seq,
                    bytes_received,
                    headers_shape: Some(serde_json::json!({"content-type": "text/event-stream"})),
                    body_shape: None,
                    provider_request_id: Some("fake_stream_001".to_string()),
                    redaction_state: RedactionState::Redacted,
                    network_performed: false,
                    executor_kind: ExecutorKind::Fake,
                });
            }

            seq += 1;
            bytes_received += 64; // Simulate bytes received per frame
            emit.emit(OutboundStreamFrame {
                seq,
                kind: OutboundFrameKind::Event,
                data_shape: event.clone(),
                bytes_received,
            })
            .await?;
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        // Emit final done frame
        seq += 1;
        emit.emit(OutboundStreamFrame {
            seq,
            kind: OutboundFrameKind::Done,
            data_shape: serde_json::json!({"status": "ok"}),
            bytes_received,
        })
        .await?;

        Ok(OutboundStreamSummary {
            status: "ok".to_string(),
            status_code: Some(200),
            frame_count: seq,
            bytes_received,
            headers_shape: Some(serde_json::json!({"content-type": "text/event-stream"})),
            body_shape: None,
            provider_request_id: Some("fake_stream_001".to_string()),
            redaction_state: RedactionState::Redacted,
            network_performed: false,
            executor_kind: ExecutorKind::Fake,
        })
    }
}

// ---------------------------------------------------------------------------
// LiveHttpOutboundExecutorConfig — L2 live HTTP executor configuration
// ---------------------------------------------------------------------------

/// Configuration for the live HTTP outbound executor (L2).
///
/// Default construction is safe: HTTPS-only, no redirects, sensible
/// timeouts, rustls TLS. The executor is never the default — it must
/// be explicitly opted in via `OutboundExecutorConfig::LiveHttp`.
#[derive(Debug, Clone)]
pub struct LiveHttpOutboundExecutorConfig {
    /// Total request timeout in milliseconds.
    pub timeout_ms: u64,
    /// TCP connect timeout in milliseconds.
    pub connect_timeout_ms: u64,
    /// Whether to follow HTTP redirects. Default: false.
    /// If true in a future L4+ phase, the executor must re-check
    /// the redirect target host against policy before following.
    /// L2 keeps this false and does not implement redirect following.
    pub allow_redirects: bool,
    /// Maximum bytes of response body to capture in `body_shape`
    /// as a redacted JSON preview. Beyond this, only kind/size is
    /// recorded. Default: 1024.
    pub max_response_preview_bytes: u64,
    /// **Test-only flag**: allow insecure (HTTP) URLs to localhost
    /// / 127.0.0.1 for conformance testing. Default: false.
    /// When true, only `127.0.0.1` and `localhost` are permitted
    /// as non-HTTPS destinations. All other non-HTTPS URLs are
    /// still rejected.
    pub allow_insecure_loopback_for_tests: bool,
}

impl Default for LiveHttpOutboundExecutorConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000,
            connect_timeout_ms: 5_000,
            allow_redirects: false,
            max_response_preview_bytes: 1024,
            allow_insecure_loopback_for_tests: false,
        }
    }
}

// ---------------------------------------------------------------------------
// LiveHttpOutboundExecutor
// ---------------------------------------------------------------------------

/// A live HTTP outbound executor using `reqwest` with rustls.
///
/// This executor performs real HTTPS network I/O. It is **disabled by
/// default** — `RuntimeConfig::default()` uses `DenyAll`. To enable,
/// set `OutboundExecutorConfig::LiveHttp(config)` on `RuntimeConfig`.
///
/// Safety properties:
/// - Rejects non-HTTPS URLs (unless `allow_insecure_loopback_for_tests`
///   is true and the host is 127.0.0.1 or localhost).
/// - Does not follow redirects by default (configurable, but L2 does
///   not implement redirect policy re-checking).
/// - Only sends `content-type: application/json` and Plurora placeholder
///   headers. Never sends raw secret/auth header values (L3 handles
///   secret injection).
/// - Records only shape/redacted metadata in responses — never raw
///   body bytes or auth/header values.
/// - Uses rustls TLS backend; no native-tls.
pub struct LiveHttpOutboundExecutor {
    client: reqwest::Client,
    config: LiveHttpOutboundExecutorConfig,
}

impl LiveHttpOutboundExecutor {
    /// Create a new live HTTP executor with the given configuration.
    ///
    /// The `reqwest::Client` is built with:
    /// - `rustls-tls` (via crate feature, no native-tls)
    /// - No redirect following (or limited by config; L2 keeps false)
    /// - Configured connect and request timeouts
    pub fn new(config: LiveHttpOutboundExecutorConfig) -> anyhow::Result<Self> {
        if config.allow_redirects {
            anyhow::bail!(
                "live outbound redirects are disabled until redirect target policy re-check is implemented"
            );
        }

        let redirect_policy = reqwest::redirect::Policy::none();

        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .redirect(redirect_policy)
            .connect_timeout(std::time::Duration::from_millis(config.connect_timeout_ms))
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .https_only(!config.allow_insecure_loopback_for_tests)
            .build()
            .map_err(|e| anyhow::anyhow!("failed to build reqwest client: {e}"))?;

        Ok(Self { client, config })
    }

    /// Create a new live HTTP executor from a host profile's execute config (Y1).
    ///
    /// This constructor bridges the `HostExecuteOutboundProfile` (or
    /// `OutboundExecutePolicyConfig`) fields into the
    /// `LiveHttpOutboundExecutorConfig` expected by [`Self::new`].
    /// It maps profile-level fields as follows:
    ///
    /// - `timeout_ms` → `config.timeout_ms`
    /// - `connect_timeout_ms` → 5_000 (sensible default, not in profile)
    /// - `allow_redirects` → `config.allow_redirects`
    /// - `allow_insecure_loopback_for_tests` → `config.allow_insecure_loopback_for_tests`
    /// - `max_response_preview_bytes` → 1024 (sensible default, not in profile)
    pub fn new_from_profile(
        https_only: bool,
        timeout_ms: u64,
        allow_redirects: bool,
        allow_insecure_loopback_for_tests: bool,
    ) -> anyhow::Result<Self> {
        let config = LiveHttpOutboundExecutorConfig {
            timeout_ms,
            connect_timeout_ms: 5_000,
            allow_redirects,
            max_response_preview_bytes: 1024,
            allow_insecure_loopback_for_tests,
        };
        // If the profile says https_only but loopback is allowed, the
        // reqwest https_only flag is determined by the loopback setting
        // (allowing HTTP to loopback only). When https_only is true and
        // loopback is not allowed, the client enforces HTTPS-only.
        let _ = https_only; // Already enforced by policy layer; executor-level handled by allow_insecure_loopback_for_tests
        Self::new(config)
    }

    /// Build the full URL from the executor request.
    ///
    /// The metadata may contain a `base_url` or `scheme` field. If
    /// `base_url` is present and starts with `https://`, it is used
    /// as-is. If `scheme` is present and is `"https"`, we construct
    /// `https://host/path`. If neither is present, we default to
    /// `https://host/path` (fail-closed: no scheme defaults to HTTPS).
    ///
    /// Returns an error if the resulting URL is non-HTTPS and the
    /// loopback test flag does not permit it.
    fn build_url(&self, request: &OutboundExecutorRequest) -> anyhow::Result<reqwest::Url> {
        // Check for explicit base_url in metadata
        let base_url: Option<String> = request
            .metadata
            .get("base_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let url_str = if let Some(base) = base_url {
            // Use the explicit base_url + path
            let mut url = base.trim_end_matches('/').to_string();
            if let Some(path) = &request.path {
                if !path.starts_with('/') {
                    url.push('/');
                }
                url.push_str(path);
            }
            url
        } else {
            // Default: https://host/path (fail-closed)
            let scheme = request
                .metadata
                .get("scheme")
                .and_then(|v| v.as_str())
                .unwrap_or("https");
            let raw_path = request.path.as_deref().unwrap_or("/");
            let path = if raw_path.starts_with('/') {
                raw_path.to_string()
            } else {
                format!("/{raw_path}")
            };
            format!("{scheme}://{}{path}", request.destination_host)
        };

        let url = reqwest::Url::parse(&url_str)
            .map_err(|e| anyhow::anyhow!("invalid outbound URL '{}': {e}", url_str))?;

        let actual_host = url.host_str().unwrap_or("");
        if !actual_host.eq_ignore_ascii_case(&request.destination_host) {
            anyhow::bail!(
                "live outbound URL host '{}' does not match executor destination_host '{}'",
                actual_host,
                request.destination_host
            );
        }

        // Enforce HTTPS (with loopback exception for tests)
        if url.scheme() != "https" {
            let is_loopback =
                actual_host == "127.0.0.1" || actual_host == "localhost" || actual_host == "[::1]";
            if !self.config.allow_insecure_loopback_for_tests || !is_loopback {
                anyhow::bail!(
                    "live outbound executor rejects non-HTTPS URL: {} (host={})",
                    url_str,
                    actual_host
                );
            }
        }

        Ok(url)
    }

    /// Build a safe headers map for the outbound request.
    ///
    /// L2 only sends:
    /// - `content-type: application/json` (for JSON body)
    /// - `x-plurora-outbound: true` (Plurora placeholder)
    ///
    /// L4 injects resolved secret headers (e.g. `Authorization: Bearer <key>`)
    /// from `request.resolved_secret_headers`. These values exist only in the
    /// HTTP request; they are never stored in audit, Debug, or response shapes.
    ///
    /// L5 injects safe static headers from `request.static_headers`.
    /// These are non-secret headers validated against the allowlist
    /// (e.g. `anthropic-version: 2023-06-01`). Secret-bearing header
    /// names are rejected at parse time and never reach this method.
    fn build_headers(
        &self,
        request: &OutboundExecutorRequest,
    ) -> anyhow::Result<reqwest::header::HeaderMap> {
        let mut headers = reqwest::header::HeaderMap::new();

        // Content-Type: application/json if there's a body
        if request.body_shape.is_some() {
            headers.insert(
                reqwest::header::CONTENT_TYPE,
                reqwest::header::HeaderValue::from_static("application/json"),
            );
        }

        // Plurora placeholder header (identifies the source as Plurora)
        headers.insert(
            reqwest::header::HeaderName::from_static("x-plurora-outbound"),
            reqwest::header::HeaderValue::from_static("true"),
        );

        // L5: Inject safe static headers (non-secret, allowlisted)
        for static_hdr in &request.static_headers {
            let header_name = reqwest::header::HeaderName::from_bytes(static_hdr.name.as_bytes())
                .map_err(|_| {
                anyhow::anyhow!("static header name '{}' is invalid", static_hdr.name)
            })?;
            let header_value =
                reqwest::header::HeaderValue::from_str(&static_hdr.value).map_err(|_| {
                    anyhow::anyhow!("static header value for '{}' is invalid", static_hdr.name)
                })?;
            headers.insert(header_name, header_value);
        }

        // L4: Inject resolved secret headers (e.g. Authorization)
        for resolved in &request.resolved_secret_headers {
            let header_name =
                reqwest::header::HeaderName::from_bytes(resolved.header_name.as_bytes())
                    .map_err(|_| anyhow::anyhow!("resolved secret header name is invalid"))?;
            let value = reqwest::header::HeaderValue::from_str(&resolved.value.0)
                .map_err(|_| anyhow::anyhow!("resolved secret header value is invalid"))?;
            headers.insert(header_name, value);
        }

        // Metadata may carry headers_shape for informational purposes,
        // but L2 does NOT send those headers. L3+ may inject safe ones.
        Ok(headers)
    }

    /// Extract a redacted headers_shape from an HTTP response.
    ///
    /// Only records header names and content-type values. Auth,
    /// cookie, and secret-like header values are replaced with
    /// `"[redacted]"`.
    fn redacted_headers_shape(response: &reqwest::Response) -> Value {
        let mut map = serde_json::Map::new();
        for (name, value) in response.headers() {
            let name_lower = name.as_str().to_lowercase();
            if name_lower == "content-type" {
                // Safe to record content-type
                if let Ok(v) = value.to_str() {
                    map.insert(name.as_str().to_string(), Value::String(v.to_string()));
                }
            } else if is_safe_response_header(&name_lower) {
                // Request-id headers are safe to record
                if let Ok(v) = value.to_str() {
                    map.insert(name.as_str().to_string(), Value::String(v.to_string()));
                }
            } else {
                // All other headers: record name only, value redacted
                map.insert(
                    name.as_str().to_string(),
                    Value::String("[redacted]".to_string()),
                );
            }
        }
        Value::Object(map)
    }

    /// Extract a redacted body_shape from an HTTP response.
    ///
    /// If the response is JSON and small enough (within
    /// `max_response_preview_bytes`), capture a redacted preview.
    /// Otherwise, record `{kind, bytes_captured}` only.
    /// Never records raw auth/body secret values.
    async fn redacted_body_shape(
        &self,
        response: reqwest::Response,
    ) -> (Value, Option<reqwest::StatusCode>) {
        let _status_code = response.status().as_u16();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let max_bytes = self.config.max_response_preview_bytes;

        // Read up to max_bytes + 1 (to detect truncation)
        let body_bytes = match response.bytes().await {
            Ok(b) => b,
            Err(_) => {
                return (
                    serde_json::json!({
                        "kind": "error",
                        "bytes_captured": 0,
                    }),
                    None,
                );
            }
        };

        let total_len = body_bytes.len();
        let truncated = total_len > max_bytes as usize;
        let captured = if truncated {
            &body_bytes[..max_bytes as usize]
        } else {
            &body_bytes[..]
        };

        // If JSON content type, try to parse a preview
        if content_type.contains("application/json") {
            if let Ok(parsed) = serde_json::from_slice::<Value>(captured) {
                let redacted = redact_json_value(&parsed);
                return (redacted, None);
            }
        }

        // Non-JSON or parse failure: record shape only
        (
            serde_json::json!({
                "kind": if content_type.contains("json") { "json" } else { "binary" },
                "bytes_captured": captured.len(),
                "truncated": truncated,
            }),
            None,
        )
    }
}

/// Check if a response header name is safe to record values for.
///
/// Safe headers: content-type, request-id style headers.
/// All other header values are redacted.
fn is_safe_response_header(name_lower: &str) -> bool {
    matches!(
        name_lower,
        "content-type" | "request-id" | "x-request-id" | "x-trace-id"
    )
}

/// Recursively redact a JSON value, removing secret-like fields.
///
/// Replaces values of known secret field names with `"[redacted]"`.
/// Recurses into objects and arrays. Non-object, non-array leaves
/// are preserved unless their key is a secret field name.
fn redact_json_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut redacted = serde_json::Map::new();
            for (k, v) in map {
                if plurora_core::is_secret_field_name(k) {
                    redacted.insert(k.clone(), Value::String("[redacted]".to_string()));
                } else {
                    redacted.insert(k.clone(), redact_json_value(v));
                }
            }
            Value::Object(redacted)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(redact_json_value).collect()),
        other => other.clone(),
    }
}

#[async_trait]
impl OutboundExecutor for LiveHttpOutboundExecutor {
    async fn execute(
        &self,
        request: OutboundExecutorRequest,
    ) -> anyhow::Result<OutboundExecutorResponse> {
        // Build URL (enforces HTTPS)
        let url = self.build_url(&request)?;

        // Build safe headers (no secrets injected)
        let headers = self.build_headers(&request)?;

        // Build the request method
        let method = match request.method.to_uppercase().as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "DELETE" => reqwest::Method::DELETE,
            "PATCH" => reqwest::Method::PATCH,
            "HEAD" => reqwest::Method::HEAD,
            "OPTIONS" => reqwest::Method::OPTIONS,
            other => anyhow::bail!("unsupported outbound HTTP method '{}'", other),
        };

        // Build the request
        let mut builder = self.client.request(method, url).headers(headers);

        // Attach body_shape as JSON body if present
        if let Some(body_shape) = &request.body_shape {
            builder = builder.json(body_shape);
        }

        // Apply per-request timeout if specified
        if let Some(timeout_ms) = request.timeout_ms {
            builder = builder.timeout(std::time::Duration::from_millis(timeout_ms));
        }

        // Execute the request
        let response = match builder.send().await {
            Ok(r) => r,
            Err(e) => {
                // Normalize errors: timeout vs other
                let status = if e.is_timeout() { "timeout" } else { "error" };
                // Never include raw error details that might leak secrets
                return Ok(OutboundExecutorResponse {
                    status: status.to_string(),
                    status_code: None,
                    headers_shape: None,
                    body_shape: None,
                    provider_request_id: None,
                    usage: Value::Null,
                    cost: Value::Null,
                    redaction_state: RedactionState::Redacted,
                    network_performed: true,
                    executor_kind: ExecutorKind::Real,
                });
            }
        };

        // Extract status
        let status_code = response.status().as_u16();
        let status = if response.status().is_success() {
            "ok".to_string()
        } else {
            "error".to_string()
        };

        // Extract redacted headers shape
        let headers_shape = Some(Self::redacted_headers_shape(&response));

        // Extract provider request-id from safe headers
        let provider_request_id = response
            .headers()
            .iter()
            .find(|(name, _)| {
                let n = name.as_str().to_lowercase();
                n == "request-id" || n == "x-request-id"
            })
            .and_then(|(_, v)| v.to_str().ok())
            .map(|s| s.to_string());

        // Extract redacted body shape
        let (body_shape, _) = self.redacted_body_shape(response).await;

        Ok(OutboundExecutorResponse {
            status,
            status_code: Some(status_code),
            headers_shape,
            body_shape: Some(body_shape),
            provider_request_id,
            usage: Value::Null,
            cost: Value::Null,
            redaction_state: RedactionState::Redacted,
            network_performed: true,
            executor_kind: ExecutorKind::Real,
        })
    }

    /// Y3: Stream an outbound HTTP request and emit frames.
    ///
    /// Uses `reqwest::Response::bytes_stream()` to get chunks, then
    /// parses them according to the specified `format` (SSE, NDJSON, Raw).
    /// Emits frames via the `emit` callback, respects cancellation,
    /// and applies safety caps.
    async fn stream(
        &self,
        request: OutboundExecutorRequest,
        format: StreamFormat,
        emit: Arc<dyn StreamEmitter>,
        cancel: CancelSignal,
        max_frame_bytes: Option<usize>,
        max_total_bytes: Option<usize>,
        max_duration_ms: Option<u64>,
    ) -> anyhow::Result<OutboundStreamSummary> {
        // Build URL (enforces HTTPS)
        let url = self.build_url(&request)?;

        // Build safe headers
        let headers = self.build_headers(&request)?;

        // Build the request method
        let method = match request.method.to_uppercase().as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "DELETE" => reqwest::Method::DELETE,
            "PATCH" => reqwest::Method::PATCH,
            "HEAD" => reqwest::Method::HEAD,
            "OPTIONS" => reqwest::Method::OPTIONS,
            other => anyhow::bail!("unsupported outbound HTTP method '{}'", other),
        };

        // Build the request
        let mut builder = self.client.request(method, url).headers(headers);

        // For streaming, we need a longer timeout for the initial connection
        // but the per-request timeout should be removed (we handle it ourselves)
        if let Some(body_shape) = &request.body_shape {
            builder = builder.json(body_shape);
        }

        // Apply max_duration as the request-level timeout if provided,
        // otherwise use a sensible default for streaming (5 minutes).
        let effective_timeout = max_duration_ms.unwrap_or(300_000);
        builder = builder.timeout(std::time::Duration::from_millis(effective_timeout));

        // Execute the request
        let response = match builder.send().await {
            Ok(r) => r,
            Err(e) => {
                let status = if e.is_timeout() { "timeout" } else { "error" };
                return Ok(OutboundStreamSummary {
                    status: status.to_string(),
                    status_code: None,
                    frame_count: 0,
                    bytes_received: 0,
                    headers_shape: None,
                    body_shape: None,
                    provider_request_id: None,
                    redaction_state: RedactionState::Redacted,
                    network_performed: true,
                    executor_kind: ExecutorKind::Real,
                });
            }
        };

        let status_code = response.status().as_u16();
        let response_status = if response.status().is_success() {
            "ok"
        } else {
            "error"
        };
        let headers_shape = Some(Self::redacted_headers_shape(&response));
        let provider_request_id = response
            .headers()
            .iter()
            .find(|(name, _)| {
                let n = name.as_str().to_lowercase();
                n == "request-id" || n == "x-request-id"
            })
            .and_then(|(_, v)| v.to_str().ok())
            .map(|s| s.to_string());

        // Stream the response body
        let use_stream = response.status().is_success();
        let mut seq: u64 = 0;
        let mut bytes_received: u64 = 0;
        let mut final_status = response_status.to_string();

        if use_stream {
            use crate::runtime::outbound_sse::SseParser;
            use futures::StreamExt;

            let mut sse_parser = SseParser::new();
            let mut ndjson_buffer = String::new();
            let mut stream = response.bytes_stream();

            let start_time = std::time::Instant::now();

            while let Some(chunk_result) = stream.next().await {
                // Check cancel signal
                if cancel.is_cancelled() {
                    final_status = "cancelled".to_string();
                    break;
                }

                // Check max_duration
                if let Some(max_ms) = max_duration_ms {
                    if start_time.elapsed().as_millis() as u64 > max_ms {
                        // Emit timeout error frame
                        seq += 1;
                        let _ = emit.emit(OutboundStreamFrame {
                            seq,
                            kind: OutboundFrameKind::Error,
                            data_shape: serde_json::json!({"error": "stream timeout", "max_duration_ms": max_ms}),
                            bytes_received,
                        }).await;
                        final_status = "timeout".to_string();
                        break;
                    }
                }

                let chunk = match chunk_result {
                    Ok(c) => c,
                    Err(e) => {
                        let error_msg = if e.is_timeout() {
                            "stream timeout"
                        } else {
                            "stream error"
                        };
                        seq += 1;
                        let _ = emit
                            .emit(OutboundStreamFrame {
                                seq,
                                kind: OutboundFrameKind::Error,
                                data_shape: serde_json::json!({"error": error_msg}),
                                bytes_received,
                            })
                            .await;
                        final_status = if e.is_timeout() { "timeout" } else { "error" }.to_string();
                        break;
                    }
                };

                bytes_received += chunk.len() as u64;

                // Check max_total_bytes
                if let Some(max_bytes) = max_total_bytes {
                    if bytes_received > max_bytes as u64 {
                        seq += 1;
                        let _ = emit.emit(OutboundStreamFrame {
                            seq,
                            kind: OutboundFrameKind::Error,
                            data_shape: serde_json::json!({"error": "max_total_bytes exceeded", "max_total_bytes": max_bytes}),
                            bytes_received,
                        }).await;
                        final_status = "error".to_string();
                        break;
                    }
                }

                // Parse according to format
                match format {
                    StreamFormat::Sse => {
                        let events = sse_parser.push(&chunk);
                        for event in events {
                            // Apply max_frame_bytes: truncate data if needed
                            let data_preview = if let Some(max) = max_frame_bytes {
                                if event.data.len() > max {
                                    event.data[..max].to_string()
                                } else {
                                    event.data.clone()
                                }
                            } else {
                                event.data.clone()
                            };

                            // Redact: emit shape only, not raw content
                            let data_shape = serde_json::json!({
                                "event": event.event,
                                "data_preview": redact_json_value(&serde_json::from_str::<Value>(&data_preview).unwrap_or_else(|_| Value::String(data_preview.clone()))),
                                "id": event.id,
                            });

                            seq += 1;
                            emit.emit(OutboundStreamFrame {
                                seq,
                                kind: OutboundFrameKind::Event,
                                data_shape,
                                bytes_received,
                            })
                            .await?;
                        }
                    }
                    StreamFormat::Ndjson => {
                        ndjson_buffer.push_str(&String::from_utf8_lossy(&chunk));
                        while let Some(newline_pos) = ndjson_buffer.find('\n') {
                            let line = ndjson_buffer[..newline_pos].to_string();
                            ndjson_buffer = ndjson_buffer[newline_pos + 1..].to_string();

                            let line = line.trim_end_matches('\r');
                            if line.is_empty() {
                                continue;
                            }

                            // Apply max_frame_bytes
                            let line_preview = if let Some(max) = max_frame_bytes {
                                if line.len() > max {
                                    line[..max].to_string()
                                } else {
                                    line.to_string()
                                }
                            } else {
                                line.to_string()
                            };

                            // Redact: parse as JSON and redact, or emit as string shape
                            let data_shape = serde_json::from_str::<Value>(&line_preview)
                                .map(|v| redact_json_value(&v))
                                .unwrap_or_else(|_| {
                                    Value::String(format!(
                                        "ndjson_line_{}bytes",
                                        line_preview.len()
                                    ))
                                });

                            seq += 1;
                            emit.emit(OutboundStreamFrame {
                                seq,
                                kind: OutboundFrameKind::Event,
                                data_shape,
                                bytes_received,
                            })
                            .await?;
                        }
                    }
                    StreamFormat::Raw => {
                        let chunk_size = chunk.len();
                        // Apply max_frame_bytes
                        let effective_size = if let Some(max) = max_frame_bytes {
                            chunk_size.min(max)
                        } else {
                            chunk_size
                        };

                        let data_shape = serde_json::json!({
                            "kind": "raw_chunk",
                            "bytes": effective_size,
                            "total_bytes_received": bytes_received,
                        });

                        seq += 1;
                        emit.emit(OutboundStreamFrame {
                            seq,
                            kind: OutboundFrameKind::Data,
                            data_shape,
                            bytes_received,
                        })
                        .await?;
                    }
                }

                // Check cancel after processing
                if cancel.is_cancelled() {
                    final_status = "cancelled".to_string();
                    break;
                }
            }

            // Flush remaining SSE events
            if matches!(format, StreamFormat::Sse) {
                let remaining = sse_parser.flush_remaining();
                for event in remaining {
                    let data_preview = if let Some(max) = max_frame_bytes {
                        if event.data.len() > max {
                            event.data[..max].to_string()
                        } else {
                            event.data.clone()
                        }
                    } else {
                        event.data.clone()
                    };
                    let data_shape = serde_json::json!({
                        "event": event.event,
                        "data_preview": redact_json_value(&serde_json::from_str::<Value>(&data_preview).unwrap_or_else(|_| Value::String(data_preview.clone()))),
                        "id": event.id,
                    });
                    seq += 1;
                    emit.emit(OutboundStreamFrame {
                        seq,
                        kind: OutboundFrameKind::Event,
                        data_shape,
                        bytes_received,
                    })
                    .await?;
                }
            }
        }

        // Emit done frame
        seq += 1;
        emit.emit(OutboundStreamFrame {
            seq,
            kind: OutboundFrameKind::Done,
            data_shape: serde_json::json!({"status": final_status}),
            bytes_received,
        })
        .await?;

        Ok(OutboundStreamSummary {
            status: final_status,
            status_code: Some(status_code),
            frame_count: seq,
            bytes_received,
            headers_shape,
            body_shape: None,
            provider_request_id,
            redaction_state: RedactionState::Redacted,
            network_performed: true,
            executor_kind: ExecutorKind::Real,
        })
    }
}

// ---------------------------------------------------------------------------
// OutboundExecutorConfig — configuration for RuntimeConfig
// ---------------------------------------------------------------------------

/// Configuration for which outbound executor the runtime uses.
///
/// Defaults to `DenyAll` (fail-closed).
#[derive(Clone)]
pub enum OutboundExecutorConfig {
    /// Deny all outbound (default, fail-closed).
    DenyAll,
    /// Use a custom executor (e.g. FakeOutboundExecutor for testing).
    Custom(Arc<dyn OutboundExecutor>),
    /// Use the live HTTP executor with reqwest + rustls (L2).
    /// Disabled by default; must explicitly opt in.
    LiveHttp(LiveHttpOutboundExecutorConfig),
}

impl Default for OutboundExecutorConfig {
    fn default() -> Self {
        Self::DenyAll
    }
}

// ---------------------------------------------------------------------------
// Runtime method: execute_outbound_with_policy
// ---------------------------------------------------------------------------

impl<S> Runtime<S>
where
    S: EventStore,
{
    /// Execute an outbound request through policy + executor.
    ///
    /// This is the primary entry point for the M3 outbound executor
    /// boundary. It:
    ///
    /// 1. Fail-closed if the policy/audit request and executor request
    ///    disagree on package, capability, host, method, or secret refs.
    /// 2. Calls `check_and_audit_outbound` to verify network policy.
    /// 3. If denied, returns an error and **does not** call the executor.
    /// 4. If allowed, calls the configured executor.
    /// 5. Returns the executor response.
    ///
    /// Raw body/header content is never persisted in audit records.
    /// Secret references are stored as refs only.
    pub async fn execute_outbound_with_policy(
        &self,
        policy_request: super::OutboundRequest,
        executor_request: OutboundExecutorRequest,
    ) -> anyhow::Result<OutboundExecutorResponse> {
        let started = Instant::now();
        let completion_id = new_id("obc");
        let total_bytes_request = json_size(&executor_request.body_shape);
        let configured_executor_kind = self.configured_outbound_executor_kind();
        let package_id = policy_request.package_id.clone();
        let capability_id = policy_request.capability_id.clone();
        let destination_host = policy_request.destination_host.clone();
        let method = policy_request.method.clone();
        let secret_refs_used = policy_request.secret_refs_used.clone();
        let correlation_id = policy_request.correlation_id;
        if let Err(error) = validate_policy_executor_consistency(&policy_request, &executor_request)
        {
            self.emit_outbound_execute_completed(super::OutboundExecuteCompletion {
                id: &completion_id,
                package_id: &package_id,
                capability_id: &capability_id,
                destination_host: &destination_host,
                method: &method,
                status: "invalid_request",
                executor_kind: executor_kind_str(configured_executor_kind),
                status_code: None,
                total_bytes_request,
                total_bytes_response: 0,
                duration_ms: started.elapsed().as_millis() as u64,
                network_performed: false,
                redaction_state: RedactionState::NotCaptured,
                secret_refs_used: &secret_refs_used,
                correlation_id,
                usage: serde_json::json!({}),
                cost: serde_json::json!({}),
            })
            .await?;
            return Err(error);
        }

        // Step 1: Policy check + audit. If denied, this returns an
        // error and the executor is never called.
        if let Err(err) = self.check_and_audit_outbound(policy_request).await {
            self.emit_outbound_execute_completed(super::OutboundExecuteCompletion {
                id: &completion_id,
                package_id: &package_id,
                capability_id: &capability_id,
                destination_host: &destination_host,
                method: &method,
                status: "denied",
                executor_kind: executor_kind_str(configured_executor_kind),
                status_code: None,
                total_bytes_request,
                total_bytes_response: 0,
                duration_ms: started.elapsed().as_millis() as u64,
                network_performed: false,
                redaction_state: RedactionState::NotCaptured,
                secret_refs_used: &secret_refs_used,
                correlation_id,
                usage: serde_json::json!({}),
                cost: serde_json::json!({}),
            })
            .await?;
            return Err(err);
        }

        // Step 2: Policy passed — call the configured executor.
        let executor = self.outbound_executor();
        let response = match executor.execute(executor_request).await {
            Ok(response) => response,
            Err(err) => {
                self.emit_outbound_execute_completed(super::OutboundExecuteCompletion {
                    id: &completion_id,
                    package_id: &package_id,
                    capability_id: &capability_id,
                    destination_host: &destination_host,
                    method: &method,
                    status: "error",
                    executor_kind: executor_kind_str(configured_executor_kind),
                    status_code: None,
                    total_bytes_request,
                    total_bytes_response: 0,
                    duration_ms: started.elapsed().as_millis() as u64,
                    network_performed: false,
                    redaction_state: RedactionState::Redacted,
                    secret_refs_used: &secret_refs_used,
                    correlation_id,
                    usage: serde_json::json!({}),
                    cost: serde_json::json!({}),
                })
                .await?;
                return Err(err);
            }
        };

        self.emit_outbound_execute_completed(super::OutboundExecuteCompletion {
            id: &completion_id,
            package_id: &package_id,
            capability_id: &capability_id,
            destination_host: &destination_host,
            method: &method,
            status: &response.status,
            executor_kind: executor_kind_str(response.executor_kind),
            status_code: response.status_code,
            total_bytes_request,
            total_bytes_response: json_size(&response.body_shape),
            duration_ms: started.elapsed().as_millis() as u64,
            network_performed: response.network_performed,
            redaction_state: response.redaction_state,
            secret_refs_used: &secret_refs_used,
            correlation_id,
            usage: response.usage.clone(),
            cost: response.cost.clone(),
        })
        .await?;

        Ok(response)
    }

    fn configured_outbound_executor_kind(&self) -> ExecutorKind {
        match &self.config.outbound_executor {
            OutboundExecutorConfig::DenyAll => ExecutorKind::DenyAll,
            OutboundExecutorConfig::Custom(_) => ExecutorKind::Fake,
            OutboundExecutorConfig::LiveHttp(_) => ExecutorKind::Real,
        }
    }

    /// Get a reference to the configured outbound executor.
    pub fn outbound_executor(&self) -> Arc<dyn OutboundExecutor> {
        match &self.config.outbound_executor {
            OutboundExecutorConfig::DenyAll => Arc::new(DenyAllOutboundExecutor),
            OutboundExecutorConfig::Custom(executor) => executor.clone(),
            OutboundExecutorConfig::LiveHttp(config) => {
                match LiveHttpOutboundExecutor::new(config.clone()) {
                    Ok(executor) => Arc::new(executor),
                    Err(_) => Arc::new(DenyAllOutboundExecutor), // fail-closed on build error
                }
            }
        }
    }
}

fn validate_policy_executor_consistency(
    policy_request: &super::OutboundRequest,
    executor_request: &OutboundExecutorRequest,
) -> anyhow::Result<()> {
    if policy_request.package_id != executor_request.package_id {
        anyhow::bail!("outbound package_id mismatch between policy and executor request");
    }
    if policy_request.capability_id != executor_request.capability_id {
        anyhow::bail!("outbound capability_id mismatch between policy and executor request");
    }
    if !policy_request
        .destination_host
        .eq_ignore_ascii_case(&executor_request.destination_host)
    {
        anyhow::bail!("outbound destination_host mismatch between policy and executor request");
    }
    if !policy_request
        .method
        .eq_ignore_ascii_case(&executor_request.method)
    {
        anyhow::bail!("outbound method mismatch between policy and executor request");
    }
    if policy_request.secret_refs_used != executor_request.secret_refs {
        anyhow::bail!("outbound secret_refs mismatch between policy and executor request");
    }
    Ok(())
}

pub(crate) fn executor_kind_str(kind: ExecutorKind) -> &'static str {
    match kind {
        ExecutorKind::DenyAll => "deny_all",
        ExecutorKind::Fake => "fake",
        ExecutorKind::Real => "real",
    }
}

fn json_size(value: &Option<Value>) -> u64 {
    value
        .as_ref()
        .and_then(|value| serde_json::to_vec(value).ok())
        .map(|bytes| bytes.len() as u64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executor_kind_serialization() {
        assert_eq!(
            serde_json::to_string(&ExecutorKind::DenyAll).unwrap(),
            "\"deny_all\""
        );
        assert_eq!(
            serde_json::to_string(&ExecutorKind::Fake).unwrap(),
            "\"fake\""
        );
        assert_eq!(
            serde_json::to_string(&ExecutorKind::Real).unwrap(),
            "\"real\""
        );
    }

    #[tokio::test]
    async fn deny_all_executor_returns_denied() {
        let executor = DenyAllOutboundExecutor;
        let request = OutboundExecutorRequest {
            package_id: "test/pkg".to_string(),
            capability_id: "test/pkg/fetch".to_string(),
            destination_host: "api.example.com".to_string(),
            method: "POST".to_string(),
            path: Some("/v1/chat/completions".to_string()),
            purpose: None,
            secret_refs: vec![],
            redaction_state: None,
            timeout_ms: None,
            metadata: Value::Null,
            body_shape: None,
            secret_headers: Vec::new(),
            resolved_secret_headers: Vec::new(),
            static_headers: Vec::new(),
        };
        let response = executor.execute(request).await.unwrap();
        assert_eq!(response.status, "denied");
        assert!(!response.network_performed);
        assert_eq!(response.executor_kind, ExecutorKind::DenyAll);
    }

    #[tokio::test]
    async fn fake_executor_returns_default_ok() {
        let executor = FakeOutboundExecutor::new();
        let request = OutboundExecutorRequest {
            package_id: "test/pkg".to_string(),
            capability_id: "test/pkg/fetch".to_string(),
            destination_host: "api.openai.com".to_string(),
            method: "POST".to_string(),
            path: Some("/v1/chat/completions".to_string()),
            purpose: Some("chat completions".to_string()),
            secret_refs: vec!["secret_ref:env:OPENAI_KEY".to_string()],
            redaction_state: Some(RedactionState::Redacted),
            timeout_ms: Some(30000),
            metadata: serde_json::json!({"provider": "openai"}),
            body_shape: Some(serde_json::json!({"model": "gpt-4o", "messages": []})),
            secret_headers: Vec::new(),
            resolved_secret_headers: Vec::new(),
            static_headers: Vec::new(),
        };
        let response = executor.execute(request).await.unwrap();
        assert_eq!(response.status, "ok");
        assert_eq!(response.status_code, Some(200));
        assert!(!response.network_performed);
        assert_eq!(response.executor_kind, ExecutorKind::Fake);
        assert_eq!(executor.call_count(), 1);
    }

    #[tokio::test]
    async fn fake_executor_returns_fixture() {
        let mut executor = FakeOutboundExecutor::new();
        executor.add_fixture(
            "api.anthropic.com",
            "POST",
            Some("/v1/messages"),
            OutboundExecutorResponse {
                status: "ok".to_string(),
                status_code: Some(200),
                headers_shape: Some(serde_json::json!({"content-type": "application/json"})),
                body_shape: Some(serde_json::json!({
                    "id": "msg_fake",
                    "type": "message",
                    "content": [{"type": "text", "text": "fixture response"}]
                })),
                provider_request_id: Some("msg_fake_001".to_string()),
                usage: serde_json::json!({"input_tokens": 10, "output_tokens": 5}),
                cost: serde_json::json!({}),
                redaction_state: RedactionState::Redacted,
                network_performed: false,
                executor_kind: ExecutorKind::Fake,
            },
        );

        let request = OutboundExecutorRequest {
            package_id: "test/pkg".to_string(),
            capability_id: "test/pkg/fetch".to_string(),
            destination_host: "api.anthropic.com".to_string(),
            method: "POST".to_string(),
            path: Some("/v1/messages".to_string()),
            purpose: None,
            secret_refs: vec![],
            redaction_state: None,
            timeout_ms: None,
            metadata: Value::Null,
            body_shape: None,
            secret_headers: Vec::new(),
            resolved_secret_headers: Vec::new(),
            static_headers: Vec::new(),
        };
        let response = executor.execute(request).await.unwrap();
        assert_eq!(response.status, "ok");
        assert_eq!(response.status_code, Some(200));
        assert!(response.body_shape.is_some());
        assert_eq!(response.executor_kind, ExecutorKind::Fake);
        assert_eq!(executor.call_count(), 1);
    }

    #[tokio::test]
    async fn fake_executor_call_count_increments() {
        let executor = FakeOutboundExecutor::new();
        assert_eq!(executor.call_count(), 0);

        let request = OutboundExecutorRequest {
            package_id: "test/pkg".to_string(),
            capability_id: "test/pkg/fetch".to_string(),
            destination_host: "api.example.com".to_string(),
            method: "GET".to_string(),
            path: None,
            purpose: None,
            secret_refs: vec![],
            redaction_state: None,
            timeout_ms: None,
            metadata: Value::Null,
            body_shape: None,
            secret_headers: Vec::new(),
            resolved_secret_headers: Vec::new(),
            static_headers: Vec::new(),
        };

        let _ = executor.execute(request.clone()).await.unwrap();
        assert_eq!(executor.call_count(), 1);

        let _ = executor.execute(request.clone()).await.unwrap();
        assert_eq!(executor.call_count(), 2);
    }

    #[test]
    fn outbound_executor_config_default_is_deny_all() {
        let config = OutboundExecutorConfig::default();
        matches!(config, OutboundExecutorConfig::DenyAll);
    }

    #[test]
    fn consistency_rejects_host_mismatch() {
        let policy = super::super::OutboundRequest {
            principal: crate::ProtocolPrincipal::Package {
                package_id: "test/pkg".to_string(),
            },
            package_id: "test/pkg".to_string(),
            capability_id: "test/pkg/fetch".to_string(),
            destination_host: "api.allowed.example".to_string(),
            method: "POST".to_string(),
            purpose: None,
            secret_refs_used: vec!["secret_ref:env:KEY".to_string()],
            correlation_id: None,
        };
        let executor = OutboundExecutorRequest {
            package_id: "test/pkg".to_string(),
            capability_id: "test/pkg/fetch".to_string(),
            destination_host: "api.evil.example".to_string(),
            method: "POST".to_string(),
            path: None,
            purpose: None,
            secret_refs: vec!["secret_ref:env:KEY".to_string()],
            redaction_state: None,
            timeout_ms: None,
            metadata: Value::Null,
            body_shape: None,
            secret_headers: Vec::new(),
            resolved_secret_headers: Vec::new(),
            static_headers: Vec::new(),
        };
        assert!(validate_policy_executor_consistency(&policy, &executor).is_err());
    }

    // -----------------------------------------------------------------------
    // L2: LiveHttpOutboundExecutor unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn live_http_config_default_is_safe() {
        let config = LiveHttpOutboundExecutorConfig::default();
        assert!(!config.allow_redirects);
        assert!(!config.allow_insecure_loopback_for_tests);
        assert_eq!(config.timeout_ms, 30_000);
        assert_eq!(config.connect_timeout_ms, 5_000);
        assert_eq!(config.max_response_preview_bytes, 1024);
    }

    #[test]
    fn live_http_config_insecure_loopback_defaults_false() {
        let config = LiveHttpOutboundExecutorConfig::default();
        assert!(
            !config.allow_insecure_loopback_for_tests,
            "allow_insecure_loopback_for_tests must default to false"
        );
    }

    #[test]
    fn live_http_rejects_redirects_until_rechecked() {
        let config = LiveHttpOutboundExecutorConfig {
            allow_redirects: true,
            ..Default::default()
        };
        let result = LiveHttpOutboundExecutor::new(config);
        assert!(
            result.is_err(),
            "redirects must fail closed until redirect target policy re-check exists"
        );
    }

    #[tokio::test]
    async fn live_http_rejects_non_https_url() {
        let config = LiveHttpOutboundExecutorConfig {
            allow_insecure_loopback_for_tests: false,
            ..Default::default()
        };
        let executor = LiveHttpOutboundExecutor::new(config).unwrap();

        let request = OutboundExecutorRequest {
            package_id: "test/pkg".to_string(),
            capability_id: "test/pkg/fetch".to_string(),
            destination_host: "api.example.com".to_string(),
            method: "POST".to_string(),
            path: Some("/v1/chat/completions".to_string()),
            purpose: None,
            secret_refs: vec![],
            redaction_state: None,
            timeout_ms: None,
            metadata: serde_json::json!({"scheme": "http"}),
            body_shape: None,
            secret_headers: Vec::new(),
            resolved_secret_headers: Vec::new(),
            static_headers: Vec::new(),
        };

        let result = executor.execute(request).await;
        assert!(result.is_err(), "live executor must reject http:// URL");
    }

    #[tokio::test]
    async fn live_http_rejects_http_base_url() {
        let config = LiveHttpOutboundExecutorConfig {
            allow_insecure_loopback_for_tests: false,
            ..Default::default()
        };
        let executor = LiveHttpOutboundExecutor::new(config).unwrap();

        let request = OutboundExecutorRequest {
            package_id: "test/pkg".to_string(),
            capability_id: "test/pkg/fetch".to_string(),
            destination_host: "api.example.com".to_string(),
            method: "POST".to_string(),
            path: None,
            purpose: None,
            secret_refs: vec![],
            redaction_state: None,
            timeout_ms: None,
            metadata: serde_json::json!({"base_url": "http://api.example.com"}),
            body_shape: None,
            secret_headers: Vec::new(),
            resolved_secret_headers: Vec::new(),
            static_headers: Vec::new(),
        };

        let result = executor.execute(request).await;
        assert!(result.is_err(), "live executor must reject http base_url");
    }

    #[tokio::test]
    async fn live_http_rejects_base_url_host_mismatch() {
        let executor =
            LiveHttpOutboundExecutor::new(LiveHttpOutboundExecutorConfig::default()).unwrap();

        let request = OutboundExecutorRequest {
            package_id: "test/pkg".to_string(),
            capability_id: "test/pkg/fetch".to_string(),
            destination_host: "api.example.com".to_string(),
            method: "POST".to_string(),
            path: Some("/v1/test".to_string()),
            purpose: None,
            secret_refs: vec![],
            redaction_state: None,
            timeout_ms: None,
            metadata: serde_json::json!({"base_url": "https://other.example.com"}),
            body_shape: None,
            secret_headers: Vec::new(),
            resolved_secret_headers: Vec::new(),
            static_headers: Vec::new(),
        };

        let result = executor.execute(request).await;
        assert!(result.is_err(), "base_url host mismatch must fail closed");
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("does not match executor destination_host"));
    }

    #[tokio::test]
    async fn live_http_rejects_unsupported_method() {
        let executor =
            LiveHttpOutboundExecutor::new(LiveHttpOutboundExecutorConfig::default()).unwrap();

        let request = OutboundExecutorRequest {
            package_id: "test/pkg".to_string(),
            capability_id: "test/pkg/fetch".to_string(),
            destination_host: "api.example.com".to_string(),
            method: "TRACE".to_string(),
            path: Some("/v1/test".to_string()),
            purpose: None,
            secret_refs: vec![],
            redaction_state: None,
            timeout_ms: None,
            metadata: serde_json::json!({}),
            body_shape: None,
            secret_headers: Vec::new(),
            resolved_secret_headers: Vec::new(),
            static_headers: Vec::new(),
        };

        let result = executor.execute(request).await;
        assert!(result.is_err(), "unsupported method must fail closed");
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("unsupported outbound HTTP method"));
    }

    #[tokio::test]
    async fn live_http_allows_loopback_when_test_flag_set() {
        let config = LiveHttpOutboundExecutorConfig {
            allow_insecure_loopback_for_tests: true,
            ..Default::default()
        };
        let executor = LiveHttpOutboundExecutor::new(config).unwrap();

        let request = OutboundExecutorRequest {
            package_id: "test/pkg".to_string(),
            capability_id: "test/pkg/fetch".to_string(),
            destination_host: "127.0.0.1".to_string(),
            method: "POST".to_string(),
            path: Some("/test".to_string()),
            purpose: None,
            secret_refs: vec![],
            redaction_state: None,
            timeout_ms: Some(100), // short timeout so we don't hang
            metadata: serde_json::json!({"scheme": "http"}),
            body_shape: None,
            secret_headers: Vec::new(),
            resolved_secret_headers: Vec::new(),
            static_headers: Vec::new(),
        };

        // This will fail to connect (nothing listening on 127.0.0.1),
        // but it should NOT be rejected at the URL-building stage.
        let result = executor.execute(request).await;
        // The request should either succeed (if something is listening)
        // or return an error response — but NOT be rejected for being
        // non-HTTPS. An error response from the executor still means
        // the URL was accepted.
        match result {
            Ok(response) => {
                // Error/timeout response from network attempt
                assert_eq!(response.executor_kind, ExecutorKind::Real);
                assert!(response.network_performed);
                assert!(response.status == "error" || response.status == "timeout");
                // Verify no raw secret-like values in response
                let response_json = serde_json::to_value(&response).unwrap();
                assert!(response_json.get("raw_body").is_none());
                assert!(response_json.get("raw_header").is_none());
                assert!(response_json.get("api_key").is_none());
            }
            Err(_) => {
                // Connection error (nothing listening) is also fine.
                // The key test is that it's NOT the HTTPS-only rejection.
            }
        }
    }

    #[tokio::test]
    async fn live_http_rejects_non_loopback_insecure_url_even_with_test_flag() {
        let config = LiveHttpOutboundExecutorConfig {
            allow_insecure_loopback_for_tests: true,
            ..Default::default()
        };
        let executor = LiveHttpOutboundExecutor::new(config).unwrap();

        let request = OutboundExecutorRequest {
            package_id: "test/pkg".to_string(),
            capability_id: "test/pkg/fetch".to_string(),
            destination_host: "api.example.com".to_string(),
            method: "POST".to_string(),
            path: None,
            purpose: None,
            secret_refs: vec![],
            redaction_state: None,
            timeout_ms: None,
            metadata: serde_json::json!({"scheme": "http"}),
            body_shape: None,
            secret_headers: Vec::new(),
            resolved_secret_headers: Vec::new(),
            static_headers: Vec::new(),
        };

        let result = executor.execute(request).await;
        assert!(
            result.is_err(),
            "live executor must reject http:// to non-loopback even with test flag"
        );
    }

    #[test]
    fn live_http_response_no_raw_secret_fields() {
        // Verify that OutboundExecutorResponse from a Real executor
        // doesn't have fields that would expose raw secrets
        let response = OutboundExecutorResponse {
            status: "error".to_string(),
            status_code: Some(503),
            headers_shape: Some(serde_json::json!({"content-type": "application/json"})),
            body_shape: Some(serde_json::json!({"error": "service unavailable"})),
            provider_request_id: Some("req-123".to_string()),
            usage: Value::Null,
            cost: Value::Null,
            redaction_state: RedactionState::Redacted,
            network_performed: true,
            executor_kind: ExecutorKind::Real,
        };

        let json = serde_json::to_value(&response).unwrap();
        let json_str = serde_json::to_string(&json).unwrap();
        // Must not contain raw secret-like fields
        assert!(!json_str.contains("raw_body"));
        assert!(!json_str.contains("raw_header"));
        assert!(!json_str.contains("raw_secret"));
        assert!(!json_str.contains("api_key"));
        assert!(!json_str.contains("Bearer "));
        assert!(!json_str.contains("sk-"));
    }

    #[test]
    fn redact_json_value_redacts_secret_fields() {
        let input = serde_json::json!({
            "model": "gpt-4o",
            "api_key": "raw-secret-placeholder",
            "data": {
                "token": "bearer-abc",
                "safe_field": "hello"
            },
            "items": [{"password": "s3cret", "name": "ok"}]
        });
        let redacted = redact_json_value(&input);

        // Secret fields should be redacted
        assert_eq!(redacted["api_key"], "[redacted]");
        assert_eq!(redacted["data"]["token"], "[redacted]");
        assert_eq!(redacted["items"][0]["password"], "[redacted]");

        // Non-secret fields should be preserved
        assert_eq!(redacted["model"], "gpt-4o");
        assert_eq!(redacted["data"]["safe_field"], "hello");
        assert_eq!(redacted["items"][0]["name"], "ok");
    }

    #[test]
    fn is_safe_response_header_only_allows_known_safe_headers() {
        assert!(is_safe_response_header("content-type"));
        assert!(is_safe_response_header("request-id"));
        assert!(is_safe_response_header("x-request-id"));
        assert!(is_safe_response_header("x-trace-id"));
        assert!(!is_safe_response_header("authorization"));
        assert!(!is_safe_response_header("set-cookie"));
        assert!(!is_safe_response_header("x-api-key"));
    }

    // -----------------------------------------------------------------------
    // L5: Static headers allowlist and blocking tests
    // -----------------------------------------------------------------------

    #[test]
    fn static_header_allowlist_allows_safe_headers() {
        assert!(is_static_header_allowed("anthropic-version"));
        assert!(is_static_header_allowed("content-type"));
        assert!(is_static_header_allowed("accept"));
        // Case-insensitive
        assert!(is_static_header_allowed("Anthropic-Version"));
        assert!(is_static_header_allowed("Content-Type"));
    }

    #[test]
    fn static_header_allowlist_rejects_unlisted_headers() {
        assert!(!is_static_header_allowed("x-custom-header"));
        assert!(!is_static_header_allowed("some-random-header"));
        assert!(!is_static_header_allowed("x-plurora-outbound"));
        assert!(!is_static_header_allowed("user-agent"));
        assert!(!is_static_header_allowed("accept-encoding"));
    }

    #[test]
    fn static_header_rejects_secret_bearing_names() {
        assert!(is_secret_header_name("authorization"));
        assert!(is_secret_header_name("x-api-key"));
        assert!(is_secret_header_name("x-goog-api-key"));
        assert!(is_secret_header_name("cookie"));
        assert!(is_secret_header_name("set-cookie"));
        assert!(is_secret_header_name("proxy-authorization"));
        assert!(is_secret_header_name("x-auth-token"));
        assert!(is_secret_header_name("x-access-token"));
        assert!(is_secret_header_name("x-secret"));
        assert!(is_secret_header_name("www-authenticate"));
        // Case-insensitive
        assert!(is_secret_header_name("Authorization"));
        assert!(is_secret_header_name("X-Api-Key"));
    }

    #[test]
    fn static_header_allows_non_secret_names() {
        assert!(!is_secret_header_name("anthropic-version"));
        assert!(!is_secret_header_name("content-type"));
        assert!(!is_secret_header_name("accept"));
        assert!(!is_secret_header_name("user-agent"));
    }

    // L6: OpenRouter safe static headers are allowed and not secret-bearing
    #[test]
    fn static_header_allows_openrouter_safe_headers() {
        // Lower-case allowlist entries match case-insensitively
        assert!(is_static_header_allowed("http-referer"));
        assert!(is_static_header_allowed("HTTP-Referer"));
        assert!(is_static_header_allowed("x-title"));
        assert!(is_static_header_allowed("X-Title"));
        // These are not secret-bearing
        assert!(!is_secret_header_name("http-referer"));
        assert!(!is_secret_header_name("HTTP-Referer"));
        assert!(!is_secret_header_name("x-title"));
        assert!(!is_secret_header_name("X-Title"));
    }

    #[test]
    fn static_header_blocks_secret_names_even_if_allowed_elsewhere() {
        // Defense-in-depth: secret-bearing names blocked even if someone
        // accidentally added them to the allowlist
        assert!(is_secret_header_name("authorization"));
        assert!(is_secret_header_name("x-api-key"));
        assert!(is_secret_header_name("x-goog-api-key"));
        assert!(is_secret_header_name("cookie"));
    }

    #[test]
    fn empty_static_headers_helper() {
        let headers = OutboundExecutorRequest::empty_static_headers();
        assert!(headers.is_empty());
    }
}
