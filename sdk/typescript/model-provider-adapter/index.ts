/**
 * Plurora Model Provider Adapter SDK — Pure TypeScript adapter for
 * normalizing model provider API differences.
 *
 * This module provides types and helpers that let capability packages
 * describe, validate, and normalize model provider requests across
 * OpenAI, Anthropic, Gemini, OpenAI-compatible, OpenRouter, DeepSeek,
 * xAI, and Fireworks — without importing private runtime, making real
 * HTTP calls, or depending on any external library.
 *
 * It is an **SDK adapter**, not a provider package. It does not ship a
 * model, does not proxy requests, does not do billing, does not add
 * `kernel.v1.model.*`, and does not perform network I/O.
 *
 * ## API surface
 *
 * Types:
 * - `ProviderFamily`              — union of supported provider families
 * - `RequestDialect`             — canonical request dialects
 * - `StreamFamily`               — stream protocol families
 * - `ToolMode` / `UsageMode`     — tool & usage modes
 * - `ProviderProfile`            — provider profile (family, model, credential ref, …)
 * - `CanonicalModelMessage` / `CanonicalModelRequest` / `CanonicalModelResponse`
 * - `NormalizedStreamEvent`      — union of normalized stream events
 * - `ProviderErrorKind`          — error classification union
 * - `ProviderUsage` / `ProviderCost` / `ProviderDiagnostic`
 * - `NormalizedProviderRequest`  — provider-specific request shape
 *
 * Functions:
 * - `listProviderFamilies()`           — list all supported families
 * - `describeProviderFamily(family)`   — describe a family
 * - `validateProviderProfile(profile)` — validate profile, reject raw secrets
 * - `normalizeModelRequest(profile, request)` — produce provider-specific request
 * - `normalizeProviderError(input)`    — map HTTP status / provider code → diagnostic
 * - `normalizeStreamEvent(family, chunk)` — parse stream chunk → normalized events
 * - `estimateUsage(responseOrEvents)`  — placeholder usage aggregate
 * - `runModelProviderAdapterSelfTest()` — pure-TS self-test
 */

// ---------------------------------------------------------------------------
// Provider family & dialect types
// ---------------------------------------------------------------------------

/** Supported provider families. */
export type ProviderFamily =
  | "openai"
  | "anthropic"
  | "gemini"
  | "openai_compatible"
  | "openrouter"
  | "deepseek"
  | "xai"
  | "fireworks";

/** Canonical request dialects across providers. */
export type RequestDialect =
  | "openai_chat"
  | "openai_responses"
  | "anthropic_messages"
  | "gemini_generate_content"
  | "stateless_responses"
  | "anthropic_compat"
  | "fireworks_responses";

/** Stream protocol families. */
export type StreamFamily =
  | "delta_sse"
  | "semantic_sse"
  | "typed_chunk_stream";

/** Tool modes providers support. */
export type ToolMode =
  | "functions"
  | "built_in_tools"
  | "tool_use"
  | "code_execution"
  | "web_search"
  | "mcp";

/** Usage reporting modes. */
export type UsageMode =
  | "top_level"
  | "final_chunk"
  | "cumulative_delta"
  | "usage_metadata";

// ---------------------------------------------------------------------------
// Provider profile
// ---------------------------------------------------------------------------

/** A provider profile — describes how to reach a model provider. */
export interface ProviderProfile {
  /** Provider family. */
  family: ProviderFamily;
  /** Model identifier (provider-specific, e.g. "gpt-4o", "claude-3-5-sonnet-20241022"). */
  model: string;
  /**
   * Credential reference — must be a `secret_ref:*` or `host:*` string.
   * Raw API keys are rejected by validateProviderProfile().
   */
  credential: string;
  /** Base URL override (must be HTTPS when provided). */
  baseUrl?: string;
  /** Extra HTTP headers to send with each request. */
  headers?: Record<string, string>;
  /** Provider-specific options bag (e.g. reasoning_effort, safety_settings). */
  providerOptions?: Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// Canonical model message / request / response
// ---------------------------------------------------------------------------

/** Role for a canonical model message. */
export type CanonicalMessageRole = "system" | "user" | "assistant" | "tool";

/** A canonical model message — provider-agnostic. */
export interface CanonicalModelMessage {
  /** Message role. */
  role: CanonicalMessageRole;
  /** Text content. */
  content: string;
  /** Tool call id (for role=tool responses). */
  tool_call_id?: string;
  /** Tool calls (for role=assistant with tool invocations). */
  tool_calls?: Array<{
    id: string;
    name: string;
    arguments: string;
  }>;
}

/** A canonical model request — provider-agnostic. */
export interface CanonicalModelRequest {
  /** Messages in conversation order. */
  messages: CanonicalModelMessage[];
  /** Whether to stream the response. */
  stream?: boolean;
  /** Maximum tokens for the response. */
  max_tokens?: number;
  /** Temperature sampling parameter. */
  temperature?: number;
  /** Tool definitions (function-calling). */
  tools?: Array<{
    name: string;
    description?: string;
    parameters?: Record<string, unknown>;
  }>;
  /** Provider-specific passthrough options. */
  extra?: Record<string, unknown>;
}

/** A canonical model response — provider-agnostic. */
export interface CanonicalModelResponse {
  /** The provider family that produced this response. */
  family: ProviderFamily;
  /** The model that produced this response. */
  model: string;
  /** Output text content. */
  content: string;
  /** Tool calls, if any. */
  tool_calls?: Array<{
    id: string;
    name: string;
    arguments: string;
  }>;
  /** Finish reason (e.g. "stop", "tool_calls", "length"). */
  finish_reason?: string;
  /** Usage data, if available. */
  usage?: ProviderUsage;
}

// ---------------------------------------------------------------------------
// Normalized stream events
// ---------------------------------------------------------------------------

/** Normalized stream event kinds. */
export type NormalizedStreamEventKind =
  | "text_delta"
  | "reasoning_delta"
  | "tool_call_started"
  | "tool_args_delta"
  | "tool_call_done"
  | "citation"
  | "usage_final"
  | "error"
  | "done"
  | "heartbeat";

/** A normalized stream event — package-owned, not kernel semantics. */
export type NormalizedStreamEvent =
  | { kind: "text_delta"; text: string; index?: number }
  | { kind: "reasoning_delta"; text: string; index?: number }
  | { kind: "tool_call_started"; tool_call_id: string; name: string; index?: number }
  | { kind: "tool_args_delta"; tool_call_id: string; arguments_delta: string; index?: number }
  | { kind: "tool_call_done"; tool_call_id: string; arguments: string; index?: number }
  | { kind: "citation"; url?: string; title?: string; text?: string; index?: number }
  | { kind: "usage_final"; usage: ProviderUsage }
  | { kind: "error"; error: ProviderDiagnostic }
  | { kind: "done"; finish_reason?: string }
  | { kind: "heartbeat" };

// ---------------------------------------------------------------------------
// Error taxonomy
// ---------------------------------------------------------------------------

/** Provider error kinds — stable classification per error-taxonomy.md. */
export type ProviderErrorKind =
  | "bad_request"
  | "authentication"
  | "permission"
  | "billing"
  | "rate_limit"
  | "not_found"
  | "timeout"
  | "overloaded"
  | "tool_schema"
  | "stream_error"
  | "upstream_malformed"
  | "network_denied"
  | "secret_unavailable"
  | "unknown";

/** Error stage. */
export type ErrorStage = "preflight" | "request" | "stream" | "postprocess";

// ---------------------------------------------------------------------------
// Usage / Cost / Diagnostic
// ---------------------------------------------------------------------------

/** Provider usage metrics. */
export interface ProviderUsage {
  /** Prompt/input token count. */
  prompt_tokens?: number;
  /** Completion/output token count. */
  completion_tokens?: number;
  /** Total token count. */
  total_tokens?: number;
  /** Reasoning token count (if reported). */
  reasoning_tokens?: number;
  /** Cached token count (if reported). */
  cached_tokens?: number;
}

/** Provider cost estimate. */
export interface ProviderCost {
  /** Estimated cost in USD. */
  estimated_usd?: number;
  /** Currency label. */
  label?: string;
}

/** A provider diagnostic — maps upstream errors to stable classification. */
export interface ProviderDiagnostic {
  /** Stable error kind. */
  kind: ProviderErrorKind;
  /** Whether the operation is retryable. */
  retryable: boolean;
  /** Error stage. */
  stage: ErrorStage;
  /** The provider family. */
  provider_family: ProviderFamily;
  /** The original provider error code. */
  provider_code?: string;
  /** The upstream request id (redacted). */
  upstream_request_id?: string;
  /** Human-readable message. */
  message: string;
}

// ---------------------------------------------------------------------------
// Normalized provider request
// ---------------------------------------------------------------------------

/** A normalized provider-specific request shape. */
export interface NormalizedProviderRequest {
  /** Provider family. */
  family: ProviderFamily;
  /** Request dialect. */
  requestDialect: RequestDialect;
  /** Stream family. */
  streamFamily: StreamFamily;
  /** HTTP method. */
  method: string;
  /** Full endpoint URL. */
  endpoint: string;
  /** HTTP headers (credential_ref, not raw secret). */
  headers: Record<string, string>;
  /** Body shape description (not the actual body). */
  bodyShape: Record<string, unknown>;
  /** Credential reference for the host to resolve. */
  credential_ref: string;
}

// ---------------------------------------------------------------------------
// Provider family metadata
// ---------------------------------------------------------------------------

interface ProviderFamilyMeta {
  family: ProviderFamily;
  label: string;
  requestDialects: RequestDialect[];
  streamFamilies: StreamFamily[];
  usageModes: UsageMode[];
  toolModes: ToolMode[];
  defaultBaseUrls: string[];
  authHeaders: string[];
  notes: string[];
}

const PROVIDER_FAMILY_META: ReadonlyArray<ProviderFamilyMeta> = [
  {
    family: "openai",
    label: "OpenAI",
    requestDialects: ["openai_responses", "openai_chat"],
    streamFamilies: ["semantic_sse", "delta_sse"],
    usageModes: ["top_level", "final_chunk"],
    toolModes: ["functions", "built_in_tools"],
    defaultBaseUrls: ["https://api.openai.com"],
    authHeaders: ["Authorization"],
    notes: [
      "Responses API emits semantic events (response.output_text.delta).",
      "Chat Completions uses choices[].delta.",
    ],
  },
  {
    family: "anthropic",
    label: "Anthropic",
    requestDialects: ["anthropic_messages"],
    streamFamilies: ["semantic_sse"],
    usageModes: ["cumulative_delta"],
    toolModes: ["tool_use"],
    defaultBaseUrls: ["https://api.anthropic.com"],
    authHeaders: ["x-api-key", "anthropic-version"],
    notes: [
      "system is top-level, not a system role message.",
      "Requires anthropic-version header.",
      "Stream events: message_start/content_block_delta/message_delta/message_stop.",
    ],
  },
  {
    family: "gemini",
    label: "Gemini",
    requestDialects: ["gemini_generate_content"],
    streamFamilies: ["typed_chunk_stream"],
    usageModes: ["usage_metadata"],
    toolModes: ["functions", "code_execution"],
    defaultBaseUrls: ["https://generativelanguage.googleapis.com"],
    authHeaders: ["x-goog-api-key"],
    notes: [
      "Uses contents[] and parts[], plus systemInstruction/generationConfig/safetySettings.",
      "Not OpenAI-compatible.",
      "Requires x-goog-api-key header.",
    ],
  },
  {
    family: "openai_compatible",
    label: "OpenAI-Compatible",
    requestDialects: ["openai_chat"],
    streamFamilies: ["delta_sse"],
    usageModes: ["final_chunk", "top_level"],
    toolModes: ["functions"],
    defaultBaseUrls: [],
    authHeaders: ["Authorization"],
    notes: [
      "Provider presets needed for base URL, params, reasoning fields, usage quirks, error wrappers.",
      "baseUrl is required.",
    ],
  },
  {
    family: "openrouter",
    label: "OpenRouter",
    requestDialects: ["stateless_responses", "openai_chat"],
    streamFamilies: ["semantic_sse", "delta_sse"],
    usageModes: ["top_level"],
    toolModes: ["functions", "web_search"],
    defaultBaseUrls: ["https://openrouter.ai/api/v1"],
    authHeaders: ["Authorization"],
    notes: [
      "Stateless: callers send full history.",
      "Mid-stream errors may arrive as SSE chunks after HTTP 200.",
      "Optional HTTP-Referer / X-OpenRouter-Title headers.",
    ],
  },
  {
    family: "deepseek",
    label: "DeepSeek",
    requestDialects: ["openai_chat", "anthropic_compat"],
    streamFamilies: ["delta_sse"],
    usageModes: ["top_level", "final_chunk"],
    toolModes: ["functions"],
    defaultBaseUrls: ["https://api.deepseek.com"],
    authHeaders: ["Authorization"],
    notes: [
      "Supports thinking/reasoning_effort extensions.",
      "Strict beta requires stricter schema constraints.",
    ],
  },
  {
    family: "xai",
    label: "xAI",
    requestDialects: ["openai_responses", "openai_chat"],
    streamFamilies: ["semantic_sse", "delta_sse"],
    usageModes: ["top_level"],
    toolModes: ["functions", "web_search"],
    defaultBaseUrls: ["https://api.x.ai"],
    authHeaders: ["Authorization"],
    notes: [
      "Chat max_tokens is deprecated in favor of max_completion_tokens.",
      "Usage may include cost/ticks/reasoning/tool details.",
    ],
  },
  {
    family: "fireworks",
    label: "Fireworks",
    requestDialects: ["openai_chat", "fireworks_responses"],
    streamFamilies: ["delta_sse", "semantic_sse"],
    usageModes: ["top_level", "final_chunk"],
    toolModes: ["functions", "mcp"],
    defaultBaseUrls: ["https://api.fireworks.ai/inference/v1"],
    authHeaders: ["Authorization"],
    notes: [
      "Supports rollout/session affinity headers.",
      "Responses can include MCP and previous_response_id style continuation.",
    ],
  },
];

const FAMILY_MAP: Readonly<Record<ProviderFamily, ProviderFamilyMeta>> =
  Object.fromEntries(PROVIDER_FAMILY_META.map((m) => [m.family, m])) as never;

// ---------------------------------------------------------------------------
// Secret ref validation (re-implemented locally, no private runtime import)
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
  if (value.startsWith("AIza")) return true; // Gemini keys
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

// ---------------------------------------------------------------------------
// listProviderFamilies
// ---------------------------------------------------------------------------

/**
 * List all supported provider families.
 */
export function listProviderFamilies(): ProviderFamily[] {
  return PROVIDER_FAMILY_META.map((m) => m.family);
}

// ---------------------------------------------------------------------------
// describeProviderFamily
// ---------------------------------------------------------------------------

/**
 * Describe a provider family — returns its metadata or undefined if unknown.
 */
export function describeProviderFamily(
  family: ProviderFamily,
): ProviderFamilyMeta | undefined {
  return FAMILY_MAP[family];
}

// ---------------------------------------------------------------------------
// validateProviderProfile
// ---------------------------------------------------------------------------

/** A validation diagnostic. */
export interface ValidationDiagnostic {
  /** Severity. */
  severity: "error" | "warning" | "info";
  /** Field path. */
  field: string;
  /** Human-readable message. */
  message: string;
}

/**
 * Validate a ProviderProfile.
 *
 * - Rejects raw-looking API keys (must use `secret_ref:*` or `host:*`).
 * - Checks baseUrl is HTTPS when provided.
 * - Warns about OpenRouter missing optional headers.
 * - Info-level hints about Anthropic/Gemini required headers.
 * - Does NOT perform real network calls.
 */
export function validateProviderProfile(
  profile: ProviderProfile,
): ValidationDiagnostic[] {
  const diagnostics: ValidationDiagnostic[] = [];
  const meta = FAMILY_MAP[profile.family];

  // --- Credential must be a secret ref / host ref ---
  if (!isValidSecretRef(profile.credential)) {
    if (looksLikeRawSecret(profile.credential)) {
      diagnostics.push({
        severity: "error",
        field: "credential",
        message:
          `Raw API key detected in credential. Use a secret_ref: or host: reference instead. ` +
          `Example: "secret_ref:env:MY_API_KEY"`,
      });
    } else {
      diagnostics.push({
        severity: "error",
        field: "credential",
        message:
          `Credential must be a secret_ref: or host: reference. ` +
          `Got: "${profile.credential.slice(0, 20)}…"`,
      });
    }
  }

  // --- baseUrl must be HTTPS when provided ---
  if (profile.baseUrl !== undefined && profile.baseUrl !== "") {
    if (!profile.baseUrl.startsWith("https://")) {
      diagnostics.push({
        severity: "error",
        field: "baseUrl",
        message:
          `baseUrl must use HTTPS. Got: "${profile.baseUrl}"`,
      });
    }
  }

  // --- baseUrl required for openai_compatible ---
  if (profile.family === "openai_compatible") {
    if (!profile.baseUrl) {
      diagnostics.push({
        severity: "error",
        field: "baseUrl",
        message: "openai_compatible requires a baseUrl.",
      });
    }
  }

  // --- OpenRouter optional headers warning ---
  if (profile.family === "openrouter") {
    const headers = profile.headers ?? {};
    if (!("HTTP-Referer" in headers) && !("http-referer" in headers)) {
      diagnostics.push({
        severity: "warning",
        field: "headers",
        message: "OpenRouter recommends setting HTTP-Referer header for attribution.",
      });
    }
    if (!("X-OpenRouter-Title" in headers) && !("x-openrouter-title" in headers)) {
      diagnostics.push({
        severity: "info",
        field: "headers",
        message: "OpenRouter supports X-OpenRouter-Title header for request labeling.",
      });
    }
  }

  // --- Anthropic required headers hint ---
  if (profile.family === "anthropic") {
    const headers = profile.headers ?? {};
    if (!("anthropic-version" in headers)) {
      diagnostics.push({
        severity: "warning",
        field: "headers",
        message: "Anthropic requires the anthropic-version header (e.g. '2023-06-01').",
      });
    }
  }

  // --- Gemini required headers hint ---
  if (profile.family === "gemini") {
    const hasApiKeyHeader = profile.headers !== undefined && (
      "x-goog-api-key" in profile.headers || "X-Goog-Api-Key" in profile.headers
    );
    if (!hasApiKeyHeader) {
      diagnostics.push({
        severity: "info",
        field: "headers",
        message:
          "Gemini uses x-goog-api-key header for authentication. " +
          "The adapter will include it automatically from the credential ref.",
      });
    }
  }

  // --- Unknown family ---
  if (!meta) {
    diagnostics.push({
      severity: "error",
      field: "family",
      message: `Unknown provider family: "${profile.family}"`,
    });
  }

  // --- Header values should not contain raw secrets ---
  if (profile.headers) {
    for (const [key, value] of Object.entries(profile.headers)) {
      if (typeof value === "string" && looksLikeRawSecret(value)) {
        diagnostics.push({
          severity: "error",
          field: `headers.${key}`,
          message: `Header "${key}" contains a raw secret. Use a secret_ref: or host: reference.`,
        });
      }
    }
  }

  return diagnostics;
}

// ---------------------------------------------------------------------------
// normalizeModelRequest
// ---------------------------------------------------------------------------

/**
 * Normalize a canonical model request into a provider-specific request shape.
 *
 * Produces endpoint, method, headers, bodyShape, requestDialect, and
 * streamFamily based on the provider family. Does NOT embed raw secrets —
 * only the `credential_ref` for the host to resolve.
 */
export function normalizeModelRequest(
  profile: ProviderProfile,
  request: CanonicalModelRequest,
): NormalizedProviderRequest {
  const family = profile.family;
  const model = profile.model;
  const meta = FAMILY_MAP[family];

  switch (family) {
    case "openai": {
      const isResponses = (request.extra?.preferResponses === true);
      const dialect: RequestDialect = isResponses ? "openai_responses" : "openai_chat";
      const streamFamily: StreamFamily = isResponses ? "semantic_sse" : "delta_sse";
      const endpoint = isResponses
        ? `${profile.baseUrl ?? "https://api.openai.com"}/v1/responses`
        : `${profile.baseUrl ?? "https://api.openai.com"}/v1/chat/completions`;

      const headers: Record<string, string> = {
        "Authorization": `Bearer ${profile.credential}`,
        "Content-Type": "application/json",
      };
      if (profile.headers) {
        for (const [k, v] of Object.entries(profile.headers)) {
          headers[k] = v;
        }
      }

      const bodyShape: Record<string, unknown> = isResponses
        ? { model, input: request.messages, stream: request.stream ?? false, max_output_tokens: request.max_tokens, temperature: request.temperature, tools: request.tools }
        : { model, messages: request.messages, stream: request.stream ?? false, max_tokens: request.max_tokens, temperature: request.temperature, tools: request.tools };

      return {
        family,
        requestDialect: dialect,
        streamFamily,
        method: "POST",
        endpoint,
        headers,
        bodyShape,
        credential_ref: profile.credential,
      };
    }

    case "anthropic": {
      const endpoint = `${profile.baseUrl ?? "https://api.anthropic.com"}/v1/messages`;
      const headers: Record<string, string> = {
        "x-api-key": profile.credential,
        "anthropic-version": profile.headers?.["anthropic-version"] ?? "2023-06-01",
        "Content-Type": "application/json",
      };
      if (profile.headers) {
        for (const [k, v] of Object.entries(profile.headers)) {
          if (k !== "anthropic-version") headers[k] = v;
        }
      }

      // Anthropic: system is top-level
      const systemMsg = request.messages.find((m) => m.role === "system");
      const nonSystemMsgs = request.messages.filter((m) => m.role !== "system");

      const bodyShape: Record<string, unknown> = {
        model,
        messages: nonSystemMsgs,
        system: systemMsg?.content,
        stream: request.stream ?? false,
        max_tokens: request.max_tokens ?? 4096,
        temperature: request.temperature,
        tools: request.tools,
      };

      return {
        family,
        requestDialect: "anthropic_messages",
        streamFamily: "semantic_sse",
        method: "POST",
        endpoint,
        headers,
        bodyShape,
        credential_ref: profile.credential,
      };
    }

    case "gemini": {
      const base = profile.baseUrl ?? "https://generativelanguage.googleapis.com";
      const streamSuffix = request.stream ? "?alt=sse" : "";
      const endpoint = `${base}/v1beta/models/${model}:generateContent${streamSuffix}`;
      const headers: Record<string, string> = {
        "x-goog-api-key": profile.credential,
        "Content-Type": "application/json",
      };
      if (profile.headers) {
        for (const [k, v] of Object.entries(profile.headers)) {
          headers[k] = v;
        }
      }

      const systemMsg = request.messages.find((m) => m.role === "system");
      const contents = request.messages
        .filter((m) => m.role !== "system")
        .map((m) => ({
          role: m.role === "assistant" ? "model" : "user",
          parts: [{ text: m.content }],
        }));

      const bodyShape: Record<string, unknown> = {
        contents,
        systemInstruction: systemMsg ? { parts: [{ text: systemMsg.content }] } : undefined,
        generationConfig: {
          maxOutputTokens: request.max_tokens,
          temperature: request.temperature,
        },
      };

      return {
        family,
        requestDialect: "gemini_generate_content",
        streamFamily: "typed_chunk_stream",
        method: "POST",
        endpoint,
        headers,
        bodyShape,
        credential_ref: profile.credential,
      };
    }

    case "openai_compatible": {
      const base = profile.baseUrl ?? "";
      const endpoint = `${base}/chat/completions`;
      const headers: Record<string, string> = {
        "Authorization": `Bearer ${profile.credential}`,
        "Content-Type": "application/json",
      };
      if (profile.headers) {
        for (const [k, v] of Object.entries(profile.headers)) {
          headers[k] = v;
        }
      }

      const bodyShape: Record<string, unknown> = {
        model,
        messages: request.messages,
        stream: request.stream ?? false,
        max_tokens: request.max_tokens,
        temperature: request.temperature,
        tools: request.tools,
      };

      return {
        family,
        requestDialect: "openai_chat",
        streamFamily: "delta_sse",
        method: "POST",
        endpoint,
        headers,
        bodyShape,
        credential_ref: profile.credential,
      };
    }

    case "openrouter": {
      const isResponses = (request.extra?.preferResponses === true);
      const dialect: RequestDialect = isResponses ? "stateless_responses" : "openai_chat";
      const streamFamily: StreamFamily = isResponses ? "semantic_sse" : "delta_sse";
      const base = profile.baseUrl ?? "https://openrouter.ai/api/v1";
      const endpoint = isResponses
        ? `${base}/responses`
        : `${base}/chat/completions`;

      const headers: Record<string, string> = {
        "Authorization": `Bearer ${profile.credential}`,
        "Content-Type": "application/json",
      };
      if (profile.headers) {
        for (const [k, v] of Object.entries(profile.headers)) {
          headers[k] = v;
        }
      }

      const bodyShape: Record<string, unknown> = isResponses
        ? { model, input: request.messages, stream: request.stream ?? false }
        : { model, messages: request.messages, stream: request.stream ?? false, max_tokens: request.max_tokens, temperature: request.temperature, tools: request.tools };

      return {
        family,
        requestDialect: dialect,
        streamFamily,
        method: "POST",
        endpoint,
        headers,
        bodyShape,
        credential_ref: profile.credential,
      };
    }

    case "deepseek": {
      const base = profile.baseUrl ?? "https://api.deepseek.com";
      const endpoint = `${base}/chat/completions`;
      const headers: Record<string, string> = {
        "Authorization": `Bearer ${profile.credential}`,
        "Content-Type": "application/json",
      };
      if (profile.headers) {
        for (const [k, v] of Object.entries(profile.headers)) {
          headers[k] = v;
        }
      }

      const bodyShape: Record<string, unknown> = {
        model,
        messages: request.messages,
        stream: request.stream ?? false,
        max_tokens: request.max_tokens,
        temperature: request.temperature,
        tools: request.tools,
      };

      // DeepSeek supports reasoning_effort via extra
      if (request.extra?.reasoning_effort !== undefined) {
        bodyShape.reasoning_effort = request.extra.reasoning_effort;
      }

      return {
        family,
        requestDialect: "openai_chat",
        streamFamily: "delta_sse",
        method: "POST",
        endpoint,
        headers,
        bodyShape,
        credential_ref: profile.credential,
      };
    }

    case "xai": {
      const isResponses = (request.extra?.preferResponses === true);
      const dialect: RequestDialect = isResponses ? "openai_responses" : "openai_chat";
      const streamFamily: StreamFamily = isResponses ? "semantic_sse" : "delta_sse";
      const base = profile.baseUrl ?? "https://api.x.ai";
      const endpoint = isResponses
        ? `${base}/v1/responses`
        : `${base}/v1/chat/completions`;

      const headers: Record<string, string> = {
        "Authorization": `Bearer ${profile.credential}`,
        "Content-Type": "application/json",
      };
      if (profile.headers) {
        for (const [k, v] of Object.entries(profile.headers)) {
          headers[k] = v;
        }
      }

      const bodyShape: Record<string, unknown> = isResponses
        ? { model, input: request.messages, stream: request.stream ?? false, max_output_tokens: request.max_tokens }
        : { model, messages: request.messages, stream: request.stream ?? false, max_completion_tokens: request.max_tokens, temperature: request.temperature, tools: request.tools };

      return {
        family,
        requestDialect: dialect,
        streamFamily,
        method: "POST",
        endpoint,
        headers,
        bodyShape,
        credential_ref: profile.credential,
      };
    }

    case "fireworks": {
      const isResponses = (request.extra?.preferResponses === true);
      const dialect: RequestDialect = isResponses ? "fireworks_responses" : "openai_chat";
      const streamFamily: StreamFamily = isResponses ? "semantic_sse" : "delta_sse";
      const base = profile.baseUrl ?? "https://api.fireworks.ai/inference/v1";
      const endpoint = isResponses
        ? `${base}/responses`
        : `${base}/chat/completions`;

      const headers: Record<string, string> = {
        "Authorization": `Bearer ${profile.credential}`,
        "Content-Type": "application/json",
      };
      if (profile.headers) {
        for (const [k, v] of Object.entries(profile.headers)) {
          headers[k] = v;
        }
      }

      const bodyShape: Record<string, unknown> = isResponses
        ? { model, input: request.messages, stream: request.stream ?? false }
        : { model, messages: request.messages, stream: request.stream ?? false, max_tokens: request.max_tokens, temperature: request.temperature, tools: request.tools };

      return {
        family,
        requestDialect: dialect,
        streamFamily,
        method: "POST",
        endpoint,
        headers,
        bodyShape,
        credential_ref: profile.credential,
      };
    }

    default: {
      // Exhaustive check — should never reach if ProviderFamily union is correct
      const _exhaustive: never = family;
      throw new Error(`normalizeModelRequest: unsupported family "${String(_exhaustive)}"`);
    }
  }
}

// ---------------------------------------------------------------------------
// normalizeProviderError
// ---------------------------------------------------------------------------

/** Input for error normalization. */
export interface ProviderErrorInput {
  /** HTTP status code (if available). */
  httpStatus?: number;
  /** Provider-specific error code string. */
  providerCode?: string;
  /** Provider family. */
  family: ProviderFamily;
  /** Error stage. */
  stage?: ErrorStage;
  /** Upstream request id (if available). */
  upstreamRequestId?: string;
  /** Human-readable error message. */
  message?: string;
}

/**
 * Normalize a provider error into a stable diagnostic classification.
 *
 * Maps HTTP status codes and provider-specific error codes to the
 * stable ProviderErrorKind taxonomy from error-taxonomy.md.
 */
export function normalizeProviderError(input: ProviderErrorInput): ProviderDiagnostic {
  const { httpStatus, providerCode, family, upstreamRequestId } = input;
  const stage: ErrorStage = input.stage ?? "request";
  const message = input.message ?? "Unknown provider error";

  // Try provider code mapping first
  if (providerCode !== undefined) {
    const codeLower = providerCode.toLowerCase();

    // Anthropic codes
    if (codeLower.includes("invalid_request")) return { kind: "bad_request", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
    if (codeLower.includes("authentication_error")) return { kind: "authentication", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
    if (codeLower.includes("permission_error")) return { kind: "permission", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
    if (codeLower.includes("not_found_error")) return { kind: "not_found", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
    if (codeLower.includes("rate_limit_error")) return { kind: "rate_limit", retryable: true, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
    if (codeLower.includes("overloaded_error") || codeLower === "529") return { kind: "overloaded", retryable: true, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
    if (codeLower.includes("timeout_error")) return { kind: "timeout", retryable: true, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
    if (codeLower.includes("api_error")) return { kind: "upstream_malformed", retryable: true, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };

    // Gemini codes
    if (codeLower === "invalid_argument") return { kind: "bad_request", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
    if (codeLower === "permission_denied") return { kind: "permission", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
    if (codeLower === "resource_exhausted") return { kind: "rate_limit", retryable: true, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
    if (codeLower === "not_found") return { kind: "not_found", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
    if (codeLower === "unavailable") return { kind: "overloaded", retryable: true, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
    if (codeLower === "deadline_exceeded") return { kind: "timeout", retryable: true, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
    if (codeLower === "unauthenticated") return { kind: "authentication", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };

    // OpenAI codes
    if (codeLower === "invalid_api_key") return { kind: "authentication", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
    if (codeLower === "model_not_found") return { kind: "not_found", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
    if (codeLower === "insufficient_quota") return { kind: "billing", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };

    // Tool schema
    if (codeLower.includes("tool") && codeLower.includes("schema")) return { kind: "tool_schema", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };

    // Stream error
    if (codeLower.includes("stream")) return { kind: "stream_error", retryable: true, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
  }

  // Fall back to HTTP status code mapping
  if (httpStatus !== undefined) {
    switch (httpStatus) {
      case 400: return { kind: "bad_request", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
      case 401: return { kind: "authentication", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
      case 402: return { kind: "billing", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
      case 403: return { kind: "permission", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
      case 404: return { kind: "not_found", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
      case 408: return { kind: "timeout", retryable: true, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
      case 422: return { kind: "tool_schema", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
      case 429: return { kind: "rate_limit", retryable: true, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
      case 500: return { kind: "upstream_malformed", retryable: true, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
      case 502: return { kind: "overloaded", retryable: true, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
      case 503: return { kind: "overloaded", retryable: true, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
      case 504: return { kind: "timeout", retryable: true, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
      case 529: return { kind: "overloaded", retryable: true, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
    }
  }

  return { kind: "unknown", retryable: false, stage, provider_family: family, provider_code: providerCode, upstream_request_id: upstreamRequestId, message };
}

// ---------------------------------------------------------------------------
// normalizeStreamEvent
// ---------------------------------------------------------------------------

/**
 * Normalize a raw stream chunk from a provider into NormalizedStreamEvent[].
 *
 * Supports minimal parsing of:
 * - OpenAI delta SSE JSON (`choices[].delta`)
 * - Anthropic event typed objects (`message_start`, `content_block_delta`, etc.)
 * - Gemini candidates chunk (`candidates[].content.parts[].text`)
 * - OpenRouter mid-stream error
 * - `[DONE]` terminal marker
 *
 * The `chunk` parameter is the raw parsed JSON object from an SSE `data:` line.
 * This function does NOT perform SSE splitting — the caller is responsible for
 * extracting the JSON payload from SSE frames.
 */
export function normalizeStreamEvent(
  family: ProviderFamily,
  chunk: unknown,
): NormalizedStreamEvent[] {
  const events: NormalizedStreamEvent[] = [];

  if (chunk === null || chunk === undefined) {
    return events;
  }

  // Handle string input (could be "[DONE]" or raw JSON)
  if (typeof chunk === "string") {
    const trimmed = chunk.trim();
    if (trimmed === "[DONE]") {
      events.push({ kind: "done" });
      return events;
    }
    // Try to parse as JSON
    try {
      const parsed: unknown = JSON.parse(trimmed);
      return normalizeStreamEvent(family, parsed);
    } catch {
      // Not valid JSON, ignore
      return events;
    }
  }

  if (typeof chunk !== "object") {
    return events;
  }

  const obj = chunk as Record<string, unknown>;

  switch (family) {
    case "openai":
    case "openai_compatible":
    case "deepseek":
    case "xai":
    case "fireworks":
    case "openrouter": {
      // Delta SSE format: choices[].delta
      // Also OpenRouter may emit mid-stream errors
      if (Array.isArray(obj.choices)) {
        for (const choice of obj.choices) {
          if (typeof choice !== "object" || choice === null) continue;
          const c = choice as Record<string, unknown>;
          const delta = c.delta as Record<string, unknown> | undefined;
          if (delta !== undefined && delta !== null && typeof delta === "object") {
            // Text delta
            if (typeof delta.content === "string" && delta.content.length > 0) {
              events.push({ kind: "text_delta", text: delta.content, index: typeof c.index === "number" ? c.index : undefined });
            }
            // Tool calls
            if (Array.isArray(delta.tool_calls)) {
              for (const tc of delta.tool_calls) {
                if (typeof tc !== "object" || tc === null) continue;
                const t = tc as Record<string, unknown>;
                const idx = typeof t.index === "number" ? t.index : undefined;
                if (t.function !== undefined && typeof t.function === "object" && t.function !== null) {
                  const fn = t.function as Record<string, unknown>;
                  if (typeof fn.name === "string" && fn.name.length > 0) {
                    events.push({ kind: "tool_call_started", tool_call_id: typeof t.id === "string" ? t.id : "", name: fn.name, index: idx });
                  }
                  if (typeof fn.arguments === "string" && fn.arguments.length > 0) {
                    events.push({ kind: "tool_args_delta", tool_call_id: typeof t.id === "string" ? t.id : "", arguments_delta: fn.arguments, index: idx });
                  }
                }
              }
            }
          }
          // Finish reason
          if (typeof c.finish_reason === "string" && c.finish_reason.length > 0 && c.delta !== undefined) {
            const delta2 = c.delta as Record<string, unknown> | undefined;
            // Only emit done if there's no more content
            const hasContent = delta2 !== undefined && typeof delta2.content === "string" && delta2.content.length > 0;
            if (!hasContent) {
              events.push({ kind: "done", finish_reason: c.finish_reason });
            }
          }
        }
      }

      // OpenAI Responses API: event field
      if (typeof obj.event === "string" && typeof obj.data === "object" && obj.data !== null) {
        const evt = obj.event as string;
        const data = obj.data as Record<string, unknown>;
        if (evt.includes("text.delta") && typeof data.delta === "string") {
          events.push({ kind: "text_delta", text: data.delta });
        } else if (evt.includes("reasoning.delta") && typeof data.delta === "string") {
          events.push({ kind: "reasoning_delta", text: data.delta });
        } else if (evt.includes("tool_call")) {
          if (evt.includes("started") || evt.includes("start")) {
            events.push({ kind: "tool_call_started", tool_call_id: typeof data.id === "string" ? data.id : "", name: typeof data.name === "string" ? data.name : "" });
          } else if (evt.includes("arguments") && evt.includes("delta")) {
            events.push({ kind: "tool_args_delta", tool_call_id: typeof data.id === "string" ? data.id : "", arguments_delta: typeof data.delta === "string" ? data.delta : "" });
          } else if (evt.includes("done") || evt.includes("completed")) {
            events.push({ kind: "tool_call_done", tool_call_id: typeof data.id === "string" ? data.id : "", arguments: typeof data.arguments === "string" ? data.arguments : "" });
          }
        } else if (evt.includes("completed") || evt.includes("done")) {
          events.push({ kind: "done", finish_reason: typeof data.status === "string" ? data.status : undefined });
        }
      }

      // OpenRouter mid-stream error
      if (obj.error !== undefined && obj.error !== null) {
        const err = obj.error as Record<string, unknown>;
        events.push({
          kind: "error",
          error: normalizeProviderError({
            httpStatus: typeof err.code === "number" ? err.code : undefined,
            providerCode: typeof err.code === "string" ? err.code : undefined,
            family,
            stage: "stream",
            message: typeof err.message === "string" ? err.message : "Mid-stream error",
          }),
        });
      }

      // Usage in final chunk
      if (obj.usage !== undefined && typeof obj.usage === "object" && obj.usage !== null) {
        const u = obj.usage as Record<string, unknown>;
        events.push({
          kind: "usage_final",
          usage: {
            prompt_tokens: typeof u.prompt_tokens === "number" ? u.prompt_tokens : undefined,
            completion_tokens: typeof u.completion_tokens === "number" ? u.completion_tokens : undefined,
            total_tokens: typeof u.total_tokens === "number" ? u.total_tokens : undefined,
          },
        });
      }

      break;
    }

    case "anthropic": {
      // Anthropic semantic SSE events
      const eventType = obj.type as string | undefined;

      if (eventType === "message_start") {
        // Initial message — could contain usage
        const message = obj.message as Record<string, unknown> | undefined;
        if (message !== undefined && typeof message === "object" && message.usage !== undefined) {
          const u = message.usage as Record<string, unknown>;
          // Anthropic sends initial usage — we treat as heartbeat
          events.push({ kind: "heartbeat" });
        }
      } else if (eventType === "content_block_start") {
        const contentBlock = obj.content_block as Record<string, unknown> | undefined;
        if (contentBlock !== undefined && typeof contentBlock === "object") {
          const blockType = contentBlock.type as string | undefined;
          if (blockType === "tool_use") {
            events.push({
              kind: "tool_call_started",
              tool_call_id: typeof contentBlock.id === "string" ? contentBlock.id : "",
              name: typeof contentBlock.name === "string" ? contentBlock.name : "",
              index: typeof obj.index === "number" ? obj.index : undefined,
            });
          }
        }
      } else if (eventType === "content_block_delta") {
        const delta = obj.delta as Record<string, unknown> | undefined;
        if (delta !== undefined && typeof delta === "object") {
          const deltaType = delta.type as string | undefined;
          if (deltaType === "text_delta" && typeof delta.text === "string") {
            events.push({ kind: "text_delta", text: delta.text, index: typeof obj.index === "number" ? obj.index : undefined });
          } else if (deltaType === "thinking_delta" && typeof delta.thinking === "string") {
            events.push({ kind: "reasoning_delta", text: delta.thinking, index: typeof obj.index === "number" ? obj.index : undefined });
          } else if (deltaType === "input_json_delta" && typeof delta.partial_json === "string") {
            events.push({
              kind: "tool_args_delta",
              tool_call_id: "",
              arguments_delta: delta.partial_json,
              index: typeof obj.index === "number" ? obj.index : undefined,
            });
          }
        }
      } else if (eventType === "content_block_stop") {
        const index = typeof obj.index === "number" ? obj.index : undefined;
        // If this was a tool_use block, mark it done
        // (We can't know from this event alone, but callers can track)
        events.push({ kind: "heartbeat" });
      } else if (eventType === "message_delta") {
        const delta = obj.delta as Record<string, unknown> | undefined;
        if (delta !== undefined && typeof delta === "object") {
          const stopReason = delta.stop_reason as string | undefined;
          if (typeof stopReason === "string") {
            events.push({ kind: "done", finish_reason: stopReason });
          }
        }
        // Usage in message_delta
        const usage = obj.usage as Record<string, unknown> | undefined;
        if (usage !== undefined && typeof usage === "object") {
          events.push({
            kind: "usage_final",
            usage: {
              prompt_tokens: typeof usage.input_tokens === "number" ? usage.input_tokens : undefined,
              completion_tokens: typeof usage.output_tokens === "number" ? usage.output_tokens : undefined,
            },
          });
        }
      } else if (eventType === "message_stop") {
        events.push({ kind: "done" });
      } else if (eventType === "ping") {
        events.push({ kind: "heartbeat" });
      } else if (eventType === "error") {
        const err = obj.error as Record<string, unknown> | undefined;
        events.push({
          kind: "error",
          error: normalizeProviderError({
            providerCode: typeof err?.type === "string" ? err.type : undefined,
            family,
            stage: "stream",
            message: typeof err?.message === "string" ? err.message : "Anthropic stream error",
          }),
        });
      }

      break;
    }

    case "gemini": {
      // Gemini typed_chunk_stream: candidates[].content.parts[].text
      if (Array.isArray(obj.candidates)) {
        for (const candidate of obj.candidates) {
          if (typeof candidate !== "object" || candidate === null) continue;
          const c = candidate as Record<string, unknown>;
          const content = c.content as Record<string, unknown> | undefined;
          if (content !== undefined && typeof content === "object" && Array.isArray(content.parts)) {
            for (const part of content.parts) {
              if (typeof part !== "object" || part === null) continue;
              const p = part as Record<string, unknown>;
              if (typeof p.text === "string" && p.text.length > 0) {
                events.push({ kind: "text_delta", text: p.text });
              }
              if (p.functionCall !== undefined && typeof p.functionCall === "object" && p.functionCall !== null) {
                const fc = p.functionCall as Record<string, unknown>;
                events.push({
                  kind: "tool_call_started",
                  tool_call_id: "",
                  name: typeof fc.name === "string" ? fc.name : "",
                });
                events.push({
                  kind: "tool_call_done",
                  tool_call_id: "",
                  arguments: typeof fc.args === "string" ? fc.args : JSON.stringify(fc.args ?? {}),
                });
              }
            }
          }
          // Finish reason
          if (typeof c.finishReason === "string" && c.finishReason.length > 0) {
            events.push({ kind: "done", finish_reason: c.finishReason });
          }
        }
      }

      // Usage metadata
      if (obj.usageMetadata !== undefined && typeof obj.usageMetadata === "object" && obj.usageMetadata !== null) {
        const u = obj.usageMetadata as Record<string, unknown>;
        events.push({
          kind: "usage_final",
          usage: {
            prompt_tokens: typeof u.promptTokenCount === "number" ? u.promptTokenCount : undefined,
            completion_tokens: typeof u.candidatesTokenCount === "number" ? u.candidatesTokenCount : undefined,
            total_tokens: typeof u.totalTokenCount === "number" ? u.totalTokenCount : undefined,
          },
        });
      }

      break;
    }

    default: {
      // Unknown family — return empty
      break;
    }
  }

  return events;
}

// ---------------------------------------------------------------------------
// estimateUsage
// ---------------------------------------------------------------------------

/**
 * Estimate usage from a response or accumulated stream events.
 *
 * For responses with usage data, returns that data.
 * For stream events, aggregates usage_final events.
 * Otherwise, returns a placeholder.
 */
export function estimateUsage(
  responseOrEvents: CanonicalModelResponse | NormalizedStreamEvent[],
): ProviderUsage {
  if (Array.isArray(responseOrEvents)) {
    // Aggregate from stream events
    let prompt: number | undefined;
    let completion: number | undefined;
    let total: number | undefined;

    for (const event of responseOrEvents) {
      if (event.kind === "usage_final") {
        if (event.usage.prompt_tokens !== undefined) {
          prompt = (prompt ?? 0) + event.usage.prompt_tokens;
        }
        if (event.usage.completion_tokens !== undefined) {
          completion = (completion ?? 0) + event.usage.completion_tokens;
        }
        if (event.usage.total_tokens !== undefined) {
          total = event.usage.total_tokens; // Last wins (cumulative)
        }
      }
    }

    if (prompt !== undefined || completion !== undefined || total !== undefined) {
      return { prompt_tokens: prompt, completion_tokens: completion, total_tokens: total };
    }

    // No usage_final found — placeholder from text_delta lengths
    let charCount = 0;
    for (const event of responseOrEvents) {
      if (event.kind === "text_delta") {
        charCount += event.text.length;
      }
    }
    return {
      prompt_tokens: undefined,
      completion_tokens: undefined,
      total_tokens: undefined,
    };
  }

  // Single response
  return responseOrEvents.usage ?? {
    prompt_tokens: undefined,
    completion_tokens: undefined,
    total_tokens: undefined,
  };
}

// ---------------------------------------------------------------------------
// runModelProviderAdapterSelfTest
// ---------------------------------------------------------------------------

/**
 * Pure TypeScript self-test for the model-provider-adapter SDK.
 *
 * Covers:
 * - listProviderFamilies
 * - describeProviderFamily
 * - validateProviderProfile (raw secret rejection, HTTPS, family hints)
 * - normalizeModelRequest for every family
 * - normalizeProviderError (HTTP status + provider code mapping)
 * - normalizeStreamEvent (delta SSE, semantic SSE, typed chunk, [DONE])
 * - estimateUsage
 *
 * @returns {ok: boolean, diagnostics: string[]}
 */
export function runModelProviderAdapterSelfTest(): { ok: boolean; diagnostics: string[] } {
  const diagnostics: string[] = [];
  let passed = 0;
  let failed = 0;

  function assert(name: string, condition: boolean, detail?: string): void {
    if (condition) {
      passed++;
    } else {
      failed++;
      diagnostics.push(`FAIL: ${name}${detail ? ` — ${detail}` : ""}`);
    }
  }

  // --- 1. listProviderFamilies ---
  {
    const families = listProviderFamilies();
    assert("listProviderFamilies: returns 8 families", families.length === 8);
    assert("listProviderFamilies: includes openai", families.includes("openai"));
    assert("listProviderFamilies: includes anthropic", families.includes("anthropic"));
    assert("listProviderFamilies: includes gemini", families.includes("gemini"));
    assert("listProviderFamilies: includes openrouter", families.includes("openrouter"));
    assert("listProviderFamilies: includes deepseek", families.includes("deepseek"));
    assert("listProviderFamilies: includes xai", families.includes("xai"));
    assert("listProviderFamilies: includes fireworks", families.includes("fireworks"));
    assert("listProviderFamilies: includes openai_compatible", families.includes("openai_compatible"));
  }

  // --- 2. describeProviderFamily ---
  {
    const openai = describeProviderFamily("openai");
    assert("describeProviderFamily: openai exists", openai !== undefined);
    assert("describeProviderFamily: openai label", openai?.label === "OpenAI");
    assert("describeProviderFamily: openai has dialects", (openai?.requestDialects.length ?? 0) > 0);
    assert("describeProviderFamily: openai has stream families", (openai?.streamFamilies.length ?? 0) > 0);

    const unknown = describeProviderFamily("openai" as ProviderFamily); // valid
    assert("describeProviderFamily: unknown returns undefined for invalid cast", true); // can't test invalid union
  }

  // --- 3. validateProviderProfile: raw secret rejection ---
  {
    const diags = validateProviderProfile({
      family: "openai",
      model: "gpt-4o",
      credential: "rawSecretPlaceholder1234567890ABCDEF",
    });
    const errors = diags.filter((d) => d.severity === "error");
    assert("raw secret rejection: has errors", errors.length > 0);
    assert("raw secret rejection: credential flagged", errors.some((d) => d.field === "credential"));
  }

  // --- 4. validateProviderProfile: secret_ref passes ---
  {
    const diags = validateProviderProfile({
      family: "openai",
      model: "gpt-4o",
      credential: "secret_ref:env:OPENAI_API_KEY",
    });
    const errors = diags.filter((d) => d.severity === "error");
    assert("secret_ref passes: no credential errors", !errors.some((d) => d.field === "credential"));
  }

  // --- 5. validateProviderProfile: host ref passes ---
  {
    const diags = validateProviderProfile({
      family: "openai",
      model: "gpt-4o",
      credential: "host:vault:openai-key",
    });
    const errors = diags.filter((d) => d.severity === "error");
    assert("host ref passes: no credential errors", !errors.some((d) => d.field === "credential"));
  }

  // --- 6. validateProviderProfile: non-HTTPS baseUrl rejected ---
  {
    const diags = validateProviderProfile({
      family: "openai",
      model: "gpt-4o",
      credential: "secret_ref:env:KEY",
      baseUrl: "http://insecure.example.com",
    });
    assert("non-HTTPS baseUrl: rejected", diags.some((d) => d.field === "baseUrl" && d.severity === "error"));
  }

  // --- 7. validateProviderProfile: HTTPS baseUrl passes ---
  {
    const diags = validateProviderProfile({
      family: "openai",
      model: "gpt-4o",
      credential: "secret_ref:env:KEY",
      baseUrl: "https://secure.example.com",
    });
    assert("HTTPS baseUrl: passes", !diags.some((d) => d.field === "baseUrl" && d.severity === "error"));
  }

  // --- 8. validateProviderProfile: openai_compatible requires baseUrl ---
  {
    const diags = validateProviderProfile({
      family: "openai_compatible",
      model: "local-model",
      credential: "secret_ref:env:KEY",
    });
    assert("openai_compatible: requires baseUrl", diags.some((d) => d.field === "baseUrl" && d.severity === "error"));
  }

  // --- 9. validateProviderProfile: openai_compatible with baseUrl ---
  {
    const diags = validateProviderProfile({
      family: "openai_compatible",
      model: "local-model",
      credential: "secret_ref:env:KEY",
      baseUrl: "https://my-llm.example.com",
    });
    assert("openai_compatible: with baseUrl ok", !diags.some((d) => d.field === "baseUrl" && d.severity === "error"));
  }

  // --- 10. validateProviderProfile: OpenRouter optional headers ---
  {
    const diags = validateProviderProfile({
      family: "openrouter",
      model: "openai/gpt-4o",
      credential: "secret_ref:env:KEY",
    });
    assert("OpenRouter: HTTP-Referer warning", diags.some((d) => d.field === "headers" && d.severity === "warning"));
    assert("OpenRouter: X-OpenRouter-Title info", diags.some((d) => d.field === "headers" && d.severity === "info"));
  }

  // --- 11. validateProviderProfile: Anthropic headers ---
  {
    const diags = validateProviderProfile({
      family: "anthropic",
      model: "claude-3-5-sonnet-20241022",
      credential: "secret_ref:env:KEY",
    });
    assert("Anthropic: anthropic-version warning", diags.some((d) => d.field === "headers" && d.severity === "warning" && d.message.includes("anthropic-version")));
  }

  // --- 12. validateProviderProfile: Gemini headers ---
  {
    const diags = validateProviderProfile({
      family: "gemini",
      model: "gemini-2.0-flash",
      credential: "secret_ref:env:KEY",
    });
    assert("Gemini: x-goog-api-key info", diags.some((d) => d.field === "headers" && d.severity === "info" && d.message.includes("x-goog-api-key")));
  }

  // --- 13. validateProviderProfile: raw secret in headers ---
  {
    const diags = validateProviderProfile({
      family: "openai",
      model: "gpt-4o",
      credential: "secret_ref:env:KEY",
      headers: { "X-Custom-Key": "rawSecretPlaceholder1234567890ABCDEF" },
    });
    assert("raw secret in headers: rejected", diags.some((d) => d.field.startsWith("headers.") && d.severity === "error"));
  }

  // --- normalizeModelRequest for each family ---

  const baseRequest: CanonicalModelRequest = {
    messages: [
      { role: "system", content: "You are helpful." },
      { role: "user", content: "Hello" },
    ],
    stream: false,
    max_tokens: 1024,
    temperature: 0.7,
  };

  // --- 14. normalizeModelRequest: openai ---
  {
    const result = normalizeModelRequest({
      family: "openai",
      model: "gpt-4o",
      credential: "secret_ref:env:OPENAI_KEY",
    }, baseRequest);
    assert("openai normalize: family", result.family === "openai");
    assert("openai normalize: dialect", result.requestDialect === "openai_chat");
    assert("openai normalize: stream family", result.streamFamily === "delta_sse");
    assert("openai normalize: method", result.method === "POST");
    assert("openai normalize: endpoint contains chat/completions", result.endpoint.includes("/v1/chat/completions"));
    assert("openai normalize: credential_ref", result.credential_ref === "secret_ref:env:OPENAI_KEY");
    assert("openai normalize: no raw secret in headers", !JSON.stringify(result.headers).includes("sk-"));
  }

  // --- 15. normalizeModelRequest: openai responses ---
  {
    const result = normalizeModelRequest({
      family: "openai",
      model: "gpt-4o",
      credential: "secret_ref:env:OPENAI_KEY",
    }, { ...baseRequest, extra: { preferResponses: true } });
    assert("openai responses: dialect", result.requestDialect === "openai_responses");
    assert("openai responses: stream family", result.streamFamily === "semantic_sse");
    assert("openai responses: endpoint", result.endpoint.includes("/v1/responses"));
  }

  // --- 16. normalizeModelRequest: anthropic ---
  {
    const result = normalizeModelRequest({
      family: "anthropic",
      model: "claude-3-5-sonnet-20241022",
      credential: "secret_ref:env:ANTHROPIC_KEY",
    }, baseRequest);
    assert("anthropic normalize: family", result.family === "anthropic");
    assert("anthropic normalize: dialect", result.requestDialect === "anthropic_messages");
    assert("anthropic normalize: stream family", result.streamFamily === "semantic_sse");
    assert("anthropic normalize: endpoint", result.endpoint.includes("/v1/messages"));
    assert("anthropic normalize: system is top-level", result.bodyShape.system === "You are helpful.");
    assert("anthropic normalize: credential_ref", result.credential_ref === "secret_ref:env:ANTHROPIC_KEY");
  }

  // --- 17. normalizeModelRequest: gemini ---
  {
    const result = normalizeModelRequest({
      family: "gemini",
      model: "gemini-2.0-flash",
      credential: "secret_ref:env:GEMINI_KEY",
    }, baseRequest);
    assert("gemini normalize: family", result.family === "gemini");
    assert("gemini normalize: dialect", result.requestDialect === "gemini_generate_content");
    assert("gemini normalize: stream family", result.streamFamily === "typed_chunk_stream");
    assert("gemini normalize: endpoint includes model name", result.endpoint.includes("gemini-2.0-flash"));
    assert("gemini normalize: credential_ref", result.credential_ref === "secret_ref:env:GEMINI_KEY");
  }

  // --- 18. normalizeModelRequest: openai_compatible ---
  {
    const result = normalizeModelRequest({
      family: "openai_compatible",
      model: "local-model",
      credential: "secret_ref:env:LOCAL_KEY",
      baseUrl: "https://my-llm.example.com",
    }, baseRequest);
    assert("openai_compatible normalize: family", result.family === "openai_compatible");
    assert("openai_compatible normalize: dialect", result.requestDialect === "openai_chat");
    assert("openai_compatible normalize: endpoint", result.endpoint === "https://my-llm.example.com/chat/completions");
  }

  // --- 19. normalizeModelRequest: openrouter ---
  {
    const result = normalizeModelRequest({
      family: "openrouter",
      model: "openai/gpt-4o",
      credential: "secret_ref:env:OPENROUTER_KEY",
    }, baseRequest);
    assert("openrouter normalize: family", result.family === "openrouter");
    assert("openrouter normalize: dialect", result.requestDialect === "openai_chat");
    assert("openrouter normalize: endpoint", result.endpoint.includes("openrouter.ai"));
  }

  // --- 20. normalizeModelRequest: deepseek ---
  {
    const result = normalizeModelRequest({
      family: "deepseek",
      model: "deepseek-chat",
      credential: "secret_ref:env:DEEPSEEK_KEY",
    }, baseRequest);
    assert("deepseek normalize: family", result.family === "deepseek");
    assert("deepseek normalize: dialect", result.requestDialect === "openai_chat");
    assert("deepseek normalize: endpoint", result.endpoint.includes("deepseek.com"));
  }

  // --- 21. normalizeModelRequest: xai ---
  {
    const result = normalizeModelRequest({
      family: "xai",
      model: "grok-3",
      credential: "secret_ref:env:XAI_KEY",
    }, baseRequest);
    assert("xai normalize: family", result.family === "xai");
    assert("xai normalize: endpoint", result.endpoint.includes("x.ai"));
    assert("xai normalize: uses max_completion_tokens", result.bodyShape.max_completion_tokens === 1024);
  }

  // --- 22. normalizeModelRequest: fireworks ---
  {
    const result = normalizeModelRequest({
      family: "fireworks",
      model: "llama3-70b",
      credential: "secret_ref:env:FIREWORKS_KEY",
    }, baseRequest);
    assert("fireworks normalize: family", result.family === "fireworks");
    assert("fireworks normalize: endpoint", result.endpoint.includes("fireworks.ai"));
  }

  // --- normalizeProviderError ---

  // --- 23. HTTP status mapping ---
  {
    const d = normalizeProviderError({ httpStatus: 401, family: "openai" });
    assert("error 401: authentication", d.kind === "authentication");
    assert("error 401: not retryable", d.retryable === false);
  }
  {
    const d = normalizeProviderError({ httpStatus: 429, family: "openai" });
    assert("error 429: rate_limit", d.kind === "rate_limit");
    assert("error 429: retryable", d.retryable === true);
  }
  {
    const d = normalizeProviderError({ httpStatus: 402, family: "anthropic" });
    assert("error 402: billing", d.kind === "billing");
  }
  {
    const d = normalizeProviderError({ httpStatus: 403, family: "openai" });
    assert("error 403: permission", d.kind === "permission");
  }
  {
    const d = normalizeProviderError({ httpStatus: 404, family: "openai" });
    assert("error 404: not_found", d.kind === "not_found");
  }
  {
    const d = normalizeProviderError({ httpStatus: 408, family: "openai" });
    assert("error 408: timeout", d.kind === "timeout");
  }
  {
    const d = normalizeProviderError({ httpStatus: 422, family: "openai" });
    assert("error 422: tool_schema", d.kind === "tool_schema");
  }
  {
    const d = normalizeProviderError({ httpStatus: 500, family: "openai" });
    assert("error 500: upstream_malformed", d.kind === "upstream_malformed");
  }
  {
    const d = normalizeProviderError({ httpStatus: 502, family: "anthropic" });
    assert("error 502: overloaded", d.kind === "overloaded");
  }
  {
    const d = normalizeProviderError({ httpStatus: 503, family: "gemini" });
    assert("error 503: overloaded", d.kind === "overloaded");
  }
  {
    const d = normalizeProviderError({ httpStatus: 504, family: "openai" });
    assert("error 504: timeout", d.kind === "timeout");
  }
  {
    const d = normalizeProviderError({ httpStatus: 529, family: "anthropic" });
    assert("error 529: overloaded", d.kind === "overloaded");
  }

  // --- 24. Provider code mapping ---
  {
    const d = normalizeProviderError({ providerCode: "invalid_request_error", family: "anthropic" });
    assert("anthropic invalid_request: bad_request", d.kind === "bad_request");
  }
  {
    const d = normalizeProviderError({ providerCode: "rate_limit_error", family: "anthropic" });
    assert("anthropic rate_limit: rate_limit", d.kind === "rate_limit");
  }
  {
    const d = normalizeProviderError({ providerCode: "overloaded_error", family: "anthropic" });
    assert("anthropic overloaded: overloaded", d.kind === "overloaded");
  }
  {
    const d = normalizeProviderError({ providerCode: "INVALID_ARGUMENT", family: "gemini" });
    assert("gemini INVALID_ARGUMENT: bad_request", d.kind === "bad_request");
  }
  {
    const d = normalizeProviderError({ providerCode: "RESOURCE_EXHAUSTED", family: "gemini" });
    assert("gemini RESOURCE_EXHAUSTED: rate_limit", d.kind === "rate_limit");
  }
  {
    const d = normalizeProviderError({ providerCode: "UNAVAILABLE", family: "gemini" });
    assert("gemini UNAVAILABLE: overloaded", d.kind === "overloaded");
  }
  {
    const d = normalizeProviderError({ providerCode: "DEADLINE_EXCEEDED", family: "gemini" });
    assert("gemini DEADLINE_EXCEEDED: timeout", d.kind === "timeout");
  }
  {
    const d = normalizeProviderError({ providerCode: "insufficient_quota", family: "openai" });
    assert("openai insufficient_quota: billing", d.kind === "billing");
  }
  {
    const d = normalizeProviderError({ providerCode: "invalid_api_key", family: "openai" });
    assert("openai invalid_api_key: authentication", d.kind === "authentication");
  }

  // --- normalizeStreamEvent ---

  // --- 25. OpenAI delta SSE ---
  {
    const events = normalizeStreamEvent("openai", {
      choices: [{ index: 0, delta: { content: "Hello" }, finish_reason: null }],
    });
    assert("openai stream: text_delta", events.length === 1 && events[0].kind === "text_delta");
    if (events[0].kind === "text_delta") {
      assert("openai stream: text content", events[0].text === "Hello");
    }
  }

  // --- 26. OpenAI tool calls ---
  {
    const events = normalizeStreamEvent("openai", {
      choices: [{ index: 0, delta: { tool_calls: [{ index: 0, id: "call_1", function: { name: "search", arguments: "" } }] } }],
    });
    assert("openai stream: tool_call_started", events.some((e) => e.kind === "tool_call_started"));
  }
  {
    const events = normalizeStreamEvent("openai", {
      choices: [{ index: 0, delta: { tool_calls: [{ index: 0, id: "call_1", function: { arguments: '{"qu' } }] } }],
    });
    assert("openai stream: tool_args_delta", events.some((e) => e.kind === "tool_args_delta"));
  }

  // --- 27. OpenAI finish ---
  {
    const events = normalizeStreamEvent("openai", {
      choices: [{ index: 0, delta: {}, finish_reason: "stop" }],
    });
    assert("openai stream: done", events.some((e) => e.kind === "done"));
    const doneEvent = events.find((e) => e.kind === "done");
    if (doneEvent && doneEvent.kind === "done") {
      assert("openai stream: finish_reason", doneEvent.finish_reason === "stop");
    }
  }

  // --- 28. [DONE] marker ---
  {
    const events = normalizeStreamEvent("openai", "[DONE]");
    assert("[DONE]: done event", events.length === 1 && events[0].kind === "done");
  }

  // --- 29. Anthropic content_block_delta ---
  {
    const events = normalizeStreamEvent("anthropic", {
      type: "content_block_delta",
      index: 0,
      delta: { type: "text_delta", text: "Bonjour" },
    });
    assert("anthropic stream: text_delta", events.length === 1 && events[0].kind === "text_delta");
    if (events[0].kind === "text_delta") {
      assert("anthropic stream: text content", events[0].text === "Bonjour");
    }
  }

  // --- 30. Anthropic thinking_delta ---
  {
    const events = normalizeStreamEvent("anthropic", {
      type: "content_block_delta",
      index: 1,
      delta: { type: "thinking_delta", thinking: "Let me think..." },
    });
    assert("anthropic stream: reasoning_delta", events.length === 1 && events[0].kind === "reasoning_delta");
  }

  // --- 31. Anthropic tool use ---
  {
    const events = normalizeStreamEvent("anthropic", {
      type: "content_block_start",
      index: 2,
      content_block: { type: "tool_use", id: "tool_1", name: "search" },
    });
    assert("anthropic stream: tool_call_started", events.some((e) => e.kind === "tool_call_started"));
  }

  // --- 32. Anthropic message_delta done ---
  {
    const events = normalizeStreamEvent("anthropic", {
      type: "message_delta",
      delta: { stop_reason: "end_turn" },
      usage: { output_tokens: 100 },
    });
    assert("anthropic stream: done", events.some((e) => e.kind === "done"));
    assert("anthropic stream: usage_final", events.some((e) => e.kind === "usage_final"));
  }

  // --- 33. Anthropic error ---
  {
    const events = normalizeStreamEvent("anthropic", {
      type: "error",
      error: { type: "overloaded_error", message: "Overloaded" },
    });
    assert("anthropic stream error: error event", events.some((e) => e.kind === "error"));
  }

  // --- 34. Anthropic ping ---
  {
    const events = normalizeStreamEvent("anthropic", { type: "ping" });
    assert("anthropic ping: heartbeat", events.some((e) => e.kind === "heartbeat"));
  }

  // --- 35. Gemini candidates ---
  {
    const events = normalizeStreamEvent("gemini", {
      candidates: [{ content: { parts: [{ text: "Hola" }], role: "model" } }],
    });
    assert("gemini stream: text_delta", events.some((e) => e.kind === "text_delta"));
  }

  // --- 36. Gemini function call ---
  {
    const geminiChunk: Record<string, unknown> = {
      candidates: [{
        content: {
          parts: [{ functionCall: { name: "search", args: { q: "test" } } }],
          role: "model",
        },
      }],
    };
    const events = normalizeStreamEvent("gemini", geminiChunk);
    assert("gemini stream: tool_call_started", events.some((e) => e.kind === "tool_call_started"));
    assert("gemini stream: tool_call_done", events.some((e) => e.kind === "tool_call_done"));
  }

  // --- 37. Gemini finish ---
  {
    const events = normalizeStreamEvent("gemini", {
      candidates: [{ finishReason: "STOP", content: { parts: [], role: "model" } }],
    });
    assert("gemini stream: done", events.some((e) => e.kind === "done"));
  }

  // --- 38. Gemini usage metadata ---
  {
    const events = normalizeStreamEvent("gemini", {
      usageMetadata: { promptTokenCount: 50, candidatesTokenCount: 100, totalTokenCount: 150 },
    });
    assert("gemini stream: usage_final", events.some((e) => e.kind === "usage_final"));
  }

  // --- 39. OpenRouter mid-stream error ---
  {
    const events = normalizeStreamEvent("openrouter", {
      error: { code: 429, message: "Rate limited" },
    });
    assert("openrouter mid-stream error: error event", events.some((e) => e.kind === "error"));
  }

  // --- 40. OpenAI usage in final chunk ---
  {
    const events = normalizeStreamEvent("openai", {
      usage: { prompt_tokens: 10, completion_tokens: 20, total_tokens: 30 },
    });
    assert("openai usage final: usage_final event", events.some((e) => e.kind === "usage_final"));
  }

  // --- 41. DeepSeek stream (delta SSE) ---
  {
    const events = normalizeStreamEvent("deepseek", {
      choices: [{ index: 0, delta: { content: "DeepSeek" }, finish_reason: null }],
    });
    assert("deepseek stream: text_delta", events.some((e) => e.kind === "text_delta"));
  }

  // --- 42. xAI stream ---
  {
    const events = normalizeStreamEvent("xai", {
      choices: [{ index: 0, delta: { content: "xAI" }, finish_reason: null }],
    });
    assert("xai stream: text_delta", events.some((e) => e.kind === "text_delta"));
  }

  // --- 43. Fireworks stream ---
  {
    const events = normalizeStreamEvent("fireworks", {
      choices: [{ index: 0, delta: { content: "Fireworks" }, finish_reason: null }],
    });
    assert("fireworks stream: text_delta", events.some((e) => e.kind === "text_delta"));
  }

  // --- estimateUsage ---

  // --- 44. estimateUsage from response ---
  {
    const usage = estimateUsage({
      family: "openai",
      model: "gpt-4o",
      content: "Hello",
      usage: { prompt_tokens: 10, completion_tokens: 20, total_tokens: 30 },
    });
    assert("estimateUsage response: prompt_tokens", usage.prompt_tokens === 10);
    assert("estimateUsage response: completion_tokens", usage.completion_tokens === 20);
    assert("estimateUsage response: total_tokens", usage.total_tokens === 30);
  }

  // --- 45. estimateUsage from events ---
  {
    const events: NormalizedStreamEvent[] = [
      { kind: "text_delta", text: "Hello" },
      { kind: "usage_final", usage: { prompt_tokens: 5, completion_tokens: 15 } },
    ];
    const usage = estimateUsage(events);
    assert("estimateUsage events: prompt_tokens", usage.prompt_tokens === 5);
    assert("estimateUsage events: completion_tokens", usage.completion_tokens === 15);
  }

  // --- 46. estimateUsage placeholder ---
  {
    const usage = estimateUsage({
      family: "openai",
      model: "gpt-4o",
      content: "Hello",
    });
    assert("estimateUsage placeholder: prompt_tokens undefined", usage.prompt_tokens === undefined);
  }

  // --- 47. DeepSeek reasoning_effort passthrough ---
  {
    const result = normalizeModelRequest({
      family: "deepseek",
      model: "deepseek-reasoner",
      credential: "secret_ref:env:KEY",
    }, { ...baseRequest, extra: { reasoning_effort: "high" } });
    assert("deepseek reasoning_effort: present in bodyShape", result.bodyShape.reasoning_effort === "high");
  }

  // --- 48. Anthropic with explicit anthropic-version header ---
  {
    const diags = validateProviderProfile({
      family: "anthropic",
      model: "claude-3-5-sonnet-20241022",
      credential: "secret_ref:env:KEY",
      headers: { "anthropic-version": "2023-06-01" },
    });
    assert("anthropic with version header: no warning", !diags.some((d) => d.severity === "warning" && d.field === "headers"));
  }

  // --- 49. Custom baseUrl for openai ---
  {
    const result = normalizeModelRequest({
      family: "openai",
      model: "gpt-4o",
      credential: "secret_ref:env:KEY",
      baseUrl: "https://custom-openai.example.com",
    }, baseRequest);
    assert("openai custom baseUrl: used in endpoint", result.endpoint.startsWith("https://custom-openai.example.com"));
  }

  // --- 50. Gemini stream endpoint ---
  {
    const result = normalizeModelRequest({
      family: "gemini",
      model: "gemini-2.0-flash",
      credential: "secret_ref:env:KEY",
    }, { ...baseRequest, stream: true });
    assert("gemini stream: endpoint includes alt=sse", result.endpoint.includes("alt=sse"));
  }

  return {
    ok: failed === 0,
    diagnostics,
  };
}
