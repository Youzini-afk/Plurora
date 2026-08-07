# Experimental World Bundle

> [English](./WORLD_BUNDLE.en.md) · [中文](./WORLD_BUNDLE.md)

状态：Experimental profile `plurora.world.bundle/experimental/v1`。

World Bundle 为持久化的高层 World 提供跨 Host 携带、读取和迁移的可选 Profile，但不会因此把 `World` 变成 substrate 类型。具体 archive 无需加载原始 Package、启动 Component、启用模型 provider 或挂载 Web Shell，就能被读取、校验和审计。

## Archive 结构

[`world-bundle.schema.json`](v1/schemas/world-bundle.schema.json) 发布 `WorldBundleArchive`：

```text
WorldBundleArchive
├── archive_format
├── bundle_descriptor
├── manifest
│   ├── world_head
│   ├── journal_ranges
│   ├── object_descriptors
│   ├── composition_lock
│   ├── protocol_profiles
│   ├── policy_refs
│   ├── effect_receipts
│   ├── lineage
│   └── original_v1_envelopes
└── objects[]
    ├── descriptor
    └── data_base64
```

`bundle_descriptor.digest` 是 canonical manifest JSON 的 SHA-256 digest，其 references 枚举可移植对象清单。导入前会重新计算每个内联对象的摘要并校验大小。可移植 Bundle 中的 Artifact reference 必须是 SHA-256 digest，不能是宿主路径、进程 ID、临时 URL 或 Package 本地 Runtime Handle。

digest 标识字节本身。Artifact type、media type、角色 annotation 等 descriptor metadata 是受 manifest 完整性保护的描述视图，不会为同一 digest 制造第二个对象身份。按 digest 建索引的库存保留一个合并了闭包 reference 的 canonical 视图；角色局部视图可以拥有不同的 type、media type 或 annotation，但 digest 与 size 必须一致，且其声明的全部 reference 都必须被库存覆盖。未知 `artifact_type_uri` 仍然有效，并按原字节复制。

## Head、Journal 与 Lineage

[`world-head.schema.json`](v1/schemas/world-head.schema.json) 发布由协议定义的当前 head：

```text
WorldHead
├── state_root
├── history_root
├── composition_lock
├── protocol_profiles
├── policy_root
├── provenance_root
├── effect_receipts
└── parent_heads
```

[`world-journal-range.schema.json`](v1/schemas/world-journal-range.schema.json) 把一个 session ID、连续的闭区间 sequence，以及每个原始 v1 `EventEnvelope` 的内容寻址 Artifact 绑定在一起。系统不会虚构跨 session 的全局顺序：v1 只保证 session 内顺序，跨分支因果由 lineage 与 parent head 表达。

导出时生成的原始 envelope 字节会作为对象保留。导入保持 event ID、session ID、sequence、timestamp、writer、kind、payload 与 metadata 不变。派生 head 指向 parent head；重新执行一步不会修改已导入的 head 或 receipt。

## 生命周期

当前实现的生命周期是：

1. 选择一个或多个连续 journal range 与 state root。
2. 固定 composition 和精确 protocol profile。
3. 将 event envelope、receipt、policy/provenance 记录和全部传递对象物化进 SHA-256 ObjectStore。
4. 计算并验证完整 reference closure。
5. 导出 canonical manifest 与 base64 object payload。
6. 目标宿主在写入任何数据之前完整验证 archive。
7. 把对象与原始 envelope 导入空 scope，再重新水化当前支持的 substrate projection。
8. 在不调用 executor 的前提下审计或历史回放 envelope 与 receipt output。
9. 可选安装另一种实现，在新 session branch 上执行，并导出 lineage 指向已导入 parent 的 child head。

ObjectStore 与 EventStore 没有跨后端共享事务。因此 import 会先验证完整 archive、确认目标 session scope 为空，并持有 import lock 后再写入。内置 InMemory 与 SQLite EventStore 会在同一个原子操作里重新确认全部目标 session 为空并追加整批事件；不支持这一增强操作的 EventStore（包括当前 PostgreSQL 实现）会在对象写入前显式失败。Session 重建与当前支持的 substrate projection 会在 journal 提交前完成校验；提交后只合并本次导入的 projection entry，不再执行可能失败的解码，也不会整体替换无关的 Runtime entry。ObjectStore 失败或 event batch 被拒绝时，最多留下不可达的 immutable CAS object，不会提交部分 event journal，也不会静默改写对象。

## Headless CLI

Archive 是不依赖 Shell 的数据 Artifact：

```text
plurora world-bundle verify <archive.json> [--json]
plurora world-bundle audit <archive.json> [--json]
plurora world-bundle replay <archive.json> [--json]
plurora world-bundle import <archive.json> --data-dir <fresh-dir> [--json]
```

`replay` 只做 historical replay：它解码已记录的 envelope、receipt 与 receipt output，并报告 executor invocation 为零。它不会调用原始 capability provider、网络 executor、模型 provider、本地进程 executor 或 shell bridge。

## 失败模型

以下情况会让验证或导入显式失败：

- 对象缺失、base64 无效、digest 不匹配或 size 不匹配；
- 非 SHA-256 reference 或未解析的传递引用；
- 同一 digest 声明了冲突的大小；
- bundle manifest 或原始 event envelope 被修改；
- journal range 不连续，或 envelope 的 session/sequence 与 range 不一致；
- composition lock、world head、protocol version 或必需 profile 不匹配；
- policy 或 receipt reference 无法解析；
- 目标 session scope 非空。

不认识 Artifact 语义不是错误；其字节与 descriptor 会继续留在已验证清单中。

## 可执行 Conformance

`plurora.runtime.world-bundle` 已注册为第一个 production implementation claim，因为五条协议自有向量全部可执行通过：

- `world_bundle.reference_closure`；
- `world_bundle.cross_host_import`；
- `world_bundle.offline_replay`；
- `world_bundle.reexecution_branch`；
- `world_bundle.shell_independence`。

当前 integration fixture 使用 `plurora/playable-creation-board` Package。测试在 Host A 创建状态、branch 和受控 capability receipt；在 Host B 的独立 SQLite journal 与 filesystem CAS 中导入；不加载原始 Package 完成回放；在新分支使用 echo-backed 替代实现；最后由 headless CLI 读取同一个 archive。该 fixture 检查此 Profile 的兼容行为，不把该 Package 或 World 本体提升为平台中心。

## 当前限制

- v1 archive 编码是带 base64 object 的 JSON。压缩、分块传输、签名和 authenticated envelope 属于未来 profile，本 schema 不作隐含保证。
- Headless CLI 会拒绝大于 1 GiB 的 archive 文件，只允许导入不存在或完全为空、且由独占锁保护的数据目录。Runtime 校验最多接受 100,000 个对象与 4 GiB 解码后对象数据。
- World Bundle 不是进程内存快照。Package、subprocess handle、socket、临时 URL 与 live stream 被有意排除。
- import 会重新水化当前 event model 支持的 substrate projection。即使删除 component 后无法恢复其 live projection code，历史审计仍然可用。
- Bundle 内容纳入必须经过显式 authority/policy 决策。可移植历史可能包含用户创作内容；Bundle 不宣称任意 asset body 都不敏感。
