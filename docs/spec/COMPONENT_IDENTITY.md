# Package Envelope 与 Component Identity

状态：Experimental，描述符 Schema 版本 1。

Plurora 将“软件如何被获取”与“软件声称实现什么行为”分离。package 是安装 envelope；component 是 envelope 内独立命名的实现。协议、内容根、surface 与辅助 artifact 各自保留独立 descriptor 和 SHA-256 identity。

## Identity 分层

| 层 | 负责 | 不代表 |
|---|---|---|
| Package envelope | 获取来源、manifest、安装事务、完整性证据 | 稳定的实现 identity |
| Component | 可执行或静态实现、行为声明、执行边界 | 内容所有权或 package 来源 |
| Protocol implementation | protocol/version/profile/vector 声明 | 路由优先级或官方偏好 |
| Content root | 不可变世界、项目或媒体数据 | 某个特定可执行实现 |
| Surface | Shell contribution 与激活 metadata | 底层 world state 的所有权 |

因此，`PackageManifest.id` 继续作为 package management key；`EntryDescriptor.component.id` 才是 component key。两者可以不同，同一个显式 component 声明也可以由多个 package envelope 携带。

当前 manifest 只有一个 entry component。descriptor 与 lock 使用 vector，以便未来多 component envelope 能保持 additive 演进。

## Descriptor 与 digest

以下 descriptor 使用 canonical JSON，并散列为 `sha256:<64 lowercase hex>`：

- `ComponentDescriptor`：component artifact、behavior artifact、trust class、边界声明、capability、协议实现、内容根与 surface；
- `PackageEnvelopeDescriptor`：package manifest、component descriptor、packaged protocol、内容根、surface 与辅助 artifact；
- `CompositionLock`：分别锁定 component artifact、protocol profile 与 content root。

对象键会 canonicalize；无序声明集合在散列前排序、去重。package identity 参与 envelope digest，但不参与显式 component 的 behavior digest。因此两个 package 可以不同，同时保持相同 component ID 与行为声明。

安装 tree hash 仍然只证明复制进不可变 store 的字节与获取完整性，不复用为逻辑 package-envelope 或 component identity。

## 显式与旧版 identity

显式 component 声明包括：

- namespaced component ID 与语义版本；
- package 提供的 capability 集合，空列表表示推断完整集合；
- 带 version、profile 与 conformance-vector ID 的协议实现；
- content-root artifact descriptor；
- package 的 surface ID，空列表表示推断完整集合。

Capability 与 surface 声明必须和 package 实际提供内容一致。Shell descriptor 可以使用 package namespace 或显式 component namespace。

没有 `entry.component` 的 v1 manifest 会获得合成 ID：

```text
<package-id>/component/default
```

这个 `legacy_adapted` identity 保留 package-contract composability，但不声称跨 package 的 component portability。

## Trust class 与边界声明

Trust 名称是可核验记录，不是宣传标签。只有当前 host 实际执行的边界才能报告为 enforced。

| Trust class | 当前保证 |
|---|---|
| `trusted_native` | 代码运行在可信 host 进程内；不声明隔离 |
| `isolated_process` | 只保证进程故障隔离；不声明 OS network/filesystem 隔离 |
| `sandboxed_component` | 已选择 component boundary；host 未实现前不声明资源、网络或文件系统限制 |
| `remote_boundary` | host 完成验证前，不声明 identity、tenancy、network 或 revocation 已被强制执行 |
| `static_resource` | 不执行代码 |
| `foreign_capsule` | 仅支持 host 启动；不声明 conforming、composable、portable 或隔离 |

旧 `TrustLevel` 字段为了 API 兼容仍保留在 package record 中。`ComponentTrustClass` 与 `ComponentBoundaryClaims` 是 Contract v2 的 canonical 字段。

## Foreign Capsule

`contract:none` 始终映射为 `foreign_capsule`。

- Rust in-process 与 subprocess capsule 仍可按现有 Path B 规则启动；
- 声明的 capability/hook 不会注册，不会 mint v1 binding；package principal 的 capability、event、network 与 secret-ref authority 均被拒绝；
- manifest 校验拒绝 protocol conformance 声明，descriptor 构建也会防御性移除这些声明；
- package conformance 必须以 warning 明示 composability 与 portability 不受保证；
- 不根据 entry kind 推断 network、filesystem、tenancy、revocation 或 sandbox 保证。

它是明确的 containment category，不是较低等级的 conformance。

## Runtime 证据

Package record 与 lifecycle event 会公开 package-envelope digest 和 component descriptor。Capability discovery、invocation result 与成功 effect receipt 会携带：

- provider package ID；
- provider component ID；
- component artifact digest；
- behavior digest；
- component trust class。

Conforming in-process package 通过 `KernelEnv` 收到 component ID 与 digest；subprocess handshake 收到 package-envelope digest 与 component descriptor，Foreign Capsule handshake 的 v1 capability/permission/binding 集合为空。这样 audit 与 replay 可以在 installer envelope 之外独立识别具体实现。

## Composition lock

`CompositionLock` 分别维护三组 pin：

```text
components         component ID + artifact digest + behavior digest + trust class
protocol_profiles  protocol ID + version + selected profile
content_roots      完整 ArtifactDescriptor
```

替换 component pin 不会修改 content root。安装 lock entry 持久化相同的 component/profile/content pin，并额外记录 package-envelope digest。`check_lockfile` 会从已安装 manifest 重新派生这些值，并将其漂移与 manifest、tree、静态 surface hash 的漂移分别报告。

## Conformance 含义

Package conformance 包含完整 package envelope，以及每个 component 的记录：

- `declared`：显式 identity 与结构性 composability 已独立锁定；portability 仍需独立 implementation-conformance 报告通过；
- `legacy_adapted`：保留 package-contract composability，不保证跨 package portability；
- `foreign_capsule`：可以启动，但不保证 composability 与 portability。

Protocol 与 implementation conformance 仍是独立报告。把实现装进 package 不会获得协议优先级，也不能跳过协议自有行为向量。

## Schema

- `component-descriptor.schema.json`
- `package-envelope-descriptor.schema.json`
- `composition-lock.schema.json`

三者均为 `docs/spec/v1/schemas/` 下的 additive Experimental schema。
