# 公开协议

> [English](./PUBLIC_PROTOCOL.en.md) · [中文](./PUBLIC_PROTOCOL.md)

Plurora 通过一份公开契约暴露 Substrate、Host、Protocol 与 Shell Profile 能力。第一方 Web/Desktop、CLI、in-process Component、子进程、未来 WASM Component 与远端服务使用同一套身份、authority 与行为语义。

不存在私有旁路。第一方客户端与第三方使用同一套协议。

每个公开方法只有一个按 owner 分层的 wire ID。第一段声明 Substrate、Host、Protocol 或 Shell 归属；逐项边界见 [`../spec/CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.md)。

## 传输层

所有传输层最终呈现同一份公开行为。当前 Host 先实现可运行子集；其余 transport 只有在身份、authority、错误、取消和 terminal semantics 清楚后才开放。

- In-process：与线上格式一一对应的 Rust API。
- Subprocess：基于 stdio 的 JSON-RPC。当前 host 必须实现。
- HTTP：用于非流式方法的 request/response。当前 host 必须实现。
- Profile-backed HTTP Host：`plurora host serve --http 127.0.0.1:8787 --profile profiles/forge-alpha.yaml` 在 autoload profile Package 后启动 `/rpc` 与公开 Host SSE route。
- Host stdio：用于自动化和 conformance 的 JSON-RPC。当前 host 必须实现。
- WebSocket：用于订阅和流式方法。计划在 sequence-range replay 之后实现。
- TCP：基于本地 socket 的 JSON-RPC。Deferred。
- Remote endpoint：对声明 URL 的 HTTP 和 WebSocket。Deferred。
- WASM Host：通过 runtime-provided Component ABI 进行编组调用。Deferred。

传输层选择是 Host 的职责。状态文档只有在公开传输路径可用、行为检查存在并且没有绕过 runtime authority 时，才把一个方法标记为 implemented。

## 协议信封

规范的 request/response 传输层使用以下格式：

```json
{
  "id": "request-1",
  "method": "capability.invoke",
  "params": {}
}
```

Host 附加 principal 与 transport context。调用者不能通过 request JSON 自行声明 Package/admin 身份。Request 还可以携带可选的 `session_id` 与 `contract` selection；省略 contract 时使用 `plurora.contract.default/v1`。

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
    "code": "runtime/error/permission_denied",
    "message": "...",
    "details": {}
  }
}
```

## 方法格式

每个方法具有：

- `id`：精确的 owner-based 公开方法 ID，例如 `context.open`、`host.installation.list` 或 `shell.contribution.list`。
- `input`：根据已发布 schema 验证的 JSON 值。
- `output`：JSON 值，可能是流。
- `errors`：包含 `code`、`message`、`details` 的结构化错误模型。

## 方法语义在哪里

方法 ID、params/result schema 与 implemented / partial / planned 状态以 [`../spec/PUBLIC_CONTRACT.md`](../spec/PUBLIC_CONTRACT.md) 和 `docs/spec/v1/schemas/` 为准。本文只保留传输、信封，以及必须在传输层解释的细节。

下面几节是 Host 传输相关的补充，不是第二份方法矩阵。

### SSE 订阅

`journal.list` 与 `journal.subscribe` 对 Package principal 要求 `events.read`。当前 Host 将 HTTP SSE 作为 Host-dev 流暴露：

```text
GET /journal/subscribe/:session_id?after_sequence=42&kind_prefix=host/&writer_package_id=plurora/runtime
```

`journal.list` 接受 `session_id`、`after_sequence`、`limit`、`kind_prefix` 和 `writer_package_id`。

### Outbound 传输门闩

出站协议提供三个原语：`host.outbound.execute`（一元 HTTPS）、`host.outbound.stream`（SSE / NDJSON / raw）、`host.outbound.websocket.*`（双向 WSS）。`websocket.open` 是 streaming 方法，返回的 `connection_id` 也是 `stream_id`；`capability.cancel` 走同一条关闭路径。

请求/响应 shape 以运行时类型和协议分发解析为准，不在本文重复完整结构：HTTP/stream 类型见 `crates/plurora-runtime/src/runtime/outbound.rs`，WebSocket 类型见 `crates/plurora-runtime/src/runtime/outbound_websocket.rs`，协议解析见 `crates/plurora-runtime/src/runtime/protocol_dispatch.rs`。核心字段包括 `capability_id`、`destination_host`、`method`、可选 `path`、`body_shape`、`metadata`、`secret_headers`、`static_headers`、`timeout_ms`；`stream` 额外接受 `stream_format`（`sse` / `ndjson` / `raw`）与帧/时长上限；`websocket.open` 接受目标 host/path、可选 subprotocol、headers、`secret_refs` 和连接/帧/字节上限。

Outbound request 通过两层 fail-closed gate：Package Manifest 必须声明匹配的 `permissions.network.declarations`（WebSocket 使用 `WEBSOCKET` method），所有 `secret_headers` / `secret_refs` 也必须声明在 `permissions.secret_refs`。Host profile 必须显式启用对应 primitive；destination 必须精确匹配 allowlist 或 `*.suffix`；HTTP/SSE 要求 HTTPS；WebSocket 默认 WSS；redirect 默认拒绝。`capability_id` 必须属于 caller Package namespace。Subprocess reverse public call 使用 Host-bound Package principal，不能 spoof 其他 Package。

WebSocket 专用事件使用 `host/outbound.websocket.*`：`opened` 记录握手成功和 connection/subprotocol 元数据；`frame` 记录 inbound/outbound、frame kind、字节数和序号，不记录 payload；`error` 记录脱敏错误；`completed` 记录关闭码、原因、帧/字节计数、耗时、executor kind、network_performed、redaction state 与 secret_ref 引用。

所有三种出站原语都有完成审计事件：`host/outbound.execute.completed`、`host/outbound.stream.completed`、`host/outbound.websocket.completed`。这些事件只记录状态、计数、耗时、执行器种类、network_performed、redaction state 和 `secret_ref` 引用；不会记录 raw header/body/secret/frame payload/response。

`host.outbound.audit` 只返回脱敏审计记录：package、capability、destination host、method、purpose、使用的 `secret_ref` 与 redaction state。raw header/body/secret/response 不进入审计或协议响应。

Git 安装不是公开契约 transport。当前 `plurora install <github-url>` 组合 `plurora/git-tools-lab`、`plurora/integrity-lab` 与 `plurora/install-lab` 等普通第一方 Package，并受声明的 network/filesystem authority 约束；它不会向 substrate 增加 Git method。

## 包方法

每个 Package 贡献 Package-owned capability，也可以声明 extension contract。Capability descriptor 通过 `capability.discover` 发现，schema 由 Manifest 与生成 contract artifact 发布；`capability.describe` 与 `protocol.extension.describe` 仍是 reserved planned query。

公开契约不预定义 `session.input`、`prompt_frame.get`、`model.call`、`memory.search` 等内容方法。这些行为属于显式 Protocol 或 Package-owned capability。

## 错误

```text
runtime/error/internal
runtime/error/invalid_request
runtime/error/permission_denied
runtime/error/not_found
runtime/error/ambiguous_route
runtime/error/schema_invalid
runtime/error/package_state
protocol/error/unsupported_contract
```

Provider failure 按 method schema 进入 capability result 或统一 `ProtocolError` envelope。稳定 string identifier 与 JSON-RPC numeric equivalent 见 [`../spec/v1/ERROR_CODES.md`](../spec/v1/ERROR_CODES.md)。

## Streaming

流式输出通过 WebSocket 或等效传输层进行。流携带类型化帧，其 schema 随方法发布。

对于 `journal.subscribe`，帧是事件 envelope 加上用于恢复的 `cursor`。

对于 `capability.stream`，帧是 provider 定义的块加上终端状态帧。

## 认证和 principal

Host 在 transport layer 强制认证。每个 connection 关联 user、assistant、Package、Host tool、anonymous caller 或 remote system principal。Runtime 在每次 operation 中根据 Host-established principal 检查 authority。

Substrate 不规定 identity provider；Host 负责接入并把 authenticated identity 映射为公开 principal。

当前 v1 principal class：

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
authority.grant.create
authority.grant.revoke
authority.grant.list
authority.decision.list
```

## Surface 贡献

Package 可以在 Manifest 中声明 UI surface descriptor。Runtime 不渲染或解释这些 descriptor 的内容，而是通过 Shell Profile method 把它们暴露给公开 client：

```text
shell.contribution.list
shell.contribution.describe
```

当前 slot 为 `experience_entry`、`home_card`、`quick_action`、`workshop_card`、`play_renderer`、`forge_panel`、`asset_editor` 和 `assistant_action`。

`quick_action`、`workshop_card`，以及带 `metadata.shell_schema_version: 1` 的 `home_card` 是结构化 shell descriptor。它们只允许受限文本、icon hint、排序和同包 target。Web shell 自己渲染这些入口，不加载包 JS、不解析 HTML、不 mount iframe。包贡献的 action 目前是发现入口；未来若要执行，仍必须走公开协议、权限、提案和审计。

Surface descriptor 可以包含 version、launch capability、Context template、input schema、permission UX metadata 与 approval policy。它们始终只是 descriptor；runtime 不会将其转化为内建体验/游戏语义。

## 提案生命周期

Assistant 和包驱动的变更使用通用提案信封，而不是特权变更路径：

```text
change.proposal.create
change.proposal.get
change.proposal.list
change.proposal.approve
change.proposal.reject
change.proposal.apply
```

Proposal status 为 `created`、`approved`、`rejected`、`applied` 与 `failed`。初始 operation 保持通用，例如 `object.put` 与 `projection.rebuild`。Operation 会产生 registry 中的 Change/Object/Projection event 与 durable effect evidence。

## 版本控制

Request envelope 可携带显式 `contract` selection。`host.info` 发布受支持 registry、profile、layer version、method 与 Protocol Commons descriptor。当前 Host 支持精确 v1 boundary，对 unsupported selection fail closed。

Method schema 在 v1 内只做 additive evolution。Breaking change 需要新的显式 contract/profile/layer version boundary、migration tooling、较早数据可读性与 conformance vector，不能隐藏在 alias 中。

## 稳定性

`session.input`、`prompt_frame.get`、`model.call` 等内容方法不属于宪法基底。没有被采用的 Protocol 与明确 owner，就把它们加入通用公开契约 ontology，属于 Charter 违规。
