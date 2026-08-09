# 分享与分发指南

> [English](./SHARING_DISTRIBUTION.en.md) · [中文](./SHARING_DISTRIBUTION.md)

本文说明当前 Contract V1 中可分享、可验证、可导入的 Work 与 Session 分发能力。`plurora/sharing-lab` 是普通 Package / Component；共享语义可以属于可选 Sharing Protocol，不进入宪法基底。

## 核心原则

- Work Bundle 只负责搬运不可变的 `WorkRevision` / 根 `AssemblyRevision` / `AssemblyLock` 内容引用及其 closure。`work_id` 是逻辑名称，不替代内容摘要。
- Package-set lockfile 是分发辅助信息，不拥有 Work 身份，也不能替代 `AssemblyLock`。
- 当前实现只做本地文件交换，不引入 marketplace、签名网络、依赖解析经济或托管计费。
- Bundle 不保存 raw secret；`secret_ref` 只是一种不解析的引用。
- Import 只验证输入并产生需用户审批的 plan；它不安装、不运行、不联网，也不从 agent 输出获得执行权。

## 分享契约

`plurora/sharing-lab` 提供 9 项能力和 3 个 surface（`forge_panel`、`assistant_action`、`home_card`）：

| 能力 | 用途 |
|---|---|
| `describe_sharing_contract` | 描述能力、surface、输出形状与红线 |
| `export_work_bundle` | 导出 `WorkRevision` + 根 `AssemblyRevision` + `AssemblyLock` 内容引用、分发锁与披露信息 |
| `import_work_bundle` | 验证 Work Bundle 并产生 `plan_only` 导入结果 |
| `create_branch_session_bundle` | 创建特定 Session 状态的 branch/session bundle manifest |
| `create_package_set_lockfile` | 锁定精确 Package 版本和内容地址 |
| `compatibility_report` | 对比两个 bundle 或 Package 集并生成兼容性报告 |
| `ai_disclosure_bundle` | 记录 Work、Artifact 或 Session 内容的 AI 来源披露 |
| `read_only_share_manifest` | 创建本地文件级只读 Session 分享清单 |
| `async_fork_share_plan` | 创建需审批的异步 fork 分享计划 |

不存在旧 bundle capability 的 alias、旧字段 reader 或格式 fallback。

## Work Bundle 机器形状

```json
{
  "kind": "work_bundle",
  "bundle_id": "work-bundle:example/playable-creation-board:sha256:e329fd36961fcf2b2e6a4b5e6796f3b761a602b5a0c7bcbecfeff9fe62b38539",
  "format_version": "1",
  "work_id": "example/playable-creation-board",
  "work_revision": {
    "artifact_type_uri": "urn:plurora:work-revision:v1",
    "media_type": "application/json",
    "digest": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
    "size_bytes": 768,
    "references": [
      "sha256:3333333333333333333333333333333333333333333333333333333333333333",
      "sha256:4444444444444444444444444444444444444444444444444444444444444444"
    ],
    "annotations": {}
  },
  "assembly_revision": {
    "artifact_type_uri": "urn:plurora:assembly-revision:v1",
    "media_type": "application/json",
    "digest": "sha256:3333333333333333333333333333333333333333333333333333333333333333",
    "size_bytes": 896,
    "references": [
      "sha256:4444444444444444444444444444444444444444444444444444444444444444"
    ],
    "annotations": {}
  },
  "assembly_lock": {
    "artifact_type_uri": "urn:plurora:assembly-lock:v1",
    "media_type": "application/json",
    "digest": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
    "size_bytes": 1024,
    "references": [
      "sha256:3333333333333333333333333333333333333333333333333333333333333333",
      "sha256:4444444444444444444444444444444444444444444444444444444444444444"
    ],
    "annotations": {}
  },
  "package_set_lockfile": {
    "lockfile_id": "lockfile:sha256:5c4415f0c1f5b3d7bab60554c5520da7526e845cbe05f421a824479f2d83de90",
    "format_version": "1",
    "packages": [
      {
        "package_id": "plurora/playable-creation-board",
        "version": "0.1.0",
        "content_address": "sha256:1b6d7f605b8d106a43c0f13fb41996552011e0101e10ffae9ea00796edef566e"
      }
    ],
    "content_address": "sha256:5c4415f0c1f5b3d7bab60554c5520da7526e845cbe05f421a824479f2d83de90"
  },
  "ai_disclosure": {
    "disclosure_id": "ai-disclosure:sha256:5758295a886ffad108a6eee14628a7ec02c15b89f31cda336294d4e4c0bf560c",
    "items": [
      {
        "content_ref": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        "disclosure_kind": "mixed",
        "description": "Work bundle with AI-generated and human-created content"
      }
    ],
    "content_address": "sha256:5758295a886ffad108a6eee14628a7ec02c15b89f31cda336294d4e4c0bf560c"
  },
  "no_marketplace_fields": true,
  "no_billing_fields": true,
  "no_signing_network_fields": true
}
```

`work_revision`、`assembly_revision` 与 `assembly_lock` 都使用 `plurora_core::ArtifactDescriptor` 的完整 wire shape。Handler 复用 `plurora_work` 的类型常量与验证：

- `artifact_type_uri` 必须精确匹配对应类型；
- `media_type` 必须是 canonical JSON 的 `application/json`；
- `digest` 和每个 `references[]` 必须是完整 SHA-256；
- `size_bytes` 必须是非零整数；
- reference 不能重复或自引用；
- annotations 必须满足 portable value 规则；
- `work_revision` 与 `assembly_lock` 都必须精确引用 `assembly_revision.digest`；共享普通 content root 不能冒充同一 Assembly；
- package-set lockfile identity 由 Package pins 重算，AI disclosure identity 覆盖实际 items，`bundle_id` 再覆盖三类根 descriptor、Package pins 与 disclosure。

缺少字段、出现合同外字段、类型错误、摘要不完整、错误 size/reference 形状或不相关 closure 都会返回 `sharing_lab_rejected`。Handler 不从 Package 名、标题或默认值合成 Work。

## Import 结果

`import_work_bundle` 接受上面的 `kind: work_bundle` 形状。支持的格式通过验证后返回：

```json
{
  "kind": "work_bundle_import",
  "bundle_id": "...",
  "format_version": "1",
  "work_id": "example/playable-creation-board",
  "work_revision": { "artifact_type_uri": "urn:plurora:work-revision:v1", "media_type": "application/json", "digest": "sha256:...", "size_bytes": 768, "references": ["sha256:..."] },
  "assembly_revision": { "artifact_type_uri": "urn:plurora:assembly-revision:v1", "media_type": "application/json", "digest": "sha256:...", "size_bytes": 896, "references": ["sha256:..."] },
  "assembly_lock": { "artifact_type_uri": "urn:plurora:assembly-lock:v1", "media_type": "application/json", "digest": "sha256:...", "size_bytes": 1024, "references": ["sha256:..."] },
  "ai_disclosure": { "disclosure_id": "ai-disclosure:sha256:...", "items": [{ "content_ref": "sha256:...", "disclosure_kind": "mixed", "description": "..." }], "content_address": "sha256:..." },
  "compatibility_status": "compatible",
  "diagnostics": [],
  "requires_user_approval": true,
  "plan_only": true
}
```

其他 `format_version` 返回 `unsupported`，不会启动旧格式迁移或读取旧字段。缺少 Package 时返回结构化 `minor_incompatibility`；这仍只是计划，后续 Installation / resolver 流程必须重新检查当前 authority、policy 和内容摘要。

## Session 分享、AI 披露与异步 Fork

Branch/session bundle、只读分享清单和异步 fork 计划保持 Session 层身份，不冒充 Work artifact。只读分享使用 `share_scope: local_file` 与 `no_remote_service: true`；异步 fork 输出 `status: draft`、`requires_user_approval: true`、`plan_only: true`。

AI disclosure 的 `disclosure_kind` 可以是 `ai_generated`、`ai_assisted`、`human_created`、`ai_reviewed`、`mixed` 或 `undisclosed`。披露是声明，不是 authority、计费凭据或法律判断。

## 红线

- 不接受 marketplace、payment、subscription、billing、签名网络或 license-key 字段。
- 不接受 raw API key、token 或 password；只允许引用。
- 不创建 `platform.sharing.*`、`platform.marketplace.*` 或 `platform.billing.*` 基底命名空间。
- 不需要公网、远端服务或隐藏的第一方权限。
- Export / import、Session 分享和异步 fork 都不执行外部效果。

## 示例与验证

完整 fixture 位于 `examples/bundles/playable-creation-board-work-bundle/`：

- `bundle.json` — WorkRevision、根 AssemblyRevision、AssemblyLock 引用、package-set lockfile 与 AI disclosure；
- `branch-session-bundle.json` — branch/session bundle manifest；
- `read-only-share-manifest.json` — 只读 Session 分享清单；
- `async-fork-share-plan.json` — 异步 fork 分享计划。

```bash
cargo test -p plurora-runtime sharing_lab
cargo run -p plurora-cli -- conformance --tag sharing --fail-fast
```

验证覆盖 capability discovery、Work/Assembly descriptor 类型与 portable shape、export/import、format 拒绝、lockfile、兼容性报告、AI disclosure、只读分享、异步 fork 和红线。
