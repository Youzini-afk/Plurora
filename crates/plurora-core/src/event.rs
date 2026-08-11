use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::ids::{CapabilityId, EventId, PackageId, SessionId};
use crate::manifest::ContractMode;

pub type SchemaVersion = u16;
pub type EventKind = String;
pub type EventSequence = u64;

pub const PLATFORM_RUNTIME_ID: &str = "plurora/runtime";
pub const EVENT_SESSION_OPENED: &str = "context/opened";
pub const EVENT_SESSION_CLOSED: &str = "context/closed";
pub const EVENT_SESSION_FORKED: &str = "context/forked";
pub const EVENT_PACKAGE_LOADED: &str = "host/package.loaded";
pub const EVENT_PACKAGE_LOADING: &str = "host/package.loading";
pub const EVENT_PACKAGE_STARTING: &str = "host/package.starting";
pub const EVENT_PACKAGE_READY: &str = "host/package.ready";
pub const EVENT_PACKAGE_STOPPING: &str = "host/package.stopping";
pub const EVENT_PACKAGE_STOPPED: &str = "host/package.stopped";
pub const EVENT_PACKAGE_UNLOADED: &str = "host/package.unloaded";
pub const EVENT_PACKAGE_DEGRADED: &str = "host/package.degraded";
pub const EVENT_PACKAGE_LOG: &str = "host/package.log";
pub const INSTALLATION_CREATED: &str = "host/installation.created";
pub const INSTALLATION_UPDATED: &str = "host/installation.updated";
pub const INSTALLATION_REMOVED: &str = "host/installation.removed";
pub const EVENT_RUN_STARTING: &str = "host/run.starting";
pub const EVENT_RUN_STARTED: &str = "host/run.started";
pub const EVENT_RUN_STOPPING: &str = "host/run.stopping";
pub const EVENT_RUN_STOPPED: &str = "host/run.stopped";
pub const EVENT_RUN_FAILED: &str = "host/run.failed";
pub const EVENT_EXPOSURE_CREATED: &str = "host/exposure.created";
pub const EVENT_EXPOSURE_REVOKED: &str = "host/exposure.revoked";
pub const EVENT_EXPOSURE_EXPIRED: &str = "host/exposure.expired";
pub const EVENT_BINDING_SELECTED: &str = "host/binding.selected";
pub const EVENT_BINDING_REVOKED: &str = "host/binding.revoked";
pub const EVENT_BINDING_EXPIRED: &str = "host/binding.expired";
pub const EVENT_ASSET_PUT: &str = "object/put";
pub const EVENT_PROJECTION_UPDATED: &str = "projection/updated";
pub const EVENT_PROPOSAL_CREATED: &str = "change/proposal.created";
pub const EVENT_PROPOSAL_APPROVED: &str = "change/proposal.approved";
pub const EVENT_PROPOSAL_REJECTED: &str = "change/proposal.rejected";
pub const EVENT_PROPOSAL_APPLIED: &str = "change/proposal.applied";
pub const EVENT_PROPOSAL_FAILED: &str = "change/proposal.failed";
pub const EVENT_CAPABILITY_INVOKED: &str = "capability/invoked";
pub const EVENT_CAPABILITY_COMPLETED: &str = "capability/completed";
pub const EVENT_CAPABILITY_FAILED: &str = "capability/failed";
pub const EVENT_PERMISSION_DENIED: &str = "authority/denied";
pub const EVENT_PERMISSION_GRANTED: &str = "authority/grant.created";
pub const EVENT_PERMISSION_REVOKED: &str = "authority/grant.revoked";
pub const EVENT_ERROR: &str = "runtime/error";
pub const EVENT_OUTBOUND_REQUEST: &str = "host/outbound.request";
pub const EVENT_OUTBOUND_DENIED: &str = "host/outbound.denied";
pub const EVENT_OUTBOUND_EXECUTE_COMPLETED: &str = "host/outbound.execute.completed";
pub const EVENT_OUTBOUND_STREAM_COMPLETED: &str = "host/outbound.stream.completed";
pub const EVENT_STREAM_STARTED: &str = "capability/stream.started";
pub const EVENT_STREAM_CHUNK: &str = "capability/stream.chunk";
pub const EVENT_STREAM_PROGRESS: &str = "capability/stream.progress";
pub const EVENT_STREAM_ENDED: &str = "capability/stream.ended";
pub const EVENT_STREAM_ERROR: &str = "capability/stream.error";
pub const EVENT_STREAM_CANCELLED: &str = "capability/stream.cancelled";
pub const EVENT_STREAM_TIMEOUT: &str = "capability/stream.timeout";
pub const EVENT_OUTBOUND_WEBSOCKET_OPENED: &str = "host/outbound.websocket.opened";
pub const EVENT_OUTBOUND_WEBSOCKET_FRAME: &str = "host/outbound.websocket.frame";
pub const EVENT_OUTBOUND_WEBSOCKET_ERROR: &str = "host/outbound.websocket.error";
pub const EVENT_OUTBOUND_WEBSOCKET_COMPLETED: &str = "host/outbound.websocket.completed";
pub const EVENT_EXEC_REQUEST: &str = "host/exec.request";
pub const EVENT_EXEC_DENIED: &str = "host/exec.denied";
pub const EVENT_EXEC_STARTED: &str = "host/exec.started";
pub const EVENT_EXEC_STOPPED: &str = "host/exec.stopped";
pub const EVENT_EXEC_COMPLETED: &str = "host/exec.completed";
pub const EVENT_EXEC_FAILED: &str = "host/exec.failed";
pub const EVENT_PORT_LEASED: &str = "host/port.leased";
pub const EVENT_PORT_RELEASED: &str = "host/port.released";
pub const EVENT_PORT_DENIED: &str = "host/port.denied";
pub const EVENT_PROXY_REGISTERED: &str = "host/proxy.registered";
pub const EVENT_PROXY_UNREGISTERED: &str = "host/proxy.unregistered";
pub const EVENT_PROXY_DENIED: &str = "host/proxy.denied";
pub const EVENT_DEPLOYMENT_RECONCILED: &str = "host/deployment.reconciled";
pub const EVENT_DEPLOYMENT_HEALTH: &str = "host/deployment.health";

/// Event kinds emitted exclusively by the Plurora runtime.
///
/// The list is explicit because public event ownership follows semantic owner
/// namespaces rather than a single reserved string prefix.
pub const PLATFORM_EVENT_KINDS: &[&str] = &[
    EVENT_SESSION_OPENED,
    EVENT_SESSION_CLOSED,
    EVENT_SESSION_FORKED,
    EVENT_PACKAGE_LOADED,
    EVENT_PACKAGE_LOADING,
    EVENT_PACKAGE_STARTING,
    EVENT_PACKAGE_READY,
    EVENT_PACKAGE_STOPPING,
    EVENT_PACKAGE_STOPPED,
    EVENT_PACKAGE_UNLOADED,
    EVENT_PACKAGE_DEGRADED,
    EVENT_PACKAGE_LOG,
    INSTALLATION_CREATED,
    INSTALLATION_UPDATED,
    INSTALLATION_REMOVED,
    EVENT_RUN_STARTING,
    EVENT_RUN_STARTED,
    EVENT_RUN_STOPPING,
    EVENT_RUN_STOPPED,
    EVENT_RUN_FAILED,
    EVENT_EXPOSURE_CREATED,
    EVENT_EXPOSURE_REVOKED,
    EVENT_EXPOSURE_EXPIRED,
    EVENT_BINDING_SELECTED,
    EVENT_BINDING_REVOKED,
    EVENT_BINDING_EXPIRED,
    EVENT_ASSET_PUT,
    EVENT_PROJECTION_UPDATED,
    EVENT_PROPOSAL_CREATED,
    EVENT_PROPOSAL_APPROVED,
    EVENT_PROPOSAL_REJECTED,
    EVENT_PROPOSAL_APPLIED,
    EVENT_PROPOSAL_FAILED,
    EVENT_CAPABILITY_INVOKED,
    EVENT_CAPABILITY_COMPLETED,
    EVENT_CAPABILITY_FAILED,
    EVENT_PERMISSION_DENIED,
    EVENT_PERMISSION_GRANTED,
    EVENT_PERMISSION_REVOKED,
    EVENT_ERROR,
    EVENT_OUTBOUND_REQUEST,
    EVENT_OUTBOUND_DENIED,
    EVENT_OUTBOUND_EXECUTE_COMPLETED,
    EVENT_OUTBOUND_STREAM_COMPLETED,
    EVENT_STREAM_STARTED,
    EVENT_STREAM_CHUNK,
    EVENT_STREAM_PROGRESS,
    EVENT_STREAM_ENDED,
    EVENT_STREAM_ERROR,
    EVENT_STREAM_CANCELLED,
    EVENT_STREAM_TIMEOUT,
    EVENT_OUTBOUND_WEBSOCKET_OPENED,
    EVENT_OUTBOUND_WEBSOCKET_FRAME,
    EVENT_OUTBOUND_WEBSOCKET_ERROR,
    EVENT_OUTBOUND_WEBSOCKET_COMPLETED,
    EVENT_EXEC_REQUEST,
    EVENT_EXEC_DENIED,
    EVENT_EXEC_STARTED,
    EVENT_EXEC_STOPPED,
    EVENT_EXEC_COMPLETED,
    EVENT_EXEC_FAILED,
    EVENT_PORT_LEASED,
    EVENT_PORT_RELEASED,
    EVENT_PORT_DENIED,
    EVENT_PROXY_REGISTERED,
    EVENT_PROXY_UNREGISTERED,
    EVENT_PROXY_DENIED,
    EVENT_DEPLOYMENT_RECONCILED,
    EVENT_DEPLOYMENT_HEALTH,
];

pub fn is_platform_event_kind(kind: &str) -> bool {
    PLATFORM_EVENT_KINDS.contains(&kind)
}

// ---------------------------------------------------------------------------
// Outbound audit / redaction types (Phase S2)
// ---------------------------------------------------------------------------

/// Redaction state for an outbound audit record.
///
/// Every outbound request carries one of these states to indicate
/// whether raw body/header/prompt/response data was preserved.
/// The default is `NotCaptured` — raw data is never saved unless
/// explicitly approved.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RedactionState {
    /// Raw data was not captured (default).
    NotCaptured,
    /// Raw data was redacted before recording.
    Redacted,
    /// Only a policy reference is stored; no data captured.
    PolicyRef,
    /// Request was blocked as unsafe; no data recorded.
    UnsafeBlocked,
    /// Explicit user/host approval to capture raw data (rare).
    ExplicitlyApproved,
}

impl Default for RedactionState {
    fn default() -> Self {
        Self::NotCaptured
    }
}

/// Generic outbound audit record / envelope.
///
/// Records an outbound network request made by a package through
/// Plurora-provided network/request helpers. This is a platform event
/// payload — it does NOT contain raw secrets, bodies, headers,
/// prompts, or responses. Only `secret_ref` identifiers and the
/// `redaction_state` are recorded.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OutboundAuditRecord {
    /// Unique record id.
    pub id: String,
    /// The principal that initiated the request.
    pub principal: String,
    /// Package that owns the outbound request.
    pub package_id: PackageId,
    /// Capability through which the request was made.
    pub capability_id: CapabilityId,
    /// Destination host.
    pub destination_host: String,
    /// HTTP method (GET, POST, etc).
    pub method: String,
    /// Declared purpose from the manifest or request context.
    #[serde(default)]
    pub purpose: Option<String>,
    /// Redaction state — what data, if any, was recorded.
    #[serde(default)]
    pub redaction_state: RedactionState,
    /// Secret references used (not raw secrets).
    #[serde(default)]
    pub secret_refs_used: Vec<String>,
    /// Usage placeholder (e.g. token count).
    #[serde(default)]
    pub usage: Value,
    /// Cost placeholder.
    #[serde(default)]
    pub cost: Value,
    /// Request status: "allowed", "denied", "error", etc.
    pub status: String,
    /// Error message if status is not "allowed".
    #[serde(default)]
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Type aliases and EventEnvelope
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EventEnvelope {
    pub id: EventId,
    pub session_id: SessionId,
    pub sequence: EventSequence,
    pub writer_package_id: PackageId,
    pub kind: EventKind,
    pub schema_version: SchemaVersion,
    pub timestamp: DateTime<Utc>,
    pub payload: Value,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PackageLifecyclePayload {
    pub package_id: PackageId,
    pub version: String,
    pub state: String,
    pub entry_kind: String,
    /// Contract mode selected by the package entry. `none` marks Path B: a
    /// self-contained app hosted without contract enforcement.
    pub contract_mode: ContractMode,
    pub capability_count: usize,
    pub hook_count: usize,
    pub extension_point_count: usize,
    #[serde(default)]
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Stream frame envelope types (Phase S3)
// ---------------------------------------------------------------------------

/// The type of a stream frame — content-free, no model/prompt semantics.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StreamFrameType {
    /// First frame of a streaming invocation.
    Start,
    /// A data chunk in the stream.
    Chunk,
    /// Progress indication (no data payload).
    Progress,
    /// Normal terminal frame.
    End,
    /// Error terminal frame.
    Error,
    /// Cancelled terminal frame.
    Cancelled,
    /// Timeout terminal frame.
    Timeout,
}

/// Generic stream frame envelope — the unit of streaming capability output.
///
/// This is a content-free protocol shape. It carries invocation/stream
/// identifiers, sequencing, and redaction state, but no model, prompt,
/// agent, or message semantics. The `payload` field is opaque JSON
/// controlled by the capability provider.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StreamFrameEnvelope {
    /// The invocation this frame belongs to.
    pub invocation_id: crate::ids::InvocationId,
    /// Unique stream id (may differ from invocation_id if a capability
    /// produces multiple concurrent streams).
    pub stream_id: String,
    /// Frame type discriminant.
    pub frame_type: StreamFrameType,
    /// Monotonically increasing sequence within this stream.
    pub sequence: u64,
    /// Redaction state applied to the payload.
    #[serde(default)]
    pub redaction_state: RedactionState,
    /// Timestamp of frame emission.
    #[serde(
        default = "default_timestamp",
        skip_serializing_if = "never_skip_timestamp"
    )]
    pub timestamp: DateTime<Utc>,
    /// Opaque payload — capability-provider-defined; no kernel content
    /// semantics. May be `Null` for progress/end/cancelled/timeout frames.
    #[serde(default)]
    pub payload: Value,
    /// Opaque metadata — capability-provider-defined.
    #[serde(default)]
    pub metadata: Value,
}

fn default_timestamp() -> DateTime<Utc> {
    Utc::now()
}

fn never_skip_timestamp(_: &DateTime<Utc>) -> bool {
    false
}

// ---------------------------------------------------------------------------
// Streaming invocation record (Phase S3 — in-memory registry)
// ---------------------------------------------------------------------------

/// The terminal state of a streaming invocation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StreamInvocationState {
    /// Streaming is actively producing frames.
    Active,
    /// Stream ended normally.
    Ended,
    /// Stream terminated with an error.
    Error,
    /// Stream was cancelled by caller.
    Cancelled,
    /// Stream timed out.
    Timeout,
}

/// A record in the ongoing streaming invocation registry.
///
/// This tracks the lifecycle of a streaming capability invocation.
/// It is content-free — it records state, identifiers, and audit
/// metadata, but no model/prompt/message semantics.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StreamInvocationRecord {
    /// Unique invocation id.
    pub invocation_id: crate::ids::InvocationId,
    /// Unique stream id.
    pub stream_id: String,
    /// The capability being streamed.
    pub capability_id: crate::ids::CapabilityId,
    /// The package providing the capability.
    pub provider_package_id: crate::ids::PackageId,
    /// The session this invocation belongs to.
    pub session_id: crate::ids::SessionId,
    /// Current state of the invocation.
    pub state: StreamInvocationState,
    /// Number of frames emitted so far.
    #[serde(default)]
    pub frame_count: u64,
    /// Timestamp of invocation start.
    pub started_at: DateTime<Utc>,
    /// Timestamp of terminal state, if ended.
    #[serde(default)]
    pub ended_at: Option<DateTime<Utc>>,
    /// Opaque metadata — capability-provider-defined.
    #[serde(default)]
    pub metadata: Value,
}

impl StreamInvocationRecord {
    /// Whether further frames can be appended to this invocation.
    pub fn is_terminal(&self) -> bool {
        !matches!(self.state, StreamInvocationState::Active)
    }
}

impl EventEnvelope {
    pub fn new(
        id: EventId,
        session_id: SessionId,
        sequence: EventSequence,
        writer_package_id: PackageId,
        kind: impl Into<EventKind>,
        payload: Value,
    ) -> Self {
        Self {
            id,
            session_id,
            sequence,
            writer_package_id,
            kind: kind.into(),
            schema_version: 1,
            timestamp: Utc::now(),
            payload,
            metadata: Value::Object(Map::new()),
        }
    }

    pub fn is_platform_event(&self) -> bool {
        self.writer_package_id == PLATFORM_RUNTIME_ID && is_platform_event_kind(&self.kind)
    }

    pub fn writer_owns_kind(&self) -> bool {
        if is_platform_event_kind(&self.kind) {
            return self.writer_package_id == PLATFORM_RUNTIME_ID;
        }
        self.kind
            .starts_with(&format!("{}/", self.writer_package_id))
    }
}
