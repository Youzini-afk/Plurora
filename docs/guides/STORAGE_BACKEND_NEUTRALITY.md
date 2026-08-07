# Storage Backend Neutrality

> [English](./STORAGE_BACKEND_NEUTRALITY.en.md) · [中文](./STORAGE_BACKEND_NEUTRALITY.md)

Plurora 目前有 SQLite-backed append-only event log，但平台契约不能绑定到 SQLite。SQLite、PostgreSQL、TDB、对象存储和向量/多模态检索都必须位于 backend/provider 层，而不是成为内核 ontology 或公开协议里的数据库产品。

## 分层模型

1. Kernel event spine：内容无关的 append、replay、range、kind-prefix、subscription 与 rehydrate。当前实现有 in-memory 与 SQLite；未来 PostgreSQL 可以成为 host/runtime backend，但不改变协议。
2. Package-scoped state store：普通能力包可以预览 package-owned document/KV 状态契约，但不能获得 raw DB、SQL 或 backend credential。
3. Blob / asset store：大对象应通过 content address、hash、size、mime 和来源进入平台。blob content 不应塞进 event payload。
4. Projection / index materialization：projection/index 是 event/assets 派生的 package-owned view。它可以计划 materialization、query preview 和 migration plan，但不暴露 table、SQL 或 query product。
5. Retrieval / vector / multimodal providers：TDB、pgvector、OpenSearch、Redis Vector、本地 embedding index 和远端检索服务都是 provider slots。TDB 已有 opt-in Rust adapter proof，但检索仍是 package/provider 层能力，不替代 event log、audit、proposal lifecycle 或 branch lineage。
6. Forge observability：Web shell 只通过公开协议调用 `official/storage-lab` 展示契约摘要，不读 SQLite、PostgreSQL、TDB、文件系统或 runtime internals。

## 红线

- 不新增 `platform.sqlite.*`、`platform.postgres.*`、`platform.tdb.*`、`platform.vector.*`、`platform.embedding.*`、`platform.collection.*`、`platform.sql.*`、`platform.database.*`。
- 不把 `EventStore` 扩成万能 `DatabaseBackend`。
- 不向 package 暴露 SQL、DSN、connection string、table、transaction isolation、ANN index、vector dimension、backend topology 或 raw credential。
- SQLite 是 early/default/local backend，不是平台契约。
- PostgreSQL 是 future server/team backend，不是 package API。
- TDB 是 future multimodal retrieval provider slot，不是 kernel database。
- Retrieval/vector/multimodal search 不能替代 append-only events、audit、proposal lifecycle 或 branch/fork/replay。

## `official/storage-lab`

`official/storage-lab` 是普通 manifest-loaded package，用来证明 storage/data contract 可以通过能力包表达，而不是进入内核 database namespace。

能力分组：

- storage contract：`describe_storage_contract`、`describe_backend_classes`
- package state：`plan_package_state_store`、`put_document_preview`、`get_document_preview`、`query_document_prefix_preview`、`delete_document_tombstone_preview`、`export_store_snapshot_preview`
- blob / asset：`describe_blob_store_contract`、`put_blob_preview`、`get_blob_metadata_preview`、`export_blob_manifest_preview`
- projection / index：`describe_projection_store_contract`、`plan_projection_materialization`、`query_projection_preview`、`migrate_projection_plan_preview`
- retrieval / multimodal：`describe_retrieval_provider_contract`、`draft_multimodal_index_plan`、`draft_vector_search_plan`、`explain_retrieval_backend_fit`

这些能力全部是可重放的预览或计划：

- 不写真实 DB
- 不读写文件
- 不联网
- 不生成 embedding
- 不存 vector
- 不 materialize projection
- 不保存 blob content
- 不返回 raw backend secret

## Forge Storage Inspector

`clients/web/src/storage/storage-inspector.ts` 通过公开协议调用 `official/storage-lab`，在 Forge 中展示：

- event spine 与 backend class 摘要
- package-scoped state plan
- blob/asset content-addressed contract
- projection/index materialization contract
- retrieval/TDB provider slot with opt-in Rust adapter proof
- multimodal index plan preview

Assistant drawer 也提供轻量 storage lane。它只展示 contract/readiness，不执行数据库操作。

## 当前验证

常用验证命令：

```bash
cargo test --workspace
cargo run -p plurora-cli -- conformance --tag storage
cargo run -p plurora-cli -- package check packages/official/storage-lab/manifest.yaml
```

## 下一步

PostgreSQL + TDB integration 已完成首个 opt-in backend/provider proof。后续接入其他 retrieval/vector 后端前仍应补：

- backend selection / host policy
- migration/export/import contract
- quota / retention / compaction policy
- content-addressed blob persistence
- projection rebuild scheduling
- retrieval provider permission/audit/redaction

这些都应继续走 package/provider + host policy，而不是把数据库产品语义放进内核。
