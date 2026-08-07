# 公开协议 v0

> [English](./PROTOCOL_V0.en.md) · [中文](./PROTOCOL_V0.md)

Yggdrasil 当前通过一份公开合同暴露 substrate、Host、Protocol 和 Shell Profile 能力。官方 Web/Desktop、CLI、in-process Component、子进程、未来 WASM Component 和远端服务使用同一套身份、authority 与行为语义。

不存在私有旁路。官方客户端使用这份协议；第三方也使用这份协议。

本文保留 `kernel.v1.*` 兼容名称来描述当前 wire surface；这些名称不自动决定长期架构归属。逐项 owner 见 [`../spec/CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.md)。

## 传输层

所有传输层最终呈现同一份公开行为。当前 Host 先实现可运行子集；其余 transport 只有在身份、authority、错误、取消和 terminal semantics 清楚后才开放。

- In-process：与线上格式一一对应的 Rust API。
- Subprocess：基于 stdio 的 JSON-RPC。当前 host 必须实现。
- HTTP：用于非流式方法的 request/response。当前 host 必须实现。
- Profile 支持的 HTTP host：`ygg host serve --http 127.0.0.1:8787 --profile profiles/forge-alpha.yaml` 在自动加载 profile 包之后启动 `/rpc` 以及 ad hoc SSE 路由。
- Host stdio：用于自动化和 conformance 的 JSON-RPC。当前 host 必须实现。
- WebSocket：用于订阅和流式方法。计划在 sequence-range replay 之后实现。
- TCP：基于本地 socket 的 JSON-RPC。Deferred。
- Remote endpoint：对声明 URL 的 HTTP 和 WebSocket。Deferred。
- WASM host：通过内核提供的 ABI 进行编组调用。Deferred。

传输层选择是 Host 的职责。状态文档只有在公开传输路径可用、行为检查存在并且没有绕过 runtime authority 时，才把一个方法标记为 implemented。

## 协议信封

规范的 request/response 传输层使用以下格式：

```json
{
  "id": "request-1",
  "method": "kernel.v1.capability.invoke",
  "params": {}
}
```

host 附加身份和传输层上下文。调用者不能通过请求 JSON 自行声明 package/admin 身份。

成功：

```json
{
  "id": "request-1",
  "result": {}
}
```

失败：

```json
{
  "id": "request-1",
  "error": {
    "code": "kernel/v1/error/permission_denied",
    "message": "...",
    "details": {}
  }
}
```

## 方法格式

每个方法具有：

- `id`：内核方法在 `kernel/v1/...` 下命名，包方法在 package id 下命名。
- `input`：根据已发布 schema 验证的 JSON 值。
- `output`：JSON 值，可能是流。
- `errors`：包含 `code`、`message`、`details` 的结构化错误模型。

## 内核方法

内核暴露一个最小集合。未列出的任何内容都归某个包所有。

### Session

```text
kernel.v1.session.open      open a session with labels and a package set
kernel.v1.session.close     close a session
kernel.v1.session.fork      fork a session at an event sequence
kernel.v1.session.branch.list list branch lineage records
kernel.v1.session.get       get session metadata
kernel.v1.session.list      list sessions visible to the caller
```

内核不存储任何内容层面的会话状态。标签和包集是仅有的主观判断。

### 事件

```text
kernel.v1.event.append      append an event under the caller's namespace
kernel.v1.event.list        list events for a session by sequence range
kernel.v1.event.subscribe   stream events as they are appended (resumable)
```

`event.append` 要求调用者清单中包含 `events.append`。`event.list` 和 `event.subscribe` 对 package 身份要求 `events.read`。当前 host 将 HTTP SSE 作为 host-dev 流暴露：

```text
GET /kernel/v1/event.subscribe/:session_id?after_sequence=42&kind_prefix=kernel/v1/&writer_package_id=kernel
```

`kernel.v1.event.list` 接受 `session_id`、`after_sequence`、`limit`、`kind_prefix` 和 `writer_package_id`。

### 包

```text
kernel.v1.package.list      list packages visible in the host
kernel.v1.package.describe  fetch a manifest snapshot
kernel.v1.package.load      load a package from a manifest reference
kernel.v1.package.unload    stop and remove a package
kernel.v1.package.status    current state and health
kernel.v1.package.restart   restart a package when its entry form supports restart
kernel.v1.package.logs      read captured package logs
```

加载包可能受 host 策略限制。

### Capability

```text
kernel.v1.capability.discover    enumerate capabilities, optionally filtered
kernel.v1.capability.describe    fetch input/output schemas and metadata
kernel.v1.capability.invoke      invoke a capability with input
kernel.v1.capability.stream      invoke a capability that streams
kernel.v1.capability.cancel      cancel an in-flight invocation
```

`invoke` 通过 id、可选的 `provider_package_id`、可选版本约束，以及最终的会话包集解析到 provider。如果多个 provider 匹配且调用者未指定 `provider_package_id`，内核返回 ambiguous-route 错误。当前 host 支持精确版本或同主版本 `^x.y` 约束。

### 扩展点和钩子

```text
kernel.v1.extension_point.list        list live extension points
kernel.v1.extension_point.describe    fetch payload schema and timing
kernel.v1.hook.list                   list subscribers to a point
```

内核不暴露在运行时注入钩子的方法。订阅在清单中声明。运行时注册只允许通过包生命周期进行。

### Asset

```text
kernel.v1.asset.put         store an asset blob under the caller's namespace
kernel.v1.asset.get         fetch an asset by id
kernel.v1.asset.list        list assets visible to the caller
```

内核记录 `mime`、`hash`、`size` 和 `origin_package`。它不解析或解释资产内容。

### Projection

```text
kernel.v1.projection.register  register a generic projection definition
kernel.v1.projection.rebuild   rebuild projection state from event filters
kernel.v1.projection.get       fetch projection state
kernel.v1.projection.list      list projection records
```

当前 runtime 管理 projection 记录和 rebuild 生命周期，但不解释领域状态语义。Projection 的共享合同属于可选 Protocol，具体 materializer 由 Component 实现；Contract V1 通过 Package writer 注册和分发它们。

### 健康与身份

```text
kernel.v1.host.info         host version, kernel ABI, transports
kernel.v1.host.principal    the calling principal (user, package, remote)
kernel.v1.host.ping         liveness
kernel.v1.host.diagnostics  local host diagnostics for package/capability/hook observability
```

### Outbound

```text
kernel.v1.outbound.execute    unary HTTP-style outbound through the host executor
kernel.v1.outbound.stream     streaming outbound through SSE / NDJSON / raw frames
kernel.v1.outbound.websocket.open   open an outbound WebSocket stream and return connection_id
kernel.v1.outbound.websocket.send   send one outbound WebSocket frame
kernel.v1.outbound.websocket.close  close an outbound WebSocket connection
kernel.v1.outbound.audit      list redacted outbound audit records for a package
```

出站协议提供三个出站原语：`execute` 是一元 HTTP-style 请求，`stream` 是 SSE / NDJSON / raw 单向流，`kernel.v1.outbound.websocket.*` 是双向 WebSocket。`websocket.open` 是 streaming 方法，建立 WSS 连接并返回 `connection_id`；`websocket.send` 和 `websocket.close` 是 unary 方法。`connection_id` 也是 `stream_id`，调用 `kernel.v1.capability.cancel` 并传入该 id 会走同一条取消/关闭路径。

请求/响应 shape 以运行时类型和协议分发解析为准，不在本文重复完整结构：HTTP/stream 类型见 `crates/ygg-runtime/src/runtime/outbound.rs`，WebSocket 类型见 `crates/ygg-runtime/src/runtime/outbound_websocket.rs`，协议解析见 `crates/ygg-runtime/src/runtime/protocol_dispatch.rs`。核心字段包括 `capability_id`、`destination_host`、`method`、可选 `path`、`body_shape`、`metadata`、`secret_headers`、`static_headers`、`timeout_ms`；`stream` 额外接受 `stream_format`（`sse` / `ndjson` / `raw`）与帧/时长上限；`websocket.open` 接受目标 host/path、可选 subprotocol、headers、`secret_refs` 和连接/帧/字节上限。

出站请求按两层 fail-closed 校验：能力包 manifest 必须声明匹配的 `permissions.network.declarations`（WebSocket 使用 `WEBSOCKET` method），并且所有 `secret_headers` / `secret_refs` 必须声明在 `permissions.secret_refs`。host profile 还必须显式启用对应的 outbound primitive，目标 host 必须精确匹配 allowlist（支持 `*.suffix`），HTTP/SSE 使用 HTTPS-only，WebSocket 默认强制 WSS-only，redirect 默认拒绝。`capability_id` 必须属于调用包 namespace；subprocess reverse kernel calls 也使用 host 绑定的 package principal，不能 spoof。

WebSocket 专用事件使用 `kernel/v1/outbound.websocket.*`：`opened` 记录握手成功和 connection/subprotocol 元数据；`frame` 记录 inbound/outbound、frame kind、字节数和序号，不记录 payload；`error` 记录脱敏错误；`completed` 记录关闭码、原因、帧/字节计数、耗时、executor kind、network_performed、redaction state 与 secret_ref 引用。

所有三种出站原语都有完成审计事件：`kernel/v1/outbound.execute.completed`、`kernel/v1/outbound.stream.completed`、`kernel/v1/outbound.websocket.completed`。这些事件只记录状态、计数、耗时、执行器种类、network_performed、redaction state 和 `secret_ref` 引用；不会记录 raw header/body/secret/frame payload/response。

`kernel.v1.outbound.audit` 只返回脱敏审计记录：package、capability、destination host、method、purpose、使用的 `secret_ref` 与 redaction state。raw header/body/secret/response 不进入审计或协议响应。

Git 安装不属于内核传输。未来的 `yg install <github-url>` 会作为普通能力包能力实现，走 `kernel.v1.outbound.execute` 与文件系统写权限，而不是新增内核 git fetch 方法。

## 包方法

每个包通过能力注册和扩展点声明贡献自己的协议方法。它们的 schema 可以通过 `kernel.v1.capability.describe` 和 `kernel.v1.extension_point.describe` 发现。

内核不预定义 `session.input`、`prompt_frame.get`、`model.call`、`memory.search` 等方法。如果它们存在，它们属于特定的包。

## 错误

```text
kernel/v1/error/transport
kernel/v1/error/schema_validation
kernel/v1/error/manifest
kernel/v1/error/permission_denied
kernel/v1/error/ambiguous_route
kernel/v1/error/not_found
kernel/v1/error/timeout
kernel/v1/error/cancelled
kernel/v1/error/capacity
kernel/v1/error/package_state
```

包错误作为 `package_error` 携带 provider 定义的详情，在 `capability.invoke` 响应中传递。

## Streaming

流式输出通过 WebSocket 或等效传输层进行。流携带类型化帧，其 schema 随方法发布。

对于 `event.subscribe`，帧是事件信封加上用于恢复的 `cursor`。

对于 `capability.stream`，帧是 provider 定义的块加上终端状态帧。

## 认证和 principal

host 在传输层强制执行认证。每个连接关联一个身份：用户、assistant、包、host 工具、匿名调用者或远端系统。内核在每次操作时根据身份检查权限。

内核不附带 identity provider。Host 自行接入。

当前身份种类：

```text
host_admin
host_dev
package { package_id }
human { user_id }
assistant { assistant_id, delegated_user_id? }
anonymous
```

Human 和 assistant 身份对敏感操作需要显式的有范围授权：

```text
kernel.v1.permission.grant
kernel.v1.permission.revoke
kernel.v1.permission.list
kernel.v1.permission.audit
```

## Surface 贡献

包可以在其清单中声明 UI surface 描述符。内核不渲染或解释这些描述符的内容；它只将它们暴露给公开客户端：

```text
kernel.v1.surface.contribution.list
kernel.v1.surface.contribution.describe
```

当前 slot 为 `experience_entry`、`home_card`、`quick_action`、`workshop_card`、`play_renderer`、`forge_panel`、`asset_editor` 和 `assistant_action`。

`quick_action`、`workshop_card`，以及带 `metadata.shell_schema_version: 1` 的 `home_card` 是结构化 shell descriptor。它们只允许受限文本、icon hint、排序和同包 target。Web shell 自己渲染这些入口，不加载包 JS、不解析 HTML、不 mount iframe。包贡献的 action 目前是发现入口；未来若要执行，仍必须走公开协议、权限、提案和审计。

Surface 描述符可以包含版本、启动能力、会话模板、input schema、权限 UX 元数据和 approval 策略。这些始终只是描述符；内核不会将它们转化为内建的体验/游戏语义。

## 提案生命周期

Assistant 和包驱动的变更使用通用提案信封，而不是特权变更路径：

```text
kernel.v1.proposal.create
kernel.v1.proposal.get
kernel.v1.proposal.list
kernel.v1.proposal.approve
kernel.v1.proposal.reject
kernel.v1.proposal.apply
```

提案状态为 `created`、`approved`、`rejected`、`applied` 和 `failed`。初始操作支持刻意保持通用，例如 `asset.put` 和 `projection.rebuild`。它们必须产生内核审计/提案事件。

## 版本控制

协议携带 `protocol_version`。内核按版本发布 schema 集。破坏性变更需要新版本；内核可以同时服务多个版本。

方法 schema 可以在一个版本内以向后兼容的方式演进（增量字段）。破坏性方法变更需要新的方法 id。

## 稳定性

任何类似 `session.input`、`prompt_frame.get`、`model.call` 或其他内容方法的东西，永远不在内核协议的范围内。向内核添加此类方法是 charter 违规。
