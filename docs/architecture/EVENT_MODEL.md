# 事件模型

> [English](./EVENT_MODEL.en.md) · [中文](./EVENT_MODEL.md)

公开契约的 journal 保存需要持久顺序、审计和因果关系的事实。它按 Context 作用域组织、只追加、持久且有序。大型或可移植内容应通过 ObjectStore 与 ArtifactDescriptor 引用，而不是复制进每条事件。

Runtime 不解释领域 payload。被采用的 Protocol 定义共享语义，Component 与 Product 拥有具体状态。

## Envelope

每条持久事件都使用同一个 `EventEnvelope`：

```text
id                  唯一事件 id
session_id          目标 context
sequence            context 内单调递增
timestamp           runtime 分配
writer_package_id   runtime 或 Package writer 身份
kind                按 owner 分层的事件 kind
schema_version      payload schema 版本
payload             opaque JSON
metadata            用于因果、关联、trace 与 hint 的 opaque JSON
```

Runtime 分配 `id`、`sequence`、`timestamp` 与最终 writer 身份。Package principal 不能自行声明另一个 writer。

## 平台拥有的事件 kind

58 个平台事件由显式 registry 定义，不依赖魔法字符串前缀。它们使用语义 owner namespace，例如：

```text
context/opened
host/package.loading
capability/stream.started
authority/grant.created
object/put
projection/updated
change/proposal.applied
runtime/error
```

只有 writer `plurora/runtime` 可以追加 registry 中的平台事件。完整列表与 payload schema 见 [`../spec/v1/EVENT_KIND_REGISTRY.md`](../spec/v1/EVENT_KIND_REGISTRY.md)。

显式 registry 很重要，因为平台事件横跨 Substrate、Host、Protocol 与 runtime 职责。用单一保留前缀反而会掩盖 owner。

## Package 拥有的事件 kind

Package 事件 kind 必须以精确 Package ID 加 `/` 开头：

```text
someorg/conversation/turn.started
someorg/world-sim/tick.completed
someorg/memory-pack/proposal.created
```

Package 不能写入另一个 Package 的 kind，也不能冒充 registry 中的平台事件。跨 Package 协作应使用公开 capability、被采用的 Protocol 或 extension point，而不是伪造 writer。

## 校验与 authority

除 writer 为 `plurora/runtime` 外，追加事件要求 writer Package Manifest 声明 `events.append`。当 Package 为某个事件 kind 声明 payload schema 时，runtime 会在持久化前使用支持的 JSON Schema 子集校验 payload。

读取 journal 需要对应公开方法的 authority，并可按 Context、sequence range、writer 或 kind prefix 限定。

## 持久化规则

- 只追加：已持久事件不被编辑。
- 单调顺序：sequence 在单个 Context 内单调递增；不承诺跨 Context 全序。
- 持久：只有 EventStore 提交 envelope 后 append 才成功。
- 可重放：消费者可从 sequence cursor 读取并重建自己的 projection。
- 默认 opaque：runtime 拥有 envelope 完整性，不拥有领域解释。

## Replay 与 projection

Replay 服务于新连接客户端、重建中的 Component、审计工具与 projection materializer。Runtime 原样返回持久 envelope；Protocol 或 Component 解释 payload 并负责迁移。

每个事件 kind 都携带 `schema_version`。语义 owner 负责版本演进，EventStore 保留当时写入的事实。

## 因果与关联

`metadata` 可以携带 `causation_id`、`correlation_id`、trace id 或其他 owner-defined 字段。Runtime 将其视为 opaque，不从中推断领域语义。

## 刻意省略

平台事件 registry 不把聊天历史、turn、prompt、model call、memory、world 或 agent task 定义为通用本体。Protocol 或 Package 可以在自己的 owner namespace 下定义这些事实，而无需把它们提升为宪法基底职责。
