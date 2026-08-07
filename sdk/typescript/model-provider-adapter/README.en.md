# Plurora Model Provider Adapter SDK

Pure TypeScript adapter for normalizing model provider API differences across OpenAI / Anthropic / Gemini / OpenAI-compatible / OpenRouter / DeepSeek / xAI / Fireworks — without importing private runtime, making real HTTP calls, doing billing/proxying, or adding `platform.model.*`.

**This is an SDK adapter, not a provider package.** It does not go online, does not bill or proxy, and does not add platform.model semantics. It only handles provider profile description, request normalization, error classification, and stream event parsing.

## References

- [`integrations/model-providers/provider-matrix.yaml`](../../../integrations/model-providers/provider-matrix.yaml) — provider API matrix
- [`integrations/model-providers/README.en.md`](../../../integrations/model-providers/README.en.md) — stream protocol families and error taxonomy

## Usage

```ts
import {
  // Types
  ProviderFamily,
  RequestDialect,
  StreamFamily,
  ToolMode,
  UsageMode,
  ProviderProfile,
  CanonicalModelMessage,
  CanonicalModelRequest,
  CanonicalModelResponse,
  NormalizedStreamEvent,
  ProviderErrorKind,
  ProviderUsage,
  ProviderCost,
  ProviderDiagnostic,
  NormalizedProviderRequest,
  ValidationDiagnostic,
  ProviderErrorInput,

  // Functions
  listProviderFamilies,
  describeProviderFamily,
  validateProviderProfile,
  normalizeModelRequest,
  normalizeProviderError,
  normalizeStreamEvent,
  estimateUsage,
  runModelProviderAdapterSelfTest,
} from "./index";
```

### List provider families

```ts
const families = listProviderFamilies();
// ["openai", "anthropic", "gemini", "openai_compatible", "openrouter", "deepseek", "xai", "fireworks"]

const meta = describeProviderFamily("openai");
// meta.requestDialects, meta.streamFamilies, meta.usageModes, …
```

### Validate a provider profile

```ts
const diagnostics = validateProviderProfile({
  family: "openai",
  model: "gpt-4o",
  credential: "secret_ref:env:OPENAI_API_KEY",
  baseUrl: "https://api.openai.com",
});

// Error-level diagnostics must be fixed
// Raw API keys are rejected:
validateProviderProfile({
  family: "openai",
  model: "gpt-4o",
  credential: "rawSecretPlaceholder1234567890ABCDEF",
});
// → error: "Raw API key detected in credential. Use a secret_ref: or host: reference."

// Non-HTTPS baseUrl is rejected
// OpenRouter missing HTTP-Referer → warning
// Anthropic missing anthropic-version → warning
// Gemini → x-goog-api-key info hint
```

### Normalize a model request

```ts
const normalized = normalizeModelRequest(
  {
    family: "openai",
    model: "gpt-4o",
    credential: "secret_ref:env:OPENAI_KEY",
  },
  {
    messages: [
      { role: "system", content: "You are helpful." },
      { role: "user", content: "Hello" },
    ],
    stream: true,
    max_tokens: 1024,
  },
);

// normalized.method === "POST"
// normalized.endpoint === "https://api.openai.com/v1/chat/completions"
// normalized.requestDialect === "openai_chat"
// normalized.streamFamily === "delta_sse"
// normalized.credential_ref === "secret_ref:env:OPENAI_KEY"
// normalized.bodyShape — contains model, messages, stream, max_tokens, temperature
```

Anthropic normalization lifts system messages to the top level:

```ts
const normalized = normalizeModelRequest(
  { family: "anthropic", model: "claude-3-5-sonnet-20241022", credential: "secret_ref:env:KEY" },
  { messages: [{ role: "system", content: "You are helpful." }, { role: "user", content: "Hi" }] },
);
// normalized.bodyShape.system === "You are helpful."
// normalized.bodyShape.messages does not contain the system message
```

### Error normalization

```ts
const diag = normalizeProviderError({
  httpStatus: 429,
  family: "openai",
  stage: "request",
});
// diag.kind === "rate_limit"
// diag.retryable === true

const diag2 = normalizeProviderError({
  providerCode: "overloaded_error",
  family: "anthropic",
  stage: "stream",
  message: "Overloaded",
});
// diag2.kind === "overloaded"
// diag2.retryable === true
```

### Stream event normalization

```ts
// OpenAI delta SSE
const events = normalizeStreamEvent("openai", {
  choices: [{ index: 0, delta: { content: "Hello" }, finish_reason: null }],
});
// events → [{ kind: "text_delta", text: "Hello", index: 0 }]

// [DONE] marker
const done = normalizeStreamEvent("openai", "[DONE]");
// done → [{ kind: "done" }]

// Anthropic content_block_delta
const events = normalizeStreamEvent("anthropic", {
  type: "content_block_delta",
  index: 0,
  delta: { type: "text_delta", text: "Bonjour" },
});
// events → [{ kind: "text_delta", text: "Bonjour", index: 0 }]

// Gemini candidates chunk
const events = normalizeStreamEvent("gemini", {
  candidates: [{ content: { parts: [{ text: "Hola" }], role: "model" } }],
});
// events → [{ kind: "text_delta", text: "Hola" }]

// OpenRouter mid-stream error
const events = normalizeStreamEvent("openrouter", {
  error: { code: 429, message: "Rate limited" },
});
// events → [{ kind: "error", error: { kind: "rate_limit", retryable: true, … } }]
```

### Usage estimation

```ts
const usage = estimateUsage({
  family: "openai",
  model: "gpt-4o",
  content: "Hello",
  usage: { prompt_tokens: 10, completion_tokens: 20, total_tokens: 30 },
});
// usage.prompt_tokens === 10

// Can also aggregate from stream event arrays
const events: NormalizedStreamEvent[] = [
  { kind: "text_delta", text: "Hello" },
  { kind: "usage_final", usage: { prompt_tokens: 5, completion_tokens: 15 } },
];
const usage = estimateUsage(events);
```

### Self-test

```ts
const result = runModelProviderAdapterSelfTest();
if (!result.ok) {
  for (const d of result.diagnostics) console.error(d);
  throw new Error("Self-test failed");
}
```

## API Reference

| Export | Type | Description |
|---|---|---|
| `ProviderFamily` | type | Supported family union |
| `RequestDialect` | type | Request dialect union |
| `StreamFamily` | type | Stream protocol family union |
| `ToolMode` | type | Tool mode union |
| `UsageMode` | type | Usage mode union |
| `ProviderProfile` | interface | Provider profile |
| `CanonicalModelMessage` | interface | Canonical model message |
| `CanonicalModelRequest` | interface | Canonical model request |
| `CanonicalModelResponse` | interface | Canonical model response |
| `NormalizedStreamEvent` | type | Normalized stream event union |
| `ProviderErrorKind` | type | Error classification union |
| `ProviderUsage` | interface | Usage metrics |
| `ProviderCost` | interface | Cost estimate |
| `ProviderDiagnostic` | interface | Diagnostic result |
| `NormalizedProviderRequest` | interface | Provider-specific request shape |
| `ValidationDiagnostic` | interface | Validation diagnostic |
| `ProviderErrorInput` | interface | Error normalization input |
| `listProviderFamilies` | function | List all supported families |
| `describeProviderFamily` | function | Describe a family |
| `validateProviderProfile` | function | Validate profile, reject raw secrets |
| `normalizeModelRequest` | function | Normalize model request |
| `normalizeProviderError` | function | Normalize provider error |
| `normalizeStreamEvent` | function | Normalize stream event |
| `estimateUsage` | function | Estimate/aggregate usage |
| `runModelProviderAdapterSelfTest` | function | Pure-TS self-test |

## Supported Provider Families

| Family | Request dialects | Stream family | Notes |
|---|---|---|---|
| `openai` | `openai_responses`, `openai_chat` | `semantic_sse`, `delta_sse` | Responses API uses semantic events |
| `anthropic` | `anthropic_messages` | `semantic_sse` | system is top-level; requires `anthropic-version` |
| `gemini` | `gemini_generate_content` | `typed_chunk_stream` | Not OpenAI-compatible; requires `x-goog-api-key` |
| `openai_compatible` | `openai_chat` | `delta_sse` | Custom baseUrl required |
| `openrouter` | `stateless_responses`, `openai_chat` | `semantic_sse`, `delta_sse` | Stateless; mid-stream errors possible |
| `deepseek` | `openai_chat` | `delta_sse` | Supports `reasoning_effort` |
| `xai` | `openai_responses`, `openai_chat` | `semantic_sse`, `delta_sse` | Uses `max_completion_tokens` |
| `fireworks` | `openai_chat`, `fireworks_responses` | `delta_sse`, `semantic_sse` | MCP/rollout headers |

## Normalized Stream Events

| Event | Description |
|---|---|
| `text_delta` | Text increment |
| `reasoning_delta` | Reasoning/thinking increment |
| `tool_call_started` | Tool call begins |
| `tool_args_delta` | Tool arguments increment |
| `tool_call_done` | Tool call completed |
| `citation` | Citation/source |
| `usage_final` | Final usage |
| `error` | Mid-stream error |
| `done` | Stream terminated |
| `heartbeat` | Keepalive/ping |

## Error Taxonomy

| ProviderErrorKind | HTTP | Description |
|---|---|---|
| `bad_request` | 400 | Malformed request |
| `authentication` | 401 | Authentication failed |
| `billing` | 402 | Account/billing issue |
| `permission` | 403 | Insufficient permissions |
| `not_found` | 404 | Resource not found |
| `timeout` | 408/504 | Timeout |
| `rate_limit` | 429 | Rate limited |
| `tool_schema` | 422 | Tool argument validation failure |
| `overloaded` | 502/503/529 | Upstream overloaded |
| `upstream_malformed` | 500 | Upstream internal error |
| `stream_error` | — | Stream error |
| `network_denied` | — | Network policy denied |
| `secret_unavailable` | — | Secret unavailable |
| `unknown` | — | Unknown error |

## Design Constraints

- **Does not import** any private runtime module
- **No external dependencies**, pure TypeScript
- **No network I/O**, no real HTTP calls
- **No billing**, no proxying
- **Does not add** `platform.model.*`, `platform.prompt.*`, `platform.chat.*`
- **Does not use** `any` type; prefers `unknown` + type guards
- **Does not hardcode** secrets; only accepts `secret_ref:*` or `host:*` references
- Raw API keys are rejected by `validateProviderProfile`
