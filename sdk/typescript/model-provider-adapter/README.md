# Plurora Model Provider Adapter SDK

纯 TypeScript 的 model provider adapter，让能力包能描述、验证、归一化 OpenAI / Anthropic / Gemini / OpenAI-compatible / OpenRouter / DeepSeek / xAI / Fireworks 的 API 差异，而不依赖私有 runtime、不做真实 HTTP 调用、不做计费/中转、不新增 `platform.model.*`。

**这是 SDK adapter，不是 provider package。** 它不出网、不做计费或中转站、不新增 platform.model 语义；只负责 provider profile 描述、请求归一化、错误分类、流事件解析。

## 参考

- [`integrations/model-providers/provider-matrix.yaml`](../../../integrations/model-providers/provider-matrix.yaml) — provider API 矩阵
- [`integrations/model-providers/README.md`](../../../integrations/model-providers/README.md) — 流协议族说明与错误分类

## 用法

```ts
import {
  // 类型
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

  // 函数
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

### 列出 provider family

```ts
const families = listProviderFamilies();
// ["openai", "anthropic", "gemini", "openai_compatible", "openrouter", "deepseek", "xai", "fireworks"]

const meta = describeProviderFamily("openai");
// meta.requestDialects, meta.streamFamilies, meta.usageModes, …
```

### 验证 provider profile

```ts
const diagnostics = validateProviderProfile({
  family: "openai",
  model: "gpt-4o",
  credential: "secret_ref:env:OPENAI_API_KEY",
  baseUrl: "https://api.openai.com",
});

// diagnostics 中的 error 级别条目必须修正
// raw API key 会被拒绝：
validateProviderProfile({
  family: "openai",
  model: "gpt-4o",
  credential: "rawSecretPlaceholder1234567890ABCDEF",
});
// → error: "Raw API key detected in credential. Use a secret_ref: or host: reference."

// 非 HTTPS baseUrl 被拒绝
// OpenRouter 缺少 HTTP-Referer 时有 warning
// Anthropic 缺少 anthropic-version 时有 warning
// Gemini 有 x-goog-api-key 提示
```

### 归一化模型请求

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
// normalized.bodyShape — 包含 model, messages, stream, max_tokens, temperature
```

Anthropic 归一化自动把 system 消息提升到顶层：

```ts
const normalized = normalizeModelRequest(
  { family: "anthropic", model: "claude-3-5-sonnet-20241022", credential: "secret_ref:env:KEY" },
  { messages: [{ role: "system", content: "You are helpful." }, { role: "user", content: "Hi" }] },
);
// normalized.bodyShape.system === "You are helpful."
// normalized.bodyShape.messages 不含 system
```

### 错误归一化

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

### 流事件归一化

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

### 使用量估算

```ts
const usage = estimateUsage({
  family: "openai",
  model: "gpt-4o",
  content: "Hello",
  usage: { prompt_tokens: 10, completion_tokens: 20, total_tokens: 30 },
});
// usage.prompt_tokens === 10

// 也可以从流事件数组聚合
const events: NormalizedStreamEvent[] = [
  { kind: "text_delta", text: "Hello" },
  { kind: "usage_final", usage: { prompt_tokens: 5, completion_tokens: 15 } },
];
const usage = estimateUsage(events);
```

### 自测

```ts
const result = runModelProviderAdapterSelfTest();
if (!result.ok) {
  for (const d of result.diagnostics) console.error(d);
  throw new Error("Self-test failed");
}
```

## API 参考

| 导出 | 类型 | 说明 |
|---|---|---|
| `ProviderFamily` | type | 支持的家庭联合类型 |
| `RequestDialect` | type | 请求方言联合类型 |
| `StreamFamily` | type | 流协议族联合类型 |
| `ToolMode` | type | 工具模式联合类型 |
| `UsageMode` | type | 使用量模式联合类型 |
| `ProviderProfile` | interface | provider 配置文件 |
| `CanonicalModelMessage` | interface | 归一化模型消息 |
| `CanonicalModelRequest` | interface | 归一化模型请求 |
| `CanonicalModelResponse` | interface | 归一化模型响应 |
| `NormalizedStreamEvent` | type | 归一化流事件联合类型 |
| `ProviderErrorKind` | type | 错误分类联合类型 |
| `ProviderUsage` | interface | 使用量指标 |
| `ProviderCost` | interface | 成本估算 |
| `ProviderDiagnostic` | interface | 诊断结果 |
| `NormalizedProviderRequest` | interface | provider 特定请求形状 |
| `ValidationDiagnostic` | interface | 验证诊断 |
| `ProviderErrorInput` | interface | 错误归一化输入 |
| `listProviderFamilies` | function | 列出所有支持的 family |
| `describeProviderFamily` | function | 描述一个 family |
| `validateProviderProfile` | function | 验证 profile，拒绝 raw secret |
| `normalizeModelRequest` | function | 归一化模型请求 |
| `normalizeProviderError` | function | 归一化 provider 错误 |
| `normalizeStreamEvent` | function | 归一化流事件 |
| `estimateUsage` | function | 估算/聚合使用量 |
| `runModelProviderAdapterSelfTest` | function | 纯 TS 自测 |

## 支持的 Provider Family

| Family | 请求方言 | 流族 | 备注 |
|---|---|---|---|
| `openai` | `openai_responses`, `openai_chat` | `semantic_sse`, `delta_sse` | Responses API 使用语义事件 |
| `anthropic` | `anthropic_messages` | `semantic_sse` | system 在顶层；需要 `anthropic-version` |
| `gemini` | `gemini_generate_content` | `typed_chunk_stream` | 不兼容 OpenAI；需要 `x-goog-api-key` |
| `openai_compatible` | `openai_chat` | `delta_sse` | 需自定义 baseUrl |
| `openrouter` | `stateless_responses`, `openai_chat` | `semantic_sse`, `delta_sse` | 无状态；流中可能有错误 |
| `deepseek` | `openai_chat` | `delta_sse` | 支持 `reasoning_effort` |
| `xai` | `openai_responses`, `openai_chat` | `semantic_sse`, `delta_sse` | 用 `max_completion_tokens` |
| `fireworks` | `openai_chat`, `fireworks_responses` | `delta_sse`, `semantic_sse` | 支持 MCP/rollout headers |

## 归一化流事件

| 事件 | 说明 |
|---|---|
| `text_delta` | 文本增量 |
| `reasoning_delta` | 推理/thinking 增量 |
| `tool_call_started` | 工具调用开始 |
| `tool_args_delta` | 工具参数增量 |
| `tool_call_done` | 工具调用完成 |
| `citation` | 引用/来源 |
| `usage_final` | 最终使用量 |
| `error` | 流中错误 |
| `done` | 流终止 |
| `heartbeat` | 心跳/保活 |

## 错误分类

| ProviderErrorKind | HTTP | 说明 |
|---|---|---|
| `bad_request` | 400 | 请求格式错误 |
| `authentication` | 401 | 认证失败 |
| `billing` | 402 | 账户/余额问题 |
| `permission` | 403 | 权限不足 |
| `not_found` | 404 | 资源不存在 |
| `timeout` | 408/504 | 超时 |
| `rate_limit` | 429 | 限流 |
| `tool_schema` | 422 | 工具参数校验失败 |
| `overloaded` | 502/503/529 | 上游过载 |
| `upstream_malformed` | 500 | 上游内部错误 |
| `stream_error` | — | 流错误 |
| `network_denied` | — | 网络策略拒绝 |
| `secret_unavailable` | — | secret 不可用 |
| `unknown` | — | 未知错误 |

## 设计约束

- **不 import** 任何私有 runtime 模块
- **无外部依赖**，纯 TypeScript
- **不出网**，不做真实 HTTP 调用
- **不做计费**，不做中转站
- **不新增** `platform.model.*`、`platform.prompt.*`、`platform.chat.*`
- **不使用** `any` 类型，尽量 `unknown` + type guards
- **不硬编码** secrets，只接受 `secret_ref:*` 或 `host:*` 引用
- Raw API key 被 `validateProviderProfile` 拒绝
