# v1 版本策略

> [English](./VERSIONING.en.md) · [中文](./VERSIONING.md)

## 精确身份

Contract Registry `0.1.0` 对每个 method 只发布一个 owner-based wire ID。v1 工作树不包含 method alias 或平行 event namespace。Schema 文件名、`$id` URN、OpenAPI operation ID、runtime dispatch 与生成 SDK method 都来自同一精确身份。

## v1 additive 规则

`docs/spec/v1/schemas/` 是公开 v1 契约产物。在已经发布的 v1 边界内：

- 只有在现有实现可安全忽略时，才可新增 optional field、method、event kind 或开放 enum value；
- 不得删除字段或改变已有字段类型；
- optional field 不得变成 required；
- closed enum 不得收窄；
- 已有 error 与 event 语义不得改变；
- 声明为 forward-compatible 的 unknown field 必须继续可读。

Schema 变更必须通过 `scripts/validate-schemas.sh`。CI 会与 base schema tree 比较，并检查文件删除、type/const 变化、新增 required、enum 收窄、property/definition 删除、边界收紧与不兼容 combinator 变化。

Serialized wire guarantee 与 crate/SDK 的 semantic versioning 不同。Rust crate 与生成 SDK 仍处于 pre-1.0；wire 层 additive field 仍可能影响 source-level struct literal。消费者应优先使用 constructor、builder 或 deserialization，并遵循各 artifact 的语义版本。

## Breaking change

Breaking change 使用新的显式 contract/profile/version 边界，例如新的 `plurora.contract.default/v2` profile 或新的逐层 major version。它不会覆盖 v1，也不会隐藏在未声明 alias 中。

Breaking change 包括：

- required field 或已有字段类型变化；
- 不兼容 authority 或 error semantics；
- event payload 重新解释；
- 删除或重命名 Stable method/event identity；
- 不兼容的 Protocol Commons lifecycle 或 profile 变化。

Stable breaking transition 必须提供 migration tooling、旧数据可读性、显式 negotiation、support policy 与每个受支持边界的 conformance vector。

## 协商

Client 调用 `host.info`，读取 `contract_registry_version`、`contract_methods`、profile、layer version、支持的 transport 与 Protocol Commons descriptor。Client 选择受支持的 contract/profile；不支持时拒绝继续，不猜测也不静默 downgrade。

省略 contract selection 时选择 `plurora.contract.default/v1`。显式 requirement 必须精确匹配；未知 profile、重复 requirement、不支持的 Protocol major 与 version mismatch 会在 method dispatch 前以 `protocol/error/unsupported_contract` fail closed。

## 预发布重置

边界进入 Stable 前，可以通过一次协调一致的 destructive reset 替换 Experimental identity set。重置必须原子覆盖 runtime、schema、SDK、client、test、data convention 与 documentation。完成后的工作树只保留被选定的一套身份，不把临时 alias 留作永久架构。
