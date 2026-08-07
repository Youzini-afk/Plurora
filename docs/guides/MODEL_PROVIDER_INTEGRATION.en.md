# Cloud Model Provider Integration

> [English](./MODEL_PROVIDER_INTEGRATION.en.md) · [中文](./MODEL_PROVIDER_INTEGRATION.md)

This guide documents Plurora's cloud model provider adapter integration path. It is not a relay gateway, billing system, provider admin backend, or kernel model abstraction. Cloud model integration must be delivered as ordinary capability packages using the same manifest, permission, `secret_ref`, outbound audit, stream/cancel, and validation boundaries as every other package.

## Scope: cloud adapter, not platform abstraction

`plurora/model-provider-lab` is a cloud API adapter lab:

- It is not the Plurora model abstraction.
- It is not a LiteLLM / OneAPI compatible gateway.
- It is not a provider marketplace, billing system, or channel admin backend.
- It has no kernel privilege; first-party and third-party packages must use the same public protocol / permission / secret / outbound boundary.
- OpenAI, Anthropic, Gemini, OpenRouter, DeepSeek, xAI, and Fireworks schemas are adapter-local details, not platform public protocol.
- `normalize_request` is a cloud-adapter-local request builder helper, not the Plurora canonical inference request.

For transport-neutral inference packages, read [`INFERENCE_CAPABILITY_AUTHORING.md`](./INFERENCE_CAPABILITY_AUTHORING.en.md). For the non-HTTP / local / self-host seam proof, see `plurora/inference-local-lab`.

## Current delivery

The current delivery includes:

- `integrations/model-providers/` stores the provider research ledger, provider matrix, stream compatibility notes, and error taxonomy.
- `sdk/typescript/model-provider-adapter` provides a pure TypeScript cloud adapter for provider profiles, adapter-local request builders, error classification, and stream event parsing. It does not go online, do billing, or access private runtime APIs.
- `plurora/model-provider-lab` is an ordinary official cloud adapter capability package covering OpenAI, Anthropic, Gemini, OpenAI-compatible, OpenRouter, DeepSeek, xAI, Fireworks, and related cloud provider families.
- `plurora/model-provider-lab` exposes `list_supported_families`, `validate_profile`, `normalize_request`, `invoke`, `normalize_stream`, `explain_error`, and `echo`.
- `invoke` remains a fake/local provider adapter path for default validation and adapter-shape checks. It returns provider-shaped responses and auditable `outbound_request_shape`.
- The host has a content-free `OutboundExecutor` boundary. It defaults to deny-all and has fake executor, loopback live HTTP executor, and hostile validation coverage. This proves request shapes can flow through host policy/audit boundaries, but it does not claim OS-level interception of arbitrary subprocess networking.
- `host.outbound.execute` is the public outbound protocol. Ordinary and first-party Packages must use the same path; the package principal comes from protocol context and cannot spoof another package.
- `EnvSecretResolver` supports host-owned `secret_ref:env:NAME` allowlists. Raw secrets only exist briefly inside the host and never enter events, logs, audits, or responses.
- `LiveHttpOutboundExecutor` uses `reqwest + rustls`, is disabled by default, enforces HTTPS-only, fails closed on redirects, requires timeouts, and records only redacted response/audit shapes. Loopback conformance uses `allow_insecure_loopback_for_tests=true` and does not depend on public internet.
- `secret_headers` provides host-side header injection (for example Authorization bearer, x-api-key, and x-goog-api-key). Missing/invalid secrets fail closed. `static_headers` accepts only a tiny set of non-secret provider/version/format headers (anthropic-version, content-type, accept, http-referer, x-title) and blocks Authorization/x-api-key/Cookie and host-owned headers.
- Live-call conformance covers the DeepSeek canary, OpenAI Chat/Responses, Anthropic Messages, Gemini generateContent, OpenRouter, xAI, and Fireworks loopback shapes, missing-secret fail-closed behavior, and no raw-secret leakage.
- `normalize_stream` maps delta SSE, semantic SSE, and typed chunk streams into `StreamFrameEnvelope`-style frames: `start`, `chunk`, `progress`, `end`, `error`, `cancelled`, and `timeout`. It also covers common provider quirks.

## Provider families

| Family | Request dialects | Stream shape | Default host |
| --- | --- | --- | --- |
| `openai` | `openai_chat`, `openai_responses` | `delta_sse`, `semantic_sse` | `api.openai.com` |
| `anthropic` | `anthropic_messages` | `semantic_sse` | `api.anthropic.com` |
| `gemini` | `gemini_generate_content` | `typed_chunk_stream` | `generativelanguage.googleapis.com` |
| `openai_compatible` | `openai_chat` | `delta_sse` | explicit `baseUrl` required |
| `openrouter` | `openai_chat`, `stateless_responses` | `delta_sse`, `semantic_sse` | `openrouter.ai` |
| `deepseek` | `openai_chat` | `delta_sse` | `api.deepseek.com` |
| `xai` | `openai_chat`, `openai_responses` | `delta_sse`, `semantic_sse` | `api.x.ai` |
| `fireworks` | `openai_chat`, `fireworks_responses` | `delta_sse`, `semantic_sse` | `api.fireworks.ai` |

OpenAI-compatible is an adapter family, not Plurora's only model worldview. Anthropic, Gemini, and Responses-style streams have different structures and must be adapted explicitly.

## Profile shape

Example profiles live under `examples/model-provider-profiles/`. All examples use `secret_ref`; none contain real keys.

```json
{
  "family": "anthropic",
  "model": "claude-3-5-sonnet-20241022",
  "credential": "secret_ref:env:ANTHROPIC_API_KEY",
  "baseUrl": "https://api.anthropic.com",
  "headers": {
    "anthropic-version": "2023-06-01"
  }
}
```

Rules:

- `credential` must be a `secret_ref:*`, `secretRef:*`, `secret-ref:*`, or `host:*` reference.
- Raw credentials, raw-looking headers, `api_key`, and `secret` fields are rejected.
- `invoke` requires an HTTPS `baseUrl`; `openai_compatible` has no default host and must provide an explicit HTTPS `baseUrl`.
- Profiles may carry provider-specific `headers` and `extra`, but these remain package semantics and do not become kernel ontology.

## Common capabilities

### `list_supported_families`

Returns provider families with default base URL, request dialect, stream family, credential header, and notes. It does not go online.

### `validate_profile`

Checks family, model, credential, base URL, and headers, returning:

- `valid`
- `diagnostics`
- `network_required`
- `secret_refs`
- `network_performed=false`
- `inference_performed=false`

### `normalize_request`

`normalize_request` is an internal request-builder helper for the cloud adapter package: it converts adapter-local input into provider-specific request shapes. It is not the Plurora canonical inference request and should not be treated as a platform-wide unified chat schema. The transport-neutral inference contract lives in `sdk/typescript/inference-capability`.

Examples:

- OpenAI Chat: `/v1/chat/completions`
- OpenAI Responses: `/v1/responses`
- Anthropic: `/v1/messages`
- Gemini: `/v1beta/models/{model}:generateContent`
- OpenRouter: `/chat/completions` or `/responses`
- DeepSeek: `/chat/completions`
- xAI: `/v1/chat/completions` or `/v1/responses`
- Fireworks: `/chat/completions` or `/responses`

### `invoke`

`plurora/model-provider-lab/invoke` itself remains the fake/local adapter path. Real network calls do not use private official-package runtime access; they must be made by ordinary packages through public `host.outbound.execute`, under host policy, secret resolution, and outbound execution.

Outputs must keep:

```json
{
  "network_performed": false,
  "inference_performed": false,
  "executor_kind": "fake_local",
  "live_call_supported": false
}
```

This preserves replayable adapter testing and default validation. It also prevents the first-party provider package from gaining private outbound privileges unavailable to third parties.

### `host.outbound.execute`

The live HTTP path for ordinary capability packages:

```json
{
  "capability_id": "example/provider/fetch",
  "destination_host": "api.openai.com",
  "method": "POST",
  "path": "/v1/chat/completions",
  "secret_headers": {
    "Authorization": {"secret_ref": "secret_ref:env:OPENAI_API_KEY", "scheme": "bearer"}
  },
  "body_shape": {"model": "gpt-4o", "messages": [{"role": "user", "content": "hello"}]}
}
```

Rules:

- The caller package comes from `ProtocolContext::Package`, not from spoofable params; `capability_id` must belong to the caller package namespace.
- The host resolves `secret_headers` first. Resolution failures fail closed and do not send requests.
- The policy/audit request and executor request must agree on package/capability/host/method/secret_refs or fail closed.
- Raw secrets, Authorization values, request bodies, and response bodies are not audited; only redacted shapes, secret_refs, host, method, purpose, status, usage/cost/error metadata are recorded.
- The default runtime remains deny-all; live execution requires explicit host opt-in.

### `normalize_stream`

Maps provider events/chunks into `StreamFrameEnvelope`-style frames. Provider-specific fields stay in `payload` or `metadata`:

- OpenAI Chat / OpenAI-compatible / OpenRouter / DeepSeek / xAI / Fireworks: delta SSE.
- OpenAI Responses / Anthropic: semantic SSE.
- Gemini: typed chunk stream; `GenerateContentResponse`-style snapshots are adapted into chunk/end/progress frames.

Structural events such as Anthropic `message_start`, `content_block_start`, OpenRouter comments, and Gemini snapshots with no new text become `progress`, not text chunks. Usage, finish reasons, safety ratings, performance metrics, and generation IDs go into `metadata` or end payload, not kernel semantics.

### `explain_error`

Normalizes provider-specific code/status values into package-level error categories, such as:

- `authentication`
- `permission`
- `rate_limit`
- `billing`
- `not_found`
- `bad_request`
- `tool_schema`
- `timeout`
- `overloaded`
- `stream_error`
- `upstream_malformed`
- `unknown`

This is package-level normalization, not `platform.model.error`.

## Manual live call boundary

Default validation does not require public internet access. Manual/live provider requests must satisfy:

1. the provider package declares minimal network permissions;
2. caller or host policy explicitly allows them;
3. secrets are resolved through host resolver, with raw secrets never entering events/logs/audits;
4. requests go through public `host.outbound.execute` and the host outbound boundary;
5. audits record only host, method, purpose, secret_refs, usage/cost/error metadata, and redaction state;
6. streams map to the content-free frame lifecycle;
7. cancel/timeout are not swallowed by provider adapters;
8. third-party provider packages can replace first-party Packages without official priority.

The optional real DeepSeek smoke path only runs when explicitly enabled:

```bash
PLURORA_LIVE_MODEL_TESTS=1 DEEPSEEK_API_KEY=... cargo run -p plurora-cli -- conformance
```

Default CI / default conformance never accesses public internet.

## Relationship to `inference-capability` / `inference-local-lab`

- `sdk/typescript/inference-capability`: transport-neutral inference envelope, stream frame, error taxonomy, and provider capability manifest helpers; does not require URL/header/status-code/OpenAI messages fields.
- `plurora/inference-local-lab`: deterministic non-HTTP fake local provider proof; proves the inference seam does not depend on HTTP, bearer tokens, JSON provider schemas, or network.
- `plurora/model-provider-lab`: cloud API adapter lab for realistic cloud provider integration; not the platform abstraction.

The dependency direction is: transport-neutral contract → cloud/local adapter packages. The kernel does not import, know, or hardcode these provider semantics.

## Non-goals

- User balances, top-ups, billing admin, multipliers, or channel management systems.
- Hosted platform relay keys.
- `platform.model.*`, `platform.prompt.*`, `platform.chat.*`, or `platform.embedding.*`.
- Treating OpenAI-compatible as the only model protocol.
- Treating `normalize_request` as the platform canonical request.
- Letting first-party Packages bypass manifest, permission, secret, network, or audit boundaries.

## Validation

```bash
cargo test --workspace
cargo run -p plurora-cli -- conformance
cargo run -p plurora-cli -- package check packages/plurora/model-provider-lab/manifest.yaml
tsc -p clients/web/tsconfig.json --noEmit
```

Current validation can cover `first_party.model_provider_lab`, `first_party.model_provider_lab_invoke_core`, `first_party.model_provider_lab_normalize_stream`, `first_party.inference_local_lab_*`, public `host.outbound.execute`, secret header injection, live loopback provider shapes, provider quirk fixtures, non-HTTP inference seam proof, and outbound policy checks.
