# Plurora 宪法 v2（候选）

> [English](./CONSTITUTION_V2.en.md) · [中文](./CONSTITUTION_V2.md)

> 状态：候选架构。当前 [`CHARTER.md`](../CHARTER.md) 与
> [`PUBLIC_CONTRACT.md`](../spec/PUBLIC_CONTRACT.md) 仍是仓库现行契约。
> 本文只有在显式采纳后才取代其中的边界；采纳前不应宣称已有 v2 实现。

## 核心承诺

Plurora 承载可移植、可组合、可审计并能长期演化的应用、工具、服务、世界、游戏、agent 与其他数字形态。
模型、agent、组件、引擎、客户端和 Host 都可以替换；重要内容、历史与所有权不属于任何单一实现或服务。

产品可以把「世界」作为媒介身份，但宪法基底不把 `World` 固化为系统本体。基底只认识身份、权力、对象、引用、因果、效果和证明；世界由上层协议定义。

一句话边界：

> 基底拥有物理法则，协议公地拥有共同语言，发行版和产品拥有观点；数据与内容属于用户和创作者。

## 为什么需要新的边界

现有设计在可扩展性上很强：包可以声明能力、事件、权限、surface 和多种 entry form。但长期生命力还要求另外四件事：

- **可演化性：** 错误抽象可以被废弃、迁移和替换。
- **可移植性：** 宿主或组件消失后，内容与历史仍可带走。
- **可组合性：** 独立作者只凭共同协议即可互操作，而不只共享 JSON 形状。
- **可治理性：** 协议有成熟度、版本协商、支持窗口、迁移工具和行为一致性测试。

「内核不认识内容概念」并不等于内核没有本体。`session`、`package`、`project`、`proposal`、`surface`、`target`、`proxy` 同样是本体。任何进入长期稳定合同的名词，都必须满足“无法安全移到上层”的严格下沉门槛。

## 不可变原则

### 1. 权力必须显式

Manifest 声明的是请求上限，不是实际权威。实际权威由不可伪造、可衰减、可租赁、可撤销的 capability 表达。

所有跨边界行为都必须能回答：谁、对什么资源、在什么条件下、获得了什么权力、权力来自哪里、何时失效。

### 2. 第一方实现没有特权

官方组件、协议实现和 shell profile 与第三方使用相同的注册、授权、调用、审计和 conformance 机制。官方身份可以表达维护责任，不能表达隐式权威或路由优先级。

### 3. 公共合同高于内部调用

宿主内调用、HTTP、stdio、WASM import、远程调用和未来传输必须呈现相同的授权与行为语义。内部实现不能依靠私有旁路获得生态能力。

### 4. 重要对象具有内容身份

可移植对象通过内容摘要而不是宿主路径或进程内 ID 获得稳定身份。引用至少携带类型、摘要和大小；消费者必须能够验证内容未被替换。

### 5. 非确定性与外部效果必须留下收据

模型调用、工具调用、网络请求、进程执行和其他不可逆或非确定性行为必须产生可审计的 effect receipt。Receipt 记录引用和决策，不复制 raw secret 或不必要的用户内容。

### 6. 历史重放与重新执行必须分离

- **历史重放**使用已经记录的输出和 receipt，不重新触发外部效果。
- **重新执行**使用当前组件、模型和策略，必须创建新的因果分支。

平台不得静默混合两种模式。

### 7. 共同语义属于协议公地

Schema 说明数据形状；协议还必须说明含义、生命周期、错误、安全要求和行为。Agent、World、Memory、Surface、Evaluation 等共同语言属于可竞争、可迁移的协议，不属于宪法基底，也不应退化为某个包的私有约定。

### 8. Package 是分发信封，不是本体单位

Package 可以携带组件、协议描述、内容、surface 或适配器，但这些工件拥有独立身份、版本、摘要、依赖和迁移逻辑。升级一个执行组件不应强迫迁移全部内容；导出内容不应强迫携带宿主 UI 和可执行代码。

### 9. 调用模型可以统一，信任模型必须显式不同

WASM、进程、远程服务和 native in-process 可以实现相同协议，但不能宣称具有相同隔离、故障、延迟或供应链保证。每个实现必须公开自己的 trust class 与实际强制边界。

### 10. 错误抽象可以退出历史舞台

稳定不是 additive-only 的永久堆积。每个稳定协议都必须有弃用、支持窗口、迁移与 versioned reader 机制。兼容 reader 可以长期读取旧数据，但旧抽象不因此继续获得新的行为能力。

## 分层模型

```text
┌──────────────────────────────────────────────┐
│ Experiences / Worlds / Products              │
│ 强观点、可分叉、属于用户                       │
├──────────────────────────────────────────────┤
│ Shell Profiles                               │
│ Web / Desktop / VR / IDE / headless 映射      │
├──────────────────────────────────────────────┤
│ Protocol Commons                             │
│ 共享语义、profiles、迁移、行为 conformance     │
├──────────────────────────────────────────────┤
│ Components & Adapters                        │
│ WASM / process / remote / trusted native      │
├──────────────────────────────────────────────┤
│ Constitutional Substrate                     │
│ 身份、权力、对象、日志、效果、事务、流、证明      │
└──────────────────────────────────────────────┘

Registry / Governance / Provenance 横跨所有层。
Host Control Plane 与上述层正交，管理本机安装、进程、端口、代理、secret 和部署。
```

## Constitutional Substrate：宪法基底

基底只拥有无法在用户空间安全重复实现的机制。

### 基底拥有

- principal 与认证后的调用上下文；
- capability mint、attenuate、delegate、lease、refresh、revoke；
- 内容寻址对象与可验证引用；
- 只追加 journal、稳定排序、明确的因果引用与 head；
- effect receipt 与 provenance 连接点；
- compare-and-swap、前置条件、原子 commit 与幂等键；
- invoke、stream、cancel、deadline、backpressure；
- 组件实例的最小生命周期与健康状态；
- 协议/版本/profile 协商；
- 审计、查询和 conformance 入口。

### 基底不拥有

- Agent、Prompt、Message、Turn、Memory；
- World、Entity、Scene、Quest、规则、经济或模拟时间；
- Home、Play、Forge、Assistant、Editor；
- Project 安装架、workspace、Docker、target、exec、port、proxy；
- provider、模型目录、计费策略；
- package registry、marketplace 或具体 secret store 产品。

这些概念可以被平台发行版使用，但必须位于协议、shell profile、host control plane 或体验层。

## 工件与内容寻址

基底不枚举所有可能的工件类型。它只提供可验证的通用描述符；常见类型由协议 profile 定义。

非规范性最小形状：

```text
ArtifactDescriptor
├── artifact_type_uri
├── media_type
├── digest
├── size_bytes
├── references[]
└── annotations
```

规则：

- `digest` 是身份，不是装饰字段；读取后必须重新验证。
- 未知 `artifact_type_uri` 仍可被复制、存储和导出；不认识语义不等于拒绝数据。
- 宿主绝对路径、进程 ID 和临时 URL 不能成为可移植身份。
- Package、component、protocol descriptor、content、composition、receipt 和 world bundle 都可以使用同一引用机制，但基底不把它们硬编码成封闭枚举。

该形状借鉴 OCI Content Descriptor 的 `mediaType + digest + size` 思路；Plurora 不要求采用 OCI manifest，也不把容器语义带进基底。

## Journal、因果与 Head

顺序日志和因果图解决不同问题，两者都保留：

- journal sequence 提供单个 scope 内便宜、确定、可分页的操作顺序；
- causal references 表达跨 scope、跨分支和跨 effect 的依赖；
- head 是一组对状态、历史、composition、policy 和 provenance 的内容引用；
- merge 由拥有领域语义的协议执行，基底不假装所有状态都能通用合并。

`WorldHead` 是 World 协议定义的 head profile，不是基底类型。其他协议可以定义 document head、workspace head 或 simulation head，而无需修改基底。

大对象、媒体、快照和模型输出进入内容寻址 object store；journal 与 receipt 只保存引用及必要的审计摘要。

## Effect Receipt

Receipt 是已经发生之效果的证据，不是计划或日志文本。

非规范性最小形状：

```text
EffectReceipt
├── receipt_type_uri
├── principal
├── component_ref
├── protocol_profiles[]
├── input_refs[]
├── output_refs[]
├── external_effect_refs[]
├── policy_decision_ref
├── approval_ref
├── cost / latency / status
├── trace_id
├── parent_receipts[]
└── replay_mode
```

Receipt 必须区分计划值与实际值，区分被拒绝、取消、超时、部分完成和成功。Raw secret、完整 credential、默认用户内容和未经需要的完整 prompt 不得进入 receipt。

Receipt 的 envelope、statement 和具体 predicate 可以分层演进；这一点与 in-toto attestation 将 subject、predicate type、认证 envelope 分离的做法相似，但 Plurora receipt 面向运行时效果，而非只面向软件供应链。

## Protocol Commons：协议公地

协议是共享语义与行为合同，不是某个实现包的 API 文档。

每个协议至少包含：

- 稳定、带命名空间的 protocol ID 与 major version；
- schema、WIT world 或等价类型描述；
- 字段语义、单位、坐标系、时钟和一致性假设；
- 生命周期与状态机；
- 错误与取消语义；
- 所需 authority、effect 和隐私边界；
- 测试向量与行为 conformance；
- compatibility profile；
- 迁移、adapter 和弃用说明；
- 已通过 conformance 的独立实现列表。

协议可以相互竞争和分叉。第一方维护的协议不获得内核路由优先级。外部协议如 MCP、A2A 或引擎协议可以通过 adapter 成为协议公地成员，不要求重新发明 Plurora 专用版本。

## Component 与执行信任

建议的 trust class：

| Trust class | 承诺 |
|---|---|
| `sandboxed_component` | 可移植、显式 imports、资源受限；WASM Component 是首选候选 |
| `isolated_process` | 进程故障隔离；OS 级网络/文件系统强制需宿主另行证明 |
| `remote_boundary` | 远程身份、网络故障、租户和服务策略显式化 |
| `trusted_native` | 宿主级信任与性能逃生口，不面向不可信动态代码 |
| `static_resource` | 不执行代码，仅提供内容或 surface bundle |
| `foreign_capsule` | 可以托管，但不承诺协议合规、可组合或可移植 |

调用协议可以一致，但 conformance 报告必须分别声明：类型兼容、权限强制、隔离、资源限制、可重放性和供应链证明。

`contract: none` 对应 `foreign_capsule`。它可以存在于产品中，但不能被描述为与 conforming component 具有相同生态保证。

## Change workflow

基底不拥有「assistant 提案」这一产品语义。通用改变链为：

```text
Intent
→ Plan / ChangeSet
→ PolicyDecision
→ Commit
→ EffectReceipt
```

- Intent 表达目标，不授予权力。
- ChangeSet 描述预期读取、写入、前置条件和预计效果。
- PolicyDecision 决定允许、拒绝、预算、审批或分支要求。
- Commit 原子检查前置条件并产生新 head。
- EffectReceipt 记录实际完成的效果。

现有 Proposal 可以成为该协议的一个 profile；其 UI 可以继续叫「提案」，但不再是宪法基底本体。

## Shell Profile

基底只保护资源、调用桥和权限，不硬编码 Home、Play、Forge 或 Assistant。

Shell profile 定义宿主如何解释交互资源，例如 view、action、editor、presence、stream、layout hint 和 input intent。`experience_entry`、`home_card`、`forge_panel` 等现有 slot 迁移为 `plurora.shell.default/v1` profile 的词汇。

Shell 可以是 Web、desktop、VR、IDE 或 headless 服务。更换 shell 不得改变世界历史、对象身份、协议状态或 effect receipt。

## Experience 与 World

Experience 是声明式组合，World 是运行后持续存在的上层实体。它们属于协议公地与用户数据，不属于 substrate。

建议的 World profile 可引用：

```text
WorldHead
├── state_root
├── history_root
├── composition_lock
├── protocol_profiles
├── policy_root
└── provenance_root
```

World bundle 必须能在不执行原组件的情况下被读取和审计。升级组件产生新的 composition lock；重新执行非确定性步骤产生新分支；删除组件不能使历史变得不可读。

## 协议成熟度

```text
Experimental
→ Candidate
→ Stable
→ Deprecated
→ Versioned Reader
```

- **Experimental：** 可快速破坏；不得获得长期兼容承诺。
- **Candidate：** 语义、错误、测试向量和迁移草案已存在；至少有两个不同消费者。
- **Stable：** 通过反僵化规则、行为 conformance 和独立实现要求。
- **Deprecated：** 仍在支持窗口内，但有明确替代者和迁移路径。
- **Versioned Reader：** 只读取或转换较早 contract/data boundary，不接收新的行为能力。

版本协商必须显式；客户端不得在回退时静默丢失所需能力。

## 反僵化规则

一个新概念只有同时满足以下条件，才能进入 Stable substrate：

1. 至少被三个互不相似的协议或体验需要；
2. 无法在 Protocol Commons、Host Control Plane 或 Shell Profile 中可靠实现；
3. 不依赖当前 UI、模型、题材、游戏类型或部署方式；
4. 至少有两个独立实现通过行为 conformance；
5. 有明确的版本协商、弃用和迁移路径。

未满足条件的概念保留在 Experimental/Candidate 协议或宿主层。使用频率高、由第一方维护或当前实现方便，都不是进入 substrate 的充分理由。

## 与当前 Contract V1 的关系

本文不要求删除现有实现。当前方法可以继续服务现有 Web、CLI、package 和 conformance，但其长期所有权要按
[`CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.md) 重新分类。

在候选分层被显式采纳为稳定宪法前：

- 精确 owner-based Contract V1 仍是现行公开边界，但不自动等同于永久宪法；
- v1 compatible evolution 遵守 additive rule，breaking behavior 必须进入新的可协商 contract/profile/version boundary；
- 新机制优先进入明确 owner 的 Experimental namespace；
- 较早数据通过显式 versioned reader 与 migration tooling 保持可读，而不是依赖隐藏 dispatch alias；
- 未来 v2 必须保留原始 v1 envelope 与 unknown field，以支持无损转移。

当前代码已经具备 owner-based identity、精确 Contract Registry、ObjectStore、EffectReceipt、Change primitive、Protocol Commons 与 World Bundle。它们为候选分层提供可运行基础，但不自动把本文升级为 Stable；长期归属仍以 [`CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.md) 为审查依据。

## 稳定采纳条件

候选基底在进入 Stable 前，必须满足以下兼容、可移植与独立实现条件：

1. 同一个 World Bundle 能导入第二个独立 host。
2. 替换模型 provider 不需要迁移世界数据。
3. 替换 WASM、process、remote 或 native 实现时，协议行为保持一致。
4. 两个独立作者只凭协议和 conformance 即可组合组件。
5. Agent 生成的组件可以被限权、评估、撤销和晋升。
6. 删除组件后，历史仍可读取、验证和审计。
7. 协议 major 升级后，旧世界可通过 adapter 运行。
8. 模型服务离线时，历史仍可确定性重放。
9. Web shell 被 desktop、VR 或 headless shell 替换时，世界数据不变。

这些条件约束稳定性声明和兼容承诺，不决定 Plurora 应该为了它们建设哪些产品功能。

## 非规范性参考

- [OCI Content Descriptors](https://github.com/opencontainers/image-spec/blob/main/descriptor.md) — 内容类型、摘要、大小和可验证引用。
- [in-toto Attestation Framework](https://github.com/in-toto/attestation/tree/main/spec) — subject、predicate、statement 与认证 envelope 分层。
- [OpenTelemetry Semantic Conventions](https://opentelemetry.io/docs/specs/semconv/) — 类型之外的共享语义与成熟度。
- [WebAssembly Component Model](https://component-model.bytecodealliance.org/) — 可组合组件与 WIT 合同。
- [A2A Protocol](https://a2a-protocol.org/latest/specification/) 与 [MCP](https://modelcontextprotocol.io/specification/) — 外部协议、版本协商与 adapter 边界。
