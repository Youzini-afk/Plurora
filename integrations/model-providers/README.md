# Model providers integration ledger

> [English](./README.en.md) · [中文](./README.md)
>
> 中文默认说明。本文件是 model providers 集成的唯一主台账；它合并了原 stream compatibility、error taxonomy、new-api ledger 与 TavernHeadless ledger 的事实。

## Scope

本台账把模型 provider 接入视为 cloud provider 级别的普通能力包复杂度样本。目标是帮助 Plurora 的 `official/model-provider-lab` 等普通能力包安全接入真实 provider API，而不是构建中转站、计费系统、渠道后台或平台模型网关。

模型 provider 不是 kernel ontology。Provider profile、模型列表、prompt/messages schema、usage/cost 和错误映射都属于 package output、diagnostics 或 outbound audit metadata。真实出网必须走 host-enforced outbound boundary 或等价 fake/local executor；默认 conformance 使用 fake executor/local mock，不依赖真实 API key 或外网。手动真实调用必须 opt-in，使用 `secret_ref`、network allowlist 和 redacted audit，并且不作为 CI/release gate。

## Provider matrix

| Provider family | 接入要点 |
|---|---|
| OpenAI | 同时覆盖 Chat Completions 与 Responses；Chat 使用 delta SSE，Responses 使用 semantic SSE；request normalization 必须保留 endpoint/dialect 差异。 |
| Anthropic | 独立 Messages dialect；`x-api-key` secret header 与 `anthropic-version` safe static header 分离；stream 是 `message_start` / `content_block_delta` / `message_delta` 等 semantic SSE。 |
| Gemini | 独立 `generateContent` / `streamGenerateContent` dialect；stream 是 typed `GenerateContentResponse` chunk；finish reason、safety/block reason 和 usage metadata 需要单独映射。 |
| OpenAI-compatible | 是 adapter family，不是唯一模型世界观；必须要求明确 `base_url`，拒绝缺失或 HTTP base URL，并通过 provider preset 处理 path/header quirks。 |
| OpenRouter | OpenAI-style，但需要 safe static headers（`HTTP-Referer`、`X-Title`）和 chat/responses 形态；HTTP 200 后的 mid-stream error 需要映射为 error frame。 |
| DeepSeek | OpenAI-style chat；reasoning stream 可出现 `reasoning_content` 与 cache usage，需要转成 reasoning/progress frames；适合 SSE normalization canary。 |
| xAI | OpenAI-style chat/responses；reasoning/usage metadata 需要 sanitization；Authorization bearer 仍走 secret header。 |
| Fireworks | OpenAI-style chat/responses；endpoint 常见为 `/inference/v1/chat/completions`；perf/usage metadata 需要 sanitization。 |

详表仍保留在 [`provider-matrix.yaml`](./provider-matrix.yaml)，用于 provider/request/stream/tool/usage/error 差异的机器可读审阅。

## Stream compatibility

不要把所有模型流都抽象成 OpenAI Chat Completions 的 `choices[].delta`。Provider adapter 应把不同上游流解析成 package-owned normalized stream events，再由 host streaming substrate 包装为 content-free `StreamFrameEnvelope`。

主要流族：

- **Delta SSE**：OpenAI Chat Completions、OpenAI-compatible、DeepSeek、xAI Chat、Fireworks Chat、OpenRouter Chat。常见 `data: {...}` lines、`data: [DONE]` terminal marker、文本在 `choices[].delta.content`，tool call arguments 可能分片，usage 可能只在最终 chunk 或非 streaming response 中出现。
- **Semantic SSE**：OpenAI Responses、Anthropic Messages、OpenRouter Responses。上游事件带显式语义，例如 `response.output_text.delta`、`message_start`、`content_block_delta`、`message_delta`；tool call start/args/done 不是简单文本 delta；error 可能作为 stream event 出现。
- **Typed chunk stream**：Gemini `streamGenerateContent`。Chunk 是 `GenerateContentResponse` 增量对象，文本在 `candidates[].content.parts[].text`，finish reason、safety/block reason、usage metadata 需要单独映射。
- **NDJSON / raw**：outbound substrate 支持 NDJSON 与 raw frames；adapter 仍应把 provider-specific bytes 转成 package-owned normalized events 后再进入通用 stream lifecycle。

建议 SDK/package 层归一化事件包括 `text_delta`、`reasoning_delta`、`tool_call_started`、`tool_args_delta`、`tool_call_done`、`citation`、`usage_final`、`error`、`done`、`heartbeat`。这些是 provider package output/trace 语义，不是 kernel 语义。必须覆盖 mid-stream provider error、缺失 terminal marker、heartbeat/keepalive comments、tool argument JSON fragment、usage only in final chunk、client cancel 后不能继续 append chunk、timeout terminal、per-frame redaction state。

## Error taxonomy

Plurora 不在 kernel 中定义模型错误。Provider adapters 应在 package output 与 diagnostics 中把上游错误映射到稳定分类，同时保留 provider 原始 code/message/request id 的 redacted metadata。

建议分类：`bad_request`、`authentication`、`permission`、`billing`、`rate_limit`、`not_found`、`timeout`、`overloaded`、`tool_schema`、`stream_error`、`upstream_malformed`、`network_denied`、`secret_unavailable`、`unknown`。

建议附加字段：`retryable: boolean`、`stage: preflight | request | stream | postprocess`、`provider_family`、`provider_code`、`upstream_request_id`、`redaction_state`。

映射例子：Anthropic `invalid_request_error`、Gemini `INVALID_ARGUMENT`、DeepSeek `400/422` → `bad_request`；OpenAI/Anthropic/OpenRouter/DeepSeek/xAI/Fireworks `401` → `authentication`；Anthropic/OpenRouter/DeepSeek/Fireworks `402` → `billing`；Gemini `RESOURCE_EXHAUSTED` 或 HTTP `429` → `rate_limit`；Anthropic `529 overloaded_error`、Gemini `UNAVAILABLE`、HTTP `502/503` → `overloaded`；Anthropic `timeout_error`、Gemini `DEADLINE_EXCEEDED`、HTTP `408/504` → `timeout`；strict schema/tool argument validation → `tool_schema`；SSE error event 或 malformed stream chunk → `stream_error` / `upstream_malformed`。

非目标：不做用户余额或账单系统；不在 kernel 中新增 provider error enum；不持久化 raw request/response body。

## Reference projects

### new-api

[new-api](https://github.com/Youzini-afk/new-api) 是 provider 接入复杂度样本，不是 Plurora 采用的实现。可吸收的是 adapter 分层：provider adapter 负责 URL、headers、request conversion、response conversion 和 stream handling。Runtime context bus、request conversion chain、stream scanner、model mapping、header/base URL quirks、usage metadata、error wrapping 都提示 Plurora 需要可观测的 transport layer、canonical model layer 与 provider quirk layer。

Plurora 不吸收用户余额、充值、倍率、pre-consume/refund、subscription、admin/channel UI、自动禁用/启用 channel 的运营治理、平台代理 API key 或统一 relay endpoint，也不把 channel/provider ontology 放进 kernel。Usage/cost 只作为 package output/audit metadata；base URL 和 redirect 必须走 host policy 检查。

### TavernHeadless

[TavernHeadless](https://github.com/Youzini-afk/TavernHeadless) 是 provider/profile 经验参考，不是 Plurora 采用的实现。它提示 provider profile 应是可激活、可 fallback、可 masking 的 package 配置对象，而不是 kernel state；routing 按 provider type 选择 adapter；OpenAI/DeepSeek/xAI/openai-compatible 可共享 OpenAI-style 工厂，Anthropic/Gemini 需要独立 adapter。

Request normalization 应留在 package/SDK 层：generation params、history normalization、assistant prefill、token budget 都是产品语义。Streaming 有 provider stream parser 与 UI reducer/tool-event grouper 两层；kernel 只需要通用 stream frame lifecycle。Discovery/hello probe、model discovery、slot routing、session/global fallback、active profile resolution、tool event grouping 和 replay safety hints 都不进入 kernel。

## Boundaries

模型接入是普通能力包能力，例如 `official/model-provider-lab`、`model-connector-lab` 与 `model-routing-lab`，不是 kernel ontology。Plurora 不新增 `kernel.v1.model.*`、`kernel.v1.prompt.*`、`kernel.v1.chat.*`、`kernel.v1.embedding.*`，不托管用户金额或平台代理 API key，不做中转站，不提供 channel admin，也不给官方 provider 包任何隐式 network、secret、routing 或 UI 特权。
