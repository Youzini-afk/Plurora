# Installation 模型

> [English](./INSTALLATION_MODEL.en.md) · [中文](./INSTALLATION_MODEL.md)

Installation 是某台 Host 对一个不可变 WorkRevision 的本地采用记录。它不是内容身份，也不是运行实例：同一个 WorkRevision 可以在多台 Host 上产生多个 Installation；一个 Installation 又可以在后续产生多个 Run。

## 对象边界

| 对象 | 所有者 | 作用 |
|---|---|---|
| WorkRevision | 可移植 artifact | 用户认知的作品与入口，引用 AssemblyRevision。 |
| AssemblyRevision | 可移植 artifact | 组件图、Port、Binding、State Slot 与暴露映射。 |
| AssemblyLock | 可移植 artifact | 解析后的节点、绑定、协议 profile 与 content roots。 |
| Workspace | Host-local | 创作或构建所需的可变源码目录；位置不进入 Work identity。 |
| Installation | Host-local journal | 当前激活的 WorkRevision / AssemblyLock、来源、state bindings、secret policy 与状态。 |
| Run | Host Service Run journal | 一次实际运行；由 `host.run.*` 管理独立状态、节点实例、health 与 terminal record。 |

Package 仍是可替换能力与组件的分发单元。Package manifest 可以归一化为单节点 Assembly，但 Package ID 不自动成为 Work ID，源码可见也不等于获得运行权威。

## 创建路径

```text
work.yaml / assembly.yaml / Package source / Foreign source
  ↓ plurora work check / pack
WorkRevision + AssemblyRevision + AssemblyLock + closure
  ↓ plurora installation create --idempotency-key ...
Installation journal event
  ↓ projection rebuild
installation.json
```

`work pack` 只写内容寻址 ObjectStore，不安装、不运行、不修改 Host profile。Installation create 会重新验证 WorkRevision 与 AssemblyLock artifact 已存在且摘要正确，然后在 journal 中创建记录。

## 生命周期

```text
resolving → ready
ready → updating → ready
resolving/updating → blocked | failed
ready/blocked/failed → removing → removed
```

每次修改都带单调递增的 `revision`。调用方更新时必须提交 `expected_revision`；不匹配时返回冲突，不能覆盖新状态。

创建、更新和移除都要求 `idempotency_key`。同一个 key 加同一个 request fingerprint 会返回已提交结果；相同 key 搭配不同请求会 fail closed。

## 更新与状态

更新会先验证新的 WorkRevision / AssemblyLock，再对受影响状态建立 snapshot，并要求显式 state action：

- `preserve`：状态契约兼容时保留现有绑定；
- `replace`：CLI 将公开的 typed snapshot JSON canonicalize 后通过 `object.put` 上传，update 线路只提交固定类型的 descriptor；
- `reset`：明确放弃不兼容状态。

Host 从已验证 CAS 中的当前与候选 WorkRevision、根 AssemblyRevision 及 AssemblyLock 重建 diff，不信任 request 携带的摘要元数据。diff 保留兼容用的顶层 changed 标记，同时按稳定 ID 排序报告 Work entrypoints、content roots、rights、transparency、operational intent，Assembly nodes（含 inline Ports/config）、authoring bindings、exposed Ports、State Slots，以及 Lock nodes、bindings、protocol profiles、content roots 的 `added` / `removed` / `changed` before/after 值。

每个 State Slot 会比较 owner、schema digest、scope、portability、backup policy 与 migration Port。已有 durable state 的兼容 slot 可 `preserve`；不兼容 slot 只有在候选 Assembly 已验证 migration Port 时才允许 `replace`，否则必须独立选择破坏性的 `reset`；移除 durable slot 同样要求 `reset`。Run-scoped slot 不要求 durable migration，新增 slot 可直接进入候选。空 state tree 仍报告 diff，但不强迫没有实际数据的迁移或 reset。

调用方不提供 approval、replacement decision receipt 或 authority evidence。显式 `reset` / `replace` 意图与当前 exact `installation.manage` authority 到达 Host 后，Host 在状态 effect 前重新验证 grant/expiry，自行持久化 authority evidence 和允许该次操作的 decision receipt。`replace` 使用 `urn:plurora:installation-state-replacement-decision-receipt:v1`，其 payload schema 是 `plurora.installation-state-replacement-decision-receipt.v1`；它只证明 Host 决定并应用了提交的 snapshot replacement，不表示 migration component 已执行。update 结果的 `receipts` 返回这些内容寻址 descriptor；同一幂等请求在重启后仍返回同一组 descriptor。

当前 MVP 的 `replace` effect 是：Host 严格验证并原子应用调用方提交的 canonical typed snapshot，然后为这次 snapshot replacement 持久化 decision/evidence。候选声明 migration Port 是允许 `replace` 的可执行合同，但本阶段 Host 不会调用 migration component，也不会伪造“组件已执行”的 effect receipt；实际 migration component output 必须先被表达成该 typed snapshot。公开 `object.get` 读取 state decision/evidence 时，除结构与 CAS identity 外还必须完整匹配当前 Installation authoritative journal 中已发行的 descriptor；仅通过 `object.put` 放入形状正确的对象不能把它变成 Host receipt。

`replace` 文件使用公开的 `InstallationStateSnapshot` 形状：

```json
{
  "schema": "plurora.installation-state-snapshot.v1",
  "entries": [
    { "path": "save/profile.bin", "bytes": [1, 2, 3, 255] }
  ]
}
```

`entries` 按 `path` 严格排序且路径唯一。路径只允许 `/` 分隔的非空相对段；绝对路径、平台 prefix、`.`、`..`、反斜杠与 NUL 会被拒绝。CLI 不把本机文件路径或 raw state bytes 放进 update DTO 或错误；它可连接数据目录完全不同的远程 Host，因为 snapshot 先经公开 `object.put` 传输。

active Work/Lock pointer 只在验证、状态准备和 journal compare-and-set 成功后切换。失败会保留旧 pointer，并保留有界 rollback evidence；未知外部效果不会被猜测为成功。

## 移除

移除必须显式选择：

- `keep`：终止 Installation authority，但保留 Host-owned state；
- `delete`：只删除经过 containment 与 symlink/reparse 检查的 Installation state。

对于 device authority，Host 在最初的 exact `installation.manage` 授权后只通过不可上 wire 的 sidecar 传递刷新能力。remove 等待 Installation apply lock 并同步 journal 后，会在 snapshot、delete/replace effect 及 terminal commit 前后重新检查 exact Installation subject、grant、expiry 与 delegation；`keep` 也在 durable terminal commit 前刷新。刷新发生在 owner lease 检查之后并紧贴 journal append 或文件 effect；等待期间过期或撤销会 fail closed，不删除 state，也不写入虚假的成功 terminal。已持久化 claim 的 replay 直接返回 authoritative terminal result，不修补 projection 或产生 filesystem effect；已经 Removed 的新 key no-op 只有在 append 边界再次刷新 authority 后才持久化 claim。

linked-local Workspace 指向用户拥有的源码，因此两种选择都不会删除或改写该源码。managed Workspace 只能在确认 canonical path 位于 Host-owned Workspace root 内后处理。

## 数据布局

```text
~/.plurora/
├── objects/                         # 内容寻址 artifact
├── installations/<installation-id>/
│   ├── installation.json            # journal 的可重建 projection，不是 authority
│   ├── assembly.lock.json            # 当前锁定闭包的本地 projection
│   ├── secrets.dat                   # age 加密的 Installation secret store
│   ├── state/                        # Host-owned state
│   └── diagnostics/
├── workspaces/<workspace-id>/
│   ├── workspace.json
│   └── source/
└── runtime/
    └── installations.sqlite3         # 默认 durable journal backend
```

启动时从 EventStore journal 重建 Installation projection。`installation.json` 损坏或缺失不能改变 authority，也不会触发旧目录或旧 descriptor 的兼容读取。

## Secret 与 authority

Installation-local secret 使用：

```text
secret_ref:installation:OPENAI_API_KEY
```

解析范围来自经过 Host 验证的 Installation context，而不是客户端提供的任意路径。`secret_policy.allow_platform_fallback` 决定本地值缺失时能否回退 `secret_ref:store:*`；raw secret 不进入 Work、Assembly、journal event、日志或诊断。

设备 grant 使用显式 action scope 与资源 selector。Installation 变更要求 `installation.manage`；列表和读取要求 `observe`。Wildcard 必须在线路上显式写为 `id: null`，省略 `id` 不是 wildcard。

## CLI 与公开协议

```bash
plurora installation list
plurora installation info <installation-id>
plurora installation create <work-source> --idempotency-key <key>
plurora installation update <installation-id> <work-source> --expected-revision <n> \
  --state-action preserve --idempotency-key <key>
plurora installation update <installation-id> <work-source> --expected-revision <n> \
  --state-action reset --idempotency-key <key>
plurora installation update <installation-id> <work-source> --expected-revision <n> \
  --state-action replace --replacement-snapshot <snapshot.json> \
  --idempotency-key <key>
plurora installation remove <installation-id> --state keep \
  --idempotency-key <key>
```

对应的精确 wire ID：

```text
host.installation.list
host.installation.get
host.installation.create
host.installation.update
host.installation.remove
```

生命周期事件：

```text
host/installation.created
host/installation.updated
host/installation.removed
```

不存在兼容 alias。Web Home 与第三方客户端使用同一组公开方法。

## 与 Run 的边界

Installation `ready` 只表示本地采用记录与 artifact 闭包有效，不表示进程已经启动、端口已经分配或 endpoint 已暴露。`host.run.*` 使用独立 Run journal 建立 starting → running → degraded → stopping → stopped（或 failed / interrupted）生命周期；打开 Library 或 Installation 详情不会自动创建 Run。Run start 只激活已安装、已验证且唯一匹配 AssemblyLock 的本地实现；缺失、歧义、unsupported backend 或需要机器资源时返回结构化 gap 和 next step，不隐式 build/apply。Run 的 context 与浏览器 tab 独立，关闭 tab 不会 stop；stop 只释放该 Run 的 activation，不卸载全局 Package。Exposure/Binding 使用 Host journal、exact provider/consumer Port 与可选 Run pin，关闭、撤销、到期和版本漂移都不回写 Installation lock。Realization 以 Installation revision 为 precondition 编译并执行 managed resources，同样不会回写便携 Work/Lock。详见 [`RUN_LIBRARY.md`](RUN_LIBRARY.md)、[`POWERBOX_BINDING.md`](POWERBOX_BINDING.md) 与 [`REALIZATION.md`](REALIZATION.md)。

## 安装操作

在把 `plurora` 装进 PATH 之前，把下面的 `plurora` 换成 `cargo run -p plurora-cli --`。

```bash
cargo run -p plurora-cli -- work init ./my-work --id example/my-work
cargo run -p plurora-cli -- work check ./my-work
cargo run -p plurora-cli -- work pack ./my-work
cargo run -p plurora-cli -- installation create ./my-work --idempotency-key install-my-work-v1
cargo run -p plurora-cli -- installation list
cargo run -p plurora-cli -- installation info <installation-id>
```

`work pack` 只写 ObjectStore。`installation create` 才改 Host journal。`ready` 仍不表示已经启动 Run。

可接受来源：显式 `work.yaml` / `assembly.yaml`、Package manifest（单节点 Assembly）、普通源码仓库（先成为 Workspace / inspection）、Foreign Capsule、content-only bundle。多个同等 provider 会报歧义，不按 publisher 自动选择。

Workspace 是 Host-local 可变源码位置，不进入 Work identity。`linked_local` 源码永不删除。受控写入走 Host development control plane，见 [`../architecture/HOST_DEVELOPMENT_CONTROL_PLANE.md`](../architecture/HOST_DEVELOPMENT_CONTROL_PLANE.md)。
