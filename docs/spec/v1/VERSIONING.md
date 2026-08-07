# v1 版本策略

## 方法命名空间

历史 v1 schema 文件继续保留 `platform.*` 名称。Contract Registry 可以发布分层 canonical wire ID（例如 `host.target.list`），并把相应 `platform.*` ID 作为显式 compatibility alias 保留。两个 ID 在唯一解析边界进入同一 handler，并保持完全相同的 v1 payload 语义。未来破坏性 wire contract 使用单独协商的 major version，不覆盖 v1。

## Schema 规则

`docs/spec/v1/schemas/` 是 v1 的公开规范产物。v1 只能 additive-only 演进：

- 可以新增可选字段、事件类型、方法或 enum 变体（如果旧实现可安全忽略）。
- 不得删除字段、改变字段类型、把可选字段改为必填、收紧已发布 enum、改变事件 payload 的既有含义。
- schema 变更必须通过 `scripts/validate-schemas.sh`；CI 会设置 `BASE_SCHEMA_DIR`，检测文件删除以及常见结构性破坏，包括 type/const 变化、新增 required 字段、enum 收窄、property/definition 删除、边界收紧和不兼容的组合器变化。

这里的 additive-only 保证针对序列化后的 v1 wire contract。Rust crates 与生成 SDK 目前仍处于 pre-1.0 阶段：新增 wire 层可选字段可能会给公开 Rust struct 增加字段，从而使下游的 struct literal 无法继续编译。消费者应优先使用构造函数、builder 或反序列化入口，并在升级时遵循对应 crate / SDK 的语义版本。

## 何时进入 v2

当需要破坏性变更时进入 `kernel.v2.*`，例如：必填字段变化、已有字段类型变化、错误码语义变化、权限模型不兼容、或事件 payload 需要不可兼容重塑。v2 不会覆盖 v1；host 可同时暴露多个版本。

## 协商

新客户端调用 canonical `host.info`，除历史 method/status 字段外，还应读取 `contract_registry_version`、`contract_methods`、`aliases`、profiles 与 protocol descriptors。旧客户端仍可调用 `host.info`；Registry `0.4.0` 将该 alias 标记为 Deprecated，Registry `0.5.0` 再将它转换为 identity Legacy Adapter，并保留原支持窗口 metadata 与迁移诊断。该 adapter 只接受安全修复和数据读取兼容，不增加新字段语义。需要某个方法的客户端应显式选择受支持的 contract/profile、优先使用 host 发布的 canonical ID，并在版本不支持时拒绝继续，而不是猜测或静默降级。
