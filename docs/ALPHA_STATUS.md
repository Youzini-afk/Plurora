# 平台现状

> [English](./ALPHA_STATUS.en.md) · [中文](./ALPHA_STATUS.md)

这是 Plurora 当前实现状态的快照。它记录代码、公开合同和质量检查已经支持什么，明确标注 partial 或 deferred 的除外；它不负责定义平台为什么存在或未来只能往哪里发展。

平台目标与原则见 [`CHARTER.md`](CHARTER.md) 和 [`architecture/VISION.md`](architecture/VISION.md)；官方产品责任见 [`product/PLATFORM_PRODUCT_MODEL.md`](product/PLATFORM_PRODUCT_MODEL.md)，游创只是可选 Profile。建设方向见 [`roadmap/NEXT_STEPS.md`](roadmap/NEXT_STEPS.md)。

## 概要

- **Conformance：** 具名 CLI conformance 外加 crate / service 单元测试持续通过；200 个 v1 schema（92 methods + 69 events + 39 top-level）通过校验；Phase 5 Powerbox 另有 6 个具名 conformance case。
- **章程纪律：** 内核对内容无意见；第一方 Package 没有特权；公开协议是唯一入口；入口形态平等；能力句柄、bindings 注入、Path A / Path B、conformance kit 与生成 SDK 已落地；可信路径阻断 raw secret，全部走 manifest 声明的 `secret_ref`；权限授权可重新水化；网络声明带审计与脱敏；通用流式与取消生命周期；外发执行有边界，默认全拒；公开 HTTPS 出站走同样的 host policy / 审计 / 脱敏边界；一元、SSE/NDJSON/raw 流和 WebSocket 三个原语都有完成审计事件。
- **代码健康：** CLI、运行时各域行为、协议分发、in-process 处理器、事件存储——都已按域拆分，不再继续往单文件里堆。
- **人测底座：** Work source 可安全 pack 为内容寻址 WorkRevision、AssemblyRevision 与 AssemblyLock；Install Lab 产出 Installation candidate，`host.installation.*` 以 durable journal、幂等键和 revision CAS 创建、更新与移除 Installation；Workspace 与 Installation 分离，linked-local source 永不删除。RunRegistry、`host.run.*` 与 Run lifecycle events 已以独立 durable journal 落地；Library 通过显式 Run action 展示启动、状态和停止，打开详情不会隐式运行。Phase 5 ExposureRegistry、Powerbox candidate disclosure、lease/revoke、runtime handle injection 与 provider stop/revoke/expiry/version-drift invalidation 已落地；Surface bridge 继续使用 allowlist、stream ownership、诊断脱敏、secret 输入清理与 CSP/CORS；managed Realization 仍由后续 Phase 6 负责。

当前已经形成较大的可运行面，但平台和官方产品都不等于“完成”。后续建设同时关注开放性、多样性、先进执行与协议能力、长期数据演化，以及普通用户和创作者的完整体验。

## 内核

- 不带内容的会话、只追加的不透明事件、清单驱动的能力包、能力机制、钩子机制、surface 贡献、提案生命周期、资产 / 分支 / projection 底座。
- SQLite 事件日志，每会话单调递增的序号，可重新水化的底座。
- Experimental SHA-256 ObjectStore 与 ArtifactDescriptor 已落地：内存/文件系统 CAS、读取后校验、stream、较早 FNV asset record 的幂等转换；asset 事件只保存 descriptor/ref，不再保存正文。
- Experimental EffectReceipt 与 Change primitive 已落地：capability/outbound/stream/WebSocket/exec terminal path 生成内容寻址 receipt；历史 replay 不调用 executor；capability re-execute 创建新 branch 与 parent-linked receipt；当前 `change.proposal.*` facade 把 approval-gated operation 映射为 Intent/ChangeSet/PolicyDecision/Commit evidence。
- Experimental Protocol Commons 注册表已落地：`host.info` 发布 Change、Shell Default、World Bundle、Work 与 Assembly 五个描述符；显式协议/Profile 协商先于 dispatch；不支持的 major 以结构化原因拒绝；protocol、implementation 与 package conformance 使用独立可执行报告。
- `plurora-work` 已提供内容无关的便携 Work / Assembly / Port / State Slot、Rights / Transparency、Operational Intent / Target Inventory、Installation / Run / Exposure 与 Realization wire 模型，并实现安全 source reader、Package/Foreign/content 归一化、capability-to-Port 投影、带 digest memoization 的递归 resolver、普通 Component Adapter 与有界结构化诊断。`plurora work init|check|pack|inspect` 将 source 与完整 AssemblyLock 闭包内容寻址；旧 Composition 机器身份已移除。
- Host Installation lifecycle 已替换旧实例模型：EventStore journal 是唯一 mutable authority，`installation.json` 只是可重建 projection；create/update/remove 支持幂等重放、revision CAS、显式 state keep/delete、state snapshot 与失败回滚。数据布局分为 `objects/`、`installations/`、`workspaces/` 与 `runtime/`，启动不读取旧目录。
- Package envelope 与 component identity 已分离：显式 component/behavior digest 在重新打包后保持稳定；runtime 与 effect evidence 携带 component trust/边界数据；AssemblyLock 分离 component/profile/content pin；`contract:none` 明确报告为不可移植 Foreign Capsule。
- Experimental World Bundle 已落地，并由 `plurora/playable-creation-board` 的跨 Host conformance fixture 覆盖：canonical archive descriptor 保留原始 v1 envelope 与完整 SHA-256 closure；全新 SQLite/filesystem Host 保持 object、lineage 与 receipt；historical replay 不调用 executor；替代实现生成 child branch/head；headless CLI 无需 Web Shell 状态即可读取同一 archive。
- 用 JSON Schema 子集校验能力 I/O 与能力包声明的事件 payload。
- Contract V1 身份 union 继续保持 `host_admin`、`host_dev`、`package`、`human`、`assistant`、`anonymous`。配对设备在远程 RPC 边界使用 fail-closed 的 `anonymous` V1 sentinel，并通过 Host 建立的 authority envelope 保留 grant、delegation 与资源约束；旧 runtime 忽略新 envelope 时只会拒绝而不会扩大权限。脱敏 Host 控制面审计仍以逻辑 `host_device` 记录设备；human 与 assistant 身份支持作用域授权。
- 审计事件：`authority/grant.created|revoked`、`authority/denied`、`host/package.*` 生命周期与 `change/proposal.*` 生命周期；Contract V1 之外另有脱敏的 `host/control/v1/authority.decision` Host 控制面授权判定日志。
- 持久授权：grant / revoke 事件可在 SQLite-backed 运行时中重新水化。
- Contract V1 是公开平台规范：92 个协议方法、69 个事件类型、200 个 JSON Schema。`host.run.*`、Run lifecycle events、`host.exposure.*`、`host.binding.*`、Exposure/Binding lifecycle events、`authority.handle.*`、`host.package.audit`、能力句柄、bindings 注入、Path B、conformance kit 与 SDK 生成均为 implemented。

## 安全执行

- **`secret_ref` 引用：** 支持 `secret_ref:<vault>:<key>`、`secretRef:`、`secret-ref:`、`host:` 各种前缀。能力包通过引用提及 secret，原始值不出现在事件、提案、日志、审计里。
- **环境变量解析器：** host 拥有的解析器，带显式 allowlist。默认全拒；env 名要先放行才能解析。错误只带 env 名，绝不带原始值。
- **本地加密 secret store：** `secret_ref:store:NAME` 通过 `StoreSecretResolver` 从 `~/.plurora/secrets.dat` 解析；`secret_ref:installation:NAME` 先读当前 Installation 的 `secrets.dat`，再按 `secret_policy.allow_platform_fallback` 决定是否回退平台 store；store 使用 age (X25519) 加密，主密钥来自 OS keyring（延后启用）或 0600 本地 key 文件。
- **Raw secret 阻断：** 提案的 operations / expected effects 与资产 metadata 会被保守扫描，明显的 API key、token、password 字段被拒绝。资产内容与普通文本不扫描，避免误伤用户内容。
- **网络权限声明：** 清单中的 `permissions.network` 同时支持扁平 `hosts`（向后兼容）和结构化 `declarations`（带 `host` / `methods` / `purpose`）。无声明的 Package 不能出网；第一方 Package 也没有绕过。
- **外发审计与脱敏：** 每条出站请求都生成审计记录，只含身份、能力包 id、目标主机、方法、用途、脱敏状态、用到的 `secret_ref`，不含原始 body / header / 提示词 / 响应。
- **外发执行边界：** 内容无关的 HTTP 与 WebSocket executor trait。默认 deny-all（fail-closed），可切换为 fake executor（带 fixture，用于 conformance）或 live executor（HTTP 使用 reqwest + rustls，WebSocket 使用 tokio-tungstenite + rustls；默认关闭；HTTP 为 HTTPS-only，WebSocket 为 WSS-only；重定向 fail-closed；secret header 只在执行时注入，不进审计）。真实 live 模型 / WebSocket 出站必须通过 profile 与环境变量显式 opt-in；默认 conformance 不联网，真实 WebSocket smoke 还要求 `PLURORA_LIVE_WEBSOCKET_TESTS=1`。
- **协议方法：** `host.outbound.audit` 列出某个能力包的出站审计事件；`host.outbound.execute` 让普通能力包通过 host executor 发起一元出站请求；`host.outbound.stream` 提供 SSE/NDJSON/raw 流式出站；`host.outbound.websocket.open|send|close` 提供双向 WebSocket 出站。
- **完成审计事件：** `host/outbound.execute.completed`、`host/outbound.stream.completed`、`host/outbound.websocket.completed` 覆盖三种出站原语；事件只记录状态、计数、耗时、executor kind、network_performed、redaction state 与 `secret_ref` 引用。
- **流式生命周期：** 流注册表跟踪进行中的流式调用，按序发出 `capability/stream.started|chunk|progress|ended|error|cancelled|timeout`。取消和超时阻断后续 chunk。非流式能力被拒绝。

## 公开协议与传输

- 规范的请求 / 响应信封，自带 host 绑定的身份上下文。调用方不能自己声称是某个能力包或 admin。
- 同一份 dispatcher 同时承载 HTTP `POST /rpc` 和 host JSON-RPC stdio (`plurora host-stdio`)。
- Contract Registry `0.1.0` 发布 92 个精确 owner-based method ID、两个显式 contract profile、逐层版本要求与 fail-closed Protocol Commons negotiation。HTTP RPC、Host stdio、in-process 调用与 subprocess reverse stdio 使用同一个精确 resolver；生成 SDK 对每个 wire ID 只暴露一个 method identity，Web client 也只发送这些 ID。
- 通过 SSE 订阅事件，支持 `after_sequence` 回放和实时追尾。
- 基于 profile 的 `plurora host serve` 自动加载能力包，对外暴露 `/rpc` 与 SSE。
- Host 控制平面在 Contract V1 之外保持独立：root token 是根凭据；持久化设备 grant 同时按 action scope 与 Work / Workspace / Installation / Run / Target / Exposure / Binding / Realization 资源选择器衰减，支持有界委托、祖先撤销级联、过期、单项撤销和原子的管理员批量撤销。HTTP 与 RPC 在进入运行时前保留同一设备身份和 authority；每次设备协议调用都会写入脱敏的 allow/deny 判定日志。移动 PWA 与 `plurora host access` CLI 通过同一 Host API 管理授权，pairing 仍只经 HTTPS 一次性交换为 Secure/HttpOnly Cookie。详见 [`architecture/HOST_REMOTE_ACCESS.md`](architecture/HOST_REMOTE_ACCESS.md)。
- TCP 传输留作后续工作；WASM 与远程入口在清单中已是一等形式，执行延后。

## 包执行

- `rust_inproc` 包通过 host 提供的 trait 与 catalog 执行。声明了 in-process provider 但 catalog 里找不到的清单会被拒绝。
- `subprocess` 包通过 stdio 上的 JSON-RPC 执行：握手、调用、超时、degraded 状态、重启、卸载即杀、stderr 日志捕获。
- `wasm` 与 `remote` 入口：清单已支持，执行延后。
- 路径 A (`entry.contract: "v1"`) 接收能力句柄 bindings 并接受权限强制；路径 B (`entry.contract: "none"`) 自包含运行，不接收 v1 权威，但生命周期仍可观察。
- 能力路由支持显式 provider 选择，以及精确匹配和 `^x.y` 简单版本约束。歧义时拒绝，除非调用方指定 `provider_package_id`。
- Hook fabric：为 `journal/before_append|after_append` 与 `capability/before_invoke|after_invoke` 提供确定性 subscription 排序、veto 报告、metadata/input 修改路径与卸载清理。任意 handler capability 的通用执行、独立 async delivery 与每个 handler 的 quota 尚未完成。

## 底座

- 资产注册表：不透明的 `id` / `mime` / `hash` / `size` / `origin_package_id` / `metadata`，可从 SQLite 重新水化。权限执行与内容寻址 blob 存储留待后续。
- 会话 fork / 分支沿革，可从事件日志重新水化。
- 通用 projection 注册表：通过 `kind_prefix` 与 `writer_package_id` 过滤事件来重建，写入 `projection/updated`。包持有的 projection 执行留待后续。
- Installation 控制面：EventStore journal、可重建 projection、ObjectStore artifact 校验、显式 state action、restart rehydrate，以及 `plurora installation list/info/create/update/remove` 已落地。
- 部署运行时：`host.target.*`、`host.exec.*`、`host.port.*`、`host.proxy.*` 已落地；默认 deny-all，profile 可显式启用 `LiveLocalExecExecutor`；内置 `local` 与 enrolled Agent 使用相同的 durable operation/artifact/verifier/deployment receipt 合同，端口只绑定 loopback，proxy upstream 必须引用 active port lease，Agent 流量只经认证 tunnel 返回 Host proxy。`ProxyRouteAccess` 默认 `host_authenticated`，只有显式 `public` route 才启用可选 `<slug>.apps.<host>/` 免 Host 认证 vhost，`/p/<route_id>/...` 始终保留在 Host auth 内。Web 项目控制台支持显式 Docker Deploy / Stop、Dockerfile / nixpacks Build & Deploy，以及由 immutable build-context artifact 驱动的 verified ChangeSet private preview、独立审批、`VerifiedActivate` revision、显式 reconcile/recover/rollback；verified replay 在记录的 target 上重建，不读取 live workspace 或重新抓取源码。真实 MDN 仓库与结构不同的 Python fixture 已在 GitHub CI 覆盖故障、Host restart 和 rollback。
- Surface 贡献：带版本、slot、激活方式、所需权限、审批策略、metadata 的描述符。Slot 包括 `experience_entry`、`home_card`、`quick_action`、`workshop_card`、`play_renderer`、`forge_panel`、`asset_editor`、`assistant_action`。`quick_action`、`workshop_card` 与带 `metadata.shell_schema_version: 1` 的 `home_card` 是结构化 shell descriptor：Web shell 只读取受限文本、icon hint、排序和同包 target，由平台渲染；不加载包 JS、不解析 HTML、不 mount iframe。复杂项目 surface 继续走 `surface_bundle` + sandbox iframe。通过 `shell.contribution.list` 与 `.describe` 发现。
- Surface bundle：`surface_bundle` 是清单里的静态浏览器 bundle 入口，不是可执行 package entry；bundle 按 Package source 解析，原始路径要求 Host 身份。opaque-origin sandbox 获得绑定 grant/bundle root 的五分钟 `/surface-assets/<lease>/...` 只读句柄，不携带 Host credential。
- 提案生命周期：`change.proposal.create|get|list|approve|reject|apply`。当前 `apply` 只跑通用操作 `asset.put` 与 `projection.rebuild`。更广泛的事务和回滚留待后续。

## Work、Workspace 与 Installation

| 能力 | 状态 |
|---|---|
| `plurora work init/check/pack/inspect` | implemented |
| Work / Assembly source、Package、Foreign 与 content-only 归一化 | implemented |
| 递归 resolver、显式 Adapter、Port / Binding 诊断 | implemented |
| AssemblyLock 与完整内容寻址闭包 | implemented |
| `plurora/install-lab` Work candidate 编排 | implemented |
| ObjectStore 持久化 Work / Assembly / Lock closure | implemented |
| `plurora installation list/info/create/update/remove` | implemented |
| `host.installation.list/get/create/update/remove` | implemented |
| `host/installation.created/updated/removed` | implemented |
| Installation journal / projection restart rehydrate | implemented |
| idempotency fingerprint 与 revision CAS | implemented |
| update state snapshot、显式 migration/reset、失败回滚 | implemented |
| remove `keep` / `delete` state decision | implemented |
| linked-local source 永不删除 | implemented |
| managed Workspace containment / symlink 防护 | implemented |
| `objects/`、`installations/`、`workspaces/`、`runtime/` 新布局 | implemented |
| `secret_ref:installation:NAME` + 可选平台 fallback | implemented |
| Web Home Installation cards / detail | implemented |
| Run lifecycle（`host.run.*` 与 5 个 Run events） | implemented |
| Exposure lifecycle | implemented |
| Binding lifecycle / Powerbox | implemented |
| Planner / Realization | Phase 6 planned |

Work pack 与 Installation create 分离：pack 只产生不可变 artifact；create/update/remove 才修改 Host journal。Workspace 位置是 Host-local 绑定，不进入便携 Work identity。

## 真实模型端到端路径

| 能力 | 状态 |
|---|---|
| `huggingface-fetcher` 测试通过 | implemented |
| Surface bundle 解析由 metadata 驱动 | implemented |
| `host.surface.bundle.resolve` | implemented |
| 需 Host 身份的 `/surface-bundles/<prefix>/<file>` 路由 | implemented |
| grant/root-bound 五分钟 `/surface-assets/<lease>/...` sandbox 句柄 | implemented |
| Installation-scoped secret 与 authority context | implemented |
| Installation → Run 启动与停止 | implemented |
| Exposure lifecycle / Binding lifecycle | implemented |
| Surface 通过 `initialProps` 接收 `session_id` | implemented |
| TavernProvider.sendMessage 调用 engine `model.live_call` | implemented |
| API Connections 抽屉支持 platform / Installation 范围切换 | implemented |
| Engine manifest 声明 `secret_ref:installation:*` | implemented |
| Surface 流式响应 UX | implemented |
| Surface-host stream postMessage 协议 | implemented |
| Surface bridge allowlist / stream ownership / diagnostics redaction / secret input cleanup / CSP/CORS hardening | implemented |
| `streamCapability` helper（YdlTavern host-rpc） | implemented |
| `AsyncIterable<StreamFrame>` 消费 + iterator early-return 清理 | implemented |
| `cancelGeneration` action + Stop 按钮 | implemented |
| 单 chat 多并发生成 | deferred |
| token-rate UI | deferred |
| Realtime / WebSocket streaming UX | deferred |

## 第一方 capability Package

全部是普通能力包，没有内核特权。位于 `packages/plurora/`，通过普通清单加载。

**平台基础**

- `package-lab`、`schema-tools`、`event-tools`、`asset-lab`、`projection-lab`、`assistant-lab`。
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
- `inference-playtest-lab` —— Plurora-native 的「推理 → 提案 → 审视 → 批/拒 → 应用 → fork」纵切片。

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

- `thirdparty/playable-seed`、`thirdparty/agent-runtime`、`thirdparty/agentic-forge`、`thirdparty/memory-lab` —— 证明对应第一方 Package 都可被第三方替换，没有 publisher priority。

Forge profile (`profiles/forge-alpha.yaml`) 会自动加载这些包以及示例 fixture 包。

## TypeScript SDK

`sdk/typescript/` 下：

- `subprocess` —— 子进程能力包脚手架与模板运行时。
- `secure-execution` —— `secret_ref` 构造与校验、网络声明、出站审计、伪造流帧客户端。
- `inference-capability` —— transport-neutral 推理契约。
- `model-provider-adapter` —— 云 provider adapter helper。
- `agent-adapter` —— 把 Plurora 能力映射为 pi 风格 tool。
- `agentic-forge` —— 运行生命周期、计划图、工作状态、candidate / compare / promote、推理节点、tool bridge v2 helper。
- `experience-runtime` —— 体验运行时类型与构造器。
- `text-surface` —— 前端文字 surface helper（流式 buffer、frame 适配、滚动锚、字体加载）。

`text-surface`、`agentic-forge`、`inference-capability` 等都自带纯 TS 自测。

## Contract v1 与 SDK 生成

- `docs/spec/PUBLIC_CONTRACT.md` 是公开平台规范。
- `docs/spec/v1/schemas/` 是 SDK 与 conformance 的单一可信源：92 methods、69 events、39 top-level，共 200 个 schema；Phase 5 生成链校验 hash 为 `9c3923c6ffd365a3b7a0a5e64a177a9d89708e70e215e454bfd9b1a5098d6e94`。
- `sdk/typescript/contract-sdk/` 与 `sdk/rust/plurora-contract-sdk/` 由 schema 生成；TypeScript 包可通过 npm、工作空间路径或自行 codegen 使用。
- `plurora conformance package --contract v1 --path <package>` 提供第三方包 8 项验收检查。

## 包模板

`plurora init-package --template <name>`：`basic`、`experience`、`play-renderer`、`forge-panel`、`assistant-action`、`asset-editor`、`full-surface`、`networked`、`streaming`、`agent-runtime`、`experience-runtime`、`playable-board`、`playable-experience`。生成的包默认安全：no raw secret、不隐式联网。

## Web shell（`clients/web`）

平台用户面 chrome —— Home、Settings、Installation 流程、Installation frame、Toast 系统。基于 React 19 + Tailwind v4 + Motion + Radix + Phosphor 的 SPA，由 Vite 构建，路由 / modal 已 lazy-split。视觉规则与设计系统见 [`design/PLATFORM_UI_DESIGN.md`](design/PLATFORM_UI_DESIGN.md)；shell 详细文档见 [`../clients/web/README.md`](../clients/web/README.md)。

- **Home：** Installation 货架数据来自 `host.installation.list`；detail 使用 `host.installation.get`。Home 也消费结构化 shell descriptor；包 action 只提供发现入口，不绕过 proposal / permission / audit。`⌘N` 打开 Installation 创建模态。
- **Settings：** 六个面板都接真实数据。
  - API Connections —— `plurora/secret-store-lab/{list,put,delete}_secret` + health。UI 永远不读 raw secret 值，secret-edit modal 关闭时清掉输入态。
  - Installed Packages —— `host.package.list` + Cmd/Ctrl+F focus。
  - Profiles —— `host.diagnostics`（active profile、packages_loaded、network allowlist）。
  - Storage —— storage area summary + 真实 event store kind（sqlite/postgres/memory），不在 Web UI 暴露 host 绝对路径。
  - Host Access —— 当前 root / device 身份、action scope、Work / Workspace / Installation / Run / Target / Exposure / Binding / Realization 资源选择器、委托链、HTTPS pairing link、pending 邀请、设备期限与级联 grant 撤销；默认邀请只选择 `observe`。
  - About —— 平台身份、license、links、致谢。
- **Installation 流程：** UI 通过 `host.installation.create|update|remove` 使用真实 public DTO；update 带 expected revision 与显式 state action，remove 必须选择 keep/delete。
- **Installation Frame：** `/installation/<id>` 展示 Installation detail、Library affordance 与当前 Run 状态。启动和停止通过显式 `host.run.start` / `host.run.stop`，缺少本地匹配实现或需要 Realization 时显示结构化 gap 与下一步；打开页面不自动启动 Run。
- **Failure Modal：** Deep Rust accent stripe、诊断 / 影响双列、redacted stderr 日志面板（含 Copy log）、Restart / Stop-and-uninstall / Close 三选项；数据来自 `host.package.list/status/logs`，不复制 raw log。
- **Toast 系统：** 5 个 variant（info/success/warning/error/progress），右下队列，`prefers-reduced-motion` 自动收敛。
- **响应式与暗色模式：** 显式 `data-theme` 切换（system/light/dark）；`@custom-variant dark` 把 Tailwind `dark:` 绑定到属性；modal overlay 用单独的 `--color-overlay` token 不随主题翻转；`prefers-reduced-motion` 收敛动效；`:focus-visible` 键盘导航 ring。
- **PWA 与移动控制：** manifest、service worker 和离线 app-shell 缓存已接入；`/pair` 在认证 gate 前展示邀请内容，领取后使用同源 device Cookie。移动 topbar 保留 Settings、语言与退出入口，不把 root token 写入设备存储。
- **SurfaceHost：** 通过 sandboxed iframe 挂载第三方 Web surface bundle；默认没有隐式平台权限，只有宿主显式配置的 bridge 能调用公开协议。Bridge 以 typed `allowed_capability_ids` 和方法 allowlist 限定可调用能力，stream 订阅归属绑定到发起 surface，诊断与日志脱敏，secret 输入态在关闭时清理，并通过 CSP/CORS 与短期 asset lease 保持 same-origin project 边界。流式订阅通过 postMessage 桥接 `capability/stream.*`。
- **没有第一方 Package 特权通道——shell 和别的客户端一样是公开协议的客户端；调用平台工具包时也走普通 `capability.invoke`。**

## 桌面与发布

- `clients/desktop` 提供 Tauri 2.x wrapper，并拥有 loopback-only managed Host sidecar。启动时生成 root token 与一次性 bootstrap nonce，以随机端口启动 `host serve`，等待健康后导航隐藏窗口；使用持久 SQLite desktop profile、打包的 Web dist 与依赖顺序明确的第一方 Package autoload，退出时停止子进程。Web/PWA 与 Desktop 复用同一客户端核心和公开 Host 边界。
- GitHub Actions CI 与 `v*` tag release workflow 已落地，构建跨平台 Tauri 安装包并创建 draft release。`scripts/release-version.sh` 同步 Cargo、Web package、desktop package 与 Tauri 配置。
- 构建说明见 [`../BUILDING.md`](../BUILDING.md)；变更记录见 [`../CHANGELOG.md`](../CHANGELOG.md)。签名、公证、自动更新未启用。

## 创作流程

- `plurora init-package` 生成 Python 或 TypeScript 子进程包脚手架。`--template` 控制 surface 描述符；`--language *-experience` 在不指定模板时仍生成旧版 4-surface 体验，兼容旧行为。
- `plurora work init/check/pack/inspect` 提供本地 Work/Assembly 创作流程：安全读取 `work.yaml` / `assembly.yaml` 与 Package/Component source，校验递归 Port/Binding，生成内容寻址 WorkRevision、AssemblyRevision 与 authoring AssemblyLock。`pack` 只写 ObjectStore，不自动安装或运行。
- `plurora package check` 输出结构化诊断：入口类型、信任级别、能力数、按 slot 分组的 surface、权限摘要、沙箱策略；对无能力或无 surface 的包给出警告。
- `plurora package conformance` 在本地验证生成的包。
- `plurora package reload <manifest>` 把包加载进内存运行时、重启（仅子进程）、输出前后状态和日志数、再卸载。
- `plurora package run-fixture` 用确定性 fixture 输入调用所有非流式能力，输出 JSON 摘要。
- `plurora play-create-demo` 端到端跑通空白游创循环。
- `plurora perf baseline` 跑确定性性能基线（in-process 调用、官方能力调用、事件存储 append/list/range、Work check、profile 加载、子进程 echo），输出文本或 JSON。详见 [`performance/BASELINE.md`](performance/BASELINE.md)。

## 代码组织

- `crates/plurora-cli/src/main.rs` 是薄入口。CLI 类型在 `cli.rs`，命令在 `commands/`，包模板在 `templates/`。conformance runner 与 case registry 已拆分：`conformance/runner.rs` 负责 `--list`、`--case`、`--tag`、`--fail-fast`、`--slowest`，`conformance/registry/` 按域注册 453 个 `ConformanceCase { id, tags, run }`。
- `crates/plurora-cli/src/schema_export/` 负责 v1 schema 导出；`src/bin/export-schemas.rs` 只是薄入口。生成文件仍只来自 exporter，不手改 SDK 或 schema。
- `crates/plurora-runtime/src/runtime/` 按 session、events、packages、capabilities、hooks、permissions、assets、branches、projections、proposals 分模块；`runtime/protocol_dispatch.rs` 只保留 public router，具体 public protocol 处理器在 `runtime/protocol/` 下按 domain 拆分。`runtime/mod.rs` 保持公开 `Runtime<S>` API。
- 协议方法的元数据与分发共享 `PlatformMethod` 这一份事实来源，并有注册表 / 分发的一致性单测。
- `crates/plurora-runtime/src/inproc/` 把第一方 Package 行为按域拆开；`plurora/install-lab` 已拆成 `install_lab/` 子模块（types/source/detection/planner/candidate/executor/layout/intake/fs_copy），公共 helper 走 provider package + 本地能力名路由，不再用 suffix-only 兜底。
- `clients/web` 的 Home 与 Install flow 已拆成 page shell + hooks/helpers/step components；UI 继续只走公开协议，不读本地文件系统或 runtime 私有状态。

这些拆分不改变行为，只是让后续新增能力包、conformance、handler 与 UI flow 时仍然可审查。

## Conformance

`cargo run -p plurora-cli -- conformance` 跑 453 个具名 CLI 用例。支持：

- `--list` 列出 id 与 tag；
- `--case <pattern>` 子串过滤；
- `--tag <tag>` 按 tag 过滤；
- `--fail-fast` 首个失败即停；
- `--slowest <N>` 显示最慢 N 个。

每个用例有 tag（runtime / event / capability / package / subprocess / first_party / network / outbound / stream / agentic / experience / memory / sharing / secret / composition / replacement / surface / protocol / permission / hook / host / asset / projection / substrate / storage / live / external_project / project_intake / workspace_lab / retrieval 等）。详见 [`performance/CONFORMANCE_FEEDBACK.md`](performance/CONFORMANCE_FEEDBACK.md)。

外加 `cargo test --workspace` 下的 crate 与 service 单测，以及 `npm run check --prefix clients/web` / `npm run build --prefix clients/web` 检查 Web shell。

## Partial（已开始但未做完）

- 能力包身份的 `event.subscribe` 权限。
- 能力包持有的钩子处理器的超时 / 错误审计。
- 能力 provider 的持久选择策略（超出单次调用显式选择）。
- 更丰富的资源策略（filesystem 强制矩阵）。
- 内容寻址的资产 blob 存储与能力包身份的资产权限：稳定的 content-address helper 与元数据约定已完成；完整 blob 存储与运行时权限执行未完成。
- 包持有的 projection 执行。
- 主动健康探针、自动恢复 / rebind 与更丰富的失败监控；已明确观测到的 JSON-RPC stdio transport 永久失联会使受影响 Run durable 失败。
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
- [`protocol/PUBLIC_PROTOCOL.md`](protocol/PUBLIC_PROTOCOL.md) —— 公开协议。
- [`spec/`](spec/README.md) —— 可执行契约矩阵与 conformance 路线图。
- [`guides/`](guides/README.md) —— 能力包创作指南。
- [`roadmap/NEXT_STEPS.md`](roadmap/NEXT_STEPS.md) —— 下一步。
