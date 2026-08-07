# 内核 v1 契约

> [English](./KERNEL_V1_CONTRACT.en.md) · [中文](./KERNEL_V1_CONTRACT.md)

本文档是 Plurora 当前平台契约的 v1 版本规范。它定义现行公开边界：方法、事件、错误码、能力句柄、清单声明、schema 与 conformance 期望。任何参与方都可以通过此契约调用平台；任何实现都必须满足相同的 schema 与行为 conformance。`kernel.v1.*` 是兼容名称，不自动等同于长期宪法基底。

v1 的设计目标不是把某种内容形态写进核心机制，而是让组件、安全执行、审计、SDK 与第三方客户端拥有稳定边界。角色、世界、提示词、模型、消息、记忆等内容语义属于相应协议、组件或产品，不属于宪法基底。

## 状态语言

- `implemented`：代码已实现并有测试或 conformance 覆盖。
- `partial`：核心路径已实现，边角情况、传输一致性或生产级策略仍待补。
- `planned`：契约中预留，未实现，调用方不能依赖。

## 路径 A vs 路径 B

v1 契约支持两种参与方式：

- **路径 A**（默认）：包通过 `entry.contract: "v1"` 接受契约约束。Manifest 声明能力、权限与副作用；运行时强制权限；能力调用通过内核铸造的句柄；生命周期与审计事件完整记录。
- **路径 B**：包通过 `entry.contract: "none"` 选择退出契约约束。内核仍托管进程并发出生命周期事件，但不强制能力/权限检查，也不会为该包创建 v1 能力绑定。

两种路径都是平台一等公民。路径 A 面向需要内核能力、网络、secret、审计和 SDK 的集成包；路径 B 面向自包含应用、迁移期工具或不需要平台能力的第三方进程。

## 协议方法矩阵（80）

完整请求/响应 schema 位于 `docs/spec/v1/schemas/methods/`。方法名是稳定公开 API；v1 只允许 additive 变更。

### `kernel.v1.session.*`（6）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `kernel.v1.session.open` | implemented | 开启内容无关 session，写入 `kernel/v1/session.opened`。 |
| `kernel.v1.session.close` | implemented | 关闭 session，写入 `kernel/v1/session.closed`。 |
| `kernel.v1.session.fork` | partial | 从父 session 与 sequence 创建 branch lineage，不解释内容。 |
| `kernel.v1.session.branch.list` | partial | 列出与 session 相关的 branch 记录。 |
| `kernel.v1.session.get` | partial | 查询单个 session；行为与错误契约仍在加固。 |
| `kernel.v1.session.list` | planned | 预留 host 管理列表。 |

### `kernel.v1.event.*`（3）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `kernel.v1.event.append` | implemented | 对非内核 writer 强制 namespace 和 `events.append`。 |
| `kernel.v1.event.list` | partial | 按 session 列出事件，支持 sequence、limit、kind、writer 过滤与权限门控；跨后端一致性仍在加固。 |
| `kernel.v1.event.subscribe` | planned | SSE replay/tail 路由已存在；公开 method dispatch 与 package-principal subscribe 权限尚未落地。 |

### `kernel.v1.package.*`（7）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `kernel.v1.package.load` | partial | 验证 manifest、host policy、路径 A/路径 B、入口约束，注册声明并发出生命周期事件；部分 entry form 仍是占位。 |
| `kernel.v1.package.unload` | partial | 停止执行、移除注册、撤销运行时句柄、发出停止/卸载事件；不同 entry form 的完整对称性仍在加固。 |
| `kernel.v1.package.list` | implemented | 列出内存 package record。 |
| `kernel.v1.package.status` | implemented | 返回单个 package record。 |
| `kernel.v1.package.restart` | partial | 已支持 subprocess restart；其他 entry 形式按策略拒绝。 |
| `kernel.v1.package.logs` | partial | 捕获 subprocess stderr；stdout 保留给 JSON-RPC 帧。 |
| `kernel.v1.package.describe` | planned | 可由 status manifest 派生，公开方法预留。 |

### `kernel.v1.capability.*`（5）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `kernel.v1.capability.discover` | implemented | 列出注册的 capability descriptor。 |
| `kernel.v1.capability.describe` | planned | 预留 descriptor 单项查询。 |
| `kernel.v1.capability.invoke` | partial | 用调用者上下文与 capability handle 强制权限；验证 schema；completed/failed 终态挂接 EffectReceipt；支持 recorded replay 与 branch re-execute；跨 entry/transport 一致性仍在加固。 |
| `kernel.v1.capability.stream` / `cancel` | partial | 流式生命周期、取消、超时与事件已存在，ended/error/cancelled/timeout 产生可区分 terminal receipt；跨传输一致性继续加固。 |

### `kernel.v1.cap.*`（3）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `kernel.v1.cap.attenuate` | partial | 从父句柄派生子句柄；约束子集验证仍需加固。 |
| `kernel.v1.cap.revoke` | partial | 立刻撤销句柄；完整子树传播仍需加固。 |
| `kernel.v1.cap.list_for` | partial | 列出 package 当前持有的 live handles；delegate/lease refresh 尚未完成。 |

### `kernel.v1.permission.*`（4）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `kernel.v1.permission.grant` | partial | Host-dev 给 human / assistant principal 授予作用域权限，写审计事件。 |
| `kernel.v1.permission.revoke` | partial | 撤销作用域权限，写审计事件。 |
| `kernel.v1.permission.list` | partial | 列出当前 grants。 |
| `kernel.v1.permission.audit` | partial | 查询 grant/revoke 审计。 |

### `kernel.v1.proposal.*`（6）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `kernel.v1.proposal.create` | partial | 创建需要审批的通用变更。 |
| `kernel.v1.proposal.get` | partial | 查询 proposal。 |
| `kernel.v1.proposal.list` | partial | 列出 proposal。 |
| `kernel.v1.proposal.approve` | partial | 要求 proposal-scoped review authority，标记已审批并写事件。 |
| `kernel.v1.proposal.reject` | partial | 要求 proposal-scoped review authority，标记已拒绝并写 denied receipt/event。 |
| `kernel.v1.proposal.apply` | partial | 重新检查 apply 与 required authority；旧 Proposal 适配为 Intent/ChangeSet/PolicyDecision/Commit，preflight 后应用 asset/projection 操作，并以 CAS 记录 operation 与 committed/failed/partial receipt。 |

### `kernel.v1.asset.*`（3）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `kernel.v1.asset.put` | partial | 存储不透明 asset metadata，阻断 raw secret，写 `kernel/v1/asset.put`。 |
| `kernel.v1.asset.get` | partial | 读取 asset record。 |
| `kernel.v1.asset.list` | partial | 列出 asset records。 |

### `kernel.v1.projection.*`（4）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `kernel.v1.projection.register` | partial | 注册通用 projection descriptor。 |
| `kernel.v1.projection.rebuild` | partial | 基于事件过滤 rebuild 并写 `kernel/v1/projection.updated`。 |
| `kernel.v1.projection.get` | partial | 读取 projection state。 |
| `kernel.v1.projection.list` | partial | 列出 projection。 |

### `kernel.v1.outbound.*`（6）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `kernel.v1.outbound.audit` | partial | 查询出站审计与 terminal receipt descriptor；跨执行器视图一致性仍在加固。 |
| `kernel.v1.outbound.execute` | partial | 受 manifest network/secret_ref 约束的一元 HTTPS 出站；denied/error/success 均产生 receipt。 |
| `kernel.v1.outbound.stream` | partial | 受约束 SSE/NDJSON/raw 流式出站；terminal completion 产生 receipt。 |
| `kernel.v1.outbound.websocket.*` | partial | 受约束 WSS open/send/close；连接生命周期与事件覆盖仍在加固。 |

Git 安装不是内核传输；未来由普通官方能力包 `official/git-tools-lab` 通过 `kernel.v1.outbound.execute` 与 `permissions.filesystem.write` 实现。

### `kernel.v1.target.*` / `exec.*` / `port.*` / `proxy.*`（17）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `kernel.v1.target.list` | partial | HostAdmin/HostDev only；列出可运行目标。 |
| `kernel.v1.target.status` | partial | HostAdmin/HostDev only；查询单个目标。 |
| `kernel.v1.target.register` | partial | HostAdmin/HostDev only；注册受控目标。 |
| `kernel.v1.target.unregister` | partial | HostAdmin/HostDev only；注销目标。 |
| `kernel.v1.exec.start` | partial | HostAdmin/HostDev only；通过 host `LocalExecExecutor` 启动受控执行；默认 deny-all，denied/failed terminal path 挂接 receipt。 |
| `kernel.v1.exec.stop` | partial | HostAdmin/HostDev only；停止已知执行并产生 cancelled/failed/denied receipt。 |
| `kernel.v1.exec.status` | partial | HostAdmin/HostDev only；返回 runtime 已观察到的状态。live executor 会被主动监测，status/stop/重启竞态复用唯一持久化终态 receipt。 |
| `kernel.v1.exec.logs` | partial | HostAdmin/HostDev only；读取脱敏日志尾部。 |
| `kernel.v1.exec.list` | partial | HostAdmin/HostDev only；列出执行记录。 |
| `kernel.v1.port.lease` | partial | HostAdmin/HostDev only；租用 loopback 端口。 |
| `kernel.v1.port.release` | partial | HostAdmin/HostDev only；释放端口租约。 |
| `kernel.v1.port.status` | partial | HostAdmin/HostDev only；查询端口租约。 |
| `kernel.v1.port.list` | partial | HostAdmin/HostDev only；列出端口租约。 |
| `kernel.v1.proxy.register` | partial | HostAdmin/HostDev only；注册 HTTP/WebSocket route，upstream 必须引用 active port lease，且 `port_name` 匹配。 |
| `kernel.v1.proxy.unregister` | partial | HostAdmin/HostDev only；注销 route。 |
| `kernel.v1.proxy.status` | partial | HostAdmin/HostDev only；查询 route。 |
| `kernel.v1.proxy.list` | partial | HostAdmin/HostDev only；列出 route。 |


### `kernel.v1.project.*`（5）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `kernel.v1.project.list` | implemented | HostAdmin/HostDev only；列出已安装项目与状态。 |
| `kernel.v1.project.get` | implemented | HostAdmin/HostDev only；返回单个项目的完整 descriptor 与 registry 记录；Running 时包含 `running_session_id`。 |
| `kernel.v1.project.start` | implemented | HostAdmin/HostDev only；把项目从 Installed/Stopped 切到 Running、打开项目 session、返回 `session_id` 与 `already_running`，并发出生命周期事件。 |
| `kernel.v1.project.stop` | implemented | HostAdmin/HostDev only；停止 Running 项目并发出生命周期事件。 |
| `kernel.v1.project.status` | implemented | HostAdmin/HostDev only；返回项目状态、最近错误；Running 时包含 `running_session_id`。 |

### `kernel.v1.host.*`（4）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `kernel.v1.host.info` | implemented | 返回协议版本、方法、状态和传输标签。 |
| `kernel.v1.host.ping` | partial | 预留轻量健康检查。 |
| `kernel.v1.host.diagnostics` | partial | 返回包/capability/hook 计数和本地诊断。 |
| `kernel.v1.host.principal` | planned | Identity provider 集成预留。 |

### `kernel.v1.audit.*`（1）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `kernel.v1.audit.package` | partial | 报告 package declared vs used authority，供 `plurora audit --package <id>` 使用；实际使用追踪仍在扩展。 |

### Surface / extension point / hook（6）

| 方法 | 状态 | 契约 |
|---|---:|---|
| `kernel.v1.surface.contribution.list` | partial | 列出 package 声明的 typed surface contributions。 |
| `kernel.v1.surface.contribution.describe` | partial | 描述单个 contribution。 |
| `kernel.v1.surface.resolve_bundle` | partial | HostAdmin/HostDev only；按 surface contribution / project dev path / installed project 解析可挂载 bundle URL；跨来源一致性仍在加固。 |
| `kernel.v1.extension_point.list` | implemented | 列出 extension points。 |
| `kernel.v1.extension_point.describe` | planned | 描述单个 extension point。 |
| `kernel.v1.hook.list` | partial | 列出 hook subscriptions。 |

## 事件类型矩阵（59）

完整 registry 见 [`v1/EVENT_KIND_REGISTRY.md`](v1/EVENT_KIND_REGISTRY.md)。事件 payload schema 位于 `docs/spec/v1/schemas/events/`。事件分组如下：

| 分组 | 数量 | 示例 |
|---|---:|---|
| session | 3 | `kernel/v1/session.opened`、`.closed`、`.forked` |
| package lifecycle | 9 | `loading`、`starting`、`ready`、`loaded`、`stopping`、`stopped`、`unloaded`、`degraded`、`log` |
| project lifecycle | 4 | `project.installed`、`.started`、`.stopped`、`.uninstalled` |
| capability lifecycle | 3 | `capability.invoked`、`.completed`、`.failed` |
| stream lifecycle | 7 | `stream.started`、`.chunk`、`.progress`、`.ended`、`.error`、`.cancelled`、`.timeout` |
| permissions | 3 | `permission.granted`、`.revoked`、`.denied` |
| proposals | 5 | `proposal.created`、`.approved`、`.rejected`、`.applied`、`.failed` |
| assets / projections | 2 | `asset.put`、`projection.updated` |
| outbound / websocket | 8 | `outbound.request`、`.denied`、completion events、websocket frames |
| exec | 6 | `exec.request`、`.started`、`.completed`、`.failed`、`.stopped`、`.denied` |
| port | 3 | `port.leased`、`.released`、`.denied` |
| proxy | 3 | `proxy.registered`、`.unregistered`、`.denied` |
| deployment | 2 | `deployment.reconciled`、`deployment.health` |
| error | 1 | `kernel/v1/error` |

非内核事件类型必须以 writer package id 加 `/` 开头。内核必须拒绝包写入 `kernel/v1/...` 或其他包 namespace。

## 能力句柄模型

Manifest 声明的字符串是**权限上限**；运行时句柄是**实际权威**。包不能通过伪造字符串获得权威，必须使用内核在 load/handshake/init 阶段注入的 handle。

- `kernel.v1.cap.attenuate(parent, constraints)` → 子句柄。
- `kernel.v1.cap.revoke(handle)` → 立刻失效。
- `kernel.v1.cap.list_for(package_id)` → 当前持有的全部 live handles。

句柄字段：

- `id`：内核铸造的不可伪造标识。
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
| `subprocess` | `package.handshake` 返回/接收 `bindings` 字典，SDK 暴露 `kernelClient` 与句柄。 | implemented |
| `rust_inproc` | `KernelEnv` 参数传给 `InprocPackage::init`，包含 runtime bindings。 | implemented |
| `wasm` | WIT resource imports。 | planned |
| `remote` | SPIFFE + Biscuit token 兑换。 | planned |

Bindings 必须只包含调用方被授予的权威。路径 B 包不会收到 v1 能力绑定。

## 效应审计

`plurora audit --package <id>` 和 `kernel.v1.audit.package` 报告 declared vs used authority 差异。审计输入来自：

1. manifest 声明的 permissions、capabilities、secret_refs、network hosts；
2. 内核铸造与衰减的 capability handles；
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

- 方法 schema：`docs/spec/v1/schemas/methods/`（80）。
- 事件 schema：`docs/spec/v1/schemas/events/`（59）。
- 顶层 schema：`docs/spec/v1/schemas/*.schema.json`（22），包含 additive Protocol Commons、component/package-envelope、World Bundle 与 protocol-response descriptor。
- 错误码：[`v1/ERROR_CODES.md`](v1/ERROR_CODES.md)。
- 事件 registry：[`v1/EVENT_KIND_REGISTRY.md`](v1/EVENT_KIND_REGISTRY.md)。

161 个 schema 必须通过 `cargo run -p plurora-cli --bin validate-schemas`。

## 内容无关不变量

当前 kernel crate 不得定义或要求内容形态概念，如 `Turn`、`Message`、`PromptFrame`、`ModelCall`、`Agent`、`World`、`Scene`、`Director` 或 `Memory`。此类概念由相应 Protocol、Component、Product 或 Client 拥有，不属于宪法基底；Contract V1 可以通过 Package writer 的事件、projection 和 capability 承载它们。

## 对象契约

### `KernelSession`

`KernelSession` 是内容无关的执行上下文。它可以持有身份、标签、活跃包集、principal 范围、状态、时间戳和 metadata。它不得持有消息、回合、提示词、角色、世界、记忆或模型调用。

Session id 只表示当前 runtime 的排序与权限范围，不表示某种产品体验。Protocol 或 Component 可以通过当前 Package writer 的事件 payload、object 或 projection 表达领域状态，但 runtime 只按不透明数据和公开 descriptor 处理。

### `EventEnvelope`

`EventEnvelope` 是 append-only 事实记录。每个 envelope 至少包含 session id、sequence、writer package id、kind、schema version、timestamp、payload 和 metadata。Sequence 在单 session 内单调递增。

内核只验证 namespace、权限和 schema 形状。事件语义属于 writer package。

### `PackageManifest`

Manifest 声明 package 身份、entry、contract mode、提供的能力、消费的能力、surface contributions、hooks、extension points、asset/schema 声明、permissions 和 sandbox policy。

## 包依赖（manifest.requires）

`requires` 是 manifest 中的一等 package 依赖声明字段，用于表达“此包需要哪些其他包被解析和安装”。它不同于 `consumes`：`consumes` 声明 capability 需求，`requires` 声明 package 依赖数据。它不是协议方法，也不授予运行时权威；安装器用它解析依赖并写入 lockfile，运行时仍通过 permissions、bindings 与 capability handles 执行授权。

```yaml
requires:
  - id: official/model-provider-lab
    source:
      kind: git
      url: https://example.com/plurora/model-provider-lab.git
      ref: v1.2.3
    version: "^1.2"
    minimum_signed_by:
      - "0123456789ABCDEF0123456789ABCDEF01234567"
```

实际安装/解析由 `official/install-lab` 负责，内核不参与依赖解析。
详见 [`docs/guides/PACKAGE_INSTALLATION.md`](../guides/PACKAGE_INSTALLATION.md)。

Manifest 是审核与句柄铸造输入，不是运行时权威本身。运行时权威必须通过 bindings 和 capability handles 表达。

### `PackageRecord`

`PackageRecord` 记录 package id、version、entry kind、contract mode、trust level、状态、manifest 摘要、capability/hook/surface 计数与状态时间戳。Record 用于 host diagnostics、package status、lifecycle audit 与 conformance kit。

### `CapabilityDescriptor`

Descriptor 描述 provider-owned capability：id、version、input schema、output schema、streaming、side effects、description 与 metadata。Descriptor 不授予调用权；调用权来自 caller 持有的 handle。

### `HookSubscription`

Hook subscription 来自 manifest。内核负责排序、卸载清理和事件/能力生命周期分发。Hook handler 仍必须通过普通 capability 和权限边界执行。

### `AssetRecord`

Asset record 是不透明 metadata：id、origin package、mime、hash、size、metadata。内核不解释 asset 内容。Content-addressed blob 存储与 package-principal asset 权限是后续底座项。

## 权限与拒绝语义

权限检查必须 fail closed。调用方缺少句柄、句柄过期、句柄已撤销、scope 不匹配、schema 不匹配、host policy 不允许、manifest 未声明，均应拒绝。

拒绝应产生结构化错误，并在适用时写入审计事件。错误不能泄漏 raw secret、完整请求体、用户内容或 provider credential。

Host-dev 操作必须在协议上下文中显式标记为 host/dev。匿名 host 调用不能变成 package privilege。

## 命名空间规则

协议方法使用 `kernel.v1.<namespace>.<name>`。内核事件使用 `kernel/v1/<kind>`。包事件必须以 package id 加 `/` 开头。

保留规则：

- `kernel.v1.*` 方法只属于内核。
- `kernel/v1/*` 事件只由内核写入。
- `kernel.v2.*` 与 `kernel/v2/*` 留给 breaking changes。
- 包不得声明看似内核 namespace 的 capability id。

分层迁移新增的 Experimental canonical ID 与 legacy alias 由
[`CONTRACT_REGISTRY.md`](CONTRACT_REGISTRY.md) 集中管理；它们不删除或重命名任何
`kernel.v1.*` v1 入口。

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

In-process 包不享受官方特权。它仍通过 `KernelEnv`、bindings、handles、schema 和 audit 参与 v1。

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
secret_ref:project:OPENAI_API_KEY # resolved via project store, then policy fallback
```

Project-backed references resolve from the active project store first, then fall back to platform store when `secret_policy.fallback_to_platform` allows it and the key is not listed in `require_per_project`.

store-backed references are resolved via the `StoreSecretResolver` against an age-encrypted file at `~/.plurora/secrets.dat`. See [`docs/guides/SECRET_MANAGEMENT.md`](../guides/SECRET_MANAGEMENT.md).

未声明 secret_ref、解析失败、resolver deny、raw secret 出现在受保护 payload 中，都必须 fail closed。

## Proposal 契约

Proposal 是 approval-gated change，不是内容模型。内核只管理 lifecycle：create、approve、reject、apply、failed。approval/apply 边界执行显式 authority 检查，终态转换受 compare-and-set 保护；Operation payload 仍是不透明 JSON，但 raw secret scanning 与 schema 基本形状必须执行。

当前 apply 支持通用 asset/projection 操作。更广事务、补偿与 revert 属于后续工作。

## Surface 契约

Surface contribution 是 package 声明的 UI/UX 入口描述符。内核保存和列出 descriptor，不渲染 UI、不解释内容语义。Host shell 决定如何挂载 iframe、bundle 或 native surface。

官方 surface 与第三方 surface 使用同一 descriptor、同一权限声明、同一审核路径。

## Conformance 要求

一个 v1 实现至少需要证明：

1. 80 个方法 schema 可导出。
2. 59 个事件 schema 可验证。
3. 22 个顶层 schema 可验证。
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

## 与旧文档的关系

旧的 alpha 契约已被本文件取代。所有长期引用应指向 `KERNEL_V1_CONTRACT.md`。`docs/spec/v1/` 下的 registry、error codes、versioning 与 schemas 是本契约的机器可读补充。

## 附录 A：方法 namespace 计数

| Namespace | Count |
|---|---:|
| `kernel.v1.session.*` | 6 |
| `kernel.v1.event.*` | 3 |
| `kernel.v1.package.*` | 7 |
| `kernel.v1.capability.*` | 5 |
| `kernel.v1.cap.*` | 3 |
| `kernel.v1.permission.*` | 4 |
| `kernel.v1.proposal.*` | 6 |
| `kernel.v1.asset.*` | 3 |
| `kernel.v1.projection.*` | 4 |
| `kernel.v1.outbound.*` | 6 |
| `kernel.v1.target.*` | 4 |
| `kernel.v1.exec.*` | 5 |
| `kernel.v1.port.*` | 4 |
| `kernel.v1.proxy.*` | 4 |
| `kernel.v1.project.*` | 5 |
| `kernel.v1.host.*` | 4 |
| `kernel.v1.audit.*` | 1 |
| `kernel.v1.surface.*` | 3 |
| `kernel.v1.extension_point.*` | 2 |
| `kernel.v1.hook.*` | 1 |

## 附录 B：发布前检查

发布 v1 兼容 host 前，应运行：

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

这些能力可以由普通包、host policy 或未来 round 提供，但不能破坏本契约的不变量。

## 其他参考

- [`v1/EVENT_KIND_REGISTRY.md`](v1/EVENT_KIND_REGISTRY.md)
- [`v1/ERROR_CODES.md`](v1/ERROR_CODES.md)
- [`v1/VERSIONING.md`](v1/VERSIONING.md)
- [`../guides/CAPABILITY_HANDLES.md`](../guides/CAPABILITY_HANDLES.md)
- [`../guides/CONFORMANCE_KIT.md`](../guides/CONFORMANCE_KIT.md)
- [`../guides/PATH_B_SELF_CONTAINED.md`](../guides/PATH_B_SELF_CONTAINED.md)
