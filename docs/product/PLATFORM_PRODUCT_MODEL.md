# 平台产品模型

> [English](./PLATFORM_PRODUCT_MODEL.en.md) · [中文](./PLATFORM_PRODUCT_MODEL.md)

本文定义 Plurora 官方发行版应该怎样成为一款完整、好用的产品，同时不把自己的产品选择提升为整个平台的强制本体。

## 产品责任

平台底层扩大可能性，官方发行版则必须替用户作出一组清楚、连贯、可改变的默认选择。它不是只有开发者才能使用的协议调试器，也不是为了展示底座能力而存在的参考壳。

官方发行版要负责：

- 让第一次安装、启动和本地使用足够直接；
- 管理内容、组件、项目、数据、身份、权限与 Host；
- 提供可靠的日常运行、诊断、更新、备份、恢复和迁移体验；
- 让创作者能从模板开始，逐渐进入组合、调试、扩展和发布；
- 给高级用户暴露真实状态和控制，但不强迫所有人先理解平台内部结构；
- 全部使用公开边界，使第三方发行版能够重新组织同一平台能力。

## 四类主要使用者

### 使用者

他们希望发现、安装并使用应用、工具和体验，而不是先学习能力包、协议或 Host。默认路径必须安全、清楚、可恢复。

### 创作者

他们从修改现有内容、组合组件、使用 AI 辅助开始，可以逐渐进入自定义协议、数据、Surface 和执行方式。创作不应被人为分成一个封闭的“玩家模式”和另一个完全不同的“开发者模式”。

### 组件与产品开发者

他们需要稳定 SDK、清楚的合同、可调试运行环境、兼容与迁移工具、可发布工件，以及不依赖官方内部代码的接入路径。

### Host 运营者

他们需要安装、更新、身份、权限、资源、网络、日志、健康、备份、恢复和多设备管理，并能理解平台实际执行了什么。

一个人可以同时扮演多种角色，界面通过渐进式展开适应深度，而不是为每个角色创建互相割裂的平台。

## 完整的用户生命周期

官方发行版至少覆盖：

```text
获得 Plurora
→ 本地启动或连接 Host
→ 发现 / 导入内容
→ 查看来源、权限和资源需求
→ 安装
→ 使用
→ 更新
→ 诊断与恢复
→ 备份 / 导出 / 迁移
→ 停止 / 归档 / 删除
```

每个步骤都应有明确状态、取消方式、失败说明和后续动作。只完成“安装成功”的 happy path 不算完整。

## 完整的创作者生命周期

```text
创建或导入
→ 本地运行
→ 观察状态与事件
→ 修改内容、Assembly 或组件
→ 使用人类或 AI 工具辅助
→ 调试与测试
→ 打包
→ 分享或 Realization
→ 发布更新与迁移
```

创作者工具可以强观点，但不获得私有内核能力。AI 辅助和人工编辑使用同样清楚的身份、作用域、变更和效果边界。

## 官方 Shell 的当前组织方式

当前 Web/Desktop 发行版使用 Home、Settings、Installation frame、Workbench 和可贡献 Surface 来组织体验。这是一套正在演化的官方产品结构，不是所有 Plurora 客户端的强制结构。

- **Home / Library：** 发现、安装、启动和继续使用；
- **Settings / Control：** Host、身份、权限、存储、连接和包管理；
- **Installation / App frame：** 承载某个安装实例自己的主要界面；
- **Workbench / Console：** 诊断、创作、变更、Realization 和恢复等高级能力；
- **Contextual assistance：** 在明确作用域内提供解释、建议与受控操作。

第三方发行版可以没有 Home、没有 Library，或采用完全不同的根对象和导航。它只要遵守采用的公开合同与权限边界即可。

## Powerbox UX

Home/Library 负责 Work 与 Installation 的发现，Installation frame 提供 Powerbox chooser。Chooser 必须先显示 binding phase，再显示 exact Exposure、audience、expiry，以及 consumer/provider 两端 PortContract（protocol、interface、version、profiles、interaction、effects、transport、multiplicity）。Provider Work/Installation source、Component trust/claim/boundaries/evidence、artifact/behavior digest 与 stale 状态分栏展示。

第一方与第三方 provider 使用相同披露规则，候选按稳定顺序展示，不按 publisher 选择。0 个或多个候选都要求用户或显式 policy 选择；preference 只能作为排序 hint。UI 不猜 unknown，不隐式 apply/rebind，也不显示 runtime handle、credential 或 private intent。可见候选最多 256，超限显示结构化诊断而不是静默截断。

PWA/mobile 与 Desktop 复用同一 Host API；Host/Installation cache 隔离，关闭 tab、iframe 或 PWA 连接不会停止 Run，也不会撤销 Exposure/Binding。Surface 只有公开 allowlist bridge，没有 Powerbox private bridge 或第一方旁路。

## Realization UX

Installation frame 只在 Work 声明 OperationalIntent 时显示 Realization workbench。Plan 与 Apply 必须是两个可区分步骤：Plan 显示 exact Target、actions、preconditions、required authority 和每项 risk；用户逐项确认后才允许 Apply。Status、history、Stop 与 Reconcile 都显示真实 `RealizationRevision`，不把 UI 临时状态伪装为 Host truth。

0 个可满足 Target 显示结构化 gap；多个 Target 要求显式选择。关闭 frame 不会 stop，Run start 不会隐式 apply，rollback 不读取 live workspace。Local 与 Agent Target、第一方与第三方 Work 使用同一 plan/approval/receipt/authority 边界。

## Work 与 Installation 的产品边界

Work 是便携逻辑作品，Installation 是某台 Host 对 exact Work/Lock 的采用记录，Run 与 Realization 是彼此独立的运行/机器资源生命周期。它们都不是所有 Plurora 数据与交互的永久根对象。

其他产品可以围绕 World、Document、Service、Workspace、Collection、Simulation 或自己的协议对象组织。通用基底不能要求这些对象伪装成 Installation；Host 也不能因为管理 Installation 就拥有其内容语义。

## 好用的原则

### 简单路径真正简单

本地默认值、自动发现、合理权限建议和清楚的错误恢复，应减少用户必须理解的概念。高级配置保持可见但不抢占第一次使用。

### 渐进式展开

从使用、轻度修改、组合，到编写组件和协议，用户在同一产品中逐渐深入。深度来自可展开能力，而不是跳进另一套隐藏系统。

### 权限提示可理解

权限界面解释将发生的操作、目标资源、持续时间、风险和撤销方式，不只显示内部 scope 名称。

### 状态与恢复优先

长期任务、远程连接、安装、更新、Realization 和迁移都必须可观察、可取消或可恢复。失败后不要求用户手工编辑数据库。

### 数据管理是一等体验

存储占用、备份、导出、导入、保留、删除和迁移应当像安装与运行一样受到产品设计关注。

### 无障碍、国际化和性能不是收尾工作

键盘、屏幕阅读器、低性能设备、移动端、不同语言和慢网络从设计开始就属于支持范围。

## 官方观点与平台开放性的平衡

官方发行版可以选择默认协议、组件、布局和工作流，也应该积极打磨这些选择。边界是：

- 默认选择可见、可替换；
- 数据不因替换客户端而失效；
- 官方组件只使用公开接口；
- 第三方可以提供同类或完全不同的产品；
- 官方产品需求可以推动平台改进，但不能自动成为基底职责；
- 当某个能力只服务官方界面时，它留在发行版或对应 Profile。

## 产品完整性的衡量

产品是否更好，不以功能数量衡量，而看：

- 用户能否完成完整生命周期；
- 新用户是否能在合理默认值下成功；
- 高级用户是否能理解并控制真实状态；
- 失败是否能恢复；
- 创作者是否能从简单修改走到独立发布；
- 更换组件、客户端或 Host 时是否保留工作；
- 新功能是否保持公开边界和第三方空间。

具体当前状态见 [`../ALPHA_STATUS.md`](../ALPHA_STATUS.md)：Exposure、Binding、Powerbox 与 Managed Realization 均已实现；建设方向见 [`../roadmap/NEXT_STEPS.md`](../roadmap/NEXT_STEPS.md)。
