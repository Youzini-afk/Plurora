# Model providers integration ledger

> [English](./README.en.md) · [中文](./README.md)
>
> This is the single master ledger for model providers integration. It merges the facts formerly held in the stream compatibility, error taxonomy, new-api ledger, and TavernHeadless ledger notes.

## Scope

This ledger treats model-provider integration as an ordinary capability-package problem with cloud-provider-level complexity. Its goal is to help ordinary Plurora packages such as `official/model-provider-lab` safely integrate real provider APIs, not to build a relay gateway, billing system, channel admin backend, or platform model proxy.

Model providers are not kernel ontology. Provider profiles, model catalogs, prompt/messages schemas, usage/cost, and error mapping belong in package output, diagnostics, or outbound audit metadata. Real egress must pass through a host-enforced outbound boundary or an equivalent fake/local executor. Default conformance uses fake executors/local mocks, not live API keys or external network. Manual live calls must be opt-in, use `secret_ref`, network allowlists, and redacted audit, and must not be CI/release gates.

## Provider matrix

| Provider family | Integration notes |
|---|---|
| OpenAI | Cover both Chat Completions and Responses; Chat uses delta SSE, Responses uses semantic SSE; request normalization must preserve endpoint/dialect differences. |
| Anthropic | Independent Messages dialect; separate `x-api-key` secret header from the `anthropic-version` safe static header; stream is semantic SSE such as `message_start` / `content_block_delta` / `message_delta`. |
| Gemini | Independent `generateContent` / `streamGenerateContent` dialect; stream is typed `GenerateContentResponse` chunks; finish reason, safety/block reason, and usage metadata need dedicated mapping. |
| OpenAI-compatible | An adapter family, not the whole model worldview; require explicit `base_url`, reject missing or HTTP base URLs, and use provider presets for path/header quirks. |
| OpenRouter | OpenAI-style, but needs safe static headers (`HTTP-Referer`, `X-Title`) and chat/responses shapes; mid-stream errors after HTTP 200 must become error frames. |
| DeepSeek | OpenAI-style chat; reasoning streams may include `reasoning_content` and cache usage, which should become reasoning/progress frames; useful as an SSE normalization canary. |
| xAI | OpenAI-style chat/responses; reasoning/usage metadata needs sanitization; Authorization bearer still flows through secret headers. |
| Fireworks | OpenAI-style chat/responses; common endpoint is `/inference/v1/chat/completions`; perf/usage metadata needs sanitization. |

The detailed matrix remains in [`provider-matrix.yaml`](./provider-matrix.yaml) for machine-reviewable provider/request/stream/tool/usage/error differences.

## Stream compatibility

Do not flatten every model stream into OpenAI Chat Completions `choices[].delta`. Provider adapters should parse different upstream streams into package-owned normalized stream events, then let the host streaming substrate wrap them as content-free `StreamFrameEnvelope` frames.

Main stream families:

- **Delta SSE**: OpenAI Chat Completions, OpenAI-compatible providers, DeepSeek, xAI Chat, Fireworks Chat, and OpenRouter Chat. Common traits are `data: {...}` SSE lines, a `data: [DONE]` terminal marker, text in `choices[].delta.content`, tool-call arguments split across deltas, and usage that may appear only in the final chunk or non-streaming response.
- **Semantic SSE**: OpenAI Responses, Anthropic Messages, and OpenRouter Responses. Upstream events carry explicit semantics such as `response.output_text.delta`, `message_start`, `content_block_delta`, and `message_delta`; tool-call start/args/done are not simple text deltas; errors may arrive as stream events.
- **Typed chunk stream**: Gemini `streamGenerateContent`. Chunks are incremental `GenerateContentResponse` objects, text lives in `candidates[].content.parts[].text`, and finish reason, safety/block reason, and usage metadata need separate mapping.
- **NDJSON / raw**: the outbound substrate supports NDJSON and raw frames; adapters should still convert provider-specific bytes into package-owned normalized events before entering the common stream lifecycle.

Recommended SDK/package normalized events include `text_delta`, `reasoning_delta`, `tool_call_started`, `tool_args_delta`, `tool_call_done`, `citation`, `usage_final`, `error`, `done`, and `heartbeat`. These are provider package output/trace semantics, not kernel semantics. Coverage must include mid-stream provider errors, missing terminal markers, heartbeat/keepalive comments, tool argument JSON fragments, usage only in the final chunk, no append after client cancel, terminal timeout, and per-frame redaction state.

## Error taxonomy

Plurora does not define model errors in the kernel. Provider adapters should map upstream errors into stable categories in package output and diagnostics while preserving redacted provider code/message/request-id metadata.

Recommended categories: `bad_request`, `authentication`, `permission`, `billing`, `rate_limit`, `not_found`, `timeout`, `overloaded`, `tool_schema`, `stream_error`, `upstream_malformed`, `network_denied`, `secret_unavailable`, and `unknown`.

Recommended fields: `retryable: boolean`, `stage: preflight | request | stream | postprocess`, `provider_family`, `provider_code`, `upstream_request_id`, and `redaction_state`.

Examples: Anthropic `invalid_request_error`, Gemini `INVALID_ARGUMENT`, and DeepSeek `400/422` → `bad_request`; OpenAI/Anthropic/OpenRouter/DeepSeek/xAI/Fireworks `401` → `authentication`; Anthropic/OpenRouter/DeepSeek/Fireworks `402` → `billing`; Gemini `RESOURCE_EXHAUSTED` or HTTP `429` → `rate_limit`; Anthropic `529 overloaded_error`, Gemini `UNAVAILABLE`, and HTTP `502/503` → `overloaded`; Anthropic `timeout_error`, Gemini `DEADLINE_EXCEEDED`, and HTTP `408/504` → `timeout`; strict schema/tool-argument validation → `tool_schema`; SSE error events or malformed stream chunks → `stream_error` / `upstream_malformed`.

Non-goals: no user balance or billing system; no provider error enum in the kernel; no raw request/response body persistence.

## Reference projects

### new-api

[new-api](https://github.com/Youzini-afk/new-api) is a provider-integration complexity sample, not an implementation adopted by Plurora. The absorbable lesson is adapter layering: provider adapters own URLs, headers, request conversion, response conversion, and stream handling. Runtime context buses, request conversion chains, stream scanners, model mapping, header/base-URL quirks, usage metadata, and error wrapping all show that Plurora needs observable transport, canonical model, and provider quirk layers.

Plurora does not absorb user balances, recharge, multipliers, pre-consume/refund, subscriptions, admin/channel UI, operational auto-disable/enable governance, hosted platform API keys, a unified relay endpoint, or channel/provider ontology in the kernel. Usage/cost is only package output/audit metadata; base URLs and redirects must pass host policy checks.

### TavernHeadless

[TavernHeadless](https://github.com/Youzini-afk/TavernHeadless) is a provider/profile reference, not an implementation adopted by Plurora. It shows that provider profiles should be activatable, fallback-capable, maskable package configuration objects rather than kernel state; routing should select adapters by provider type; OpenAI/DeepSeek/xAI/openai-compatible can share an OpenAI-style factory, while Anthropic/Gemini need independent adapters.

Request normalization should stay in the package/SDK layer: generation params, history normalization, assistant prefill, and token budget are product semantics. Streaming has two layers: provider stream parser and UI reducer/tool-event grouper; the kernel only needs a common stream-frame lifecycle. Discovery/hello probes, model discovery, slot routing, session/global fallback, active-profile resolution, tool event grouping, and replay-safety hints do not enter the kernel.

## Boundaries

Model integration is an ordinary capability-package capability, such as `official/model-provider-lab`, `model-connector-lab`, and `model-routing-lab`; it is not kernel ontology. Plurora does not add `platform.model.*`, `platform.prompt.*`, `platform.chat.*`, or `platform.embedding.*`, does not host user balances or a platform master API key, does not act as a relay, does not provide channel admin, and gives official provider packages no implicit network, secret, routing, or UI privilege.
