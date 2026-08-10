# 创作指南

> [English](./README.en.md) · [中文](./README.md)

按领域分组的能力包创作指南。每篇都对应一类能力包或一段创作循环，全部建立在公开协议、清单与 surface 契约之上。

## 起步

- [`PACKAGE_AUTHORING_WALKTHROUGH.md`](PACKAGE_AUTHORING_WALKTHROUGH.md) — 第三方能力包创作 walkthrough（init-package、check、run-fixture、reload、composition）
- [`PACKAGE_INSTALLATION.md`](PACKAGE_INSTALLATION.md) — 包/项目安装与更新、lockfile、`~/.plurora` 布局、签名/完整性、bundle freshness 与同意提示
- [`INSTALLATION_MODEL.md`](INSTALLATION_MODEL.md) — Work / Workspace / Installation 边界、journal/projection、state update/remove 与 Installation secret policy
- [`DEPLOYMENT_RUNTIME.md`](DEPLOYMENT_RUNTIME.md) — target / exec / port / proxy 部署运行时、plurora-service 反代、Docker Deploy broker
- [`HOST_OPERATIONS.md`](HOST_OPERATIONS.md) — Host liveness/readiness、SQLite 离线备份与原子恢复、release 校验
- [`SECRET_MANAGEMENT.md`](SECRET_MANAGEMENT.md) — `secret_ref:env:` / `secret_ref:store:`、本地加密 secret store 与 API key 管理
- [`REAL_MODEL_END_TO_END.md`](REAL_MODEL_END_TO_END.md) — 从 YdlTavern Send 到真实模型 provider 响应的完整链路
- [`CAPABILITY_HANDLES.md`](CAPABILITY_HANDLES.md) — 内核 v1 能力句柄模型、衰减、撤销、bindings 与 effect audit
- [`CONFORMANCE_KIT.md`](CONFORMANCE_KIT.md) — 第三方包本地验证 v1 contract compliance
- [`PATH_B_SELF_CONTAINED.md`](PATH_B_SELF_CONTAINED.md) — 自包含 Path B 包（`entry.contract: "none"`）
- [`SURFACE_HOSTING.md`](SURFACE_HOSTING.md) — iframe SurfaceHost、第三方 Web surface bundle 约定与 host bridge
- [`ZEABUR_QUICK_VALIDATION.md`](ZEABUR_QUICK_VALIDATION.md) — 面向 Zeabur 的单容器 Web 快速验证部署

## 创作能力族

- [`CREATIVE_CAPABILITY_KIT.md`](CREATIVE_CAPABILITY_KIT.md) — Plurora 通用创作能力包（persona / knowledge / context / text-transform）
- [`MODEL_CONNECTIVITY_KIT.md`](MODEL_CONNECTIVITY_KIT.md) — model provider profile 与 route planning kit
- [`MODEL_PROVIDER_INTEGRATION.md`](MODEL_PROVIDER_INTEGRATION.md) — 多 provider 模型接入（OpenAI、Anthropic、Gemini、OpenAI-compatible、OpenRouter、DeepSeek、xAI、Fireworks）
- [`INFERENCE_CAPABILITY_AUTHORING.md`](INFERENCE_CAPABILITY_AUTHORING.md) — 与传输无关的推理能力包创作

## Agent 与体验

- [`AGENT_PACKAGE_AUTHORING.md`](AGENT_PACKAGE_AUTHORING.md) — 类 agent 能力包创作
- [`AGENTIC_FORGE_PACKAGE_AUTHORING.md`](AGENTIC_FORGE_PACKAGE_AUTHORING.md) — Agentic Forge runtime 能力包（计划图、scratch 分支、工具桥）
- [`EXPERIENCE_RUNTIME_AUTHORING.md`](EXPERIENCE_RUNTIME_AUTHORING.md) — 体验运行时能力包（checkpoint、recovery、agent run binding）
- [`MEMORY_PACKAGE_AUTHORING.md`](MEMORY_PACKAGE_AUTHORING.md) — 记忆 / 知识能力包

## 平台扩展

- [`SHARING_DISTRIBUTION.md`](SHARING_DISTRIBUTION.md) — 分享与分发：composition bundle、package-set lockfile、AI disclosure
- [`STORAGE_BACKEND_NEUTRALITY.md`](STORAGE_BACKEND_NEUTRALITY.md) — backend-neutral 存储契约与官方实验室
- [`POSTGRES_TDB_INTEGRATION.md`](POSTGRES_TDB_INTEGRATION.md) — PostgreSQL（事件后端）+ TDB（检索 provider）接入
- [`WORKSPACE_OPERATING_PLANE.md`](WORKSPACE_OPERATING_PLANE.md) — Workspace ownership、静态 intake、计划/效果分离与受控源码变更
