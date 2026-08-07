# 密钥管理

> [English](./SECRET_MANAGEMENT.en.md) · [中文](./SECRET_MANAGEMENT.md)

Plurora 通过 `secret_ref` 引用密钥，宿主在能力调用时解析为真实值。包永远拿不到原始密钥。本文档解释四种解析路径、安全模型、以及怎么从环境变量迁到本地存储或项目级存储。

## 设计原则

- 包用 `secret_ref` 引用，不接触原始值。
- 宿主只在能力调用时解析，不进 event、audit、proposal、log。
- env、store、project、未来 vault 类型由不同 resolver 实现。
- 缺失、拒绝、格式错误一律 fail-closed。
- 错误消息不泄漏值。
- `secret_ref` 是运行时权威输入，不是存储 raw secret 的容器。
- 官方包没有特殊权限；官方能力也只能通过普通 manifest 声明使用 secret。

## `secret_ref` 格式

标准格式：

```text
secret_ref:<vault>:<key>
```

当前支持：

```text
secret_ref:env:OPENAI_API_KEY
secret_ref:store:OPENAI_API_KEY
secret_ref:project:OPENAI_API_KEY
```

兼容前缀仍可解析：

```text
secretRef:env:OPENAI_API_KEY
secret-ref:env:OPENAI_API_KEY
host:env:OPENAI_API_KEY
```

新文档和新包应优先使用 `secret_ref:<vault>:<key>`。

## 四种解析路径

### `secret_ref:env:NAME` — 环境变量

读取 `$NAME` 环境变量。allowlist 控制哪些 env name 可被解析。

- 适用：开发、CI、Docker 部署。
- 优点：无需额外存储，启动前 export 一次即用。
- 缺点：每次启动都要重新设置；shell 历史可能记录；`ps` 进程列表可看到某些启动参数。

示例：

```bash
export OPENAI_API_KEY=sk-...
```

manifest 或 profile 中只写引用：

```yaml
secret_refs:
  - secret_ref:env:OPENAI_API_KEY
```

宿主必须把 `OPENAI_API_KEY` 放入 allowlist。未放行时解析失败，且不会尝试出站调用。

### `secret_ref:store:NAME` — 本地加密存储

读取 `~/.plurora/secrets.dat`（age 加密）。主密钥存 `~/.plurora/secret-store.key`（0600）或系统 keyring。

- 适用：桌面端、长期使用、产品级 UX。
- 优点：用户在 UI 内粘贴一次即可，加密落盘；下次启动自动可用。
- 缺点：需 `plurora/secret-store-lab` 加载。

示例：

```yaml
secret_refs:
  - secret_ref:store:OPENAI_API_KEY
```

宿主通过 `StoreSecretResolver` 在能力调用时读取并解密。包看到的仍然只是 `secret_ref:store:OPENAI_API_KEY`。


### `secret_ref:project:NAME` — 项目级加密存储

读取当前项目目录的 `~/.plurora/projects/<project_id>/secrets.dat`。项目 store 与平台 store 使用同一类 age 加密和同一 master key，但数据文件按项目隔离。

解析路径：

1. 从当前 `ProtocolContext.session_id` 找到活动项目。
2. 读取该项目的 `secrets.dat`。
3. 如果存在 `NAME`，返回项目值。
4. 如果不存在且 `secret_policy.fallback_to_platform: true`，回退到平台 `secret_ref:store:NAME`。
5. 如果 `NAME` 在 `secret_policy.require_per_project` 中，禁止平台回退。
6. 仍缺失时 fail-closed。

- 适用：某个项目需要覆盖平台 key，或需要项目级审计可见的配置。
- 优点：项目可以有自己的 provider key，不影响其他项目。
- 缺点：需要项目上下文；没有 active project/session 时必须失败。

示例：

```yaml
secret_refs:
  - secret_ref:project:OPENAI_API_KEY

secret_policy:
  fallback_to_platform: true
  require_per_project: []
```

### `secret_ref:vault:KEY` — 远程 vault（未来）

预留给未来 HashiCorp Vault / AWS Secrets Manager / Doppler 等接入。

- 当前状态：保留语法，无实现。
- 后续：作为独立能力包提供。
- 行为：应继续遵守 fail-closed、错误不泄漏、审计只记录引用。

## 何时用哪种

| 场景 | 推荐 |
|---|---|
| 开发本地调试 | env |
| CI / 自动化 | env |
| 桌面端产品 | store |
| Plurora 项目默认路径 | project（可按 policy 回退 store） |
| 某项目必须用专属 key | project + `require_per_project` |
| Docker 单服务部署 | env |
| 多用户共享部署 | env（按用户 export） |
| 团队共享 | 未来 vault |

一般规则：

- 需要一次性自动化：用 env。
- 需要长期桌面体验：用 store。
- 需要项目可覆盖平台配置：用 project。
- 需要团队级统一轮换：等待 vault 能力包。

## 怎么用 store

### 通过 UI（推荐）

YdlTavern 的 API Connections 抽屉支持粘贴 + 保存：

1. 选 provider（OpenAI / Anthropic / Gemini 等）。
2. 粘贴 API key。
3. 点保存。
4. UI 调用 `plurora/secret-store-lab/put_secret`。
5. UI 自动设置该 profile 的 `secretRef` 为 `secret_ref:store:OPENAI_API_KEY`。
6. 后续调用只携带引用，不携带 raw key。

如果 store 暂不可用，env 路径仍可作为 fallback。对已安装项目，profile 可以改用 `secret_ref:project:*`，项目 store 缺失时再按 `secret_policy` 回退平台 store。

### 通过命令行

```bash
# 通过 plurora conformance 调一下能力来测试可用
plurora conformance --case secret_store
```

未来会有 `plurora secret put / list / delete` 命令直接操作 store。

### 通过协议

任何能力包都可以调用：

```json
{
  "method": "capability.invoke",
  "params": {
    "capability_id": "plurora/secret-store-lab/put_secret",
    "input": { "name": "OPENAI_API_KEY", "value": "sk-..." }
  }
}
```

注意：原始 `value` 仅在调用瞬间存在，立即加密落盘。

### 读取行为

公开协议没有 `get_secret`。能力包不能请求 raw value。读取只发生在宿主执行器中：

1. 能力包声明并传入 `secret_ref:store:NAME`。
2. 宿主检查 manifest / handle / network 权限。
3. `StoreSecretResolver` 解密本地 store。
4. 执行器把值注入 provider header 或 adapter。
5. 审计只记录 `secret_ref:store:NAME`。

## 加密细节

- 算法：age（rage），认证加密，X25519 身份。
- 文件格式：age-encrypted JSON `{ schema, secrets: { name: value } }`。
- schema：`plurora.secret-store.v1`。
- store 文件：`~/.plurora/secrets.dat`。
- 主密钥文件：`~/.plurora/secret-store.key`。
- 文件权限：Unix 0600。
- 写入：原子（tmp + rename）。
- 名字限制：ASCII 字母数字 + 下划线 + 连字符，1..=128 字符。
- 值限制：UTF-8，<= 16 KiB。

这些限制保证 store 可以简单审计，也避免把任意大 payload 当作 secret 存入。

## 主密钥来源

按顺序尝试：

1. OS keyring（通过 `keyring` crate；当前版本因 dbus 系统依赖暂未启用，会落到第 2 步）。
2. `~/.plurora/secret-store.key` 文件，0600 权限。
3. 首次使用时生成新密钥并持久化到文件。

OS keyring 集成是延后项；当 CI 与跨平台构建环境提供稳定系统依赖后再启用。

## 安全属性

- **公开协议无 `get_secret`**：包不能读其他包的密钥；只有宿主 `SecretResolver` 能读。
- **fail-closed**：缺失 ref、缺失 store、缺失 entry 都返错。
- **错误不泄漏值**：错误消息只含 ref name，不含 value。
- **审计不进 store**：`put_secret` 调用本身被审计，但 put 入参的 `value` 字段会被脱敏。
- **无明文落盘**：值不会出现在日志、备份、core dump（store 内容 age 加密）。
- **无隐式网络**：解析 secret 不会触发网络；出站仍需 network 权限。
- **resolver 分层**：env、store、未来 vault 可组合，但每个 resolver 只处理自己的 vault。

## 从 env 迁到 store

如果你之前用的是 `secret_ref:env:OPENAI_API_KEY`：

1. 打开 YdlTavern API Connections 抽屉。
2. 粘贴你已 export 的 key。
3. 点保存。
4. UI 自动把 profile 切到 `secret_ref:store:OPENAI_API_KEY`。
5. （可选）unset 环境变量。

env 路径继续可用，平台 store 路径也继续可用，已有平台密钥无需迁移。项目可以逐步把某个 profile 切到 `secret_ref:project:NAME`；项目 store 没有值时会按 policy 回退平台 store。三条路径不冲突，同一个 provider 可以在不同 profile 中使用不同 resolver。

## 错误与诊断

常见失败：

| 错误 | 含义 | 处理 |
|---|---|---|
| malformed secret ref | 引用格式不合法 | 改成 `secret_ref:<vault>:<key>` |
| resolver denied | vault 不受支持或未放行 | 检查 allowlist / resolver 配置 |
| missing env var | 环境变量未设置 | export 变量或迁到 store |
| missing store entry | store 中没有该 name | 通过 UI 或能力写入 |
| missing project context | 使用 project ref 但没有活动项目 | 从项目 session 调用，或改用 store/env |
| project secret required | policy 要求项目级配置 | 在项目设置里写入该 secret |
| decrypt failed | store 或 key 文件不匹配 | 检查数据目录与权限 |

诊断输出不得包含 raw value。需要确认值是否存在时，使用 `has_secret` / `list_secrets` 这类布尔或名称级能力，不返回密钥本身。

## 与包安装的关系

包安装只记录用户同意过哪些 `secret_ref` 权限。安装不会要求输入 raw secret，也不会把 raw secret 写进 lockfile。

默认安装流程不会因为包声明 secret 而自动读取它；读取只在能力调用时发生。

## 与模型调用的关系

模型 provider 包应接收 `secret_ref`，例如：

```json
{
  "provider": "openai",
  "credential": "secret_ref:store:OPENAI_API_KEY"
}
```

provider adapter 构造请求 shape；宿主 outbound executor 在最后一刻解析并注入 header。response、audit、stream frame 中仍然只出现引用。

## Project scope 怎么工作

`secret_ref:project:*` 的范围来自项目 session，而不是 surface 自己传的 `projectId` 字符串：

1. Home Play 或 `plurora project start` 调 `host.project.start`。
2. host 创建或复用项目 session，并写入 `session.metadata.project_id`。
3. `clients/web` 把 `session_id` 注入 surface 的 `initialProps.sessionId`。
4. surface 后续 RPC 自动带 `session_id`。
5. host dispatch 设置 `ProtocolContext.session_id`。
6. outbound dispatch 解析 secret 前，用该 session 查 `metadata.project_id`。
7. runtime 设置 `ACTIVE_PROJECT_SCOPE` task-local，内容是 `ProjectScopeContext`。
8. `ProjectStoreSecretResolver` 先读 `~/.plurora/projects/<id>/secrets.dat`。
9. 如果缺失，按项目 `secret_policy` 决定是否回退平台 store。
10. fallback 允许时读取 `secret_ref:store:NAME`；fallback 关闭或 `require_per_project` 命中时 fail-closed。

因此解析顺序是：项目 store → fallback policy → 平台 store。完整真实模型调用链见 [`REAL_MODEL_END_TO_END.md`](REAL_MODEL_END_TO_END.md)；项目 session 的来源见 [`PROJECT_MODEL.md`](PROJECT_MODEL.md)。

## 实现位置

- `crates/plurora-core/src/secret_ref.rs` — `secret_ref` 解析与校验。
- `crates/plurora-core/src/paths.rs` — 文件路径（`secret_store_path` / `secret_store_key_path`）。
- `crates/plurora-runtime/src/secret.rs` — `HostSecretResolver` / `EnvSecretResolver` / `StoreSecretResolver` / `ProjectSecretResolver` / `CompositeSecretResolver`。
- `crates/plurora-runtime/src/secret_store.rs` — 共享加密文件 load/save。
- `crates/plurora-runtime/src/inproc/secret_store_lab.rs` — 能力实现。
- `packages/plurora/secret-store-lab/manifest.yaml` — 包清单。

## 当前限制

- OS keyring 集成延后，当前默认走本地 key 文件。
- `plurora secret put / list / delete` CLI 延后。
- 远程 vault resolver 未实现。
- store 是本机用户级存储，不是团队共享 vault。
- 项目级 store 是软隔离，不是多租户安全边界。

这些限制不会改变核心安全边界：包只拿引用，宿主解析，错误 fail-closed。
