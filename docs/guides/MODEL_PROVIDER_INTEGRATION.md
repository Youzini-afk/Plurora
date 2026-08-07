# Cloud Model Provider Integration

> [English](./MODEL_PROVIDER_INTEGRATION.en.md) · [中文](./MODEL_PROVIDER_INTEGRATION.md)

本指南记录 Plurora 的 cloud model provider adapter 接入方式。它不是中转站、计费系统、provider 后台或内核模型抽象。云模型接入必须作为普通能力包工作，并遵守同一套清单、权限、`secret_ref`、出站审计、流式/取消和检查边界。

## Scope：cloud adapter，不是平台抽象

`official/model-provider-lab` 是 cloud API adapter lab：

- 它不是 Plurora 的模型抽象。
- 它不是 LiteLLM / OneAPI compatible gateway。
- 它不是 provider marketplace、计费系统或渠道后台。
- 它没有内核特权；官方包和第三方包必须走同一套公开协议、权限、secret 和出站边界。
- OpenAI、Anthropic、Gemini、OpenRouter、DeepSeek、xAI、Fireworks 的 schema 是 adapter 内部细节，不是平台公共协议。
- `normalize_request` 是 cloud adapter 内部 request builder helper，不是 Plurora canonical inference request。

如果你要编写与传输无关的推理包，请看 [`INFERENCE_CAPABILITY_AUTHORING.md`](./INFERENCE_CAPABILITY_AUTHORING.md)。如果你要证明非 HTTP、本地或自托管 seam，请参考 `official/inference-local-lab`。

## 当前交付

当前交付包括：

- `integrations/model-providers/` 保存 provider research ledger、provider matrix、流式兼容笔记和错误分类。
- `sdk/typescript/model-provider-adapter` 提供纯 TypeScript cloud adapter，用于 provider profile、adapter-local request builder、错误分类和流式事件解析。它不出网、不做计费、不访问私有 runtime。
- `official/model-provider-lab` 是普通官方 cloud adapter 能力包，覆盖 OpenAI、Anthropic、Gemini、OpenAI-compatible、OpenRouter、DeepSeek、xAI、Fireworks 等 cloud provider family。
- `official/model-provider-lab` 能力包括：`list_supported_families`、`validate_profile`、`normalize_request`、`invoke`、`normalize_stream`、`explain_error`、`echo`。
- `invoke` 保留 fake/local provider adapter path。它产出 provider 形状的 response 和可审计 `outbound_request_shape`，用于默认检查和 adapter 形状验证。
- Host 侧已有 content-free `OutboundExecutor` boundary，默认拒绝。它包含 fake executor、loopback live HTTP executor 和 hostile 检查。这证明 request shape 可以走 host policy/audit 边界，但不声称 OS 级拦截子进程任意联网。
- `host.outbound.execute` 是公开出站协议，ordinary packages 和 official packages 必须走同一路径；package principal 来自 protocol context，不能 spoof 其他 package。
- `EnvSecretResolver` 支持 host-owned `secret_ref:env:NAME` allowlist；raw secret 只在 host 内部短暂存在，不进入 event、log、audit 或 response。
- `LiveHttpOutboundExecutor` 使用 `reqwest + rustls`，默认关闭。它只允许 HTTPS，redirect fail-closed，timeout 必须配置。response/audit 只保留脱敏形状。Loopback 检查用 `allow_insecure_loopback_for_tests=true`，不依赖公网。
- `secret_headers` 支持 host-side header 注入（例如 Authorization bearer、x-api-key、x-goog-api-key）；缺失/无效 secret fail-closed。`static_headers` 只允许少量非 secret provider/version/format headers（anthropic-version、content-type、accept、http-referer、x-title），并阻止 Authorization/x-api-key/Cookie 等 secret-bearing 或 host-owned headers。
- Live-call 检查覆盖 DeepSeek canary、OpenAI Chat/Responses、Anthropic Messages、Gemini generateContent、OpenRouter、xAI、Fireworks loopback shapes，以及缺失 secret fail-closed 与无 raw secret 泄漏。
- `normalize_stream` 将 delta SSE、semantic SSE、typed chunk stream 归一为 `StreamFrameEnvelope` 风格帧：`start`、`chunk`、`progress`、`end`、`error`、`cancelled`、`timeout`。它也覆盖常见 provider quirks。

## Provider families

| Family | Request dialects | Stream shape | 默认 host |
| --- | --- | --- | --- |
| `openai` | `openai_chat`, `openai_responses` | `delta_sse`, `semantic_sse` | `api.openai.com` |
| `anthropic` | `anthropic_messages` | `semantic_sse` | `api.anthropic.com` |
| `gemini` | `gemini_generate_content` | `typed_chunk_stream` | `generativelanguage.googleapis.com` |
| `openai_compatible` | `openai_chat` | `delta_sse` | 必须显式 `baseUrl` |
| `openrouter` | `openai_chat`, `stateless_responses` | `delta_sse`, `semantic_sse` | `openrouter.ai` |
| `deepseek` | `openai_chat` | `delta_sse` | `api.deepseek.com` |
| `xai` | `openai_chat`, `openai_responses` | `delta_sse`, `semantic_sse` | `api.x.ai` |
| `fireworks` | `openai_chat`, `fireworks_responses` | `delta_sse`, `semantic_sse` | `api.fireworks.ai` |

OpenAI-compatible 是 adapter family，不是 Plurora 的唯一模型世界观。Anthropic、Gemini 和 Responses 风格流都有不同结构，必须显式适配。

## Profile shape

示例 profile 位于 `examples/model-provider-profiles/`。所有示例使用 `secret_ref`，不包含真实 key。

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

规则：

- credential 必须是 `secret_ref:*`、`secretRef:*`、`secret-ref:*` 或 `host:*` reference。
- raw credential、raw-looking header、`api_key` 或 `secret` 字段会被拒绝。
- `invoke` 要求 HTTPS `baseUrl`；`openai_compatible` 没有默认 host，必须显式提供 HTTPS `baseUrl`。
- profile 可以携带 provider-specific `headers` 和 `extra`，但这些字段保持包语义，不进入内核 ontology。

## 常用能力

### `list_supported_families`

返回 provider family 的默认 base URL、request dialect、stream family、credential header 与注意事项。不会出网。

### `validate_profile`

检查 family、model、credential、base URL、headers，并返回：

- `valid`
- `diagnostics`
- `network_required`
- `secret_refs`
- `network_performed=false`
- `inference_performed=false`

### `normalize_request`

`normalize_request` 是 cloud adapter package 的内部 request-builder helper。它把 adapter-local input 转成 provider-specific request shape。它不是 Plurora 的 canonical inference request，也不应被第三方包当成平台统一 chat schema。与传输无关的推理契约位于 `sdk/typescript/inference-capability`。

示例：

- OpenAI Chat：`/v1/chat/completions`
- OpenAI Responses：`/v1/responses`
- Anthropic：`/v1/messages`
- Gemini：`/v1beta/models/{model}:generateContent`
- OpenRouter：`/chat/completions` 或 `/responses`
- DeepSeek：`/chat/completions`
- xAI：`/v1/chat/completions` 或 `/v1/responses`
- Fireworks：`/chat/completions` 或 `/responses`

### `invoke`

`official/model-provider-lab/invoke` 本身仍然是 fake/local adapter path。真实网络调用不通过官方包私有 runtime access；它必须由 ordinary package 使用公开 `host.outbound.execute`，由 host policy、secret resolver 和 outbound executor 控制。

输出必须保持：

```json
{
  "network_performed": false,
  "inference_performed": false,
  "executor_kind": "fake_local",
  "live_call_supported": false
}
```

这保留了 adapter 自测与默认检查的可重放性。同时，它避免官方 provider 包获得第三方包没有的私有出站特权。

### `host.outbound.execute`

普通能力包的 live HTTP path：

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

规则：

- caller package 来自 `ProtocolContext::Package`，不能由 params 伪造；`capability_id` 必须属于 caller package namespace。
- host 先解析 `secret_headers`，解析失败则 fail-closed，不发送请求。
- policy/audit request 与 executor request 的 package/capability/host/method/secret_refs 必须一致，否则 fail-closed。
- raw secret、Authorization value、request body 和 response body 不进审计；只记录脱敏形状、secret_refs、host、method、purpose、status、usage/cost/error metadata。
- 默认 runtime 仍是 deny-all；live executor 需要 host 显式 opt-in。

### `normalize_stream`

把 provider event/chunk 归一为 `StreamFrameEnvelope` 风格帧。Provider-specific 字段放进 `payload` 或 `metadata`：

- OpenAI Chat / OpenAI-compatible / OpenRouter / DeepSeek / xAI / Fireworks：delta SSE。
- OpenAI Responses / Anthropic：semantic SSE。
- Gemini：typed chunk stream；把 `GenerateContentResponse` 风格快照适配为 chunk/end/progress。

结构事件（Anthropic `message_start`、`content_block_start`、OpenRouter comment、Gemini 无新增文本的快照）进入 `progress`，不伪装成文本 chunk。Usage、finish_reason、safety ratings、perf metrics、generation id 等进入 `metadata` 或 end payload，不成为内核语义。

### `explain_error`

把 provider-specific code/status 归一为普通错误类别，例如：

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

这是 package-level 归一化，不是 `platform.model.error`。

## Manual live call boundary

默认检查不依赖公网；manual/live provider 请求必须满足：

1. provider package 声明最小网络权限；
2. caller 或 host 显式授权；
3. secret 通过 host resolver 解析，raw secret 不进入 event/log/audit；
4. request 走公开 `host.outbound.execute` 与 host outbound boundary；
5. audit 只记录 host、method、purpose、secret_refs、usage/cost/error metadata 和 redaction state；
6. 流式输出统一落到 content-free frame lifecycle；
7. cancel/timeout 不被 provider adapter 私自吞掉；
8. third-party provider package 可以替换官方 package，没有官方优先级。

可选真实 DeepSeek smoke path 只在同时满足以下条件时运行：

```bash
PLURORA_LIVE_MODEL_TESTS=1 DEEPSEEK_API_KEY=... cargo run -p plurora-cli -- conformance
```

默认 CI / 默认 conformance 不会访问公网。

## 与 `inference-capability` / `inference-local-lab` 的关系

- `sdk/typescript/inference-capability`：与传输无关的推理信封、流式帧、错误分类和 provider capability manifest helper；不要求 URL/header/status code/OpenAI messages。
- `official/inference-local-lab`：可重放的非 HTTP fake local provider proof；证明 inference seam 不依赖 HTTP、Bearer、JSON provider schema 或网络。
- `official/model-provider-lab`：cloud API adapter lab；用于现实云 API 接入，不定义平台抽象。

三者的依赖方向是：与传输无关的契约 → cloud/local adapter packages。内核不导入、不知道、不硬编码这些 provider 语义。

## 非目标

- 用户余额、充值、计费后台、倍率、渠道管理系统。
- 托管平台代理 key。
- `platform.model.*`、`platform.prompt.*`、`platform.chat.*`、`platform.embedding.*`。
- 把 OpenAI-compatible 当作唯一模型协议。
- 把 `normalize_request` 当作平台 canonical request。
- 让官方包绕过 manifest、permission、secret、network 或 audit 边界。

## 验证

```bash
cargo test --workspace
cargo run -p plurora-cli -- conformance
cargo run -p plurora-cli -- package check packages/official/model-provider-lab/manifest.yaml
tsc -p clients/web/tsconfig.json --noEmit
```

验证可覆盖 model-provider-lab、inference-local-lab、public `host.outbound.execute`、secret header injection、live loopback provider shapes、provider quirk fixtures、非 HTTP 推理 seam proof 和 outbound policy checks。
