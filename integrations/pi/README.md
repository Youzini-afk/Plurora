# pi Reference Ledger

> English summary is available in [`README.en.md`](./README.en.md). 中文为默认说明。

本目录记录 Plurora 如何研究并吸收 [pi](https://github.com/earendil-works/pi)，同时避免把 pi 变成内核、协议或官方特权路径。

## 上游快照

- 上游仓库：https://github.com/earendil-works/pi
- 上游项目：`pi` agent harness mono repo
- License：MIT
- 关键包：
  - `@earendil-works/pi-ai`
  - `@earendil-works/pi-agent-core`
  - `@earendil-works/pi-coding-agent`

## 分级吸收

### Adapter-now

这些能力可在 Plurora SDK / template / reference package 中立刻吸收，但要通过 Plurora public contract 表达：

- pi-agent-core 的 `AgentEvent` 思路：run/turn/message/tool execution lifecycle。
- pi-agent-core 的 `AgentTool` 思路：label、argument preparation、execution mode、result/error/terminate。
- `beforeToolCall` / `afterToolCall` policy gates。
- parallel / sequential tool execution policy。
- steer / followUp queue 概念。
- pi-ai faux provider 的 deterministic/scripted 测试策略。

### Package-internal optional

这些可以在普通能力包内部可选使用，不得进入 constitutional substrate, Host service, or Web shell：

- `pi-agent-core` agent loop。
- `pi-ai` faux provider。
- `pi-ai` stream/tool-call event shapes。

### Reference only

这些只作为设计参考：

- pi-coding-agent session tree / fork / compaction。
- pi-coding-agent resource loading / skills / extension organization。
- pi-coding-agent model resolver / provider display names。
- pi-ai provider lazy registration 和 provider onboarding checklist。
- pi TUI / web-ui 的展示经验。

### Deferred

这些推迟到专门 package 和 host policy 完成后再考虑：

- real provider calls。
- OAuth / API key login。
- provider discovery / model catalog。
- real model inference / streaming inference。
- multi-agent orchestration / director / planner graph。

### Rejected

这些明确不进入 Plurora agent infrastructure：

- `pi-coding-agent` 作为 Plurora 产品壳。
- 默认 bash/read/write/edit tools。
- kernel agent/model/prompt/memory methods。
- private runtime access。
- first-party Package priority。
- raw prompt/response/secret persistence。

## Plurora 映射

| pi-inspired idea | Plurora landing point |
|---|---|
| Agent run | ordinary package capability via `capability.invoke/stream` |
| Abort/cancel | `capability.cancel` |
| Tool | adapter view of Plurora capability |
| Tool execution | `capability.invoke/stream` with explicit provider package |
| Tool gate | permission preview + before/after helper in SDK/package |
| Trace | package-owned events and stream frames |
| Proposal | `platform.proposal.*` |
| State | package-owned assets/projections/capabilities |
| UI | surface contributions consumed by Assist/Forge |

## 验证纪律

每次 agent infrastructure 更新都要证明：

- 没有新增 `platform.agent.*`、`platform.model.*`、`platform.prompt.*`、`platform.memory.*`、`platform.turn.*`。
- official agent/reference packages 没有优先级。
- tool bridge 对 ambiguous provider 拒绝或要求显式 provider。
- unauthorized tool calls 失败并审计。
- proposal 未批准不能 apply。
- cancel 后 stream 不能继续 append。
- raw secret 不能进入 proposal/audit/trace。
