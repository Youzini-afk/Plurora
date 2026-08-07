# Package、组件与能力契约

> [English](./CAPABILITY_PACKAGE.en.md) · [中文](./CAPABILITY_PACKAGE.md)

当前 Contract V1 使用一个 Package Manifest 同时描述分发、执行、能力、协议贡献、Surface 和权限。这个模型可以继续工作，但长期架构必须区分不同所有权：

- **Package / Package Envelope：** 获取、分发、安装和供应链信封；
- **Component：** 可激活、可调用的实现单元；
- **Protocol：** 多个实现共同遵守的语义与行为合同；
- **Content / Artifact：** 用户或产品数据、静态资源和不可变工件；
- **Adapter：** 把外部系统或旧合同接入公开协议的组件。

Package 可以携带以上多种内容，但 Package 本身不是所有平台语义的本体单位。

## 平等规则

官方 Package、第三方 Package 和不同执行形态使用相同的：

- descriptor / manifest schema；
- 安装与完整性检查；
- capability 与 protocol 注册；
- authority binding；
- 调用、stream、取消和 effect receipt；
- 诊断、迁移和 conformance 入口。

没有按 Package ID 开放的私有 API，也没有隐式“官方实现优先”。维护者或签名可以影响来源信任与策略，但不能自动获得运行权威。

## 当前 V1 Manifest

V1 Manifest 是当前公开兼容格式，主要形状如下：

```yaml
schema_version: 1
id: org/name
version: 0.1.0
display_name: Example
description: ...
license: AGPL-3.0-only

entry:
  kind: rust_inproc | subprocess | wasm | remote
  contract: v1 | none
  # kind-specific fields

provides:
  - id: org/name/capability
    version: 0.1.0
    input_schema: {}
    output_schema: {}
    streaming: false
    side_effects: []

consumes:
  - id: other-org/capability
    version: ^0.2

contributes:
  schemas: []
  hooks: []
  extension_points: []
  surfaces: []

permissions:
  network: { hosts: [] }
  filesystem: { paths: [] }
  events: { read: false, append: false }
  capabilities: { invoke: [] }

sandbox_policy:
  cpu_quota_ms_per_invoke: 5000
  memory_mb: 128
  wall_clock_ms: 30000
```

实际字段以 [`../spec/v1/schemas/manifest.schema.json`](../spec/v1/schemas/manifest.schema.json) 为准。未来拆分 Package Envelope、Component Descriptor、Protocol Descriptor 和 Content Root 时，v1 Manifest 通过生成或 legacy adapter 继续可读。

## Package Envelope

Package Envelope 负责：

- 来源和获取坐标；
- 版本、签名、许可证与维护者信息；
- manifest / tree / artifact digest；
- 携带的 component、protocol、content 和 Surface 引用；
- 平台与架构兼容要求；
- 安装、更新、迁移和回滚所需元数据。

Package 安装是 Host Control Plane 操作。安装成功不等于其中所有 Component 已被激活，也不等于获得了声明中的所有权限。

## Component

Component 是实际提供行为的实现单位。它至少拥有：

- 独立 identity 与 behavior / artifact digest；
- exports、imports 和采用的 protocol profile；
- trust class 与强制边界；
- resource limits；
- activation、health 和 deactivation 状态；
- 兼容与迁移声明。

一个 Package 可以包含多个 Component；更新一个 Component 不应自动要求迁移同一 Package 中所有内容。

## 执行形态与信任

统一 capability contract 不等于统一隔离保证：

| V1 entry / component form | 长期 trust class | 说明 |
|---|---|---|
| `rust_inproc` | `trusted_native` | 性能高，拥有 Host 进程级信任；崩溃和未拦截副作用可能影响 Host |
| `subprocess` | `isolated_process` | 进程故障隔离；文件系统和网络的 OS 级强制由 Host 策略决定 |
| `wasm` | `sandboxed_component` | 目标是显式 imports、资源限制和可移植执行；当前完整执行支持仍在建设 |
| `remote` | `remote_boundary` | 远程身份、网络故障、租户和服务策略必须显式；当前通用远程组件执行仍在建设 |
| static bundle/content | `static_resource` | 不执行代码，只提供可验证内容或 Surface |
| `contract: none` | `foreign_capsule` | Host 可以托管生命周期，但不承诺 v1 binding、组合或协议保证 |

不同形态可以实现同一协议，但 conformance 和 UI 必须诚实展示实际保证，不能把它们描述成只有打包格式不同。

## Capability 契约

Capability 由稳定 ID、版本、input/output schema、streaming 与 effect 要求描述。调用方可以按 capability、protocol profile 和版本约束选择实现。

路由遵守：

1. 当前 authority 是否允许调用；
2. 组件是否已激活且健康；
3. protocol / version / profile 是否兼容；
4. distribution 或调用方是否显式选择 provider；
5. 多个实现仍然歧义时，拒绝并要求选择。

不存在隐式官方优先级。

Capability 调用产生结构化 terminal state；涉及外部效果或非确定性时，产生或引用 EffectReceipt。大输入和输出应使用 ArtifactDescriptor，而不是无限扩张线路 envelope。

## Protocol contribution

Protocol 定义共享语义，Component 实现 Protocol。一个 Package 可以携带 protocol descriptor，但维护该 Package 不代表拥有内核特权。

协议贡献至少应说明：

- protocol ID 与版本；
- schema 或等价类型合同；
- 字段语义和生命周期；
- 错误、取消和 effect 语义；
- authority 与隐私要求；
- compatibility profile 与迁移；
- 行为检查和实现声明。

Extension point、projection、change workflow、agent、memory、world、surface 等共享语义应逐渐从 Package 私有约定进入明确 Protocol，而不是继续扩大单体 kernel namespace。

## Surface contribution

当前 v1 Package 可以贡献 Surface descriptor。Surface 的 bundle、capability allowlist、activation 和 permission requirements 来自 Manifest；具体 slot 由采用它的 Shell Profile 解释。

因此：

- Surface 可以由官方或第三方 Package 提供；
- Surface 默认没有隐式 kernel access；
- `experience_entry`、`forge_panel`、`assistant_action` 等是当前 Profile 的枚举，不是宪法基底类型；
- 第三方 Shell 可以定义或协商不同的 Surface Profile；
- 静态 Surface bundle 与执行 Component 可以独立版本和寻址。

## Authority 与声明

Manifest permissions 是 Package 请求的最大范围，不是实际 grant。Host 根据用户选择、principal、Project / target selector、策略和环境铸造实际 binding。

执行时必须做到：

- 未授予的 capability、event、network、filesystem 或 secret 操作被拒绝；
- 长任务在关键副作用前重新确认 grant；
- delegation、lease、quota 和 revoke 能够生效；
- raw secret 不进入 Manifest、日志、receipt 或公开状态；
- declared-vs-used audit 不依赖 Package 名称特权。

`entry.contract: "none"` 不把 Manifest 声明转换成平台 authority；它是一种明确降低互操作保证的自包含路径，而不是绕过 Host 安全边界的方式。

## 生命周期

需要区分两个生命周期。

### Package 生命周期（Host）

```text
discovered → resolved → downloaded → verified → installed
           → update available → migrated / rolled back → removed
```

### Component 生命周期（runtime / substrate）

```text
inactive → activating → ready → degraded → stopping → inactive
                         └──────────────→ failed
```

停止 Component 不等于卸载 Package；卸载 Package 也必须先处理依赖、运行实例、用户数据和回滚信息。

## Content 与用户数据

Content 不应因为与可执行代码同包发布，就失去独立身份。重要内容应：

- 使用内容摘要和开放 ArtifactDescriptor；
- 能在不知道原始 Package 实现的情况下复制和导出；
- 明确引用依赖、schema / protocol profile 和迁移；
- 区分用户拥有的数据、可重建 cache 和可执行工件；
- 不因更新 Component 而被静默覆盖。

## 分发与更新

Package registry、marketplace 和依赖解析服务属于 Host / distribution 生态，不属于宪法基底。不同 registry 可以竞争，离线文件和本地 source 仍是一等来源。

更新应锁定：

- 获取来源与不可变 commit / digest；
- Package Envelope；
- Component artifacts；
- Protocol profiles；
- Content roots；
- 用户已同意的权限变化与迁移计划。

自动更新不能绕过新的权限、数据迁移或信任边界。

## 版本与兼容

- Package version、Component version、Protocol version 和 Manifest `schema_version` 是不同维度；
- 破坏性 Component ABI 变化不应伪装成内容迁移；
- Protocol major 变化需要 compatibility / adapter / migration；
- 未知字段和未知 Artifact 应在可行时保留；
- v1 Manifest 与未来 descriptor 拆分通过兼容生成和 adapter 共存。

## 设计判断

新增内容时先判断它是什么：

- 为了获取和安装而组合工件 → Package Envelope；
- 实现行为并被调用 → Component；
- 多个实现共享的含义 → Protocol；
- 用户或产品拥有的可移植数据 → Content / Artifact；
- 现实机器操作 → Host；
- 某个 Shell 的交互观点 → Product Profile。

不要因为现有 v1 Manifest 能装下一个字段，就默认 Package 永久拥有这个概念。
