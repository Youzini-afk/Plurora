# 事件模型

> [English](./EVENT_MODEL.en.md) · [中文](./EVENT_MODEL.md)

当前 Contract V1 事件日志保存需要长期排序、审计和因果关联的事实。它按 Session scope 组织，只追加、持久化，并保持顺序；大对象和可移植内容使用 ObjectStore / ArtifactDescriptor，而不是把所有数据复制进日志。

runtime 不解释事件 payload。共享含义由采用的 Protocol 定义，具体领域状态由 Component 或 Product 拥有；当前 V1 使用 Package writer namespace 表达事件 owner。

## 信封

每个持久化事件都使用同一种信封：

```text
EventEnvelope
- id                  unique event id
- session_id          target session
- sequence            monotonic per session
- timestamp           kernel-assigned
- writer_package_id   the package that produced the event (or "kernel")
- kind                namespaced string, e.g. "context/opened" or "org/name/event/foo"
- schema_version      payload schema version, owned by the writer
- payload             opaque JSON, validated only against the writer's declared schema
- metadata            opaque JSON; causation_id, correlation_id, trace ids, etc.
```

内核：

- 分配 `id`、`sequence`、`timestamp` 和 `writer_package_id`，
- 要求 `kind` 命名空间在写入方的 id 之下（内核事件使用 `kernel/v1/...`），
- 如果写入方声明了 schema，就用该 schema 验证 `payload`，
- 将 `metadata` 视为不透明。

## 种类

事件 kind 分为两类。

### 内核发出的 kind

内核自身只产生一小组固定 kind。它们描述内核操作，不描述内容。

Session：

```text
context/opened
context/closed
context/forked
```

能力包生命周期：

```text
host/package.loading
host/package.starting
host/package.ready
host/package.stopping
host/package.stopped
host/package.loaded
host/package.unloaded
host/package.degraded
host/package.log
```

能力调用（计划中的审计形式）：

```text
capability/invoked
capability/completed
capability/failed
```

权限审计：

```text
authority/grant.created
authority/grant.revoked
authority/denied
```

通用底座：

```text
object/put
projection/updated
```

提案生命周期：

```text
change/proposal.created
change/proposal.approved
change/proposal.rejected
change/proposal.applied
change/proposal.failed
```

传输层 / runtime 错误（计划中）：

```text
runtime/error
```

这些是内核按名称识别的全部事件 kind。它们的 payload 描述内核操作，不描述内容。

### 非核心事件 kind

当前 Contract V1 由 Package writer 在自己的 Manifest 中声明非核心事件 kind，并使用 package id 命名空间。长期语义可以由 Protocol、Component 或 Product 拥有；Package 只是当前分发与 writer 身份边界。示例仅用于说明，不属于基底：

```text
someorg/conversation/turn.started
someorg/conversation/prompt.rendered
someorg/conversation/model.streamed
someorg/world-sim/tick.completed
someorg/memory-pack/proposal.created
```

runtime 持久化并排序这些不透明事件，但不解释其领域语义。

## 权限

追加事件要求写入方清单中有 `events.append`。读取事件流要求 `events.read`，并且可以限定到特定会话。

一个 writer 不能在另一个 owner 的命名空间下追加事件。跨组件或跨协议协调应通过公开调用、协议或扩展点完成，不能在日志中冒充对方。

## 持久化规则

- 只追加。日志从不被编辑。
- 会话内排序是单调的。内核不承诺跨会话排序。
- 持久化。`journal/after_append` 触发后，事件即已提交。
- 可 replay。内核可以从 `sequence` 0 开始向前流式输出事件。

## Replay

内核可以将事件 replay 给：

- 新订阅的客户端，
- 请求追赶的新加载能力包，
- 快照工具。

内核原样 replay 信封。意义、projection 和状态重建由能力包负责。

## 版本管理

每个事件 kind 携带 `schema_version`。所属写入方负责迁移。内核不迁移 payload；它只持久化写入时的内容。

能力包可以在不改动内核的情况下为自己的 kind 发布新的 `schema_version`。

## 因果与关联

信封的 `metadata` 可以携带 `causation_id`（导致此事件的那条事件）和 `correlation_id`（一个逻辑追踪）。内核将它们视为不透明字段。能力包决定它们的含义。

## 本模型刻意省略的东西

- 没有聊天历史概念。
- 没有轮次或消息概念。
- 没有 prompt frame、上下文计划或 model call 概念。
- 没有记忆或世界状态概念。
- 没有 agent 任务或提案概念。

需要这些概念的能力包，可以把它们定义成自己的事件 kind。它们都不是内核事件。

## 稳定性

内核发出的 kind 集合刻意保持很小。新增内核 kind 需要和新增内核职责一样被论证：它确实无法合理地放进能力包。
