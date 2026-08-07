# Plurora Lockfile v1 格式

> [English](./LOCKFILE_FORMAT.en.md) · [中文](./LOCKFILE_FORMAT.md)

## 目的

Plurora lockfile 用于让 profile 的 package 安装可复现。Profile manifest 描述“想要什么”，lockfile 记录“实际解析到了什么”：版本、来源、提交、内容哈希、签名状态、安装路径以及用户在安装时授予的权限。

同一个 profile manifest 在不同机器、不同时间运行安装时，应该优先使用 lockfile 中的固定结果，避免分支漂移、远端 tag 变化、传递依赖变化或权限重新解释造成不可预期的运行时状态。

Lockfile 是安装器与 host 的数据文件，不是内核协议方法。内核 v1 仍保持内容无关；包依赖解析和获取由安装层或普通能力包实现。

## 位置

默认位置：

```text
~/.plurora/profiles/<name>.lock.toml
```

其中 `<name>` 是 profile 名称。实现可以支持显式 `--lockfile <path>`，但写入默认 profile 时应使用上述路径。

## Filesystem layout

Plurora's state lives under a single base directory, resolved as:

1. `PLURORA_DATA_DIR` environment variable (explicit override)
2. `$XDG_DATA_HOME/plurora/` (XDG-compliant)
3. `~/.plurora/` (default)

Layout:

```text
<data_dir>/
├── store/                       # Immutable, content-addressed package store
│   ├── sha256-abc.../          # One directory per tree hash
│   └── sha256-def.../
├── profiles/                    # Per-user mutable
│   ├── default.yaml            # Profile autoload list
│   ├── default.lock.toml       # Lockfile
│   ├── alpha.yaml
│   └── alpha.lock.toml
├── keys/                        # GPG public keys (trust roots)
│   └── trusted-keys.asc
└── cache/
    └── git/                    # Git fetch cache
```

The store is content-addressed: installed trees are immutable, while successful
install/update/uninstall operations remove entries that are no longer referenced
by profiles or lockfiles. Store schema migration may clear old store contents
when hash rules change; profiles and lockfiles are preserved so updates can
rebuild the store.

Permissions: data directory is created with 0700 on Unix.

## 编码

- 格式：TOML。
- 时间：RFC 3339 timestamp，带 UTC offset。
- Hash：带算法前缀的字符串，v1 要求 `sha256:<hex-or-encoded-digest>` 形式。
- 枚举值：snake_case。

## Schema 版本

顶层 `schema` 字段必须为：

```toml
schema = "plurora.lock.v1"
```

读取器必须拒绝未知 schema，除非显式启用迁移流程。v1 内仅允许 additive 变更；breaking change 必须使用新的 namespace，例如 `plurora.lock.v2`。

## 顶层字段

### `schema`

字符串。固定为 `plurora.lock.v1`。用于让读取器选择正确的解析和验证规则。

### `profile`

字符串。此 lockfile 所固定的 profile 名称。它应与路径中的 `<name>` 一致；不一致时实现应警告或拒绝，避免错误套用 lockfile。

### `generated_at`

时间。生成 lockfile 的时间，用于诊断和审计。它不参与依赖解析决策。

### `manifest_hash`

字符串。生成 lockfile 时 profile manifest 的规范化 SHA-256 哈希。安装器用它检测 profile manifest 是否已漂移。

### `package`

数组。已锁定的 package 条目。TOML 中使用 `[[package]]` 表示。空数组表示 profile 当前没有外部 package 锁定项。

## LockEntry 字段

每个 `[[package]]` 表示一个已解析并安装或可安装的 package。

### `id`

字符串。Package id，必须与被锁定 package manifest 中的 `id` 一致。

### `version`

字符串。解析后的 package 版本。它是 lockfile 的结果值，而不是 constraint。

### `source`

枚举。来源类型：

- `internal`：Plurora 内置或 host 提供，不需要获取。
- `git`：来自 Git remote。
- `local`：来自本地路径，主要用于开发。

### `url`

可选字符串。Git 来源的 origin URL。非 Git 来源通常省略。

### `ref`

可选字符串。安装时用户或 manifest 请求的 tag、branch 或 commit ref。对于 branch，它不是最终固定点；最终固定点是 `commit`。

### `commit`

可选字符串。Git 来源解析后的 commit SHA。Git 来源应填写；internal 和 local 来源可以省略。

### `tree_hash`

字符串。安装时 package tree 的 SHA-256。读取器至少验证它以 `sha256:` 开头；安装器应在写入前计算真实内容哈希。

### `manifest_hash`

字符串。Package manifest 规范化后的 SHA-256。用于检测 package manifest 内容是否在同一 commit 或路径下发生变化。

### `package_envelope_digest`

可选字符串。逻辑 package envelope 的完整 SHA-256 digest。它不同于 `tree_hash`，锁定 manifest、component、protocol、content root、surface 与辅助 artifact 之间的关系。

### `component_pins`

可选数组。每个 pin 记录独立 component ID、component artifact digest、behavior digest 与 trust class。Composition 因此不需要把 package ID 当作 component ID。

### `protocol_profile_pins`

可选数组。锁定 component 实现的精确 protocol ID、version 与 profile 选择。

### `content_roots`

可选的完整 `ArtifactDescriptor` 数组。Content root 与 component pin 分离，因此替换实现不会隐式替换不可变 world/project/media 数据。

### `signed`

布尔值。表示来源是否经过 GPG 签名验证。它记录安装时验证结果，不会自动授予额外权限。

### `signed_by`

可选字符串。签名 key fingerprint。仅当 `signed = true` 时通常填写。读取器可以用它和 manifest `requires[].minimum_signed_by` 做审计对比。

### `installed_at_store`

字符串。不可变 store 中的安装路径。实现可以使用 Nix store、Plurora 自有 CAS store 或 host 管理的只读目录。

### `granted_capabilities`

字符串数组。用户在安装时授予的 capability 权限。运行时仍必须执行普通权限与 capability handle 检查。

### `granted_network`

字符串数组。用户在安装时允许的网络 host 或 host pattern。它是 install-time grant 的记录，不替代 manifest `permissions.network`。

### `granted_secrets`

字符串数组。用户在安装时允许的 secret refs。它记录引用名，不记录 raw secret。

### `requires`

数组。传递依赖解析结果。TOML 中可以表示为 `[[package.requires]]`。

## LockRequirement 字段

### `id`

依赖 package id。必须指向另一个已锁定条目，或指向 host 可解析的 internal package。

### `constraint`

原始 semver constraint。空字符串表示任意版本。该字段保留用户或上游 manifest 的意图。

### `resolved_to`

解析结果。推荐格式是 `<package-id>@<version>`，实现也可以使用内部稳定标识，只要能映射到 `package[]` 中的条目。

## 传递解析

安装器从 profile manifest 的直接 `requires` 开始解析。每个 package manifest 可以声明自己的 `requires`。解析完成后，lockfile 应包含完整闭包，并在每个 `LockEntry.requires` 中记录该 package 的直接依赖边。

这让工具可以回答：某个 package 为什么被安装、哪个 constraint 导致当前版本、升级会影响哪些下游。Lockfile 不要求记录求解器的全部中间状态，但必须足以重建最终依赖图。

## Drift 检测

`plurora lockfile --check` 会：

1. 读取 lockfile；
2. 对每个 `LockEntry`：
   a. 验证 store 路径存在；
   b. 重新计算 `manifest_hash`，与 lockfile 比对；
   c. 在字段存在时重新派生 package-envelope digest、component pin、protocol-profile pin 与 content root；
   d. 重新计算 `tree_hash`，与 lockfile 比对；
3. 报告任何漂移。

非零退出码用于 CI：漂移 = 失败。

读取 lockfile 时，安装器应重新规范化当前 profile manifest 并计算 SHA-256。如果它与顶层 `manifest_hash` 不一致，说明 profile manifest 已经漂移。

默认策略应 fail closed：要求用户运行 install/update 重新生成 lockfile。开发模式可以允许 `--allow-drift`，但必须给出清晰警告。

对每个 package，安装器也可以重新计算 `tree_hash` 和 `manifest_hash`。哈希不匹配表示 store 损坏、本地路径变化或获取结果不一致，应拒绝使用该条目。

## 兼容性

v1 内允许 additive-only 变更：新增可选顶层字段、新增可选 LockEntry 字段、新增可选 grant 类数组、或新增读取器可忽略的 metadata。

v1 不允许删除字段、改变必填性、改变 hash 语义、重命名 enum 值、或改变 `requires` 图语义。这些 breaking change 必须进入 v2 schema namespace。

读取器应忽略未知可选字段，但不得忽略未知 `schema`。写入器应尽量保持字段顺序稳定，以减少审查 diff。

## 示例

```toml
schema = "plurora.lock.v1"
profile = "default"
generated_at = "2026-05-23T00:00:00Z"
manifest_hash = "sha256:profile"

[[package]]
id = "vendor/tool"
version = "1.2.3"
source = "git"
url = "https://example.com/vendor/tool.git"
ref = "v1.2.3"
commit = "0123456789abcdef0123456789abcdef01234567"
tree_hash = "sha256:tree"
manifest_hash = "sha256:manifest"
signed = true
signed_by = "0123456789ABCDEF0123456789ABCDEF01234567"
installed_at_store = "/store/vendor-tool"
granted_capabilities = ["model/live_call"]
granted_network = ["api.example.com"]
granted_secrets = ["secret_ref:env:API_KEY"]

[[package.requires]]
id = "plurora/core"
constraint = ">=1.0.0"
resolved_to = "plurora/core@1.0.0"
```
