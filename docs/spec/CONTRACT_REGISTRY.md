# Contract Registry 与显式协商（Experimental）

> [English](./CONTRACT_REGISTRY.en.md) · [中文](./CONTRACT_REGISTRY.md)

本文描述当前用于分层合同、canonical ID、legacy alias 与显式协商的可执行兼容机制。它是 Experimental，不表示
Constitution v2 已经 Stable，也不改变现有 `platform.*` payload 语义。

## 单一解析边界

Runtime 在进入 permission gate 和 handler 前统一执行：

1. 校验可选的 contract selection；
2. 从集中 registry 解析 canonical ID 或 alias；
3. 执行 request adapter；
4. 以 `PlatformMethod` 调用唯一 handler；
5. 执行 response adapter。

HTTP RPC、host stdio、in-process 和 subprocess reverse stdio 使用同一解析与协商逻辑。
Alias 不创建新 handler、principal、permission 或 audit 分支。

## Registry 形状

每个 `ContractMethod` 发布：

- `canonical_id` 与 `aliases`；
- `owner_layer` 与 `maturity`；
- request / response schema URI；
- request / response adapter；
- introduced / deprecated / replacement metadata；
- 当前实现状态与 streaming 标记。

Registry `0.5.0` 当前发布 36 条 identity alias：

| Canonical | Legacy alias | Owner |
|---|---|---|
| `host.info` | `host.info` | `host` |
| `host.project.{list,get,start,stop,status}` | `platform.project.*` | `host` |
| `host.target.{list,status,register,unregister}` | `platform.target.*` | `host` |
| `host.exec.{start,stop,status,logs,list}` | `platform.exec.*` | `host` |
| `host.port.{lease,release,status,list}` | `platform.port.*` | `host` |
| `host.proxy.{register,unregister,status,list}` | `platform.proxy.*` | `host` |
| `host.surface.bundle.resolve` | `host.surface.bundle.resolve` | `host` |
| `shell.contribution.{list,describe}` | `platform.surface.contribution.*` | `shell` |
| `change.proposal.{create,get,list,approve,reject,apply}` | `platform.proposal.*` | `protocol` |
| `projection.{register,rebuild,get,list}` | `platform.projection.*` | `protocol` |

表中的 `*` / `{...}` 仅是文档缩写，每个后缀都在 registry 中逐项注册。其他方法在迁移前
继续以现有 `platform.*` ID 作为 canonical ID。新增 alias 必须进入 registry，不能在
dispatcher、客户端或 transport 中加入字符串特判。

Canonical/legacy 双栈只改变 owner 与 namespace：payload、权限、事件与 handler 保持一致。尤其
`change.proposal.*` 仍使用现有 `ProposalRecord`；Intent / ChangeSet / Commit / EffectReceipt
是独立的 Experimental primitives，不能由 alias 名称推断为已被 proposal facade 完整实现。

## 显式协商

RPC envelope 可带可选字段：

```json
{
  "id": "request-1",
  "method": "host.info",
  "params": {},
  "contract": {
    "profile": "plurora.contract.default/v1",
    "versions": [
      { "layer": "host", "version": "0.1.0" }
    ]
  }
}
```

- 省略 `contract` 时，为旧客户端使用 `platform contract` legacy profile。
- 当前公开 `plurora.contract.default/v1`、`plurora.shell.default/v1` 与 `platform contract`；Shell Default
  精确要求 host、protocol、shell 三层的已发布版本。
- 一旦客户端显式给出 profile 或 layer version，host 必须精确满足。
- 未知 profile、profile 不包含所需 layer、或 version 不匹配时返回
  `runtime/error/unsupported_contract`，并在结构化 details 中报告原因。
- Host 不会自动回退到更弱 profile，也不会在协商失败后调用业务 handler。

## `host.info`

原有 `protocol_version`、`methods`、`supported_transports` 保持不变。新增字段均为
additive optional 字段：

- `contract_registry_version`、`default_profile`；
- `layers`、`versions`、`profiles`、`maturity`；
- `aliases`、`contract_methods`。

因此旧 SDK 可以忽略新字段；新 SDK 连接旧 host 时也必须允许这些字段缺失。

## SDK

生成器读取每个 method schema 的 `x-plurora-contract` metadata：

- 原有方法名调用 canonical wire ID；
- 每个 legacy wire ID 生成显式 `legacyKernelV1...` / `legacy_kernel_v1_...`
  wrapper；
- negotiated client 只有在 transport 能携带 contract selection 时才启用，不能静默忽略选择。
- 生成前校验 canonical/alias wire ID、TypeScript/Rust 函数名和 OpenAPI operation ID
  全局唯一，并校验 alias 的 canonical target 与 replacement。

Schema、SDK 与 OpenAPI 必须通过生成器更新，不手工修改生成物。

## Legacy Adapter 转换与诊断

Registry `0.4.0` 开始第一个可验证的弃用窗口，`0.5.0` 完成第一次真实的
`Deprecated → Legacy Adapter` 转换：

| Legacy alias | 当前成熟度 | Replacement | Replacement maturity | Deprecated in | Legacy Adapter from |
|---|---|---|---|---|---|
| `host.info` | Legacy Adapter | `host.info` | Candidate | `plurora.contract.registry@0.4.0` | `plurora.contract.registry@0.5.0` |
| `host.target.list` | Legacy Adapter | `host.target.list` | Candidate | `plurora.contract.registry@0.4.0` | `plurora.contract.registry@0.5.0` |

历史 `deprecated_in`、`replacement` 与 `support_until` metadata 保留。旧 ID 与 canonical ID
仍进入同一个 handler、共享同一 request/response schema，并通过 identity adapter 保持 method
result 完全一致。进入 Legacy Adapter 后，旧 ID 只接受安全修复和数据读取兼容，不再增加新字段
语义。

HTTP RPC、host stdio 和 subprocess reverse stdio 在调用受跟踪的 Legacy Adapter alias 时，
会附加 code 为 `plurora.contract.alias.legacy_adapter` 的可选顶层 `diagnostics` 数组。兼容路由
`GET /removed/host.info` 通过
`x-plurora-contract-*` response header 和指向 `/rpc` 的 `Link` 发布同一策略。
Replacement header 的值是 canonical method ID，而不是 URL；应通过 `POST /rpc` 调用。
诊断只用于迁移提示，不改变 method payload 或 error mapping；即使 contract selection
结构错误，只要仍能提取请求的 legacy method ID，也会保留对应诊断。

只读预览：

```sh
plurora contract migrate PATH --json
```

默认只迁移带已发布生命周期/deprecation metadata 的 alias；增加 `--all-aliases` 才会主动迁移全部
registered alias，且应先审阅 preview 再加 `--write`。替换要求完整 contract-ID 边界；扫描器
只接受保守的源码/Markdown 扩展名白名单，并逐项报告不支持、非 UTF-8 或超限文件，以及所有
被排除的 symlink、构建产物和依赖/vendor 目录。写入使用同目录 staging 与原子替换；若后续
文件写入失败，会回滚此前已写文件；任何 excluded path 都不会被跟随。Web 是第一个以
`--all-aliases` 完成迁移的真实客户端，
其 protocol client、surface bridge、bundle resolver、测试与说明文档现已使用 canonical ID。
