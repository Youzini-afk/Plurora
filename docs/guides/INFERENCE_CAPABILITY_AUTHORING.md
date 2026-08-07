# Inference Capability 包创作指南

> [English](./INFERENCE_CAPABILITY_AUTHORING.en.md) · [中文](./INFERENCE_CAPABILITY_AUTHORING.md)

本指南记录 Plurora 的推理能力包创作方式。Plurora 可以优先支持 API，但不把平台协议做成某个 API 的形状。云端 API adapter 只是其中一类 provider。本地进程、内存计算和 IPC 管道同样可以承载推理。

## 核心立场

1. 推理是普通能力，不是内核原语。没有 `platform.model.*`、`platform.prompt.*`、`platform.chat.*` 或 `platform.embedding.*`。
2. 请求信封与传输无关。它不包含 URL、HTTP header、status code 或 OpenAI messages 字段。
3. Cloud adapter 是一类 provider，不是平台模型抽象。`plurora/model-provider-lab` 是普通 cloud API adapter lab，不代表 Ygg 的模型世界观。
4. `local_process`、`in_memory`、`ipc`、`websocket` 与 `http` 平权。
5. Secrets 用 `secret_ref`。Raw secret 在任何字段都不被接受。

## SDK 位置

`sdk/typescript/inference-capability/` 提供：

- `InferenceRequest` — 与传输无关的推理请求信封
- `InferenceResponse` — 与传输无关的推理响应信封
- `InferenceStreamFrame` — canonical 流式帧
- `InferenceError` — 与传输无关的错误分类
- `ProviderCapabilityManifest` — provider 能力声明
- Helper 函数 — `createInferenceRequest`、`classifyInferenceError`、`InferenceStreamLifecycle`、`createProviderCapabilityManifest` 等

## 请求信封

```typescript
import { createInferenceRequest } from "@plurora/inference-capability";

// 本地进程推理请求——不需要 URL、header 或 status code
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

关键约束：
- `input_refs` 是 opaque artifact 引用，不是 URL。
- `input_payload` 是 opaque payload 描述，不是 message 数组，也不包含 `system`/`user`/`assistant` 字段。
- `secret_refs` 只接受 `secret_ref:*` 或 `host:*`；raw secret 被拒绝。
- `transport_kind` 是语义提示（`http`/`local_process`/`in_memory`/`ipc`/`websocket`/`remote`/`custom`），不是 URL。

## 错误分类

错误分类覆盖：

- Cloud 错误：`authentication`、`permission`、`billing`、`rate_limit`、`provider_overloaded`、`provider_unavailable`、`bad_request`、`not_found`
- Local/resource 错误：`local_process_failed`、`local_process_timeout`、`local_resource_exhausted`、`local_model_not_loaded`、`local_inference_error`
- 跨领域错误：`timeout`、`cancelled`、`secret_unavailable`、`network_denied`、`input_invalid`、`transport_error`、`stream_error`

它不依赖 HTTP status code。`classifyInferenceError` 接受可选的 `http_status_hint`，但不要求它。

## Stream Frame 生命周期

```typescript
import { InferenceStreamLifecycle } from "@plurora/inference-capability";

const lifecycle = new InferenceStreamLifecycle("op_001", "str_001");
const start = lifecycle.start({ capability_id: "inference/generate" });
const chunk1 = lifecycle.chunk({ text_delta: "Once upon" });
const chunk2 = lifecycle.chunk({ text_delta: " a time…" });
const end = lifecycle.end();
// 终态后调用 lifecycle.chunk() 会 throw
```

## Provider Capability Manifest

Provider 声明自己支持的 operation kinds、modalities、transport kinds、runtime kind 和 resource hints：

```typescript
import { createProviderCapabilityManifest } from "@plurora/inference-capability";

// Cloud API provider
const cloudManifest = createProviderCapabilityManifest({
  provider_id: "plurora/model-provider-lab",
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
  provider_id: "plurora/inference-local-lab",
  label: "Local Process Inference Provider",
  operation_kinds: ["generate"],
  transport_kinds: ["local_process", "in_memory"],
  runtime_kind: "gpu_local",
  streaming_supported: true,
  secrets_required: false,
  network_required: false,
});
```

Manifest 辅助检测：
- 空 `operation_kinds` 发出 warning
- `network_required=true` 但无可联网 transport 时发出 warning
- Metadata 含 raw secret 会 throw

## 与现有 SDK 的关系

- `sdk/typescript/model-provider-adapter`：cloud API adapter SDK，处理 provider-specific 请求归一化和流式解析。它内部使用 URL/header/HTTP，但这些是 adapter 包的内部细节。
- `sdk/typescript/secure-execution`：secret ref、network declaration、outbound audit 和 stream frame 通用辅助。
- `sdk/typescript/inference-capability`（本 SDK）：与传输无关的推理契约，不依赖上述 SDK 的 HTTP 或 cloud-specific 字段。

## 不做什么

- 不做 kernel model/prompt/chat/embedding 方法。
- 不做统一 chat schema（no `system`/`user`/`assistant`）。
- 不做 API gateway / LiteLLM / OneAPI 中转站。
- 不做用户余额、计费、渠道后台。
- 不做模型下载器、权重缓存、GPU 调度。
- 不把 OpenAI-compatible request 作为平台公共协议。

## 延伸阅读

- `docs/guides/MODEL_PROVIDER_INTEGRATION.md` — cloud API 接入指南
- `docs/roadmap/NEXT_STEPS.md` — 当前优先级、验收门槛与明确延后项
- `docs/guides/AGENT_PACKAGE_AUTHORING.md` — agent-like 包创作指南
- `docs/architecture/CAPABILITY_PACKAGE.md` — 能力包契约
- `sdk/typescript/secure-execution/index.ts` — 安全执行辅助
