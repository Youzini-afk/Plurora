# 项目模型

> [English](./PROJECT_MODEL.en.md) · [中文](./PROJECT_MODEL.md)

Project 是当前官方 Host 与发行版用于组织可安装、可运行实例的模型。每个 Project
拥有稳定 ID、入口、状态、数据、secret policy 和生命周期，可以独立运行，也可以与
其他 Project 并存。

Project 很重要，但不是 Plurora 上所有产品必须采用的永久根对象。World、Document、
Service、Workspace、Collection 或其他协议对象可以保持自己的身份；当官方 Home 需要
管理它们时，可以通过 adapter 或产品映射与 Project 关联。

## 所属层与边界

```text
Constitutional Substrate
  身份、authority、对象、journal、调用、流、效果与协商
             ↓
Protocols / Components / Content
  Project 可以组合它们，但不拥有其全部语义
             ↓
Host Control Plane / Official Distribution
  ProjectDescriptor、ProjectRegistry、数据目录、生命周期与 Home 映射
```

Project 属于 Host / distribution，而不是宪法基底。当前 Contract V1 仍通过公开方法暴露
Project 生命周期；第三方客户端可以使用这些方法，也可以构建完全不同的发行版。

## Steam 类比

| Steam | Plurora |
|---|---|
| Steam 客户端 | Plurora 平台 |
| 游戏库 | Home 屏幕 |
| 游戏卡片 | 项目卡片 |
| 游戏存档目录 | per-project 数据目录 |
| Steam 钱包 | 平台 secret store |
| 游戏自己的 DLC key | 项目 secret store |
| 游戏共用 OS 驱动 | 项目共用能力包 |

## 项目类型

三种 `project.type` 区分项目来源。

### plurora_native

仓库根目录有 `project.yaml`，引用 Plurora 能力包。这是为 Plurora 设计的项目，
是首选形式。

```yaml
schema_version: 1
project:
  id: my-project__abc12345
  title: My Project
  description: 一段简介
  type: plurora_native
  entry_surface_id: my-namespace/play
  packages:
    - packages/foo/manifest.yaml
    - packages/bar/manifest.yaml
  secret_policy:
    fallback_to_platform: true
```

### external_wrapped

外部项目（普通 git/npm 仓库），通过真实存在的适配器包包装。当前安装器不会伪造 adapter manifest；`--wrap-as-adapter` 会 fail-closed，并提示进入后续带 ChangeSet 审批的 adapter authoring 流程。

### external_workspace

外部项目作为 agent workspace 接入，不包装。适合临时使用、agent 协助修改的场景。默认是 host-owned managed copy；本地目录也可用 `--link-local` 建立明确的 user-owned 可变引用。两种模式都不在 intake 时执行项目代码。

## ProjectDescriptor

`project.yaml` 的顶层是 `ProjectDescriptor`。它描述项目实例，不描述某个包本身。

常用字段：

| 字段 | 含义 |
|---|---|
| `id` | 稳定 project id，用于目录、CLI、Home card。 |
| `title` | 给用户看的项目名。 |
| `description` | Home card 和详情页展示。 |
| `type` | `plurora_native` / `external_wrapped` / `external_workspace`。 |
| `entry_surface_id` | 点 Play 后进入的 surface contribution id。 |
| `packages` | 必装 package manifest 路径。 |
| `optional_packages` | 可选 package manifest 路径。 |
| `required_surfaces` | 项目认为必须存在的 surface ids。 |
| `secret_policy` | 项目密钥解析策略。 |
| `external` | 外部来源、ref、workspace root、`source_kind`、`workspace_ownership` 与可选 `source_digest`。 |

`entry_surface_id` 应该匹配某个 package manifest 中 `slot: experience_entry` 的 surface。
例如 YdlTavern 使用 `ydltavern/play`。

## 项目目录结构

```text
~/.plurora/projects/<project_id>/
├── project.yaml          # ProjectDescriptor 副本
├── secrets.dat           # age 加密的项目 secret store
├── sessions/             # 项目级 session 数据
├── state/                # 包能存的项目级状态
└── lockfile.toml         # 项目锁定的包版本
```

managed external workspace 独立存放在：

```text
~/.plurora/workspaces/external/<project_id>/<content_digest>/
```

`project.yaml` 中的 `workspace_ownership` 决定卸载权限：`managed` 路径必须位于上述 host-owned 根内才可归档/删除；`linked_local` 永远保留用户源目录。

文件权限：0700 目录，0600 文件（Unix）。
加密：同一份 master key（位于 `~/.plurora/secret-store.key` 或 OS keyring）。

## 软隔离 + 平台回退

这里的“软隔离”描述 package/workload 的 secret 与数据共享策略，不表示 Host 控制面可以跨项目访问。Host 设备 grant 已可用结构化、精确的 project selector 限定项目列表、session/event、开发、部署和私有 route；但它仍不是多用户成员系统，也不是把不可信 workload 放进强安全沙箱。默认 secret 行为：

- 项目自己的 secret 优先（`secret_ref:project:NAME`）
- 项目内没有 → 回退平台（`secret_policy.fallback_to_platform: true` 是默认）
- 平台也没有 → fail-closed

这样设计的意图：用户配一次 `OPENAI_API_KEY` 在平台层，所有项目能用。某个项目想用
单独的 key，在该项目设置里 override。两条路径都对用户可见，不是默默 fallback。

强隔离的项目可以关掉 fallback：

```yaml
secret_policy:
  fallback_to_platform: false
  require_per_project:
    - GITHUB_PAT       # 这个名字必须项目级配, 不允许平台 fallback
```

## 生命周期

```text
plurora install <url>
  ↓ (检测 project.yaml / 走 wizard)
Installed (注册到 ProjectRegistry, 可见于 Home)
  ↓ plurora project start (or Home 点 Play)
Starting → Running
  ↓ plurora project stop
Stopping → Stopped
  ↓ plurora uninstall
(询问保留数据)
  ├─ Keep: 项目数据移到 ~/.plurora/projects/.archived/<id>/；managed workspace 同步归档
  └─ Delete: 删除项目数据和经过 containment 校验的 managed workspace
```

linked-local source 不属于 Plurora，上述两种卸载选择都不会修改它。

任何状态都可以失败 → Failed。

## CLI 命令

```bash
# 安装项目
plurora install github.com/user/repo
plurora install github.com/user/repo --workspace-only    # 外部项目: 工作区
plurora install ./existing-source --link-local           # 本地外部项目: 保留用户所有权
plurora install github.com/user/repo --wrap-as-adapter   # 当前 fail-closed；不生成假 manifest

# 查看项目
plurora project list
plurora project info <id>
plurora project status <id>

# 控制
plurora project start <id>
plurora project stop <id>
plurora update --project-id <id> [--check-only]

# 卸载
plurora uninstall <id>                # 交互式问数据怎么办
plurora uninstall <id> --keep-data    # 保留 (移到 .archived)
plurora uninstall <id> --delete-data  # 立即删除
```

## Home 屏幕

`clients/web` 的 Home 路由显示所有已安装项目的卡片：

```text
┌─────────────────┐  ┌─────────────────┐
│   YdlTavern     │  │  Coding Agent   │
│   ●Running      │  │  ◯Stopped       │
│   [Play]        │  │  [Play]         │
└─────────────────┘  └─────────────────┘
┌─────────────────┐
│  + Install      │
└─────────────────┘
```

状态指示：

- ● Running (绿)
- ◯ Stopped / Installed (灰)
- ⏳ Starting / Stopping (黄)
- ❌ Failed (红)

点 Play 调用 `host.project.start`，启动后导航到项目的 `entry_surface`。

项目页带平台侧控制台：显示 bundle、包、最近事件、更新诊断、部署诊断，以及 host-plane 的 durable job / revision / recovery 状态；更新检查与执行通过 `official/install-lab/check_for_updates` / `update_project`，仍走公开协议 `capability.invoke`。

## Play 流程

Home 点 Play 后，Web shell 与 host 走固定的公开协议序列：

1. 用户点项目卡上的 Play。
2. `clients/web` 调 `host.project.start`。
3. host 把项目状态转为 Running，创建或复用项目 session。
4. Host 把已验证的 `project_id` 写入 session `metadata.project_id`，并加上 `project:<id>` label。
5. `project.start` 返回 `session_id` 与 `already_running`。
6. `clients/web` 调 `host.surface.bundle.resolve`，用项目的 `entry_surface_id` 拿 surface 包 URL。
7. `mountSurface` 挂载 sandboxed iframe。
8. iframe `initialProps` 注入 `sessionId` 与 `projectId`。
9. surface 内的 `callHostRpc` / `invokeCapability` 自动带 `session_id`。
10. host 后续把认证 principal、resource authority 与经服务端验证的 session/project binding 传到 capability 与 outbound dispatch。

这条链路让项目级 secret 解析能从 session metadata 找到项目范围，也让真实模型调用回到同一个项目 session。端到端说明见 [`REAL_MODEL_END_TO_END.md`](REAL_MODEL_END_TO_END.md)。

注：这个 `sessionId` 之后被用于：

- 所有 RPC 调用自动附带（`callHostRpc` 通过 `setActiveSessionId` 读取）。
- 流式调用（`streamCapability`）用它作为订阅范围，接收 `capability/stream.*` 事件。

## 显式部署

`project.start` 不启动外部进程。它只打开项目 session，并把项目标记为 Running。

若项目需要启动 Docker HTTP 服务，可以在 `project.metadata.deployment.docker` 声明最小部署描述符。Web 项目控制台会显示 Deploy / Stop 按钮，用户确认后由 `plurora-service` host broker 串联；浏览器只是瘦客户端：

1. `host.port.lease` 租 loopback 端口。
2. `official/docker-runtime-lab/start_container` 启动容器。
3. `host.proxy.register` 注册 HTTP/WebSocket 反代 route。

这条路径是显式操作，不会在打开项目时自动执行。完整说明见 [`DEPLOYMENT_RUNTIME.md`](DEPLOYMENT_RUNTIME.md)。

## 协议

宿主管理项目的协议（HostAdmin/HostDev，或同时拥有对应 action 与精确 project selector 的逻辑 HostDevice；其 Contract V1 context 使用 `anonymous` sentinel + authority envelope；普通包不能调）：

```text
host.project.list      列出已安装项目
host.project.get       项目详情
host.project.start     启动项目
host.project.stop      停止项目
host.project.status    项目状态
```

部署运行时协议（HostAdmin/HostDev，或拥有 `deploy` / `observe` 与匹配 target selector 的逻辑 HostDevice；普通包不能调）：

```text
platform.target.*   运行目标
platform.exec.*     受控本地执行
platform.port.*     loopback 端口租约
platform.proxy.*    HTTP/WebSocket route
```

生命周期事件：

```text
host/project.installed
host/project.started
host/project.stopped
host/project.uninstalled
```

## 与 Composition 的区别

| Composition (现有) | Project (新) |
|---|---|
| 静态 package-set 描述符 | 运行时实例 + 状态 |
| `plurora composition check` 校验 | `plurora project list/start/stop` |
| 用于 share/import bundles | 用于 Home + 安装 lifecycle |
| `plurora-cli` 内部类型 | `plurora-core` 公开类型 |

未来一个 composition 模板可以实例化为多个项目（不同 id，同一个包集）。当前
版本不强求这一点 —— 一个项目通常就是一个 composition 的具体实例。

## 安装检测

`plurora install <url>` 先检测来源与项目类型，再决定是否解析 package manifest。

- 有且 `type: plurora_native`：按原生项目安装。
- 有合法 package manifest：按包来源解析安装。
- 没有 project/package manifest：调用 `official/install-lab/prepare_external_intake`，建立 `external_workspace`。
- 有但解析失败：fail-closed，要求修正 descriptor。

外部项目默认进入 managed `external_workspace`，内容复制/获取到按 project id + digest 隔离的 host workspace；`--link-local` 仅适用于本地来源并显式保留用户所有权。重复安装同一来源/内容是幂等的，不会自动生成包装代码或运行项目脚本。

## 不做的事 (推迟)

- 多用户/项目成员/access control
- 项目导入/导出 bundle (sharing-lab 已有 bundle 格式)
- 多用户项目成员、workload 级强沙箱与跨 Host 项目授权
- 项目自动归档清理 (留给用户手动)
- 项目市场/marketplace (违反平台开放原则)
