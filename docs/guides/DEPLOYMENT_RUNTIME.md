# 部署运行时

> [English](./DEPLOYMENT_RUNTIME.en.md) · [中文](./DEPLOYMENT_RUNTIME.md)

Plurora 现在可以作为自托管 AI / agent 项目的部署宿主。这里的“部署”不是内核里的 Docker 概念，而是一组通用运行原语，再由普通能力包与 Host target driver 组合成 Docker、本地进程或已注册的远程 Agent 目标。

## 边界

内核只提供四类通用原语：

| 原语 | 协议族 | 作用 |
|---|---|---|
| target | `platform.target.*` | 描述一个可运行目标。内置 `local` 与 enrolled remote Agent 进入同一 registry/driver 合同。 |
| exec | `platform.exec.*` | 启动、停止、查询一个受控本地执行。默认全拒。 |
| port | `platform.port.*` | 向 host 租一个 loopback 端口。 |
| proxy | `platform.proxy.*` | 把受管 HTTP / WebSocket route 绑定到某个 port lease，并显式记录 Host 认证或公开访问。 |

Docker、git、安装、secret store、workspace、adapter 都不是内核概念。它们由普通能力包实现。

## 当前实现

- `LocalExecExecutor` trait：默认 `DenyAllLocalExecExecutor`，profile 显式开启后可用 `LiveLocalExecExecutor`。
- `LiveLocalExecExecutor`：只接受 argv 数组，不接受 shell 字符串；cwd、env、日志、超时、kill 都由 host 控制。
- `plurora-service` 反代：`/p/<route_id>/...` 继续保留并位于 Host 认证内；如果 route 显式选择 `public`，且设置 `PLURORA_APP_BASE_DOMAIN=apps.example.com` 或 `--app-base-domain apps.example.com`，才会额外启用 `<slug>.apps.example.com/` 免 Host 认证虚拟主机，让社区应用拥有根路径 `/`。两种入口都只指向 active loopback port lease；禁 redirect；剥离或重写危险 header；限制响应体；支持 HTTP 与 WebSocket。
- `plurora/docker-runtime-lab`：普通第一方 capability Package，使用 `bollard` 管理 Docker 容器。默认无 Docker 时 fail-closed；真实 Docker smoke 需要显式 opt-in。
- Target driver：内置 `local` 与 enrolled Agent 使用相同的 durable operation、artifact transfer、declarative verifier、deployment apply/stop 和 receipt 模型；Agent 上游仍只绑定 loopback，并经 target/route/lease/epoch 约束的认证 tunnel 回到 Host proxy。
- Web 项目控制台：显示 target / exec / port / proxy 诊断，以及 host-plane 的活动修订、恢复状态、修订历史和最近任务。若项目声明部署描述符，用户可显式选择 Host 认证或公开 route，再点击 Deploy / Stop、启动 Build & Deploy、恢复或回滚；Development 区还可把已验证 ChangeSet 送入 private preview、独立部署审批、activation 和中断对账。默认保持 Host 认证。
- 持久化与回放：exec / port / proxy 注册表的变更都写进事件日志，host 重启时回放重建。
- 重启对账：host 重启后，回放出来的记录先降级（exec → unknown、port → reserved、proxy → stale 且 `ready=false`），再与真实世界对账。
- readiness gating：proxy route 注册时 `ready=false`，反代对未就绪的 route 返回 503；只有就绪后才放行。
- 健康监督：host 后台回路定期探测每条 active route 的 upstream，连续失败翻 `ready=false`、恢复翻 `ready=true`，并在状态转换时写审计事件。
- Build & Deploy broker：`POST /host/v1/build-deploy` 创建带可选 `idempotency_key` 的 durable job intent，host 侧完成 git clone、Dockerfile / nixpacks 构建、容器启动、proxy 注册、readiness probe。浏览器通过 job status / SSE 查看进度，可取消任务。

## Docker 部署描述符

Phase 6 Realization 替换完成前，显式 deployment 请求仍可提交最小 Docker 信息；这些字段属于 Host-local operation，不写入 WorkRevision：

```yaml
project:
  metadata:
    deployment:
      docker:
        image: ghcr.io/example/app:latest
        container_port: 3000
        port_name: web        # optional, default: web
        route_id: my-app-web  # optional, default derived from Installation
        route_access: host_authenticated # optional; host_authenticated | public
        health_path: /healthz # optional, 用于 readiness probe
        pull_if_missing: false
```

当前 Web broker 只接受这些字段。旧描述符或缺失 `route_access` 时按 `host_authenticated` 处理；公开访问必须在部署 UI 再次显式选择。`env`、`volumes`、`mounts`、`binds`、`secrets` 会被拒绝，避免在第一版部署路径里悄悄扩大权限。

## Build & Deploy 描述符

如果项目没有预构建镜像，可以在 `project.metadata.deployment.build_deploy` 声明从源码构建：

```yaml
project:
  metadata:
    deployment:
      build_deploy:
        source_url: https://github.com/example/app.git
        ref_name: HEAD
        strategy: dockerfile # dockerfile | nixpacks
        dockerfile_path: Dockerfile # optional
        container_port: 3000
        port_name: web
        route_id: my-app-web
        route_access: host_authenticated # host_authenticated | public
        health_path: /healthz
        runtime_env:
          - name: NODE_ENV
            value: production
          - name: OPENAI_API_KEY
            secret_ref: project:OPENAI_API_KEY
        runtime_mounts:
          - source_host_path: /srv/plurora-data/my-app
            container_path: /app/data
            mode: ro
            approved: true
            high_risk_approved: false
            reason: persistent app data
```

`dockerfile` 策略使用仓库里的 Dockerfile。`nixpacks` 策略先运行本机 `nixpacks` 生成 Dockerfile / context，再由 `docker-runtime-lab` 通过 Docker 构建镜像。`nixpacks` 不可用时 fail-closed。

运行时 secret 只接受 `store:` / `project:` / `env:` 形式的 `secret_ref`。原始 secret 由 host 私有 Docker runner 注入容器，不经过 `docker-runtime-lab` 包边界，也不写入事件、日志或 job 状态。构建时 secret 暂不支持，遇到 build secret 字段会 fail-closed。

volume 可以指向任意宿主路径，但必须逐条批准。默认建议只读；读写挂载必须额外确认。host 会拒绝 Docker socket、系统目录、密钥目录、Plurora secret store、过宽 home 目录以及这些路径的祖先目录。

## 显式 Deploy 流程

项目控制台里的 Deploy 按钮不会自动触发。用户确认后，请求发到 host-plane 的 `POST /host/v1/deploy`，由 host broker 在服务端串起整条链路（浏览器只是瘦客户端，不再亲自编排）：

1. host 侧重新校验请求（不信任客户端字段）。
2. `host.port.lease`：向 host 租 loopback 端口。
3. `capability.invoke` → `plurora/docker-runtime-lab/start_container`：启动 Docker 容器，传入 `approved: true`、`host_port` 与 `port_lease_id`。
4. `host.proxy.register`：把 route 与显式 `route_access` 绑定到刚租到的 port lease（此时 `ready=false`）。
5. readiness probe：对 loopback 端口做 TCP 连接（带可选 health_path 的 HTTP 探测），有界超时内成功才把 route 翻成 `ready=true` 并返回成功。

任意一步失败后，broker 会反向回滚：注销 proxy、停止刚启动的 container、释放 port lease。因为编排在 host 侧，关闭浏览器标签页不会留下孤儿容器或端口租约。

Stop deployment（`POST /host/v1/deploy/stop`）按 Docker label（`route_id`）查找并停止对应容器，不依赖浏览器记住的 container id，也不会因为同名 `port_name` 去停止未知容器。

## 虚拟主机路由

路径前缀 `/p/<route_id>/...` 对平台调试很方便，但真实社区应用通常假设自己拥有 `/`。例如前端会写 `fetch('/api/...')`、静态资源从 `/assets/...` 读取、WebSocket 连 `/ws`。因此 plurora-service 支持可选虚拟主机入口：

```bash
plurora host serve --app-base-domain apps.example.com
# 或
PLURORA_APP_BASE_DOMAIN=apps.example.com plurora host serve
```

启用 base domain 本身不会公开任何 route。只有注册为 `public` 的 route 才会从 `route_id` 派生 DNS-safe slug，并把 public URL 展示为 `https://<slug>.apps.example.com/`。`host_authenticated` route 和没有配置 base domain 的环境都继续返回受 Host 认证的 `/p/<route_id>/`。

边界规则：

- `ProxyRouteAccess` 是 proxy route 的通用访问属性；hostname 仍是 service 层派生方式，kernel 不知道 DNS。
- 只接受 `<slug>.<app_base_domain>`。任意其他 Host、裸域、伪后缀（如 `foo.apps.example.com.evil.com`）不会命中。
- 不信任 `X-Forwarded-Host`。
- 只有 `route_access=public` 的 vhost 入口不要求 Plurora Host 身份；它代表被部署应用自己的公开入口。私有 vhost 返回 404，`/p`、RPC、Host API 等仍走 Host 身份和 scope。
- 上游仍必须是 loopback lease，且 route 必须 active + ready。
- vhost 请求会把 `Host` 设为应用自己的 hostname；`Authorization`、Plurora `access_token` query、`Referer` 不转发。
- vhost 响应会把 `Set-Cookie` 的 `Domain` 去掉，变成 host-only cookie；只重写同 upstream 的 `Location`，外部 absolute redirect 仍被剥离。

## Build & Deploy 流程

Build & Deploy 使用 `POST /host/v1/build-deploy`。默认立即返回 `job_id`、status URL 和 SSE events URL，长耗时工作留在 host broker 后台执行：

1. 校验源码 URL、策略、runtime env、runtime mounts 和用户批准。
2. 通过 `git-tools-lab` 克隆到项目工作区；project/workspace 祖先必须是 canonical data root 下的真实目录，选定 tree 的 materialization 超过 100,000 个文件、100,000 个目录或 1 GiB 时 fail-closed。submodule entry、绝对/逃逸根目录的 symlink，以及无法保留 symlink 的平台上的 symlink entry 都会明确失败。当前 transport 仍会执行临时 bare fetch，因此这些 tree 上限尚不能视为 repository download budget。
3. 若策略为 `nixpacks`，先生成 Dockerfile / context。
4. 调用 `plurora/docker-runtime-lab/build_image` 构建镜像，打上 `installation_id`、`workspace_id`、`build_id`、`source_commit`、`strategy`、`build_descriptor_hash` 等 label。
5. 如果 Installation 已有活动修订，构建完成后再处理旧容器、route 和 lease；旧修订在新修订提交前仍是 durable active pointer，因而替换失败会明确进入“需要恢复”状态。
6. 进入普通部署链路：port lease → 容器启动 → proxy 注册 → readiness probe。
7. readiness 成功后先原子追加修订激活事件，再把内存状态翻成 Ready；事件提交失败会回滚新部署。

job intent、最新状态快照、不可变部署修订和 active pointer 都写入当前 profile 的 `EventStore`。SQLite / Postgres profile 因此可以跨 host 重启恢复控制面；内存 profile 仍只适合临时开发。未完成 job 在重启后会被确定性标记为 Failed，host 不会自动重放 clone / build / deploy 副作用。完整实时日志仍是有界内存环，journal 只保留脱敏状态与最后事件。

每个成功的 Build & Deploy 会产生一个 `DeploymentRevision`：包含源码 ref、构建产物身份、`route_access`、route 配置和脱敏回执，但不保存原始 secret 或宿主挂载路径。只有全部 runtime env 来自 `secret_ref` 且没有 host mount 的修订可自动恢复；明文 env 或 mount 会记录 blocker，并要求手动重新构建。recover / rollback 保留修订的 route 暴露选择。journal event 始终不可变；为限制重启内存和响应体，实时控制面投影及 API 每个项目保留最近 64 个修订。

## Verified ChangeSet preview 与 activation

这条路径只接受已提交的 `managed_external` ChangeSet、`docker_build` 验证结果和完整 provenance。验证镜像在验证后删除；部署输入是不可变 build-context artifact，而不是镜像或 live workspace。

1. `POST /host/v1/development/workspace/<workspace_id>/changes/<change_set_id>/deployment/preview` 重新校验 descriptor、tree、verification/build-context artifact 与 Workspace/Installation/Target authority，再在显式 `local` 或 Agent target 上执行类型化 artifact transfer、Docker build 和 deployment apply。生成的 preview route 固定为 `host_authenticated`。
2. `POST .../deployment/approve` 单独批准或拒绝精确 preview；approval artifact 绑定 candidate receipt、artifact refs、target 与 authority，源码审批不会隐式批准部署。
3. `POST .../deployment/activate` 再次验证全部证据和 readiness，把用户请求的私有或显式公开 route 指向同一 candidate，提交不可变 `VerifiedActivate` revision 后才 drain 上一修订。
4. Host 在 preview/activation 期间崩溃或 effect 结果不确定时，事务进入 `recovery_required`。`POST .../deployment/reconcile` 只采用 provenance 完全匹配的 durable activation，或清理精确 candidate/route/lease；歧义状态继续阻断。

项目级 host API：

- `GET /host/v1/installations/<installation_id>/deployments`：活动修订、runtime readiness、恢复需求、任务和修订历史。
- `POST /host/v1/installations/<installation_id>/deployments/recover`：显式恢复活动修订。普通 `GitClone` 修订复用保留的本地镜像，不重新 clone/build；`VerifiedArtifact` 修订重新校验证据并在记录的 target 上从 durable build context 重建。
- `POST /host/v1/installations/<installation_id>/deployments/rollback`：把历史修订激活为新的不可变 rollback revision；普通修订复用保留镜像，verified 修订从其 durable context 在记录的 target 上重建。显式 stop 清除 active pointer 后仍可回滚，旧记录不会被修改。
- `POST /host/v1/deploy/stop`：清理 route 对应的 host 资源；如果它属于活动 durable 修订，同时追加 deactivation 事件。

recover / rollback 都是显式用户动作。普通修订要求 replay-safe、本地镜像仍存在且 secret 仍可解析；verified 修订要求 artifact closure、preview/approval evidence 与当前 project/target authority 仍有效。verified replay 永不读取 live workspace 或重新抓取源码。任何失败都会保留原 active pointer 并显示 recovery required，不会静默声称已经恢复。直接的预构建镜像 `/host/v1/deploy` 目前仍是临时 broker 操作，不会创建 durable revision。

## Run 不自动部署

Installation `ready` 只表示采用记录有效；它不启动进程、不分配端口、不注册 proxy。Phase 4 的 `host.run.start` 只激活已安装、已验证且唯一匹配的本地 Run-bound 实现；缺少组件或需要机器资源时返回结构化 gap，不隐式 build/deploy。打开 Library/Installation UI 与 Run 启动保持可见分离。

部署仍是单独、显式的 Host 行为。Managed Realization 的 plan/apply 属于 Phase 6；本页的 transitional deployment controller 不定义 Run 生命周期。

## 安全红线

- 默认全拒。没有 profile opt-in，不会真实启动本地进程。
- 端口只绑定 loopback。
- proxy upstream 必须引用 active port lease，并且 `port_name` 必须匹配。
- proxy route 默认 `host_authenticated`；公开 vhost 必须由 `route_access: public` 显式开启，配置 wildcard 域名不会批量公开现有 route。
- path-prefix 反代不跟随上游 redirect，不转发 `Set-Cookie` / `Location` / CORS 等危险响应头；vhost 反代只允许 host-only cookie，并只重写同 upstream 的 `Location`。
- 预构建镜像部署不接受 env / volume / secret。源码 Build & Deploy 只接受显式批准的 runtime env / volume；runtime secret 原始值只在 host 私有 runner 内注入，构建时 secret 暂不支持。
- Verified ChangeSet 部署不复用验证镜像，不从 live workspace 构建，也不绕过独立部署审批；preview 始终先保持 Host 认证。
- Docker 通过普通能力包实现，没有官方快速路径。

## 后续

- 原生执行仍只适合 trusted / dev 场景，不是完整 OS 沙箱。
- **自动重启**还没做，且是单独的后续阶段。host-plane 已有 durable revision 和显式 recovery，但健康监督仍只做监测 + 翻 readiness + 审计；它不会未经用户授权自动重放部署副作用。
- Remote Target Agent、Project Console 与 verified dev-to-deploy 接线已完成 Candidate 闭环，并由 GitHub CI 覆盖故障、Host 重启、recover 与 rollback。target-edge ingress 和应用身份仍需单独设计；该能力不等于任意网络代理。
- Docker 描述符还没有 pull 进度和长期日志归档。
- 外部项目 ChangeSet 现在可以受控添加 Dockerfile；更丰富的部署描述符/adapter 引导式创作仍是后续，而且部署必须保持显式审批与激活。
