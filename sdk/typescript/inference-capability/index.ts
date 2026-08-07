/**
 * Plurora Inference Capability SDK — Transport-neutral inference envelope,
 * stream frames, error taxonomy, and provider capability manifest helpers.
 *
 * This module defines the **inference capability contract** at the package/SDK
 * layer. It does NOT enter the kernel, does NOT add Rust protocol methods, and
 * does NOT require URL/header/status-code/OpenAI-messages fields.
 *
 * ## Design principles
 *
 * - **API-first but not API-shaped**: Cloud adapters are one kind of provider;
 *   local/self-hosted/non-HTTP providers are first-class.
 * - **Transport-neutral**: The envelope carries a `transport_kind` hint, not
 *   transport-specific fields like URLs or HTTP headers.
 * - **No canonical chat schema**: No `system`/`user`/`assistant` role fields.
 *   Input is opaque payload refs, not message arrays.
 * - **Secret-safe**: Uses `secret_ref` identifiers; rejects raw secrets.
 * - **Content-free kernel alignment**: No `platform.model.*`, `platform.prompt.*`,
 *   `platform.chat.*`, or `platform.embedding.*` coupling.
 *
 * ## API surface
 *
 * Types:
 * - `InferenceOperationKind`        — what kind of inference
 * - `TransportKind`                 — transport hint enum
 * - `InferenceRequest`              — transport-neutral inference request envelope
 * - `InferenceResponse`             — transport-neutral inference response envelope
 * - `InferenceStreamFrameKind`      — canonical stream frame types
 * - `InferenceStreamFrame`          — canonical stream frame
 * - `InferenceErrorKind`            — transport-neutral error taxonomy
 * - `InferenceError`                — classified error
 * - `ProviderCapabilityManifest`    — provider capability declaration
 * - `ModalityKind` / `RuntimeKind` — provider manifest hints
 *
 * Helpers:
 * - `createInferenceRequest()`      — build a valid request, reject raw secrets
 * - `validateInferenceRequest()`    — validate request, return diagnostics
 * - `classifyInferenceError()`      — map raw error → InferenceError taxonomy
 * - `InferenceStreamLifecycle`      — stream frame lifecycle builder
 * - `createProviderCapabilityManifest()` — build a provider manifest
 * - `validateProviderCapabilityManifest()` — validate a provider manifest
 * - `runInferenceCapabilitySelfTest()` — pure-TS self-test
 */

// ---------------------------------------------------------------------------
// Operation kinds — what kind of inference (not "what API endpoint")
// ---------------------------------------------------------------------------

/** Kind of inference operation. Extensible — not limited to chat/embed. */
export type InferenceOperationKind =
  | "generate"
  | "classify"
  | "embed"
  | "rank"
  | "score"
  | "transform"
  | "analyze"
  | "summarize"
  | "extract"
  | "custom";

// ---------------------------------------------------------------------------
// Transport kind — how the provider is reached (hint, not requirement)
// ---------------------------------------------------------------------------

/** Transport kind hints. Not URL/header/status — just a semantic hint. */
export type TransportKind =
  | "http"
  | "local_process"
  | "in_memory"
  | "ipc"
  | "websocket"
  | "remote"
  | "custom";

// ---------------------------------------------------------------------------
// Modality and runtime kinds for provider manifest
// ---------------------------------------------------------------------------

/** Input/output modality kinds. */
export type ModalityKind =
  | "text"
  | "image"
  | "audio"
  | "video"
  | "code"
  | "structured"
  | "custom";

/** Runtime environment kind. */
export type RuntimeKind =
  | "cloud_api"
  | "local_process"
  | "in_memory"
  | "gpu_local"
  | "cpu_local"
  | "wasm"
  | "custom";

// ---------------------------------------------------------------------------
// Inference request envelope
// ---------------------------------------------------------------------------

/** Input artifact reference — opaque, not a URL or file path. */
export interface InputRef {
  /** Unique reference id (package-defined, opaque). */
  ref_id: string;
  /** Optional MIME type hint. */
  mime_hint?: string;
  /** Opaque metadata (no raw secrets). */
  metadata?: Record<string, unknown>;
}

/** Opaque input payload — not a message array, not a prompt string. */
export interface OpaquePayload {
  /** Payload kind (package-defined, e.g. "json", "binary", "text"). */
  kind: string;
  /** Payload shape description (not the raw payload body). */
  shape?: Record<string, unknown>;
  /** Opaque metadata. */
  metadata?: Record<string, unknown>;
}

/** Resource hints — caller guidance for the provider. */
export interface ResourceHints {
  /** Preferred max output tokens/units (if applicable). */
  max_output_units?: number;
  /** Preferred temperature/sampling (0.0–2.0, if applicable). */
  temperature?: number;
  /** GPU/memory budget hint (provider-specific). */
  compute_budget?: Record<string, unknown>;
  /** Latency preference hint. */
  latency_preference?: "low" | "balanced" | "best_effort";
  /** Opaque provider-specific hints. */
  custom?: Record<string, unknown>;
}

/** Cancellation signal for an inference request. */
export interface CancellationSignal {
  /** Deadline as ISO-8601 timestamp. */
  deadline?: string;
  /** Explicit cancel flag. */
  cancelled?: boolean;
  /** Reason for cancellation (if any). */
  reason?: string;
}

/**
 * Transport-neutral inference request envelope.
 *
 * This is the **canonical request shape** for any inference capability,
 * regardless of whether the provider is a cloud API, a local process,
 * an in-memory compute graph, or any other transport.
 *
 * Invariants:
 * - No URL, no HTTP header, no status code, no OpenAI messages.
 * - Secrets via `secret_refs` only; raw secrets rejected.
 * - `input_refs` and `input_payload` are opaque; no system/user/assistant.
 */
export interface InferenceRequest {
  /** Operation id — unique per invocation, caller-assigned. */
  operation_id: string;
  /** Operation kind — what kind of inference. */
  operation_kind: InferenceOperationKind;
  /** Input artifact references (opaque, not URLs). */
  input_refs: InputRef[];
  /** Opaque input payload (not a message array). */
  input_payload?: OpaquePayload;
  /** Whether streaming is requested. */
  streaming: boolean;
  /** Deadline and cancellation. */
  cancellation?: CancellationSignal;
  /** Resource hints for the provider. */
  resource_hints?: ResourceHints;
  /** Secret references — must be `secret_ref:*` or `host:*`. */
  secret_refs: string[];
  /** Transport kind hint — how the provider prefers to be reached. */
  transport_kind: TransportKind;
  /** Opaque metadata (no raw secrets). */
  metadata?: Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// Inference response envelope
// ---------------------------------------------------------------------------

/** Transport-neutral inference response. */
export interface InferenceResponse {
  /** The operation id this response belongs to. */
  operation_id: string;
  /** Whether the operation succeeded. */
  success: boolean;
  /** Output refs (opaque, not URLs). */
  output_refs: Array<{
    ref_id: string;
    mime_hint?: string;
    metadata?: Record<string, unknown>;
  }>;
  /** Opaque output payload description. */
  output_payload?: OpaquePayload;
  /** Usage metrics (provider-specific, no raw secrets). */
  usage?: Record<string, unknown>;
  /** Provider-assigned request id (for audit, not raw request body). */
  provider_operation_id?: string;
  /** Opaque metadata. */
  metadata?: Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// Canonical stream frames
// ---------------------------------------------------------------------------

/** Canonical inference stream frame kinds. */
export type InferenceStreamFrameKind =
  | "start"
  | "chunk"
  | "progress"
  | "end"
  | "error"
  | "cancelled"
  | "timeout";

/** A canonical inference stream frame. */
export interface InferenceStreamFrame {
  /** Operation id this frame belongs to. */
  operation_id: string;
  /** Stream id for this invocation. */
  stream_id: string;
  /** Frame kind. */
  frame_kind: InferenceStreamFrameKind;
  /** Monotonic sequence number. */
  sequence: number;
  /** ISO-8601 timestamp. */
  timestamp: string;
  /** Opaque payload (chunk data, progress info, error detail, etc.). */
  payload: unknown;
  /** Frame metadata. */
  metadata: Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// Transport-neutral error taxonomy
// ---------------------------------------------------------------------------

/**
 * Transport-neutral inference error taxonomy.
 *
 * Covers cloud API errors AND local/resource errors equally.
 * No HTTP status code dependency — errors can originate from any transport.
 */
export type InferenceErrorKind =
  // --- Cloud/provider errors ---
  | "authentication"          // credential invalid/expired
  | "permission"              // not authorized for operation
  | "billing"                 // quota/billing exhausted
  | "rate_limit"              // throttled
  | "provider_overloaded"     // upstream overloaded
  | "provider_unavailable"    // upstream unreachable
  | "bad_request"             // malformed request
  | "not_found"               // model/resource not found
  | "provider_error"          // generic upstream error
  // --- Local/resource errors ---
  | "local_process_failed"    // local process crashed/errored
  | "local_process_timeout"   // local process exceeded deadline
  | "local_resource_exhausted" // OOM, GPU memory, disk, etc.
  | "local_model_not_loaded"  // local model not available
  | "local_inference_error"    // error during local inference
  // --- Cross-cutting errors ---
  | "timeout"                 // operation deadline exceeded
  | "cancelled"               // operation cancelled by caller
  | "secret_unavailable"      // secret ref could not be resolved
  | "network_denied"          // network access denied by policy
  | "input_invalid"           // input refs/payload validation failure
  | "transport_error"         // transport-level failure (connection, IPC, etc.)
  | "stream_error"            // mid-stream failure
  | "tool_schema"             // tool/input schema mismatch
  | "unknown";                // unclassifiable

/** Error stage — where in the lifecycle the error occurred. */
export type InferenceErrorStage =
  | "preflight"    // before invocation (validation, credential resolution)
  | "invocation"   // during invocation (transport, request)
  | "stream"       // mid-stream
  | "postprocess"; // after invocation (response processing)

/** A classified inference error. */
export interface InferenceError {
  /** Stable error kind. */
  kind: InferenceErrorKind;
  /** Whether the operation is retryable. */
  retryable: boolean;
  /** Error stage. */
  stage: InferenceErrorStage;
  /** Provider or transport kind that produced this error. */
  source_kind?: TransportKind | string;
  /** Human-readable message (no raw secrets). */
  message: string;
  /** Source error code (provider-specific, opaque). */
  source_code?: string;
  /** Opaque detail (no raw body/secret). */
  detail?: Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// Provider capability manifest
// ---------------------------------------------------------------------------

/** A provider capability manifest — declares what a provider supports. */
export interface ProviderCapabilityManifest {
  /** Manifest version. */
  version: 1;
  /** Provider id (package-defined). */
  provider_id: string;
  /** Human-readable label. */
  label: string;
  /** Description. */
  description?: string;
  /** Supported operation kinds. */
  operation_kinds: InferenceOperationKind[];
  /** Supported input modalities. */
  input_modalities: ModalityKind[];
  /** Supported output modalities. */
  output_modalities: ModalityKind[];
  /** Supported transport kinds. */
  transport_kinds: TransportKind[];
  /** Runtime environment kind. */
  runtime_kind: RuntimeKind;
  /** Whether streaming is supported. */
  streaming_supported: boolean;
  /** Whether secrets are required (and must be secret_ref). */
  secrets_required: boolean;
  /** Network requirements. */
  network_required: boolean;
  /** Resource constraints (provider-specific). */
  resource_constraints?: Record<string, unknown>;
  /** Supported resource hints. */
  supported_resource_hints?: string[];
  /** Opaque metadata. */
  metadata?: Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// Secret ref validation (local re-implementation, no private runtime import)
// ---------------------------------------------------------------------------

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
  if (value.startsWith("key-") || value.startsWith("key_")) return true;
  if (value.startsWith("AIza")) return true;
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

const SECRET_FIELD_NAMES = [
  "api_key", "apikey", "api_secret", "apisecret",
  "secret_key", "secretkey", "secret", "token",
  "access_token", "access_secret", "auth_token",
  "password", "passwd", "private_key", "privatekey",
  "credential", "credentials", "bearer_token",
];

function isSecretFieldName(fieldName: string): boolean {
  const lower = fieldName.toLowerCase();
  return SECRET_FIELD_NAMES.some((n) => lower === n)
    || (lower.includes("secret")
      && !lower.includes("secret_ref")
      && !lower.includes("secretref")
      && !lower.includes("secret-ref"));
}

/** Check whether a metadata-like object contains raw secrets. */
function metadataContainsRawSecret(
  obj: Record<string, unknown> | undefined,
  path: string,
  diagnostics: Array<{ field: string; message: string }>,
): void {
  if (obj === undefined) return;
  for (const [key, value] of Object.entries(obj)) {
    const fieldPath = path ? `${path}.${key}` : key;
    if (typeof value === "string" && looksLikeRawSecret(value)) {
      diagnostics.push({
        field: fieldPath,
        message: `Field "${fieldPath}" contains a raw secret. Use secret_ref: or host: reference.`,
      });
    }
    if (typeof value === "string" && isSecretFieldName(key) && !isValidSecretRef(value)) {
      diagnostics.push({
        field: fieldPath,
        message: `Secret field "${fieldPath}" must use secret_ref: or host: reference, not a raw value.`,
      });
    }
    if (typeof value === "object" && value !== null && !Array.isArray(value)) {
      metadataContainsRawSecret(value as Record<string, unknown>, fieldPath, diagnostics);
    }
  }
}

// ---------------------------------------------------------------------------
// createInferenceRequest
// ---------------------------------------------------------------------------

/** Options for creating an inference request. */
export interface CreateInferenceRequestOptions {
  operation_id: string;
  operation_kind: InferenceOperationKind;
  input_refs?: InputRef[];
  input_payload?: OpaquePayload;
  streaming?: boolean;
  cancellation?: CancellationSignal;
  resource_hints?: ResourceHints;
  secret_refs?: string[];
  transport_kind: TransportKind;
  metadata?: Record<string, unknown>;
}

/**
 * Create a transport-neutral inference request.
 *
 * Validates that no raw secrets appear in the request.
 * Throws on raw secret detection; use `validateInferenceRequest` for
 * non-throwing diagnostics.
 */
export function createInferenceRequest(
  opts: CreateInferenceRequestOptions,
): InferenceRequest {
  const diagnostics = validateInferenceRequestInternal(opts);
  if (diagnostics.length > 0) {
    const messages = diagnostics.map((d) => `${d.field}: ${d.message}`);
    throw new Error(
      `Invalid inference request:\n${messages.join("\n")}`,
    );
  }
  return {
    operation_id: opts.operation_id,
    operation_kind: opts.operation_kind,
    input_refs: opts.input_refs ?? [],
    input_payload: opts.input_payload,
    streaming: opts.streaming ?? false,
    cancellation: opts.cancellation,
    resource_hints: opts.resource_hints,
    secret_refs: opts.secret_refs ?? [],
    transport_kind: opts.transport_kind,
    metadata: opts.metadata,
  };
}

// ---------------------------------------------------------------------------
// validateInferenceRequest
// ---------------------------------------------------------------------------

/** A validation diagnostic for inference request. */
export interface InferenceValidationDiagnostic {
  /** Severity. */
  severity: "error" | "warning" | "info";
  /** Field path. */
  field: string;
  /** Human-readable message. */
  message: string;
}

/**
 * Validate an inference request without throwing.
 *
 * Returns diagnostics array — empty means valid.
 */
export function validateInferenceRequest(
  opts: CreateInferenceRequestOptions,
): InferenceValidationDiagnostic[] {
  const raw = validateInferenceRequestInternal(opts);
  return raw.map((d) => ({ severity: "error" as const, ...d }));
}

function validateInferenceRequestInternal(
  opts: CreateInferenceRequestOptions,
): Array<{ field: string; message: string }> {
  const diagnostics: Array<{ field: string; message: string }> = [];

  // Operation id required
  if (!opts.operation_id || opts.operation_id.trim() === "") {
    diagnostics.push({
      field: "operation_id",
      message: "operation_id is required and must be non-empty.",
    });
  }

  // Secret refs must be valid
  for (const ref of opts.secret_refs ?? []) {
    if (!isValidSecretRef(ref)) {
      if (looksLikeRawSecret(ref)) {
        diagnostics.push({
          field: "secret_refs",
          message: `Raw secret detected in secret_refs. Use secret_ref: or host: reference. Got: "${ref.slice(0, 20)}…"`,
        });
      } else {
        diagnostics.push({
          field: "secret_refs",
          message: `Invalid secret reference: "${ref}". Must be secret_ref:<vault>:<key> or host:<key>.`,
        });
      }
    }
  }

  // Input refs metadata must not contain raw secrets
  for (let i = 0; i < (opts.input_refs?.length ?? 0); i++) {
    const ref = opts.input_refs![i];
    metadataContainsRawSecret(ref.metadata, `input_refs[${i}].metadata`, diagnostics);
  }

  // Input payload metadata must not contain raw secrets
  if (opts.input_payload?.metadata) {
    metadataContainsRawSecret(opts.input_payload.metadata, "input_payload.metadata", diagnostics);
  }

  // Top-level metadata must not contain raw secrets
  metadataContainsRawSecret(opts.metadata, "metadata", diagnostics);

  // Resource hints custom must not contain raw secrets
  if (opts.resource_hints?.custom) {
    metadataContainsRawSecret(opts.resource_hints.custom, "resource_hints.custom", diagnostics);
  }

  // Cancellation metadata must not contain raw secrets
  if (opts.cancellation?.reason && looksLikeRawSecret(opts.cancellation.reason)) {
    diagnostics.push({
      field: "cancellation.reason",
      message: "Cancellation reason contains a raw secret. Use secret_ref: or host: reference.",
    });
  }

  return diagnostics;
}

// ---------------------------------------------------------------------------
// classifyInferenceError
// ---------------------------------------------------------------------------

/** Input for error classification. */
export interface InferenceErrorInput {
  /** Raw error message or code. */
  message: string;
  /** Error code (provider-specific, transport-specific, or generic). */
  code?: string;
  /** Transport kind where the error originated. */
  transport_kind?: TransportKind | string;
  /** Error stage (if known). */
  stage?: InferenceErrorStage;
  /** Whether the operation was cancelled. */
  cancelled?: boolean;
  /** Whether the operation timed out. */
  timed_out?: boolean;
  /** Whether a secret could not be resolved. */
  secret_unavailable?: boolean;
  /** Whether network access was denied. */
  network_denied?: boolean;
  /** HTTP status code (if cloud, used as hint only). */
  http_status_hint?: number;
  /** Opaque detail. */
  detail?: Record<string, unknown>;
}

/**
 * Classify a raw error into the transport-neutral inference error taxonomy.
 *
 * Works for cloud API errors, local process errors, IPC errors, and
 * generic failures alike.
 */
export function classifyInferenceError(input: InferenceErrorInput): InferenceError {
  const stage: InferenceErrorStage = input.stage ?? "invocation";
  const msg = input.message ?? "Unknown inference error";

  // Explicit flags first
  if (input.cancelled) {
    return {
      kind: "cancelled",
      retryable: false,
      stage,
      source_kind: input.transport_kind,
      message: msg,
      source_code: input.code,
    };
  }
  if (input.timed_out) {
    return {
      kind: "timeout",
      retryable: true,
      stage,
      source_kind: input.transport_kind,
      message: msg,
      source_code: input.code,
    };
  }
  if (input.secret_unavailable) {
    return {
      kind: "secret_unavailable",
      retryable: false,
      stage: "preflight",
      source_kind: input.transport_kind,
      message: msg,
      source_code: input.code,
    };
  }
  if (input.network_denied) {
    return {
      kind: "network_denied",
      retryable: false,
      stage: "preflight",
      source_kind: input.transport_kind,
      message: msg,
      source_code: input.code,
    };
  }

  // Code-based mapping
  const code = (input.code ?? "").toLowerCase();
  const transport = input.transport_kind;

  // Local process errors
  if (transport === "local_process" || transport === "in_memory") {
    if (code.includes("crash") || code.includes("exit") || code.includes("fail") || code.includes("sigkill") || code.includes("sigsegv")) {
      return { kind: "local_process_failed", retryable: true, stage, source_kind: transport, message: msg, source_code: input.code };
    }
    if (code.includes("oom") || code.includes("out_of_memory") || code.includes("memory") || code.includes("resource_exhausted")) {
      return { kind: "local_resource_exhausted", retryable: false, stage, source_kind: transport, message: msg, source_code: input.code };
    }
    if (code.includes("model_not_loaded") || code.includes("not_loaded") || code.includes("not_available")) {
      return { kind: "local_model_not_loaded", retryable: false, stage, source_kind: transport, message: msg, source_code: input.code };
    }
    if (code.includes("timeout") || code.includes("deadline")) {
      return { kind: "local_process_timeout", retryable: true, stage, source_kind: transport, message: msg, source_code: input.code };
    }
    if (code.includes("inference_error") || code.includes("runtime") || code.includes("compute")) {
      return { kind: "local_inference_error", retryable: true, stage, source_kind: transport, message: msg, source_code: input.code };
    }
    // Default for local: local inference error
    return { kind: "local_inference_error", retryable: false, stage, source_kind: transport, message: msg, source_code: input.code };
  }

  // Cloud / HTTP-hinted errors
  if (code.includes("auth") || code.includes("credential") || code.includes("invalid_api_key")) {
    return { kind: "authentication", retryable: false, stage, source_kind: transport, message: msg, source_code: input.code };
  }
  if (code.includes("permission") || code.includes("forbidden") || code.includes("denied")) {
    return { kind: "permission", retryable: false, stage, source_kind: transport, message: msg, source_code: input.code };
  }
  if (code.includes("billing") || code.includes("quota") || code.includes("insufficient")) {
    return { kind: "billing", retryable: false, stage, source_kind: transport, message: msg, source_code: input.code };
  }
  if (code.includes("rate_limit") || code.includes("throttl") || code.includes("429")) {
    return { kind: "rate_limit", retryable: true, stage, source_kind: transport, message: msg, source_code: input.code };
  }
  if (code.includes("overload") || code.includes("529") || code.includes("503") || code.includes("502")) {
    return { kind: "provider_overloaded", retryable: true, stage, source_kind: transport, message: msg, source_code: input.code };
  }
  if (code.includes("unavailable") || code.includes("unreachable")) {
    return { kind: "provider_unavailable", retryable: true, stage, source_kind: transport, message: msg, source_code: input.code };
  }
  if (code.includes("bad_request") || code.includes("invalid_request") || code.includes("invalid_argument")) {
    return { kind: "bad_request", retryable: false, stage, source_kind: transport, message: msg, source_code: input.code };
  }
  if (code.includes("not_found") || code.includes("model_not_found")) {
    return { kind: "not_found", retryable: false, stage, source_kind: transport, message: msg, source_code: input.code };
  }
  if (code.includes("tool") && code.includes("schema")) {
    return { kind: "tool_schema", retryable: false, stage, source_kind: transport, message: msg, source_code: input.code };
  }
  if (code.includes("stream")) {
    return { kind: "stream_error", retryable: true, stage, source_kind: transport, message: msg, source_code: input.code };
  }
  if (code.includes("input") || code.includes("validation")) {
    return { kind: "input_invalid", retryable: false, stage, source_kind: transport, message: msg, source_code: input.code };
  }
  if (code.includes("transport") || code.includes("connection") || code.includes("ipc") || code.includes("socket")) {
    return { kind: "transport_error", retryable: true, stage, source_kind: transport, message: msg, source_code: input.code };
  }

  // HTTP status hint (for cloud adapters that happen to have one)
  const httpStatus = input.http_status_hint;
  if (httpStatus !== undefined) {
    if (httpStatus === 401) return { kind: "authentication", retryable: false, stage, source_kind: transport, message: msg, source_code: input.code };
    if (httpStatus === 402) return { kind: "billing", retryable: false, stage, source_kind: transport, message: msg, source_code: input.code };
    if (httpStatus === 403) return { kind: "permission", retryable: false, stage, source_kind: transport, message: msg, source_code: input.code };
    if (httpStatus === 404) return { kind: "not_found", retryable: false, stage, source_kind: transport, message: msg, source_code: input.code };
    if (httpStatus === 422) return { kind: "input_invalid", retryable: false, stage, source_kind: transport, message: msg, source_code: input.code };
    if (httpStatus === 429) return { kind: "rate_limit", retryable: true, stage, source_kind: transport, message: msg, source_code: input.code };
    if (httpStatus === 502 || httpStatus === 503 || httpStatus === 529) return { kind: "provider_overloaded", retryable: true, stage, source_kind: transport, message: msg, source_code: input.code };
    if (httpStatus === 504) return { kind: "timeout", retryable: true, stage, source_kind: transport, message: msg, source_code: input.code };
  }

  // Default
  return { kind: "unknown", retryable: false, stage, source_kind: transport, message: msg, source_code: input.code, detail: input.detail };
}

// ---------------------------------------------------------------------------
// InferenceStreamLifecycle — stream frame lifecycle builder
// ---------------------------------------------------------------------------

/**
 * Builder for inference stream frame lifecycle.
 *
 * Produces a valid sequence of InferenceStreamFrame objects:
 * start → (chunk|progress)* → (end|error|cancelled|timeout)
 *
 * Terminal states block further frame emission.
 *
 * Usage:
 * ```ts
 * const lifecycle = new InferenceStreamLifecycle("op_123", "str_abc");
 * const start = lifecycle.start({ capability_id: "inference/generate" });
 * const chunk = lifecycle.chunk({ text_delta: "hello" });
 * const end = lifecycle.end();
 * ```
 */
export class InferenceStreamLifecycle {
  private frameCount: number;
  private ended: boolean;

  constructor(
    private readonly operationId: string,
    private readonly streamId: string,
  ) {
    this.frameCount = 0;
    this.ended = false;
  }

  private nextFrame(
    kind: InferenceStreamFrameKind,
    payload: unknown,
    metadata: Record<string, unknown> = {},
  ): InferenceStreamFrame {
    const seq = this.frameCount;
    this.frameCount++;
    return {
      operation_id: this.operationId,
      stream_id: this.streamId,
      frame_kind: kind,
      sequence: seq,
      timestamp: new Date().toISOString(),
      payload,
      metadata,
    };
  }

  private ensureActive(): void {
    if (this.ended) {
      throw new Error(
        `Cannot emit frame: stream is in terminal state (operation_id=${this.operationId})`,
      );
    }
  }

  /** Emit a start frame. */
  start(payload: unknown, metadata?: Record<string, unknown>): InferenceStreamFrame {
    this.ensureActive();
    return this.nextFrame("start", payload, metadata);
  }

  /** Emit a chunk frame. */
  chunk(payload: unknown, metadata?: Record<string, unknown>): InferenceStreamFrame {
    this.ensureActive();
    return this.nextFrame("chunk", payload, metadata);
  }

  /** Emit a progress frame. */
  progress(payload: unknown, metadata?: Record<string, unknown>): InferenceStreamFrame {
    this.ensureActive();
    return this.nextFrame("progress", payload, metadata);
  }

  /** Emit an end frame (terminal). */
  end(payload?: unknown, metadata?: Record<string, unknown>): InferenceStreamFrame {
    this.ensureActive();
    this.ended = true;
    return this.nextFrame("end", payload ?? null, metadata);
  }

  /** Emit an error frame (terminal). */
  error(error: InferenceError, metadata?: Record<string, unknown>): InferenceStreamFrame {
    this.ensureActive();
    this.ended = true;
    return this.nextFrame("error", { kind: error.kind, message: error.message }, metadata);
  }

  /** Emit a cancelled frame (terminal). */
  cancelled(reason?: string, metadata?: Record<string, unknown>): InferenceStreamFrame {
    this.ensureActive();
    this.ended = true;
    return this.nextFrame("cancelled", { reason: reason ?? "cancelled" }, metadata);
  }

  /** Emit a timeout frame (terminal). */
  timeout(metadata?: Record<string, unknown>): InferenceStreamFrame {
    this.ensureActive();
    this.ended = true;
    return this.nextFrame("timeout", { reason: "timeout" }, metadata);
  }

  /** Whether the stream is in a terminal state. */
  isEnded(): boolean {
    return this.ended;
  }

  /** Current frame count. */
  getFrameCount(): number {
    return this.frameCount;
  }
}

// ---------------------------------------------------------------------------
// createProviderCapabilityManifest
// ---------------------------------------------------------------------------

/** Options for creating a provider capability manifest. */
export interface CreateProviderManifestOptions {
  provider_id: string;
  label: string;
  description?: string;
  operation_kinds: InferenceOperationKind[];
  input_modalities?: ModalityKind[];
  output_modalities?: ModalityKind[];
  transport_kinds: TransportKind[];
  runtime_kind: RuntimeKind;
  streaming_supported?: boolean;
  secrets_required?: boolean;
  network_required?: boolean;
  resource_constraints?: Record<string, unknown>;
  supported_resource_hints?: string[];
  metadata?: Record<string, unknown>;
}

/**
 * Create a provider capability manifest.
 *
 * Validates and returns a ProviderCapabilityManifest.
 * Throws on raw secret detection in metadata.
 */
export function createProviderCapabilityManifest(
  opts: CreateProviderManifestOptions,
): ProviderCapabilityManifest {
  const diagnostics: Array<{ field: string; message: string }> = [];
  metadataContainsRawSecret(opts.metadata, "metadata", diagnostics);
  metadataContainsRawSecret(opts.resource_constraints, "resource_constraints", diagnostics);

  if (diagnostics.length > 0) {
    const messages = diagnostics.map((d) => `${d.field}: ${d.message}`);
    throw new Error(
      `Invalid provider capability manifest:\n${messages.join("\n")}`,
    );
  }

  return {
    version: 1,
    provider_id: opts.provider_id,
    label: opts.label,
    description: opts.description,
    operation_kinds: opts.operation_kinds,
    input_modalities: opts.input_modalities ?? ["text"],
    output_modalities: opts.output_modalities ?? ["text"],
    transport_kinds: opts.transport_kinds,
    runtime_kind: opts.runtime_kind,
    streaming_supported: opts.streaming_supported ?? false,
    secrets_required: opts.secrets_required ?? false,
    network_required: opts.network_required ?? false,
    resource_constraints: opts.resource_constraints,
    supported_resource_hints: opts.supported_resource_hints,
    metadata: opts.metadata,
  };
}

// ---------------------------------------------------------------------------
// validateProviderCapabilityManifest
// ---------------------------------------------------------------------------

/** A validation diagnostic for provider capability manifest. */
export interface ManifestValidationDiagnostic {
  severity: "error" | "warning" | "info";
  field: string;
  message: string;
}

/**
 * Validate a provider capability manifest without throwing.
 *
 * Returns diagnostics array — empty means valid.
 */
export function validateProviderCapabilityManifest(
  manifest: ProviderCapabilityManifest,
): ManifestValidationDiagnostic[] {
  const diagnostics: ManifestValidationDiagnostic[] = [];

  // Required fields
  if (!manifest.provider_id) {
    diagnostics.push({ severity: "error", field: "provider_id", message: "provider_id is required." });
  }
  if (!manifest.label) {
    diagnostics.push({ severity: "error", field: "label", message: "label is required." });
  }
  if (manifest.operation_kinds.length === 0) {
    diagnostics.push({ severity: "warning", field: "operation_kinds", message: "No operation kinds declared." });
  }
  if (manifest.transport_kinds.length === 0) {
    diagnostics.push({ severity: "warning", field: "transport_kinds", message: "No transport kinds declared." });
  }

  // Secrets required but no secret-accepting transport
  if (manifest.secrets_required && manifest.transport_kinds.length === 0) {
    diagnostics.push({ severity: "warning", field: "transport_kinds", message: "Secrets required but no transport kinds declared." });
  }

  // Network required but no network-capable transport
  const networkTransports: TransportKind[] = ["http", "websocket", "remote"];
  if (manifest.network_required && !manifest.transport_kinds.some((t) => networkTransports.includes(t))) {
    diagnostics.push({
      severity: "warning",
      field: "transport_kinds",
      message: "Network required but no network-capable transport kind declared (http, websocket, remote).",
    });
  }

  // Metadata raw secrets
  const rawDiagnostics: Array<{ field: string; message: string }> = [];
  metadataContainsRawSecret(manifest.metadata, "metadata", rawDiagnostics);
  for (const d of rawDiagnostics) {
    diagnostics.push({ severity: "error", ...d });
  }

  return diagnostics;
}

// ---------------------------------------------------------------------------
// Self-test
// ---------------------------------------------------------------------------

/**
 * Run pure-TS self-test for the inference capability SDK.
 *
 * Returns an object with pass/fail counts and any failure messages.
 */
export function runInferenceCapabilitySelfTest(): {
  passed: number;
  failed: number;
  failures: string[];
} {
  const failures: string[] = [];
  let passed = 0;

  function assert(condition: boolean, label: string): void {
    if (condition) {
      passed++;
    } else {
      failures.push(label);
    }
  }

  // ---- Test 1: Construct a non-HTTP inference request ----
  try {
    const req = createInferenceRequest({
      operation_id: "op_local_001",
      operation_kind: "generate",
      input_refs: [{ ref_id: "artifact:scene_state_v3", mime_hint: "application/json" }],
      input_payload: { kind: "json", shape: { type: "scene_state" } },
      streaming: true,
      cancellation: { deadline: "2026-12-31T23:59:59Z" },
      resource_hints: { max_output_units: 512, temperature: 0.7, latency_preference: "low" },
      secret_refs: ["secret_ref:env:LOCAL_MODEL_KEY"],
      transport_kind: "local_process",
      metadata: { source: "plurora-session-42" },
    });
    assert(req.operation_id === "op_local_001", "non-HTTP request: operation_id");
    assert(req.operation_kind === "generate", "non-HTTP request: operation_kind");
    assert(req.transport_kind === "local_process", "non-HTTP request: transport_kind");
    assert(req.streaming === true, "non-HTTP request: streaming");
    assert(req.secret_refs.length === 1, "non-HTTP request: secret_refs count");
    assert(req.input_refs.length === 1, "non-HTTP request: input_refs count");
    assert(req.cancellation?.deadline === "2026-12-31T23:59:59Z", "non-HTTP request: deadline");
    assert(req.resource_hints?.max_output_units === 512, "non-HTTP request: resource_hints");
  } catch (e) {
    failures.push(`non-HTTP request construction: ${String(e)}`);
  }

  // ---- Test 2: Reject raw secret-looking values in secret_refs ----
  try {
    createInferenceRequest({
      operation_id: "op_bad_secret",
      operation_kind: "generate",
      secret_refs: ["RawSecretExample1234567890abcdefABCDEF"],
      transport_kind: "http",
    });
    failures.push("raw secret in secret_refs: should have thrown");
  } catch (e) {
    assert(
      String(e).includes("raw secret") || String(e).includes("Raw secret"),
      "raw secret in secret_refs: error message mentions raw secret",
    );
  }

  // ---- Test 2b: Reject raw secret in metadata ----
  try {
    createInferenceRequest({
      operation_id: "op_bad_meta",
      operation_kind: "generate",
      transport_kind: "in_memory",
      metadata: { api_key: "RawSecretExample1234567890abcdefABCDEF" },
    });
    failures.push("raw secret in metadata: should have thrown");
  } catch (e) {
    assert(
      String(e).includes("secret") || String(e).includes("secret_ref"),
      "raw secret in metadata: error message mentions secret",
    );
  }

  // ---- Test 2c: Reject raw secret in input_refs metadata ----
  try {
    createInferenceRequest({
      operation_id: "op_bad_ref_meta",
      operation_kind: "generate",
      transport_kind: "local_process",
      input_refs: [{
        ref_id: "ref_1",
        metadata: { token: "RawSecretExample1234567890abcdefABCDEF" },
      }],
    });
    failures.push("raw secret in input_refs metadata: should have thrown");
  } catch (e) {
    assert(
      String(e).includes("secret"),
      "raw secret in input_refs metadata: error message mentions secret",
    );
  }

  // ---- Test 3: Error taxonomy covers cloud errors ----
  const cloudAuthError = classifyInferenceError({
    message: "Invalid API key",
    code: "invalid_api_key",
    transport_kind: "http",
  });
  assert(cloudAuthError.kind === "authentication", "cloud auth error: kind=authentication");
  assert(cloudAuthError.retryable === false, "cloud auth error: not retryable");

  const cloudRateLimitError = classifyInferenceError({
    message: "Rate limit exceeded",
    code: "rate_limit_error",
    transport_kind: "http",
  });
  assert(cloudRateLimitError.kind === "rate_limit", "cloud rate limit: kind=rate_limit");
  assert(cloudRateLimitError.retryable === true, "cloud rate limit: retryable");

  const cloudBillingError = classifyInferenceError({
    message: "Insufficient quota",
    code: "insufficient_quota",
    transport_kind: "http",
  });
  assert(cloudBillingError.kind === "billing", "cloud billing: kind=billing");

  // ---- Test 3b: Error taxonomy covers local/resource errors ----
  const localOOM = classifyInferenceError({
    message: "CUDA out of memory",
    code: "out_of_memory",
    transport_kind: "local_process",
  });
  assert(localOOM.kind === "local_resource_exhausted", "local OOM: kind=local_resource_exhausted");

  const localCrash = classifyInferenceError({
    message: "Process exited with code 137",
    code: "process_exit_sigkill",
    transport_kind: "local_process",
  });
  assert(localCrash.kind === "local_process_failed", "local crash: kind=local_process_failed");

  const localModelNotLoaded = classifyInferenceError({
    message: "Model not available",
    code: "model_not_loaded",
    transport_kind: "in_memory",
  });
  assert(localModelNotLoaded.kind === "local_model_not_loaded", "local model not loaded: kind=local_model_not_loaded");

  const localInferenceErr = classifyInferenceError({
    message: "Inference failed mid-compute",
    code: "inference_error",
    transport_kind: "local_process",
  });
  assert(localInferenceErr.kind === "local_inference_error", "local inference error: kind=local_inference_error");

  const localTimeout = classifyInferenceError({
    message: "Process deadline exceeded",
    code: "deadline_exceeded",
    transport_kind: "local_process",
  });
  assert(localTimeout.kind === "local_process_timeout", "local timeout: kind=local_process_timeout");

  // ---- Test 3c: Cross-cutting errors ----
  const timeoutErr = classifyInferenceError({
    message: "Deadline exceeded",
    timed_out: true,
  });
  assert(timeoutErr.kind === "timeout", "cross-cutting timeout: kind=timeout");

  const cancelledErr = classifyInferenceError({
    message: "Cancelled by caller",
    cancelled: true,
  });
  assert(cancelledErr.kind === "cancelled", "cross-cutting cancelled: kind=cancelled");

  const secretErr = classifyInferenceError({
    message: "Could not resolve secret_ref:env:MISSING_KEY",
    secret_unavailable: true,
  });
  assert(secretErr.kind === "secret_unavailable", "cross-cutting secret: kind=secret_unavailable");

  const networkDeniedErr = classifyInferenceError({
    message: "Network access denied",
    network_denied: true,
  });
  assert(networkDeniedErr.kind === "network_denied", "cross-cutting network denied: kind=network_denied");

  // ---- Test 4: Stream frame lifecycle ----
  const lifecycle = new InferenceStreamLifecycle("op_stream_001", "str_001");

  const startFrame = lifecycle.start({ capability_id: "inference/generate" });
  assert(startFrame.frame_kind === "start", "lifecycle: start frame_kind");
  assert(startFrame.sequence === 0, "lifecycle: start sequence=0");
  assert(startFrame.operation_id === "op_stream_001", "lifecycle: start operation_id");

  const chunk1 = lifecycle.chunk({ text_delta: "Once upon" });
  assert(chunk1.frame_kind === "chunk", "lifecycle: chunk frame_kind");
  assert(chunk1.sequence === 1, "lifecycle: chunk sequence=1");

  const chunk2 = lifecycle.chunk({ text_delta: " a time…" });
  assert(chunk2.sequence === 2, "lifecycle: chunk2 sequence=2");

  const progressFrame = lifecycle.progress({ percent: 50 });
  assert(progressFrame.frame_kind === "progress", "lifecycle: progress frame_kind");
  assert(progressFrame.sequence === 3, "lifecycle: progress sequence=3");

  const endFrame = lifecycle.end();
  assert(endFrame.frame_kind === "end", "lifecycle: end frame_kind");
  assert(endFrame.sequence === 4, "lifecycle: end sequence=4");
  assert(lifecycle.isEnded() === true, "lifecycle: isEnded after end");

  // Terminal state blocks further frames
  try {
    lifecycle.chunk({ text_delta: "should fail" });
    failures.push("lifecycle: chunk after end should throw");
  } catch {
    assert(true, "lifecycle: chunk after end throws correctly");
  }

  // ---- Test 4b: Lifecycle with error termination ----
  const errLifecycle = new InferenceStreamLifecycle("op_err_001", "str_err_001");
  errLifecycle.start({});
  const errFrame = errLifecycle.error({
    kind: "local_resource_exhausted",
    retryable: false,
    stage: "invocation",
    message: "GPU OOM",
  });
  assert(errFrame.frame_kind === "error", "error lifecycle: error frame_kind");
  assert(errLifecycle.isEnded() === true, "error lifecycle: isEnded after error");

  // ---- Test 4c: Lifecycle with cancel ----
  const cancelLifecycle = new InferenceStreamLifecycle("op_cancel_001", "str_cancel_001");
  cancelLifecycle.start({});
  cancelLifecycle.chunk({ text_delta: "partial" });
  const cancelFrame = cancelLifecycle.cancelled("user requested");
  assert(cancelFrame.frame_kind === "cancelled", "cancel lifecycle: cancelled frame_kind");
  assert(cancelLifecycle.isEnded() === true, "cancel lifecycle: isEnded after cancelled");

  // ---- Test 4d: Lifecycle with timeout ----
  const timeoutLifecycle = new InferenceStreamLifecycle("op_timeout_001", "str_timeout_001");
  timeoutLifecycle.start({});
  const timeoutFrame = timeoutLifecycle.timeout();
  assert(timeoutFrame.frame_kind === "timeout", "timeout lifecycle: timeout frame_kind");
  assert(timeoutLifecycle.isEnded() === true, "timeout lifecycle: isEnded after timeout");

  // ---- Test 5: Provider capability manifest ----

  // Cloud API manifest
  const cloudManifest = createProviderCapabilityManifest({
    provider_id: "plurora/model-provider-lab",
    label: "Cloud API Model Provider",
    description: "Cloud API adapter for model inference",
    operation_kinds: ["generate", "embed", "classify"],
    input_modalities: ["text", "image"],
    output_modalities: ["text", "structured"],
    transport_kinds: ["http"],
    runtime_kind: "cloud_api",
    streaming_supported: true,
    secrets_required: true,
    network_required: true,
    resource_constraints: { max_concurrent: 8 },
    supported_resource_hints: ["max_output_units", "temperature", "latency_preference"],
  });
  assert(cloudManifest.provider_id === "plurora/model-provider-lab", "cloud manifest: provider_id");
  assert(cloudManifest.runtime_kind === "cloud_api", "cloud manifest: runtime_kind=cloud_api");
  assert(cloudManifest.secrets_required === true, "cloud manifest: secrets_required");
  assert(cloudManifest.network_required === true, "cloud manifest: network_required");
  assert(cloudManifest.streaming_supported === true, "cloud manifest: streaming_supported");
  assert(cloudManifest.transport_kinds.includes("http"), "cloud manifest: http transport");
  assert(cloudManifest.input_modalities.includes("image"), "cloud manifest: image modality");

  // Local process manifest
  const localManifest = createProviderCapabilityManifest({
    provider_id: "plurora/inference-local-lab",
    label: "Local Process Inference Provider",
    operation_kinds: ["generate"],
    input_modalities: ["text"],
    output_modalities: ["text"],
    transport_kinds: ["local_process", "in_memory"],
    runtime_kind: "gpu_local",
    streaming_supported: true,
    secrets_required: false,
    network_required: false,
    supported_resource_hints: ["max_output_units", "temperature", "compute_budget"],
    resource_constraints: { min_vram_mb: 4096 },
  });
  assert(localManifest.runtime_kind === "gpu_local", "local manifest: runtime_kind=gpu_local");
  assert(localManifest.network_required === false, "local manifest: no network");
  assert(localManifest.transport_kinds.includes("local_process"), "local manifest: local_process transport");
  assert(localManifest.transport_kinds.includes("in_memory"), "local manifest: in_memory transport");
  assert(localManifest.secrets_required === false, "local manifest: no secrets required");

  // Validate manifest
  const cloudDiags = validateProviderCapabilityManifest(cloudManifest);
  assert(cloudDiags.length === 0, "cloud manifest: valid (no diagnostics)");

  const localDiags = validateProviderCapabilityManifest(localManifest);
  assert(localDiags.length === 0, "local manifest: valid (no diagnostics)");

  // ---- Test 5b: Manifest with raw secret in metadata throws ----
  try {
    createProviderCapabilityManifest({
      provider_id: "bad-provider",
      label: "Bad Provider",
      operation_kinds: ["generate"],
      transport_kinds: ["http"],
      runtime_kind: "cloud_api",
      metadata: { api_key: "RawSecretExample1234567890abcdefABCDEF" },
    });
    failures.push("manifest with raw secret: should have thrown");
  } catch (e) {
    assert(
      String(e).includes("secret"),
      "manifest raw secret: error message mentions secret",
    );
  }

  // ---- Test 5c: Manifest validation warns on empty operation_kinds ----
  const emptyManifest: ProviderCapabilityManifest = {
    version: 1,
    provider_id: "empty-provider",
    label: "Empty Provider",
    operation_kinds: [],
    input_modalities: ["text"],
    output_modalities: ["text"],
    transport_kinds: ["http"],
    runtime_kind: "cloud_api",
    streaming_supported: false,
    secrets_required: false,
    network_required: false,
  };
  const emptyDiags = validateProviderCapabilityManifest(emptyManifest);
  assert(
    emptyDiags.some((d) => d.field === "operation_kinds" && d.severity === "warning"),
    "empty manifest: warns on empty operation_kinds",
  );

  // ---- Test 5d: Manifest with network_required but no network transport warns ----
  const noNetworkTransportManifest: ProviderCapabilityManifest = {
    version: 1,
    provider_id: "mismatch-provider",
    label: "Mismatch Provider",
    operation_kinds: ["generate"],
    input_modalities: ["text"],
    output_modalities: ["text"],
    transport_kinds: ["local_process"],
    runtime_kind: "cpu_local",
    streaming_supported: false,
    secrets_required: false,
    network_required: true,
  };
  const mismatchDiags = validateProviderCapabilityManifest(noNetworkTransportManifest);
  assert(
    mismatchDiags.some((d) => d.field === "transport_kinds" && d.severity === "warning"),
    "mismatch manifest: warns on network_required but no network transport",
  );

  // ---- Test 6: IPC and websocket transport kinds are accepted ----
  const ipcReq = createInferenceRequest({
    operation_id: "op_ipc_001",
    operation_kind: "transform",
    transport_kind: "ipc",
  });
  assert(ipcReq.transport_kind === "ipc", "IPC request: transport_kind=ipc");

  const wsReq = createInferenceRequest({
    operation_id: "op_ws_001",
    operation_kind: "generate",
    transport_kind: "websocket",
    secret_refs: ["secret_ref:vault:ws_token"],
  });
  assert(wsReq.transport_kind === "websocket", "WebSocket request: transport_kind=websocket");

  // ---- Test 7: validateInferenceRequest returns diagnostics without throwing ----
  const badDiags = validateInferenceRequest({
    operation_id: "",
    operation_kind: "generate",
    secret_refs: ["not_a_ref"],
    transport_kind: "http",
  });
  assert(badDiags.length > 0, "validate: returns diagnostics for invalid request");
  assert(
    badDiags.some((d) => d.field === "operation_id"),
    "validate: flags empty operation_id",
  );
  assert(
    badDiags.some((d) => d.field === "secret_refs"),
    "validate: flags invalid secret_ref",
  );

  // ---- Test 8: HTTP status hint in error classification ----
  const http401Error = classifyInferenceError({
    message: "Unauthorized",
    http_status_hint: 401,
  });
  assert(http401Error.kind === "authentication", "http 401 hint: kind=authentication");

  const http429Error = classifyInferenceError({
    message: "Too many requests",
    http_status_hint: 429,
  });
  assert(http429Error.kind === "rate_limit", "http 429 hint: kind=rate_limit");

  // ---- Test 9: InMemory request with no secrets ----
  const inMemReq = createInferenceRequest({
    operation_id: "op_inmem_001",
    operation_kind: "embed",
    input_refs: [{ ref_id: "artifact:text_chunk_42" }],
    transport_kind: "in_memory",
  });
  assert(inMemReq.secret_refs.length === 0, "in_memory request: no secrets needed");
  assert(inMemReq.transport_kind === "in_memory", "in_memory request: transport_kind");

  return { passed, failed: failures.length, failures };
}
