# 创作指南

> [English](./README.en.md) · [中文](./README.md)

按读者任务分组。全部建立在公开协议、清单与 Surface 契约之上；第一方没有旁路。

## 起步

先读这五篇：

- [`GETTING_STARTED.md`](GETTING_STARTED.md) — 从源码启动 Host 与 Web Shell
- [`PACKAGE_AUTHORING_WALKTHROUGH.md`](PACKAGE_AUTHORING_WALKTHROUGH.md) — 第一个第三方 Package
- [`INSTALLATION_MODEL.md`](INSTALLATION_MODEL.md) — Work / Workspace / Installation 模型与安装、更新、移除
- [`RUN_LIBRARY.md`](RUN_LIBRARY.md) — 启动和停止 Run
- [`SECRET_MANAGEMENT.md`](SECRET_MANAGEMENT.md) — `secret_ref` 与 API key

## 进阶

- [`POWERBOX_BINDING.md`](POWERBOX_BINDING.md) — Exposure、Powerbox 与跨 Installation Binding
- [`REALIZATION.md`](REALIZATION.md) — 把 Work 编译到 Target；含 executor / 代理附录
- [`FOREIGN_WORK_RIGHTS.md`](FOREIGN_WORK_RIGHTS.md) — ForeignCapsule、Rights / Transparency 与不透明启动
- [`HOST_OPERATIONS.md`](HOST_OPERATIONS.md) — 健康检查、备份恢复、部署与 release 校验
- [`CAPABILITY_HANDLES.md`](CAPABILITY_HANDLES.md) — authority handle、衰减、撤销与 effect audit
- [`CONFORMANCE_KIT.md`](CONFORMANCE_KIT.md) — 第三方包的 Contract V1 行为检查
- [`PATH_B_SELF_CONTAINED.md`](PATH_B_SELF_CONTAINED.md) — `entry.contract: "none"` 自包含路径
- [`SURFACE_HOSTING.md`](SURFACE_HOSTING.md) — iframe SurfaceHost 与第三方 Web Surface
- [`SHARING_DISTRIBUTION.md`](SHARING_DISTRIBUTION.md) — 分享、bundle 与 package-set lockfile
- [`STORAGE_BACKEND_NEUTRALITY.md`](STORAGE_BACKEND_NEUTRALITY.md) — 与具体后端无关的存储契约

## 模型

从不出网的连通性合同读到真实调用：

- [`MODEL_CONNECTIVITY_KIT.md`](MODEL_CONNECTIVITY_KIT.md) — provider profile 与路由计划（默认不出网）
- [`MODEL_PROVIDER_INTEGRATION.md`](MODEL_PROVIDER_INTEGRATION.md) — 多云 adapter 形状与 quirk
- [`INFERENCE_CAPABILITY_AUTHORING.md`](INFERENCE_CAPABILITY_AUTHORING.md) — 与传输无关的推理能力
- [`REAL_MODEL_END_TO_END.md`](REAL_MODEL_END_TO_END.md) — 真实 provider 端到端调用

## Agent 与体验

三篇保留独立入口，模板级内容互相引用而不重复展开：

- [`AGENT_PACKAGE_AUTHORING.md`](AGENT_PACKAGE_AUTHORING.md) — 类 agent 能力包
- [`AGENTIC_FORGE_PACKAGE_AUTHORING.md`](AGENTIC_FORGE_PACKAGE_AUTHORING.md) — 计划图、scratch 分支、工具桥
- [`EXPERIENCE_RUNTIME_AUTHORING.md`](EXPERIENCE_RUNTIME_AUTHORING.md) — checkpoint、recovery、Play / Forge / Assist 绑定
- [`MEMORY_PACKAGE_AUTHORING.md`](MEMORY_PACKAGE_AUTHORING.md) — 记忆 / 知识能力包

## 实验室与示例

这些不是默认上手路径：

- [`MODULAR_SIMULATION.md`](MODULAR_SIMULATION.md) — 可继续创作的 simulation Work kit
- [`POSTGRES_TDB_INTEGRATION.md`](POSTGRES_TDB_INTEGRATION.md) — PostgreSQL 事件后端与 TDB 检索 provider
