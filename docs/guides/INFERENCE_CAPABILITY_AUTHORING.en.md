# Inference Capability Package Authoring Guide

> [English](./INFERENCE_CAPABILITY_AUTHORING.en.md) · [中文](./INFERENCE_CAPABILITY_AUTHORING.md)

This guide documents how to author inference capability packages for Plurora. Plurora can be API-first without making the platform protocol look like one vendor API. Cloud API adapters are one kind of provider. Local processes, in-memory compute, IPC pipes, and WebSocket channels can carry inference too.

## Core position

1. Inference is an ordinary capability, not a kernel primitive. There are no `platform.model.*`, `platform.prompt.*`, `platform.chat.*`, or `platform.embedding.*`.
2. The request envelope is transport-neutral. It does not contain URL, HTTP header, status code, or OpenAI messages fields.
3. Cloud adapters are one class of provider, not the platform model abstraction. `official/model-provider-lab` is an ordinary cloud API adapter lab; it does not represent Ygg's model worldview.
4. `local_process`, `in_memory`, `ipc`, `websocket`, and `http` have equal standing.
5. Secrets use `secret_ref`. Raw secrets are rejected in every field.

## SDK location

`sdk/typescript/inference-capability/` provides:

- `InferenceRequest` — transport-neutral inference request envelope
- `InferenceResponse` — transport-neutral inference response envelope
- `InferenceStreamFrame` — canonical stream frame
- `InferenceError` — transport-neutral error classification
- `ProviderCapabilityManifest` — provider capability declaration
- Helper functions — `createInferenceRequest`, `classifyInferenceError`, `InferenceStreamLifecycle`, `createProviderCapabilityManifest`, etc.

## Request envelope

```typescript
import { createInferenceRequest } from "@plurora/inference-capability";

// Local process inference request — no URL, header, or status code needed
const req = createInferenceRequest({
  operation_id: "op_local_001",
  operation_kind: "generate",
  input_refs: [{ ref_id: "artifact:scene_state_v3", mime_hint: "application/json" }],
  input_payload: { kind: "json", shape: { type: "scene_state" } },
  streaming: true,
  cancellation: { deadline: "2026-12-31T23:59:59Z" },
  resource_hints: { max_output_units: 512, temperature: 0.7 },
  secret_refs: ["secret_ref:env:LOCAL_MODEL_KEY"],
  transport_kind: "local_process",
});
```

Key constraints:
- `input_refs` are opaque artifact references, not URLs.
- `input_payload` is an opaque payload description, not a message array, and has no `system`/`user`/`assistant` fields.
- `secret_refs` only accepts `secret_ref:*` or `host:*`; raw secrets are rejected.
- `transport_kind` is a semantic hint (`http`/`local_process`/`in_memory`/`ipc`/`websocket`/`remote`/`custom`), not a URL.

## Error taxonomy

The transport-neutral error taxonomy covers:

- Cloud errors: `authentication`, `permission`, `billing`, `rate_limit`, `provider_overloaded`, `provider_unavailable`, `bad_request`, `not_found`
- Local/resource errors: `local_process_failed`, `local_process_timeout`, `local_resource_exhausted`, `local_model_not_loaded`, `local_inference_error`
- Cross-cutting errors: `timeout`, `cancelled`, `secret_unavailable`, `network_denied`, `input_invalid`, `transport_error`, `stream_error`

The taxonomy does not depend on HTTP status codes. `classifyInferenceError` accepts an optional `http_status_hint` but never requires it.

## Stream frame lifecycle

```typescript
import { InferenceStreamLifecycle } from "@plurora/inference-capability";

const lifecycle = new InferenceStreamLifecycle("op_001", "str_001");
const start = lifecycle.start({ capability_id: "inference/generate" });
const chunk1 = lifecycle.chunk({ text_delta: "Once upon" });
const chunk2 = lifecycle.chunk({ text_delta: " a time…" });
const end = lifecycle.end();
// Calling lifecycle.chunk() after terminal state throws
```

## Provider capability manifest

Providers declare their supported operation kinds, modalities, transport kinds, runtime kind, and resource hints:

```typescript
import { createProviderCapabilityManifest } from "@plurora/inference-capability";

// Cloud API provider
const cloudManifest = createProviderCapabilityManifest({
  provider_id: "official/model-provider-lab",
  label: "Cloud API Model Provider",
  operation_kinds: ["generate", "embed"],
  transport_kinds: ["http"],
  runtime_kind: "cloud_api",
  streaming_supported: true,
  secrets_required: true,
  network_required: true,
});

// Local process provider
const localManifest = createProviderCapabilityManifest({
  provider_id: "official/inference-local-lab",
  label: "Local Process Inference Provider",
  operation_kinds: ["generate"],
  transport_kinds: ["local_process", "in_memory"],
  runtime_kind: "gpu_local",
  streaming_supported: true,
  secrets_required: false,
  network_required: false,
});
```

Manifest validation checks:
- Empty `operation_kinds` emits a warning
- `network_required=true` without a network-capable transport emits a warning
- Raw secrets in metadata throw

## Relationship to existing SDKs

- `sdk/typescript/model-provider-adapter`: Cloud API adapter SDK handling provider-specific request normalization and stream parsing. It uses URL/header/HTTP internally, but those are adapter-package-internal details.
- `sdk/typescript/secure-execution`: Secret ref, network declaration, outbound audit, and generic stream frame helpers.
- `sdk/typescript/inference-capability` (this SDK): Transport-neutral inference contract that does not depend on the HTTP/cloud-specific fields of the above SDKs.

## What this does not do

- No kernel model/prompt/chat/embedding methods.
- No unified chat schema (no `system`/`user`/`assistant`).
- No API gateway / LiteLLM / OneAPI relay.
- No user balances, billing, channel admin.
- No model downloader, weight cache, GPU scheduler.
- No OpenAI-compatible request as a public platform protocol.

## Further reading

- `docs/guides/MODEL_PROVIDER_INTEGRATION.md` — cloud API integration guide
- `docs/roadmap/NEXT_STEPS.en.md` — current priorities, acceptance gates, and explicit deferrals
- `docs/guides/AGENT_PACKAGE_AUTHORING.md` — agent-like package authoring guide
- `docs/architecture/CAPABILITY_PACKAGE.md` — capability package contract
- `sdk/typescript/secure-execution/index.ts` — secure execution helpers
