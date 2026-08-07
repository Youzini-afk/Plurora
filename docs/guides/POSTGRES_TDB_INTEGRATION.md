# PostgreSQL + TDB Integration

> [English](./POSTGRES_TDB_INTEGRATION.en.md) · [中文](./POSTGRES_TDB_INTEGRATION.md)

本指南记录 PostgreSQL 与 TDB 的接入边界。PostgreSQL 是可选的 host-owned `EventStore` backend。TDB/TriviumDB 是普通检索和多模态 provider adapter 路线，已有 Rust API adapter proof，但不是内核数据库。

## PostgreSQL event store

`plurora-runtime` 提供 feature-gated `PostgresEventStore`：

- feature：`postgres`
- driver：`tokio-postgres` + `deadpool-postgres`
- schema：`events` table、`unique(session_id, sequence)`、session/range/kind/session+kind indexes，payload/metadata 使用 JSONB
- per-session sequence：在 transaction 内使用 session-scoped advisory lock、`max(sequence)+1` 和 unique constraint
- subscribe：当前为 host-local broadcast，不依赖 PostgreSQL LISTEN/NOTIFY
- 默认：不启用，不影响普通 build / CI / conformance

真实 PostgreSQL conformance 只有在同时满足以下条件时运行：

```bash
cargo run -p plurora-cli --features postgres -- conformance --tag postgres
```

并设置 host-owned 测试环境变量：

```text
PLURORA_POSTGRES_TEST_DATABASE_URL
```

环境变量值不得写入 package manifest、event、proposal、log 或 public diagnostics。

## Host profile backend selection

Host profile 支持选择 event store backend：

```yaml
event_store:
  kind: memory
```

```yaml
event_store:
  kind: sqlite
  path: ./forge-alpha.sqlite
```

```yaml
event_store:
  kind: postgres
  env: PLURORA_POSTGRES_DATABASE_URL
```

示例：

```text
profiles/forge-postgres.example.yaml
```

注意：profile 只引用 env var 名称。真实连接信息只属于 host runtime。Host stdout diagnostics 只显示 backend kind 与脱敏状态。

## TDB / TriviumDB route

TDB 源码 review 位于：

```text
integrations/tdb/TRIVIUMDB_REVIEW.md
```

结论：TriviumDB/TDB 适合成为检索和多模态 provider adapter，不适合成为：

- kernel event store；
- canonical asset store；
- projection authority；
- package raw database；
- global memory/chat/agent/world store。

原因是 TDB 的强项是本地嵌入式向量、图、文档和多模态混合检索。Plurora 的事件、权限、提案、分支 lineage 和审计仍应由 event spine 承担。

## `official/tdb-retrieval-lab`

新增普通官方包：

```text
packages/official/tdb-retrieval-lab
```

能力：

```text
describe_tdb_retrieval_contract
draft_tdb_index_plan
draft_tdb_query_plan
explain_tdb_backend_fit
inspect_tdb_adapter_surface
describe_real_tdb_opt_in_seam
```

该包仍是可重放的 contract/plan 层：

- 不链接真实 TDB crate（真实调用由 `tdb-rust-adapter` opt-in proof 负责）
- 不打开 backend
- 不创建 index
- 不生成 embedding
- 不存 vector
- 不联网
- 不读写文件
- 不保存或输出 raw backend secret

真实 TDB 接线由 `official/tdb-rust-adapter` 与 `integrations/tdb/rust-adapter-real-crate` 承担。`tdb-retrieval-lab` 保持为默认安全的 contract/plan 层。

## `official/tdb-rust-adapter`

新增显式加载的普通 subprocess package：

```text
examples/packages/tdb-rust-adapter/manifest.yaml
```

adapter 源码：

```text
integrations/tdb/rust-adapter
integrations/tdb/rust-adapter-real-crate
```

默认 adapter：

- 可由 Ygg runtime 作为 ordinary subprocess package 加载；
- 提供 `describe_real_tdb_adapter` 与 `run_real_tdb_smoke`；
- 不依赖 `triviumdb`；
- 不打开 backend；
- `run_real_tdb_smoke` 返回 `real_tdb_available=false` 与 `smoke_executed=false`。

真实 published-crate proof：

```bash
cargo test --manifest-path integrations/tdb/rust-adapter-real-crate/Cargo.toml --features real-tdb
```

该 proof 使用已发布 `triviumdb = "0.7.0"` crate，并真实调用：

```text
Database::<f32>::open_with_config
insert
link
search
search_hybrid
```

它使用临时脱敏 store，不输出 raw path，不联网，不进入默认 workspace build。默认 profile 不打开真实 backend，是为了保留 host policy、approval 和 resource limit 边界。真实 Rust proof 走已发布 `triviumdb = "0.7.0"` crate，而不是本地绝对路径或开发机路径覆盖。

推荐真实模式顺序：

1. 子进程 adapter package：优先。隔离 native dependency、file lock、panic、repair/compaction 生命周期。
2. feature-gated in-process adapter：仅在 TDB 可稳定解析（发布、vendored、submodule 或固定 git rev）且 host 显式接受 native in-process 风险时启用。

示例 profile shape：

```text
examples/tdb-provider-profiles/tdb-local.example.json
```

## UI

Forge Storage Inspector 通过 public protocol 调用：

```text
official/storage-lab
official/tdb-retrieval-lab
official/tdb-rust-adapter（仅显式加载时）
```

展示：

- event spine / backend classes
- package state / blob / projection contracts
- retrieval provider slot
- TDB adapter contract
- real TDB opt-in seam readiness
- real TDB Rust adapter shell / real-crate proof status

Web shell 不读 SQLite/PostgreSQL/TDB、本地文件系统或 runtime internals。

## 红线

不得新增：

```text
kernel.v1.postgres.*
kernel.v1.sql.*
kernel.v1.database.*
kernel.v1.tdb.*
kernel.v1.vector.*
kernel.v1.embedding.*
```

不得让包获得 raw PostgreSQL pool、SQL、DSN、TDB path、backend topology 或 raw backend error。

## 验证

常用验证命令：

```bash
cargo test --workspace
cargo run -p plurora-cli -- conformance --tag storage
cargo run -p plurora-cli -- conformance --tag tdb
cargo run -p plurora-cli -- package check packages/official/tdb-retrieval-lab/manifest.yaml
cargo run -p plurora-cli -- package check examples/packages/tdb-rust-adapter/manifest.yaml
cargo check -p plurora-cli --features postgres
```
