# 公开契约 v1

> [English](./PUBLIC_CONTRACT.en.md) · [中文](./PUBLIC_CONTRACT.md)

本文档定义 Plurora 当前 v1 公开契约：方法、事件、错误、capability handle、Manifest 声明、schema、协商与 conformance。所有参与方使用同一套按 owner 分层的身份与行为规则；契约不暴露并行 alias surface。

v1 的设计目标不是把某种内容形态写进核心机制，而是让组件、安全执行、审计、SDK 与第三方客户端拥有稳定边界。角色、世界、提示词、模型、消息、记忆等内容语义属于相应协议、组件或产品，不属于宪法基底。

## 状态语言

- `implemented`：代码已实现并有测试或 conformance 覆盖。
- `partial`：核心路径已实现，边角情况、传输一致性或生产级策略仍待补。
- `planned`：契约中预留，未实现，调用方不能依赖。

## 路径 A vs 路径 B

v1 契约支持两种参与方式：

- **路径 A**（默认）：Package 通过 `entry.contract: "v1"` 接受契约约束。Manifest 声明 capability、permission 与 effect；runtime 强制执行；调用使用 runtime-minted handle；生命周期与审计事件会被记录。
- **路径 B**：Package 通过 `entry.contract: "none"` 退出 v1 capability enforcement。Host 仍可运行进程并记录生命周期，但不会注入 v1 authority binding。

路径 A 适合需要平台 authority、network、secret、audit 与 SDK 的 Package。路径 B 适合只需要托管、不需要平台 authority 的自包含应用与工具。

## 公开方法矩阵（99）

完整请求/响应 schema 位于 `docs/spec/v1/schemas/methods/`。方法名是稳定公开 API；v1 只允许 additive 变更。

### `context.*`（6）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `context.open` | implemented | 开启内容无关 session，写入 `context/opened`。 |
| `context.close` | implemented | 关闭 session，写入 `context/closed`。 |
| `context.fork` | partial | 从父 session 与 sequence 创建 branch lineage，不解释内容。 |
| `context.branch.list` | partial | 列出与 session 相关的 branch 记录。 |
| `context.get` | partial | 查询单个 session；行为与错误契约仍在加固。 |
| `context.list` | planned | 预留 host 管理列表。 |

### `journal.*`（3）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `journal.append` | implemented | 对 Package writer 强制 event ownership 与 `events.append`。 |
| `journal.list` | partial | 按 session 列出事件，支持 sequence、limit、kind、writer 过滤与权限门控；跨后端一致性仍在加固。 |
| `journal.subscribe` | planned | SSE replay/tail 路由已存在；公开 method dispatch 与 package-principal subscribe 权限尚未落地。 |

### `host.package.*`（7）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `host.package.load` | partial | 验证 manifest、host policy、路径 A/路径 B、入口约束，注册声明并发出生命周期事件；部分 entry form 仍是占位。 |
| `host.package.unload` | partial | 停止执行、移除注册、撤销运行时句柄、发出停止/卸载事件；不同 entry form 的完整对称性仍在加固。 |
| `host.package.list` | implemented | 列出内存 package record。 |
| `host.package.status` | implemented | 返回单个 package record。 |
| `host.package.restart` | partial | 已支持 subprocess restart；其他 entry 形式按策略拒绝。 |
| `host.package.logs` | partial | 捕获 subprocess stderr；stdout 保留给 JSON-RPC 帧。 |
| `host.package.describe` | planned | 可由 status manifest 派生，公开方法预留。 |

### `capability.*`（5）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `capability.discover` | implemented | 列出注册的 capability descriptor。 |
| `capability.describe` | planned | 预留 descriptor 单项查询。 |
| `capability.invoke` | partial | 用调用者上下文与 capability handle 强制权限；验证 schema；completed/failed 终态挂接 EffectReceipt；支持 recorded replay 与 branch re-execute；跨 entry/transport 一致性仍在加固。 |
| `capability.stream` | partial | 启动 streaming invocation，产生有序 start/chunk/progress/terminal frame；ended/error/cancelled/timeout 生成可区分 terminal receipt。 |
| `capability.cancel` | partial | 取消已知 active invocation、阻止后续 chunk，并记录 cancelled terminal state。 |

### `authority.handle.*`（3）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `authority.handle.attenuate` | partial | 从父句柄派生子句柄；约束子集验证仍需加固。 |
| `authority.handle.revoke` | partial | 立刻撤销句柄；完整子树传播仍需加固。 |
| `authority.handle.list` | partial | 列出 package 当前持有的 live handles；delegate/lease refresh 尚未完成。 |

### `authority.grant.*` / `authority.decision.*`（4）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `authority.grant.create` | partial | Host-dev 给 human / assistant principal 授予作用域权限，写审计事件。 |
| `authority.grant.revoke` | partial | 撤销作用域权限，写审计事件。 |
| `authority.grant.list` | partial | 列出当前 grants。 |
| `authority.decision.list` | partial | 查询 grant/revoke 审计。 |

### `change.proposal.*`（6）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `change.proposal.create` | partial | 创建需要审批的通用变更。 |
| `change.proposal.get` | partial | 查询 proposal。 |
| `change.proposal.list` | partial | 列出 proposal。 |
| `change.proposal.approve` | partial | 要求 proposal-scoped review authority，标记已审批并写事件。 |
| `change.proposal.reject` | partial | 要求 proposal-scoped review authority，标记已拒绝并写 denied receipt/event。 |
| `change.proposal.apply` | partial | 重新检查 apply 与 required authority；把当前 proposal facade 映射为 Intent/ChangeSet/PolicyDecision/Commit evidence，preflight object/projection operation，并以 CAS 记录 committed/failed/partial receipt。 |

### `object.*`（3）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `object.put` | partial | 存储不透明 asset metadata，阻断 raw secret，写 `object/put`。 |
| `object.get` | partial | 读取 asset record。 |
| `object.list` | partial | 列出 asset records。 |

### `projection.*`（4）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `projection.register` | partial | 注册通用 projection descriptor。 |
| `projection.rebuild` | partial | 基于事件过滤 rebuild 并写 `projection/updated`。 |
| `projection.get` | partial | 读取 projection state。 |
| `projection.list` | partial | 列出 projection。 |

### `host.outbound.*`（6）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `host.outbound.audit` | partial | 查询出站审计与 terminal receipt descriptor；跨执行器视图一致性仍在加固。 |
| `host.outbound.execute` | partial | 受 manifest network/secret_ref 约束的一元 HTTPS 出站；denied/error/success 均产生 receipt。 |
| `host.outbound.stream` | partial | 受约束 SSE/NDJSON/raw 流式出站；terminal completion 产生 receipt。 |
| `host.outbound.websocket.open` | partial | 通过 Host policy、secret resolution、audit 与 streaming event boundary 打开受 Manifest 约束的 WSS connection。 |
| `host.outbound.websocket.send` | partial | 在 caller 与 connection 检查后，向 Host-owned connection 发送 bounded frame。 |
| `host.outbound.websocket.close` | partial | 关闭 Host-owned connection，并发出 terminal completion evidence。 |

Git 安装不是 transport primitive；它属于普通第一方 capability Package `plurora/git-tools-lab`，通过 `host.outbound.execute` 与声明的 filesystem authority 实现。

### `host.target.*` / `host.exec.*` / `host.port.*` / `host.proxy.*`（17）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `host.target.list` | partial | HostAdmin/HostDev only；列出可运行目标。 |
| `host.target.status` | partial | HostAdmin/HostDev only；查询单个目标。 |
| `host.target.register` | partial | HostAdmin/HostDev only；注册受控目标。 |
| `host.target.unregister` | partial | HostAdmin/HostDev only；注销目标。 |
| `host.exec.start` | partial | HostAdmin/HostDev only；通过 host `LocalExecExecutor` 启动受控执行；默认 deny-all，denied/failed terminal path 挂接 receipt。 |
| `host.exec.stop` | partial | HostAdmin/HostDev only；停止已知执行并产生 cancelled/failed/denied receipt。 |
| `host.exec.status` | partial | HostAdmin/HostDev only；返回 runtime 已观察到的状态。live executor 会被主动监测，status/stop/重启竞态复用唯一持久化终态 receipt。 |
| `host.exec.logs` | partial | HostAdmin/HostDev only；读取脱敏日志尾部。 |
| `host.exec.list` | partial | HostAdmin/HostDev only；列出执行记录。 |
| `host.port.lease` | partial | HostAdmin/HostDev only；租用 loopback 端口。 |
| `host.port.release` | partial | HostAdmin/HostDev only；释放端口租约。 |
| `host.port.status` | partial | HostAdmin/HostDev only；查询端口租约。 |
| `host.port.list` | partial | HostAdmin/HostDev only；列出端口租约。 |
| `host.proxy.register` | partial | HostAdmin/HostDev only；注册 HTTP/WebSocket route，upstream 必须引用 active port lease，且 `port_name` 匹配。 |
| `host.proxy.unregister` | partial | HostAdmin/HostDev only；注销 route。 |
| `host.proxy.status` | partial | HostAdmin/HostDev only；查询 route。 |
| `host.proxy.list` | partial | HostAdmin/HostDev only；列出 route。 |


### `host.installation.*`（5）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `host.installation.list` | implemented | `observe` authority；列出由 Host journal 重建的 Installation projection，以及从 exact WorkRevision 校验投影的 Work summary / entrypoints。 |
| `host.installation.get` | implemented | `observe` authority；返回单个 Installation、当前 revision、Work summary 与可选 rollback pointer。 |
| `host.installation.create` | implemented | `installation.manage` + exact Work authority；请求带 typed `work_id`，必须与 canonical CAS `WorkRevision.work_id` 一致；服务在 durable boundary 刷新 current grant，幂等键同 fingerprint 重放返回同一结果。 |
| `host.installation.update` | implemented | `installation.manage` + exact Installation authority；所有 state action（含 Preserve）都在 durable/effect boundary 刷新 current grant；要求期望 revision 并以 CAS 原子切换 active Work/Lock；Reset/Replace 由 Host 在真实 state effect 前签发并持久化 authority evidence 与 decision receipt，失败保留旧 pointer。 |
| `host.installation.remove` | implemented | `installation.manage` + exact Installation authority；要求显式 `keep` 或 `delete` state decision并在 durable/effect boundary 刷新 current grant；只删除 Host-owned state，linked-local source 永不删除。 |

### `host.exposure.*` 与 `host.binding.*`（7）

| 方法 | 状态 | Owner / action / exact resource / typed request → result |
|---|---:|---|
| `host.exposure.list` | implemented | Host；`observe`；可选 provider Installation/Run/Port selector；`ExposureListRequest → ExposureView[]`。 |
| `host.exposure.create` | implemented | Host；`exposure.manage`；provider Installation + Run + export Port；`ExposureCreateRequest → ExposureMutationResult`，包含 Exposure、revision、幂等标记。 |
| `host.exposure.revoke` | implemented | Host；`exposure.manage`；provider Installation + Run + export Port + Exposure；`ExposureRevokeRequest → ExposureMutationResult`，并报告受影响 Binding。 |
| `host.binding.list` | implemented | Host；`observe`；可选 consumer Installation/Run/status selector；`BindingListRequest → BindingView[]`。 |
| `host.binding.candidates` | implemented | Host；`observe`；exact consumer Installation + import Port（可选 launch/runtime Run pin）；`BindingCandidatesRequest → BindingCandidatesResult`，只读、含候选与 gaps。 |
| `host.binding.select` | implemented | Host；`binding.manage`；consumer Installation + import Port + Exposure + provider Installation（可选 exact runtime Run）；`BindingSelectRequest → BindingMutationResult`。 |
| `host.binding.revoke` | implemented | Host；`binding.manage`；consumer Installation + import Port + Exposure + Binding（可选 exact runtime Run）；`BindingRevokeRequest → BindingMutationResult`。 |

### `host.run.*`（5）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `host.run.list` | implemented | `observe` authority；按可见 Installation 与可选状态过滤，列出 Host Run journal projections。 |
| `host.run.get` | implemented | `observe` authority；要求 exact Installation 与 Run selector，返回 Run identity、revision、entrypoint、节点实例和 health。 |
| `host.run.start` | implemented | `run` authority + exact Installation；校验 expected Installation revision 与 entrypoint，Host 在 preflight 通过后生成 RunId。只激活已安装、已验证且唯一匹配 AssemblyLock 的本地 Component；缺失、歧义、unsupported backend 或需要 Realization 时返回结构化 `gaps`，不隐式 build/apply。 |
| `host.run.stop` | implemented | `run` authority + exact Installation 与请求中的 Run child；registry 验证 I/R 归属，要求 expected Run revision 与幂等键，停止该 Run 的激活上下文并写入 terminal event，不卸载全局 Package。 |
| `host.run.status` | implemented | `observe` + exact Installation；返回当前 Installation revision、active Run 概况，以及可选 entrypoint 的零副作用 preflight gaps；结果不授予 authority，start 会重验。 |

### `host.realization.*`（7）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `host.realization.plan` | implemented | `realization.plan` + exact Installation/Target；读取已验证 Work、AssemblyLock、OperationalIntent 与 TargetInventory，纯编译并持久化 content-addressed `RealizationPlan`；不执行 target effect。 |
| `host.realization.apply` | implemented | `realization.apply` + exact Installation/Target/Realization；重验 plan digest、Installation/Target preconditions、逐项风险 approval 与 current authority，然后经同一 typed Target operation path 执行并持久化资源和 receipt。 |
| `host.realization.get` | implemented | `observe` + exact Installation/Realization；返回单个 durable `RealizationRevision`。 |
| `host.realization.list` | implemented | `observe`；按可见 Installation、Realization 与可选 Target 过滤 durable projections。 |
| `host.realization.stop` | implemented | `realization.apply` + exact Installation/Target/Realization；先持久化私有 stopping intent，关闭已记录资源，再提交公开 Stopped；重试续作而不重新猜测 live workspace。 |
| `host.realization.rollback` | implemented | `realization.apply` + exact Installation/Target/current/historic Realization；只读取持久化 historic plan 与 approval，Host 生成 replacement ID；apply/stop 的私有 effect checkpoint 支持安全续作。 |
| `host.realization.reconcile` | implemented | `realization.apply` + exact Installation/Target/Realization；只观察 Target truth，写入 receipt 和结构化 `outcome_unknown` / `recovery_required`，不隐式重放 apply。 |

### `host.*` / `identity.current`（4）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `host.info` | implemented | 返回协议版本、方法、状态和传输标签。 |
| `host.ping` | partial | 预留轻量健康检查。 |
| `host.diagnostics` | partial | 返回包/capability/hook 计数和本地诊断。 |
| `identity.current` | planned | Identity provider 集成预留。 |

### `host.package.audit`（1）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `host.package.audit` | partial | 报告 package declared vs used authority，供 `plurora audit --package <id>` 使用；实际使用追踪仍在扩展。 |

### Surface / extension point / hook（6）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `shell.contribution.list` | partial | 列出 package 声明的 typed surface contributions。 |
| `shell.contribution.describe` | partial | 描述单个 contribution。 |
| `host.surface.bundle.resolve` | partial | HostAdmin/HostDev only；按 package surface contribution 解析可挂载 bundle URL；跨来源一致性仍在加固。 |
| `protocol.extension.list` | implemented | 列出 extension points。 |
| `protocol.extension.describe` | planned | 描述单个 extension point。 |
| `protocol.hook.list` | partial | 列出 hook subscriptions。 |

## 事件类型矩阵（76）

完整 registry 见 [`v1/EVENT_KIND_REGISTRY.md`](v1/EVENT_KIND_REGISTRY.md)。事件 payload schema 位于 `docs/spec/v1/schemas/events/`。事件分组如下：

| 分组 | 数量 | 示例 |
|---|---:|---|
| Context | 3 | `context/opened`、`context/closed`、`context/forked` |
| Package lifecycle | 9 | `host/package.loading`、`.starting`、`.ready`、`.loaded`、`.stopping`、`.stopped`、`.unloaded`、`.degraded`、`.log` |
| Installation lifecycle | 3 | `host/installation.created`、`.updated`、`.removed` |
| Exposure lifecycle | 3 | `host/exposure.created`、`.revoked`、`.expired` |
| Binding lifecycle | 3 | `host/binding.selected`、`.revoked`、`.expired` |
| Run lifecycle | 5 | `host/run.starting`、`.started`、`.stopping`、`.stopped`、`.failed` |
| Realization lifecycle | 7 | `host/realization.planned`、`.applying`、`.active`、`.stopped`、`.failed`、`.rolled_back`、`.reconciled` |
| Capability lifecycle | 3 | `capability/invoked`、`capability/completed`、`capability/failed` |
| Stream lifecycle | 7 | `capability/stream.started`、`.chunk`、`.progress`、`.ended`、`.error`、`.cancelled`、`.timeout` |
| Authority | 3 | `authority/grant.created`、`authority/grant.revoked`、`authority/denied` |
| Change proposal | 5 | `change/proposal.created`、`.approved`、`.rejected`、`.applied`、`.failed` |
| Object / projection | 2 | `object/put`、`projection/updated` |
| Outbound / WebSocket | 8 | `host/outbound.request`、`.denied`、completion event、WebSocket frame |
| Exec | 6 | `host/exec.request`、`.started`、`.completed`、`.failed`、`.stopped`、`.denied` |
| Port | 3 | `host/port.leased`、`.released`、`.denied` |
| Proxy | 3 | `host/proxy.registered`、`.unregistered`、`.denied` |
| Workload adapter | 2 | `host/workload.reconciled`、`host/workload.health` |
| error | 1 | `runtime/error` |

Package-owned event kind 必须以精确 writer Package ID 加 `/` 开头。Registry 中的平台事件只能由 `plurora/runtime` 写入；Package 不能冒充平台事件或其他 Package namespace。

## 能力句柄模型

Manifest 字符串是**authority ceiling**，runtime handle 是**实际 authority**。Package 不能通过伪造字符串提权，必须使用 runtime 在 load/handshake/init 阶段 mint 的 handle。

- `authority.handle.attenuate(parent, constraints)` → 子句柄。
- `authority.handle.revoke(handle)` → 立刻失效。
- `authority.handle.list(package_id)` → 当前持有的全部 live handles。

句柄字段：

- `id`：不可伪造的 runtime-minted identifier。
- `cap_type`：能力种类，如 capability invoke、events read、outbound。
- `cap_version`：句柄语义版本。
- `scope`：package、session、capability、provider、host 等范围。
- `constraints`：方法、host、schema、次数、字节数、deadline 等约束。
- `lease`：过期时间或租约策略。
- `provenance`：谁铸造、为何铸造、对应 manifest 声明。
- `parent`：可选父句柄，用于衰减树与撤销传播。

详见 [`../guides/CAPABILITY_HANDLES.md`](../guides/CAPABILITY_HANDLES.md)。

## 注入模型

每种 entry form 在启动时收到 bindings：

| Entry | 注入方式 | v1 状态 |
|---|---|---:|
| `subprocess` | `package.handshake` 返回/接收 `bindings` 字典，SDK 暴露 `pluroraClient` 与句柄。 | implemented |
| `rust_inproc` | `ComponentEnv` 参数传给 `InprocPackage::init`，包含 runtime bindings。 | implemented |
| `wasm` | WIT resource imports。 | planned |
| `remote` | SPIFFE + Biscuit token 兑换。 | planned |

Bindings 必须只包含调用方被授予的权威。路径 B 包不会收到 v1 能力绑定。

## 效应审计

`plurora audit --package <id>` 和 `host.package.audit` 报告 declared vs used authority 差异。审计输入来自：

1. manifest 声明的 permissions、capabilities、secret_refs、network hosts；
2. runtime mint 与 attenuate 后的 capability handle；
3. `capability.invoked|completed|failed` 与 outbound audit events；
4. permission grants/revokes 与 package lifecycle；
5. Path B 的 `contract_mode: "none"` 标记。

审计报告用于发现未使用声明、声明外使用、权限扩张、过期句柄使用、撤销后使用、未声明 secret_ref、未声明网络目标等问题。详见 [`../guides/CAPABILITY_HANDLES.md`](../guides/CAPABILITY_HANDLES.md) 的审计章节。

## Conformance kit

第三方包可以运行：

```bash
plurora conformance package --contract v1 --path <package>
```

kit 包含 8 个验收检查：manifest parse、contract mode、entry support、bindings/handshake、capability declarations、permission declarations、audit visibility、fixture invocation。输出 PASS/FAIL/SKIP/WARNING 与合规百分比。路径 A 包需要通过适用检查；路径 B 包会跳过能力/权限相关检查，但仍必须自包含、生命周期可观测。

详见 [`../guides/CONFORMANCE_KIT.md`](../guides/CONFORMANCE_KIT.md)。

## SDK 生成

`docs/spec/v1/schemas/` 是单一可信源。SDK 通过三个发行渠道获得：

- npm：`@plurora/contract-sdk`（`sdk/typescript/contract-sdk/`）。
- 工作空间路径：`file:../plurora/sdk/typescript/contract-sdk`。
- 自行生成：读取 `docs/spec/v1/schemas/`，使用任意 codegen 工具。

更多信息见 [`../../sdk/README.md`](../../sdk/README.md)。

## 版本演进策略

详见 [`v1/VERSIONING.md`](v1/VERSIONING.md)。

v1 仅允许 additive 变更：新增可选字段、新增方法、新增事件、新增错误码、新增 schema 均可；删除字段、改变必填性、改变语义、重命名方法或事件属于 breaking change，必须进入 v2 namespace。

## Schema 与错误码

- 方法 schema：`docs/spec/v1/schemas/methods/`（99）。
- 事件 schema：`docs/spec/v1/schemas/events/`（76）。
- 顶层 schema：`docs/spec/v1/schemas/*.schema.json`（39），包含 additive Protocol Commons、component/package-envelope、World Bundle、便携 Work / Assembly 契约、Host-local Installation / Run / Exposure / Binding / Realization wire record，以及 Installation state snapshot、decision receipt 与 authority evidence。
- 错误码：[`v1/ERROR_CODES.md`](v1/ERROR_CODES.md)。
- 事件 registry：[`v1/EVENT_KIND_REGISTRY.md`](v1/EVENT_KIND_REGISTRY.md)。

214 个 schema 必须通过 `cargo run -p plurora-cli --bin validate-schemas`。

## 内容无关不变量

宪法基底与公开契约 runtime 不得要求 `Turn`、`Message`、`PromptFrame`、`ModelCall`、`Agent`、`World`、`Scene`、`Director` 或 `Memory` 等内容形态概念。这些概念由相应 Protocol、Component、Product 或 Client 拥有；Contract V1 仅把它们作为 opaque 的 Package-owned event、object、projection 与 capability 承载。

## 对象契约

### `SessionRecord`

`SessionRecord` 是内容无关的执行上下文。它可以持有身份、标签、活跃包集、principal 范围、状态、时间戳和 metadata。它不得持有消息、回合、提示词、角色、世界、记忆或模型调用。

Session id 只表示当前 runtime 的排序与权限范围，不表示某种产品体验。Protocol 或 Component 可以通过当前 Package writer 的事件 payload、object 或 projection 表达领域状态，但 runtime 只按不透明数据和公开 descriptor 处理。

### `EventEnvelope`

`EventEnvelope` 是 append-only 事实记录。每个 envelope 至少包含 session id、sequence、writer package id、kind、schema version、timestamp、payload 和 metadata。Sequence 在单 session 内单调递增。

Runtime 只校验 owner、authority 与 schema 形状；事件语义属于其语义 owner。

### `PackageManifest`

Manifest 声明 package 身份、entry、contract mode、提供的能力、消费的能力、surface contributions、hooks、extension points、asset/schema 声明、permissions 和 sandbox policy。

## 包依赖（manifest.requires）

`requires` 是 manifest 中的一等 package 依赖声明字段，用于表达“此包需要哪些其他包被解析和安装”。它不同于 `consumes`：`consumes` 声明 capability 需求，`requires` 声明 package 依赖数据。它不是协议方法，也不授予运行时权威；安装器用它解析依赖并写入 lockfile，运行时仍通过 permissions、bindings 与 capability handles 执行授权。

```yaml
requires:
  - id: plurora/model-provider-lab
    source:
      kind: git
      url: https://example.com/plurora/model-provider-lab.git
      ref: v1.2.3
    version: "^1.2"
    minimum_signed_by:
      - "0123456789ABCDEF0123456789ABCDEF01234567"
```

实际安装和解析由 `plurora/install-lab` 处理；宪法基底不负责依赖解析。
详见 [`docs/guides/PACKAGE_INSTALLATION.md`](../guides/PACKAGE_INSTALLATION.md)。

Manifest 是审核与句柄铸造输入，不是运行时权威本身。运行时权威必须通过 bindings 和 capability handles 表达。

### `PackageRecord`

`PackageRecord` 记录 package id、version、entry kind、contract mode、trust level、状态、manifest 摘要、capability/hook/surface 计数与状态时间戳。Record 用于 host diagnostics、package status、lifecycle audit 与 conformance kit。

### `CapabilityDescriptor`

Descriptor 描述 provider-owned capability：id、version、input schema、output schema、streaming、side effects、description 与 metadata。Descriptor 不授予调用权；调用权来自 caller 持有的 handle。

### `HookSubscription`

Hook subscription 来自 Manifest。Runtime 拥有稳定排序、卸载清理，以及当前四个 journal/capability interception point。任意 hook handler 的通用执行尚未完成；见 [`../architecture/EXTENSION_POINTS.md`](../architecture/EXTENSION_POINTS.md)。

### `AssetRecord`

Asset record 是 opaque metadata：id、origin Package、mime、hash、size 与 metadata。Runtime 不解释 asset 内容。

## 权限与拒绝语义

权限检查必须 fail closed。调用方缺少句柄、句柄过期、句柄已撤销、scope 不匹配、schema 不匹配、host policy 不允许、manifest 未声明，均应拒绝。

拒绝应产生结构化错误，并在适用时写入审计事件。错误不能泄漏 raw secret、完整请求体、用户内容或 provider credential。

Host-dev 操作必须在协议上下文中显式标记为 host/dev。匿名 host 调用不能变成 package privilege。

## Namespace 规则

公开方法使用精确的 owner-based dot ID。第一段标明 Substrate、Host、Protocol 或 Shell owner，例如 `context.open`、`host.installation.list`、`change.proposal.apply` 与 `shell.contribution.list`。

平台事件由显式 63 项 registry 定义，只能由 `plurora/runtime` 写入。Package-owned event kind 必须以精确 Package ID 加 `/` 开头；Package capability ID 也使用同一 Package-owned slash namespace 约定。

保留规则：

- 当前 registry 对每个公开方法只暴露一个 wire ID，不提供 alias；
- Package 不能占用公开方法 ID，也不能写入 registry 中的平台事件 kind；
- `plurora/*` 只表示第一方 Package 身份，不授予 runtime authority 或 routing priority；
- method、event 或 schema 的 breaking semantic change 必须进入新的显式版本边界，不能原地改名。

可执行身份与协商规则见 [`CONTRACT_REGISTRY.md`](CONTRACT_REGISTRY.md)。

## Schema 规则

v1 schema 是发布工件。每个方法 schema 描述 request 与 response。每个事件 schema 描述 payload。顶层 schema 描述 manifest、permission、protocol context、capability descriptor 等共享对象。

Schema 变更规则：

1. 可以新增可选字段。
2. 可以新增 enum 值，但调用方必须把未知值当作可恢复扩展处理。
3. 不得删除字段。
4. 不得把可选字段改成必填。
5. 不得改变字段语义。
6. 不得重命名方法、事件或错误码。

## 传输一致性

同一 protocol envelope 可以通过 in-process dispatcher、HTTP `/rpc`、host JSON-RPC stdio 与未来传输承载。传输层不得改变授权语义。

HTTP 与 stdio 可以有不同 framing，但 request id、method、params、可选 contract selection、context、result/error 语义必须一致。显式选择无法满足时必须返回 `unsupported_contract`，不能静默降级。详见 [`CONTRACT_REGISTRY.md`](CONTRACT_REGISTRY.md)。

## Package lifecycle

实现 entry 的包应按顺序进入：loading、starting、ready、loaded。停止时进入 stopping、stopped、unloaded。执行失败或健康丢失时发出 degraded。

Lifecycle events 必须足以让 operator 和 conformance kit 区分：

- manifest 被接受还是被拒绝；
- entry 是否启动；
- handshake 是否完成；
- contract mode 是 `v1` 还是 `none`；
- unload 是否撤销了句柄；
- subprocess stderr 是否被捕获为 log。

## Subprocess 契约

Subprocess stdout 是 JSON-RPC 协议帧，不能写普通日志。日志必须写 stderr。Host 可以捕获 stderr 并生成 package log 事件。

Handshake 必须声明 package id、protocol version、contract mode、可用 capability endpoints 与 bindings 兼容性。路径 A handshake 失败时 package 不应进入 ready。路径 B 可以用更窄 handshake，但必须让 host 判断它是 self-contained。

## Rust in-process 契约

Rust in-process package 只能通过 host catalog 加载。Manifest 声明的 in-process entry 必须能映射到 host 提供的 trait 实现。找不到 catalog entry 时 fail closed。

In-process Package 不享受第一方特权，仍通过 `ComponentEnv`、binding、handle、schema 与 audit 参与 v1。

## WASM 与 remote 预留

WASM 与 remote 是一等 manifest entry form，但执行仍待完成。v1 已为其保留 contract shape：

- WASM 使用 WIT resources 表达 handles。
- Remote 使用 mTLS/SPIFFE 身份和 Biscuit token 表达 attenuated authority。
- 两者必须遵守相同 schema、event、audit 与 namespace 规则。

## 出站执行边界

出站请求只能通过 v1 outbound 原语获得平台管理的网络权威。Manifest 必须声明 host、method、purpose 和所需 `secret_ref`。Host policy 可进一步收紧。

Audit 记录只包含 destination、method、package id、capability id、purpose、redaction state、secret_ref 引用、状态、耗时与计数。Raw body、headers、prompt、response 与 raw secret 不得写入事件。

## Secret reference 契约

包只能传 `secret_ref:<vault>:<key>` 等引用。Host resolver 在运行时解析，解析结果只进入 executor 或 provider adapter，不写回事件、日志、proposal 或 audit。

```yaml
secret_ref:env:OPENAI_API_KEY    # resolved via host env var（allowlisted）
secret_ref:store:OPENAI_API_KEY  # resolved via local encrypted store
secret_ref:installation:OPENAI_API_KEY # resolved via the active Installation store, then policy fallback
```

Installation-backed references resolve from the active Installation store first, then fall back to the platform store only when `secret_policy.allow_platform_fallback` permits it.

store-backed references are resolved via the `StoreSecretResolver` against an age-encrypted file at `~/.plurora/secrets.dat`. See [`docs/guides/SECRET_MANAGEMENT.md`](../guides/SECRET_MANAGEMENT.md).

未声明 secret_ref、解析失败、resolver deny、raw secret 出现在受保护 payload 中，都必须 fail closed。

## Proposal 契约

Proposal 是 approval-gated change，不是内容模型。Runtime 管理 create、approve、reject、apply 与 failed 状态。Approval/apply 边界执行显式 authority 检查，terminal transition 受 compare-and-set 保护；operation payload 仍是 opaque JSON，并接受 raw-secret 与 schema 检查。

当前 apply 支持通用 asset/projection 操作。更广事务、补偿与 revert 属于后续工作。

## Surface 契约

Surface contribution 是 Package 声明的 UI/UX 入口 descriptor。Runtime 保存和列出 descriptor，不渲染 UI，也不解释内容语义。Host shell 决定如何挂载 iframe、bundle 或 native surface。

第一方与第三方 surface 使用同一 descriptor、permission declaration 与 review path。

## Conformance 要求

一个 v1 实现至少需要证明：

1. 99 个方法 schema 可导出。
2. 76 个事件 schema 可验证。
3. 39 个顶层 schema 可验证。
4. 方法 registry 与 dispatcher 一致。
5. capability handle mint/attenuate/revoke/list 行为可测试。
6. invoke instrumentation 生成生命周期事件。
7. bindings 注入覆盖 subprocess 与 rust_inproc。
8. Path B 自包含路径可观察。
9. package audit report 可解释 declared vs used。

## 操作员可观察性

Host operator 应能通过公开方法或 CLI 看见：

- 已加载 packages 与 contract mode；
- 每个 package 的 capabilities、surfaces、hooks；
- live handles 与撤销状态；
- denied permission、outbound、secret、schema 错误；
- Path B package 的 lifecycle 与 logs；
- conformance percentage 与失败原因。

## Canonical reference

长期引用应指向 `PUBLIC_CONTRACT.md`。Contract Registry、error code、versioning rule、event registry 与 `docs/spec/v1/` 下的 schema 是本契约的机器可读补充。

## 附录 A：方法 owner-prefix 计数

| Prefix | Count |
|---|---:|
| `authority.*` | 7 |
| `capability.*` | 5 |
| `change.*` | 6 |
| `context.*` | 6 |
| `host.*` | 59 |
| `identity.*` | 1 |
| `journal.*` | 3 |
| `object.*` | 3 |
| `projection.*` | 4 |
| `protocol.*` | 3 |
| `shell.*` | 2 |

## 附录 B：发布前检查

发布 v1 Host 前，应运行：

```bash
cargo test -p plurora-core
cargo test -p plurora-runtime
cargo test -p plurora-cli
cargo run -p plurora-cli -- conformance
cargo run -p plurora-cli --bin export-schemas
cargo run -p plurora-cli --bin validate-schemas
cargo run -p plurora-cli --bin generate-sdks
```

并对示例路径 A / 路径 B 包运行 package conformance。

## 附录 C：非目标

v1 不承诺：

- 内核提供聊天、agent、模型、世界、记忆或导演语义；
- 任意 subprocess OS 级别网络拦截；
- 生产级 secret vault 集成；
- WASM / remote 执行已完成；
- 市场、包签名网络或依赖解析经济；
- UI framework 或 Studio 私有 API。

这些能力可以由普通 Package、Host policy 或未来 round 提供，但不能破坏本契约的不变量。

## 其他参考

- [`v1/EVENT_KIND_REGISTRY.md`](v1/EVENT_KIND_REGISTRY.md)
- [`v1/ERROR_CODES.md`](v1/ERROR_CODES.md)
- [`v1/VERSIONING.md`](v1/VERSIONING.md)
- [`../guides/CAPABILITY_HANDLES.md`](../guides/CAPABILITY_HANDLES.md)
- [`../guides/CONFORMANCE_KIT.md`](../guides/CONFORMANCE_KIT.md)
- [`../guides/PATH_B_SELF_CONTAINED.md`](../guides/PATH_B_SELF_CONTAINED.md)
