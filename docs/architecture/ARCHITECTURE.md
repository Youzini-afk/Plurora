# 架构

> [English](./ARCHITECTURE.en.md) · [中文](./ARCHITECTURE.md)

Plurora 不是“内核、能力包、单一产品容器”三层的封闭产品栈。它是一组有明确所有权和单向依赖的开放层次：极小的宪法基底、可演化的协议、可替换的组件与内容、管理现实资源的 Host、可替换的发行版，以及自由发展的产品。

当前 Contract V1 公开 `context`、`package`、`surface`、`change`、Work/Installation/Run/Exposure/Binding/Realization 等边界。它们是正在运行的合同，不自动等同于永久架构。长期归属见 [`CONSTITUTION_V2.md`](CONSTITUTION_V2.md) 与 [`../spec/CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.md)。

## 分层模型

```text
┌─────────────────────────────────────────────────────────────────┐
│ Products / Experiences / Services                               │
│ 应用、工具、世界、游戏、文档、agent workspace、服务与未知形态       │
├─────────────────────────────────────────────────────────────────┤
│ Distributions / Shells / Clients                                │
│ 官方与第三方 Web、Desktop、PWA、CLI、IDE、headless client          │
├─────────────────────────────────────────────────────────────────┤
│ Protocol Commons                                                │
│ 共享语义、profiles、状态机、迁移、行为合同与 conformance             │
├─────────────────────────────────────────────────────────────────┤
│ Components / Content / Adapters                                 │
│ WASM、process、remote、trusted native、静态资源、内容与适配器        │
├─────────────────────────────────────────────────────────────────┤
│ Constitutional Substrate                                        │
│ 身份、权力、对象、journal、因果、调用、流、事务、效果、协商            │
└─────────────────────────────────────────────────────────────────┘

Host Control Plane / Runtime Fabric 与上述层正交：
安装、进程、文件、secret、网络、端口、代理、target、资源 Realization、备份、诊断。
```

“正交”意味着 Host 可以为不同产品和协议提供机器能力，但不能因此拥有它们的内容语义。一个无头服务、一个本地创作工具和一个多人世界可以共享 Host 与基底，却采用完全不同的协议和 Shell。

## 各层职责

### Constitutional Substrate

基底只拥有无法安全地由普通上层重复实现的机制：

- principal 与认证后的调用上下文；
- authority mint、attenuate、delegate、lease、refresh 与 revoke；
- 内容寻址对象、可验证引用和不可伪造句柄；
- 只追加 journal、稳定顺序、因果引用与 head 原语；
- invoke、stream、cancel、deadline、backpressure；
- compare-and-swap、前置条件、幂等和原子 commit；
- effect receipt、审计与 provenance 连接点；
- 组件实例的最小生命周期与健康状态；
- 协议、版本和 Profile 协商。

基底不拥有 Library、Home、Play、Forge、Assistant、模型、agent、记忆、世界、文档、资源编排产品或具体 secret store。

现有 `plurora-core` / `plurora-runtime` 中仍混合了一部分 Host、协议和 Shell 语义；后续拆分必须保持数据安全、公开合同精确性与单一机器身份。

### Protocol Commons

协议公地拥有多个实现需要共享的语义和行为，例如：

- 数据形状和字段含义；
- 生命周期、状态机、错误与取消；
- authority、隐私与 effect 要求；
- compatibility profile、版本协商与迁移；
- 可执行行为合同和实现声明。

Agent、Memory、World、Document、Workspace、Surface、Change、Inference 等可以在这里形成协议，也可以存在竞争协议。第一方维护不赋予内核路由优先级；只有显式采用某个协议或 Profile 的参与者受其约束。

### Components / Content / Adapters

组件实现协议能力，内容承载用户和产品数据，adapter 连接外部系统。执行形态可以是：

- sandboxed WASM component；
- isolated process；
- remote service boundary；
- trusted native implementation；
- static resource / surface bundle；
- foreign capsule。

它们可以通过同一调用协议暴露能力，但不能宣称拥有相同的隔离、故障、延迟或供应链保证。

Package 是获取、分发和安装的信封，不是所有语义的本体单位。一个 Package 可以携带多个组件、协议描述、内容和 Surface；组件、内容和协议应尽量拥有独立身份、摘要、版本与迁移路径。详见 [`CAPABILITY_PACKAGE.md`](CAPABILITY_PACKAGE.md)。

### Host Control Plane / Runtime Fabric

Host 管理现实环境中的资源和操作：

- package / component 获取、安装、更新与卸载；
- 本地进程、WASM、远程服务和 target 生命周期；
- 文件、workspace、secret、网络、端口与代理；
- Work 的 Host-local Installation、Run、Exposure、Binding 与 Realization 记录；
- Realization、运行健康、日志、备份、恢复和诊断；
- 设备身份、资源 selector 与 Host 管理策略。

Host 操作仍必须使用基底提供的身份、权力、效果和审计机制。Host 不因能启动一个 World 或 Document 产品，就获得解释其内容的权力。

### Distributions / Shells / Clients

发行版把平台能力组织成可用产品。当前官方发行版包括 React Web/PWA、Tauri Desktop 和 Rust CLI，并使用 Library、Settings、Installation frame、Realization workbench 与 Surface bridge。

这些都是官方产品选择：

- 可以强观点并持续优化；
- 必须只使用公开合同与明确的 Host API；
- 不能成为组件获取额外权威的旁路；
- 可以被第三方 Shell、IDE、无头客户端或完全不同的发行版替换。

Surface slot、Home card、Forge panel 和 Assistant action 属于当前 Shell Profile / Contract V1 兼容面，不属于宪法基底。

### Products / Experiences / Services

最上层拥有领域本体、业务规则和最终交互。聊天、世界、游戏、设计工具、普通 Web 服务、IDE、自动化系统和未来未知形态都在这里自由发展。

产品可以选择：

- 是否采用 Work，或使用另一种领域协议对象；
- 是否使用事件溯源、分支或审批；
- 是否使用 AI；
- 是否提供 UI；
- 是否本地、远程、多人或离线；
- 采用哪些协议、组件与 Host 能力。

产品选择不能反向成为所有 Plurora 产品的要求。

## 单向依赖与下沉门槛

合理依赖方向是：

```text
Product
  ↓
Distribution / Shell Profile
  ↓
Protocols and Components
  ↓
Constitutional Substrate
```

Host 为各层提供受权的现实资源，但不创造反向语义依赖。

一个上层需求若希望下沉，必须先回答：

1. 它是这个产品的观点，还是多个独立产品共同需要的语义？
2. 它属于可竞争协议、Host 机器操作，还是确实无法安全上移的基底机制？
3. 下沉后是否减少锁定，而不是把当前第一方实现冻结成平台法律？
4. 是否有版本、迁移、未知数据保留和旧客户端兼容路径？

默认答案不是“一律做成包”，而是放到拥有该语义和生命周期的最上层。

## 当前 Contract V1 的位置

Contract V1 是当前可运行、可生成 SDK、由 conformance 守护的公开合同。它同时承载了多层职责：

- `context.*`、`journal.*`、`capability.*`、`authority.*`、`object.*` 与 `identity.*` 属于 Substrate；
- `host.*` 拥有 Installation、Run、Exposure、Binding、Realization、target、exec、port、proxy、outbound adapter、Package operation 与 diagnostics；
- `protocol.*`、`change.*` 与 `projection.*` 属于可演化 Protocol；
- `shell.*` contribution discovery 与 surface interpretation 属于 Shell Profile。

当前客户端和第三方集成都使用这套精确 v1 identity。新增语义必须声明 owner、maturity、schema 与可协商版本边界，不能积累成隐藏 alias 或第一方 shortcut。逐项归属见 [`../spec/CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.md)。

## 当前官方发行版

### Web / PWA

`clients/web` 是 React 19 + Tailwind v4 + Vite 的官方平台 Shell，并可作为 PWA 安装。它通过 HTTP `POST /rpc`、SSE 与 `/host/v1/*` 使用公开平台和 Host 边界，不读取 SQLite，也不导入 runtime 私有状态。

Powerbox chooser 通过 `host.exposure.*` 与 `host.binding.*` 显示 exact provider/consumer Port、audience、lease、trust 与 evidence；它只使用 Host public relay，不持有 private intent 或 runtime handle。Surface bridge 仍是显式 allowlist，没有 Powerbox private bridge。

Realization workbench 通过 `host.realization.*` 先显示 effect-free plan、stable digest、preconditions 与风险，再执行 approval-bound apply/stop/rollback/reconcile。Run start、Binding select 与关闭 UI 都不会隐式触发 Realization effect。

### SurfaceHost

第三方 Web Surface 由 sandboxed iframe 挂载。默认 Surface 没有 kernel access；Host 只转发显式方法和 capability allowlist，并把 stream ownership、session、Installation/Run grant 与短期 asset lease 绑定到当前 mount。完整边界见 [`../guides/SURFACE_HOSTING.md`](../guides/SURFACE_HOSTING.md)。

### Desktop

`clients/desktop` 是 Tauri 2.x wrapper，并管理 loopback-only Host sidecar。它准备持久 profile、以随机 loopback 端口启动 `plurora host serve`、完成健康与一次性 bootstrap 后显示 Web Shell，退出时终止 sidecar。

Desktop、Web/PWA 与远程 Host 连接复用同一 client core 和公开边界。官方 Desktop 不拥有第二套协议或私有 Studio。

### CLI 与无头使用

`plurora-cli` 提供 Host、Work、Installation、Run、Exposure/Binding、Realization、Package、Contract、conformance 和运维入口。CLI 在本机运行也不能通过读取 Host 数据目录获得公开协议之外的产品权威。

## Work / Workspace / Installation 模型

WorkRevision 是可移植内容身份，Workspace 是某台 Host 上的可变源码位置，Installation 是 Host journal 中对 Work 的采用记录。Run、Exposure/Binding 与 Realization 是相互独立的 Host-local 事实，不能被 Installation `ready` 代替，也不能互相隐式创建。

World、Document、Service 或其他协议对象可以独立存在；它们可以通过 Component、Port、Adapter 与 Work 组合，而不丢失自身身份。详见 [`../guides/INSTALLATION_MODEL.md`](../guides/INSTALLATION_MODEL.md) 与 [`../product/PLATFORM_PRODUCT_MODEL.md`](../product/PLATFORM_PRODUCT_MODEL.md)。

## 仓库地图

```text
crates/plurora-core      当前核心类型、schema、身份、事件与合同对象
crates/plurora-runtime   当前 runtime、组件执行、协议调度与部分 Host 能力
crates/plurora-service   HTTP / RPC / SSE 与 Host service 边界
crates/plurora-cli       CLI、Host、脚手架、contract 与 conformance 工具
clients/web          官方 React Web Shell / PWA
clients/desktop      Tauri wrapper + managed Host sidecar
packages/plurora    通过普通清单加载的第一方组件与实验能力
sdk/                 生成合同 SDK 与领域 SDK
profiles/            发行版 / Host 的组件与策略组合
examples/            示例、fixture 与第三方接入样例
docs/                章程、架构、协议、产品、指南与状态
```

代码目录仍反映 Contract V1 的历史聚合，不能单凭 crate 名称推断永久所有权。

## 接下来读什么

- [`../CHARTER.md`](../CHARTER.md) — 平台目标与不可妥协原则；
- [`VISION.md`](VISION.md) — 长期整体形态；
- [`CONSTITUTION_V2.md`](CONSTITUTION_V2.md) — 候选宪法基底；
- [`../spec/CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.md) — 当前合同逐项归属；
- [`CONSTITUTIONAL_SUBSTRATE.md`](CONSTITUTIONAL_SUBSTRATE.md) — 基底职责与当前 kernel 兼容边界；
- [`CAPABILITY_PACKAGE.md`](CAPABILITY_PACKAGE.md) — Package、component、content 与执行信任；
- [`../product/PLATFORM_PRODUCT_MODEL.md`](../product/PLATFORM_PRODUCT_MODEL.md) — 官方发行版的产品责任；
- [`../guides/INSTALLATION_MODEL.md`](../guides/INSTALLATION_MODEL.md) — Work / Workspace / Installation 边界；
- [`../ALPHA_STATUS.md`](../ALPHA_STATUS.md) — 实际实现状态；
- [`../roadmap/NEXT_STEPS.md`](../roadmap/NEXT_STEPS.md) — 当前建设方向。
