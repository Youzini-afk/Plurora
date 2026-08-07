# Host 远程访问与路由暴露

> [English](./HOST_REMOTE_ACCESS.en.md) · [中文](./HOST_REMOTE_ACCESS.md)

Plurora 的 Web/PWA、桌面和 CLI 都是同一个 Host 的客户端。远程访问不会建立第二套写入接口，也不会把 root token 复制到手机；它在现有 Host API / RPC 前增加可撤销、可过期、同时按动作和结构化 project/target 资源衰减的设备身份。应用数据面的公开访问则是另一条显式边界，不能由“配置了域名”隐式开启。

## 两个平面

```mermaid
flowchart LR
  D["Desktop / root operator"] -->|"root credential"| C["Host control plane"]
  M["Mobile PWA / paired device"] -->|"scoped device cookie"| C
  C --> A["RPC + Host API + authenticated /p routes"]
  V["Public visitor"] -->|"public vhost only"| P["Explicitly public app route"]
  P --> L["Active loopback port lease"]
  A --> L
```

- **Host 控制平面：** 项目、部署、ChangeSet、访问授权和 `/p/<route_id>/...`。需要 root 或设备身份，并按动作 scope 检查。
- **应用数据平面：** 只有 `route_access: public` 的 route 才能经 `<slug>.<app_base_domain>` 免 Host 认证访问。
- Web 静态文件和 `/pair` 页面本身不构成权限；真正的读取和修改仍在受保护 API 后面。公开 pairing API 只有持有一次性高熵 token 的调用者才能 inspect / claim。

## 身份

| 身份 | 凭据 | 用途 |
|---|---|---|
| Host root | `PLURORA_HTTP_ACCESS_TOKEN` / `--access-token` 的 Bearer token；桌面可通过一次性 bootstrap 换取 root cookie | 本机管理、首次授权、紧急恢复；拥有全部 scope |
| Paired device | `yggaccess.*` token；PWA claim 后只放入 `__Host-plurora_remote_session` Cookie | 日常远程控制；仅拥有 grant 中列出的 scope 与 project/target selector |

未配置 root token 时的可选认证只用于 loopback 开发。`host serve` 绑定非 loopback 地址时会拒绝没有非空 root token 的启动。root token 是根凭据，不应进入 pairing URL、浏览器持久存储、应用上游或日志。

## Scope

| Scope | 能力边界 |
|---|---|
| `observe` | 读取 Host / 项目 / package / target / exec / port / proxy 状态，以及访问受 Host 认证的 `/p` route |
| `project_operate` | 启动、停止项目和管理项目 session |
| `deploy` | 部署、取消部署任务，以及 target / exec / port / proxy 变更 |
| `develop_propose` | 读取开发 ChangeSet 并草拟新 ChangeSet |
| `develop_approve` | 批准或拒绝精确 ChangeSet |
| `develop_execute` | 执行或恢复已批准 ChangeSet |
| `access_manage` | 查看设备和邀请、创建/取消 pairing、撤销 grant |

未知 HTTP 路径、未知 RPC 方法和宽泛管理变更默认需要 `access_manage`，因此 scoped device 会 fail closed。新 grant 只能是调用者权限的子集，必须包含 `observe`；只有 root 可以把 `access_manage` 授予新设备。Web UI 默认只选择 `observe`，额外动作必须逐项开启。

## Project 与 target 资源

grant 的 `resources` 使用结构化 selector：`{ kind: "project" | "target", id?: string }`。省略 `id` 表示该 kind 的全部资源；带 `id` 时只做结构化精确比较，不做字符串前缀匹配。旧 journal 中没有 `resources` 的 grant 会按兼容语义恢复为 all-projects + all-targets；新 Web/CLI pairing 会显式提交 selector。

HTTP project 路径与静态 project bundle、canonical/legacy RPC、项目列表、session/event、ChangeSet、部署 job/revision、私有 `/p` route 和 target/exec/port/proxy 对象都在服务端检查或过滤。设备身份不会在 `/rpc` 被折叠为 `HostDev`。项目 session 由 Host 写入并验证 `metadata.project_id`；fork 保留该绑定，调用方提交的 `session_id` 本身不构成权威。

子 grant 的 scope、resource、期限都不能超过父 grant；认证会验证完整 delegation chain，撤销或过期任一祖先会使后代立即失效。设备协议 allow/deny 写入 `host_control_authority` journal，记录 grant、delegation、canonical method、action、结构化资源和 correlation，但不记录 token、Cookie 或原始请求参数。

sandbox surface frame 是 opaque origin，不能安全携带 Host Cookie 或 Bearer。`host.surface.bundle.resolve` 先验证 project authority，再把内部 `/surface-bundles/...` 地址换成随机、五分钟、只读且只覆盖该 bundle root 的 `/surface-assets/<lease>/...` 句柄；相对 module/CSS 资源保持在同一 root。句柄绑定设备 grant，撤销/过期会立即失效；原始 bundle 路径继续要求 Host 身份。这个句柄不包含 Host credential，也不能调用 RPC 或读取另一项目。

## Pairing 生命周期

1. 拥有 `access_manage` 的客户端调用 `POST /host/v1/access/pairings`，提交设备名、scope、project/target selector 和期限。
2. Host 返回最多存活 10 分钟的一次性 `yggpair.*` token。Web UI 把它放进用户指定的 HTTPS Host origin 下的 `/pair` URL。
3. 新设备打开链接后立即从地址栏清除 token，只在内存中保留；先调用公开 inspect，让用户核对设备名、scope 和过期时间。
4. 用户确认后调用公开 claim。Host 原子消费 pairing，创建最长 365 天的 grant，并设置 Secure、HttpOnly、SameSite=Strict、host-only Cookie。
5. grant 到期或被撤销后，每次认证都会立即失败；撤销当前设备还会清除其 Cookie。pending pairing 可在领取前取消。

相关路由：

| 认证 | Method / route | 作用 |
|---|---|---|
| public + pairing token | `POST /host/v1/access/pair/inspect` | 在消费前查看邀请内容 |
| public + pairing token | `POST /host/v1/access/pair` | 一次性领取 grant |
| any Host identity | `GET /host/v1/access/me` | 查看当前身份、scope、resource 与 delegation chain |
| `access_manage` | `GET /host/v1/access` | 查看 grant 与 pairing 投影 |
| `access_manage` | `POST /host/v1/access/pairings` | 创建邀请 |
| `access_manage` | `POST .../pairings/:id/cancel` | 取消 pending 邀请 |
| `access_manage` | `POST .../grants/:id/revoke` | 撤销设备 grant |
| `access_manage` | `POST /host/v1/access/grants/revoke` | 原子撤销最多 256 个设备 grant |
| any Host identity | `POST /host/v1/access/logout` | 清除浏览器 Host Cookie |

## 持久化与凭据边界

- pairing 和 grant 转换写入 EventStore 的专用 `host_control_access` journal；SQLite / PostgreSQL Host 重启后重新水化同一投影。
- journal 只保存带 domain separation 的 SHA-256 credential digest，不保存 pairing token、access token 或 Cookie 原值。
- pairing claim、cancel、grant revoke 使用 expected-tail compare-and-append；并发领取只有一个能成功。批量撤销会先校验全部 grant，再以一条有界、已排序的 journal transition 提交，并支持幂等重试。
- grant 的撤销和过期在每次认证时检查，不依赖浏览器主动刷新状态。
- 委派 grant 保存 `parent_grant_id` 与 `delegation_depth`；每次认证沿祖先链 fail closed，父 grant 撤销会级联失效。
- Bearer / Cookie 有明确优先级。查询参数凭据仅允许在 `GET /kernel/v1/event.subscribe/:session_id` 和 `GET /host/v1/build-deploy/:job_id/events` 这两个浏览器 SSE 入口使用；其他路径不会把 URL token 当作凭据。

## CLI 管理

CLI 使用与 Web/PWA 相同的 Host API，不直接读写 grant journal：

CLI 只允许 loopback 使用明文 HTTP；非 loopback Host 必须使用 HTTPS，Host origin 不能包含路径或凭据，并且 Host access 请求不跟随重定向。保存的连接配置只包含显示名称、endpoint 和按 Host 隔离的 project/target context；access token 仍只来自显式参数或环境变量。

```bash
plurora host connection save workshop --endpoint https://host.example.com
plurora host connection context --project my-project__abc12345 --target remote-builder
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" me
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" projects
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" project-status
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" target-status
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" \
  pair --device-name phone --scopes observe,project_operate,deploy \
  --project my-project__abc12345 --target local
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" \
  revoke <grant-id>
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" \
  bulk-revoke <grant-id> <grant-id> ...
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" \
  changes --project my-project__abc12345 list
plurora host access --access-token "$PLURORA_HTTP_ACCESS_TOKEN" \
  changes --project my-project__abc12345 draft --request change.json
```

`changes` 还提供 `get`、`bundle`、`approve`、`reject`、`execute` 与 `recover`；所有命令都使用和 Web/Desktop/PWA 相同的公开 ChangeSet API。`--endpoint` / `PLURORA_HOST_URL` 仍可为单次命令覆盖当前连接；`plurora host connection local` 返回默认 loopback Host。

## HTTPS 与同源要求

远程 PWA 只支持 HTTPS origin。Pairing 页面在明文 HTTP 下拒绝领取，因为 `__Host-` Cookie 必须同时满足 Secure、host-only 和 `Path=/`。

生产拓扑应把 Host 放在 TLS 反向代理或可信 overlay 后：

```bash
PLURORA_HTTP_ACCESS_TOKEN='<high-entropy-root-token>' \
  plurora host serve --http 0.0.0.0:8787 --static-dir clients/web/dist
```

裸 HTTP 端口必须由防火墙限制在代理/overlay 内；外部 origin 例如 `https://host.example.com`。代理必须保留原始 `Host`，并让浏览器的 `Origin` 到达 Host。Cookie pairing 仍保持同源，系统不启用携带凭据的跨源 CORS。共享 Web/PWA/Desktop 客户端可使用按 Host 隔离的 Bearer token 跨源调用用户显式选择的远程 Host；control route 只允许 `GET`/`POST` 与 `Authorization`/`Content-Type`，且绝不启用 credentialed request，proxy 与 raw surface-bundle route 不发出这项 CORS policy。项目 surface 的 sandbox frame 与衰减 asset 从所选 Host 加载，因此需要渲染 surface 的 Host 必须提供匹配版本的 Web 静态 bundle。

## 应用 route 暴露

`kernel.v1.proxy.register` 和部署描述符使用：

```yaml
route_access: host_authenticated # 默认；旧描述符也按此解释
# route_access: public           # 必须由用户显式选择
```

- `host_authenticated`：只提供 `/p/<route_id>/...`；它位于 Host auth middleware 内，至少需要 `observe`。
- `public`：在配置 `--app-base-domain` 后额外启用派生 vhost，且只有该 vhost 绕过 Host 认证。没有 base domain 时仍只有受保护的 `/p` fallback。
- route access 会写进 proxy 注册事件和 durable deployment revision，recover / rollback 保留原选择。
- public vhost 不把 Host `Authorization`、Ygg query token、Host session Cookie 或 `Referer` 转发给应用；upstream 仍必须是 active、ready 的 loopback lease。

公开 route 的应用必须自己承担互联网输入、应用级身份、CSRF、速率限制和内容安全。Plurora 的 Host grant 不是应用用户系统。

## 刻意未提供

- target-edge ingress、应用身份、任意网络 transport 或远端 package transport。远程 deployment 端口仍只绑定 loopback，并且只能通过认证 Target Agent tunnel 到达。
- 多用户项目成员模型、workload 级强沙箱和跨 Host delegation chain；当前 project selector 是单 Host 控制面边界。
- 把 root token 自动同步到手机，或用本地 CLI 绕开 Host API 写入。
- 未经用户确认的部署、公开 route 或自动重放副作用。
- 为被部署应用代做登录、公开 CORS 或互联网边缘防护。
