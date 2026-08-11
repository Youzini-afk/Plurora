# 事件类型注册表（v1）

本表列出 76 个只能由 Plurora 平台运行时发出的事件 kind。普通 Package writer 使用自己的 Package ID 命名空间，不能冒充平台拥有的事件。

| 事件类型 | Payload schema | Writer | 触发 | 状态 |
|---|---|---|---|---|
| `context/opened` | [`./schemas/events/context__opened.schema.json`](./schemas/events/context__opened.schema.json) | `plurora/runtime` | Session 开启 | implemented |
| `context/closed` | [`./schemas/events/context__closed.schema.json`](./schemas/events/context__closed.schema.json) | `plurora/runtime` | Session 关闭 | implemented |
| `context/forked` | [`./schemas/events/context__forked.schema.json`](./schemas/events/context__forked.schema.json) | `plurora/runtime` | Session fork 创建分支谱系 | implemented |
| `host/package.loaded` | [`./schemas/events/host__package.loaded.schema.json`](./schemas/events/host__package.loaded.schema.json) | `plurora/runtime` | 包已接受并注册；载荷包含 `contract_mode`（`v1` 或 `none`） | implemented |
| `host/package.loading` | [`./schemas/events/host__package.loading.schema.json`](./schemas/events/host__package.loading.schema.json) | `plurora/runtime` | 包进入加载中 | implemented |
| `host/package.starting` | [`./schemas/events/host__package.starting.schema.json`](./schemas/events/host__package.starting.schema.json) | `plurora/runtime` | 包执行入口启动中 | implemented |
| `host/package.ready` | [`./schemas/events/host__package.ready.schema.json`](./schemas/events/host__package.ready.schema.json) | `plurora/runtime` | 包启动后就绪 | implemented |
| `host/package.stopping` | [`./schemas/events/host__package.stopping.schema.json`](./schemas/events/host__package.stopping.schema.json) | `plurora/runtime` | 包执行停止中 | implemented |
| `host/package.stopped` | [`./schemas/events/host__package.stopped.schema.json`](./schemas/events/host__package.stopped.schema.json) | `plurora/runtime` | 包执行已停止 | implemented |
| `host/package.unloaded` | [`./schemas/events/host__package.unloaded.schema.json`](./schemas/events/host__package.unloaded.schema.json) | `plurora/runtime` | 包从注册表移除 | implemented |
| `host/package.degraded` | [`./schemas/events/host__package.degraded.schema.json`](./schemas/events/host__package.degraded.schema.json) | `plurora/runtime` | 执行失败或健康状态降级 | implemented |
| `host/package.log` | [`./schemas/events/host__package.log.schema.json`](./schemas/events/host__package.log.schema.json) | `plurora/runtime` | 捕获 subprocess stderr 日志行 | implemented |
| `host/installation.created` | [`./schemas/events/host__installation.created.schema.json`](./schemas/events/host__installation.created.schema.json) | `plurora/runtime` | Installation journal 已创建并投影 | implemented |
| `host/installation.updated` | [`./schemas/events/host__installation.updated.schema.json`](./schemas/events/host__installation.updated.schema.json) | `plurora/runtime` | Installation active Work/Lock 或状态已更新 | implemented |
| `host/installation.removed` | [`./schemas/events/host__installation.removed.schema.json`](./schemas/events/host__installation.removed.schema.json) | `plurora/runtime` | Installation 已移除并记录显式 state disposition | implemented |
| `host/exposure.created` | [`./schemas/events/host__exposure.created.schema.json`](./schemas/events/host__exposure.created.schema.json) | `plurora/runtime` | provider Installation 的 exact export Port 已 Exposure | implemented |
| `host/exposure.revoked` | [`./schemas/events/host__exposure.revoked.schema.json`](./schemas/events/host__exposure.revoked.schema.json) | `plurora/runtime` | Exposure 被 owner 或 authority 撤销 | implemented |
| `host/exposure.expired` | [`./schemas/events/host__exposure.expired.schema.json`](./schemas/events/host__exposure.expired.schema.json) | `plurora/runtime` | Exposure lease 到期 | implemented |
| `host/binding.selected` | [`./schemas/events/host__binding.selected.schema.json`](./schemas/events/host__binding.selected.schema.json) | `plurora/runtime` | consumer import Port 选择了 exact Exposure candidate | implemented |
| `host/binding.revoked` | [`./schemas/events/host__binding.revoked.schema.json`](./schemas/events/host__binding.revoked.schema.json) | `plurora/runtime` | Binding 被 consumer/provider authority 撤销 | implemented |
| `host/binding.expired` | [`./schemas/events/host__binding.expired.schema.json`](./schemas/events/host__binding.expired.schema.json) | `plurora/runtime` | Binding lease 或 Exposure 到期 | implemented |
| `host/run.starting` | [`./schemas/events/host__run.starting.schema.json`](./schemas/events/host__run.starting.schema.json) | `plurora/runtime` | Run journal 已记录 starting，准备开始激活 | implemented |
| `host/run.started` | [`./schemas/events/host__run.started.schema.json`](./schemas/events/host__run.started.schema.json) | `plurora/runtime` | Run 激活完成并进入 running | implemented |
| `host/run.stopping` | [`./schemas/events/host__run.stopping.schema.json`](./schemas/events/host__run.stopping.schema.json) | `plurora/runtime` | Run 停止已获准并进入 stopping | implemented |
| `host/run.stopped` | [`./schemas/events/host__run.stopped.schema.json`](./schemas/events/host__run.stopped.schema.json) | `plurora/runtime` | Run 激活上下文已停止并提交 terminal record | implemented |
| `host/run.failed` | [`./schemas/events/host__run.failed.schema.json`](./schemas/events/host__run.failed.schema.json) | `plurora/runtime` | Run 激活失败或 Host 重启将未完成 Run 标为 interrupted | implemented |
| `host/realization.planned` | [`./schemas/events/host__realization.planned.schema.json`](./schemas/events/host__realization.planned.schema.json) | `plurora/runtime` | 纯 planner 已持久化 exact RealizationPlan 与 Planned revision | implemented |
| `host/realization.applying` | [`./schemas/events/host__realization.applying.schema.json`](./schemas/events/host__realization.applying.schema.json) | `plurora/runtime` | apply/rollback 已持久化 intent，准备执行 Target effect | implemented |
| `host/realization.active` | [`./schemas/events/host__realization.active.schema.json`](./schemas/events/host__realization.active.schema.json) | `plurora/runtime` | Target effect、receipt 与 actual resources 已提交为 Active | implemented |
| `host/realization.stopped` | [`./schemas/events/host__realization.stopped.schema.json`](./schemas/events/host__realization.stopped.schema.json) | `plurora/runtime` | 已记录资源关闭后提交 Stopped terminal revision | implemented |
| `host/realization.failed` | [`./schemas/events/host__realization.failed.schema.json`](./schemas/events/host__realization.failed.schema.json) | `plurora/runtime` | apply 失败或结果不确定，payload 给出稳定 reason code | implemented |
| `host/realization.rolled_back` | [`./schemas/events/host__realization.rolled_back.schema.json`](./schemas/events/host__realization.rolled_back.schema.json) | `plurora/runtime` | 持久化 historic plan 已形成新的 active replacement | implemented |
| `host/realization.reconciled` | [`./schemas/events/host__realization.reconciled.schema.json`](./schemas/events/host__realization.reconciled.schema.json) | `plurora/runtime` | effect-free Target observation 已更新 Realization truth | implemented |
| `object/put` | [`./schemas/events/object__put.schema.json`](./schemas/events/object__put.schema.json) | `plurora/runtime` | 不透明 asset 已存储 | implemented |
| `projection/updated` | [`./schemas/events/projection__updated.schema.json`](./schemas/events/projection__updated.schema.json) | `plurora/runtime` | projection 状态已重建/更新 | implemented |
| `change/proposal.created` | [`./schemas/events/change__proposal.created.schema.json`](./schemas/events/change__proposal.created.schema.json) | `plurora/runtime` | proposal 已创建 | partial |
| `change/proposal.approved` | [`./schemas/events/change__proposal.approved.schema.json`](./schemas/events/change__proposal.approved.schema.json) | `plurora/runtime` | proposal 已批准 | partial |
| `change/proposal.rejected` | [`./schemas/events/change__proposal.rejected.schema.json`](./schemas/events/change__proposal.rejected.schema.json) | `plurora/runtime` | proposal 已拒绝 | partial |
| `change/proposal.applied` | [`./schemas/events/change__proposal.applied.schema.json`](./schemas/events/change__proposal.applied.schema.json) | `plurora/runtime` | proposal 已应用 | partial |
| `change/proposal.failed` | [`./schemas/events/change__proposal.failed.schema.json`](./schemas/events/change__proposal.failed.schema.json) | `plurora/runtime` | proposal 应用失败 | partial |
| `capability/invoked` | [`./schemas/events/capability__invoked.schema.json`](./schemas/events/capability__invoked.schema.json) | `plurora/runtime` | 能力调用开始 | planned |
| `capability/completed` | [`./schemas/events/capability__completed.schema.json`](./schemas/events/capability__completed.schema.json) | `plurora/runtime` | 能力调用成功 | planned |
| `capability/failed` | [`./schemas/events/capability__failed.schema.json`](./schemas/events/capability__failed.schema.json) | `plurora/runtime` | 能力调用失败 | planned |
| `authority/denied` | [`./schemas/events/authority__denied.schema.json`](./schemas/events/authority__denied.schema.json) | `plurora/runtime` | 权限检查拒绝 | implemented |
| `authority/grant.created` | [`./schemas/events/authority__grant.created.schema.json`](./schemas/events/authority__grant.created.schema.json) | `plurora/runtime` | 权限授予已记录 | implemented |
| `authority/grant.revoked` | [`./schemas/events/authority__grant.revoked.schema.json`](./schemas/events/authority__grant.revoked.schema.json) | `plurora/runtime` | 权限授予已撤销 | implemented |
| `runtime/error` | [`./schemas/events/runtime__error.schema.json`](./schemas/events/runtime__error.schema.json) | `plurora/runtime` | 结构化内核错误 | planned |
| `host/outbound.request` | [`./schemas/events/host__outbound.request.schema.json`](./schemas/events/host__outbound.request.schema.json) | `plurora/runtime` | 出站请求已允许并审计 | partial |
| `host/outbound.denied` | [`./schemas/events/host__outbound.denied.schema.json`](./schemas/events/host__outbound.denied.schema.json) | `plurora/runtime` | 出站请求被拒绝 | partial |
| `host/outbound.execute.completed` | [`./schemas/events/host__outbound.execute.completed.schema.json`](./schemas/events/host__outbound.execute.completed.schema.json) | `plurora/runtime` | 出站 execute 完成 | implemented |
| `host/outbound.stream.completed` | [`./schemas/events/host__outbound.stream.completed.schema.json`](./schemas/events/host__outbound.stream.completed.schema.json) | `plurora/runtime` | 出站 stream 完成 | implemented |
| `capability/stream.started` | [`./schemas/events/capability__stream.started.schema.json`](./schemas/events/capability__stream.started.schema.json) | `plurora/runtime` | streaming 调用开始 | partial |
| `capability/stream.chunk` | [`./schemas/events/capability__stream.chunk.schema.json`](./schemas/events/capability__stream.chunk.schema.json) | `plurora/runtime` | streaming chunk 已发出 | partial |
| `capability/stream.progress` | [`./schemas/events/capability__stream.progress.schema.json`](./schemas/events/capability__stream.progress.schema.json) | `plurora/runtime` | streaming 进度已发出 | partial |
| `capability/stream.ended` | [`./schemas/events/capability__stream.ended.schema.json`](./schemas/events/capability__stream.ended.schema.json) | `plurora/runtime` | streaming 正常结束 | partial |
| `capability/stream.error` | [`./schemas/events/capability__stream.error.schema.json`](./schemas/events/capability__stream.error.schema.json) | `plurora/runtime` | streaming 出错 | partial |
| `capability/stream.cancelled` | [`./schemas/events/capability__stream.cancelled.schema.json`](./schemas/events/capability__stream.cancelled.schema.json) | `plurora/runtime` | streaming 已取消 | partial |
| `capability/stream.timeout` | [`./schemas/events/capability__stream.timeout.schema.json`](./schemas/events/capability__stream.timeout.schema.json) | `plurora/runtime` | streaming 超时 | partial |
| `host/outbound.websocket.opened` | [`./schemas/events/host__outbound.websocket.opened.schema.json`](./schemas/events/host__outbound.websocket.opened.schema.json) | `plurora/runtime` | 出站 WebSocket 已打开 | implemented |
| `host/outbound.websocket.frame` | [`./schemas/events/host__outbound.websocket.frame.schema.json`](./schemas/events/host__outbound.websocket.frame.schema.json) | `plurora/runtime` | 出站 WebSocket frame 已记录 | implemented |
| `host/outbound.websocket.error` | [`./schemas/events/host__outbound.websocket.error.schema.json`](./schemas/events/host__outbound.websocket.error.schema.json) | `plurora/runtime` | 出站 WebSocket 错误 | implemented |
| `host/outbound.websocket.completed` | [`./schemas/events/host__outbound.websocket.completed.schema.json`](./schemas/events/host__outbound.websocket.completed.schema.json) | `plurora/runtime` | 出站 WebSocket 完成/关闭 | implemented |
| `host/exec.request` | [`./schemas/events/host__exec.request.schema.json`](./schemas/events/host__exec.request.schema.json) | `plurora/runtime` | exec 请求已记录 | implemented |
| `host/exec.denied` | [`./schemas/events/host__exec.denied.schema.json`](./schemas/events/host__exec.denied.schema.json) | `plurora/runtime` | exec 请求被拒绝 | implemented |
| `host/exec.started` | [`./schemas/events/host__exec.started.schema.json`](./schemas/events/host__exec.started.schema.json) | `plurora/runtime` | exec 已启动 | implemented |
| `host/exec.stopped` | [`./schemas/events/host__exec.stopped.schema.json`](./schemas/events/host__exec.stopped.schema.json) | `plurora/runtime` | exec 已停止 | implemented |
| `host/exec.completed` | [`./schemas/events/host__exec.completed.schema.json`](./schemas/events/host__exec.completed.schema.json) | `plurora/runtime` | exec 已完成 | planned |
| `host/exec.failed` | [`./schemas/events/host__exec.failed.schema.json`](./schemas/events/host__exec.failed.schema.json) | `plurora/runtime` | exec 失败 | planned |
| `host/port.leased` | [`./schemas/events/host__port.leased.schema.json`](./schemas/events/host__port.leased.schema.json) | `plurora/runtime` | host port lease 已创建 | implemented |
| `host/port.released` | [`./schemas/events/host__port.released.schema.json`](./schemas/events/host__port.released.schema.json) | `plurora/runtime` | host port lease 已释放 | implemented |
| `host/port.denied` | [`./schemas/events/host__port.denied.schema.json`](./schemas/events/host__port.denied.schema.json) | `plurora/runtime` | host port lease 被拒绝 | implemented |
| `host/proxy.registered` | [`./schemas/events/host__proxy.registered.schema.json`](./schemas/events/host__proxy.registered.schema.json) | `plurora/runtime` | proxy route 已注册 | implemented |
| `host/proxy.unregistered` | [`./schemas/events/host__proxy.unregistered.schema.json`](./schemas/events/host__proxy.unregistered.schema.json) | `plurora/runtime` | proxy route 已移除 | implemented |
| `host/proxy.denied` | [`./schemas/events/host__proxy.denied.schema.json`](./schemas/events/host__proxy.denied.schema.json) | `plurora/runtime` | proxy 注册被拒绝 | implemented |
| `host/deployment.reconciled` | [`./schemas/events/host__deployment.reconciled.schema.json`](./schemas/events/host__deployment.reconciled.schema.json) | `plurora/runtime` | 启动后部署状态 reconcile 汇总 | implemented |
| `host/deployment.health` | [`./schemas/events/host__deployment.health.schema.json`](./schemas/events/host__deployment.health.schema.json) | `plurora/runtime` | host TCP 健康探测触发部署 ready 状态变化 | implemented |
