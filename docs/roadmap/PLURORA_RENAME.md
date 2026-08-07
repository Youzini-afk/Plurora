# Plurora 破坏性改名方案

> [English](./PLURORA_RENAME.en.md) · [中文](./PLURORA_RENAME.md)

> 状态：临时实施方案。改名完成后删除本文，并把仍有长期价值的命名规则收敛到架构、合同、指南和状态文档。
>
> 决策：项目正式名称改为 **Plurora**。当前仍处于开发阶段，本次改名采用一次性破坏性重置，不提供 Yggdrasil 兼容层、数据迁移、命令别名、协议别名或旧格式读取。

## 目标

这不是把界面标题从 Yggdrasil 换成 Plurora，而是把项目的全部公开身份一次性改干净：

- 品牌、仓库、可执行文件、安装包和发布资产；
- Rust crate、TypeScript package、SDK 和源码目录；
- 环境变量、数据目录、临时目录、容器名和 Web storage key；
- Package publisher namespace、Protocol/Profile ID、URN、MIME、lockfile schema 和 JSON Schema `$id`；
- RPC method、event kind、生成 SDK 与 Contract Registry；
- Web、Desktop、PWA、Docker、CI、示例、fixture 和文档；
- 图标、wordmark 和仍然依赖“世界树”的视觉隐喻。

完成后，当前 Git 历史是旧名称唯一需要保留的地方。工作树、构建产物和新创建的数据中不应再出现旧身份。

## 破坏性原则

### 不保留兼容

明确不做：

- 不保留 `ygg`、`yg` 或其他旧 CLI 入口；
- 不读取 `YGG_*` 环境变量；
- 不探测或迁移 `~/.yggdrasil`；
- 不读取 `yggdrasil.lock.v1`、`urn:yggdrasil:*`、`application/vnd.yggdrasil.*` 或旧 bundle；
- 不保留 `@yggdrasil/*`、`ygg-*`、`yg-*` crate/package 名；
- 不为旧 RPC method、event kind、header、Web storage key 或文件扩展名添加 alias；
- 不维护 deprecated/legacy adapter 生命周期；
- 不提供 `contract migrate` 一类旧 ID 转换命令；
- 不在文档中保留“旧名称也可以继续使用”的说明。

旧开发数据应直接删除。旧客户端、旧 Package、旧 SDK 和旧导出文件视为无效开发产物。

### 不做机械全局替换

破坏性重置不等于把每个 `ygg` 字符串无脑替换成 `plurora`。这次应同时切开两类身份：

1. **品牌身份** 使用 `Plurora / plurora / PLURORA`；
2. **架构语义** 使用 `host`、`context`、`journal`、`capability`、`authority`、`object`、`change`、`projection`、`shell` 等 owner namespace。

RPC method 与 event kind 不应因为品牌存在就全部带 `plurora.` 前缀。这样可以让平台语义清楚，也避免未来再次改品牌时连架构名词一起变化。真正需要全球唯一身份的 Protocol、URN、MIME、Package publisher 和 schema 才使用 `plurora` namespace。

## 目标命名表

### 产品与仓库

| 当前 | 目标 |
|---|---|
| `Yggdrasil` | `Plurora` |
| GitHub repository `Youzini-afk/Yggdrasil` | `Youzini-afk/Plurora` |
| 本地仓库目录 `D:\project\Yggdrasil\Yggdrasil` | `D:\project\Plurora` |
| `Yggdrasil Host` | `Plurora Host` |
| `Yggdrasil Desktop` | `Plurora Desktop` |
| `Yggdrasil Web` | `Plurora Web` |
| `Yggdrasil Protocol Commons` | `Plurora Protocol Commons` |

GitHub 可能自动保留旧仓库 URL 跳转；这是 GitHub 行为，不在代码、文档或发布流程中维护兼容入口。

### Rust workspace

| 当前 | 目标目录 / package |
|---|---|
| `crates/ygg-core` / `ygg-core` | `crates/plurora-core` / `plurora-core` |
| `crates/ygg-runtime` / `ygg-runtime` | `crates/plurora-runtime` / `plurora-runtime` |
| `crates/ygg-service` / `ygg-service` | `crates/plurora-service` / `plurora-service` |
| `crates/ygg-cli` / `ygg-cli` | `crates/plurora-cli` / `plurora-cli` |
| `ygg-desktop` | `plurora-desktop` |
| `ygg-tdb-rust-adapter` | `plurora-tdb-rust-adapter` |
| `yg-kernel-sdk` | `plurora-contract-sdk` |
| Rust imports `ygg_core` 等 | `plurora_core` 等 |

公开生成 SDK 不再使用 `kernel-sdk` 名称。它描述完整公开合同，因此统一命名为 **Contract SDK**。

### CLI 与 sidecar

| 当前 | 目标 |
|---|---|
| binary `ygg` | binary `plurora` |
| 文档中混用的 `yg` | `plurora` |
| `ygg-host` sidecar | `plurora-host` |
| `YGG_HOST_LISTEN_ADDR=` | `PLURORA_HOST_LISTEN_ADDR=` |
| `ygg host ...` / `yg ...` | `plurora host ...` / `plurora ...` |

不创建短命令。对普通用户和脚本只发布 `plurora` 一个入口。

### TypeScript 与 npm

| 当前 | 目标 |
|---|---|
| `@yggdrasil/web` | `@plurora/web` |
| `ygg-desktop` | `@plurora/desktop`（private） |
| `@yggdrasil/kernel-sdk` | `@plurora/contract-sdk` |
| `@yggdrasil/subprocess` | `@plurora/subprocess` |
| `@yggdrasil/agentic-forge` | `@plurora/agentic-forge` |
| `@yggdrasil/experience-runtime` | `@plurora/experience-runtime` |
| `@yggdrasil/inference-capability` | `@plurora/inference-capability` |
| `sdk/typescript/ygg-agent-adapter` | `sdk/typescript/agent-adapter` / `@plurora/agent-adapter` |
| generated `KernelClient` | `PluroraClient` |
| generated `KernelMethods` | `PlatformMethods` |
| generated `KernelEvent` | `PlatformEvent` |

在实施前先占用 npm scope 和计划公开的 crate 名；未发布包仍一次性改名，不做旧 scope 转发包。

### 数据、环境与本地状态

| 当前 | 目标 |
|---|---|
| `YGG_*` | `PLURORA_*` |
| `YGG_DATA_DIR` | `PLURORA_DATA_DIR` |
| `YGG_HOST_URL` | `PLURORA_HOST_URL` |
| `YGG_HTTP_ACCESS_TOKEN` | `PLURORA_HTTP_ACCESS_TOKEN` |
| `YGG_TARGET_AGENT_*` | `PLURORA_TARGET_AGENT_*` |
| `XDG_DATA_HOME/yggdrasil` | `XDG_DATA_HOME/plurora` |
| `~/.yggdrasil` | `~/.plurora` |
| `.yggdrasil-nixpacks` | `.plurora-nixpacks` |
| `ygg-*` temp/container/target names | `plurora-*` |

新程序只创建和读取 Plurora 路径。若机器上存在旧目录，应由开发者手动删除；程序不得导入、移动或合并旧数据。

### Desktop、PWA 与 Web runtime

| 当前 | 目标 |
|---|---|
| Tauri `productName: Yggdrasil` | `Plurora` |
| `com.yggdrasil.desktop` | `io.github.youzini-afk.plurora` |
| window title | `Plurora` |
| sidecar `ygg-host` | `plurora-host` |
| `__YGG_RUNTIME__` | `__PLURORA_RUNTIME__` |
| `ygg-language` | `plurora-language` |
| `ygg-recently-opened` | `plurora-recently-opened` |
| `ygg-*` CSS class / DOM id | `plurora-*` |
| `.ygg-change.json` | `.plurora-change.json` |
| `.ygg-world.json` | `.plurora-world.json` |
| `yggdrasil.svg` / maskable icon | 新的 `plurora.svg` / `plurora-maskable.svg` |

旧世界树图标不只改文件名，应替换为新的 Plurora 视觉身份。改名期间可以先使用结构正确的临时图标，但合并到 `main` 前不保留世界树图形和 Yggdrasil wordmark。

### Package 与第一方 publisher

当前 `official/*` 是一个过于泛化的 publisher namespace。破坏性重置后改为：

```text
official/install-lab        → plurora/install-lab
official/package-lab        → plurora/package-lab
official/model-provider-lab → plurora/model-provider-lab
...
```

`plurora/*` 只表示发布者，不产生任何权限、路由优先级或隐藏能力。`thirdparty/*`、`example/*`、`fixture/*` 等测试 namespace 可继续存在，但所有第一方 Manifest、composition、fixture、profile、测试和文档必须同步切换。

### 全球唯一机器标识

| 当前 | 目标 |
|---|---|
| `urn:yggdrasil:*` | `urn:plurora:*` |
| `application/vnd.yggdrasil.*` | `application/vnd.plurora.*` |
| `yggdrasil.lock.v1` | `plurora.lock.v1` |
| `ygg.contract.default/v1` | `plurora.contract.default/v1` |
| `ygg.shell.default/v1` | `plurora.shell.default/v1` |
| `ygg.change` | `plurora.change` |
| `ygg.change/default/v1` | `plurora.change/default/v1` |
| `ygg.world.bundle` | `plurora.world.bundle` |
| `ygg.runtime.*` implementation ID | `plurora.runtime.*` |
| `x-ygg-*` header | `x-plurora-*` |
| `ygg.*` WebSocket subprotocol | `plurora.*` |

JSON Schema `$id` 不依赖尚未拥有的 `.com`。改为稳定 URN，例如：

```text
urn:plurora:schema:method:host.info:v1
urn:plurora:schema:event:host/project.started:v1
urn:plurora:schema:type:effect-receipt:v1
```

仓库相对 `$ref` 继续由 exporter 生成和校验。

## 公开合同重置

品牌改名应同时删除当前尚未发布的兼容层，而不是把它复制成 Plurora 版本。

### 删除的机制

删除：

- `kernel.v1` legacy contract profile；
- 所有 `kernel.v1.*` method alias；
- 所有 `kernel/v1/*` event kind；
- `LEGACY_CONTRACT_PROFILE`；
- `ContractMaturity::LegacyAdapter` 与 deprecated alias metadata；
- alias diagnostics，例如 `ygg.contract.alias.legacy_adapter`；
- `legacyKernelV1*` / `legacy_kernel_v1_*` generated SDK wrapper；
- `contract migrate` CLI 与对应扫描、preview、write/rollback 代码；
- legacy route `GET /kernel/v1/host.info`；
- 只用于证明 alias 等价性的测试和 conformance case；
- 文档中的支持窗口、replacement、legacy adapter 叙事。

Contract Registry 保留，但只记录唯一 canonical method、owner、版本、Profile、成熟度和 schema；不再维护 alias 表。

### 唯一 method namespace

现有 80 个 method 重新生成唯一 owner-based ID。版本由 Contract/Profile metadata 管理，不嵌入 method 名。

| 当前类别 | 目标 ID 形状 |
|---|---|
| Session | `context.open`、`context.close`、`context.fork`、`context.branch.list`、`context.get`、`context.list` |
| Event | `journal.append`、`journal.list`、`journal.subscribe` |
| Package | `host.package.load|unload|restart|logs|list|status|describe` |
| Project | `host.project.*` |
| Target / exec / port / proxy | `host.target.*`、`host.exec.*`、`host.port.*`、`host.proxy.*` |
| Capability | `capability.discover|describe|invoke|stream|cancel` |
| Capability handle | `authority.handle.attenuate|revoke|list` |
| Permission grant | `authority.grant.create|revoke|list`、`authority.decision.list` |
| Extension / hook | `protocol.extension.list|describe`、`protocol.hook.list` |
| Asset | `object.put|get|list` |
| Projection | `projection.register|rebuild|get|list` |
| Host identity | `host.info|ping|diagnostics`、`identity.current` |
| Package audit | `host.package.audit` |
| Proposal adapter | `change.proposal.*` |
| Surface | `host.surface.bundle.resolve`、`shell.contribution.list|describe` |
| Outbound | `host.outbound.audit|execute|stream|websocket.open|send|close` |

实施前把 80 个 variant 的完整 old → new 表写成单一机器可读源，exporter、dispatcher、OpenAPI、SDK 和测试都从它生成，禁止在多处手写映射。

### 唯一 event namespace

59 个事件改为 owner-based path，版本继续使用 envelope 的 `schema_version`：

```text
kernel/v1/session.opened          → context/opened
kernel/v1/package.ready           → host/package.ready
kernel/v1/project.started         → host/project.started
kernel/v1/asset.put               → object/put
kernel/v1/proposal.approved       → change/proposal.approved
kernel/v1/capability.completed    → capability/completed
kernel/v1/permission.denied       → authority/denied
kernel/v1/outbound.request        → host/outbound.request
kernel/v1/exec.started            → host/exec.started
kernel/v1/deployment.health       → host/deployment.health
```

完整事件表同样由单一 registry 生成 schema 文件名、事件常量、文档和 SDK union。

### 内部类型去 kernel 化

旧类型名按实际职责重命名，而不是统一替换为 `PluroraKernel*`：

| 当前 | 目标 |
|---|---|
| `KernelMethod` | `PlatformMethod` |
| `KernelSession` | `SessionRecord` |
| `KernelEvent` | `EventEnvelope` 或 `PlatformEvent`（按所在层） |
| `KernelEnv` | `ComponentEnv` |
| generated `KernelClient` | `PluroraClient` |
| `KernelOutboundStreamResponse` | `OutboundStreamResponse` |
| `KERNEL_PACKAGE_ID` | `PLATFORM_RUNTIME_ID` 或职责更准确的常量名 |

这一步只改名称和公开边界，不在改名任务中重写业务状态机。

## 实施顺序

全部工作在 `rename/plurora` 分支完成。允许分支内阶段性破坏构建，但不把半改名状态合入 `main`。

### 1. 固定 rename map 与红线检查

先新增机器可读 rename map 和检查脚本，至少覆盖：

- 文件/目录；
- Cargo package / crate import；
- npm package / import；
- CLI / sidecar；
- environment / filesystem；
- Protocol / Profile / URN / MIME / schema；
- RPC method / event kind；
- first-party Package ID；
- Web storage / global / CSS；
- Docker / CI / release。

检查脚本从第一步开始阻止新增旧品牌字符串，但在改名尚未完成时允许显式的待清理清单。最后切换为零容忍。

### 2. 重命名 build graph

使用 `git mv` 重命名 crate、SDK 和 adapter 目录，更新：

- workspace members / dependencies；
- Cargo package、bin、feature 和 import 名；
- npm package、lockfile、workspace import；
- codegen 输出目录和生成注释；
- Desktop sidecar staging；
- Dockerfile、entrypoint 和 release scripts；
- CI job 命令、cache key 和 artifact 名。

这一阶段结束时，`cargo metadata --no-deps`、npm install 和代码生成入口必须能够定位所有新路径。

### 3. 重置合同、schema 与 SDK

一次性完成：

- `KernelMethod` → `PlatformMethod`；
- 80 个 canonical method ID；
- 59 个 canonical event kind；
- 删除 alias/legacy adapter/migrate；
- `urn:plurora:*`、MIME、lockfile schema 和 Profile ID；
- JSON Schema 文件名与 `$id`；
- OpenAPI operation ID；
- Rust / TypeScript Contract SDK；
- subprocess reverse dispatch；
- Web protocol client；
- conformance registry。

生成文件只由 exporter/codegen 重建，不手工搜索替换。

### 4. 重命名 Package 生态

把所有第一方 `official/*` 改为 `plurora/*`，随后更新：

- Manifest `id`、provides、consumes、hooks、Surface 和 dependencies；
- profiles；
- composition；
- Package routing 和 catalog；
- fixtures / replacement cases；
- install lockfile fixture；
- generated authoring templates；
- docs 和示例命令。

这里不得加入 `official/*` alias，也不得让新 publisher namespace获得特权。

### 5. 重命名 Host、Web、Desktop 与视觉资产

处理：

- `PLURORA_*` 环境变量；
- `~/.plurora` 和 XDG path；
- Host backup/restore 临时名；
- target、container、Docker label、header 和 WebSocket subprotocol；
- Web globals、localStorage、CSS、DOM id、下载扩展名；
- PWA manifest、page title、metadata；
- Tauri package、bundle identifier、sidecar 和 installer 名；
- 图标、wordmark、About、license 展示和截图。

使用全新数据目录做 smoke，不从旧目录复制任何文件。

### 6. 文档、仓库与外部名称

更新 README、Charter、Architecture、Guides、Status、Building、Changelog、examples 和注释。

随后执行外部操作：

1. 将 GitHub repository 改名为 `Plurora`；
2. 将本地目录移到 `D:\project\Plurora`；
3. 更新 `origin` 与 Git `safe.directory`；
4. 检查 GitHub Actions、release、container registry 和 badge；
5. 占用 `@plurora` scope 与计划发布的 crate 名。

独立的 `Yggdrasil-Tavern` / `YdlTavern` 不机械改成 `PluroraTavern`。它需要自己的产品名。核心仓库在其改名完成前移除旧仓库名的主导航引用，避免 Plurora 正式身份继续依赖旧品牌。

### 7. 清理和最终合入

删除本临时方案前：

- 删除 rename map 中已完成的临时兼容说明；
- 删除旧图标、旧生成文件、旧 SDK 目录和空目录；
- 重新生成 Cargo.lock、npm lockfiles、schema、SDK、OpenAPI 和 perf baseline；
- 将长期命名规范写入 `STYLE`、架构和 Contract 文档；
- 确认 `main` 只接收完整 Plurora 状态。

## 验收红线

### 旧身份必须为零

除 `.git` 历史和外部依赖缓存外，以下搜索结果必须为零：

```text
Yggdrasil
yggdrasil
@yggdrasil
YGG_
YggRuntime / __YGG_RUNTIME__
~/.yggdrasil
yggdrasil.lock
urn:yggdrasil
application/vnd.yggdrasil
ygg.contract / ygg.shell / ygg.change / ygg.world
ygg- / ygg_ / yg-kernel-sdk
binary ygg
CLI text yg
kernel.v1
kernel/v1
LegacyAdapter
legacyKernelV1
legacy_kernel_v1
contract migrate
official/
```

如果确实需要引用历史名称，只允许放在 Git commit message；当前文档和 Changelog 也不保留双品牌迁移说明。

### 构建与生成

必须通过：

```text
cargo metadata --no-deps
cargo check --workspace
cargo test --workspace
schema export + validation
generated SDK clean check
Contract/OpenAPI ID uniqueness
npm check/test/build for Web
npm build/smoke for Desktop sidecar
Docker build + clean data-dir start
full conformance
host operations acceptance
backup/restore on a fresh Plurora data dir
scripts/check-docs.py
git diff --check
```

大型 Rust、Docker、Windows 和 Desktop 验收继续由 GitHub CI 执行；本机只跑有界检查。

### Clean-room 行为

使用一台没有旧数据的环境或全新临时目录检查：

```text
PLURORA_DATA_DIR=<empty>
plurora host serve
plurora install ...
plurora project list/start/stop
Plurora Web/PWA connect
Plurora Desktop starts plurora-host
schema / SDK client invokes only canonical IDs
```

程序不得访问 `YGG_*` 或 `.yggdrasil`，不得为旧数据给出迁移提示，也不得在失败后静默回退。

## 提交结构

建议在分支中保留以下可审阅提交：

```text
chore(rename): establish Plurora identity map
refactor(rename): rename Rust and Node workspace
refactor(contract): reset public method and event identities
refactor(ecosystem): rename first-party package namespace
feat(brand): replace Web Desktop and Host identity
docs(rename): complete Plurora documentation
chore(rename): enforce zero Yggdrasil residue
```

每个提交只负责一个边界；最终合入前整条分支必须全绿。不要为了让中间提交暂时通过而添加旧名 alias。

## 不在本任务中做

- 不增加新产品功能；
- 不重写现有业务状态机；
- 不发布正式版本；
- 不建设用户数据迁移器；
- 不建立旧包、旧客户端或旧协议的兼容测试；
- 不因 `.com` 已注册而让机器标识依赖该域名；
- 不在尚未选定独立产品名时仓促重命名 Tavern 项目。

## 完成定义

改名完成意味着：

1. 新用户、开发者、Package 作者和自动化只会看到 Plurora；
2. 新构建只产生 `plurora` 名称的二进制、包、安装器、目录和工件；
3. 公开合同只有一套 canonical ID，不存在 Yggdrasil 或 `kernel.v1` 双栈；
4. 新 Host 只读取 `PLURORA_*` 和 `~/.plurora`；
5. 生成 schema、SDK、OpenAPI、Package template 和 conformance 使用同一份新身份；
6. 当前仓库不包含为了尚未发布用户而维护的兼容债；
7. 本文被删除，长期文档只描述 Plurora 的现行事实。