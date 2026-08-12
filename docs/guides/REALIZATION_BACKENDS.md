# Realization 执行器的 Runtime adapter

> [English](./REALIZATION_BACKENDS.en.md) · [中文](./REALIZATION_BACKENDS.md)

本页记录 `ServiceRealizationExecutor` 使用的低层 Target、exec、port 与 proxy adapter。公开 managed lifecycle 由 [`REALIZATION.md`](REALIZATION.md) 和 `host.realization.*` 定义；本页的 adapter 不是另一套 managed lifecycle、revision 或 authority。

## 边界

- Work 通过 `OperationalIntent` 表达便携 workload 要求。
- Pure planner 用 `TargetInventorySnapshot` 编译 content-addressed `RealizationPlan`。
- `host.realization.apply` 在 exact approval、precondition 与 authority 下调用 executor。
- Executor 把 plan action 翻译为 typed `TargetOperationSpec`，并将结果写成 `RealizedResource` 与 `RealizationEffectReceipt`。
- `host.exec.*`、`host.port.*` 与 `host.proxy.*` 是 HostAdmin/HostDev 的低层 adapter；普通 device 不能用这些方法绕过 Realization。

Web 与 CLI 只调用 `host.realization.*`；低层 adapter 没有面向普通 device 的并行 managed API。

## Local 与 Target Agent 同一路径

Local Host 与远程 Target Agent 都接收同一个 typed operation：

- declarative verifier / Dockerfile build；
- managed workload apply；
- workload observe；
- workload stop。

Target operation 带 Installation、Target authority epoch、request digest、idempotency key、状态和 terminal receipt。Local driver 与 Agent transport 只改变执行位置，不改变 Plan 或 receipt 的语义。

## Docker backend

当前首个 backend 有两种输入：

1. immutable OCI image，必须包含 `@sha256:` digest；
2. Docker build，输入为已持久化 build context artifact、相对 Dockerfile、network policy、workspace/source digest 与 build descriptor hash。

Build 返回 immutable image 后才进入同一 launch path。Docker 不是 Work/Assembly 的必填概念；后续 backend 通过新的 execution class 与 typed action 扩展。

## 端口与路由

Executor 为每个 endpoint 创建 Host-owned loopback port lease，并注册绑定该 lease 的 HTTP route。`RealizedResource.properties` 记录 Target workload reference、route、port lease、observed host port、public URL 与 immutable image。Stop 只根据这些已记录资源清理，不扫描 live workspace 或任意容器。

默认 route access 是 Host-authenticated；只有 Realization backend selection 明确选择 public policy 时才公开。Route ID 与 workload ID 必须通过现有 token validation，不能是绝对路径、URL 或 shell fragment。

## 健康与重启

Host 启动顺序是：

1. hydrate Runtime backend event projection；
2. reconcile Target workload/runtime adapter truth；
3. hydrate Installation、Powerbox、Realization、Run authority journals；
4. Realization 对 Applying/Stopping 记录执行 effect-free observation 或消费 durable effect checkpoint。

公开 readiness 返回 durable/active/degraded Realization 数量，并只把 Realization projection 视为 managed truth。

如果 target result 不确定，executor 返回 `outcome_unknown`；如果资源/receipt 无法安全确认，Realization 进入 `recovery_required`。Host 不以端口占用、进程存在或 route 名称推断成功。

## Receipt 与日志

Target terminal receipt 包含 operation/action、Target、request digest、时间和结构化状态。Realization 将 receipt 作为 content-addressed artifact 引用。公开错误只返回稳定 code 与 next step，不包含 credential、raw stderr、secret、Docker build output 或本机绝对路径。

低层 exec/proxy/port lifecycle event 仍用于 adapter 诊断；Realization 的用户可见状态只由七个 `host/realization.*` lifecycle event 表达。私有 stopping/effect checkpoint 不进入公共 journal。

## 非目标

- 不提供任意 shell 或任意网络代理；
- 不允许 Surface、Package 或 Agent 获得 root Target authority；
- 不让 Run start 隐式 build/apply；
- 不从 live workspace 执行 rollback；
- 不把 Docker、route 或 Target operation 写入 Constitutional Substrate。

详细 authority 与恢复规则见 [`REALIZATION.md`](REALIZATION.md)、[`../architecture/HOST_RESOURCE_AUTHORITY.md`](../architecture/HOST_RESOURCE_AUTHORITY.md) 与 [`../architecture/TARGET_AGENT_PROTOCOL.md`](../architecture/TARGET_AGENT_PROTOCOL.md)。
