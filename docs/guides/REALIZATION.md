# Realization：把便携 Work 编译到 Target

> [English](./REALIZATION.en.md) · [中文](./REALIZATION.md)

Realization 是 Host 拥有的可变执行记录。`WorkRevision`、`AssemblyLock` 与 `OperationalIntent` 描述“要运行什么”；`TargetInventorySnapshot` 描述某个 Target 当前能做什么；纯 planner 把两者编译成 content-addressed `RealizationPlan`。Plan 本身不授予执行权，也不产生 Target effect。

Phase 6 实现了 `host.realization.plan/apply/get/list/stop/rollback/reconcile`、七个公开生命周期事件、CLI 命令与 Installation frame 的 Realization workbench。Docker/OCI 是首个 backend，不是模型中的唯一长期执行形态。

## 数据与所有权

- `OperationalIntent` 是 Work 引用的便携意图：workload、允许的 execution class、资源、endpoint、state、placement 与 update policy。
- `TargetInventorySnapshot` 是 Host 观察到的 Target capability/capacity/trust/topology 快照；它作为 artifact 进入 plan digest。
- `RealizationPlan` 是纯编译产物，固定 Work、AssemblyLock、OperationalIntent、inventory、actions、preconditions、required authority 与 risk summary。
- `RealizationRevision` 是 Host journal projection，记录状态、实际资源、receipt、health、parent 与时间戳。它不是 portable artifact。
- approval 与 effect receipt 进入 ObjectStore；公开事件只携带经过类型化、可脱敏的 Realization view。

Runtime 不把 Realization 写入 Constitutional Substrate。Host control journal 是可变事实的权威来源，Runtime public journal 只接收七个公开 relay event；`host-private/realization.*` 的 stopping/effect checkpoint 永不进入公开事件流。

## 生命周期

```text
Planned → Applying → Active / Failed / OutcomeUnknown / RecoveryRequired
Active / Degraded → Stopping → Stopped
historic Planned/Active → Applying replacement → RolledBack(active replacement)
```

`plan` 读取并验证当前 Installation、Work/Lock、OperationalIntent 与 Target inventory，生成稳定 plan bytes/digest，再持久化 Planned revision。它可以写不可变 artifact 和 Host journal，但不调用 build、launch、route 或 stop effect。

`apply` 必须精确匹配持久 plan reference，并重新验证：

1. Installation revision、Work digest、AssemblyLock digest 与 OperationalIntent；
2. Target lease/policy epoch 与 inventory precondition；
3. approval 的 exact plan digest、有效期与全部 `risk_summary`；
4. current `realization.apply` grant/delegation/lease，以及 exact Installation、Target、Realization selectors；
5. 每个 build、route、launch 与 terminal journal boundary 前的 current authority。

通过后，Host 使用 typed Target operation path 执行 backend action。Local 与 remote Agent 共享同一 plan、operation、receipt 和 Realization 状态机，不存在第一方旁路。

## Durable effect checkpoint

外部 effect 成功后、公开 Active/Stopped/RolledBack 之前，Host 先在 control journal 写私有 effect checkpoint。Checkpoint 保存实际资源与结构化 receipt facts；receipt artifact 暂时不可写时，重启或同 key 重试可以继续物化，而不会再次 apply 已完成的外部 effect。

- 同一 apply key 命中 Applying + apply checkpoint 时，只续交公开终态。
- Stopping + stop checkpoint 在重启后直接提交 Stopped，不重复 stop。
- rollback replacement apply 完成但旧版本 stop 暂时失败时，replacement 与 parent 的两个 checkpoint 都保留；同 key 重试从旧版本 stop 继续，不重新 apply replacement。
- 没有 effect checkpoint 的未完成 Applying 不会被自动重放；重启后进入 `recovery_required`，需要显式 reconcile/stop/rollback。
- build 已完成但 launch/route 未完整成功时，不会丢弃部分事实：已知 receipt 与仍可能存在的 workload identity 会进入 `recovery_required` 或 `outcome_unknown` revision，后续必须显式 reconcile；不能把它降格为无效果的普通失败。
- private event 不计入 76 个平台事件，也不能通过 public journal/SSE 观察。

这不能把外部系统与 Host journal 变成单一原子事务：如果外部 effect 已发生，而 control journal 本身同时不可写，Host 只能 fail closed 并要求 reconcile。Target operation 使用稳定 idempotency key，恢复不会从 live workspace 猜测输入。

## Stop、rollback 与 reconcile

`stop` 只处理 `actual_resources` 中已记录的资源。它先写 stopping intent，调用 typed Target stop，保存 receipt checkpoint，最后提交 Stopped。关闭浏览器标签、离开 Installation frame 或读取状态都不会触发 stop。

`rollback` 只读取 Host 已持久化的 historic `RealizationPlan` 与新的 exact approval。它不会读取 live workspace、重新打包源目录或沿旧 Deployment revision 猜测参数。Host 为 replacement 生成新 RealizationId，先 checkpoint replacement effect，再停止 parent，最后提交 `host/realization.rolled_back`。

`reconcile` 是显式的 effect-free Target observation。它更新 observed resources、health 与 receipt，并区分 `outcome_unknown`、`recovery_required`、`target_unsatisfied` 等稳定 reason code；它不会隐式再次 apply。

## Backend 与 Target

当前实现支持：

- 首个 executor 每个 plan 精确执行一个 workload；便携模型仍保留多 workload/action 结构。多 workload 请求在任何 Target effect 前返回 `unsupported_backend`，待后续 executor 实现原子 checkpoint/补偿后再开放；
- 预构建的 immutable OCI image（必须为 `name@sha256:<digest>`）；
- Dockerfile build，经 declarative verifier 产生 content-addressed image，再走同一 launch path；
- 本机 Target 与远程 Target Agent 的 typed build/apply/observe/stop operation；
- Host-owned loopback port lease、route 与 actual resource receipt。

Planner 只接受 OperationalIntent 允许、TargetInventory 明确提供的 execution class。未知/离线 Target、缺失能力、过期 authority epoch、非内容寻址 image、越过 lock/Work precondition 都返回结构化 gap 或错误。没有基于 publisher 的优先级，也没有自动选择另一个 Target。

## 公开方法与 authority

| 方法 | Action | Exact resources |
|---|---|---|
| `host.realization.plan` | `realization.plan` | Installation + Target |
| `host.realization.get/list` | `observe` | 可见 Installation + Realization（list 可按 Target 过滤） |
| `host.realization.apply/stop/reconcile` | `realization.apply` | Installation + Target + Realization |
| `host.realization.rollback` | `realization.apply` | Installation + Target + current Realization + historic Realization |

省略 resource ID 不会产生 wildcard。Root credential 仍只用于 Host 维护；Plan、UI、Agent 输出或 approval artifact 都不等于执行 authority。旧 `deploy` device scope 已删除；raw Target/exec/port/proxy effects 只保留为 HostAdmin/HostDev adapter，不能替代 `host.realization.apply`。

## CLI 与 Web

CLI 提供：

```text
plurora realization list
plurora realization info
plurora realization plan
plurora realization apply
plurora realization stop
plurora realization rollback
plurora realization reconcile
```

所有 effect 命令要求显式 Installation、Target、Realization revision 与 idempotency key。Apply/rollback 要求对 exact plan 的 `--approve` 以及每项 risk acceptance；输出不包含 credential、绝对路径、raw stderr 或 secret。

Installation frame 只在 Work 声明 OperationalIntent 时显示 Realization workbench。用户先选择 Target/backend 并执行 plan，检查 actions/preconditions/authority/risk，再逐项确认风险后 apply。UI 不把当前浏览器未持有的 plan body 猜回出来；旧 Planned revision 需要重新选择 exact Target 或使用 CLI。关闭 frame 不 stop Realization。

## 与 Run、Binding 和后续 Phase 的边界

- `host.run.start` 仍不隐式 build 或 apply Realization；缺少 managed Realization 是结构化 gap。
- Exposure/Binding 只授予选中 Port 的最小 runtime handle，不创建或迁移 managed resources。
- Realization 不共享 cross-Installation state；StateSlot/backup/migration 仍按显式 owner 与 policy 工作。
- Phase 7 负责 Foreign Work、Rights/Transparency、闭源入口和 opaque-state backup。
- Phase 8 负责开发闭环与 companion agent；ChangeSet/Plan 仍不能绕过本页 authority/effect boundary。

机器可读契约见 [`../spec/PUBLIC_CONTRACT.md`](../spec/PUBLIC_CONTRACT.md)、[`../spec/v1/EVENT_KIND_REGISTRY.md`](../spec/v1/EVENT_KIND_REGISTRY.md) 与 `docs/spec/v1/schemas/`。
