# Extension Point

> [English](./EXTENSION_POINTS.en.md) · [中文](./EXTENSION_POINTS.md)

Extension point 是当前公开契约 runtime 中具名的拦截点。Package Manifest 声明 subscription；runtime 拥有注册、稳定排序、卸载清理，以及该 point 已实现的有界 dispatch 行为。

Extension point 不是私有 API，也不授予 authority。Subscriber 仍是普通 Package participant，必须遵守 Manifest、permission 与 runtime 边界。

## Descriptor 与 subscription 形状

Manifest 可以声明 `ExtensionPointDescriptor`：

- 不可变的 `id` 与 `version`；
- `payload_schema`；
- `timing`（`sync` 或 `async`）；
- `modifiable`；
- `short_circuit`。

`HookSubscription` 声明：

- `extension_point`；
- `handler`；
- `timing`；
- 整数 `precedence`。

Subscription 先按 precedence，再按 subscriber Package ID，最后按 handler 名稳定排序。Package 卸载时，其 subscription 会被移除。

## 当前实际调用的核心 point

当前 runtime 只调用四个 built-in point：

| Point | 当前行为 |
|---|---|
| `journal/before_append` | 持久化前 await；可 veto；当前 dispatcher 可返回修改后的 metadata。 |
| `journal/after_append` | 持久化后 await；接收已存储 envelope；返回值被忽略。 |
| `capability/before_invoke` | provider 解析与执行前 await；可 veto；当前 dispatcher 可返回修改后的 input。 |
| `capability/after_invoke` | 成功调用后 await；接收 invocation result；返回值被忽略。 |

`protocol.extension.list` 返回这四个 ID。`protocol.extension.describe` 已保留，但尚未 dispatch。

## 当前实现边界

Registry、确定性排序、veto 报告、metadata/input 修改路径与卸载清理已实现，并有 runtime/conformance 检查。

任意 Package hook handler 执行、独立 async delivery、每个 handler 的 deadline/quota、failure audit、descriptor 版本协商，以及 Package-declared extension point 的通用 dispatch 尚未完成。Manifest 支持声明，不等于已经拥有完整的通用 extension bus。

## Package-owned extension 语义

Package 可以在自己的 Package ID namespace 下发布 descriptor data。需要互操作的共享语义应由显式 Protocol 拥有。Runtime 不得从 ID 猜测含义，也不得给第一方实现特殊路由。

示例 descriptor：

```yaml
contributes:
  extension_points:
    - id: someorg/conversation/before_step
      version: 1.0.0
      payload_schema: {}
      timing: sync
      modifiable: true
      short_circuit: true
```

在通用 Package-owned dispatch 实现之前，这类声明是可发现的契约数据，不能作为 runtime 已会发出任意调用的证明。

## 稳定性

新增 built-in point 会扩大公开 runtime 边界，因此必须先明确 owner、payload schema、authority model、terminal/error 行为与 conformance proof。产品便利性本身不足以成为理由。
