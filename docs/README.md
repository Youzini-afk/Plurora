# Plurora 文档

> [English](./README.en.md) · [中文](./README.md)

这里按长期原则、当前实现、架构层、产品、协议和创作指南组织文档。每篇主要文档都有英文与简体中文版本；写作规范见 [`STYLE.md`](STYLE.md)。

## 新人 1 / 2 / 3 路径

1. 读 [`CHARTER.md`](CHARTER.md) → [`architecture/VISION.md`](architecture/VISION.md)，理解平台为什么存在、长期追求什么。
2. 读 [`architecture/ARCHITECTURE.md`](architecture/ARCHITECTURE.md) → [`architecture/CONSTITUTIONAL_SUBSTRATE.md`](architecture/CONSTITUTIONAL_SUBSTRATE.md) → [`architecture/CAPABILITY_PACKAGE.md`](architecture/CAPABILITY_PACKAGE.md)，理解基底、协议、组件、Host、发行版和产品的边界。
3. 读 [`product/PLATFORM_PRODUCT_MODEL.md`](product/PLATFORM_PRODUCT_MODEL.md) 和 [`guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](guides/PACKAGE_AUTHORING_WALKTHROUGH.md)，分别理解官方产品责任并完成第一个 Package / Component。

随后按需要进入相应 guide、spec 或 roadmap。

## 原则、产品与现状

- [`CHARTER.md`](CHARTER.md) — 平台身份、五个长期目标与不可妥协原则
- [`architecture/VISION.md`](architecture/VISION.md) — 长期整体形态与技术方向
- [`product/PLATFORM_PRODUCT_MODEL.md`](product/PLATFORM_PRODUCT_MODEL.md) — 官方发行版的用户生命周期、好用原则与产品边界
- [`product/PLAY_CREATION_MODEL.md`](product/PLAY_CREATION_MODEL.md) — 可选的游创产品 Profile，不代表整个平台唯一形态
- [`ALPHA_STATUS.md`](ALPHA_STATUS.md) — 当前已实现、partial 与 deferred 的事实快照
- [`roadmap/NEXT_STEPS.md`](roadmap/NEXT_STEPS.md) — 围绕开放、多样、先进、长久、好用的建设方向
- [`STYLE.md`](STYLE.md) — 文档事实层级、双语同步与写作红线
- [`LICENSING.md`](LICENSING.md) — `AGPL-3.0-only` 第一方范围与第三方许可证边界
- [`../BUILDING.md`](../BUILDING.md) — Rust、Web、Tauri Desktop 与 release 构建说明

## 架构与公开合同

- [`architecture/`](architecture/README.md) — 分层架构、宪法基底、组件与 Host 控制平面
- [`protocol/`](protocol/README.md) — 当前公开传输与调用协议
- [`spec/`](spec/README.md) — Contract V1 schema、Contract Registry、协议公地与兼容合同
- [`architecture/CONSTITUTION_V2.md`](architecture/CONSTITUTION_V2.md) → [`spec/CONTRACT_LAYERING_MATRIX.md`](spec/CONTRACT_LAYERING_MATRIX.md) — 候选长期宪法与当前合同逐项归属；尚未替代 v1

## 创作、安装与运行

- [`guides/`](guides/README.md) — 按基础、agent、模型、推理、体验、记忆、存储、外部项目和分发分组的指南
- [`guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](guides/PACKAGE_AUTHORING_WALKTHROUGH.md) — 第一个 Package / Component
- [`guides/CAPABILITY_HANDLES.md`](guides/CAPABILITY_HANDLES.md) — authority handle、衰减、撤销与 effect audit
- [`guides/CONFORMANCE_KIT.md`](guides/CONFORMANCE_KIT.md) — 第三方实现的 Contract V1 行为检查
- [`guides/PACKAGE_INSTALLATION.md`](guides/PACKAGE_INSTALLATION.md) — 安装、更新、lockfile、内容寻址 store 与同意提示
- [`guides/INSTALLATION_MODEL.md`](guides/INSTALLATION_MODEL.md) — Work / Workspace / Installation 对象与生命周期边界
- [`guides/SECRET_MANAGEMENT.md`](guides/SECRET_MANAGEMENT.md) — `secret_ref`、本地加密 store 与 API key 管理
- [`guides/REAL_MODEL_END_TO_END.md`](guides/REAL_MODEL_END_TO_END.md) — 真实 provider 端到端调用
- [`guides/PATH_B_SELF_CONTAINED.md`](guides/PATH_B_SELF_CONTAINED.md) — `entry.contract: "none"` 自包含路径
- [`guides/SURFACE_HOSTING.md`](guides/SURFACE_HOSTING.md) — iframe SurfaceHost 与第三方 Web Surface 托管

## 性能、状态与外部接入

- [`performance/`](performance/README.md) — 性能基线、质量反馈与代码健康
- [`roadmap/`](roadmap/README.md) — 当前仍影响取舍的建设方向
- [`tavern/`](tavern/README.md) — Plurora 与独立接入项目 YdlTavern 的关系

## 最短读路径

| 你想 | 先读 |
|---|---|
| 理解平台目标 | [`CHARTER.md`](CHARTER.md) → [`architecture/VISION.md`](architecture/VISION.md) |
| 理解分层架构 | [`architecture/ARCHITECTURE.md`](architecture/ARCHITECTURE.md) → [`architecture/CONSTITUTIONAL_SUBSTRATE.md`](architecture/CONSTITUTIONAL_SUBSTRATE.md) → [`architecture/CAPABILITY_PACKAGE.md`](architecture/CAPABILITY_PACKAGE.md) |
| 理解官方产品 | [`product/PLATFORM_PRODUCT_MODEL.md`](product/PLATFORM_PRODUCT_MODEL.md) → [`design/PLATFORM_UI_DESIGN.md`](design/PLATFORM_UI_DESIGN.md) |
| 审阅长期合同边界 | [`architecture/CONSTITUTION_V2.md`](architecture/CONSTITUTION_V2.md) → [`spec/CONTRACT_LAYERING_MATRIX.md`](spec/CONTRACT_LAYERING_MATRIX.md) → [`spec/CONTRACT_REGISTRY.md`](spec/CONTRACT_REGISTRY.md) |
| 接入公开协议 | [`protocol/PUBLIC_PROTOCOL.md`](protocol/PUBLIC_PROTOCOL.md) → [`spec/PUBLIC_CONTRACT.md`](spec/PUBLIC_CONTRACT.md) |
| 写第一个 Package / Component | [`guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](guides/PACKAGE_AUTHORING_WALKTHROUGH.md) |
| 打包 Work / 创建 Installation | [`guides/PACKAGE_INSTALLATION.md`](guides/PACKAGE_INSTALLATION.md) → [`guides/INSTALLATION_MODEL.md`](guides/INSTALLATION_MODEL.md) |
| 管理 API key / secret | [`guides/SECRET_MANAGEMENT.md`](guides/SECRET_MANAGEMENT.md) |
| 跑真实模型调用 | [`guides/REAL_MODEL_END_TO_END.md`](guides/REAL_MODEL_END_TO_END.md) |
| 挂载第三方 Web Surface | [`guides/SURFACE_HOSTING.md`](guides/SURFACE_HOSTING.md) |
| 构建 Web / Desktop / Release | [`../BUILDING.md`](../BUILDING.md) |
| 看当前状态 | [`ALPHA_STATUS.md`](ALPHA_STATUS.md) |
| 看建设方向 | [`roadmap/NEXT_STEPS.md`](roadmap/NEXT_STEPS.md) |
| 写文档 | [`STYLE.md`](STYLE.md) |
