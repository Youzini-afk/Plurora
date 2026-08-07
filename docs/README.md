# Yggdrasil 文档

> [English](./README.en.md) · [中文](./README.md)

按主题分组的开发文档导航。每篇都同时提供英文与简体中文版本，文件顶部的双语 blockquote 可在两种语言间切换。写文档时请遵循 [`STYLE.md`](STYLE.md)。

## 新人 1 / 2 / 3 路径

1. 读 [`CHARTER.md`](CHARTER.md) → [`architecture/VISION.md`](architecture/VISION.md) → [`product/PLAY_CREATION_MODEL.md`](product/PLAY_CREATION_MODEL.md)，理解平台立场。
2. 读 [`architecture/ARCHITECTURE.md`](architecture/ARCHITECTURE.md) → [`architecture/PLATFORM_KERNEL.md`](architecture/PLATFORM_KERNEL.md) → [`architecture/CAPABILITY_PACKAGE.md`](architecture/CAPABILITY_PACKAGE.md)，理解三层架构与边界。
3. 读 [`guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](guides/PACKAGE_AUTHORING_WALKTHROUGH.md) 跑通第一个能力包。

随后按需要查相应的 guide / spec / roadmap。

## 立场与现状

- [`CHARTER.md`](CHARTER.md) — 不变的根本原则
- [`ALPHA_STATUS.md`](ALPHA_STATUS.md) — 当前状态、partial、deferred 的活快照
- [`STYLE.md`](STYLE.md) — 文档体例与红线
- [`LICENSING.md`](LICENSING.md) — `AGPL-3.0-only` 第一方范围与第三方许可证边界
- [`../BUILDING.md`](../BUILDING.md) — Rust、Web、Tauri desktop 与 release 构建说明
- [`product/`](product/README.md) — 游创一体产品立场

## 架构与协议

- [`architecture/`](architecture/README.md) — kernel + packages + projects 三层架构、能力包契约、扩展点、事件模型、生命周期
- [`protocol/`](protocol/README.md) — 公开协议规范
- [`spec/`](spec/README.md) — 可执行 v1 契约矩阵、conformance 路线图、schemas
- [`architecture/CONSTITUTION_V2.md`](architecture/CONSTITUTION_V2.md) → [`spec/CONTRACT_LAYERING_MATRIX.md`](spec/CONTRACT_LAYERING_MATRIX.md) — 候选 v2 宪法与逐项合同归属；尚未替代 v1

## 创作

- [`guides/`](guides/README.md) — 能力包创作指南，按域分组（基础 / agent / 模型 / 推理 / 体验 / 记忆 / 存储 / 外部项目 / 分发）
- [`guides/CAPABILITY_HANDLES.md`](guides/CAPABILITY_HANDLES.md) — v1 能力句柄、衰减、撤销与 effect audit
- [`guides/CONFORMANCE_KIT.md`](guides/CONFORMANCE_KIT.md) — 第三方包 v1 conformance kit
- [`guides/PACKAGE_INSTALLATION.md`](guides/PACKAGE_INSTALLATION.md) — `yg install/update`、lockfile、内容寻址 store、bundle freshness 与同意提示
- [`guides/PROJECT_MODEL.md`](guides/PROJECT_MODEL.md) — Home 项目架、`project.yaml`、项目 lifecycle、控制台诊断/更新与项目级 secrets
- [`guides/SECRET_MANAGEMENT.md`](guides/SECRET_MANAGEMENT.md) — `secret_ref:env:` / `secret_ref:store:`、本地加密 store 与 API key 管理
- [`guides/REAL_MODEL_END_TO_END.md`](guides/REAL_MODEL_END_TO_END.md) — YdlTavern Send 到真实 provider 响应的端到端链路
- [`guides/PATH_B_SELF_CONTAINED.md`](guides/PATH_B_SELF_CONTAINED.md) — `entry.contract: "none"` 自包含路径
- [`guides/SURFACE_HOSTING.md`](guides/SURFACE_HOSTING.md) — `clients/web` iframe SurfaceHost 与第三方 Web surface bundle 托管

## 性能与路线图

- [`performance/`](performance/README.md) — 性能基线、conformance 反馈环、代码健康
- [`roadmap/`](roadmap/README.md) — 当前与下一阶段、模型推理前置条件
- [`roadmap/CONTRACT_V2_MIGRATION.md`](roadmap/CONTRACT_V2_MIGRATION.md) — 分层合同迁移顺序、兼容门与完成定义
- [`tavern/`](tavern/README.md) — Yggdrasil 与 SillyTavern 兼容接入项目 YdlTavern 的关系

## 最短读路径

| 你想 | 先读 |
|---|---|
| 理解平台立场 | [`CHARTER.md`](CHARTER.md) → [`architecture/VISION.md`](architecture/VISION.md) → [`product/PLAY_CREATION_MODEL.md`](product/PLAY_CREATION_MODEL.md) |
| 理解架构 | [`architecture/ARCHITECTURE.md`](architecture/ARCHITECTURE.md) → [`guides/PROJECT_MODEL.md`](guides/PROJECT_MODEL.md) → [`architecture/PLATFORM_KERNEL.md`](architecture/PLATFORM_KERNEL.md) → [`architecture/CAPABILITY_PACKAGE.md`](architecture/CAPABILITY_PACKAGE.md) |
| 审阅候选 v2 边界 | [`architecture/CONSTITUTION_V2.md`](architecture/CONSTITUTION_V2.md) → [`spec/CONTRACT_LAYERING_MATRIX.md`](spec/CONTRACT_LAYERING_MATRIX.md) → [`roadmap/CONTRACT_V2_MIGRATION.md`](roadmap/CONTRACT_V2_MIGRATION.md) |
| 接入公开协议 | [`protocol/PROTOCOL_V0.md`](protocol/PROTOCOL_V0.md) → [`spec/KERNEL_V1_CONTRACT.md`](spec/KERNEL_V1_CONTRACT.md) |
| 写第一个能力包 | [`guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](guides/PACKAGE_AUTHORING_WALKTHROUGH.md) |
| 安装能力包/项目 | [`guides/PACKAGE_INSTALLATION.md`](guides/PACKAGE_INSTALLATION.md) → [`guides/PROJECT_MODEL.md`](guides/PROJECT_MODEL.md) |
| 管理 API key / secret | [`guides/SECRET_MANAGEMENT.md`](guides/SECRET_MANAGEMENT.md) |
| 跑真实模型端到端调用 | [`guides/REAL_MODEL_END_TO_END.md`](guides/REAL_MODEL_END_TO_END.md) |
| 挂载第三方 Web surface | [`guides/SURFACE_HOSTING.md`](guides/SURFACE_HOSTING.md) |
| 构建 Web / Desktop / Release | [`../BUILDING.md`](../BUILDING.md) |
| 看当前状态 | [`ALPHA_STATUS.md`](ALPHA_STATUS.md) |
| 看下一步 | [`roadmap/NEXT_STEPS.md`](roadmap/NEXT_STEPS.md) |
| 写文档 | [`STYLE.md`](STYLE.md) |
