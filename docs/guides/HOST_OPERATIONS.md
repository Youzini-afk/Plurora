# Host 运维手册

> [English](./HOST_OPERATIONS.en.md) · [中文](./HOST_OPERATIONS.md)

本手册覆盖当前本地 SQLite Host 运维基线。PostgreSQL 备份、在线不停机备份和原目录覆盖恢复尚未提供。

## 健康检查

- `GET /livez`：进程和 HTTP reactor liveness，正文为 `ok`。
- `GET /health`、`GET /healthz`：兼容 liveness，与 `/livez` 相同。
- `GET /readyz`：公开、脱敏的结构化 readiness。event store 或 Host 控制面租约失效时返回 HTTP 503；Host 可接管但某个 durable deployment 未就绪时返回 HTTP 200 且 `status: "degraded"`。

不要用 liveness 判断是否可以发送变更请求；编排器应以 `/readyz` 的 HTTP 状态和 `ready` 字段为准。

## 创建备份

前提：

1. 停止 desktop/CLI Host，避免外部安装器同时修改 data dir。
2. profile 必须位于 data dir 内，`event_store.kind` 为 `sqlite`，`event_store.path` 使用相对 profile 的相对路径。
3. 输出目录必须不存在并位于 data dir 外。

```bash
ygg host backup \
  --data-dir /srv/ygg \
  --profile /srv/ygg/profiles/host.yaml \
  --output /srv/backups/ygg-2026-07-23
```

命令会取得持久 Host 控制面租约；如果仍有 Host 持有租约，它会失败而不会复制。快照排除顶层 `cache/`，拒绝 data dir 中的符号链接，并在 `manifest.json` 中记录 `data/` 下每个文件的大小和 SHA-256。SQLite 文件通过 online backup API 生成，不是复制活跃数据库文件。

## 恢复与验收

恢复只接受不存在的新目录，不覆盖旧数据：

```bash
ygg host restore \
  --backup /srv/backups/ygg-2026-07-23 \
  --data-dir /srv/ygg-restored
```

恢复先在目标同级 staging 中校验 manifest、路径、文件类型、大小、SHA-256、profile 到 SQLite 的引用和 SQLite integrity；全部通过后才原子 rename。验收时用恢复后的 data dir 和其中的 profile 启动 Host，确认 `/readyz`，再验证关键项目、secret 引用和部署历史。验收完成前保留旧 data dir。

## 发行校验

`v*` tag 不能绕过 CI：release workflow 先复用完整 Contract/Rust/Web/Desktop gate，并检查 tag、精确 commit、所有 Cargo/npm/Tauri 版本一致。平台构建仅在 gate 成功后开始；draft release 为每个平台包含 installer、`SHA256SUMS` 和 SPDX SBOM，对 installer digest 记录 provenance 与 SBOM attestation。

下载后可核验：

```bash
sha256sum -c Yggdrasil-<target>-SHA256SUMS.txt
gh attestation verify <installer> -R Youzini-afk/Yggdrasil
```

当前没有配置平台 signing/notarization，draft release 不应被描述为已签名发行版。
