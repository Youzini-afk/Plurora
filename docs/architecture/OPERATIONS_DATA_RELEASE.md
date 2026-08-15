# 运行、数据与发行安全

> [English](./OPERATIONS_DATA_RELEASE.en.md) · [中文](./OPERATIONS_DATA_RELEASE.md)

本文定义 Plurora Host 承载真实 Work 和远程 Target 时必须满足的数据、健康、诊断、升级和发行底线。哪些已经落地、哪些仍是加固项，见 [`../ALPHA_STATUS.md`](../ALPHA_STATUS.md)。

## Host 必须满足的行为

- Install Lab 的 store schema 不匹配不再删除数据；旧 store 被原子移动到带版本和随机后缀的保留目录，新 store 再初始化当前 marker。
- `plurora host backup` 对位于 data dir 内、使用相对路径的 SQLite Host profile 创建离线目录快照。命令先取得持久 Host 控制面租约，排除显式 `cache`，使用 SQLite online backup API，并为所有文件写 SHA-256 manifest；原始 secret、key、objects、installations、profiles、journals 均在同一租约边界内复制。
- `plurora host restore` 只接受不存在的新 data dir；它拒绝路径穿越、符号链接、重复项和 checksum/schema 不一致，在 staging 中完成验证与 SQLite integrity check 后才原子切换。
- `/livez`、`/health`、`/healthz` 是兼容 liveness；`/readyz` 返回无资源标识的结构化状态。event store 或 Host 控制面租约失败返回 `503/unready`，单个 durable Realization 不健康返回 `200/degraded`。
- `host.diagnostics` 现在包含 Host 版本和 runtime 聚合计数，不额外公开 Work、Installation、route 或 lease 标识。
- tag release 显式复用完整 CI workflow，严格验证 tag/commit/Cargo/npm/Tauri 版本一致，再执行平台构建。每个平台产出 SHA-256 清单、SPDX SBOM，并通过 GitHub OIDC/Sigstore artifact attestation 记录 provenance 和 SBOM；只有 build job 获得发行权限。

仍未完成：通用 migration ledger、PostgreSQL backup reference、独立 `backup inspect/verify` 命令、authenticated `/host/v1/status` 与 diagnostics export、object/secret 主动 probe、统一 HTTP continuous health policy、干净 runner installer 启动 smoke、Actions/toolchain 的 reviewed SHA 固定，以及平台 signing/notarization。发行保持 draft，未配置签名时不得描述为已签名。

## 数据分类先于迁移

每类数据必须声明：事实源、可重建性、备份一致性边界、schema version、保留和恢复顺序。

| 数据类 | 默认性质 | 备份要求 |
|---|---|---|
| Event journal / Host control journals | 权威、只追加 | 必须；保持 sequence/CAS 语义 |
| Object store | 可能被 journal/descriptor 引用 | 与引用它的 journal 同一备份集合 |
| Secret store + key | 不可重建的敏感权威数据 | 成对加密备份；严格权限 |
| Installation record/state/managed workspace | 用户承重数据 | 必须或由 Installation policy 显式排除 |
| Profiles/lockfiles/keys | 运行与供应链配置 | 必须；保留权限和版本 |
| Realization intents/revisions/receipts | 恢复和回滚事实源 | 必须；与 event journal 一致 |
| Download/build cache | 可重建缓存 | 可排除；必须明确标记为 cache |
| Package/content store | 条件可重建 | 只有证明有来源时才允许自动重建 |

任何“schema 不匹配则删除目录”的行为只能用于显式 cache。marker 缺失不能自动证明数据是缓存或过期格式。

## Schema migration

每个持久 backend 使用单调递增 schema version 和 migration ledger：

```text
MigrationRecord
  component
  from_version / to_version
  migration_id
  started_at / completed_at
  preflight_digest
  backup_ref?
  result / diagnostic_ref?
```

启动流程：

1. 只读识别数据布局和版本；
2. 完整性 preflight；
3. 判断是否需要备份、磁盘空间和独占锁；
4. 执行可重入 migration；
5. 校验目标 schema 与关键引用；
6. 原子提交 version/ledger；
7. migration 未完成时 Host 不进入 ready。

破坏性迁移必须要求显式 operator flag 或已经创建可验证 backup；不能在普通启动路径静默执行。

## 备份合同

备份是一个带 manifest 的不可变集合：

```text
BackupManifest
  format_version
  host_id / created_at / created_by
  application_version / schema_versions{}
  consistency_mode
  included_components[] / excluded_components[]
  files[{path, size, digest, mode?}]
  encrypted_secret_payload_ref?
  journal_heads{}
```

要求：

- SQLite 使用在线 backup API 或在独占 checkpoint 后复制，不能只复制活跃数据库文件；
- PostgreSQL 记录外部 backup reference 与 journal heads，不假装文件级备份；
- objects 与引用 journal 必须有一致的 cut，恢复后执行 reachability/integrity scan；
- secret data 与 key 成对备份，并允许 operator 使用外部 wrapping key；
- backup 默认不包含临时 build/download cache；
- restore 先解包到新目录、验证全部 digest/schema，再原子切换；
- 原 data dir 保留为 rollback source，直到恢复验收完成。

提供 `backup create/inspect/verify/restore`；restore 默认要求 Host 停止且目标目录为空，覆盖必须显式确认。

## 健康语义

| Endpoint | 认证 | 含义 |
|---|---|---|
| `/livez` | public、最少信息 | 进程和 HTTP reactor 可响应 |
| `/readyz` | public 只返回状态码/简短状态 | hydration/migration 完成，必需 store 可用，可接受控制请求 |
| `/host/v1/status` | Host identity | 结构化 component 状态、degraded reasons、版本和 journal heads |
| `/host/v1/diagnostics/export` | 明确 diagnostic 权威 | 脱敏诊断 bundle |

`/health`、`/healthz` 在兼容期映射到明确的 liveness 或 readiness，并在文档中固定；不能继续含糊地恒定返回成功。

readiness 至少检查：

- runtime 已完成 hydration；
- event store 读写/CAS 基础检查；
- object store 可读写并能验证临时对象；
- secret store 状态可判定（不读取 secret 值）；
- Realization controller 不处于 migration/recovery fatal 状态；
- profile 与 contract registry 已加载。

可选能力失败导致 `degraded`，必需能力失败导致 not-ready。详细原因需要认证，避免公开泄露路径、backend 或项目信息。

## 部署 health policy

部署 probe 是 revision 的声明式配置：protocol、path、expected status range、interval、timeout、success/failure thresholds、initial delay。

- HTTP 默认只把 2xx 视为成功；3xx/4xx 必须由 policy 显式允许；
- startup readiness 和持续 health 使用同一 policy 解析器；
- probe 只观察，不直接启动 replacement；
- 状态变化写审计事件，并由 Realization Controller 的 restart policy 决定操作；
- probe 日志和响应 body 有严格大小/脱敏限制。

## 可观测性

最小结构化信号：

- 请求 correlation、principal/grant ref、canonical method、policy decision ref；
- Realization operation/step/target/generation/lease epoch；
- build/apply queue latency、operation duration、retry/cancel/rollback；
- target heartbeat、offline/reconnect、tunnel bytes/errors；
- route readiness transition 和 probe failures；
- journal append/CAS failure、object verification failure、backup/migration result。

指标不含 project 名、secret、token、完整 URL query 或源码内容。高基数 resource id 只进入受控 trace/log，不作为默认 metric label。

诊断 bundle 包含版本、配置形状（脱敏）、component status、最近受限日志、journal heads、Realization summaries 和 integrity results；生成与下载都写审计。

## 支持的 Host 拓扑

1. **Desktop managed Host：** 随机 loopback 端口、一次性 bootstrap、本地持久 profile。
2. **Local/LAN operator Host：** 明确 bind、非空 root credential、防火墙限制。
3. **Internet-facing Host：** TLS reverse proxy 或可信 overlay；裸 HTTP 只对代理可见，保留 Host/Origin，明确 trusted-proxy policy。

项目 public route 与 Host control API 始终是两个暴露平面。配置 app domain 不能自动公开项目，也不能让 proxy headers 伪造 authenticated origin/client identity。

## Release gate

Tag 只选择已经验证的 commit，不能绕开 CI。发行图：

```text
source commit
  -> contract/schema clean check
  -> locked Rust/Web tests and conformance
  -> desktop sidecar smoke
  -> platform builds
  -> installer smoke
  -> checksums + SBOM + provenance/attestation
  -> signing/notarization where available
  -> draft release
```

要求：

- release workflow 显式依赖完整 gate，而不是假设 branch CI 曾运行；
- Cargo 使用 lockfile，Node 使用 `npm ci`，toolchain 版本固定；
- GitHub Actions 固定到已审查 commit SHA；更新由受控依赖流程完成；
- 权限按 job 最小化，只有发布 job 获得 `contents: write`；
- tag、Cargo/npm/Tauri version 必须一致；
- 每个资产附 checksum、SBOM 和源码 commit provenance；
- installer 在干净 runner 上做最小启动/sidecar/版本 smoke；
- signing 未配置时清楚标记为 unsigned，不能暗示已验证来源。

## 升级与回退

- 升级前执行 schema preflight、兼容性检查和 backup policy；
- binary rollback 与 data rollback 分开陈述；已执行不可逆 migration 时禁止只降级 binary；
- release notes 列出最低/最高可读 schema、backup 要求和已知拓扑限制；
- managed desktop 升级协调 sidecar 与 shell 版本，不能让两者合同不兼容；
- 每个 release candidate 在 CI 运行旧版数据 → 新版迁移 → backup → restore round trip。

## 完成门槛

- 权威数据没有静默 destructive reset 路径；cache reset 有明确分类和审计；
- backup/restore 在空白目录和故障注入下可验证，secret/object/journal 引用保持一致；
- live、ready、degraded 能区分 listener、存储和 controller 故障；
- release 只来自通过 gate 的精确 commit，资产可校验、可追溯；
- 支持拓扑有自动 smoke 和操作手册；
- 大型 migration、restore、installer 和跨平台矩阵只在 GitHub CI 运行。
