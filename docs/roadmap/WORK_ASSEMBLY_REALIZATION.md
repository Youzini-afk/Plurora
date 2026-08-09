# Work、Assembly、Installation 与 Realization 实施设计

> [English](./WORK_ASSEMBLY_REALIZATION.en.md) · [中文](./WORK_ASSEMBLY_REALIZATION.md)

> 状态：候选架构与 Codex 执行简报。本文是开发期的一次性实施计划，不是 Stable 协议承诺。完成全部实施后，应删除本文，并把仍然有效的语义收敛到 architecture、spec、guide 与 `ALPHA_STATUS`。

## 0. 给执行 Agent 的直接指令

本文交给 Codex 作为实现依据。开始编码前必须依次阅读：

1. [`../CHARTER.md`](../CHARTER.md)
2. [`../architecture/VISION.md`](../architecture/VISION.md)
3. [`../architecture/ARCHITECTURE.md`](../architecture/ARCHITECTURE.md)
4. [`../architecture/CONSTITUTION_V2.md`](../architecture/CONSTITUTION_V2.md)
5. [`../architecture/CAPABILITY_PACKAGE.md`](../architecture/CAPABILITY_PACKAGE.md)
6. [`../product/PLATFORM_PRODUCT_MODEL.md`](../product/PLATFORM_PRODUCT_MODEL.md)
7. [`../ALPHA_STATUS.md`](../ALPHA_STATUS.md)
8. 本文

执行规则：

- 从最新 `main` 创建 `feature/work-assembly-realization`，不要直接在 `main` 上堆半完成状态。
- 每个 Phase 必须形成一个可构建、可审查、可回溯的提交，并立即推送远端分支。
- Phase 内允许短暂不可构建；Phase 结束时工作树必须干净，目标检查必须通过。
- 这是预发布开发期的破坏性重构。不要保留 `Project`、旧 Composition、旧 RPC、旧事件、旧目录、旧读取器、alias、fallback 或迁移器。
- 不迁移 `~/.plurora/projects/` 的开发数据；新代码只认新布局。测试使用全新临时数据目录。
- 删除安装实例或工作区时，仍必须保留现有的路径 containment、symlink 防护和 `linked_local` 源永不删除规则。破坏合同不等于破坏用户文件。
- 不手改生成的 schema、OpenAPI、Rust SDK 或 TypeScript SDK；只改事实来源和 generator。
- 不给第一方 Package、官方 Shell 或 agent 增加私有 API、publisher priority、root shell 或隐藏 authority。
- 不把 Work、Assembly、Game、Library、Deployment 写入 Constitutional Substrate；它们属于 Candidate Protocol、Host 或官方发行版。
- 不修改 YdlTavern，不做发布，不做 marketplace、支付、DRM 或通用商业授权服务。
- 不使用子代理替代主实现者进行架构审查。可以做有界搜索，但设计、修改、审查和最终验证由当前执行者负责。
- 本机只跑有界检查；完整 Rust、Docker、Windows、Desktop 与 Host operations 验收交给 GitHub CI。
- 若上下文不足，必须在完整 Phase 边界停下并汇报，不得留下未提交的大面积半迁移。

建议提交主题在各 Phase 中给出。除非代码事实证明本文某处不可行，否则不要为普通实现细节反复询问；选择满足本文件不变量的最小方案。

Codex 的第一轮实际动作不是复述本文，而是：核对 `git status`、远端、最新 `main`、当前 CI 基线与相关模型位置；创建实施分支；记录 Phase 1 会触及的事实来源和生成链；随后直接开始 Phase 1。代码是“当前实现”的事实来源，本文是“目标边界”的执行依据；若二者存在未预见冲突，保留本文不变量，选择最小可审查实现，并在 Phase 汇报中明确记录偏差。

可直接交给 Codex 的启动指令：

```text
在 D:\project\Plurora 打开最新 main，完整阅读
`docs/roadmap/WORK_ASSEMBLY_REALIZATION.md` 及其中列出的前置文档。
把该文件视为本轮目标架构与执行合同，不要停留在重新规划或总结。
核对干净工作树和远端后创建 `feature/work-assembly-realization`，立即执行 Phase 1。
每个 Phase 在本地有界检查通过后独立 commit、push，并等待该提交的 GitHub CI；
失败先在当前 Phase 修复，不要带病进入下一 Phase。
不要保留旧 Project / Composition / Deployment 兼容层，不要修改 YdlTavern，
不要给 Agent 或第一方实现私有权力。全部 Phase 完成并通过后再 fast-forward 合并 main，
删除临时计划和实施分支，最后一次性提交完整汇报。
```

每个 Phase 汇报只需保持可审计，使用固定结构：

```text
Phase / commit SHA
完成的用户能力与架构边界
删除的旧身份或债务
本地有界检查
GitHub CI run 与结论
与本文的任何偏差及理由
下一 Phase 的输入前提
```

## 1. 最终要形成什么

Plurora 不建设三套互相粘连的系统：游戏引擎、部署平台和游戏启动器。三类需求统一成同一条对象与生命周期链：

```text
可变 Source / Workspace
        ↓ 构建、解析、封装
不可变 WorkRevision
        ↓ 引用
递归 AssemblyRevision
        ↓ 在某个 Host 上解析 provider、权限与状态
InstallationRecord + AssemblyLock
        ↓ 启动
RunRecord
        ↓ 若节点需要远程或持久机器资源
OperationalIntent
        ↓ 对 Target inventory 编译
RealizationPlan
        ↓ 审批与确定性执行
RealizationRevision + EffectReceipts
```

同一模型应同时容纳：

- 由 Rust、TypeScript、WASM、隔离进程或远程服务组成的开放游戏；
- 从一个完整作品中抽取、封装后供其他作品复用的组件或嵌套 Assembly；
- Agent 帮助理解并部署的普通开源仓库；
- 只有启动入口和存档备份能力的闭源游戏；
- 实现公开协议、因此能够参与组合的闭源二进制或远程服务；
- 本地客户端、远程服务器、存储、模型服务分布在多个 Target 的作品。

Plurora 的独特价值不在于替代 Godot、Unity、Unreal、Docker、Kubernetes、Git 或商店，而在于统一：

- 作品由什么构成；
- 各部分通过什么公开语义连接；
- 谁拥有状态；
- 谁被授予什么权力；
- 哪些实现可替换；
- 作品怎样在不同机器上形成一次具体运行；
- 哪些部分可以复制、导出、备份、部署或保持不透明。

## 2. 第一性原理

### 2.1 定义、实例、运行和机器实现必须分开

以下四件事不是同一个对象：

- **WorkRevision：** 一个不可变、可移植的作品定义。
- **Installation：** 某个 Host 采用该作品后的本地实例，拥有选择、权限和用户状态。
- **Run：** 一次实际活着的运行，拥有组件实例、流、租约与健康状态。
- **Realization：** Assembly 的节点和连接在一组 Target 上的具体机器实现。

更新作品不等于覆盖用户状态；停止 Run 不等于删除 Installation；停止 Realization 不等于丢失 Work；重新部署也不等于重新解释源码。

### 2.2 身份不能等于位置

长期身份使用逻辑 ID 与内容摘要，不能使用：

- 本机绝对路径；
- 临时 URL；
- 进程 ID；
- Docker container name；
- 当前 Host 数据库行号。

路径、端口、容器和进程只存在于 Installation、Run 或 Realization 的局部记录与 receipt 中。

### 2.3 开源、可组合、可部署和可迁移是独立维度

源码开放并不自动意味着有稳定边界；闭源也不自动意味着不可组合。

```text
Source visibility
× Protocol participation
× State portability
× Rebuildability
× Redistribution rights
× Runtime trust class
```

这些维度分别记录。不能再用 `plurora_native / external_wrapped / external_workspace` 一个枚举承担全部含义。

### 2.4 语义、控制面和数据面必须分开

- **Protocol** 定义含义、生命周期、错误、权限和行为。
- **Control plane** 负责发现、选择、授权、激活、停止、恢复和审计。
- **Data plane** 承担实际高频数据传输。

游戏每帧状态、音频、GPU buffer 或大规模实体同步不能强制经过 JSON-RPC。公开 Capability 继续适合控制、配置、存档、AI、资产和低频操作；高频数据由 Binding 协商 WASM direct call、IPC、共享内存、WebSocket、QUIC、引擎原生桥或其他 transport。

### 2.5 计划和效果必须分开

Agent、planner 或 UI 可以产生候选计划，但计划不授予执行权。实际执行必须经过：

```text
Intent → Plan / ChangeSet → PolicyDecision → Effect → Receipt
```

外部效果由确定性 Host control plane 执行。Agent 不持有长期 root shell，也不在每次部署时临场重写操作语义。

### 2.6 状态属于明确的所有者

任何可变状态都必须声明：

- 谁拥有它；
- 生命周期是 run、installation、user、shared 还是 external；
- schema 或 opaque 身份；
- 是否可备份、导出、合并或迁移；
- 替换组件时由谁执行迁移。

组件不能因为被更新就静默覆盖用户内容。

### 2.7 递归组合优于“宿主 + 插件”

一个 Assembly 可以包含 Component，也可以包含另一个 Assembly，并把内部 Port 重新暴露为自己的 Port。结构包含关系必须无环；运行期消息图可以在协议允许时形成反馈环。

这使一个完整后端、编辑器工具组或游戏子系统能够被封装成一个更高层组件，而不是要求调用者理解内部所有节点。

## 3. 所属层

| 概念 | 所属层 | 不属于 |
|---|---|---|
| `ArtifactDescriptor`、authority、journal、effect、invoke、stream | Constitutional Substrate | 游戏或部署产品 |
| Work / Assembly / Port / Binding / State Slot | Experimental Protocol Commons | substrate |
| Package Envelope / Component artifact | Components 与分发 | Work 本体 |
| Installation / Run / Exposure / Binding lease | Host Control Plane | 可移植 Work |
| OperationalIntent | Work 可携带的 portable artifact | 某个 Docker backend |
| Target inventory / RealizationPlan / RealizationRevision | Host Control Plane | substrate |
| Library、Affordance、Play/Edit/Deploy 按钮 | 官方发行版 | 通用协议法律 |
| Foreign launch adapter、商店 entitlement adapter | Component / Adapter | 平台 DRM |

新增两个 Experimental protocol：

```text
plurora.work       profile plurora.work/experimental/v1
plurora.assembly   profile plurora.assembly/experimental/v1
```

它们只约束选择这些 Profile 的参与者。不要把它们加入候选宪法的 Stable substrate 列表。

## 4. 统一词汇

### Work

创作者和用户识别的长期逻辑作品。`WorkId` 是命名身份，不能充当版本或内容身份。

### WorkRevision

不可变、内容寻址的作品修订。它引用 Assembly、内容 roots、入口、权利声明、来源与可选 OperationalIntent。其 digest 是精确修订身份。

### Workspace

可变的创作或导入空间。可能来自 Git、用户目录、Agent managed copy 或生成器。Workspace 不是 Work，也不进入可移植运行锁。

### Component

实际提供行为或静态资源的独立实现单元。继续使用现有 `ComponentDescriptor`、trust class、artifact digest 与 protocol implementation 基础。

### Port

Component 或 Assembly 对外提供或需要的 typed connection。Port 表达语义与交互类别，不固定具体 transport。

### Binding

把一个 export Port 接到一个 import Port 的连接决定。Binding 可以在 authoring、installation、launch 或 runtime 阶段确定。

### Assembly

Component 与嵌套 Assembly 的递归装配图，包含节点、bindings、暴露 ports 和 state slots。

### AssemblyLock

在特定解析上下文中得到的精确、不可变解析结果。它锁定节点 artifact、behavior digest、protocol profile、provider、binding 和 content roots。

### Installation

某个 Host 上采用一个 WorkRevision 的实例。它拥有 AssemblyLock、用户选择、权限、secret policy、state binding 和更新来源。

### Run

Installation 的一次运行。一个 Installation 同时最多一个默认 Run，但模型不禁止未来多个并行 Run。

### Exposure

一个 Installation 显式向其他 Installation 或 principal 暴露某个 Assembly export 的记录。Exposure 有 audience、lease、resource scope 和 revoke。

### OperationalIntent

作品希望获得的运行条件：工作负载、资源、连接、状态、endpoint、health、更新策略与 placement 约束。它不包含目标机器上的具体端口、进程 ID 或容器 ID。

### TargetInventory

Host 对某个 Target 在某一时刻观察到的能力、容量、拓扑、信任域与可用性快照。

### RealizationPlan

把 AssemblyLock + OperationalIntent + TargetInventory 编译后得到的不可变计划。它声明具体 placement、transport、build、launch、route、state 与 required authority，但尚未执行。

### RealizationRevision

一次已执行或正在执行的机器实现。它引用 Plan、parent revision、实际资源、receipt、health 和 active 状态。

## 5. 身份与内容规则

### 5.1 ID 类型

在新 crate 中定义并验证：

```rust
WorkId              // namespace/name
AssemblyId          // namespace/name
NodeId              // Assembly 内局部 ID
PortId              // Component 或 Assembly 内局部 ID
StateSlotId         // Assembly 内局部 ID
InstallationId      // Host-local opaque ID
RunId               // Host-local opaque ID
ExposureId          // Host-local opaque ID
BindingId           // Host-local opaque ID
RealizationId       // Host-local opaque ID
```

规则：

- 逻辑 ID 与 digest 分离。
- `WorkId`、`AssemblyId` 使用与 Package ID 相同的安全 namespace/name 规则，但不是 Package ID alias。
- 局部 ID 只允许 ASCII 字母、数字、`-`、`_`、`.`，禁止路径分隔符、`..` 与 shell 特殊字符。
- Host-local ID 使用随机 UUID 或等价不可预测 ID，不从标题、路径或用户输入直接派生。
- 所有跨 Host 可移植引用必须携带完整 `ArtifactDescriptor` 或至少可验证 digest，不只携带逻辑 ID。

### 5.2 Canonical artifact

Authoring 文件可以使用 YAML；进入 ObjectStore 的 Work、Assembly、Lock、Intent、Rights、Plan 使用 canonical JSON。

内容摘要不得包含：

- 构建机器绝对路径；
- 当前时间；
- 随机临时 ID；
- 未排序 map；
- raw secret；
- 实际端口或进程信息。

时间、builder、source commit、签名与 provenance 通过独立 attestation / receipt 关联，不污染可复现的语义摘要。

### 5.3 Artifact 类型

至少新增：

```text
urn:plurora:work-revision:v1
urn:plurora:assembly-revision:v1
urn:plurora:assembly-lock:v1
urn:plurora:rights-declaration:v1
urn:plurora:transparency-declaration:v1
urn:plurora:operational-intent:v1
urn:plurora:realization-plan:v1
urn:plurora:foreign-capsule:v1
```

未知 artifact type 仍可复制、存储和导出；不认识语义时不能执行或自动授予权限。

## 6. 规范性 MVP 数据模型

实现时允许拆分模块，但字段语义不得模糊回 `metadata`。

### 6.1 WorkRevision

```rust
pub struct WorkRevision {
    pub schema: String,                       // plurora.work-revision.v1
    pub work_id: WorkId,
    pub title: String,
    pub description: String,
    pub assembly: ArtifactDescriptor,
    pub content_roots: Vec<ArtifactDescriptor>,
    pub entrypoints: Vec<WorkEntrypoint>,
    pub rights: Option<ArtifactDescriptor>,
    pub transparency: Option<ArtifactDescriptor>,
    pub operational_intent: Option<ArtifactDescriptor>,
    pub annotations: BTreeMap<String, Value>,
}

pub struct WorkEntrypoint {
    pub id: String,
    pub intent_uri: String,
    pub target: WorkEntrypointTarget,
    pub annotations: BTreeMap<String, Value>,
}

pub enum WorkEntrypointTarget {
    AssemblyPort { port_id: PortId },
    Surface { surface_id: String },
    ForeignLaunch { launch_id: String },
}
```

`intent_uri` 是可扩展提示，不是 substrate enum。官方 Shell 可以识别 `play`、`open`、`edit`、`inspect` 等自身 Profile 词汇；未知 intent 仍可展示为通用 action。

### 6.2 PortDescriptor

```rust
pub struct PortDescriptor {
    pub port_id: PortId,
    pub contract: PortContract,
    pub interaction: InteractionModelId,
    pub role: PortRole,
    pub transport: TransportRequirements,
    pub annotations: BTreeMap<String, Value>,
}

pub struct PortContract {
    pub protocol_id: String,
    pub interface_id: String,
    pub version: String,              // export 为 exact；import 为 semver requirement
    pub profiles: Vec<String>,
}

#[serde(transparent)]
pub struct InteractionModelId(pub String);

pub enum PortRole {
    Import {
        multiplicity: PortMultiplicity,
        latest_binding_phase: BindingPhase,
        availability: AvailabilityPolicy,
        accepted_effects: Vec<EffectClass>,
    },
    Export {
        multiplicity: PortMultiplicity,
        effect_class: EffectClass,
    },
}

pub struct PortMultiplicity {
    pub min: u16,
    pub max: Option<u16>,
}

pub enum BindingPhase {
    Authoring,
    Installation,
    Launch,
    Runtime,
}

pub enum AvailabilityPolicy {
    Required,
    DegradedWithout,
    Optional,
}

pub enum EffectClass {
    Pure,
    DeterministicStateful,
    RecordedNondeterministic,
    ExternalEffecting,
    RealtimeBestEffort,
}
```

`InteractionModelId` 的首批已知值是 `plurora.interaction.capability-unary/v1`、`capability-stream/v1`、`event-stream/v1`、`duplex-stream/v1`、`artifact/v1`、`snapshot/v1` 与 `endpoint/v1`。Wire 形状使用开放的 namespaced string，而不是封闭 enum：未知值必须保真读取和转移，但在没有已声明实现或 Adapter 时不能绑定或执行。

Import 的 `latest_binding_phase` 表示最迟必须在何时完成选择；允许更早固定。Effect class 没有隐式全序，Import 必须显式列出可接受 classes，不能把 `realtime_best_effort` 与 `external_effecting` 之类语义做数值比较。

`TransportRequirements` 只表达约束，如 same-process、local-only、ordered、reliable、max latency class、large payload、shared-memory allowed；它不直接写死某个 socket path。

现有 `provides` 自动投影成 export Capability Port；`consumes` 自动投影成 import Capability Port。显式 ports 用于分组协议接口和非 Capability 数据面，不要求所有现有 Manifest 重复声明相同信息。

### 6.3 AssemblyRevision

```rust
pub struct AssemblyRevision {
    pub schema: String,                       // plurora.assembly-revision.v1
    pub assembly_id: AssemblyId,
    pub nodes: Vec<AssemblyNode>,
    pub bindings: Vec<AssemblyBinding>,
    pub exposed_ports: Vec<AssemblyPortExposure>,
    pub state_slots: Vec<StateSlotDescriptor>,
    pub annotations: BTreeMap<String, Value>,
}

pub struct AssemblyNode {
    pub node_id: NodeId,
    pub source: AssemblyNodeSource,
    pub ports: Vec<PortDescriptor>,
    pub configuration: Option<ArtifactDescriptor>,
    pub annotations: BTreeMap<String, Value>,
}

pub enum AssemblyNodeSource {
    Component { component: ArtifactDescriptor },
    Assembly { assembly: ArtifactDescriptor },
}

pub struct PortEndpoint {
    pub node_id: NodeId,
    pub port_id: PortId,
}

pub struct AssemblyBinding {
    pub binding_id: String,
    pub provider: PortEndpoint,
    pub consumer: PortEndpoint,
    pub phase: BindingPhase,
    pub transport_policy: TransportPolicy,
    pub annotations: BTreeMap<String, Value>,
}

pub struct AssemblyPortExposure {
    pub port_id: PortId,
    pub direction: PortDirection,
    pub target: PortEndpoint,
    pub annotations: BTreeMap<String, Value>,
}
```

验证规则：

- 节点 ID、binding ID、exposed port ID 唯一。
- Component 节点的 Port 合同直接进入 AssemblyRevision 的 canonical 内容；修改版本、profile、interaction、transport 或 effect 必须改变 Assembly/Work/Lock digest。嵌套 Assembly 通过自身 exposed ports 提供边界，不重复声明 inline ports。
- 嵌套 Assembly 的内容引用闭包完整，包含图无环。
- provider 必须是 export，consumer 必须是 import。
- protocol、interface、version、profile、interaction 和 multiplicity 兼容。
- authoring-time binding 必须在 Work pack 时完全解析。
- 未绑定 required import 只有在其 phase 晚于 authoring 时才允许进入 WorkRevision。
- 同一 import 超过 max cardinality 时拒绝。
- 未知 interaction 或 transport requirement 不静默降级。

### 6.4 StateSlotDescriptor

```rust
pub struct StateSlotDescriptor {
    pub state_slot_id: StateSlotId,
    pub owner_node_id: NodeId,
    pub schema_ref: Option<ArtifactDescriptor>,
    pub scope: StateScope,
    pub portability: StatePortability,
    pub migration_port: Option<PortEndpoint>,
    pub backup_policy: BackupPolicy,
    pub annotations: BTreeMap<String, Value>,
}

pub enum StateScope {
    Run,
    Installation,
    User,
    Shared,
    External,
}

pub enum StatePortability {
    Portable,
    OpaqueExportable,
    HostBound,
    ExternalAuthority,
}

pub enum BackupPolicy {
    Required,
    Allowed,
    Forbidden,
}
```

规则：

- `Portable` 必须有 schema_ref。
- `HostBound` 和 `ExternalAuthority` 不得被 Work export 冒充为可移植内容。
- 替换拥有 durable state 的节点时，若 schema/behavior 不兼容，必须存在 migration port 或显式清空决定。
- raw filesystem path 不进入 WorkRevision 或 AssemblyLock。

### 6.5 AssemblyLock

```rust
pub struct AssemblyLock {
    pub schema: String,                       // plurora.assembly-lock.v1
    pub assembly: ArtifactDescriptor,
    pub nodes: Vec<NodeLock>,
    pub bindings: Vec<BindingLock>,
    pub protocol_profiles: Vec<ProtocolProfilePin>,
    pub content_roots: Vec<ArtifactDescriptor>,
}
```

`NodeLock` 至少锁定：node ID、component/assembly artifact、behavior digest、trust class。`BindingLock` 至少锁定 provider、consumer、具体 provider component、transport class 与 phase。

当前 `CompositionLock` 被删除并由 `AssemblyLock` 取代。World Bundle、lockfile、schema、SDK、conformance 和文档全部同步更新，不保留 reader 或 alias。

### 6.6 RightsDeclaration 与 TransparencyDeclaration

权利声明不是法律裁决，而是由发布者或安装来源给出的可审计声明。每个声明可引用签名或 attestation。

```rust
pub enum RightDisposition {
    Allowed,
    Denied,
    RequiresEntitlement,
    Unspecified,
}

pub struct RightsDeclaration {
    pub license_expression: Option<String>,
    pub terms_uri: Option<String>,
    pub install: RightDisposition,
    pub execute: RightDisposition,
    pub backup: RightDisposition,
    pub export_state: RightDisposition,
    pub copy_across_hosts: RightDisposition,
    pub redistribute_artifacts: RightDisposition,
    pub modify: RightDisposition,
    pub derive: RightDisposition,
    pub modding: RightDisposition,
    pub dedicated_server: RightDisposition,
    pub entitlement_requirements: Vec<ProtocolRequirement>,
    pub evidence_refs: Vec<ArtifactDescriptor>,
}

pub struct TransparencyDeclaration {
    pub source_visibility: SourceVisibility,
    pub source_refs: Vec<ArtifactDescriptor>,
    pub reproducible_build_claim: ClaimStatus,
    pub sbom_refs: Vec<ArtifactDescriptor>,
    pub provenance_refs: Vec<ArtifactDescriptor>,
    pub signature_refs: Vec<ArtifactDescriptor>,
    pub telemetry_disclosures: Vec<String>,
    pub state_portability: StatePortability,
    pub evidence_refs: Vec<ArtifactDescriptor>,
}
```

UI 必须把“声明”“已验证证据”和“Host 实际强制边界”分开显示。闭源不是错误状态；未知权利、opaque state、无 provenance 或 trusted native 必须清楚可见。Rights 声明本身不是 capability；它驱动保守的 Host / distribution policy。`Denied` 或 `Unspecified` 默认阻止无人值守复制、导出和再分发，任何用户 override 都必须是单独、可审计的产品策略决定，不能伪装成平台法律结论。

### 6.7 InstallationRecord

```rust
pub struct InstallationRecord {
    pub schema_version: u16,
    pub installation_id: InstallationId,
    pub work_revision: ArtifactDescriptor,
    pub assembly_lock: ArtifactDescriptor,
    pub display_name: String,
    pub source: AcquisitionRecord,
    pub state_bindings: Vec<StateBindingRecord>,
    pub secret_policy: InstallationSecretPolicy,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: InstallationStatus,
}
```

Installation 是 Host-owned mutable record，不以内容摘要作为自身身份。它不能保存 raw secret、任意绝对路径或可重放的明文 credential。

### 6.8 RunRecord

```rust
pub struct RunRecord {
    pub run_id: RunId,
    pub installation_id: InstallationId,
    pub context_id: Option<String>,
    pub status: RunStatus,
    pub node_instances: Vec<NodeInstanceRecord>,
    pub bindings: Vec<ActiveBindingRecord>,
    pub started_at: DateTime<Utc>,
    pub stopped_at: Option<DateTime<Utc>>,
    pub health: RunHealth,
}
```

`host.run.start` 不隐式部署缺失节点。若 required node 没有可用 realization 或 binding，返回结构化缺口和可执行下一步。

### 6.9 Exposure 与 runtime Binding

```rust
pub struct ExposureRecord {
    pub exposure_id: ExposureId,
    pub installation_id: InstallationId,
    pub run_id: Option<RunId>,
    pub export_port: PortId,
    pub audience: Vec<ResourceSelector>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: ExposureStatus,
}

pub struct ActiveBindingRecord {
    pub binding_id: BindingId,
    pub consumer_installation_id: InstallationId,
    pub consumer_port: PortId,
    pub exposure_id: ExposureId,
    pub authority_handle_id: String,
    pub transport: SelectedTransport,
    pub expires_at: Option<DateTime<Utc>>,
}
```

跨 Installation 连接必须通过显式 Exposure。候选 provider 只显示当前 principal 有权看到且协议兼容的 export。Binding 返回 opaque ID；实际 capability handle 由 Host 注入组件，不返回给不受信任 UI。

### 6.10 OperationalIntent

```rust
pub struct OperationalIntent {
    pub schema: String,
    pub workloads: Vec<WorkloadIntent>,
    pub endpoints: Vec<EndpointIntent>,
    pub state: Vec<StatePlacementIntent>,
    pub placement: Vec<PlacementConstraint>,
    pub update_policy: UpdatePolicy,
    pub annotations: BTreeMap<String, Value>,
}
```

`WorkloadIntent` 引用 Assembly node，不直接携带 Docker command。它描述：

- 允许的 execution classes；
- CPU、memory、GPU、duration 与 concurrency 需求；
- network / filesystem / secret imports；
- replica 与 restart 需求；
- health interface；
- hard/soft placement constraints。

### 6.11 TargetInventory

扩展现有 `ExecutionTarget`，从固定 enum 走向 namespaced capability records：

```rust
pub struct TargetCapabilityRecord {
    pub capability_id: String,
    pub version: String,
    pub properties: BTreeMap<String, Value>,
    pub available: bool,
    pub evidence_refs: Vec<ArtifactDescriptor>,
}

pub struct TargetInventorySnapshot {
    pub target_id: String,
    pub observed_at: DateTime<Utc>,
    pub capabilities: Vec<TargetCapabilityRecord>,
    pub capacity: ResourceCapacity,
    pub labels: BTreeMap<String, String>,
    pub trust_zone: String,
    pub topology: Vec<TopologyRelation>,
}
```

现有 `LocalExec`、`ArtifactTransfer`、`Deployment` 等能力通过 adapter 投影成 namespaced records。不要保留两套永久枚举；迁移完成后删除旧 enum。

### 6.12 RealizationPlan 与 RealizationRevision

```rust
pub struct RealizationPlan {
    pub schema: String,
    pub installation_id: InstallationId,
    pub work_revision: ArtifactDescriptor,
    pub assembly_lock: ArtifactDescriptor,
    pub operational_intent: ArtifactDescriptor,
    pub inventory_refs: Vec<ArtifactDescriptor>,
    pub placements: Vec<NodePlacement>,
    pub transports: Vec<TransportBindingPlan>,
    pub build_actions: Vec<BuildAction>,
    pub launch_actions: Vec<LaunchAction>,
    pub state_actions: Vec<StateAction>,
    pub endpoint_actions: Vec<EndpointAction>,
    pub preconditions: Vec<ChangePrecondition>,
    pub required_authority: Vec<String>,
    pub risk_summary: Vec<String>,
}

pub struct RealizationRevision {
    pub realization_id: RealizationId,
    pub installation_id: InstallationId,
    pub plan_ref: ArtifactDescriptor,
    pub parent_realization_id: Option<RealizationId>,
    pub status: RealizationStatus,
    pub actual_resources: Vec<RealizedResource>,
    pub receipts: Vec<ArtifactDescriptor>,
    pub health: RealizationHealth,
    pub created_at: DateTime<Utc>,
    pub activated_at: Option<DateTime<Utc>>,
}
```

Planner 必须是纯函数：相同输入 refs 与 policy 产生相同 plan bytes。Executor 只接受已持久化、digest 验证通过且 authority 当前有效的 Plan。

### 6.13 有界 MVP 决策

为防止执行 Agent 把第一版扩张成分布式平台重写，MVP 固定：

- 一个 Installation 的 mutable authority 由一个 Host 拥有；多 Host 通过 Target、Exposure 与 Artifact 交换协作，不引入分布式共识数据库；
- 一个 Installation 只有一个 active WorkRevision / AssemblyLock 指针和一个默认 Run；未来并行 Run 不进入首批合同；
- Resolver 对显式候选集合做确定性、有界解析，不实现全生态 SAT solver；
- authoring graph 的变化产生新 Work/Assembly revision，运行时只允许声明为 Runtime 的 Binding 动态变化；
- 首批数据面只实现当前 Capability、Artifact 与 Endpoint 原语；其他 interaction 只保真描述，不伪装为已支持；
- 首个 Realization backend 复用现有 Docker/local/Agent 能力，不同时建设通用调度集群；
- catalog discovery 与 Installation 分离，浏览一个 Work 不授予权限、不下载可执行 artifact，也不创建 Run。

### 6.14 生命周期、幂等与不确定结果

首批状态机：

```text
Installation
resolving → ready → updating → ready
     └────→ blocked / failed
ready → removing → removed

Run
starting → running ↔ degraded → stopping → stopped
    └────→ failed / interrupted

Exposure
active → expired / revoked

Binding
selected → active → expired / revoked / broken

RealizationRevision
planned → applying → active ↔ degraded → stopping → stopped
             └────→ failed / outcome_unknown / recovery_required
```

规则：

- `removed`、`revoked`、`expired` 与某个 revision 的 `stopped` 是终态；恢复通过新 record / revision，不回写历史终态。
- 所有会产生 durable mutation 或外部 effect 的 public method 接受 `idempotency_key`。相同 key + 相同 fingerprint 重放已知结果；相同 key + 不同 fingerprint 返回 conflict。
- terminal event 只提交一次；重复 stop/revoke/remove 返回幂等结果。
- cancel、timeout、deny、partial completion 与 success 是不同 terminal state。
- effect 已发出但结果无法确认时必须进入 `outcome_unknown` 或 `recovery_required`，不得猜测失败或成功。
- active Work/Lock、Run 与 Realization 指针使用 CAS / parent precondition 更新；stale caller 不能覆盖较新状态。
- Binding 失效后默认不自动改绑。只有采用显式 policy 且新 provider 不扩大 authority、effect 与 rights 风险时，才可产生候选 rebind；实际切换仍记录新 Binding。

### 6.15 Run-bound 与 managed Realization

所有执行最终都有具体 placement，但产品不应把普通本地启动都呈现成“部署”。MVP 区分：

- **Run-bound Realization：** 只使用已安装、已验证 artifact，在当前 Host 或已预选 local Target 上启动；生命周期绑定 Run；不构建源码、不创建 public route、不改变跨 Run 的持久机器资源。它可由 `host.run.start` 在现有 run authority 内确定性生成并执行。
- **Managed Realization：** 包含构建、远程 Target、persistent workload、共享 endpoint、public route、跨 Run 资源或复杂 placement；必须先持久化 `RealizationPlan`，再显式 `apply`。

两者都产生 receipt、health 和 actual resource 记录。`host.run.start` 不能把缺失的 managed Realization 静默升级为部署；它返回 structured gap。RunRecord 引用所使用的 Realization revision，而不是复制机器细节。

## 7. Work authoring 格式

源仓库使用两个显式文件，不再把所有内容塞进 `project.yaml`：

```text
work.yaml
assembly.yaml
```

示例：

```yaml
# work.yaml
schema: plurora.work-source.v1
work:
  id: example/modular-simulation
  title: Modular Simulation
  description: A rule-driven simulation assembled from replaceable components.
  assembly: assembly.yaml
  entrypoints:
    - id: play
      intent_uri: plurora.shell.default/play
      surface_id: example/simulation-ui/play
  content:
    - content/
  rights: rights.yaml
  operational_intent: operation.yaml
```

```yaml
# assembly.yaml
schema: plurora.assembly-source.v1
assembly:
  id: example/modular-simulation-main
  nodes:
    - id: simulation
      component: packages/simulation/manifest.yaml#main
    - id: save
      component: packages/save/manifest.yaml#main
    - id: ui
      component: packages/ui/manifest.yaml#main
  bindings:
    - id: save-binding
      from: {node_id: save, port_id: save-export}
      to: {node_id: simulation, port_id: save-import}
  exposed_ports:
    - id: play
      direction: export
      target: {node_id: ui, port_id: play}
```

Port endpoint 在 source wire 中使用明确的 `{node_id, port_id}` 对象。两类 local ID 都允许 `.`，因此不使用无法无歧义解析的 `node.port` 简写。

`plurora work pack`：

1. 安全打开源文件，拒绝 symlink escape、path traversal 与超限文件；
2. 读取 Package Envelope / Component Descriptor；
3. 导入内容到 ObjectStore；
4. 生成 AssemblyRevision；
5. 解析 authoring-time bindings；
6. 生成 WorkRevision；
7. 输出 digest、closure、diagnostics，不自动安装或运行。

### 7.1 所有来源先归一化为 Work

Host 不再为“Package、原生项目、外部仓库、闭源游戏”维护彼此独立的永久安装本体。进入 Library 或 Installation 前，来源按以下规则归一化：

- 有 `work.yaml`：正常 pack 为 WorkRevision；
- 只有 Package Manifest：生成单节点 Assembly 与合成 WorkRevision，原始 Component identity 不变；
- 普通源码仓库：先建立 Workspace，只产生 inspection / BuildGraph / Work candidate，不因源码可见就自动成为可运行 Work；
- 外部 URI、本地 executable、OCI image 或远程服务：生成 ForeignCapsule WorkRevision，具体位置只保存在 Installation local binding；
- 只有内容的 bundle：生成没有可执行节点的 content Work，入口可以是 Open、Compose 或 Inspect。

未安装的 catalog 条目可以只有 WorkRevision；进入某个 Host 的日常生命周期后必须有 Installation。这样获取来源可以多样，安装、权限、状态、Run 与 Realization 不再各自复制一套类型分支。

### 7.2 分发信封不拥有 Work 身份

WorkRevision 是 Artifact DAG 的根，可以通过本地 archive、Git、OCI artifact、Package/registry 或远程服务获取；获取坐标进入 `AcquisitionRecord`，不进入 Work 的内容身份。Package 主要分发 Component，Work 主要引用和组合 Component；二者可以由同一 archive 携带，但不能互相充当永久本体。

需要离线分享时，可以定义只负责搬运 WorkRevision closure 的 Work Bundle。它复用 ArtifactDescriptor、完整性和 provenance，不获得新的 substrate 类型或官方 registry 依赖。

## 8. Resolver 与递归装配

### 8.1 Port 兼容

兼容必须同时满足：

- protocol ID 相同；
- interface ID 相同；
- export exact version 满足 import requirement；
- import required profiles 被 export 声明覆盖；
- interaction model 可直接兼容或存在显式 adapter；
- transport constraints 有交集；
- provider effect class 位于 consumer / policy 显式接受集合内；
- multiplicity 未超限。

不能只看 capability 字符串相同。

### 8.2 Adapter

Adapter 是普通 Component。它显式导入一个 Port、导出另一个 Port，并声明转换行为与 effect。Resolver 不能隐藏自动 schema coercion。

### 8.3 递归 flatten

Resolver 可以为执行生成 flatten graph，但必须保留：

- 原始 nested Assembly identity；
- 节点路径，如 `backend/matchmaking/store`；
- 每层 exposed port mapping；
- provenance 与 lock refs。

执行层可以扁平，用户和审计层不能丢失封装边界。

### 8.4 Binding phase

- `Authoring`：Work pack 时固定，进入 WorkRevision。
- `Installation`：安装时由解析器、用户或 policy 选择，进入 AssemblyLock。
- `Launch`：每次 Run 启动前选择，可保存 preference，但必须重新确认可用性和 authority。
- `Runtime`：运行期间通过 Exposure + lease 接入，断开后按 availability policy 处理。

### 8.5 Powerbox

官方产品中的 Powerbox 流程：

```text
Component 需要某个 import
→ Host 计算协议兼容、可见且有权使用的候选 Exposure
→ Shell 显示 provider、来源、trust、scope、duration、数据与 effect 风险
→ 用户或显式 policy 选择
→ Host 铸造最小 authority handle
→ Runtime 注入 binding
→ 到期、撤销、provider 停止或协议漂移时失效
```

没有候选时显示如何安装、启动或实现 provider；多个候选时不按 publisher 自动选择。

### 8.6 更新、替换与回滚

WorkRevision 永不原地修改。Installation 更新先解析候选 WorkRevision 与候选 AssemblyLock，再产生结构化差异：

- Component artifact / behavior digest；
- Port、Protocol/Profile 与 Binding；
- content roots；
- Rights 与 Transparency；
- authority、network、filesystem、secret 与 effect 请求；
- StateSlot schema、owner、portability 与 migration；
- OperationalIntent 与现有 Realization 的影响。

更新通过 ChangeSet 执行：检查 source/digest 和当前 Installation 前置条件，建立所需 state snapshot，运行显式 migration，验证候选 Run 或 Realization，最后原子切换 active Work/Lock 指针。失败时保持原指针；切换后仍保留有界 rollback 信息。新增权限、权利限制、状态清空或不透明迁移都必须单独展示并获得明确决定。

Component replacement 只有在 Port 合同兼容、required binding 可满足、durable state 已迁移或被明确重置时才可激活。远程 provider 版本漂移、Exposure 到期和 authority 撤销属于 Binding 失效，不得静默改写 Installation lock。

## 9. Capability 控制面与高频数据面

MVP 支持：

- `CapabilityUnary` 与 `CapabilityStream`：复用当前 invoke/stream/cancel 与 handle。
- `Endpoint`：复用 port lease、proxy、authenticated tunnel，但绑定到 Realization 与 Port。
- `Artifact` / `Snapshot`：通过 ObjectStore descriptor 传输，不把大内容塞进 RPC。

随后扩展：

- EventStream；
- DuplexStream；
- local IPC；
- WASM Component direct binding；
- shared memory / ring buffer；
- engine-native bridge。

任何新 transport 都必须保留相同 principal、authority、deadline、cancel、effect 与 audit 语义。Rust trait 或 in-process pointer 不是公开协议。

## 10. 部署是编译，不是 Agent shell

### 10.1 Build discovery

对于普通开源仓库，Agent 可以产生：

- `SourceInspectionReport`；
- 候选 `BuildGraph`；
- 候选 `OperationalIntent`；
- 风险、未知项与需要用户回答的问题。

这些都是 Artifact，不能直接执行。用户批准后才成为 WorkRevision 或 ChangeSet 的输入。

### 10.2 BuildGraph

BuildGraph 用 provider protocol 表达 builder，而不是通用 shell 字符串：

```text
source inputs
→ build node (builder protocol + typed parameter artifact)
→ verification node
→ immutable outputs
→ provenance / receipt
```

MVP 形状：

```rust
pub struct BuildGraph {
    pub schema: String,
    pub source_inputs: Vec<ArtifactDescriptor>,
    pub nodes: Vec<BuildNode>,
    pub outputs: Vec<BuildOutputDeclaration>,
}

pub struct BuildNode {
    pub node_id: String,
    pub builder_port: PortContract,
    pub input_refs: Vec<ArtifactDescriptor>,
    pub parameter_ref: ArtifactDescriptor,
    pub network_policy: BuildNetworkPolicy,
    pub expected_outputs: Vec<String>,
    pub verification_requirements: Vec<ProtocolRequirement>,
}

pub struct BuildOutputDeclaration {
    pub output_id: String,
    pub artifact_type_uri: String,
    pub media_type: String,
    pub executable: bool,
    pub protocol_claims: Vec<ProtocolImplementationDeclaration>,
}
```

BuildGraph 不携带 raw shell script、secret value 或 mutable workspace path。具体 builder adapter 可以把 typed parameters 转成固定 argv / sandbox action；实际 source snapshot 与输出都以 artifact digest 绑定。

首批 builder adapter：

- existing Dockerfile builder；
- existing Nixpacks path；
- prebuilt artifact passthrough。

Cloud Native Buildpacks、Cargo、npm、Godot export、WASM component build 可以以后以 provider 加入。它们不是 substrate enum。

### 10.3 Planner / Builder / Verifier / Operator / Observer

平台内 Agent 协调按角色分权：

| 角色 | 可以做什么 | 不可以做什么 |
|---|---|---|
| Planner | 读允许的源码与状态，起草 Work/Assembly/Plan/ChangeSet | 执行机器 effect |
| Builder | 在受限环境将已批准 inputs 转为 artifacts | 修改 Installation authority |
| Verifier | 运行检查，产生 evidence | 激活新 revision |
| Operator | 按批准的 RealizationPlan 调用 Host effects | 自行改变计划 |
| Observer | 读健康、日志、receipt，提出恢复建议 | 自动获得 deploy 权限 |

同一个模型可以扮演多个角色，但每次调用的 principal、handle、budget 和 effect boundary 必须不同。

### 10.4 Existing deployment migration

当前 `DeploymentRevision`、Docker build/deploy、Target Agent operation、private preview、approval、reconcile、recover、rollback 不删除能力，只重构所有权：

- Project ID → Installation ID；
- Docker-specific request → RealizationPlan 中的 backend action；
- DeploymentRevision → RealizationRevision；
- current target capability enum → TargetInventory；
- route / port / container 字段 → actual_resources 与 receipts；
- verified ChangeSet、preview、approval refs 原样保留为 evidence。

首个 executor 仍可以只支持单 workload Docker/Agent realization，但数据模型不能把 Docker 写成唯一长期形态。

## 11. Foreign Work 与闭源入口

### 11.1 不使用“闭源项目类型”

闭源由以下事实组合而成：

- source visibility；
- artifact availability；
- rights；
- protocol ports；
- state portability；
- trust class；
- entitlement requirements。

### 11.2 ForeignCapsule

```rust
pub struct ForeignCapsuleDescriptor {
    pub capsule_id: String,
    pub launch_requirements: Vec<ForeignLaunchRequirement>,
    pub protocol_ports: Vec<PortDescriptor>,
    pub state_slots: Vec<StateSlotDescriptor>,
    pub rights: ArtifactDescriptor,
    pub transparency: ArtifactDescriptor,
}
```

Portable Work 不能嵌入用户本机可执行文件绝对路径。Installation 通过 local binding 满足 launch requirement：

- external URI；
- 已安装本地 executable；
- managed binary artifact；
- OCI image；
- remote service；
- store / entitlement adapter。

### 11.3 Derived integration depth

UI 可以根据事实计算：

- External Link；
- Managed Capsule；
- Protocol Participant；
- Composable Work。

这不是保存到 substrate 的等级字段。若一个闭源程序实现公开 save、health、mod 或 lobby protocol，它可以正常组合；若一个开源程序没有协议，它也可能只是 Foreign Capsule。

### 11.4 不建设 DRM

Plurora 只调用普通 entitlement adapter，记录允许的操作和失败原因。平台不复制外部商店的所有权数据库，也不把 DRM 变成基底要求。

## 12. 官方产品：Affordance-based Library

Home 不再只显示 Project 卡片。官方 Library 读取 Work、Installation、Run、Rights、Target 与当前 authority，计算当前可执行动作：

```text
Open
Play
Edit
Fork
Compose
Install
Update
Run
Stop
Deploy
Expose
Connect
Inspect
Backup
Export
Remove
```

`LibraryAffordanceResolver` 属于官方 client-core，返回：

```rust
pub struct Affordance {
    pub action: String,
    pub available: bool,
    pub reason_code: Option<String>,
    pub risk: Option<String>,
    pub next_step: Option<String>,
}
```

它只负责 UX，不替代 Host 授权。实际调用仍 fail closed。

示例：

| 条目 | 可能动作 |
|---|---|
| Plurora-native 开源游戏 | Play、Edit、Fork、Compose、Deploy、Export |
| 闭源本地游戏 | Play、Inspect、Backup |
| 开源 Web 服务 | Edit、Run、Deploy、Expose |
| 远程闭源服务 | Connect、Inspect、Disconnect |
| 共享地图或规则包 | Open、Compose、Export |
| 运行中的多人后端 | Inspect、Expose、Stop、Rollback |

简单用户看到主要动作；高级用户展开 Work digest、Assembly graph、bindings、authority、rights、realization 与 receipts。

## 13. Public Contract 的破坏性重置

### 13.1 删除

删除 method：

```text
host.project.list
host.project.get
host.project.start
host.project.stop
host.project.status
```

删除 event：

```text
host/project.installed
host/project.started
host/project.stopped
host/project.uninstalled
```

删除 `ProjectDescriptor`、`ProjectRegistry`、`ProjectState`、`ProjectType`、`project.yaml`、`~/.plurora/projects/`、Project CLI、Web Project DTO 与对应 schema/SDK/tests/docs。

删除当前 `CompositionDescriptor`、`CompositionLock`、`composition.yaml`、`init-composition`、`composition check`。其功能由 Work/Assembly tooling 替代。

不保留旧方法 alias、数据 reader、目录 fallback 或 deprecation output。

### 13.2 新增 Installation / Run methods

```text
host.installation.list
host.installation.get
host.installation.create
host.installation.update
host.installation.remove

host.run.list
host.run.get
host.run.start
host.run.stop
host.run.status
```

### 13.3 新增 Exposure / Binding methods

```text
host.exposure.list
host.exposure.create
host.exposure.revoke

host.binding.list
host.binding.candidates
host.binding.select
host.binding.revoke
```

### 13.4 新增 Realization methods

```text
host.realization.plan
host.realization.apply
host.realization.get
host.realization.list
host.realization.stop
host.realization.rollback
host.realization.reconcile
```

### 13.5 新事件

```text
host/installation.created
host/installation.updated
host/installation.removed

host/run.starting
host/run.started
host/run.stopping
host/run.stopped
host/run.failed

host/exposure.created
host/exposure.revoked
host/exposure.expired

host/binding.selected
host/binding.revoked
host/binding.expired

host/realization.planned
host/realization.applying
host/realization.active
host/realization.stopped
host/realization.failed
host/realization.rolled_back
host/realization.reconciled
```

每个 ID 只出现一次。更新 `PlatformMethod`、registry、dispatcher、schema exporter、OpenAPI、Rust/TS SDK、public contract、event registry、identity gate 与 conformance。不要创建双栈。

### 13.6 Host action 与 resource selector

把当前围绕 Project 的固定授权映射重置为可审计 action + exact resource selector：

| Action | 典型方法 | 必需 resource |
|---|---|---|
| `observe` | list/get/status/candidates | 可见的 Work、Installation、Run、Target、Realization |
| `installation.manage` | installation create/update/remove | Installation；创建时另含 acquisition/source scope |
| `run` | run start/stop | exact Installation 与 Run |
| `binding.manage` | binding select/revoke | consumer Installation、import Port、Exposure |
| `exposure.manage` | exposure create/revoke | provider Installation、Run 与 export Port |
| `realization.plan` | realization plan | Installation 与候选 Target |
| `realization.apply` | apply/stop/rollback/reconcile | Installation、Target 与 Realization |
| `develop.propose` | Work/Workspace ChangeSet draft | exact Workspace / Installation |
| `develop.approve` | approve/reject | exact ChangeSet |
| `develop.execute` | apply/promote | exact ChangeSet、Workspace / Installation 与 effect resources |

Resource kinds 至少包括 `work`、`workspace`、`installation`、`run`、`target`、`exposure`、`binding` 与 `realization`。默认设备邀请仍只授予 `observe`。通配 selector 必须显式显示，不因省略 ID 自动产生。长操作在 build、state migration、endpoint、activation 与 rollback 等每个外部 effect 前刷新 grant、祖先 delegation、lease 与 resource match。

Root credential 仍是 Host 维护入口，不注入 Component、Surface 或 Agent。Agent 每个角色只获得其任务所需 action 和 exact resources。

### 13.7 结构化失败原因

公开错误与 diagnostics 至少区分：

```text
work_invalid
assembly_cycle
artifact_missing
artifact_digest_mismatch
port_unresolved
port_incompatible
binding_ambiguous
binding_unavailable
binding_expired
unsupported_interaction
state_migration_required
state_reset_required
rights_blocked
entitlement_required
authority_denied
target_unsatisfied
plan_stale
plan_digest_mismatch
approval_required
outcome_unknown
recovery_required
unsupported_backend
```

Reason code 稳定、可本地化且不包含异常正文、绝对路径、secret 或 raw stderr。详细诊断通过脱敏 evidence/reference 获取。`absent`、`forbidden`、`unsupported`、`stale` 与 `unavailable` 不得都折叠成空列表。

## 14. Host 数据布局与权威

新布局：

```text
~/.plurora/
├── objects/
├── installations/
│   └── <installation_id>/
│       ├── installation.json          # materialized projection, not sole authority
│       ├── assembly.lock.json
│       ├── secrets.dat
│       ├── state/
│       └── diagnostics/
├── workspaces/
│   └── <workspace_id>/
│       ├── workspace.json
│       └── source/
└── runtime/
    └── ...
```

权威规则：

- WorkRevision、AssemblyRevision、Lock、Intent、Plan 在 ObjectStore。
- Installation、Exposure、Binding、Realization 的 durable authority 在 Host journal；JSON 文件是可重建 projection。
- Run 的活跃状态在内存与 journal terminal records；Host restart 将未完成 Run 标记 interrupted，不猜测成功。
- 用户 state 位于 installation state root 或外部 provider；不写入 Work artifact。
- linked local Workspace 路径只能保存为 Host-local binding，永不进入 portable WorkRevision。

## 15. 代码组织

### 当前仓库迁移地图

| 当前事实来源 | 目标归属 |
|---|---|
| `crates/plurora-core/src/project.rs` | 删除；portable 定义进入 `plurora-work`，Host mutable 数据进入 Installation |
| `crates/plurora-runtime/src/project_registry.rs` | `plurora-service/src/installations.rs` 的 journal + projection |
| `crates/plurora-core/src/component.rs::CompositionLock` | `plurora-work::AssemblyLock` |
| `crates/plurora-cli/src/cli.rs::CompositionDescriptor` 与 `commands/composition.rs` | `work.yaml` / `assembly.yaml` reader、resolver 与 Work CLI |
| `crates/plurora-runtime/src/runtime/protocol/projects.rs` | Installation 与 Run protocol handlers |
| `crates/plurora-service/src/development.rs` | 保留 ChangeSet/evidence 核心，scope 从 Project 改为 Installation/Workspace |
| `crates/plurora-service/src/lib.rs::DeploymentRevision` 与 deployment projection | 拆入 `realization/` 并改为 RealizationRevision |
| `crates/plurora-runtime/src/target_deployment.rs` | Realization 的首个 Docker/local backend executor |
| `crates/plurora-runtime/src/runtime/local_exec.rs::ExecutionTarget` | TargetInventory source 与现有 capability adapter |
| `clients/web/src/routes/home/use-home-projects.ts` | Library query 与 Affordance resolver |
| `clients/web/src/routes/project-frame.tsx` | Work entry / Installation frame |
| `clients/web/src/lib/project-deployment.ts` 与 `client-core/project-target-context.ts` | Realization client 与 Installation/Run context |
| World Bundle 中的 `CompositionLock` | AssemblyLock，同时保留历史 replay / re-execution 分离 |

迁移过程中优先移动 owner 和类型，再移动 UI 命名；不要在 `plurora-service/src/lib.rs` 中再造一套平行实现。

### 新 crate

```text
crates/plurora-work/
├── src/lib.rs
├── src/ids.rs
├── src/canonical.rs
├── src/port.rs
├── src/assembly.rs
├── src/work.rs
├── src/state.rs
├── src/rights.rs
├── src/operational.rs
├── src/lock.rs
├── src/source.rs
├── src/resolver.rs
└── src/diagnostic.rs
```

要求：

- 只依赖 `plurora-core` 和纯数据/解析库；
- 不依赖 runtime、service、Web、Docker 或本机目录；
- 所有解析、canonicalization、validation 和 planning pure helpers 可单测；
- 不把 Host mutable record 放入该 crate，除非只是 wire DTO。

### Runtime

```text
crates/plurora-runtime/src/component_ports.rs
crates/plurora-runtime/src/assembly_runtime.rs
crates/plurora-runtime/src/binding_runtime.rs
```

### Service

不要继续扩大 `plurora-service/src/lib.rs`：

```text
crates/plurora-service/src/installations.rs
crates/plurora-service/src/runs.rs
crates/plurora-service/src/exposures.rs
crates/plurora-service/src/bindings.rs
crates/plurora-service/src/realization/mod.rs
crates/plurora-service/src/realization/planner.rs
crates/plurora-service/src/realization/executor.rs
crates/plurora-service/src/realization/projection.rs
```

### CLI

```text
plurora work init|check|pack|inspect
plurora installation list|info|create|update|remove
plurora run list|info|start|stop
plurora binding candidates|select|list|revoke
plurora realization plan|apply|list|info|stop|rollback|reconcile
```

### Web

逐步将：

```text
Project card         → Library item
Project frame        → Work / Installation entry frame
Project console      → Installation workbench
Project deployment   → Realization panel
Project context      → Installation / Run / Realization context
```

UI 名称可以保留用户容易理解的“项目”，但 DTO、方法和持久对象必须使用准确模型。

## 16. 实施 Phase

### Phase 1 — Candidate model 与 canonical artifacts

实现：

- 新增 `plurora-work` crate 与第 5–6 节全部纯类型。
- ID、canonical JSON、digest、validation、raw secret / path redline。
- 新增 Experimental protocol descriptors。
- schema exporter 和 SDK 暴露新 top-level types。
- 不改现有 Project/Composition runtime。

检查：

- canonical bytes 对 map 顺序稳定；
- Work/Assembly digest 可复现；
- inclusion cycle、bad port、bad state、raw secret/path 被拒绝；
- unknown annotations 保留；
- `cargo test -p plurora-work`；
- schema/SDK 两次生成 hash 一致。

提交：

```text
feat(work): define portable work and assembly model
```

### Phase 2 — Resolver、Work authoring 与 Composition 替换

实现：

- `work.yaml` / `assembly.yaml` source reader；
- Package、普通源码仓库、Foreign Capsule 与 content-only source 的 Work 归一化；
- package/component artifact 导入；
- capability-to-port projection；
- recursive resolver、port compatibility、adapter diagnostics；
- `plurora work init|check|pack|inspect`；
- `AssemblyLock`；
- 转换现有 composition examples、creator templates、World Bundle；
- 删除 CompositionDescriptor、CompositionLock、旧 CLI 和文档。

检查：

- nested Assembly 可封装并暴露 port；
- replacement 不改变 content roots；
- unresolved install/launch/runtime import 有结构化 diagnostics；
- ambiguous provider 不按 publisher 选择；
- World Bundle 使用 AssemblyLock 并保持 replay / branch 语义。

提交：

```text
feat(assembly): resolve recursive work assemblies
```

### Phase 3 — Installation registry 与 Project 后端替换

实现：

- Installation journal、projection、filesystem layout；
- Installation create/update/remove，并实现候选 Work/Lock diff、state snapshot/migration 与 rollback 指针；
- Workspace 与 Installation 分离；
- Install lab 产出 WorkRevision + Installation；
- 新 `host.installation.*`；
- CLI installation commands；
- 删除 ProjectDescriptor、ProjectRegistry、project methods/events/schema。

安全：

- linked local source 永不删除；
- managed workspace containment；
- remove 的 keep/delete state 决策显式；
- 不读取旧 projects 目录。

提交：

```text
feat(host): replace projects with installations
```

### Phase 4 — Run lifecycle 与官方 Library

实现：

- RunRegistry、node activation、fixed Installation bindings；
- `host.run.*` 与新 lifecycle events；
- Web Home 改为 Library；
- Affordance resolver；
- entrypoint launch；
- Settings、storage、secret、failure UI 改用 Installation ID；
- 删除 Project UI/DTO/route 命名。

检查：

- 打开条目不隐式部署；
- required realization 缺失时给出下一步；
- 关闭标签不停止 Run；
- stop/failed/restart 恢复清楚；
- mobile/PWA 和 device authority 仍按 exact installation selector 工作。

提交：

```text
feat(product): introduce installation library and run lifecycle
```

### Phase 5 — Exposure、Powerbox 与跨 Installation Binding

实现：

- ExposureRegistry 与 lease/revoke；
- candidate calculation；
- Powerbox methods 与 Web chooser；
- runtime handle injection；
- provider stop、revoke、expiry、version drift 失效；
- binding preference 只是 hint，每次 launch 重新验证。

检查：

- 无 ambient service discovery；
- consumer 只获得选中 port 的最小 handle；
- cross-installation state 不被自动共享；
- third-party provider 与第一方平等；
- multiple providers 时明确要求选择。

提交：

```text
feat(binding): add scoped cross-installation powerbox
```

### Phase 6 — Realization compiler 与部署重构

实现：

- OperationalIntent、TargetInventory、pure planner；
- `host.realization.*`；
- 迁移现有 Docker/Agent executor、preview、approval、reconcile、recover、rollback；
- DeploymentRevision → RealizationRevision；
- Project selector → Installation selector；
- Web deployment panel → Realization panel；
- CLI realization commands。

检查：

- plan digest 稳定；
- planner 无 effect；
- apply 前重新验证 Plan digest、preconditions、approval 和 authority；
- Host restart 恢复或 fail closed；
- local 与 Agent target 使用同一 plan/receipt；
- rollback 不读取 live workspace；
- Docker 只是首个 backend。

提交：

```text
feat(realization): compile assemblies onto execution targets
```

### Phase 7 — Foreign Work、Rights 与闭源入口

实现：

- Rights / Transparency / ForeignCapsule；
- external URI、local executable binding、managed artifact、OCI image、remote service、entitlement adapter；
- Library trust / rights disclosure；
- opaque state backup；
- dedicated server entrypoint；
- 不允许 portable descriptor 携带本机绝对路径。

检查：

- 开源无协议可以是 Capsule；
- 闭源有协议可以参与 Binding；
- denied/unspecified rights 阻止自动复制或导出；
- entitlement failure 不泄漏 credential；
- 平台没有 DRM 特权路径。

提交：

```text
feat(foreign-work): support rights-aware opaque entries
```

### Phase 8 — 第一个真正可继续创作的游戏 Kit

把现有 playable creation 能力重组为一个有实际用途的 `modular-simulation` Work：

- Rust isolated simulation component；
- static Web renderer；
- input port；
- portable save provider；
- optional AI provider；
- inspector/editor Surface；
- local Run；
- optional remote server Realization；
- Work fork、component replacement、state migration。

不要做通用 3D 引擎。选择规则驱动、状态丰富、适合分支与组合、对每帧极端性能要求较低的 simulation / board / strategy starter。

新增“提升为组件”工作流的最小版本：选择一个 Assembly 子图，计算外部 imports/exports/state，生成新的 nested Assembly 与 diagnostics。不要让 Agent 自动发布；输出 candidate artifact。

提交：

```text
feat(creator): ship a modular simulation work kit
```

### Phase 9 — 收口

- 删除本文及英文版本；
- 删除临时 active-program 根 `AGENTS.md`，或只在仍有长期价值时替换为通用仓库规范；
- 把长期模型写入 architecture/spec；
- 更新 Product Model、guides、ALPHA_STATUS、NEXT_STEPS；
- 删除全部 Project/Composition/Deployment 旧术语的机器身份；
- 保留必要的普通英文“project”描述时，不得指代旧 DTO；
- 完整生成与 CI；
- fast-forward 合并 main；
- 删除 feature branch。

提交：

```text
docs(platform): converge work assembly and realization model
```

## 17. 每个 Phase 的完成定义

每个 Phase 提交前：

```text
cargo fmt --check
cargo metadata --no-deps
目标 crate tests
受影响 TypeScript tests/typecheck
python scripts/check-docs.py
python scripts/check-identity.py
git diff --check
生成物 clean check（若相关）
```

推送后观察 GitHub CI。大型检查只在 CI：

```text
cargo check --workspace
cargo test --workspace --all-targets --locked
full conformance
Web typecheck/test/build/PWA
Desktop sidecar smoke
External project Host operations acceptance
Windows backup/restore
Docker verified artifact / realization smoke
```

若 CI 失败，先修复当前 Phase 并追加明确 fix commit；不要开始下一 Phase。

## 18. 必须新增的质量覆盖

### Model

- canonicalization；
- logical ID 与 digest 分离；
- nested closure；
- unknown artifact preservation；
- rights/transparency claim 与 evidence 分离。

### Resolver

- version/profile/interaction mismatch；
- inclusion cycle；
- cardinality；
- authoring/install/launch/runtime phase；
- adapter；
- ambiguous provider；
- state migration requirement。

### Host

- Installation journal rehydrate；
- Run interrupted on restart；
- Exposure lease/revoke；
- exact resource authority；
- linked local source preservation；
- state keep/delete；
- no raw secrets。

### Realization

- deterministic plan；
- stale inventory/precondition；
- authority refresh before effects；
- partial effect / unknown outcome；
- replay without live source；
- rollback；
- local/Agent parity；
- target capability mismatch。

### Product

- affordance reason codes；
- closed/open combinations；
- simple mode and advanced details；
- mobile remote control；
- Host switching cache isolation；
- failure recovery。

## 19. 安全威胁与资源预算

### 19.1 必须覆盖的威胁

| 威胁 | 必需控制 |
|---|---|
| 恶意 Assembly 制造递归、引用或候选爆炸 | inclusion cycle 检查、深度/节点/Binding/candidate 硬上限、迭代解析、超限 reason code |
| dependency confusion 或 publisher 冒充 | logical ID + exact digest、来源/provenance 分离、无 publisher priority、显式 provider |
| Component 虚假声明 Protocol 或 trust | claim/evidence/enforced boundary 分栏、behavior conformance、实际 trust class |
| 跨 Installation Binding confused deputy | consumer/provider/Port/audience/lease 精确绑定，handle 只由 Host 注入，调用时重新验证 |
| stale inventory、Plan 或 approval 的 TOCTOU | inventory ref、Plan digest、parent/CAS precondition、effect 前 authority refresh |
| Agent 被源码或日志 prompt injection 诱导执行 | 外部文本视为不可信数据；Agent 只产 candidate Artifact/ChangeSet，无 ambient shell/secret |
| Foreign binary 逃逸或过度访问 | 明确 ForeignCapsule / trusted class、OS boundary、最小文件/网络/secret grants、无虚假组合保证 |
| state 被替换组件读取或外泄 | StateSlot owner、migration Port、no automatic sharing、outbound policy 与 receipt |
| Rights / Transparency 自报欺骗 | 声明与 evidence 分离；未验证状态清楚展示；不把声明转成平台 authority |
| Target 冒充能力或伪造完成 | 认证 target identity、Host-observed inventory、lease epoch、verifier evidence、typed receipt |
| artifact / archive bomb 与磁盘耗尽 | decoded size、closure、单对象、总传输、解压比例和存储 quota；写入前预算检查 |
| error / log 泄漏 secret 与本机信息 | reason code、redaction、reference-based diagnostics、无 raw stderr/absolute path |

### 19.2 首批硬上限

这些是实现安全上限，不是永久协议最大值；Profile 可以降低，调用方不能关闭。超限返回结构化 `work_too_complex` / `artifact_budget_exceeded`，以后通过新实现版本调整：

```text
单个 YAML source descriptor             1 MiB
单个 canonical metadata artifact        4 MiB（大内容必须独立引用）
单个 Assembly nodes                     4,096
单个 Assembly bindings                  16,384
最大嵌套深度                            32
单节点 ports                            1,024
exposed ports / state slots             各 4,096
单次 provider candidates                256
单个 RealizationPlan actions            4,096
单次返回 diagnostics                    1,000（其余给出截断计数）
```

Resolver 使用 digest memoization 和有界并发；不得递归复制整个 nested graph。大 Artifact 传输必须 stream、backpressure、cancel、digest verify，并在写入前确认预算。性能优化不能绕过验证或 authority。

## 20. 明确不做

本轮不做：

- 通用 3D/2D rendering engine；
- ECS、scene、entity、inventory、quest 等平台标准；
- Kubernetes 替代品；
- marketplace、支付、推荐算法；
- DRM 或官方 entitlement database；
- 任意 root shell agent；
- 每帧 JSON-RPC；
- 自动把任意开源仓库宣称为可组合作品；
- 为旧 Project / Composition 数据保留 compatibility；
- 把所有 target backend 一次实现完；
- 为了增加数量继续创建 `*-lab`。

## 21. 最终验收场景

全部完成时，至少能真实完成以下场景。

### 场景 A：组合开放游戏

1. 从 `work.yaml` pack 一个 WorkRevision；
2. Assembly 包含 Rust simulation、Web UI、save provider 和 optional AI provider；
3. 安装时选择 save provider；
4. 启动 Run；
5. 替换 simulation Component，portable state 经 migration 保留；
6. 把内部 save + sync 子图封装为 nested Assembly，供另一个 Work 使用。

### 场景 B：Agent 辅助部署开源服务

1. 导入 Git source 到 Workspace；
2. Agent 产生 SourceInspectionReport、BuildGraph 与 OperationalIntent candidate；
3. 用户批准 ChangeSet；
4. builder 生成 immutable artifact 与 provenance；
5. planner 针对 local 或 Agent Target 生成相同语义的 RealizationPlan；
6. apply、health、restart、rollback；
7. replay 不重新读取 live source。

### 场景 C：闭源本地游戏

1. 导入 catalog metadata 与 Rights/Transparency declaration；
2. Installation 绑定用户本地 executable 或 store adapter；
3. Library 显示 Play、Inspect、Backup，不虚构 Edit/Compose；
4. opaque save 被备份并可恢复；
5. 如果 dedicated server artifact 和权利允许，可单独生成 Realization；
6. 如果程序实现公开 save/mod/lobby protocol，可以出现在 Powerbox 候选中。

### 场景 D：跨 Installation 运行能力

1. 一个运行中的 backend Installation 显式 expose `game.save/v1`；
2. 第二个 Work 的 launch-time import 请求该能力；
3. 用户在 Powerbox 中选择 provider 与租期；
4. Host 注入最小 handle；
5. provider stop、grant revoke 或 lease expiry 后 binding 失效；
6. consumer 按 availability policy 降级或停止，不获得其他 backend 权力。

## 22. 外部设计参考

只借用成熟边界，不照搬产品本体：

- WebAssembly Component Model / WIT worlds：imports、exports、world 与递归 composition；
  - <https://component-model.bytecodealliance.org/design/worlds.html>
  - <https://component-model.bytecodealliance.org/design/components.html>
- OCI Content Descriptor：media type、digest、size、content-addressed DAG 与未知类型保留；
  - <https://github.com/opencontainers/image-spec/blob/main/descriptor.md>
- in-toto Attestation Statement：immutable subject 与 typed predicate 分离；
  - <https://github.com/in-toto/attestation/blob/main/spec/v1/statement.md>
- Cloud Native Buildpacks lifecycle：detect/build/export 等构建阶段与平台编排分离；
  - <https://buildpacks.io/docs/for-platform-operators/concepts/lifecycle/>

这些规范通过 adapter 或数据形状启发 Plurora；Plurora 不要求所有 Component 都是 WASM、所有 artifact 都进入 OCI registry、所有 build 都使用 Buildpacks。

## 23. 一句话约束

**Work 定义作品，Assembly 定义构成，Installation 拥有本地选择与用户状态，Run 表示活着的实例，Realization 把它编译到现实机器；任何一层都不能反向把自己的产品本体写进 Plurora 的物理法则。**
