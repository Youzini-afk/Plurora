/**
 * Plurora Agent Adapter SDK — Pure TypeScript adapter for agent-like
 * capability packages.
 *
 * This module provides the minimal stable types and helpers that let
 * capability packages act as agent tools without depending on private
 * runtime internals, `pi-coding-agent`, or any external framework.
 *
 * It is an **SDK adapter**, not an agent framework. It does not ship a
 * model, a prompt, a memory, or a turn loop. It only maps capabilities
 * to tool shapes, invokes them through the public protocol, and
 * produces trace/proposal artifacts.
 *
 * ## API surface
 *
 * - `ProtocolClient`                 — interface for `call(request)` protocol
 * - `CapabilityDescriptor`           — describes a capability
 * - `CapabilityTool`                 — tool representation of a capability
 * - `ToolCall` / `ToolResult`       — invocation request / result
 * - `AgentTraceEvent`               — trace event emitted by an agent
 * - `AgentProposalDraft`            — proposal draft artifact
 * - `createYggAgentAdapter`         — main adapter factory
 * - `capabilityToTool`              — convert descriptor → tool
 * - `createCapabilityTool`          — create a CapabilityTool
 * - `invokeCapabilityTool`          — invoke a tool through protocol
 * - `streamCapabilityTool`          — stream variant (request / frames)
 * - `createTraceEvent`             — build a trace event
 * - `createProposalDraft`          — build a proposal draft
 * - `diagnosePermissions`          — permission diagnostics helper
 * - `diagnoseProvider`             — provider diagnostics helper
 * - `blockRawSecrets`             — reject raw secrets in payloads
 * - `runYggAgentAdapterSelfTest`   — pure-TS self-test
 */

// ---------------------------------------------------------------------------
// Stable minimal types
// ---------------------------------------------------------------------------

/** Protocol request shape sent through ProtocolClient.call(). */
export interface ProtocolRequest {
  /** The capability to invoke, e.g. "example/pkg/cap". */
  capability_id: string;
  /** Input payload for the capability. */
  input?: unknown;
  /** Explicit provider package id (required when multiple providers exist). */
  provider_package_id?: string;
  /** Optional session id for multi-turn context. */
  session_id?: string;
  /** Optional correlation id for tracing. */
  correlation_id?: string;
  /** Optional metadata bag. */
  metadata?: Record<string, unknown>;
}

/** Protocol response shape returned from ProtocolClient.call(). */
export interface ProtocolResponse {
  /** Whether the call succeeded. */
  ok: boolean;
  /** The result payload (when ok=true). */
  output?: unknown;
  /** Error message (when ok=false). */
  error?: string;
  /** The capability id that was invoked. */
  capability_id: string;
  /** The provider package id that handled the call. */
  provider_package_id?: string;
  /** Optional trace id assigned by the kernel.v1. */
  trace_id?: string;
}

/**
 * ProtocolClient — abstract interface expressing `call(request)`.
 *
 * Implementations may wrap a kernel protocol client, a subprocess
 * transport, or a test double. The adapter never assumes a specific
 * transport.
 */
export interface ProtocolClient {
  call(request: ProtocolRequest): Promise<ProtocolResponse>;
}

/** A capability descriptor — the minimal metadata that identifies a capability. */
export interface CapabilityDescriptor {
  /** Fully-qualified capability id, e.g. "example/pkg/cap". */
  capability_id: string;
  /** Human-readable name for the tool. */
  name?: string;
  /** Short description of what the capability does. */
  description?: string;
  /** Input schema (JSON Schema shape). */
  input_schema?: Record<string, unknown>;
  /** Whether this capability supports streaming. */
  streamable?: boolean;
  /** Known provider package ids that can serve this capability. */
  provider_package_ids?: string[];
  /** Required permissions for this capability. */
  required_permissions?: string[];
}

/** A capability represented as a tool that an agent can invoke. */
export interface CapabilityTool {
  /** The underlying capability id. */
  capability_id: string;
  /** Tool name (derived from descriptor or capability id). */
  name: string;
  /** Tool description. */
  description: string;
  /** Input schema. */
  input_schema: Record<string, unknown>;
  /** Whether the tool supports streaming. */
  streamable: boolean;
  /** Known provider package ids. */
  provider_package_ids: string[];
  /** Required permissions. */
  required_permissions: string[];
}

/** A tool invocation request. */
export interface ToolCall {
  /** The tool to invoke. */
  tool: CapabilityTool;
  /** Input arguments for the tool. */
  input: unknown;
  /** Explicit provider (required when tool has ambiguous providers). */
  provider_package_id?: string;
  /** Optional session id. */
  session_id?: string;
  /** Optional correlation id. */
  correlation_id?: string;
}

/** A tool invocation result. */
export interface ToolResult {
  /** Whether the invocation succeeded. */
  ok: boolean;
  /** Result payload (when ok=true). */
  output?: unknown;
  /** Error message (when ok=false). */
  error?: string;
  /** The capability id that was invoked. */
  capability_id: string;
  /** The provider package id that handled the invocation. */
  provider_package_id?: string;
  /** Trace id from the kernel, if assigned. */
  trace_id?: string;
}

/** Stream request payload — what streamCapabilityTool returns. */
export interface StreamRequest {
  /** The protocol request that would initiate the stream. */
  request: ProtocolRequest;
  /** The capability tool being streamed. */
  tool: CapabilityTool;
  /** Frame adapter for building a faux stream lifecycle. */
  frames: StreamFrameAdapter;
}

/** Stream frame adapter — builds faux stream frame sequences (no real SSE). */
export interface StreamFrameAdapter {
  /** Create a start frame. */
  start(payload?: unknown): StreamAdapterFrame;
  /** Append a chunk frame. */
  chunk(payload: unknown): StreamAdapterFrame;
  /** Append a progress frame. */
  progress(metadata?: Record<string, unknown>): StreamAdapterFrame;
  /** End the stream normally. */
  end(): StreamAdapterFrame;
  /** Error-terminate the stream. */
  error(message: string): StreamAdapterFrame;
  /** Get the total frame count so far. */
  frameCount(): number;
}

/** A single frame in a stream adapter sequence. */
export interface StreamAdapterFrame {
  /** Frame kind. */
  kind: "start" | "chunk" | "progress" | "end" | "error";
  /** Sequence number. */
  sequence: number;
  /** Frame payload. */
  payload: unknown;
  /** ISO timestamp. */
  timestamp: string;
}

/** Trace event kinds an agent package can emit. */
export type AgentTraceEventKind =
  | "tool_call"
  | "tool_result"
  | "proposal"
  | "permission_check"
  | "provider_resolve"
  | "stream_start"
  | "stream_chunk"
  | "stream_end"
  | "stream_error"
  | "info"
  | "warning";

/** An agent trace event — package-owned observability. */
export interface AgentTraceEvent {
  /** Event kind. */
  kind: AgentTraceEventKind;
  /** The package id that emitted the event. */
  package_id: string;
  /** ISO timestamp. */
  timestamp: string;
  /** The capability id involved (if applicable). */
  capability_id?: string;
  /** The provider package id involved (if applicable). */
  provider_package_id?: string;
  /** The tool call id (for tool_call / tool_result correlation). */
  call_id?: string;
  /** Event payload. */
  payload: unknown;
  /** Optional correlation id. */
  correlation_id?: string;
}

/** A proposal draft — the agent's proposed action for kernel approval. */
export interface AgentProposalDraft {
  /** The package id that created the draft. */
  package_id: string;
  /** ISO timestamp. */
  timestamp: string;
  /** Human-readable title. */
  title: string;
  /** Detailed description of the proposed action. */
  description: string;
  /** The capabilities that would be invoked. */
  capability_ids: string[];
  /** The tools that would be called. */
  tool_calls: Array<{
    capability_id: string;
    input: unknown;
    provider_package_id?: string;
  }>;
  /** Risk assessment note. */
  risk_note?: string;
  /** Whether the proposal requires user confirmation. */
  requires_confirmation: boolean;
  /** Optional correlation id. */
  correlation_id?: string;
}

/** Permission diagnostics result. */
export interface PermissionDiagnostics {
  /** The capability id being checked. */
  capability_id: string;
  /** Required permissions for the capability. */
  required_permissions: string[];
  /** Whether all permissions appear to be satisfied. */
  satisfied: boolean;
  /** List of missing permissions (if any). */
  missing: string[];
}

/** Provider diagnostics result. */
export interface ProviderDiagnostics {
  /** The capability id being checked. */
  capability_id: string;
  /** Known provider package ids. */
  provider_package_ids: string[];
  /** Whether the provider is unambiguous. */
  unambiguous: boolean;
  /** If ambiguous, the reason. */
  ambiguity_reason?: string;
  /** The explicit provider to use (if resolved). */
  resolved_provider_package_id?: string;
}

/** Raw-secret scan result. */
export interface RawSecretScanResult {
  /** Whether any raw secrets were detected. */
  has_raw_secrets: boolean;
  /** The field paths that contained raw secrets. */
  flagged_fields: string[];
  /** The raw-secret values (truncated for safety). */
  flagged_values: string[];
}

/** Adapter configuration. */
export interface YggAgentAdapterConfig {
  /** Protocol client for invoking capabilities. */
  protocolClient: ProtocolClient;
  /** The package id of the adapter consumer. */
  packageId: string;
  /** Optional principal (identity) for the adapter. */
  principal?: string;
}

/** The Ygg Agent Adapter. */
export interface YggAgentAdapter {
  /** The adapter's package id. */
  readonly packageId: string;
  /** The adapter's principal (if set). */
  readonly principal: string | undefined;
  /** The underlying protocol client. */
  readonly protocolClient: ProtocolClient;
  /** Convert a capability descriptor to a tool. */
  capabilityToTool(descriptor: CapabilityDescriptor): CapabilityTool;
  /** Invoke a tool through the protocol. */
  invokeTool(call: ToolCall): Promise<ToolResult>;
  /** Stream a tool — returns a stream request + frame adapter (no real SSE). */
  streamTool(call: ToolCall): StreamRequest;
  /** Create a trace event. */
  createTraceEvent(kind: AgentTraceEventKind, payload: unknown, options?: {
    capability_id?: string;
    provider_package_id?: string;
    call_id?: string;
    correlation_id?: string;
  }): AgentTraceEvent;
  /** Create a proposal draft. */
  createProposalDraft(title: string, description: string, toolCalls: Array<{
    capability_id: string;
    input: unknown;
    provider_package_id?: string;
  }>, options?: {
    risk_note?: string;
    requires_confirmation?: boolean;
    correlation_id?: string;
  }): AgentProposalDraft;
  /** Diagnose permissions for a tool. */
  diagnosePermissions(tool: CapabilityTool): PermissionDiagnostics;
  /** Diagnose provider ambiguity for a tool. */
  diagnoseProvider(tool: CapabilityTool, explicitProviderPackageId?: string): ProviderDiagnostics;
  /** Scan a payload for raw secrets. */
  blockRawSecrets(payload: unknown, path?: string): RawSecretScanResult;
}

// ---------------------------------------------------------------------------
// Secret field names & heuristic (mirrors sdk/typescript/secure-execution)
// ---------------------------------------------------------------------------

const SECRET_FIELD_NAMES = [
  "api_key", "apikey", "api_secret", "apisecret",
  "secret_key", "secretkey", "secret", "token",
  "access_token", "access_secret", "auth_token",
  "password", "passwd", "private_key", "privatekey",
  "credential", "credentials", "bearer_token", "x-api-key",
] as const;

const SECRET_REF_PREFIX = "secret_ref:";
const SECRET_REF_ALT_PREFIXES = ["secretRef:", "secret-ref:"];

function isValidSecretRef(s: string): boolean {
  const allPrefixes = [SECRET_REF_PREFIX, ...SECRET_REF_ALT_PREFIXES];
  for (const p of allPrefixes) {
    if (s.startsWith(p)) {
      const afterPrefix = s.slice(p.length);
      return afterPrefix.includes(":") && afterPrefix.length > 2;
    }
  }
  if (s.startsWith("host:")) {
    return s.length > 5;
  }
  return false;
}

function looksLikeRawSecret(value: string): boolean {
  if (isValidSecretRef(value)) return false;
  if (value.startsWith("Bearer ") || value.startsWith("bearer ")) return true;
  if (value.startsWith("sk-") || value.startsWith("sk_")) return true;
  if (value.length >= 32) {
    const alphanum = /^[\w.-]+$/;
    if (alphanum.test(value)) {
      const hasUpper = /[A-Z]/.test(value);
      const hasLower = /[a-z]/.test(value);
      const hasDigit = /[0-9]/.test(value);
      if (hasUpper && hasLower && hasDigit) return true;
      if (/^[0-9a-f]+$/i.test(value) && value.length >= 32) return true;
    }
  }
  return false;
}

function isSecretFieldName(fieldName: string): boolean {
  const lower = fieldName.toLowerCase();
  return SECRET_FIELD_NAMES.some((n) => lower === n)
    || (lower.includes("secret")
      && !lower.includes("secret_ref")
      && !lower.includes("secretref")
      && !lower.includes("secret-ref"));
}

// ---------------------------------------------------------------------------
// capabilityToTool
// ---------------------------------------------------------------------------

/**
 * Convert a CapabilityDescriptor to a CapabilityTool.
 *
 * Derives `name` from `capability_id` (last segment) if not provided.
 */
export function capabilityToTool(descriptor: CapabilityDescriptor): CapabilityTool {
  const segments = descriptor.capability_id.split("/");
  const derivedName = descriptor.name ?? segments[segments.length - 1] ?? descriptor.capability_id;

  return {
    capability_id: descriptor.capability_id,
    name: derivedName,
    description: descriptor.description ?? "",
    input_schema: descriptor.input_schema ?? { type: "object", properties: {} },
    streamable: descriptor.streamable ?? false,
    provider_package_ids: descriptor.provider_package_ids ?? [],
    required_permissions: descriptor.required_permissions ?? [],
  };
}

// ---------------------------------------------------------------------------
// createCapabilityTool
// ---------------------------------------------------------------------------

/**
 * Create a CapabilityTool from individual fields.
 */
export function createCapabilityTool(options: {
  capability_id: string;
  name?: string;
  description?: string;
  input_schema?: Record<string, unknown>;
  streamable?: boolean;
  provider_package_ids?: string[];
  required_permissions?: string[];
}): CapabilityTool {
  return capabilityToTool({
    capability_id: options.capability_id,
    name: options.name,
    description: options.description,
    input_schema: options.input_schema,
    streamable: options.streamable,
    provider_package_ids: options.provider_package_ids,
    required_permissions: options.required_permissions,
  });
}

// ---------------------------------------------------------------------------
// invokeCapabilityTool
// ---------------------------------------------------------------------------

/**
 * Invoke a capability tool through a ProtocolClient.
 *
 * Rejects if the tool has ambiguous providers and no explicit
 * `provider_package_id` is provided.
 */
export async function invokeCapabilityTool(
  client: ProtocolClient,
  call: ToolCall,
): Promise<ToolResult> {
  // Reject ambiguous provider
  if (call.tool.provider_package_ids.length > 1 && !call.provider_package_id) {
    return {
      ok: false,
      error: `Ambiguous provider: capability "${call.tool.capability_id}" has ` +
        `${call.tool.provider_package_ids.length} providers ` +
        `(${call.tool.provider_package_ids.join(", ")}). ` +
        `Specify provider_package_id explicitly.`,
      capability_id: call.tool.capability_id,
    };
  }

  const request: ProtocolRequest = {
    capability_id: call.tool.capability_id,
    input: call.input,
    provider_package_id: call.provider_package_id ?? (
      call.tool.provider_package_ids.length === 1
        ? call.tool.provider_package_ids[0]
        : undefined
    ),
    session_id: call.session_id,
    correlation_id: call.correlation_id,
  };

  const response = await client.call(request);

  return {
    ok: response.ok,
    output: response.output,
    error: response.error,
    capability_id: response.capability_id,
    provider_package_id: response.provider_package_id,
    trace_id: response.trace_id,
  };
}

// ---------------------------------------------------------------------------
// StreamFrameAdapter implementation
// ---------------------------------------------------------------------------

function createStreamFrameAdapter(): StreamFrameAdapter {
  let seq = -1;
  let ended = false;

  function checkAlive(operation: string): void {
    if (ended) {
      throw new Error(`Cannot ${operation}: stream is in terminal state`);
    }
  }

  function nextSeq(): number {
    seq++;
    return seq;
  }

  return {
    start(payload?: unknown): StreamAdapterFrame {
      checkAlive("start");
      return { kind: "start", sequence: nextSeq(), payload: payload ?? null, timestamp: new Date().toISOString() };
    },
    chunk(payload: unknown): StreamAdapterFrame {
      checkAlive("append chunk");
      return { kind: "chunk", sequence: nextSeq(), payload, timestamp: new Date().toISOString() };
    },
    progress(metadata?: Record<string, unknown>): StreamAdapterFrame {
      checkAlive("append progress");
      return { kind: "progress", sequence: nextSeq(), payload: metadata ?? null, timestamp: new Date().toISOString() };
    },
    end(): StreamAdapterFrame {
      ended = true;
      return { kind: "end", sequence: nextSeq(), payload: null, timestamp: new Date().toISOString() };
    },
    error(message: string): StreamAdapterFrame {
      ended = true;
      return { kind: "error", sequence: nextSeq(), payload: { error: message }, timestamp: new Date().toISOString() };
    },
    frameCount(): number {
      return seq + 1;
    },
  };
}

// ---------------------------------------------------------------------------
// streamCapabilityTool
// ---------------------------------------------------------------------------

/**
 * Create a stream request for a capability tool.
 *
 * Returns the protocol request that would initiate the stream, plus a
 * frame adapter for building faux stream frame sequences. This does not
 * perform real SSE — it only prepares the request and provides a
 * frame-construction helper.
 *
 * Rejects if the tool has ambiguous providers and no explicit
 * `provider_package_id` is provided (throws).
 */
export function streamCapabilityTool(call: ToolCall): StreamRequest {
  if (call.tool.provider_package_ids.length > 1 && !call.provider_package_id) {
    throw new Error(
      `Ambiguous provider: capability "${call.tool.capability_id}" has ` +
      `${call.tool.provider_package_ids.length} providers ` +
      `(${call.tool.provider_package_ids.join(", ")}). ` +
      `Specify provider_package_id explicitly.`,
    );
  }

  const request: ProtocolRequest = {
    capability_id: call.tool.capability_id,
    input: call.input,
    provider_package_id: call.provider_package_id ?? (
      call.tool.provider_package_ids.length === 1
        ? call.tool.provider_package_ids[0]
        : undefined
    ),
    session_id: call.session_id,
    correlation_id: call.correlation_id,
  };

  return {
    request,
    tool: call.tool,
    frames: createStreamFrameAdapter(),
  };
}

// ---------------------------------------------------------------------------
// createTraceEvent
// ---------------------------------------------------------------------------

/**
 * Create an AgentTraceEvent.
 */
export function createTraceEvent(
  kind: AgentTraceEventKind,
  packageId: string,
  payload: unknown,
  options?: {
    capability_id?: string;
    provider_package_id?: string;
    call_id?: string;
    correlation_id?: string;
  },
): AgentTraceEvent {
  return {
    kind,
    package_id: packageId,
    timestamp: new Date().toISOString(),
    capability_id: options?.capability_id,
    provider_package_id: options?.provider_package_id,
    call_id: options?.call_id,
    payload,
    correlation_id: options?.correlation_id,
  };
}

// ---------------------------------------------------------------------------
// createProposalDraft
// ---------------------------------------------------------------------------

/**
 * Create an AgentProposalDraft.
 */
export function createProposalDraft(
  packageId: string,
  title: string,
  description: string,
  toolCalls: Array<{
    capability_id: string;
    input: unknown;
    provider_package_id?: string;
  }>,
  options?: {
    risk_note?: string;
    requires_confirmation?: boolean;
    correlation_id?: string;
  },
): AgentProposalDraft {
  return {
    package_id: packageId,
    timestamp: new Date().toISOString(),
    title,
    description,
    capability_ids: toolCalls.map((tc) => tc.capability_id),
    tool_calls: toolCalls,
    risk_note: options?.risk_note,
    requires_confirmation: options?.requires_confirmation ?? true,
    correlation_id: options?.correlation_id,
  };
}

// ---------------------------------------------------------------------------
// diagnosePermissions
// ---------------------------------------------------------------------------

/**
 * Diagnose permission requirements for a capability tool.
 *
 * This is a client-side check — it does not query the kernel.v1. It
 * reports which permissions a tool requires and marks them as
 * "missing" since the adapter has no access to the actual grant
 * state. Consumers should use `kernel.v1.capability.describe` for
 * authoritative permission checks.
 */
export function diagnosePermissions(tool: CapabilityTool): PermissionDiagnostics {
  const required = tool.required_permissions;
  // We cannot know what the kernel has actually granted, so all
  // required permissions are reported as "missing" by default.
  // If no permissions are required, satisfied=true.
  return {
    capability_id: tool.capability_id,
    required_permissions: required,
    satisfied: required.length === 0,
    missing: [...required],
  };
}

// ---------------------------------------------------------------------------
// diagnoseProvider
// ---------------------------------------------------------------------------

/**
 * Diagnose provider ambiguity for a capability tool.
 *
 * If the tool has exactly one provider, or an explicit provider is
 * given, the provider is resolved. If there are zero providers,
 * the situation is considered unambiguous but unresolved. If there
 * are multiple providers and no explicit choice, the provider is
 * ambiguous.
 */
export function diagnoseProvider(
  tool: CapabilityTool,
  explicitProviderPackageId?: string,
): ProviderDiagnostics {
  const providers = tool.provider_package_ids;

  if (explicitProviderPackageId) {
    return {
      capability_id: tool.capability_id,
      provider_package_ids: providers,
      unambiguous: true,
      resolved_provider_package_id: explicitProviderPackageId,
    };
  }

  if (providers.length === 0) {
    return {
      capability_id: tool.capability_id,
      provider_package_ids: [],
      unambiguous: true,
      ambiguity_reason: undefined,
      resolved_provider_package_id: undefined,
    };
  }

  if (providers.length === 1) {
    return {
      capability_id: tool.capability_id,
      provider_package_ids: providers,
      unambiguous: true,
      resolved_provider_package_id: providers[0],
    };
  }

  return {
    capability_id: tool.capability_id,
    provider_package_ids: providers,
    unambiguous: false,
    ambiguity_reason:
      `Capability "${tool.capability_id}" has ${providers.length} providers: ` +
      `${providers.join(", ")}. Specify provider_package_id explicitly.`,
  };
}

// ---------------------------------------------------------------------------
// blockRawSecrets
// ---------------------------------------------------------------------------

/**
 * Scan a payload for raw secrets and return a report.
 *
 * Walks the payload tree recursively. Flags string values that look
 * like raw secrets and field names that are known secret fields but
 * contain non-reference values.
 *
 * Does NOT import or depend on private runtime — re-implements the
 * secret_ref heuristic locally.
 */
export function blockRawSecrets(payload: unknown, basePath: string = ""): RawSecretScanResult {
  const flaggedFields: string[] = [];
  const flaggedValues: string[] = [];

  function walk(value: unknown, path: string): void {
    if (value === null || value === undefined) return;

    if (typeof value === "string") {
      // Check if the field name itself is a secret field
      const lastKey = path.split(".").pop() ?? "";
      if (isSecretFieldName(lastKey) && !isValidSecretRef(value)) {
        flaggedFields.push(path);
        flaggedValues.push(truncate(value, 16));
      }
      // Check if the value looks like a raw secret regardless of field name
      if (looksLikeRawSecret(value)) {
        if (!flaggedFields.includes(path)) {
          flaggedFields.push(path);
          flaggedValues.push(truncate(value, 16));
        }
      }
      return;
    }

    if (Array.isArray(value)) {
      for (let i = 0; i < value.length; i++) {
        walk(value[i], `${path}[${i}]`);
      }
      return;
    }

    if (typeof value === "object") {
      const record = value as Record<string, unknown>;
      for (const key of Object.keys(record)) {
        const childPath = path ? `${path}.${key}` : key;
        walk(record[key], childPath);
      }
    }
  }

  walk(payload, basePath);

  return {
    has_raw_secrets: flaggedFields.length > 0,
    flagged_fields: flaggedFields,
    flagged_values: flaggedValues,
  };
}

function truncate(s: string, maxLen: number): string {
  if (s.length <= maxLen) return s;
  return s.slice(0, maxLen) + "…";
}

// ---------------------------------------------------------------------------
// createYggAgentAdapter
// ---------------------------------------------------------------------------

/**
 * Create a YggAgentAdapter — the main entry point.
 *
 * The adapter wraps a ProtocolClient and provides tool mapping,
 * invocation, streaming, trace event creation, proposal drafting,
 * and permission/provider diagnostics.
 *
 * @param config — adapter configuration
 * @returns YggAgentAdapter
 */
export function createYggAgentAdapter(config: YggAgentAdapterConfig): YggAgentAdapter {
  const { protocolClient, packageId, principal } = config;

  return {
    get packageId() { return packageId; },
    get principal() { return principal; },
    get protocolClient() { return protocolClient; },

    capabilityToTool(descriptor: CapabilityDescriptor): CapabilityTool {
      return capabilityToTool(descriptor);
    },

    async invokeTool(call: ToolCall): Promise<ToolResult> {
      return invokeCapabilityTool(protocolClient, call);
    },

    streamTool(call: ToolCall): StreamRequest {
      return streamCapabilityTool(call);
    },

    createTraceEvent(
      kind: AgentTraceEventKind,
      payload: unknown,
      options?: {
        capability_id?: string;
        provider_package_id?: string;
        call_id?: string;
        correlation_id?: string;
      },
    ): AgentTraceEvent {
      return createTraceEvent(kind, packageId, payload, options);
    },

    createProposalDraft(
      title: string,
      description: string,
      toolCalls: Array<{
        capability_id: string;
        input: unknown;
        provider_package_id?: string;
      }>,
      options?: {
        risk_note?: string;
        requires_confirmation?: boolean;
        correlation_id?: string;
      },
    ): AgentProposalDraft {
      return createProposalDraft(packageId, title, description, toolCalls, options);
    },

    diagnosePermissions(tool: CapabilityTool): PermissionDiagnostics {
      return diagnosePermissions(tool);
    },

    diagnoseProvider(tool: CapabilityTool, explicitProviderPackageId?: string): ProviderDiagnostics {
      return diagnoseProvider(tool, explicitProviderPackageId);
    },

    blockRawSecrets(payload: unknown, path?: string): RawSecretScanResult {
      return blockRawSecrets(payload, path);
    },
  };
}

// ---------------------------------------------------------------------------
// Self-test
// ---------------------------------------------------------------------------

/**
 * Pure TypeScript self-test for the agent-adapter SDK.
 *
 * Covers:
 * - tool mapping (capabilityToTool / createCapabilityTool)
 * - ambiguous provider rejection
 * - proposal draft creation
 * - trace event creation
 * - stream request + frame adapter
 * - raw secret blocking
 *
 * @returns array of { name, passed, detail? }
 */
export async function runYggAgentAdapterSelfTest(): Promise<Array<{ name: string; passed: boolean; detail?: string }>> {
  const results: Array<{ name: string; passed: boolean; detail?: string }> = [];

  // Helper
  function assert(name: string, condition: boolean, detail?: string): void {
    results.push({ name, passed: condition, detail: condition ? undefined : (detail ?? "assertion failed") });
  }

  // --- 1. capabilityToTool ---
  {
    const tool = capabilityToTool({
      capability_id: "example/pkg/search",
      name: "Search",
      description: "Search documents",
      streamable: true,
      provider_package_ids: ["pkg/a", "pkg/b"],
      required_permissions: ["network:outbound"],
    });
    assert("capabilityToTool: capability_id", tool.capability_id === "example/pkg/search");
    assert("capabilityToTool: name", tool.name === "Search");
    assert("capabilityToTool: description", tool.description === "Search documents");
    assert("capabilityToTool: streamable", tool.streamable === true);
    assert("capabilityToTool: providers", tool.provider_package_ids.length === 2);
    assert("capabilityToTool: permissions", tool.required_permissions.length === 1);
  }

  // --- 2. capabilityToTool name derivation ---
  {
    const tool = capabilityToTool({ capability_id: "foo/bar/baz" });
    assert("capabilityToTool: derived name from last segment", tool.name === "baz");
  }

  // --- 3. createCapabilityTool ---
  {
    const tool = createCapabilityTool({
      capability_id: "x/y/z",
      name: "Z Tool",
      description: "Does Z",
    });
    assert("createCapabilityTool: name", tool.name === "Z Tool");
    assert("createCapabilityTool: default streamable", tool.streamable === false);
    assert("createCapabilityTool: default providers", tool.provider_package_ids.length === 0);
  }

  // --- 4. ambiguous provider rejection (invoke) ---
  {
    const ambiguousTool = createCapabilityTool({
      capability_id: "test/ambiguous",
      provider_package_ids: ["pkg/a", "pkg/b"],
    });
    const mockClient: ProtocolClient = {
      async call() { return { ok: true, capability_id: "test/ambiguous" }; },
    };
    const result = await invokeCapabilityTool(mockClient, {
      tool: ambiguousTool,
      input: {},
    });
    assert("ambiguous provider rejection: fails", result.ok === false);
    assert("ambiguous provider rejection: error mentions ambiguous", result.error?.includes("Ambiguous provider") === true);
  }

  // --- 5. explicit provider resolves ambiguity (invoke) ---
  {
    const ambiguousTool = createCapabilityTool({
      capability_id: "test/explicit-provider",
      provider_package_ids: ["pkg/a", "pkg/b"],
    });
    const mockClient: ProtocolClient = {
      async call(req: ProtocolRequest) {
        return {
          ok: true,
          capability_id: req.capability_id,
          provider_package_id: req.provider_package_id,
        };
      },
    };
    const result = await invokeCapabilityTool(mockClient, {
      tool: ambiguousTool,
      input: {},
      provider_package_id: "pkg/a",
    });
    assert("explicit provider resolves: ok", result.ok === true);
    assert("explicit provider resolves: provider", result.provider_package_id === "pkg/a");
  }

  // --- 6. single provider auto-resolves (invoke) ---
  {
    const singleProviderTool = createCapabilityTool({
      capability_id: "test/single-provider",
      provider_package_ids: ["pkg/only"],
    });
    const mockClient: ProtocolClient = {
      async call(req: ProtocolRequest) {
        return {
          ok: true,
          capability_id: req.capability_id,
          provider_package_id: req.provider_package_id,
        };
      },
    };
    const result = await invokeCapabilityTool(mockClient, {
      tool: singleProviderTool,
      input: { query: "hello" },
    });
    assert("single provider auto-resolves: ok", result.ok === true);
    assert("single provider auto-resolves: provider", result.provider_package_id === "pkg/only");
  }

  // --- 7. ambiguous provider rejection (stream) ---
  {
    const ambiguousTool = createCapabilityTool({
      capability_id: "test/stream-ambiguous",
      provider_package_ids: ["pkg/x", "pkg/y"],
    });
    let threw = false;
    let message = "";
    try {
      streamCapabilityTool({ tool: ambiguousTool, input: {} });
    } catch (e) {
      threw = true;
      message = e instanceof Error ? e.message : String(e);
    }
    assert("stream ambiguous rejection: throws", threw);
    assert("stream ambiguous rejection: message", message.includes("Ambiguous provider"));
  }

  // --- 8. stream request construction ---
  {
    const streamTool = createCapabilityTool({
      capability_id: "test/stream-ok",
      streamable: true,
      provider_package_ids: ["pkg/s"],
    });
    const streamReq = streamCapabilityTool({ tool: streamTool, input: { prompt: "hi" } });
    assert("stream request: capability_id", streamReq.request.capability_id === "test/stream-ok");
    assert("stream request: provider", streamReq.request.provider_package_id === "pkg/s");
    assert("stream request: frames adapter", streamReq.frames !== null && typeof streamReq.frames === "object");
  }

  // --- 9. stream frame adapter ---
  {
    const streamTool = createCapabilityTool({
      capability_id: "test/frames",
      streamable: true,
      provider_package_ids: ["pkg/f"],
    });
    const { frames } = streamCapabilityTool({ tool: streamTool, input: {} });
    const start = frames.start({ init: true });
    assert("frame adapter: start kind", start.kind === "start");
    assert("frame adapter: start sequence", start.sequence === 0);
    const chunk = frames.chunk({ text: "hello" });
    assert("frame adapter: chunk kind", chunk.kind === "chunk");
    assert("frame adapter: chunk sequence", chunk.sequence === 1);
    const prog = frames.progress({ pct: 50 });
    assert("frame adapter: progress kind", prog.kind === "progress");
    const end = frames.end();
    assert("frame adapter: end kind", end.kind === "end");
    assert("frame adapter: frameCount", frames.frameCount() === 4);
  }

  // --- 10. createTraceEvent ---
  {
    const ev = createTraceEvent("tool_call", "my/pkg", { tool: "search" }, {
      capability_id: "search/query",
      call_id: "call-1",
    });
    assert("trace event: kind", ev.kind === "tool_call");
    assert("trace event: package_id", ev.package_id === "my/pkg");
    assert("trace event: capability_id", ev.capability_id === "search/query");
    assert("trace event: call_id", ev.call_id === "call-1");
    assert("trace event: timestamp", typeof ev.timestamp === "string" && ev.timestamp.length > 0);
  }

  // --- 11. createProposalDraft ---
  {
    const draft = createProposalDraft("my/pkg", "Search", "Run a search", [
      { capability_id: "search/query", input: { q: "test" }, provider_package_id: "pkg/s" },
    ], {
      risk_note: "Read-only, low risk",
      requires_confirmation: false,
      correlation_id: "corr-1",
    });
    assert("proposal: package_id", draft.package_id === "my/pkg");
    assert("proposal: title", draft.title === "Search");
    assert("proposal: description", draft.description === "Run a search");
    assert("proposal: capability_ids", draft.capability_ids.length === 1 && draft.capability_ids[0] === "search/query");
    assert("proposal: tool_calls", draft.tool_calls.length === 1);
    assert("proposal: risk_note", draft.risk_note === "Read-only, low risk");
    assert("proposal: requires_confirmation", draft.requires_confirmation === false);
    assert("proposal: correlation_id", draft.correlation_id === "corr-1");
  }

  // --- 12. diagnosePermissions ---
  {
    const tool = createCapabilityTool({
      capability_id: "test/perm",
      required_permissions: ["network:outbound", "fs:read"],
    });
    const diag = diagnosePermissions(tool);
    assert("permissions: capability_id", diag.capability_id === "test/perm");
    assert("permissions: required count", diag.required_permissions.length === 2);
    assert("permissions: not satisfied", diag.satisfied === false);
    assert("permissions: missing count", diag.missing.length === 2);
  }

  // --- 13. diagnosePermissions (no permissions) ---
  {
    const tool = createCapabilityTool({ capability_id: "test/no-perm" });
    const diag = diagnosePermissions(tool);
    assert("permissions (none): satisfied", diag.satisfied === true);
    assert("permissions (none): no missing", diag.missing.length === 0);
  }

  // --- 14. diagnoseProvider (ambiguous) ---
  {
    const tool = createCapabilityTool({
      capability_id: "test/amb-provider",
      provider_package_ids: ["pkg/a", "pkg/b"],
    });
    const diag = diagnoseProvider(tool);
    assert("provider: ambiguous", diag.unambiguous === false);
    assert("provider: has reason", typeof diag.ambiguity_reason === "string" && diag.ambiguity_reason.length > 0);
    assert("provider: no resolved", diag.resolved_provider_package_id === undefined);
  }

  // --- 15. diagnoseProvider (explicit resolves) ---
  {
    const tool = createCapabilityTool({
      capability_id: "test/explicit-resolve",
      provider_package_ids: ["pkg/a", "pkg/b"],
    });
    const diag = diagnoseProvider(tool, "pkg/b");
    assert("provider (explicit): unambiguous", diag.unambiguous === true);
    assert("provider (explicit): resolved", diag.resolved_provider_package_id === "pkg/b");
  }

  // --- 16. diagnoseProvider (single) ---
  {
    const tool = createCapabilityTool({
      capability_id: "test/single-prov",
      provider_package_ids: ["pkg/solo"],
    });
    const diag = diagnoseProvider(tool);
    assert("provider (single): unambiguous", diag.unambiguous === true);
    assert("provider (single): resolved", diag.resolved_provider_package_id === "pkg/solo");
  }

  // --- 17. blockRawSecrets (clean payload) ---
  {
    const result = blockRawSecrets({ query: "hello", limit: 10 });
    assert("raw secrets (clean): no secrets", result.has_raw_secrets === false);
    assert("raw secrets (clean): no fields", result.flagged_fields.length === 0);
  }

  // --- 18. blockRawSecrets (secret field with raw value) ---
  {
    const result = blockRawSecrets({
      query: "hello",
      api_key: "sk-abc123longvalue12345678901234",
    });
    assert("raw secrets (api_key): has secrets", result.has_raw_secrets === true);
    assert("raw secrets (api_key): flagged field", result.flagged_fields.some((f) => f.includes("api_key")));
  }

  // --- 19. blockRawSecrets (secret_ref is ok) ---
  {
    const result = blockRawSecrets({
      api_key: "secret_ref:env:MY_KEY",
    });
    assert("raw secrets (secret_ref): no secrets", result.has_raw_secrets === false);
  }

  // --- 20. blockRawSecrets (Bearer token) ---
  {
    const result = blockRawSecrets({
      authorization: "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9",
    });
    assert("raw secrets (Bearer): has secrets", result.has_raw_secrets === true);
    assert("raw secrets (Bearer): flagged", result.flagged_fields.some((f) => f.includes("authorization")));
  }

  // --- 21. createYggAgentAdapter end-to-end ---
  {
    const mockClient: ProtocolClient = {
      async call(req: ProtocolRequest) {
        return { ok: true, capability_id: req.capability_id, provider_package_id: req.provider_package_id };
      },
    };
    const adapter = createYggAgentAdapter({
      protocolClient: mockClient,
      packageId: "test/adapter-pkg",
      principal: "user:alice",
    });
    assert("adapter: packageId", adapter.packageId === "test/adapter-pkg");
    assert("adapter: principal", adapter.principal === "user:alice");

    const tool = adapter.capabilityToTool({
      capability_id: "test/echo",
      name: "Echo",
      provider_package_ids: ["test/echo-impl"],
    });
    assert("adapter: tool name", tool.name === "Echo");

    const result = await adapter.invokeTool({ tool, input: { msg: "hi" } });
    assert("adapter: invoke ok", result.ok === true);

    const trace = adapter.createTraceEvent("tool_call", { tool: "echo" }, { capability_id: "test/echo" });
    assert("adapter: trace package_id", trace.package_id === "test/adapter-pkg");

    const draft = adapter.createProposalDraft("Echo", "Echo input", [
      { capability_id: "test/echo", input: { msg: "hi" } },
    ]);
    assert("adapter: draft package_id", draft.package_id === "test/adapter-pkg");

    const permDiag = adapter.diagnosePermissions(tool);
    assert("adapter: permissions", permDiag.capability_id === "test/echo");

    const provDiag = adapter.diagnoseProvider(tool);
    assert("adapter: provider unambiguous", provDiag.unambiguous === true);
  }

  return results;
}

// Auto-run self-test when executed directly (Node.js / tsx)
// Detection: this file is the main module being run
// We use a non-intrusive check so bundlers don't trip
if (typeof globalThis !== "undefined" && typeof (globalThis as Record<string, unknown>).__plurora_agent_adapter_self_test_auto !== "undefined") {
  const results = runYggAgentAdapterSelfTest();
  const failed = results.filter((r) => !r.passed);
  if (failed.length > 0) {
    for (const f of failed) {
      console.error(`FAIL: ${f.name} — ${f.detail}`);
    }
    throw new Error(`${failed.length} self-test(s) failed`);
  }
}
