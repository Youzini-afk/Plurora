# pi 集成边界

> [English](./PI_INTEGRATION.en.md) · [中文](./PI_INTEGRATION.md)

本文固定 Plurora 如何借鉴或承载 [pi](https://github.com/earendil-works/pi) 一类 agent framework。pi 可以作为普通能力包的实现来源，也可以帮助定义 SDK adapter；它不是 Plurora 的内核、公开合同或产品壳。

## 核心立场

Plurora 要能托管、约束、观察和替换 agent 类 Component 与 Product，但平台本身不拥有 agent ontology。Run、step、tool、trace、prompt、model、memory 和 coding workflow 的共享含义属于可选 Protocol / Profile，具体状态与行为属于 Component 或 Product，不进入宪法基底。

Agent 基础设施必须复用现有公开原语：

- `capability.discover/describe` 发现可映射为 tool 的能力；
- `capability.invoke/stream/cancel` 执行、流式推进和取消；
- `platform.proposal.*` 或通用 Change workflow 承载受审查变更；
- `journal.event.*` 以当前 Package writer namespace 承载 trace、tool-call 和 run event；
- `platform.surface.contribution.*` 让 shell 发现 agent action、trace 或 review panel；
- capability handle、permission、`secret_ref`、network declaration、outbound audit 和 stream ownership 约束副作用。

没有 `platform.agent.*` 私有旁路，也没有因为某个 agent 包是官方维护就获得的额外权威。

## pi 分层吸收

| pi 层 | Plurora 处理方式 | 边界 |
|---|---|---|
| `pi-ai` | 作为 provider、streaming 和 tool-call adapter 的实现参考 | Provider 语义留在普通 inference/model 包；secret、网络和审计由 Host 边界执行。 |
| `pi-agent-core` | 可由 SDK 或能力包包装 | `AgentEvent`、tool adapter、steer/follow-up queue 等可以留在包内；message、system prompt、thinking level 不进入内核。 |
| `pi-coding-agent` | 作为完整产品和 workflow 的参考 | TUI、bash/read/write/edit、session format、skills 和 coding policy 不成为 Plurora 平台默认值。 |

更细的上游 ledger 见 [`../../integrations/pi/README.md`](../../integrations/pi/README.md)。

## 概念映射

| Agent 概念 | Plurora 公开原语 | 规则 |
|---|---|---|
| run / turn / step | Component capability call、stream 或协议拥有的状态 | 基底不新增 agent 生命周期。 |
| cancellation | `capability.cancel` | 只能取消调用者拥有的 invocation/stream。 |
| tool discovery | `capability.discover/describe` | Tool 是 capability 的 adapter view。 |
| tool execution | `capability.invoke/stream` | 保留 caller、provider、session、权限和 receipt。 |
| provider ambiguity | 显式 `provider_package_id` | 不自动偏向官方 provider。 |
| proposed mutation | `platform.proposal.*` / Change workflow | Agent 不直接修改受信状态。 |
| trace | writer-scoped event、stream frame 或 artifact | runtime 不解释 trace payload。 |
| working state | Component / Product event、object、projection 或 capability | 不新增 substrate agent state。 |
| model / prompt / memory | 可选 Protocol 与普通 Component | 可以组合和替换，不进入宪法基底。 |
| UI | surface contribution + public client | Shell 不读取 agent runtime 私有状态。 |

## 仓库中的普通组件

当前仓库通过普通 SDK、Component Package 和 integration fixture 实现并持续检查这条边界：

- `sdk/typescript/agent-adapter` 把 Ygg capability 映射为 pi-style tool；
- `sdk/typescript/agentic-forge` 提供包拥有的 run lifecycle、plan graph、working state 和 candidate helper；
- `official/pi-agent-runtime-lab` 提供默认不联网的参考 agent 包；
- `official/capability-tool-bridge-lab` 负责 capability discovery、permission preview、显式 provider 选择和受控调用；
- `official/agentic-forge-lab` 提供 scratch branch、candidate、compare、promote 和 replay；
- 第三方 replacement fixture 检查这些官方实现没有隐式优先级。

真实模型出站已经由独立的 model/inference 包和 Host outbound boundary 提供。Agent 包可以在 manifest 权限、capability binding 和用户/Host policy 允许时消费这些能力，但不能直接读取 raw API key、绕过 network declaration，或把 prompt/response 写入未脱敏审计。

## 包和 SDK 的禁止项

Agent adapter、参考包和 shell integration 不能：

- import runtime private module；
- 绕过 package、capability、permission、proposal 或 Change boundary；
- 在 UI 中硬编码官方 package ID 作为优先实现；
- 在 event、proposal、receipt 或 audit 中暴露 raw secret；
- 默认提供不受约束的 bash/edit/write 或任意远程 shell；
- 把 caller 提供的 session、target、path 或 network destination 当作授权依据；
- 让 agent trace、prompt 或 model taxonomy 成为 kernel schema。

## 内核非目标

内核不会新增或标准化：

- `platform.agent.*`
- `platform.model.*`
- `platform.prompt.*`
- `platform.memory.*`
- `platform.turn.*`
- agent state、chat transcript、prompt template、provider registry、thinking/reasoning 或 memory taxonomy。

这些概念可以由可选 Protocol 定义、由 Component 实现并由 Product 组合。具体完成状态和建设方向分别见 [`../ALPHA_STATUS.md`](../ALPHA_STATUS.md) 与 [`../roadmap/NEXT_STEPS.md`](../roadmap/NEXT_STEPS.md)。
