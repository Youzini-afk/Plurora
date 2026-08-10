# 规范

> [English](./README.en.md) · [中文](./README.md)

可执行 v1 契约和 hostile conformance 路线图。规范文件以代码 + 用例支撑——不是愿景文档。

- [`PUBLIC_CONTRACT.md`](PUBLIC_CONTRACT.md) — v1 公开契约：80 个方法、58 个事件、capability handle、Path A / Path B、SDK 与 conformance
- [`CONTRACT_LAYERING_MATRIX.md`](CONTRACT_LAYERING_MATRIX.md) — 80 个方法、58 个事件与顶层 schema 的 Candidate owner 矩阵
- [`CONTRACT_REGISTRY.md`](CONTRACT_REGISTRY.md) — 可执行的 exact-ID registry 与显式 contract/profile/version 协商
- [`OBJECT_STORE.md`](OBJECT_STORE.md) — Experimental SHA-256 ObjectStore、ArtifactDescriptor、asset 转换与 verified read
- [`EFFECT_RECEIPTS.md`](EFFECT_RECEIPTS.md) — Experimental EffectReceipt、terminal evidence、historical replay 与 branch re-execute
- [`CHANGE_WORKFLOW.md`](CHANGE_WORKFLOW.md) — Intent/ChangeSet/PolicyDecision/Commit 与 `change.proposal.*` facade
- [`PROTOCOL_COMMONS.md`](PROTOCOL_COMMONS.md) — protocol descriptor、Work / Assembly 便携契约、语义/Profile 协商、adapter 以及独立 protocol/implementation 报告
- [`COMPONENT_IDENTITY.md`](COMPONENT_IDENTITY.md) — package envelope、独立 component identity、trust claim、Foreign Capsule 与 AssemblyLock
- [`WORLD_BUNDLE.md`](WORLD_BUNDLE.md) — 跨 host archive 完整性、原始 journal envelope、离线回放、lineage 与 Shell 独立性
- [`CONFORMANCE_MATRIX.md`](CONFORMANCE_MATRIX.md) — hostile conformance 用例清单（按 tag 与领域索引）
- [`v1/EVENT_KIND_REGISTRY.md`](v1/EVENT_KIND_REGISTRY.md) — v1 事件类型 registry
- [`v1/ERROR_CODES.md`](v1/ERROR_CODES.md) — v1 错误码
- [`v1/VERSIONING.md`](v1/VERSIONING.md) — v1 additive-only 版本策略
- [`v1/schemas/`](v1/schemas/) — 177 个 JSON Schema（80 methods + 58 events + 39 top-level），SDK 的单一可信源

跑全套 conformance：

```bash
cargo run -p plurora-cli -- conformance
```
