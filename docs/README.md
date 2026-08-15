# Plurora 文档

> [English](./README.en.md) · [中文](./README.md)

这里按试用、原则、架构、产品、协议和指南组织。每篇主要文档都有英文与简体中文版本；写作规范见 [`STYLE.md`](STYLE.md)。

**只想跑起来：** [`guides/GETTING_STARTED.md`](guides/GETTING_STARTED.md) 或 [`../BUILDING.md`](../BUILDING.md)。

## 两条阅读轨道

### 试用轨（约 15 分钟）

1. [`guides/GETTING_STARTED.md`](guides/GETTING_STARTED.md) — 启动 Host 与 Web Shell，检查示例 Work。
2. 打开 Library，或继续读 [`guides/MODULAR_SIMULATION.md`](guides/MODULAR_SIMULATION.md)。
3. 若要自己写包，再读 [`guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](guides/PACKAGE_AUTHORING_WALKTHROUGH.md)。

### 理解轨（深度）

1. [`CHARTER.md`](CHARTER.md) — 平台为什么存在。
2. [`architecture/ARCHITECTURE.md`](architecture/ARCHITECTURE.md) — 分层与所有权；这是唯一持有完整分层图的文档。
3. [`product/PLATFORM_PRODUCT_MODEL.md`](product/PLATFORM_PRODUCT_MODEL.md) — 官方发行版的产品责任。
4. [`protocol/PUBLIC_PROTOCOL.md`](protocol/PUBLIC_PROTOCOL.md) → [`spec/PUBLIC_CONTRACT.md`](spec/PUBLIC_CONTRACT.md) — 公开传输与方法语义。

实现对账和精确计数见 [`ALPHA_STATUS.md`](ALPHA_STATUS.md)，那是面向贡献者的快照，不是对外首页。

## 原则、产品与现状

- [`CHARTER.md`](CHARTER.md) — 平台身份、五个长期目标与不可妥协原则
- [`architecture/VISION.md`](architecture/VISION.md) — 长期形态与技术方向（分层图以 ARCHITECTURE 为准）
- [`product/PLATFORM_PRODUCT_MODEL.md`](product/PLATFORM_PRODUCT_MODEL.md) — 官方发行版的用户生命周期与产品边界
- [`product/PLAY_CREATION_MODEL.md`](product/PLAY_CREATION_MODEL.md) — 可选的游创 Profile，不是平台唯一形态
- [`ALPHA_STATUS.md`](ALPHA_STATUS.md) — 当前已实现、partial 与 deferred 的事实快照
- [`roadmap/NEXT_STEPS.md`](roadmap/NEXT_STEPS.md) — 建设方向，不是时间表承诺
- [`STYLE.md`](STYLE.md) — 文档事实层级、双语同步与写作红线
- [`LICENSING.md`](LICENSING.md) — `AGPL-3.0-only` 第一方范围与第三方边界
- [`../CONTRIBUTING.md`](../CONTRIBUTING.md) — 如何提交可审查的改动
- [`../BUILDING.md`](../BUILDING.md) — Rust、Web、Desktop 与 release 构建

## 架构与公开合同

- [`architecture/`](architecture/README.md) — 分层架构、基底、组件与 Host 控制平面
- [`protocol/`](protocol/README.md) — 传输与调用信封
- [`spec/`](spec/README.md) — Contract V1 schema、Registry 与兼容合同

候选长期宪法 [`architecture/CONSTITUTION_V2.md`](architecture/CONSTITUTION_V2.md) 尚未替代 v1，不在默认阅读路径。需要逐项归属时再读 [`spec/CONTRACT_LAYERING_MATRIX.md`](spec/CONTRACT_LAYERING_MATRIX.md)。

## 创作、安装与运行

- [`guides/`](guides/README.md) — 按起步 / 进阶 / 模型 / Agent / 实验室分组
- [`guides/GETTING_STARTED.md`](guides/GETTING_STARTED.md) — 从源码看到界面
- [`guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](guides/PACKAGE_AUTHORING_WALKTHROUGH.md) — 第一个 Package / Component
- [`guides/INSTALLATION_MODEL.md`](guides/INSTALLATION_MODEL.md) — Work / Workspace / Installation 与安装操作
- [`guides/RUN_LIBRARY.md`](guides/RUN_LIBRARY.md) — 启动和停止 Run
- [`guides/SECRET_MANAGEMENT.md`](guides/SECRET_MANAGEMENT.md) — `secret_ref` 与 API key
- [`guides/MODULAR_SIMULATION.md`](guides/MODULAR_SIMULATION.md) — 可继续创作的示例 Work kit

## 性能、状态与外部接入

- [`performance/`](performance/README.md) — 性能基线与代码健康
- [`roadmap/`](roadmap/README.md) — 仍影响取舍的建设方向
- [`tavern/`](tavern/README.md) — 与独立项目 YdlTavern 的边界

## 最短读路径

| 你想 | 先读 |
|---|---|
| 五分钟跑起来 | [`guides/GETTING_STARTED.md`](guides/GETTING_STARTED.md) |
| 理解平台目标 | [`CHARTER.md`](CHARTER.md) → [`architecture/ARCHITECTURE.md`](architecture/ARCHITECTURE.md) |
| 理解官方产品 | [`product/PLATFORM_PRODUCT_MODEL.md`](product/PLATFORM_PRODUCT_MODEL.md) → [`design/PLATFORM_UI_DESIGN.md`](design/PLATFORM_UI_DESIGN.md) |
| 接入公开协议 | [`protocol/PUBLIC_PROTOCOL.md`](protocol/PUBLIC_PROTOCOL.md) → [`spec/PUBLIC_CONTRACT.md`](spec/PUBLIC_CONTRACT.md) |
| 写第一个 Package | [`guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](guides/PACKAGE_AUTHORING_WALKTHROUGH.md) |
| 打包 Work / 管理 Installation | [`guides/INSTALLATION_MODEL.md`](guides/INSTALLATION_MODEL.md) |
| 管理 API key | [`guides/SECRET_MANAGEMENT.md`](guides/SECRET_MANAGEMENT.md) |
| 构建 Web / Desktop / Release | [`../BUILDING.md`](../BUILDING.md) |
| 看当前实现事实 | [`ALPHA_STATUS.md`](ALPHA_STATUS.md) |
| 写文档 | [`STYLE.md`](STYLE.md) |
