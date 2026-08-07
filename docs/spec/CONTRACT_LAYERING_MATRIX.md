# Contract 分层矩阵（候选）

> [English](./CONTRACT_LAYERING_MATRIX.en.md) · [中文](./CONTRACT_LAYERING_MATRIX.md)

> 状态：候选分类。本文不改变当前 `platform.*` 的运行行为。
> 当前规范仍见 [`KERNEL_V1_CONTRACT.md`](KERNEL_V1_CONTRACT.md)；目标原则见
> [`CONSTITUTION_V2.md`](../architecture/CONSTITUTION_V2.md)。

## 目的

当前 Contract V1 同时承载宪法机制、宿主控制、部署、协议语义和产品 shell。本文逐项回答：

1. 这个合同目前属于谁；
2. 长期应该由哪一层拥有；
3. 是保留、迁移、拆分还是替换；
4. `platform.*` 客户端如何通过兼容层继续工作。

本文中的目标名词是 owner/concept，不是已经冻结的最终 wire method ID。最终 namespace 在实现兼容路由时确定。

## 层与处置代码

| 代码 | 层 | 责任 |
|---|---|---|
| `S` | Constitutional Substrate | 身份、authority、对象、journal、调用、流、事务、receipt |
| `H` | Host Control Plane | 本机安装、进程、端口、代理、secret、部署、诊断 |
| `C` | Protocol Commons | 共享语义、状态机、change workflow、projection 等可演化协议 |
| `P` | Shell / Product Profile | Home、Forge、surface slot、bundle 挂载与交互映射 |
| `X` | Split | 当前合同混合多层职责，必须拆开 |
| `L` | Legacy Adapter | 只保留旧合同读取/转换，不再增加新语义 |

处置：

- **保留：** 语义属于目标层，仅需独立 namespace 与 conformance。
- **强化：** owner 与对象模型基本正确，但必须补足安全、审计或可移植性保证。
- **重塑：** 目标能力保留，但对象模型或边界需要泛化。
- **迁移：** 行为基本保留，owner 从 kernel 移到 host/protocol/shell。
- **拆分：** 一个旧方法拆成多个 owner 下的操作。
- **替换：** 旧抽象只通过 adapter 映射到新模型。

## 当前事实基线

- 代码中有 80 个 `PlatformMethod`、80 个 method schema。
- 代码、schema 与 `EVENT_KIND_REGISTRY.md` 均有 59 个 kernel event，包含 `host/deployment.health`。
- 有 22 个顶层 schema，覆盖 contract selection、artifact descriptor、EffectReceipt、Change primitives、protocol descriptor、component/package envelope/composition lock、World Bundle/World Head/journal range，以及 additive transport diagnostics 的 `protocol-response.schema.json`。
- `PlatformMethod::status()`、Contract 文档状态和 actual dispatch 的已知漂移已对齐，并由测试约束。
- Experimental method contract registry、集中 alias 解析、显式 profile/version 协商与 identity adapter 已落地；Host Control Plane、host bundle resolver、Shell contribution、Change/Proposal 与 Projection 当前发布 36 条 canonical/legacy 双栈。
- Experimental Protocol Commons 注册表已发布 Change、Shell Default 与 World Bundle descriptor，在 dispatch 前协商显式协议/Profile，并分离 protocol、implementation 与 package 报告；具体 World Bundle archive 与五条 portability 向量现在支撑 `plurora.runtime.world-bundle` implementation claim。
- Web 生产调用已切换到 canonical method ID；生成 SDK 从 schema metadata 生成 canonical client 与显式 legacy wrapper，以队列保留 transport diagnostics，并在生成前校验所有 wire ID、函数名与 operation ID 全局唯一。

因此迁移的第一要求不是删除旧代码，而是建立可测试的兼容路由。

## 80 个方法

### Session 与 journal（9）

| 当前方法 | 代码状态 | 目标层 | 处置 | 目标概念与兼容行为 |
|---|---:|---:|---|---|
| `context.open` | implemented | `S` | 重塑 | 打开通用 execution/journal scope；旧名映射到 `context.open` |
| `context.close` | implemented | `S` | 重塑 | 关闭 scope 并冻结写入；保留历史可读性 |
| `context.fork` | partial | `S` | 重塑 | 泛化为从 head/sequence 创建 causal branch |
| `context.branch.list` | partial | `S` | 重塑 | 查询 lineage/head，而不是绑定产品 World 语义 |
| `context.get` | partial | `S` | 保留 | 查询通用 scope metadata；Contract 文档状态需与代码对齐 |
| `context.list` | planned | `S` | 保留 | substrate scope 查询；实现前保持 Experimental |
| `journal.append` | implemented | `S` | 重塑 | `journal.append`；payload 可引用 content-addressed objects |
| `journal.list` | partial | `S` | 保留 | `journal.list`；保留稳定 sequence 分页 |
| `journal.subscribe` | planned | `S` | 保留 | `journal.subscribe`；SSE 路由与 method 语义统一 |

### Package 与 component lifecycle（7）

| 当前方法 | 代码状态 | 目标层 | 处置 | 目标概念与兼容行为 |
|---|---:|---:|---|---|
| `host.package.load` | partial | `X` | 拆分 | `H` 解析 package/artifact；`S` 激活 component instance |
| `host.package.unload` | partial | `S` | 重塑 | 停止 component instance；package envelope 不再是运行时本体 |
| `host.package.restart` | partial | `S` | 重塑 | 重启 component instance；按 trust class 明确支持范围 |
| `host.package.logs` | partial | `H` | 迁移 | host observability；日志不是 substrate 事实来源 |
| `host.package.list` | implemented | `X` | 拆分 | `H` artifact/package inventory + `S` active component list |
| `host.package.status` | implemented | `X` | 拆分 | envelope 安装状态与 component 运行状态分别查询 |
| `host.package.describe` | planned | `X` | 拆分 | artifact descriptor、component descriptor、protocol claims 分开 |

### Project（5）

| 当前方法 | 代码状态 | 目标层 | 处置 | 目标概念与兼容行为 |
|---|---:|---:|---|---|
| `host.project.list` | implemented | `H` | 迁移 | host project/installation registry；不进入 substrate |
| `host.project.get` | implemented | `H` | 迁移 | host-owned project descriptor |
| `host.project.start` | implemented | `H` | 迁移 | host 编排组件、scope 与 shell entry；旧名走 adapter |
| `host.project.stop` | implemented | `H` | 迁移 | host 生命周期控制 |
| `host.project.status` | implemented | `H` | 迁移 | host 状态与失败诊断 |

### Target / exec / port / proxy（17）

| 当前方法 | 代码状态 | 目标层 | 处置 | 目标概念与兼容行为 |
|---|---:|---:|---|---|
| `host.target.list` | partial | `H` | 迁移 | `host.target.list` |
| `host.target.status` | partial | `H` | 迁移 | `host.target.status` |
| `host.target.register` | partial | `H` | 迁移 | `host.target.register` |
| `host.target.unregister` | partial | `H` | 迁移 | `host.target.unregister` |
| `host.exec.start` | partial | `H` | 迁移 | `host.exec.start`；授权与 receipt 仍由 `S` 保证 |
| `host.exec.stop` | partial | `H` | 迁移 | `host.exec.stop` |
| `host.exec.status` | partial | `H` | 迁移 | `host.exec.status` |
| `host.exec.logs` | partial | `H` | 迁移 | `host.exec.logs`，保持脱敏 |
| `host.exec.list` | partial | `H` | 迁移 | `host.exec.list` |
| `host.port.lease` | partial | `H` | 迁移 | `host.port.lease`；authority handle 由 `S` 提供 |
| `host.port.release` | partial | `H` | 迁移 | `host.port.release` |
| `host.port.status` | partial | `H` | 迁移 | `host.port.status` |
| `host.port.list` | partial | `H` | 迁移 | `host.port.list` |
| `host.proxy.register` | partial | `H` | 迁移 | `host.proxy.register` |
| `host.proxy.unregister` | partial | `H` | 迁移 | `host.proxy.unregister` |
| `host.proxy.status` | partial | `H` | 迁移 | `host.proxy.status` |
| `host.proxy.list` | partial | `H` | 迁移 | `host.proxy.list` |

### Capability 与 authority handle（8）

| 当前方法 | 代码状态 | 目标层 | 处置 | 目标概念与兼容行为 |
|---|---:|---:|---|---|
| `capability.discover` | implemented | `S` | 重塑 | 发现 component exports + protocol claims，而非只看 package provider |
| `capability.describe` | planned | `S` | 重塑 | 描述 export、协议、schema 与 trust/conformance claims |
| `capability.invoke` | partial | `S` | 保留 | substrate invocation；修正 status 与 Contract 漂移 |
| `capability.stream` | partial | `S` | 保留 | substrate streaming invocation |
| `capability.cancel` | partial | `S` | 保留 | 统一取消、deadline 与 terminal receipt |
| `authority.handle.attenuate` | partial | `S` | 强化 | 验证衰减必须是约束子集，不允许扩大 authority |
| `authority.handle.revoke` | partial | `S` | 强化 | 支持子树撤销与撤销 receipt |
| `authority.handle.list` | partial | `S` | 强化 | principal-gated authority introspection；补 delegate/lease refresh |

### Extension point 与 hook（3）

| 当前方法 | 代码状态 | 目标层 | 处置 | 目标概念与兼容行为 |
|---|---:|---:|---|---|
| `protocol.extension.list` | implemented | `C` | 迁移 | protocol registry 查询；extension point 语义由协议拥有 |
| `protocol.extension.describe` | planned | `C` | 迁移 | protocol descriptor / extension contract |
| `protocol.hook.list` | partial | `C` | 迁移 | protocol subscription registry；host 可提供运行诊断视图 |

### Asset 与 projection（7）

| 当前方法 | 代码状态 | 目标层 | 处置 | 目标概念与兼容行为 |
|---|---:|---:|---|---|
| `object.put` | partial | `S` | 替换 | `object.put` / `artifact.commit`，摘要成为身份 |
| `object.get` | partial | `S` | 替换 | 通过 descriptor/digest 获取并验证内容 |
| `object.list` | partial | `H` | 迁移 | host object index；substrate 不保证全局可枚举 |
| `projection.register` | partial | `C` | 迁移 | projection protocol 注册派生视图 |
| `projection.rebuild` | partial | `C` | 迁移 | projection protocol 的 rebuild 行为 |
| `projection.get` | partial | `C` | 迁移 | projection profile 查询 |
| `projection.list` | partial | `C` | 迁移 | projection registry 查询 |

### Host（4）

| 当前方法 | 代码状态 | 目标层 | 处置 | 目标概念与兼容行为 |
|---|---:|---:|---|---|
| `host.info` | implemented | `H` | 强化 | host info + supported contract layers、versions、profiles、aliases |
| `host.ping` | partial | `H` | 迁移 | 轻量 host health；不属于 substrate |
| `host.diagnostics` | partial | `H` | 迁移 | host diagnostics，保持路径和 secret 脱敏 |
| `identity.current` | planned | `S` | 重塑 | authenticated principal/context introspection |

### Permission、audit 与 change workflow（11）

| 当前方法 | 代码状态 | 目标层 | 处置 | 目标概念与兼容行为 |
|---|---:|---:|---|---|
| `authority.grant.create` | partial | `S` | 重塑 | authority mint/delegate + PolicyDecision |
| `authority.grant.revoke` | partial | `S` | 重塑 | authority revoke，产生 receipt |
| `authority.grant.list` | partial | `S` | 重塑 | 查询 principal 当前 authority，而非字符串 grant 列表 |
| `authority.decision.list` | partial | `S` | 替换 | authority decision/receipt 查询 |
| `host.package.audit` | partial | `X` | 替换 | `S` authority/effect audit + `H` artifact declared-vs-used report |
| `change.proposal.create` | partial | `C` | 替换 | Change protocol：Intent / ChangeSet 创建 |
| `change.proposal.get` | partial | `C` | 替换 | Change protocol 查询 |
| `change.proposal.list` | partial | `C` | 替换 | Change protocol 索引 |
| `change.proposal.approve` | partial | `C` | 替换 | PolicyDecision / approval profile |
| `change.proposal.reject` | partial | `C` | 替换 | PolicyDecision / rejection profile |
| `change.proposal.apply` | partial | `C` | 替换 | Commit + EffectReceipt；旧 asset/projection operation 走 adapter |

### Surface（3）

| 当前方法 | 代码状态 | 目标层 | 处置 | 目标概念与兼容行为 |
|---|---:|---:|---|---|
| `host.surface.bundle.resolve` | partial | `X` | 拆分 | `H` 解析/托管 bundle；`P` 解释 shell profile 与 bridge policy |
| `shell.contribution.list` | partial | `P` | 迁移 | `plurora.shell.default/v1` contribution registry |
| `shell.contribution.describe` | partial | `P` | 迁移 | shell-profile descriptor；slot 不再是 substrate enum |

### Outbound（6）

| 当前方法 | 代码状态 | 目标层 | 处置 | 目标概念与兼容行为 |
|---|---:|---:|---|---|
| `host.outbound.audit` | partial | `S` | 替换 | 查询通用 EffectReceipt；保留网络专用 host 视图 |
| `host.outbound.execute` | partial | `X` | 拆分 | `H` HTTPS adapter + `S` authority、policy、receipt |
| `host.outbound.stream` | partial | `X` | 拆分 | `H` 流式网络 adapter + `S` stream/effect lifecycle |
| `host.outbound.websocket.open` | partial | `X` | 拆分 | `H` WebSocket adapter + `S` connection authority/receipt |
| `host.outbound.websocket.send` | partial | `X` | 拆分 | host transport operation，写 effect receipt |
| `host.outbound.websocket.close` | partial | `X` | 拆分 | host transport operation，产生 terminal receipt |

## 59 个事件

“写入”表示在 `plurora-runtime` 中找到命名写入点；`—` 表示当前只有常量/schema/registry 或范围内未找到发出点。

### Session、component 与 project（16）

| 当前事件 | 写入 | 目标层 | 处置与目标概念 |
|---|---:|---:|---|
| `context/opened` | ✓ | `S` | 重塑为 context/journal scope opened |
| `context/closed` | ✓ | `S` | context closed；历史保持可读 |
| `context/forked` | ✓ | `S` | causal head/branch created |
| `host/package.loaded` | ✓ | `S` | component activated；package 字段仅作来源引用 |
| `host/package.loading` | ✓ | `S` | component activation requested |
| `host/package.starting` | ✓ | `S` | component starting |
| `host/package.ready` | ✓ | `S` | component ready |
| `host/package.stopping` | ✓ | `S` | component stopping |
| `host/package.stopped` | ✓ | `S` | component stopped |
| `host/package.unloaded` | ✓ | `S` | component deactivated |
| `host/package.degraded` | ✓ | `S` | component health degraded |
| `host/package.log` | ✓ | `H` | host observability event，不进入 canonical history |
| `host/project.installed` | — | `H` | host project lifecycle |
| `host/project.started` | — | `H` | host project lifecycle |
| `host/project.stopped` | — | `H` | host project lifecycle |
| `host/project.uninstalled` | — | `H` | host project lifecycle |

### Object、projection 与 change（7）

| 当前事件 | 写入 | 目标层 | 处置与目标概念 |
|---|---:|---:|---|
| `object/put` | ✓ | `S` | 替换为 object/artifact committed receipt |
| `projection/updated` | ✓ | `C` | projection protocol event |
| `change/proposal.created` | ✓ | `C` | ChangeSet created |
| `change/proposal.approved` | ✓ | `C` | PolicyDecision approved |
| `change/proposal.rejected` | ✓ | `C` | PolicyDecision rejected |
| `change/proposal.applied` | ✓ | `C` | Commit completed + receipt ref |
| `change/proposal.failed` | ✓ | `C` | Change workflow failed |

### Capability、authority 与通用错误（7）

| 当前事件 | 写入 | 目标层 | 处置与目标概念 |
|---|---:|---:|---|
| `capability/invoked` | ✓ | `S` | invocation started receipt/event |
| `capability/completed` | ✓ | `S` | terminal effect receipt；大输出只存 ref |
| `capability/failed` | ✓ | `S` | terminal failed receipt |
| `authority/denied` | ✓ | `S` | authority decision denied |
| `authority/grant.created` | ✓ | `S` | authority minted/delegated |
| `authority/grant.revoked` | ✓ | `S` | authority revoked |
| `runtime/error` | — | `S` | 保留通用 protocol/transport error envelope，避免复制领域错误 |

### Outbound 与 stream（15）

| 当前事件 | 写入 | 目标层 | 处置与目标概念 |
|---|---:|---:|---|
| `host/outbound.request` | ✓ | `X` | host network request + substrate effect receipt start |
| `host/outbound.denied` | ✓ | `X` | PolicyDecision denied + host destination summary |
| `host/outbound.execute.completed` | ✓ | `X` | terminal EffectReceipt |
| `host/outbound.stream.completed` | ✓ | `X` | terminal EffectReceipt |
| `capability/stream.started` | ✓ | `S` | 保留通用 stream lifecycle |
| `capability/stream.chunk` | ✓ | `S` | chunk 可内联小数据或引用 object |
| `capability/stream.progress` | ✓ | `S` | 通用进度，不解释领域语义 |
| `capability/stream.ended` | ✓ | `S` | terminal success |
| `capability/stream.error` | ✓ | `S` | terminal failure |
| `capability/stream.cancelled` | ✓ | `S` | terminal cancellation |
| `capability/stream.timeout` | ✓ | `S` | terminal timeout |
| `host/outbound.websocket.opened` | — | `X` | host connection event + receipt link |
| `host/outbound.websocket.frame` | — | `X` | host transport telemetry；默认不进入 canonical world history |
| `host/outbound.websocket.error` | — | `X` | host transport error + terminal/partial receipt |
| `host/outbound.websocket.completed` | ✓ | `X` | terminal EffectReceipt |

### Host execution 与 deployment（14）

| 当前事件 | 写入 | 目标层 | 处置与目标概念 |
|---|---:|---:|---|
| `host/exec.request` | — | `H` | host exec lifecycle；引用 substrate PolicyDecision |
| `host/exec.denied` | — | `H` | host exec denial + receipt ref |
| `host/exec.started` | — | `H` | host exec started |
| `host/exec.stopped` | — | `H` | host exec stopped |
| `host/exec.completed` | — | `H` | host exec completed + EffectReceipt |
| `host/exec.failed` | — | `H` | host exec failed + EffectReceipt |
| `host/port.leased` | — | `H` | host port lifecycle |
| `host/port.released` | — | `H` | host port lifecycle |
| `host/port.denied` | — | `H` | host port denial |
| `host/proxy.registered` | — | `H` | host proxy lifecycle |
| `host/proxy.unregistered` | — | `H` | host proxy lifecycle |
| `host/proxy.denied` | — | `H` | host proxy denial |
| `host/deployment.reconciled` | ✓ | `H` | host deployment reconciliation |
| `host/deployment.health` | — | `H` | host deployment health；补入 v1 registry |

## 22 个顶层 schema

| 当前 schema | 目标层 | 处置 | 目标形状 |
|---|---:|---|---|
| `event-envelope.schema.json` | `S` | 重塑 | journal envelope + object refs + explicit causation/receipt refs；保留原始 v1 envelope |
| `protocol-context.schema.json` | `S` | 强化 | authenticated principal、contract/profile negotiation、trace 与 parent invocation |
| `protocol-response.schema.json` | `S` | 新增 | 为 Deprecated 与 Legacy Adapter 调用提供 additive result/error envelope diagnostics |
| `contract-selection.schema.json` | `S` | 保留 | 显式 profile 与逐 layer version requirement；不允许静默降级 |
| `protocol-descriptor.schema.json` | `C` | 新增 | 共享语义、生命周期/错误、权限、向量、Profile、迁移与实现声明 |
| `component-descriptor.schema.json` | `S` | 新增 | 独立实现 identity、behavior digest、trust class、强制边界声明与引用 |
| `package-envelope-descriptor.schema.json` | `H` | 新增 | 将 manifest 与独立寻址 component/artifact 连接起来的获取/安装 envelope |
| `composition-lock.schema.json` | `C` | 新增 | 分别锁定 component artifact、protocol profile 与不可变 content root |
| `world-bundle.schema.json` | `C` | 新增 | 可移植 manifest、原始 v1 envelope、object inventory、receipt、policy、lineage 与内联传输对象 |
| `world-head.schema.json` | `C` | 新增 | 协议定义的 state/history/composition/policy/provenance root 与 parent head |
| `world-journal-range.schema.json` | `S` | 新增 | session 内连续 sequence range 与内容寻址的原始 event envelope |
| `artifact-descriptor.schema.json` | `S` | 新增 | 开放 artifact type、SHA-256 digest、size、references 与 annotations；bytes 位于 ObjectStore |
| `effect-receipt.schema.json` | `S` | 新增 | 内容寻址 terminal evidence；引用 input/output/component/authority/policy/approval/parents |
| `intent.schema.json` | `C` | 新增 | principal goal 与 target scope；不等同于 proposal 或 command |
| `change-set.schema.json` | `C` | 新增 | open operations、preconditions、required authority 与 idempotency |
| `policy-decision.schema.json` | `S` | 新增 | allowed/denied/requires_approval 与 authority evidence |
| `commit.schema.json` | `C` | 新增 | committed/failed/partial result refs 与 operation receipts |
| `capability-descriptor.schema.json` | `S` | 重塑 | component export + protocol claim + trust/conformance metadata |
| `capability-invocation-request.schema.json` | `S` | 强化 | handle-first、idempotency、deadline、input refs、requested profile |
| `capability-invocation-result.schema.json` | `S` | 强化 | output refs、receipt ref、terminal status；避免大 payload 常驻 envelope |
| `permission-set.schema.json` | `X` | 拆分 | host policy request / manifest authority declaration / runtime capability 不再混为一物 |
| `manifest.schema.json` | `X` | 拆分 | package envelope、artifact descriptors、component descriptors、protocol claims、shell contributions 分离 |

## V1 兼容义务

任何迁移实现必须满足：

1. 旧方法名通过显式 alias registry 路由，不能依赖散落的 `match` 特判。
2. Alias 记录 canonical target、request adapter、response adapter、弃用状态和支持窗口。
3. `host.info` 返回所有 contract layers、版本、profiles、aliases 和 maturity；客户端显式选择，不静默降级。
4. v1 request/response/event 的原始 JSON 可无损保留；未知字段不得在转存时消失。
5. 旧 SDK 继续工作；新 SDK 按 substrate/host/protocol/shell 分包，并提供 legacy umbrella client。
6. Conformance 分为 substrate、host、protocol profile、shell profile 和 legacy adapter 套件。
7. 一个 legacy alias 只有在迁移工具、支持窗口和替代 conformance 均存在后才能删除。

当前运行状态见 [`../ALPHA_STATUS.md`](../ALPHA_STATUS.md)。本文只保留长期 owner 分类与兼容义务；具体优先级见 [`../roadmap/NEXT_STEPS.md`](../roadmap/NEXT_STEPS.md)。
