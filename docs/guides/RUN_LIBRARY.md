# Run 与 Library 指南

> [English](./RUN_LIBRARY.en.md) · [中文](./RUN_LIBRARY.md)

本指南描述 Run 生命周期、Powerbox 与官方 Library 入口。Run 是 Host 上一次实际执行的 durable 事实；Installation 是采用记录，Work/Assembly 是便携定义。打开 Library 条目或 Installation 详情不会隐式创建 Run、构建源码或apply Realization。

## 对象与所有权

| 对象 | 所有者 | 语义 |
|---|---|---|
| WorkRevision / AssemblyRevision / AssemblyLock | ObjectStore 中的不可变 artifact | 定义作品、构成与精确 provider/绑定解析。 |
| Installation | Host Service journal | 拥有 active Work/Lock、用户选择、state bindings 与 secret policy。 |
| InstallationWorkSummary | exact WorkRevision 的 Host 校验投影 | 向 Library 公开 WorkId、标题、完整 entrypoints 与 Rights / Transparency / OperationalIntent 引用；不是第二份 authority。 |
| Run | Host Service 的 Run journal | 拥有一次启动、激活上下文、节点实例、health 与 terminal 状态。 |
| Exposure | Host journal | provider Installation/Run 的 exact export Port、audience、lease 与撤销状态。 |
| Binding | Host journal | consumer import Port 到 Exposure 的选择、candidate digest、runtime pin 与 terminal 状态。 |
| Library | 官方 Shell / client | 从 Work、Installation、Run、Rights 与当前 authority 计算可执行 affordance；不替代 Host 授权。 |

MVP 的默认策略是一个 Installation 最多有一个 active Run。RunId 由 Host 在通过 preflight 后生成，不能由客户端预先指定。Run journal 是 durable authority；内存中的激活句柄只是运行时投影，Host 重启时不会据此猜测成功。

## 生命周期

```text
starting → running ↔ degraded → stopping → stopped
    └────→ failed / interrupted
```

Exposure 与 Binding 的动态状态由 Powerbox durable journal 管理：

```text
Exposure: active → closing → revoked / expired
Binding:  selected / active → closing → terminal
```

close 先于 terminal commit；Host restart、owner takeover、retry 与 `outcome_unknown` 不会重写历史终态，恢复通过新记录完成。

`starting` 事件先提交，再执行激活；成功后提交 `running`。显式 stop 经过 `stopping`，完成后提交 `stopped`。激活失败写入 `failed`。当 Host 明确观测到受 Run 租约约束的 JSON-RPC stdio Package transport 永久失联时，每个受影响的 active Run 都会 durable 收敛为 `failed`，health reason 为 `package_activation_lost`；重复或迟到的旧进程通知不会产生第二个终态。Host 重启会把仍处于 active 的 Run 追加为 `interrupted`，health reason 为 `host_restart`，不会自动重放或假定进程仍然可用。终态历史不回写，恢复要通过新的 Run。

Run 的 `context_id`（如存在）只属于该 Run 的执行上下文。关闭浏览器 tab、Surface iframe 或 PWA 连接不会停止 Run；停止必须由有权调用者显式执行 `host.run.stop`。Stop 只释放该 Run 的 activation/context，不执行全局 Package unload，也不影响其他 Installation 的 Run。

## 公开方法与 authority

| 方法 | 所需作用域 | 关键输入/行为 |
|---|---|---|
| `host.run.list` | `observe` | 可选 `installation_id`、`status` 过滤；仅返回 caller 可见的 Run。 |
| `host.run.get` | `observe` | 必须同时提供 exact `installation_id` 与 `run_id`；校验归属后返回 Run view。 |
| `host.run.status` | `observe` + exact Installation | 不带 `entrypoint_id` 时返回当前 Installation revision 与 active Run 概况；带入口时执行零副作用 preflight 并返回 gaps。结果不是 reservation 或 authority，start 会重新校验。 |
| `host.run.start` | `run` + exact Installation | 提供 `expected_installation_revision`、entrypoint 与 `idempotency_key`。Host 先重新验证 Installation 与 authority，再生成 RunId。 |
| `host.run.stop` | `run` + exact Installation + requested Run child | 提供 exact I/R、`expected_revision` 与 `idempotency_key`。RunId 由 Host 创建，因此公开规则是：exact Installation grant 只能派生到 registry 已证明属于它的 Run child；错误归属仍 fail closed。 |

Start/stop 的相同幂等键与相同 fingerprint 会重放 durable 结果；fingerprint 不同返回 `idempotency_conflict`。journal 只保存 key 的 SHA-256，不保存调用方原始 key。一个 Installation 的 active Run 已存在时，新的 start 返回 `active_run_exists`。

## 启动前置检查与结构化 gap

`host.run.status` 与 `host.run.start` 共用同一套入口检查；status 只读，start 在 effect 前重验。start 只会激活当前 Installation 锁定的、已安装且已验证的本地 Package Component。实现必须与 AssemblyLock 的 artifact digest、behavior digest、trust class 和 entry kind 精确匹配，并且只能有一个 ready Package 匹配；没有 publisher priority 或模糊选择。

Run-bound 激活会为这些已经 ready 的本地实现建立 Run context、节点实例记录，以及精确的 Installation 与 Package 生命周期租约；它不会加载、构建或 apply Package，也不会为每个 Run 另起一个 Package 进程。共享 Package 的进程和 capability 仍由 Runtime 全局拥有；任一 Run 持有租约时 Package 不能被 unload/restart，Run stop 只释放该 Run 的 context 与租约。

如果不能安全启动，结果保持 `run: null`，返回结构化 `gaps[]`。每项包含稳定的 `reason_code`，可选 `node_id` / `port_id`，以及可执行的 `next_step`：

- `artifact_missing`：锁定的 Component 未安装、未 ready 或 digest/behavior/trust 不匹配；安装并验证 exact Package。
- `binding_ambiguous`：有多个等同匹配的本地 Package；卸载多余实现或显式整理锁定结果，不按 publisher 选择。
- `binding_unavailable`：required Launch/Runtime import 没有固定可用绑定；通过 Exposure/Binding 选择补齐，不能篡改 Lock 绕过。
- `unsupported_backend`：WASM、remote、`contract: none`/Foreign Capsule、非 JSON-RPC subprocess 等执行形态没有可用 Run driver；改用受支持的本地实现。
- `target_unsatisfied`：entrypoint/Port 无法满足，或 Work 带有需要机器资源的 OperationalIntent；按照 gap 的 next step 准备所需 Realization。

这些 gap 是诊断与下一步，不是隐式授权。Run start 不 build 源码、不创建 public route、不执行 managed Realization，也不隐式调用 `host.realization.apply`。缺少 managed Realization 时必须停在 gap，由用户通过独立 plan/approval/apply 流程处理。

## Powerbox 与 Library

官方 Library 读取可见的 Work、Installation、Run、Rights 和当前 Host authority，计算 `Open`、`Install`、`Play/Run`、`Stop`、`Inspect`、`Update`、`Remove` 等 affordance。Affordance 的 `reason_code`、风险和 `next_step` 只帮助界面解释状态；真正请求仍由 Host 的 exact selector 和 action scope 决定。

- `Open` / 详情只读 Work 与 Installation projection，不创建 Run。
- `Play` / `Run` 调用 `host.run.start`，展示 starting、running、degraded、failed 或 gap。
- `Stop` 调用 `host.run.stop`，不会卸载共享 Package，也不会删除 Installation 或用户 state。
- 重启后的 `interrupted` Run 显示需要重新启动的新 Run，而不是伪造恢复成功。
- stop effect 已发生但 terminal journal commit 无法确认时，重放收敛为 `interrupted` + `outcome_unknown`，不会把 Stopping 猜成成功。
- Powerbox chooser 通过 `host.exposure.*` / `host.binding.*` 显示 explicit phase、exact Exposure/audience/expiry、两端 PortContract、provider source/trust/claims/boundaries/evidence、candidate digest/stale 状态；0 或多个候选都要求明确选择，preference 只是排序 hint。
- Runtime 只注入选中 Port 的最小 handle；同一 Component 多 Port 可共享 activation，不同 Component 或 node path 隔离。provider stop、revoke、expiry 或 version drift 会取消 Binding；不跨 Installation 共享 state/secret。
- Exposure、跨 Installation Binding 与 Managed Realization 均已实现，但 Run 生命周期仍与其分离，详见 [`REALIZATION.md`](REALIZATION.md)。

## 相关契约

- [`../spec/PUBLIC_CONTRACT.md`](../spec/PUBLIC_CONTRACT.md) — 公开方法、事件与 authority 约定。
- [`../spec/v1/EVENT_KIND_REGISTRY.md`](../spec/v1/EVENT_KIND_REGISTRY.md) — Run、Exposure、Binding、Realization lifecycle events。
- [`INSTALLATION_MODEL.md`](INSTALLATION_MODEL.md) — Installation journal、state 与 Work 边界。
- [`../architecture/HOST_RESOURCE_AUTHORITY.md`](../architecture/HOST_RESOURCE_AUTHORITY.md) — exact resource selector 与 `run` action。
- [`POWERBOX_BINDING.md`](POWERBOX_BINDING.md) — candidate disclosure、runtime pin 与 revoke/expiry 规则。
