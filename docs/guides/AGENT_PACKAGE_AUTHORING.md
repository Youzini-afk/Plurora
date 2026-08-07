# Agent 能力包创作指南

> [English](./AGENT_PACKAGE_AUTHORING.en.md) · [中文](./AGENT_PACKAGE_AUTHORING.md)

本文说明如何在 Plurora 当前 Contract V1 中，用普通 Package / Component 实现类 agent 能力。Agent 的共享语义可以由可选 Protocol / Profile 定义，具体运行状态由 Component 或 Product 拥有；它不是宪法基底服务。

## 你应该使用什么

- 用普通清单声明类 agent 能力。
- 用 `capability.invoke` 或 `capability.stream` 启动运行。
- 用 `capability.cancel` 取消流式调用。
- 用 `platform.proposal.*` 产出、审批和应用变更。
- 用当前 Package writer namespace 的事件、Artifact 或流式帧记录追踪。
- 用 surface contribution 暴露 `assistant_action`、`forge_panel` 或 `home_card`。
- 用 `secret_ref` 而不是 raw secrets。
- 用显式 `provider_package_id` 处理 provider 冲突。

## 你不应该使用什么

- 不新增或依赖 `platform.agent.*`、`platform.model.*`、`platform.prompt.*`、`platform.memory.*`、`platform.turn.*`。
- 不把 agent 直接写入内核状态。
- 不让 agent 直接修改可信资产、projection 或会话状态；先生成提案。
- 不通过工具桥借用其他包的权限。
- 不自动选择 first-party provider。
- 不在追踪、提案、事件、审计或错误中保存 raw secrets。
- 不默认提供 bash/read/write/edit 这类 coding-agent 工具。

## 从模板开始

生成本地可重放的 agent runtime 包：

```bash
cargo run -p plurora-cli -- init-package /tmp/plurora-agent \
  --id example/agent-runtime \
  --entry subprocess \
  --language typescript \
  --template agent-runtime
```

模板会生成：

- `example/agent-runtime/run`：流式运行能力。
- `example/agent-runtime/explain-run`：解释运行追踪。
- `example/agent-runtime/draft-proposal`：生成需审批的提案草案。
- `example/agent-runtime/echo`：本地检查兼容能力。
- `assistant_action` 和 `forge_panel` surfaces。
- 默认不出网、不调用真实模型，也不接收 raw secret。

验证生成包：

```bash
cargo run -p plurora-cli -- package check /tmp/plurora-agent/manifest.yaml
cargo run -p plurora-cli -- package conformance /tmp/plurora-agent/manifest.yaml
```

## 使用 `agent-adapter` SDK

`sdk/typescript/agent-adapter` 是一层薄适配器，不是完整 agent 框架。它用于：

- 把 Ygg 能力描述符映射为 pi 风格工具描述符。
- 构造 `capability.invoke` / `capability.stream` 请求载荷。
- 生成包拥有的追踪事件载荷。
- 生成需审批的提案草案载荷。
- 诊断 provider 歧义、权限预览和 raw secret 阻断。

示意：

```ts
import { createYggAgentAdapter } from "../../sdk/typescript/agent-adapter/index.js";

const adapter = createYggAgentAdapter({
  protocolClient,
  packageId: "example/agent-runtime",
});

const tool = adapter.createCapabilityTool({
  capability_id: "example/tool/plan",
  provider_package_ids: ["example/tool"],
  streaming: false,
});

const plan = await adapter.invokeCapabilityTool(tool, {
  input: { topic: "safe plan" },
  provider_package_id: "example/tool",
});
```

如果多个 provider 暴露同一能力，必须显式选择 `provider_package_id`。不要自动选择第一个 provider，也不要偏向 `plurora/*`。

## 官方参考包

`plurora/pi-agent-runtime-lab` 是普通参考包。它提供本地可重放能力：

- run plan
- trace summary
- proposal draft
- echo

它没有官方特权。它不是真实 agent runtime，也不做模型推理。

`plurora/capability-tool-bridge-lab` 也是普通包。它只生成工具发现、权限预览和调用计划。它不会代替 agent 调用目标能力，以避免 confused deputy。

## 第三方替换证明

参考：

- `examples/packages/thirdparty-agent-runtime/manifest.yaml`
- `examples/compositions/agent-runtime-replacement/composition.yaml`

这个例子证明第三方 agent runtime 可以提供同等 surface、能力、提案和追踪形状。official 包只是 `replacement_candidate`，没有优先级。

验证：

```bash
cargo run -p plurora-cli -- package check examples/packages/thirdparty-agent-runtime/manifest.yaml
cargo run -p plurora-cli -- composition check examples/compositions/agent-runtime-replacement/composition.yaml
```

## UI 观察

Forge 的 Agent Observability 区块和 Assist Drawer 的 Agent Readiness 面板只读取公开协议数据：surface contribution、能力、事件和提案。它们不硬编码 official 包，也不启动真实 agent 或模型。

## 与 pi 的关系

[pi](https://github.com/earendil-works/pi) 是参考来源：

- `pi-agent-core` 的事件、工具、gate 和队列思路可被普通包内部吸收。
- `pi-ai` 的 faux provider / stream shape 可作为未来模型包参考。
- `pi-coding-agent` 只作为产品和观测经验参考，不嵌入 Plurora。

更多边界见 [`../architecture/PI_INTEGRATION.md`](../architecture/PI_INTEGRATION.md) 和 [`../../integrations/pi/README.md`](../../integrations/pi/README.md)。
