# 真实模型端到端调用

> [English](./REAL_MODEL_END_TO_END.en.md) · [中文](./REAL_MODEL_END_TO_END.md)

从用户在 YdlTavern surface 点 Send，到真实 OpenAI/Anthropic/Gemini API 响应回到屏幕，整条链路都走公开协议、项目 session、能力包权限、`secret_ref` 解析和 host outbound executor。

这不是内核里的“模型 API”。模型语义属于 YdlTavern engine 包；内核只提供会话、权限、能力调用、出站执行和审计边界。

## 完整调用链

```text
用户在 YdlTavern surface 输入消息 + 点 Send
  ↓
SendForm.onSend(text)
  ↓
TavernShell → TavernProvider.sendMessage(text)
  ↓ (本地: 添加 user message 到 chat state)
  ↓
invokeCapability("ydltavern/engine/model.live_call", { ... })
  ↓ (postMessage 到 surface-host iframe parent)
  ↓
clients/web 主线程接收 RPC, 调用 client.invokeWithSession(method, params, sessionId)
  ↓ (HTTP POST /rpc 带 session_id)
  ↓
plurora host serve 路由到 dispatch_capability_invoke
  ↓ (设置 ProtocolContext.session_id, 设置 ProtocolContext.principal=Package)
  ↓
inproc dispatcher 找到 ydltavern-engine 包 (subprocess)
  ↓ (subprocess JSON-RPC 调用)
  ↓
ydltavern-engine 包执行 capability handler
  ↓ (构造 OpenAI/Anthropic/Gemini-shaped 请求)
  ↓
反向调用 host.outbound.execute, 带 secret_headers: {Authorization: secret_ref}
  ↓
host dispatch_outbound_execute 处理:
  ✓ 检查包的 permissions.network.declarations 中是否允许这个 host
  ✓ 检查包的 permissions.secret_refs 中是否声明了这个 ref
  ✓ Runtime::resolve_secret_ref_with_session 解析 ref → 真值
    ├─ 走 CompositeSecretResolver
    ├─ secret_ref:store:* → StoreSecretResolver → 解密 ~/.plurora/secrets.dat
    ├─ secret_ref:project:* → ProjectStoreSecretResolver
    │   ├─ 通过 ACTIVE_PROJECT_SCOPE task-local 查 session.metadata.project_id
    │   ├─ 解密 ~/.plurora/projects/<id>/secrets.dat
    │   ├─ 缺失 + fallback_to_platform 默认 true → 退到 store
    │   └─ 缺失 + fallback 关闭 → fail-closed
    └─ secret_ref:env:* → EnvSecretResolver (allowlist)
  ↓
LiveHttpOutboundExecutor 拼 HTTPS 请求, 注入 header
  ↓
真实 HTTPS 调到 api.openai.com / api.anthropic.com / etc.
  ↓
响应流回 ↑↑↑ 反向链路, 最终 surface 收到字符串
  ↓
TavernProvider.sendMessage 用 extractContentFromResult 解析成文本
  ↓
更新 assistant message 内容 → React 重渲染 → 用户看到回复
```

## 流式调用链 (`settings.streaming = true`)

非流式版本一次返回完整响应；流式版本通过 kernel 事件流增量推送 chunk 到 surface。

```text
SendForm onSend(text)
  ↓
TavernProvider.sendMessage(text) (settings.streaming === true)
  ↓
streamCapability("ydltavern/engine/model.live_call.stream", { ... })
  ├─ 第一步: callHostRpc("capability.stream", { capability_id, input })
  │           → 返回 stream_id
  ├─ 第二步: postMessage 到 host: { type: "stream.subscribe", id, stream_id, session_id }
  └─ 返回 StreamHandle { streamId, frames: AsyncIterable<StreamFrame>, cancel() }
  ↓
host (surface-host.ts) 收到 stream.subscribe:
  ✓ 通过 hostBridge.subscribeEvents(session_id, callback) 订阅 SSE
  ✓ 过滤 capability/stream.* 事件, 匹配 stream_id 的 payload
  ✓ 转发为 postMessage { type: "stream.frame" / "stream.ended" / "stream.error" }
  ↓
engine 在 subprocess 内部:
  ✓ 反向调用 host.outbound.stream (而非 .execute)
  ✓ 解析 SSE / chunked JSON
  ✓ 归一化为 { delta_text, kind: "chunk" } 等帧
  ✓ 通过 kernel 把帧写入 session 事件流 (capability/stream.chunk)
  ↓
host SSE 把这些事件推到 surface-host 的 subscribeEvents callback
  ↓
surface-host 转换为 postMessage 给 iframe
  ↓
TavernProvider 的 for-await 循环消费 frames:
  - "started" / "progress": 忽略
  - "chunk": extractStreamChunkDelta(frame.payload) → 累加到 assistant message
  - "ended" / "final": 标记 streaming: false, 退出循环
  - "error" / "cancelled" / "timeout": 显示部分内容或错误
  ↓
React 重渲染, 用户看到逐字流式输出
```

## 取消生成

用户点 Stop 按钮：

```text
SendForm "Stop" 按钮 onClick
  ↓
tavern.cancelGeneration()
  ↓
activeStreamRef.current.cancel()
  ├─ callHostRpc("capability.cancel", { stream_id })
  ├─ postMessage { type: "stream.unsubscribe", subscription_id }
  └─ 关闭 AsyncQueue, 移除事件监听器
  ↓
host:
  ✓ capability.cancel 取消 engine 反向调用 (engine 收到 abort 信号)
  ✓ capability/stream.cancelled 事件落入 session
  ✓ surface-host 看到 cancelled, 转发为 stream.error 给 iframe
  ↓
TavernProvider 循环退出, 保留已累积内容, isGenerating: false
```

## 同时只一个 active 生成

当前实现：`isGenerating` 为 true 时，`sendMessage` 直接 return。用户必须先 Stop 或等当前完成才能发新消息。

未来可能队列化，但不在 v1 范围。

## 配置真实调用（用户视角）

安装 YdlTavern 项目，启动 Plurora host 与 Web shell：

```bash
# 1. 安装本地原生项目到测试 data dir/profile
plurora install ../YdlTavern --data-dir <data-dir> --profile <profile> -y

# 2. 启动 host，加载安装后 profile
plurora host serve --profile <data-dir>/profiles/<profile>.yaml --http 127.0.0.1:8787 &

# 3. 启动 clients/web
npm run dev --prefix clients/web

# 4. 浏览器打开 http://localhost:1420
```

然后在 UI 内：

1. Home 屏幕显示已安装的 YdlTavern 项目卡片。
2. 点 Play。
3. 项目状态变成 Running。
4. host 创建项目 session。
5. `host.surface.bundle.resolve` 验证项目后返回短期 `/surface-assets/<lease>/projects/<project_id>/bundle.mjs`，surface bundle 被挂载到 iframe；内部原始 `/surface-bundles/...` 路径不交给 sandbox。
6. 打开 API Connections 抽屉。
7. 选 OpenAI / Anthropic / Gemini provider。
8. 粘贴 API key。
9. 选择保存范围：Platform-wide（默认）或 This project only。
10. 点 Save。
11. 关闭抽屉。
12. 输入消息。
13. 点 Send。
14. 真实 provider 响应应该回到聊天窗口。

Platform-wide 会保存成 `secret_ref:store:*`。This project only 会保存成 `secret_ref:project:*`，并只对当前项目优先可见。

## 配置真实调用（开发者视角）

host profile 必须显式打开 resolver 与 live outbound。示例：

```yaml
# profiles/forge-with-live-models.yaml
secret_resolver:
  store_enabled: true              # secret_ref:store:* / project:* 解析
  env_allowlist:                   # secret_ref:env:* 解析白名单
    - OPENAI_API_KEY
    - ANTHROPIC_API_KEY
    - GEMINI_API_KEY

outbound:
  execute:
    enabled: true
    https_only: true
    executor: live                 # 真实 HTTPS
    allowed_hosts:
      - api.openai.com
      - api.anthropic.com
      - generativelanguage.googleapis.com
      # OpenRouter / DeepSeek / xAI / Fireworks 等按需添加

surface_dev_paths:
  ydltavern: ../YdlTavern/packages/ydltavern-surface/dist
```

`surface_dev_paths` 只用于开发期直接挂载本地构建产物。安装后的项目路径不需要该设置；host 会从 project dist 提供 `/surface-bundles/projects/<project_id>/...`。Web dev server 的端口是 `localhost:1420`。CLI 的 `plurora project start/status/stop` 是项目状态命令；Home 的 Play/session flow 通过 Web public protocol 调用 `host.project.start`、接收 `session_id`，再解析并挂载 surface，二者不要当作等价入口。

三道门必须同时通过：

1. profile 允许 outbound executor 真实出网；
2. engine 包 manifest 声明目标 host；
3. engine 包 manifest 声明要使用的 `secret_ref`。

任何一步缺失都会 fail-closed。默认 conformance 不联网。

## 三种 `secret_ref` 的语义

| Ref shape | 解析路径 | 适用场景 |
|---|---|---|
| `secret_ref:env:NAME` | `EnvSecretResolver` (allowlist) | 开发 / CI / Docker |
| `secret_ref:store:NAME` | `StoreSecretResolver` (本地加密) | 桌面端用户、平台共享 |
| `secret_ref:project:NAME` | `ProjectStoreSecretResolver` | 项目隔离，可按 policy 回退平台 |

详见 [`SECRET_MANAGEMENT.md`](SECRET_MANAGEMENT.md)。

## session_id 是怎么来的

每个项目运行时有一个 kernel session，在 `project.start` 时由宿主创建：

```text
session.id = ksess_xxx
session.metadata.project_id = "youzini-afk__YdlTavern__d2a47e5c"
session.labels = ["project:youzini-afk__YdlTavern__d2a47e5c"]
```

`clients/web` 主线程从 `host.project.start` 响应拿到 `session_id`。随后它调用 `host.surface.bundle.resolve` 拿到 surface bundle URL，并通过 `mountSurface` 把 iframe 挂起来。

iframe 的 `initialProps` 包含：

```json
{
  "projectId": "youzini-afk__YdlTavern__d2a47e5c",
  "sessionId": "ksess_xxx"
}
```

surface 内的 `callHostRpc` / `invokeCapability` 会自动带这个 `session_id`。宿主收到带 `session_id` 的 RPC 后，把 `ProtocolContext.session_id` 设置好，并一路传到 outbound dispatch。

在那里，runtime 通过 session metadata 查到 `project_id`，设置 `ACTIVE_PROJECT_SCOPE` task-local，再解析 `secret_ref:project:*`。

详见 [`PROJECT_MODEL.md`](PROJECT_MODEL.md)。

## Project scope 怎么影响密钥

项目 scope 不是由 surface 自己声明的字符串决定，而是由宿主创建的 session 决定：

1. `project.start` 创建或复用项目 session。
2. session metadata 写入 `project_id`。
3. 后续 RPC 带 `session_id`。
4. `dispatch_outbound_execute` 从 `ProtocolContext.session_id` 找 session。
5. runtime 设置 `ProjectScopeContext`。
6. `ProjectStoreSecretResolver` 读取对应项目 store。

这样 surface 不能通过伪造 `projectId` 读取别的项目 secret。当前仍是软隔离；更强的多租户项目身份进入 `ProtocolContext` 是 planned。

## 权限与审计边界

真实模型调用必须同时通过这些边界：

- `capability.invoke` 检查调用者上下文和 capability handle。
- engine 包 manifest 声明 `ydltavern/engine/model.live_call`。
- engine 包 manifest 声明 `permissions.network.declarations`。
- engine 包 manifest 声明 `permissions.secret_refs`。
- host profile 打开 live executor 并 allowlist 目标 host。
- secret resolver 成功解析引用。

审计记录只保存目标 host、方法、package/capability、redaction 状态、executor kind、`secret_ref` 引用等，不保存 raw API key、prompt body 或 provider response。

## 故障排除

### `no project resolver configured`

host profile 的 `secret_resolver.store_enabled` 为 false，但用户尝试 `secret_ref:project:*`。设为 true，或改用 `secret_ref:env:*`。

### `session has no metadata.project_id`

项目通过 Home Play 的 public protocol start/session flow 启动时会自动设置。CLI `plurora project start` 可用于状态/诊断，但不等同于 Web 的 Play → session → surface mount 流程；如果 surface 走的不是项目流程，需要手动开 session 并设 `metadata.project_id`，或不要使用 project ref。

### `host '...' not in outbound.allowed_hosts`

profile 的 `outbound.execute.allowed_hosts` 漏了这个 provider 的 host。添加后重启 host。

### `secret_ref '...' not declared in package permissions`

engine 包 manifest 的 `permissions.secret_refs` 没声明这个 ref。编辑 manifest 并重新加载包。

### `401 Unauthorized` from provider

通常是 secret store 里的值错了，或 provider 的认证 header 格式变了。重新粘贴 API key，确认 provider/profile 与 key 类型匹配。

### surface 收不到回复

先确认 Play 流程返回了 `session_id`，iframe `initialProps.sessionId` 非空，`callHostRpc` 带 `session_id`，host outbound executor 是 `live` 而不是默认 deny/fake。

## 实现位置

- [`SECRET_MANAGEMENT.md`](SECRET_MANAGEMENT.md) — resolver 链与 project fallback。
- [`PROJECT_MODEL.md`](PROJECT_MODEL.md) — 项目 + session 配对。
- `../YdlTavern/packages/ydltavern-surface/src/app/TavernProvider.tsx::sendMessage`
- `../YdlTavern/packages/ydltavern-engine/src/capabilities/model-live-call.ts`
- `crates/plurora-runtime/src/runtime/protocol_dispatch.rs::dispatch_outbound_execute`
- `crates/plurora-runtime/src/runtime/outbound.rs::LiveHttpOutboundExecutor`
- `clients/web` 的 surface-host iframe bridge 与 `mountSurface`。

## 推迟事项

- 多并发活跃项目：当前 Host 以项目 session 传递 scope；更强的 multi-tenant `project_id` in `ProtocolContext` 仍待补齐。
- 生产级跨源 surface bundle allowlist、origin 校验与对应 CSP 策略。
- 单 chat 多并发生成、token-rate 展示和 Realtime/WebSocket 交互体验。

Desktop 已管理 loopback Host sidecar，并复用同一项目、secret、surface 和出站边界；它不再是本链路的待实现项。
