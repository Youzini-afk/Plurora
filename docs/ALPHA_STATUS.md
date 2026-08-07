# 平台现状

> [English](./ALPHA_STATUS.en.md) · [中文](./ALPHA_STATUS.md)

这是 Plurora 当前实现状态的快照。它记录代码、公开合同和质量检查已经支持什么，明确标注 partial 或 deferred 的除外；它不负责定义平台为什么存在或未来只能往哪里发展。

平台目标与原则见 [`CHARTER.md`](CHARTER.md) 和 [`architecture/VISION.md`](architecture/VISION.md)；官方产品责任见 [`product/PLATFORM_PRODUCT_MODEL.md`](product/PLATFORM_PRODUCT_MODEL.md)，游创只是可选 Profile。建设方向见 [`roadmap/NEXT_STEPS.md`](roadmap/NEXT_STEPS.md)。

## 概要

- **Conformance：** 473 个具名 CLI 用例通过，外加 crate / service 单元测试；161 个 v1 schema（80 methods + 59 events + 22 top-level）通过校验。
- **章程纪律：** 内核对内容无意见；官方包没有特权；公开协议是唯一入口；入口形态平等；能力句柄、bindings 注入、Path A / Path B、conformance kit 与生成 SDK 已落地；可信路径阻断 raw secret，全部走 manifest 声明的 `secret_ref`；权限授权可重新水化；网络声明带审计与脱敏；通用流式与取消生命周期；外发执行有边界，默认全拒；公开 HTTPS 出站走同样的 host policy / 审计 / 脱敏边界；一元、SSE/NDJSON/raw 流和 WebSocket 三个原语都有完成审计事件。
- **代码健康：** CLI、运行时各域行为、协议分发、in-process 处理器、事件存储——都已按域拆分，不再继续往单文件里堆。
- **人测底座：** 安装 warning 与 schema 形状已稳定；原生项目安装链路从 source → store → nested manifests/profile autoload → project registry → project dist → 受保护的 `/surface-bundles/projects/<project_id>/...` → 短期 sandbox asset lease；`surface_bundle` 是 static、non-executing 入口；`dist/` 已进入 `tree_hash`，store schema 迁移会清掉旧 store，install/update/uninstall 后会回收孤立 store；`plurora/install-lab` 提供 `check_for_updates` / `update_project`，CLI `plurora update` 与 Web 项目控制台都通过它更新；Surface bridge 已收敛 allowlist、stream ownership、诊断脱敏、secret 输入清理、CSP/CORS 加固与 typed `allowed_capability_ids`；桌面端管理 loopback Host sidecar，Web shell 可安装为 PWA；自托管部署底座包含统一 local/Agent target driver、target / exec / port / proxy、HTTP/WebSocket 反代、显式 Deploy broker、默认私有/显式公开 route、共享 Host/project/target 客户端 context，以及 Verified ChangeSet → private preview → 独立部署审批 → activation → reconcile/recover/rollback。可撤销 scoped device pairing 让手机通过同一 Host API 控制项目、部署与 ChangeSet；Web/Desktop/PWA 复用同一 client core，远程 CLI 通过同一 Bearer/public Host 边界完成 project/target 操作以及 ChangeSet 的草拟、审阅、批准/拒绝、执行、导出和恢复完整生命周期。

当前已经形成较大的可运行面，但平台和官方产品都不等于“完成”。后续建设同时关注开放性、多样性、先进执行与协议能力、长期数据演化，以及普通用户和创作者的完整体验。

## 内核

- 不带内容的会话、只追加的不透明事件、清单驱动的能力包、能力机制、钩子机制、surface 贡献、提案生命周期、资产 / 分支 / projection 底座。
- SQLite 事件日志，每会话单调递增的序号，可重新水化的底座。
- Experimental SHA-256 ObjectStore 与 ArtifactDescriptor 已落地：内存/文件系统 CAS、读取后校验、stream、旧 FNV asset 事件幂等迁移；asset 事件只保存 descriptor/ref，不再保存正文。
- Experimental EffectReceipt 与 Change primitives 已落地：capability/outbound/stream/WebSocket/exec terminal path 生成内容寻址 receipt；历史重放不调用 executor，capability re-execute 创建新 branch 与 parent-linked receipt；旧 Proposal lifecycle 作为 Intent/ChangeSet/PolicyDecision/Commit adapter 继续兼容。
- Experimental Protocol Commons 注册表已落地：`host.info` 发布 Change、Shell Default 与 World Bundle 描述符；显式协议/Profile 协商先于 dispatch；不支持的 major 以结构化原因拒绝；protocol、implementation 与 package conformance 使用独立可执行报告。
- Package envelope 与 component identity 已分离：显式 component/behavior digest 在重新打包后保持稳定；runtime 与 effect evidence 携带 component trust/边界数据；composition lock 分离 component/profile/content pin；`contract:none` 明确报告为不可移植 Foreign Capsule。
- Experimental World Bundle 已落地，并由 `plurora/playable-creation-board` 的跨 Host conformance fixture 覆盖：canonical archive descriptor 保留原始 v1 envelope 与完整 SHA-256 closure；全新 SQLite/filesystem Host 保持 object、lineage 与 receipt；historical replay 不调用 executor；替代实现生成 child branch/head；headless CLI 无需 Web Shell 状态即可读取同一 archive。
- 用 JSON Schema 子集校验能力 I/O 与能力包声明的事件 payload。
- Contract V1 身份 union 继续保持 `host_admin`、`host_dev`、`package`、`human`、`assistant`、`anonymous`。配对设备在远程 RPC 边界使用 fail-closed 的 `anonymous` V1 sentinel，并通过 Host 建立的 authority envelope 保留 grant、delegation 与资源约束；旧 runtime 忽略新 envelope 时只会拒绝而不会扩大权限。脱敏 Host 控制面审计仍以逻辑 `host_device` 记录设备；human 与 assistant 身份支持作用域授权。
- 审计事件：`authority/grant.created|revoked`、`authority/denied`、`host/package.*` 生命周期与 `change/proposal.*` 生命周期；Contract V1 之外另有脱敏的 `host/control/v1/authority.decision` Host 控制面授权判定日志。
- 持久授权：grant / revoke 事件可在 SQLite-backed 运行时中重新水化。
- Contract V1 是公开平台规范：80 个协议方法、59 个事件类型、161 个 JSON Schema。`authority.handle.*`、`host.package.audit`、能力句柄、bindings 注入、Path B、conformance kit 与 SDK 生成均为 implemented。

## 安全执行

- **`secret_ref` 引用：** 支持 `secret_ref:<vault>:<key>`、`secretRef:`、`secret-ref:`、`host:` 各种前缀。能力包通过引用提及 secret，原始值不出现在事件、提案、日志、审计里。
- **环境变量解析器：** host 拥有的解析器，带显式 allowlist。默认全拒；env 名要先放行才能解析。错误只带 env 名，绝不带原始值。
- **本地加密 secret store：** `secret_ref:store:NAME` 通过 `StoreSecretResolver` 从 `~/.plurora/secrets.dat` 解析；`secret_ref:project:NAME` 先读项目级 store，再按 `secret_policy` 回退平台 store；store 使用 age (X25519) 加密，主密钥来自 OS keyring（延后启用）或 0600 本地 key 文件。
- **Raw secret 阻断：** 提案的 operations / expected effects 与资产 metadata 会被保守扫描，明显的 API key、token、password 字段被拒绝。资产内容与普通文本不扫描，避免误伤用户内容。
- **网络权限声明：** 清单中的 `permissions.network` 同时支持扁平 `hosts`（向后兼容）和结构化 `declarations`（带 `host` / `methods` / `purpose`）。无声明的能力包不能出网。官方包没有绕过。
- **外发审计与脱敏：** 每条出站请求都生成审计记录，只含身份、能力包 id、目标主机、方法、用途、脱敏状态、用到的 `secret_ref`，不含原始 body / header / 提示词 / 响应。
- **外发执行边界：** 内容无关的 HTTP 与 WebSocket executor trait。默认 deny-all（fail-closed），可切换为 fake executor（带 fixture，用于 conformance）或 live executor（HTTP 使用 reqwest + rustls，WebSocket 使用 tokio-tungstenite + rustls；默认关闭；HTTP 为 HTTPS-only，WebSocket 为 WSS-only；重定向 fail-closed；secret header 只在执行时注入，不进审计）。真实 live 模型 / WebSocket 出站必须通过 profile 与环境变量显式 opt-in；默认 conformance 不联网，真实 WebSocket smoke 还要求 `PLURORA_LIVE_WEBSOCKET_TESTS=1`。
- **协议方法：** `host.outbound.audit` 列出某个能力包的出站审计事件；`host.outbound.execute` 让普通能力包通过 host executor 发起一元出站请求；`host.outbound.stream` 提供 SSE/NDJSON/raw 流式出站；`host.outbound.websocket.open|send|close` 提供双向 WebSocket 出站。
- **完成审计事件：** `host/outbound.execute.completed`、`host/outbound.stream.completed`、`host/outbound.websocket.completed` 覆盖三种出站原语；事件只记录状态、计数、耗时、executor kind、network_performed、redaction state 与 `secret_ref` 引用。
- **流式生命周期：** 流注册表跟踪进行中的流式调用，按序发出 `capability/stream.started|chunk|progress|ended|error|cancelled|timeout`。取消和超时阻断后续 chunk。非流式能力被拒绝。

## 公开协议与传输

- 规范的请求 / 响应信封，自带 host 绑定的身份上下文。调用方不能自己声称是某个能力包或 admin。
- 同一份 dispatcher 同时承载 HTTP `POST /rpc` 和 host JSON-RPC stdio (`plurora host-stdio`)。
- Contract Registry `0.5.0` 已完成第一次真实 Deprecated → Legacy Adapter 转换：36 条 alias 仍统一走集中解析；`host.info` 与 `host.target.list` 是 Candidate，其 the public contract alias 保留 `0.4.0` 弃用历史并从 `0.5.0` 起冻结为 identity Legacy Adapter。它们只接受安全修复和数据读取兼容，不增加新字段语义；HTTP、host stdio 与 subprocess reverse stdio 返回 additive lifecycle diagnostics，生成 SDK 以队列保留警告；`plurora contract migrate` 提供有 ID 边界的 preview 与原子写入/回滚。Web 生产调用已全部切换到 canonical ID。
- 通过 SSE 订阅事件，支持 `after_sequence` 回放和实时追尾。
- 基于 profile 的 `plurora host serve` 自动加载能力包，对外暴露 `/rpc` 与 SSE。
- Host 控制平面在 Contract V1 之外保持独立：root token 是根凭据；持久化设备 grant 同时按 action scope 与 `project` / `target` 资源选择器衰减，支持有界委托、祖先撤销级联、过期、单项撤销和原子的管理员批量撤销。HTTP 与 RPC 在进入运行时前保留同一设备身份和 authority，项目会话必须显式绑定项目；每次设备协议调用都会写入脱敏的 allow/deny 判定日志。开发长操作会在 Docker 与 managed-workspace 效应前、以及阻塞验证结束后刷新 grant/祖先状态。移动 PWA 与 `plurora host access` CLI 通过同一 Host API 管理授权，pairing 仍只经 HTTPS 一次性交换为 Secure/HttpOnly Cookie。详见 [`architecture/HOST_REMOTE_ACCESS.md`](architecture/HOST_REMOTE_ACCESS.md)。
- TCP 传输留作后续工作；WASM 与远程入口在清单中已是一等形式，执行延后。

## 包执行

- `rust_inproc` 包通过 host 提供的 trait 与 catalog 执行。声明了 in-process provider 但 catalog 里找不到的清单会被拒绝。
- `subprocess` 包通过 stdio 上的 JSON-RPC 执行：握手、调用、超时、degraded 状态、重启、卸载即杀、stderr 日志捕获。
- `wasm` 与 `remote` 入口：清单已支持，执行延后。
- 路径 A (`entry.contract: "v1"`) 接收能力句柄 bindings 并接受权限强制；路径 B (`entry.contract: "none"`) 自包含运行，不接收 v1 权威，但生命周期仍可观察。
- 能力路由支持显式 provider 选择，以及精确匹配和 `^x.y` 简单版本约束。歧义时拒绝，除非调用方指定 `provider_package_id`。
- 钩子机制：确定性排序、能力包持有的处理器、payload 元数据修改、否决、卸载清理；覆盖 `journal/before_append|after_append` 与 `capability/before_invoke|after_invoke`。

## 底座

- 资产注册表：不透明的 `id` / `mime` / `hash` / `size` / `origin_package_id` / `metadata`，可从 SQLite 重新水化。权限执行与内容寻址 blob 存储留待后续。
- 会话 fork / 分支沿革，可从事件日志重新水化。
- 通用 projection 注册表：通过 `kind_prefix` 与 `writer_package_id` 过滤事件来重建，写入 `projection/updated`。包持有的 projection 执行留待后续。
- 项目运行时：`ProjectDescriptor`、`ProjectRegistry`、`~/.plurora/projects/<id>/` 布局、项目级 secret policy、Home 项目卡、项目级 storage summary、redacted package failure summary，以及 `plurora project list/info/status/start/stop` 已落地。
- 部署运行时：`host.target.*`、`host.exec.*`、`host.port.*`、`host.proxy.*` 已落地；默认 deny-all，profile 可显式启用 `LiveLocalExecExecutor`；内置 `local` 与 enrolled Agent 使用相同的 durable operation/artifact/verifier/deployment receipt 合同，端口只绑定 loopback，proxy upstream 必须引用 active port lease，Agent 流量只经认证 tunnel 返回 Host proxy。`ProxyRouteAccess` 默认 `host_authenticated`，只有显式 `public` route 才启用可选 `<slug>.apps.<host>/` 免 Host 认证 vhost，`/p/<route_id>/...` 始终保留在 Host auth 内。Web 项目控制台支持显式 Docker Deploy / Stop、Dockerfile / nixpacks Build & Deploy，以及由 immutable build-context artifact 驱动的 verified ChangeSet private preview、独立审批、`VerifiedActivate` revision、显式 reconcile/recover/rollback；verified replay 在记录的 target 上重建，不读取 live workspace 或重新抓取源码。真实 MDN 仓库与结构不同的 Python fixture 已在 GitHub CI 覆盖故障、Host restart 和 rollback。
- Surface 贡献：带版本、slot、激活方式、所需权限、审批策略、metadata 的描述符。Slot 包括 `experience_entry`、`home_card`、`quick_action`、`workshop_card`、`play_renderer`、`forge_panel`、`asset_editor`、`assistant_action`。`quick_action`、`workshop_card` 与带 `metadata.shell_schema_version: 1` 的 `home_card` 是结构化 shell descriptor：Web shell 只读取受限文本、icon hint、排序和同包 target，由平台渲染；不加载包 JS、不解析 HTML、不 mount iframe。复杂项目 surface 继续走 `surface_bundle` + sandbox iframe。通过 `shell.contribution.list` 与 `.describe` 发现。
- Surface bundle：`surface_bundle` 是清单里的静态浏览器 bundle 入口，不是可执行 package entry；安装后的项目 bundle 内部位于 `/surface-bundles/projects/<project_id>/...`，原始路径要求 Host 身份。opaque-origin sandbox 在项目授权后获得绑定 grant/bundle root 的五分钟 `/surface-assets/<lease>/...` 只读句柄，不携带 Host credential。`dist/` 参与 `tree_hash`，因此只改浏览器 bundle 也会触发更新；project dist 通过临时目录 + 原子替换刷新。
- 提案生命周期：`change.proposal.create|get|list|approve|reject|apply`。当前 `apply` 只跑通用操作 `asset.put` 与 `projection.rebuild`。更广泛的事务和回滚留待后续。

## 包安装与项目模型

| 能力 | 状态 |
|---|---|
| `manifest.requires` 字段 | implemented |
| Lockfile schema (`plurora.lock.v1`) | implemented |
| `plurora/git-tools-lab`（基于 gix） | implemented |
| `plurora/integrity-lab`（sequoia GPG + sha256） | implemented |
| `plurora/install-lab` 编排器 | implemented |
| `plurora install` / `uninstall` / `list-installed` / `update` / `lockfile` CLI | implemented |
| `~/.plurora` 文件系统约定 | implemented |
| 交互式同意提示 | implemented |
| 静态 conformance 集成（默认 warning，`--strict` 阻断） | implemented |
| GPG 签名验证（默认关闭，`--require-signed` 启用） | implemented |
| 循环依赖检测 | implemented |
| 真实 GitHub smoke（opt-in） | implemented |
| `dist/` 纳入 `tree_hash` | implemented |
| store schema 迁移清理旧 store | implemented |
| 孤立 store GC（安装 / 更新 / 卸载后） | implemented |
| `plurora/install-lab/check_for_updates` | implemented |
| `plurora/install-lab/update_project` | implemented |
| `plurora/secret-store-lab` 加密存储 | implemented |
| `plurora/docker-runtime-lab`（Docker 容器生命周期，bollard） | implemented |
| `StoreSecretResolver` + `CompositeSecretResolver` | implemented |
| age (X25519) 加密 + 0600 文件权限 | implemented |
| OS keyring 集成 | deferred（libdbus-sys 系统依赖） |
| `plurora secret put / list / delete` CLI | deferred |
| Sigstore 验签 | deferred |
| Tauri UI 安装路径 | deferred |
| 自动更新守护 | deferred |
| 二进制包分发 | deferred |
| 项目作为一等运行时概念 | implemented |
| `ProjectDescriptor` + `ProjectId` + `ProjectType` + `SecretPolicy` | implemented |
| `~/.plurora/projects/<id>/` 布局 | implemented |
| `secret_ref:project:NAME` + 平台 fallback | implemented |
| `ProjectRegistry`（内存 + 磁盘扫描） | implemented |
| `ProtocolContext.session_id` 传递 | implemented |
| 安装识别（原生 vs 外部） | implemented |
| 外部项目 wizard（wrap / workspace） | implemented |
| `plurora project list/info/status/start/stop` | implemented |
| `plurora uninstall` 归档提示 | implemented |
| `host.project.list/get/start/stop/status` | implemented |
| `host/project.installed/started/stopped/uninstalled` | implemented |
| Home 项目卡 | implemented |
| YdlTavern `project.yaml` | implemented |
| 原生项目安装到 profile、project registry 与 project dist | implemented |
| `surface_bundle` 静态入口与 installed project bundle route | implemented |
| typed `allowed_capability_ids` bridge 声明 | implemented |
| CLI `plurora update` 通过 install-lab 更新项目 | implemented |
| 多租户级 `project_id` 进入 `ProtocolContext` | deferred |
| 项目归档超过 30 天自动清理 | deferred |

安装默认值已放宽到 cargo / npm / pip 技术基线：HTTPS-only、内容哈希、原子写入始终启用；签名验证与 conformance 阻断分别通过 `--require-signed` / `--strict` opt-in。

## 真实模型端到端路径

| 能力 | 状态 |
|---|---|
| `huggingface-fetcher` 测试通过 | implemented |
| Surface bundle 解析由 metadata 驱动 | implemented |
| `host.surface.bundle.resolve` | implemented |
| 需 Host 身份的 `/surface-bundles/<prefix>/<file>` 路由 | implemented |
| 需精确 project authority 的 `/surface-bundles/projects/<id>/<file>` 路由 | implemented |
| grant/root-bound 五分钟 `/surface-assets/<lease>/...` sandbox 句柄 | implemented |
| `project.start` 打开项目 session 并设置 `metadata.project_id` | implemented |
| `project.start` 返回 `session_id` + `already_running` | implemented |
| `project.get` / `status` 返回 `running_session_id` | implemented |
| `project.stop` 关闭项目 session 并发出事件 | implemented |
| Surface 通过 `initialProps` 接收 `session_id` | implemented |
| TavernProvider.sendMessage 调用 engine `model.live_call` | implemented |
| API Connections 抽屉支持 platform / project 范围切换 | implemented |
| Engine manifest 声明 `secret_ref:project:*` | implemented |
| Surface 流式响应 UX | implemented |
| Surface-host stream postMessage 协议 | implemented |
| Surface bridge allowlist / stream ownership / diagnostics redaction / secret input cleanup / CSP/CORS hardening | implemented |
| `streamCapability` helper（YdlTavern host-rpc） | implemented |
| `AsyncIterable<StreamFrame>` 消费 + iterator early-return 清理 | implemented |
| `cancelGeneration` action + Stop 按钮 | implemented |
| 单 chat 多并发生成 | deferred |
| token-rate UI | deferred |
| Realtime / WebSocket streaming UX | deferred |

## 官方能力包

全部是普通能力包，没有内核特权。位于 `packages/plurora/`，通过普通清单加载。

**平台基础**

- `package-lab`、`schema-tools`、`event-tools`、`composition-lab`、`asset-lab`、`projection-lab`、`assistant-lab`。
- 包安装基础：`plurora/git-tools-lab`、`plurora/integrity-lab` 与 `plurora/install-lab`。Git tree fetch 默认执行有界 materialization，并对 pack 下载做可计量、可中断的预算控制；直接调用者也不能关闭两类硬上限。

**创作能力族**

- `persona-lab`、`knowledge-lab`、`context-lab`、`text-transform-lab`。

**模型接入**

- `model-connector-lab` —— 不出网的 provider 元数据、profile 校验、secret 脱敏、发现计划、兼容性报告。
- `model-provider-lab` —— 云 API adapter 实验室。覆盖 OpenAI / Anthropic / Gemini / OpenAI-compatible / OpenRouter / DeepSeek / xAI / Fireworks，提供请求构造、伪造调用、流式归一、live loopback 形状与各家 quirk。它不是平台模型抽象，也不是 API 网关。
- `model-routing-lab` —— 不做推理的 consumer-slot 绑定、路由计划、回退计划、参数归一。

**Agent 与推理**

- `pi-agent-runtime-lab` —— 参考 agent 包，no-network 的运行计划、trace 摘要、提案草稿、echo。
- `capability-tool-bridge-lab` —— 发现能力、预览权限、显式 provider 选择、调用 / 流式计划，并覆盖嵌套委派、target branch 写入、提示词注入、secret 外泄、出站扩张、大输出 redaction 等风险。
- `agentic-forge-lab` —— Agentic Forge 的核心包：能力包持有的运行生命周期、工作状态、计划图、scratch branch / candidate / compare / promote、推理节点（确定性 / 录制 / 云适配计划 / 本地 fake）、replay、输出校验、9 类失败 taxonomy。
- `inference-local-lab` —— 不依赖云 API、HTTP、bearer token 的本地 fake 推理 provider，证明推理接缝可以脱离这些。
- `inference-playtest-lab` —— Ygg-native 的「推理 → 提案 → 审视 → 批/拒 → 应用 → fork」纵切片。

**体验**

- `experience-runtime-lab` —— 体验运行时契约：体验描述符、状态投影、checkpoint、recovery、Play / Forge / Assist surface 绑定。
- `playable-creation-board` —— 第一个真实可玩的纵切片。包持有 board / module / constraint / marker 状态，14 个能力，4 个 surface。
- `experience-observability-lab` —— 包持有的可观测性：会话健康、能力包健康、agent 运行健康、提案因果链、cost / latency 摘要、失败面包屑、guardrail 摘要。
- `memory-lab` —— 长期记忆与知识：记录、检索、检索追踪、提案审批门控的更新、修正、forget / redaction、按分支视图、provenance。
- `sharing-lab` —— 分享与分发：composition bundle 导入导出、分支 / 会话 bundle 清单、包集 lockfile、兼容性报告、AI 披露元数据、只读分享清单、异步 fork 计划。不带市场、计费、签名网络。
- `playable-seed`、`blank-experience` —— 参考与最小体验。

**存储与外部项目**

- `storage-lab` —— 存储 / 数据契约预览：分层模型、backend class 候选、包级状态库、文档 CRUD 预览、blob 内容寻址契约证明、projection 物化、检索 / 向量 / 多模态 provider 契约。
- `tdb-retrieval-lab` —— TDB 作为检索 / 多模态 provider 的契约；不是事件日志权威。
- `project-intake-lab` —— 外部项目分类、栈检测、npm 生命周期风险、工作区计划、adapter 计划、wrapper / fixture / readiness 预览。不出网、不动文件系统。
- `workspace-lab` —— 工作区行动策略边界，10 项行动 taxonomy，deny-by-default 假执行器，确定性 fixture 工作区。

**第三方替换证明**

- `thirdparty/playable-seed`、`thirdparty/agent-runtime`、`thirdparty/agentic-forge`、`thirdparty/memory-lab` —— 证明对应官方包都可被第三方替换，没有官方优先级。

Forge profile (`profiles/forge-alpha.yaml`) 会自动加载这些包以及示例 fixture 包。

## TypeScript SDK

`sdk/typescript/` 下：

- `subprocess` —— 子进程能力包脚手架与模板运行时。
- `secure-execution` —— `secret_ref` 构造与校验、网络声明、出站审计、伪造流帧客户端。
- `inference-capability` —— transport-neutral 推理契约。
- `model-provider-adapter` —— 云 provider adapter helper。
- `agent-adapter` —— 把 Ygg 能力映射为 pi 风格 tool。
- `agentic-forge` —— 运行生命周期、计划图、工作状态、candidate / compare / promote、推理节点、tool bridge v2 helper。
- `experience-runtime` —— 体验运行时类型与构造器。
- `text-surface` —— 前端文字 surface helper（流式 buffer、frame 适配、滚动锚、字体加载）。

`text-surface`、`agentic-forge`、`inference-capability` 等都自带纯 TS 自测。

## Contract v1 与 SDK 生成

- `docs/spec/KERNEL_V1_CONTRACT.md` 是公开平台规范。
- `docs/spec/v1/schemas/` 是 SDK 与 conformance 的单一可信源：80 methods、59 events、22 top-level，共 161 个 schema。
- `sdk/typescript/contract-sdk/` 与 `sdk/rust/plurora-contract-sdk/` 由 schema 生成；TypeScript 包可通过 npm、工作空间路径或自行 codegen 使用。
- `plurora conformance package --contract v1 --path <package>` 提供第三方包 8 项验收检查。

## 包模板

`plurora init-package --template <name>`：`basic`、`experience`、`play-renderer`、`forge-panel`、`assistant-action`、`asset-editor`、`full-surface`、`networked`、`streaming`、`agent-runtime`、`experience-runtime`、`playable-board`、`playable-experience`。生成的包默认安全：no raw secret、不隐式联网。

## Web shell（`clients/web`）

平台用户面 chrome —— Home、Settings、Install 流程、Project frame、Toast 系统。基于 React 19 + Tailwind v4 + Motion + Radix + Phosphor 的 SPA，由 Vite 构建，路由 / modal 已 lazy-split。视觉规则与设计系统见 [`design/PLATFORM_UI_DESIGN.md`](design/PLATFORM_UI_DESIGN.md)；shell 详细文档见 [`../clients/web/README.md`](../clients/web/README.md)。

- **Home：** 项目货架（卡片网格 + 状态 pill + Hero + utility strip + 活动 timeline + 工坊工具 bento），数据来自 `host.project.list`，磁盘用量来自项目 `storage_summary`。Home 也消费结构化 shell descriptor：平台内置 quick actions 保留，包贡献的 `quick_action` / `workshop_card` / schema-versioned `home_card` 作为发现入口进入平台渲染器；包 action 首批只提示发现，不绕过 proposal / permission / audit。`⌘N` 打开 Install 模态。
- **Settings：** 六个面板都接真实数据。
  - API Connections —— `plurora/secret-store-lab/{list,put,delete}_secret` + health。UI 永远不读 raw secret 值，secret-edit modal 关闭时清掉输入态。
  - Installed Packages —— `host.package.list` + 项目标记 + Cmd/Ctrl+F focus。
  - Profiles —— `host.diagnostics`（active profile、packages_loaded、network allowlist）。
  - Storage —— storage area summary + 真实 event store kind（sqlite/postgres/memory），不在 Web UI 暴露 host 绝对路径。
  - Host Access —— 当前 root / device 身份、action scope、项目/目标资源选择器、委托链、HTTPS pairing link、pending 邀请、设备期限与级联 grant 撤销；默认邀请只选择 `observe`。
  - About —— 平台身份、license、links、致谢。
- **Install / Update 流程：** Install modal 通过 `capability.invoke` 调用 `plurora/install-lab` 的 `resolve_plan` / `detect_kind` / `execute_plan`；原生项目走快速通道，外部项目进入 wrap-vs-workspace wizard。项目控制台展示 bundle / package / event 诊断，并通过 `check_for_updates` / `update_project` 提供更新入口。没有 `host.install.*`。
- **Project Frame：** Home 以独立 `/project/<id>` 标签页打开项目；项目页没有平台顶栏或返回按钮，只用全屏 sandbox iframe 挂载项目自有前端。关闭标签页不停止项目；项目页用 `⌘ .` / `Ctrl .` 停止当前项目。
- **Failure Modal：** Deep Rust accent stripe、诊断 / 影响双列、redacted stderr 日志面板（含 Copy log）、Restart / Stop-and-uninstall / Close 三选项；数据来自 `host.package.list/status/logs`，不复制 raw log。
- **Toast 系统：** 5 个 variant（info/success/warning/error/progress），右下队列，`prefers-reduced-motion` 自动收敛。
- **响应式与暗色模式：** 显式 `data-theme` 切换（system/light/dark）；`@custom-variant dark` 把 Tailwind `dark:` 绑定到属性；modal overlay 用单独的 `--color-overlay` token 不随主题翻转；`prefers-reduced-motion` 收敛动效；`:focus-visible` 键盘导航 ring。
- **PWA 与移动控制：** manifest、service worker 和离线 app-shell 缓存已接入；`/pair` 在认证 gate 前展示邀请内容，领取后使用同源 device Cookie。移动 topbar 保留 Settings、语言与退出入口，不把 root token 写入设备存储。
- **SurfaceHost：** 通过 sandboxed iframe 挂载第三方 Web surface bundle；默认没有隐式平台权限，只有宿主显式配置的 bridge 能调用公开协议。Bridge 以 typed `allowed_capability_ids` 和方法 allowlist 限定可调用能力，stream 订阅归属绑定到发起 surface，诊断与日志脱敏，secret 输入态在关闭时清理，并通过 CSP/CORS 与短期 asset lease 保持 same-origin project 边界。流式订阅通过 postMessage 桥接 `capability/stream.*`。
- **没有官方包特权通道——shell 和别的客户端一样是公开协议的客户端；调用平台工具包时也走普通 `capability.invoke`。**

## 桌面与发布

- `clients/desktop` 提供 Tauri 2.x wrapper，并拥有 loopback-only managed Host sidecar。启动时生成 root token 与一次性 bootstrap nonce，以随机端口启动 `host serve`，等待健康后导航隐藏窗口；使用持久 SQLite desktop profile、打包的 Web dist 与依赖顺序明确的官方包 autoload，退出时停止子进程。Web/PWA 与 Desktop 复用同一客户端核心和公开 Host 边界。
- GitHub Actions CI 与 `v*` tag release workflow 已落地，构建跨平台 Tauri 安装包并创建 draft release。`scripts/release-version.sh` 同步 Cargo、Web package、desktop package 与 Tauri 配置。
- 构建说明见 [`../BUILDING.md`](../BUILDING.md)；变更记录见 [`../CHANGELOG.md`](../CHANGELOG.md)。签名、公证、自动更新未启用。

## 创作流程

- `plurora init-package` 生成 Python 或 TypeScript 子进程包脚手架。`--template` 控制 surface 描述符；`--language *-experience` 在不指定模板时仍生成旧版 4-surface 体验，兼容旧行为。
- `plurora init-composition` + `plurora composition check` 提供本地 composition 流程，支持 v2 字段（标题、描述、可选包、所需能力、默认激活、权限期望、替换候选、兼容性说明）。
- `plurora package check` 输出结构化诊断：入口类型、信任级别、能力数、按 slot 分组的 surface、权限摘要、沙箱策略；对无能力或无 surface 的包给出警告。
- `plurora package conformance` 在本地验证生成的包。
- `plurora package reload <manifest>` 把包加载进内存运行时、重启（仅子进程）、输出前后状态和日志数、再卸载。
- `plurora package run-fixture` 用确定性 fixture 输入调用所有非流式能力，输出 JSON 摘要。
- `plurora play-create-demo` 端到端跑通空白游创循环。
- `plurora perf baseline` 跑确定性性能基线（in-process 调用、官方能力调用、事件存储 append/list/range、composition check、profile 加载、子进程 echo），输出文本或 JSON。详见 [`performance/BASELINE.md`](performance/BASELINE.md)。

## 代码组织

- `crates/plurora-cli/src/main.rs` 是薄入口。CLI 类型在 `cli.rs`，命令在 `commands/`，包模板在 `templates/`。conformance runner 与 case registry 已拆分：`conformance/runner.rs` 负责 `--list`、`--case`、`--tag`、`--fail-fast`、`--slowest`，`conformance/registry/` 按域注册 473 个 `ConformanceCase { id, tags, run }`。
- `crates/plurora-cli/src/schema_export/` 负责 v1 schema 导出；`src/bin/export-schemas.rs` 只是薄入口。生成文件仍只来自 exporter，不手改 SDK 或 schema。
- `crates/plurora-runtime/src/runtime/` 按 session、events、packages、capabilities、hooks、permissions、assets、branches、projections、proposals 分模块；`runtime/protocol_dispatch.rs` 只保留 public router，具体 public protocol 处理器在 `runtime/protocol/` 下按 domain 拆分。`runtime/mod.rs` 保持公开 `Runtime<S>` API。
- 协议方法的元数据与分发共享 `PlatformMethod` 这一份事实来源，并有注册表 / 分发的一致性单测。
- `crates/plurora-runtime/src/inproc/` 把官方包行为按域拆开；`plurora/install-lab` 已拆成 `install_lab/` 子模块（types/source/planner/executor/layout/project_kind/fs_copy），公共 helper 走 provider package + 本地能力名路由，不再用 suffix-only 兜底。
- `clients/web` 的 Home 与 Install flow 已拆成 page shell + hooks/helpers/step components；UI 继续只走公开协议，不读本地文件系统或 runtime 私有状态。

这些拆分不改变行为，只是让后续新增能力包、conformance、handler 与 UI flow 时仍然可审查。

## Conformance

`cargo run -p plurora-cli -- conformance` 跑 473 个具名 CLI 用例。支持：

- `--list` 列出 id 与 tag；
- `--case <pattern>` 子串过滤；
- `--tag <tag>` 按 tag 过滤；
- `--fail-fast` 首个失败即停；
- `--slowest <N>` 显示最慢 N 个。

每个用例有 tag（runtime / event / capability / package / subprocess / official / network / outbound / stream / agentic / experience / memory / sharing / secret / composition / replacement / surface / protocol / permission / hook / host / asset / projection / substrate / storage / live / external_project / project_intake / workspace_lab / retrieval 等）。详见 [`performance/CONFORMANCE_FEEDBACK.md`](performance/CONFORMANCE_FEEDBACK.md)。

外加 `cargo test --workspace` 下的 crate 与 service 单测，以及 `npm run check --prefix clients/web` / `npm run build --prefix clients/web` 检查 Web shell。

## Partial（已开始但未做完）

- 能力包身份的 `event.subscribe` 权限。
- 能力包持有的钩子处理器的超时 / 错误审计。
- 能力 provider 的持久选择策略（超出单次调用显式选择）。
- 更丰富的资源策略（filesystem 强制矩阵）。
- 内容寻址的资产 blob 存储与能力包身份的资产权限：稳定的 content-address helper 与元数据约定已完成；完整 blob 存储与运行时权限执行未完成。
- 包持有的 projection 执行。
- 更丰富的失败监控与健康检查。
- 更广的传输一致性覆盖。
- Desktop release code signing / notarization、auto-updater、真实应用图标，以及 managed Host 的更丰富崩溃恢复与 sidecar 更新协调。
- Surface lifecycle callback（如 `onClose`、`onProposalDraft`）与跨源 surface bundle allowlist。
- `context.get|list`、`host.package.describe`、`capability.describe`、`protocol.extension.describe`、`identity.current`、`host.ping` 完整暴露。

## Deferred（明确不在内核范围）

这些都将以普通能力包或后续工作出现，不属于内核：

- 对话运行时、提示词、模型、采样、消息 / 回合语义。
- 记忆模型、检索、摘要、agent 循环、导演。
- 世界、场景、角色、规则、骰子、背包语义。
- SillyTavern 兼容由独立项目 YdlTavern 承担，跑在 Plurora 之上（见 [`tavern/TAVERN_COMPAT.md`](tavern/TAVERN_COMPAT.md)）。
- 生产级长期自治 agent、多 agent 协作、生产级记忆系统、更完整的 live-ops。
- 外部游戏引擎桥接（UE5、Godot、Unity、Web 客户端）。
- 市场、包签名、依赖解析（本地分享 proof 已完成，见 [`guides/SHARING_DISTRIBUTION.md`](guides/SHARING_DISTRIBUTION.md)）。
- 最终 UI 视觉设计、完整 Studio、ComfyUI 风格节点编辑器。
- WASM 与远程包执行。

## 如何核对这份快照

```bash
cargo test --workspace
cargo run -p plurora-cli -- conformance
cargo run -p plurora-cli -- conformance --list
cargo run -p plurora-cli -- conformance --tag sharing --slowest 3
cargo run -p plurora-cli -- play-create-demo
npm run check --prefix clients/web
npm run build --prefix clients/web
```

任何一步失败时，以代码为准，更新这份文档。

## 延伸阅读

- [`CHARTER.md`](CHARTER.md) —— 不变的根本原则。
- [`architecture/`](architecture/README.md) —— 架构、内核、能力包契约、扩展点、事件、生命周期。
- [`product/`](product/README.md) —— 游创立场。
- [`protocol/PROTOCOL_V0.md`](protocol/PROTOCOL_V0.md) —— 公开协议。
- [`spec/`](spec/README.md) —— 可执行契约矩阵与 conformance 路线图。
- [`guides/`](guides/README.md) —— 能力包创作指南。
- [`roadmap/NEXT_STEPS.md`](roadmap/NEXT_STEPS.md) —— 下一步。
