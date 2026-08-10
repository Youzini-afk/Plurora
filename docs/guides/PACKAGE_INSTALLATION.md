# Package、Work 与 Installation

> [English](./PACKAGE_INSTALLATION.en.md) · [中文](./PACKAGE_INSTALLATION.md)

Plurora 不再把 Package 加载、源码工作区、内容身份、安装实例和运行状态合并成一次“安装”。当前流程分为：来源识别与 Work pack、Host Installation create/update、以及后续独立 Run。

## 快速路径

```bash
# 生成或检查 Work source
plurora work init ./my-work --id example/my-work
plurora work check ./my-work

# 写入内容寻址 ObjectStore；不安装、不运行
plurora work pack ./my-work

# 创建 Host-local Installation
plurora installation create ./my-work --idempotency-key install-my-work-v1

# 查看与更新
plurora installation list
plurora installation info <installation-id>
plurora installation update <installation-id> \
  --expected-revision 1 --state preserve \
  --idempotency-key update-my-work-v2

# 移除时必须明确 state 处理
plurora installation remove <installation-id> \
  --state keep --idempotency-key remove-my-work
```

Phase 4 前，Installation `ready` 不代表已经启动。CLI 和 Web 不会把 create 伪装成 Run。

## 可接受来源

| 来源 | 归一化结果 |
|---|---|
| `work.yaml` + `assembly.yaml` | 按显式 Work / Assembly source 解析。 |
| Package manifest | 生成单节点 Assembly 与合成 WorkRevision，保留 Component identity。 |
| 普通源码仓库 | 先成为 Workspace / inspection / BuildGraph 候选；源码可见不等于可运行。 |
| 外部 URI、本地 executable、OCI、远程服务 | 生成 Foreign Capsule WorkRevision；具体位置只存在于 Host-local binding。 |
| content-only bundle | 生成没有可执行节点的 content Work。 |

Package 是可替换组件与能力的分发单元。`provides` 投影成 export Capability Port，`consumes` 投影成 import Capability Port；解析器按 protocol、interface、version、profile、interaction、transport、effect 与 multiplicity 检查兼容。多个同等 provider 会报歧义，不按 publisher 自动选择。

## Work pack

`plurora work pack` 执行：

1. 以安全文件句柄读取 source descriptor；
2. 解析 Package Envelope / Component Descriptor 或 Foreign/content source；
3. 生成并校验 AssemblyRevision；
4. 解析 authoring bindings 与 nested Assembly exposed ports；
5. 生成 AssemblyLock；
6. 生成 WorkRevision；
7. 把 canonical bytes 与完整 closure 写入 ObjectStore；
8. 输出 digest、closure 与结构化诊断。

它不会写 Installation journal、不会创建 Run、不会分配端口，也不会修改 profile。

## Installation create

`host.installation.create` 接收显式 `work_id`、WorkRevision 与 AssemblyLock descriptor、display name、acquisition、state bindings、secret policy 与 idempotency key。设备调用必须具有 `installation.manage` 和精确 `host/work/<work_id>` selector；CLI 从已 pack 的 canonical WorkRevision 取得这个 WorkId。Host 会重新验证：

- artifact 存在、SHA-256 与 descriptor 一致；
- 从 canonical CAS bytes 解码的 `WorkRevision.work_id` 与请求 `work_id` 完全一致；
- Work 与 AssemblyLock type URI 正确；
- portable artifact 不含本地路径、raw secret 或 Host runtime facts；
- state slots 与 bindings 唯一；
- Installation-local secret ref 使用精确 allowlist。

通过初始协议检查后，Host 铸造不可由 wire 构造的 mutation-authority sidecar。取得 apply lock 后、closure/projection 准备与 journal terminal commit 前，服务会从 HostAccess journal 重新同步并验证 grant、期限、delegation chain、action 与精确 Work；撤销、过期或 Work 不匹配都会在创建目录或 terminal event 前 fail closed。成功后 append journal event，再原子刷新 `installation.json` projection。Projection 不是 authority；重启时从 journal 重建。

## Update

Update 必须带 `expected_revision` 与显式 state action：

- `preserve`：契约兼容时保留；
- `replace`：切换到已验证 state artifact；
- `reset`：明确舍弃不兼容 state。

Host 在切换 active Work/Lock pointer 前建立 state snapshot。CAS 或后续步骤失败时保留旧 pointer，并记录可审计 rollback / recovery evidence。

`preserve`、`replace` 与 `reset` 都使用同一 Host-only sidecar，并在 apply lock、CAS/closure、projection、state effect 和 terminal commit 边界重新验证精确 `host/installation/<installation-id>` authority；请求字段本身不授予权限。

## Workspace ownership

Workspace 是可变、Host-local 对象：

- `managed`：Host 在 `~/.plurora/workspaces/<workspace-id>/source/` 下拥有副本；
- `linked_local`：指向用户已有目录，Host 只记录绑定。

所有 managed 操作检查 canonical containment、symlink/reparse point 与目录归属。linked-local source 永不因 Installation update/remove 被删除、归档或改写。

## 路径与大小边界

这些限制对应已识别的失败模式，而不是产品配额：

- source descriptor 上限 1 MiB，防止把任意大文件当作控制面 YAML/JSON；
- managed tree 沿用 intake 的 25,000 文件/目录与 256 MiB 预算，防止无界复制耗尽 Host；
- source file、祖先目录与 ObjectStore root 遇到 symlink/reparse 或 containment 变化时 fail closed；
- logical ID 只接受安全 grammar，不能替代 filesystem containment 检查。

## Secret

安装记录只保存引用与 policy，不保存 raw key：

```yaml
secret_policy:
  allow_platform_fallback: false
  allowed_secret_refs:
    - secret_ref:installation:OPENAI_API_KEY
```

值只在 Host executor 中解析。详见 [`SECRET_MANAGEMENT.md`](SECRET_MANAGEMENT.md)。

## 本地布局

```text
~/.plurora/
├── objects/
├── installations/<installation-id>/
├── workspaces/<workspace-id>/
└── runtime/installations.sqlite3
```

新实现不会读取已退休目录、descriptor、lockfile 或 profile 作为 Installation authority，也没有 alias、fallback reader 或迁移器。测试使用全新临时数据目录。

## 检查

```bash
cargo test -p plurora-work
cargo test -p plurora-runtime install_lab
cargo test -p plurora-service installations
cargo test -p plurora-cli --test install_commands
```

完整对象边界与 journal 语义见 [`INSTALLATION_MODEL.md`](INSTALLATION_MODEL.md)。
