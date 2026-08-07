# Plurora TypeScript Agent Adapter SDK

纯 TypeScript 的 agent adapter，让 capability package 能以 tool 形式对外暴露，通过公共协议调用、生成 trace / proposal，而不依赖私有 runtime、`pi-coding-agent` 或任何外部 agent 框架。

**这是 SDK adapter，不是 agent framework。** 它不提供模型、提示词、记忆、turn 循环；只负责 capability ↔ tool 映射、协议调用、可观测性产物构建。

## 用法

```ts
import {
  createPluroraAgentAdapter,
  capabilityToTool,
  createCapabilityTool,
  invokeCapabilityTool,
  streamCapabilityTool,
  createTraceEvent,
  createProposalDraft,
  diagnosePermissions,
  diagnoseProvider,
  blockRawSecrets,
  runPluroraAgentAdapterSelfTest,
} from "./index";
```

### 创建 adapter

```ts
const adapter = createPluroraAgentAdapter({
  protocolClient: myProtocolClient,  // 实现 ProtocolClient.call(request)
  packageId: "my/agent-package",
  principal: "user:alice",           // 可选
});
```

### Capability → Tool 映射

```ts
const tool = capabilityToTool({
  capability_id: "search/query",
  name: "Search",
  description: "搜索文档",
  streamable: true,
  provider_package_ids: ["search/impl"],
  required_permissions: ["network:outbound"],
});
// tool.name === "Search", tool.streamable === true
```

### Tool 调用

```ts
const result = await invokeCapabilityTool(protocolClient, {
  tool,
  input: { q: "hello" },
});
// result.ok === true 时，result.output 为返回值
```

多 provider 时必须显式指定：

```ts
const result = await invokeCapabilityTool(protocolClient, {
  tool: ambiguousTool,   // provider_package_ids: ["pkg/a", "pkg/b"]
  input: {},
  provider_package_id: "pkg/a",  // 必填
});
```

### Stream

```ts
const { request, frames } = streamCapabilityTool({
  tool: streamableTool,
  input: { prompt: "讲个故事" },
});

// frames 是 faux 流帧构造器（不依赖真实 SSE）
frames.start({ init: true });
frames.chunk({ text: "从前" });
frames.chunk({ text: "有一座山" });
frames.end();
```

### Trace Event

```ts
const trace = createTraceEvent("tool_call", "my/pkg", { tool: "search" }, {
  capability_id: "search/query",
  call_id: "call-1",
});
```

### Proposal Draft

```ts
const draft = createProposalDraft(
  "my/pkg",
  "搜索文档",
  "根据用户查询搜索相关文档",
  [{ capability_id: "search/query", input: { q: "hello" } }],
  { risk_note: "只读操作", requires_confirmation: false },
);
```

### Provider / Permission 诊断

```ts
const provDiag = diagnoseProvider(tool);
if (!provDiag.unambiguous) {
  console.warn(provDiag.ambiguity_reason);
}

const permDiag = diagnosePermissions(tool);
if (!permDiag.satisfied) {
  console.warn("缺少权限:", permDiag.missing);
}
```

### 原始秘密检测

```ts
const scan = blockRawSecrets({ api_key: "sk-abc..." });
if (scan.has_raw_secrets) {
  throw new Error("payload 包含原始秘密: " + scan.flagged_fields.join(", "));
}

// secret_ref 形式安全通过
blockRawSecrets({ api_key: "secret_ref:env:MY_KEY" }).has_raw_secrets; // false
```

### 自测

```ts
const results = runPluroraAgentAdapterSelfTest();
const failed = results.filter(r => !r.passed);
if (failed.length > 0) {
  for (const f of failed) console.error(`FAIL: ${f.name} — ${f.detail}`);
}
```

## API 参考

| 导出 | 类型 | 说明 |
|---|---|---|
| `ProtocolClient` | interface | `call(request)` 协议客户端接口 |
| `ProtocolRequest` / `ProtocolResponse` | interface | 协议请求 / 响应 |
| `CapabilityDescriptor` | interface | 能力描述符 |
| `CapabilityTool` | interface | 能力对应的 tool 表示 |
| `ToolCall` / `ToolResult` | interface | 调用请求 / 结果 |
| `AgentTraceEvent` | interface | Agent trace 事件 |
| `AgentProposalDraft` | interface | Proposal 草稿 |
| `StreamRequest` / `StreamFrameAdapter` / `StreamAdapterFrame` | interface | 流式请求 / 帧构造器 |
| `PermissionDiagnostics` / `ProviderDiagnostics` / `RawSecretScanResult` | interface | 诊断结果 |
| `createPluroraAgentAdapter` | function | 创建 adapter |
| `capabilityToTool` | function | 描述符 → tool |
| `createCapabilityTool` | function | 从字段创建 tool |
| `invokeCapabilityTool` | function | 通过协议调用 tool |
| `streamCapabilityTool` | function | 流式调用（返回请求 + 帧构造器） |
| `createTraceEvent` | function | 创建 trace 事件 |
| `createProposalDraft` | function | 创建 proposal 草稿 |
| `diagnosePermissions` | function | 权限诊断 |
| `diagnoseProvider` | function | Provider 诊断 |
| `blockRawSecrets` | function | 原始秘密扫描 |
| `runPluroraAgentAdapterSelfTest` | function | 纯 TS 自测 |

## 设计约束

- **不 import** `clients/web`、`clients/web/private` 或任何 runtime private 模块
- **不依赖** `pi-coding-agent` 或外部 agent framework
- **无外部依赖**，纯 TypeScript
- **不引入** `platform.agent.*`、`platform.model.*`、`platform.prompt.*`、`platform.memory.*` 方法
- **不使用** `any` 类型，尽量 `unknown` 安全
- Provider 多义时**拒绝调用**，除非显式指定 `provider_package_id`
- 原始秘密**阻断**：复用 `secret_ref` 思路但不 import private runtime
