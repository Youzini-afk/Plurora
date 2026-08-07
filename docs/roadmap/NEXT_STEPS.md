# 建设方向

> [English](./NEXT_STEPS.en.md) · [中文](./NEXT_STEPS.md)

Plurora 的下一步不是围绕某个单一产品、示例或工作流扩张，也不是为了完成一张证明题而增加功能。建设方向来自章程中的五个长期目标：开放、多样、先进、长久、好用。

这些方向并行推进，不用阶段编号制造虚假的线性顺序。每一项具体工作都应落在清楚的用户生命周期、架构层和长期责任中。

## 当前最重要的工作

### 让官方发行版成为完整、好用的产品

当前 Web、Desktop、PWA 与 CLI 已经拥有大量能力，但日常体验仍容易显得像平台控制面而不是成熟产品。优先补齐完整生命周期，而不是继续堆独立面板：

- 第一次启动、本地 managed Host、远程 Host 连接和恢复；
- 发现、导入、安装、授权、运行、停止、更新、卸载；
- 状态、进度、取消、失败原因和下一步操作；
- 存储占用、备份、导出、迁移、归档和删除；
- 权限请求的人类可读解释、期限、目标资源和撤销入口；
- 移动端、键盘、无障碍、国际化、慢网络和低性能设备；
- 简单模式与高级控制之间的渐进式展开。

官方发行版继续使用公开合同，不建立 Desktop 私有能力或官方 Package 捷径。产品模型见 [`../product/PLATFORM_PRODUCT_MODEL.md`](../product/PLATFORM_PRODUCT_MODEL.md)。

### 把创作者体验做成一条连续路径

创作者不应先理解整个内核才能开始，也不应在需要深入时被困在不可扩展的低代码界面。需要连贯连接：

```text
模板 / 导入
→ 本地运行与热更新
→ 观察事件、调用、对象和状态
→ 修改内容、composition、组件或协议
→ 人类 / AI 辅助
→ 调试与测试
→ 打包、分享、部署
→ 更新与迁移
```

近期重点包括：

- 清楚、少样板的 Package / Component / Surface 模板；
- TypeScript、Rust 与未来 WASM SDK 的一致体验；
- 本地开发模式、热更新、source map、日志和错误定位；
- 可视化查看 authority、protocol binding、effect 和 artifact provenance；
- composition 与依赖冲突的可理解诊断；
- AI 工具通过普通能力、明确作用域和可审阅变更工作，而不是获得通用 root shell；
- 从本地作品到可分享工件的稳定打包路径。

游创只是可选 Profile；文档工具、服务、IDE 和无头系统应能采用不同创作流。

### 收敛 Contract V1 的长期所有权

Contract V1 继续作为支持中的公开合同，但不再无差别扩大 `kernel.v1.*`。建设重点是：

- 让 substrate、Host、Protocol Commons 和 Shell Profile 的 owner 清楚可见；
- 为新能力选择明确 namespace、version 和 maturity；
- 继续维护 Contract Registry、显式协商、canonical method 与 legacy adapter；
- 保证旧客户端、旧数据和未知字段可读取与迁移；
- 将 Surface slot、Project、target、deployment 等正确标记为 Profile / Host，而不是永久内核本体；
- 只在有真实公共语义时建立 Protocol，不把某个 Package 私有 JSON 过早冻结成平台标准。

逐项归属见 [`../spec/CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.md)，候选宪法见 [`../architecture/CONSTITUTION_V2.md`](../architecture/CONSTITUTION_V2.md)。

### 完善组件执行与信任模型

当前 rust in-process 与 subprocess 已经可运行；WASM 和通用 remote component 仍有明显空间。推进先进执行能力时，以实际收益为标准：

- WASM Component / WIT 带来可移植、显式 imports 和资源限制；
- subprocess 需要更清楚的 OS 级文件系统、网络和资源强制声明；
- remote component 需要身份、租户、deadline、重连、幂等和 effect receipt；
- trusted native 保留为高性能逃生口，但不用于不可信动态代码；
- static resource 与 executable component 分离；
- UI 和 conformance 诚实呈现每种 trust class 的实际保证。

WASM、remote 或新 transport 不因为“技术更新”自动优先；只有它们能增加可移植性、安全性、性能或生态语言选择时才推进。

### 把用户数据、内容和工件当作长期资产

ObjectStore、ArtifactDescriptor、World Bundle、deployment artifact 和 effect receipt 已经出现，但还需要统一长期数据治理：

- 用户数据、可重建 cache、可执行工件和临时诊断明确分类；
- 内容摘要、引用、provenance、可达性、保留和垃圾回收；
- 加密、备份、导出、导入、迁移和删除；
- 未知 artifact type 与未知字段的保真转移；
- Component 更新不能静默覆盖用户内容；
- 历史回放使用已记录结果，重新执行创建新因果分支；
- 多 Host 复制和冲突策略由采用的协议明确，而不是由路径碰巧决定。

可移植性不是发布前的收尾项，而是平台长期身份的一部分。

### 建设可竞争的协议公地

平台需要比“大家都传 JSON”更强的互操作，同时避免把官方观点冻结成唯一标准。协议工作应包括：

- protocol descriptor、profile、版本与成熟度；
- 字段语义、生命周期、错误、取消、effect 和 privacy；
- adapter、迁移和弃用窗口；
- 实现声明和行为检查；
- 多个实现或多个协议共存时的显式选择；
- 与 MCP、A2A、OCI、WASI 等外部生态通过 adapter 互操作，而不是无条件重新发明。

候选领域包括 Surface、Change、Workspace、Inference、Agent、Memory、World、Document、Sharing 和 Evaluation。每个领域都可以有竞争方案，不因“官方”自动进入 Stable。

### 强化 local-first、远程与多 Host

本地使用应始终是一等路径，同时允许用户把能力扩展到远程设备和服务：

- managed local Host 无需云账户即可工作；
- remote Host 使用明确 HTTPS 身份、pairing、grant 和资源 selector；
- 同一客户端严格隔离不同 Host 的凭据、缓存和 Project / target 偏好；
- 设备撤销、祖先授权撤销、离线、重连和过期处理清楚；
- 大工件传输可恢复、可校验、可限额；
- 多 Host 内容和状态迁移不依赖本机绝对路径；
- 远程能力不退化成任意 shell 或无限文件系统访问。

### 持续提高可靠性、性能与可维护性

质量保障服务于产品与平台建设：

- 测试覆盖权限边界、迁移、恢复、取消、并发和数据完整性；
- conformance 约束公开合同行为，而不是决定产品方向；
- 性能基线关注启动、交互、stream、对象传输、移动网络和资源占用；
- 故障注入覆盖进程退出、Host 重启、断网、撤权、部分 effect 和损坏工件；
- 文档、schema、SDK、代码和 CI 对实现事实保持一致；
- 删除过时兼容层和临时计划，避免复杂度永久累积。

当前实现快照见 [`../ALPHA_STATUS.md`](../ALPHA_STATUS.md)。

## 明确不是平台中心的东西

以下能力可以继续建设，但不能被写成 Plurora 唯一方向：

- Project workflow；
- 游创、Tavern、世界或聊天；
- agent 或模型推理；
- Docker 与部署；
- 官方 Web / Desktop Shell；
- 某个 protocol profile；
- 某个演示、fixture 或外部项目。

它们是平台上的产品、协议、Host 能力或使用场景。一个方向很重要，不等于它拥有其他方向。

## 暂不主动扩张

在没有清楚用户价值和所属层之前，不主动增加：

- 以功能数量为目的的新部署后端；
- 内核内置聊天、agent、memory、world 或 Project UI 语义；
- 官方 Package 私有 API、名称特权或隐藏路由；
- 任意远程 shell、无限 Host 文件系统或长期 root credential；
- 没有迁移路径的 Stable schema；
- 只因为流行而引入、却没有能力收益的新技术；
- 在核心生命周期仍不完整时过早建设市场、计费和生态经济。

这不是永久禁止；当它们有明确价值、边界和长期维护方式时，可以重新进入建设计划。

## 选择具体工作的判断方式

一项工作进入当前建设队列前，应能清楚回答：

1. 它让哪类使用者或创作者获得了什么实际能力？
2. 它使一个生命周期更完整、更可靠或更容易理解了吗？
3. 它属于基底、协议、组件、Host、发行版还是具体产品？
4. 它是否保持数据所有权、公开边界和替换空间？
5. 它采用的技术是否带来可衡量的安全、性能、可移植性或维护收益？
6. 它的错误、取消、恢复、迁移和删除路径是什么？
7. 它是否会把当前官方选择反向冻结成整个平台的要求？

测试、fixture 和 conformance 在设计之后用来保证这些目标不退化，而不是代替目标本身。

## 文档与状态

- 平台身份和原则：[`../CHARTER.md`](../CHARTER.md)
- 长期整体形态：[`../architecture/VISION.md`](../architecture/VISION.md)
- 分层架构：[`../architecture/ARCHITECTURE.md`](../architecture/ARCHITECTURE.md)
- 官方产品责任：[`../product/PLATFORM_PRODUCT_MODEL.md`](../product/PLATFORM_PRODUCT_MODEL.md)
- 当前实现快照：[`../ALPHA_STATUS.md`](../ALPHA_STATUS.md)

路线图描述建设方向，不承诺所有条目同时进行，也不把候选项写成已实现事实。
