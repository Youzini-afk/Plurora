# Protocol Commons 注册表

> [English](./PROTOCOL_COMMONS.en.md) · [中文](./PROTOCOL_COMMONS.md)

状态：Experimental，描述符 Schema 版本 1。

Protocol Commons 是共享语义的注册表。只有 JSON 形状并不构成协议；每个注册协议还必须声明生命周期、错误与取消模型、权限边界、行为向量、兼容 Profile、迁移以及实现。注册项不获得路由优先级，官方实现与第三方实现使用完全相同的向量集合。

## 描述符

[`protocol-descriptor.schema.json`](v1/schemas/protocol-descriptor.schema.json) 发布 `ProtocolDescriptor`（`urn:yggdrasil:protocol-descriptor:v1`）。稳定字段包括：

- `protocol_id`、`version`、`maturity`；
- JSON Schema 与 WIT world 引用；
- 语义、生命周期和错误模型文档引用；
- 显式权限要求；
- 由协议拥有的一致性向量 ID；
- 兼容 Profile；
- 迁移与适配器；
- 实现声明及每个声明实际使用的向量集合。

仓库内文档引用可以暂时没有 digest。若 Package 或 World Bundle 要提出跨主机完整性声明，必须先把这些引用物化为内容寻址 Artifact。

`host.info` 会公开 `protocol_commons_registry_version` 和完整描述符注册表。当前 registry 有意只注册：

| 协议 | 版本 | Profile | 状态 |
| --- | --- | --- | --- |
| `ygg.change` | `1.0.0` | `ygg.change/default/v1` | Experimental |
| `ygg.shell.default` | `1.0.0` | `ygg.shell.default/v1` | Experimental |
| `ygg.world.bundle` | `1.0.0` | `ygg.world.bundle/experimental/v1` | Experimental |

Projection 仍保留 Experimental canonical namespace，但不进入首批 Protocol Commons 描述符。至少要由两个实质不同的 Experience 验证后，才能声称它具有共享语义。

## 协商

`ContractSelection.protocols[]` 选择协议 ID、版本和可选兼容 Profile。协商发生在方法解析和 Handler 执行之前。

- 精确支持的版本/Profile 会产生 `NegotiatedProtocol`；
- 已声明的旧协议/版本使用具名适配器，并在协商结果中报告该适配器；
- 不支持的 major 返回 `kernel/v1/error/unsupported_protocol`，`reason=protocol_major_mismatch`，并给出请求/支持 major 与可用适配器；
- 未知协议和 Profile 显式失败，不会因为 Schema 能解析就降级到更弱语义。

首个显式适配器是 `kernel.v1.proposal@1.0.0 → ygg.change@1.0.0`，适配器 ID 为 `change.proposal.v1`。

## 一致性职责

协议一致性与实现/Package 一致性使用不同报告：

- `ProtocolConformanceReport` 标识协议、版本、Profile 和协议向量结果；
- `ImplementationConformanceReport` 额外标识具体实现与提供者，同时沿用同一批向量 ID；
- `PackageConformanceReport` 继续检查分发封装、声明、握手、权限、流和 Handle 生命周期。

若实现声明缺少必需向量、伪造描述符之外的向量、引用未知 Profile 或声明了不同协议版本，注册表会拒绝它。Change 描述符包含官方 Runtime 实现和一个仅测试使用的第三方参考声明，两者绑定同一组四个必需向量。`test_only` 防止测试夹具被宣传为可移植的生产实现。

三类报告可独立执行：

```text
ygg conformance protocol --protocol ygg.change --json
ygg conformance protocol --protocol ygg.change --implementation ygg.runtime.change-proposal --json
ygg conformance package --path <package>
```

## Change 协议

Change 协议引用增量的 Intent、ChangeSet、PolicyDecision、Commit 和 EffectReceipt Schema。生命周期、错误、权限、Proposal 适配器及行为证据见 [`CHANGE_WORKFLOW.md`](CHANGE_WORKFLOW.md)。

## Shell Default Profile

`ygg.shell.default/v1` 拥有把结构化贡献和沙箱 Surface Bundle 映射到 Shell 的词汇。现有固定 `SurfaceSlot` 仅作为 `shell.surface-slot.v1` 接受的旧词汇，不是 substrate ontology。

该 Profile 要求：

- 通过 `shell.contribution.*` 进行公开发现；
- 元数据有界、owner 明确，并通过当前 Package Manifest 分发；
- Surface bridge allowlist 与 session scope 显式；
- 不隐式获得 kernel、文件系统、网络或宿主 UI 权限；
- 替换 Shell 不改变 journal history、object identity 或 receipt。

当前生命周期和 bridge 错误模型见 [`SURFACE_HOSTING.md`](../guides/SURFACE_HOSTING.md)。

## World Bundle Experimental Profile

`ygg.world.bundle/experimental/v1` 定义可移植性条件，但不会把 `World` 加入 substrate。描述符引用 EventEnvelope、ArtifactDescriptor、EffectReceipt，以及具体的 [`WORLD_BUNDLE.md`](WORLD_BUNDLE.md) archive/head/journal schema。

五个必需向量覆盖引用闭包、跨 Host 导入、离线回放、新分支重执行和 Shell 独立性。当前 `official/playable-creation-board` integration fixture 已覆盖这些向量，因此 `ygg.runtime.world-bundle` 注册了第一个 production implementation claim；该声明描述当前实现覆盖，不代表 World Bundle 是平台唯一内容 Profile。

## World Bundle 生命周期

必需流程是 `选择 head → 计算闭包 → 校验 → 导出 → 导入空 scope → 审计/回放 → 可选地在新分支重执行`。导入不会把宿主路径、进程 ID、URL 或 Package 本地 Runtime Handle 当作可移植身份。

## World Bundle 错误模型

以下情况必须显式失败：对象缺失、digest/size 不匹配、传递引用闭包不完整、协议 major 不兼容、必需 Profile 不受支持、原始 envelope 被修改、policy 引用无法解析，或历史回放试图执行外部 effect。未知 Artifact 类型应保留和复制，不得丢弃。
