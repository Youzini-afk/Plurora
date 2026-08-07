# Contract Registry 与显式协商（Experimental）

> [English](./CONTRACT_REGISTRY.en.md) · [中文](./CONTRACT_REGISTRY.md)

本文描述 Plurora 公开契约的可执行 registry。Registry 是方法身份、所属层、成熟度、schema、实现状态、streaming 行为与显式契约协商的唯一事实来源。

当前预发布 registry 对每个方法只暴露一个 wire ID，不解析替代 ID，不运行 request/response adapter，也不发布并行兼容面。

## 单一解析边界

任何 permission gate 或 handler 执行前，所有 transport 都走同一顺序：

1. 校验可选的 contract selection；
2. 按 registry ID 精确解析请求方法；
3. 附加由 Host 建立的 principal 与 transport context；
4. 分发到唯一的 `PlatformMethod` handler；
5. 返回统一 result 或结构化 error envelope。

HTTP RPC、Host stdio、in-process 调用与 subprocess reverse stdio 共用这条边界。缺失或未知 ID 会在业务分发前失败。

## Registry 形状

Registry `0.1.0` 发布 80 条 `ContractMethod` 记录。每条记录包含：

- `id` —— 唯一公开 wire ID；
- `owner_layer` —— `substrate`、`host`、`protocol` 或 `shell`；
- `maturity` —— `experimental`、`candidate` 或 `stable`；
- request / response schema URI；
- `introduced_in`；
- 实现状态；
- streaming 标记。

ID 的第一段声明 owner：

| Prefix | Owner | 示例 |
|---|---|---|
| `context`、`journal`、`capability`、`authority`、`object`、`identity` | Substrate | `context.open`、`journal.append`、`authority.handle.revoke` |
| `host` | Host | `host.project.list`、`host.outbound.execute` |
| `protocol`、`change`、`projection` | Protocol | `protocol.extension.list`、`change.proposal.apply` |
| `shell` | Shell | `shell.contribution.list` |

Package capability ID 仍属于 Package 自己的 slash namespace，例如 `org/package/capability`；它们不是公开契约方法 ID。

## 显式协商

RPC envelope 可携带 contract selection：

```json
{
  "id": "request-1",
  "method": "host.info",
  "params": {},
  "contract": {
    "profile": "plurora.contract.default/v1",
    "versions": [
      { "layer": "host", "version": "0.1.0" }
    ],
    "protocols": []
  }
}
```

- 省略 `contract` 时选择 `plurora.contract.default/v1`。
- `plurora.contract.default/v1` 精确要求已发布的 Substrate、Host、Protocol 与 Shell 层版本。
- `plurora.shell.default/v1` 精确要求已发布的 Host、Protocol 与 Shell 层版本。
- 显式 layer requirement 必须精确匹配。
- 重复 requirement、未知 profile、profile 外 layer 与版本不匹配都会 fail closed。
- 显式 Protocol Commons selection 会在方法分发前完成协商。
- 协商不会静默回退到其他 profile 或版本。

无法满足的选择返回 `protocol/error/unsupported_contract` 及结构化原因，且不会调用目标 handler。

## `host.info`

`host.info` 发布 registry 版本、默认 profile、layer/version descriptor、profiles、method descriptor、支持的 transport 与 Protocol Commons descriptor。客户端必须通过该响应发现能力，不能从产品品牌或 Package ID 推断支持情况。

## Schema 与 SDK

每个 method schema 都携带由 runtime registry 派生的 `x-plurora-contract` metadata。生成器会：

- 为每个 wire ID 生成唯一的 TypeScript 与 Rust 方法身份；
- 拒绝重复 wire ID、生成函数名与 OpenAPI operation ID；
- 保持 request/result 类型与 JSON Schema 同步；
- 仅在 transport 能携带 contract selection 时生成协商客户端。

使用仓库生成脚本统一更新 schema、OpenAPI 与两个 SDK：

```sh
scripts/regen-sdks.sh
```

生成物需要审阅，但不得手工修改；干净重生成必须具有确定性。

## 变更纪律

当前 v1 边界内只允许兼容的 additive 演进。删除或重命名方法、改变 requiredness、改变字段含义，都需要新的显式版本边界。预发布破坏性重置必须作为一次协调一致的仓库变更完成；完成后的工作树只保留被选定的一套身份。
