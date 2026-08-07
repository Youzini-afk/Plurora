# 扩展点

> [English](./EXTENSION_POINTS.en.md) · [中文](./EXTENSION_POINTS.md)

扩展点是当前 Contract V1 runtime 中的具名钩子。核心 runtime 或 Package writer 可以声明它们，普通 Component 可以订阅；runtime 负责受权路由，不解释领域含义。

本文档记录当前兼容合同。长期共享扩展语义应由明确的 Protocol owner 定义，Package 负责分发实现和声明，而不是永久拥有所有扩展本体。

## 钩子契约

每个扩展点包含：

- `id`：带命名空间、不可变的标识。
- `payload_schema`：调用的 JSON 结构。
- `timing`：`sync` 或 `async`。同步处理器会阻塞操作；异步处理器不会。
- `modifiable`：订阅方是否可以返回修改后的 payload，并让下一个订阅方看到修改。
- `short_circuit`：订阅方是否可以否决该操作。
- `ordering`：分发器如何排列订阅方。先按声明的 precedence 排序；相同时使用稳定顺序。

核心 owner 为核心扩展点发布 schema；非核心扩展点由其 Protocol / Component owner 发布 schema，并通过当前 Package Manifest 分发。

## 订阅

订阅方在清单中声明：

```yaml
contributes:
  hooks:
    - extension_point: kernel/v1/event.before_append
      handler: my_handler
      timing: sync
      precedence: 100
```

内核验证订阅方的清单是否声明了钩子隐含的权限。例如，`event.before_append` 要求事件读取权限；修改 payload 需要事件追加权限。

只有在 `short_circuit: true` 时，返回错误的订阅方才会中止该操作。否则错误会被记录，分发继续。

## 取消与超时

同步处理器在操作的 deadline 内运行。异步处理器收到的 deadline 由能力包沙箱策略推导而来。超过 deadline 的处理器会被取消，并被视为一次失败调用。

## 实现状态

当前 `kernel/v1/*` 扩展点集合保持兼容稳定。实现已覆盖事件追加和能力调用的核心路径：稳定排序、Component handler、payload 元数据修改、否决和卸载清理。Session 和 Package 生命周期钩子已在契约中预留。今天它们通过 `kernel/v1/session.*` 和 `kernel/v1/package.*` 事件传递，后续可补齐同步/异步处理。新的共享扩展语义应进入有明确 owner 的 Protocol namespace；实现可以由普通 Component Package 贡献，而不是继续扩大单体 `kernel.v1`。

## 内核发出的扩展点

当前 runtime 只发出少量固定兼容扩展点。新的非核心扩展点由明确 Protocol / Component owner 定义并通过普通 Package 分发。

### 会话生命周期

- `kernel/v1/session.before_open` — sync，modifiable false，short_circuit true。
  打开权限在此执行。订阅方可以否决。
- `kernel/v1/session.after_open` — async。
- `kernel/v1/session.before_close` — sync，modifiable false，short_circuit true。
- `kernel/v1/session.after_close` — async。

Payload：会话 id、请求的 labels、包集、发起请求的身份。

### 事件日志

- `kernel/v1/event.before_append` — sync，modifiable true，short_circuit true。
  权限和 schema 校验在此执行。订阅方可以修改 metadata 或否决。
- `kernel/v1/event.after_append` — async。
  订阅方收到已持久化的信封。

Payload：事件信封。内核不解释 payload 字段。只有写入者清单为该事件 kind 引用了 payload schema 时，内核才检查声明的 schema。

### 能力调用

- `kernel/v1/capability.before_invoke` — sync，modifiable true，short_circuit true。
  权限、路由解析和配额执行在此发生。
- `kernel/v1/capability.after_invoke` — async。
  订阅方收到 input、output（或 error）、延迟和 provider id。
- `kernel/v1/capability.error` — async。
  订阅方收到结构化失败信息。

Payload：invocation envelope。

### 包生命周期

- `kernel/v1/package.loaded` — async。
- `kernel/v1/package.unloaded` — async。
- `kernel/v1/package.degraded` — async。
- `kernel/v1/package.heartbeat_lost` — async。

### 钩子注册表

- `kernel/v1/hook.registered` — async。
- `kernel/v1/hook.unregistered` — async。

这些事件让可观测性能力包发现当前活跃的扩展拓扑。

## 能力包发出的扩展点

能力包可以在 `contributes.extension_points` 下列出自己的扩展点。该能力包就是 schema 的拥有者。

内核路由调用，但不验证语义。如果拥有者能力包被卸载，内核拒绝分发该扩展点，并为所有孤立订阅方发出 `kernel/v1/hook.unregistered`。

示例（仅作示意；不属于内核）：

```yaml
contributes:
  extension_points:
    - id: someorg/conversation/before_step
      payload_schema: ...
      timing: sync
      modifiable: true
      short_circuit: true
```

其他能力包可以订阅：

```yaml
contributes:
  hooks:
    - extension_point: someorg/conversation/before_step
      handler: ...
```

内核不知道 `conversation/before_step` 意味着什么。拥有者能力包知道。

## 发现

客户端可以向内核查询当前活跃的扩展点及其订阅方。Schema 会被暴露。创作者工具、可观测性 dashboard 和其他能力包用这种方式了解当前 host 中有哪些可扩展位置。

## 版本管理

每个扩展点都有一个 `version`。订阅方声明目标版本。若订阅方声明的版本与当前活跃扩展点不兼容，内核拒绝分发。

扩展点的破坏性变更需要新的 id。拥有者能力包可以在过渡期间同时发出两个版本。

## 稳定性

内核发出的扩展点集合在设计上很小。新增内核扩展点需要和新增内核职责一样被论证：它确实无法合理地放进能力包。
