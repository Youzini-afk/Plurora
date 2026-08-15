# 性能与代码健康指南

> [English](./PERFORMANCE_AND_CODE_HEALTH.en.md) · [中文](./PERFORMANCE_AND_CODE_HEALTH.md)

本文档是性能与代码健康的长期指南。它取代临时计划，记录 Plurora 后续优化应遵守的测量、反馈环、结构、事件存储与 Web 渲染纪律。

## 原则

1. 先测量，再优化。使用 `cargo run -p plurora-cli -- perf baseline`、conformance timing、Web TypeScript diagnostics 和针对性单元测试定位热点。不要凭感觉替换架构。
2. 优化不得改变平台契约。第一方 Package 与第三方包必须继续走同一清单、能力、权限、钩子、schema、脱敏和审计路径。
3. UI 仍走公开协议。Web shell 不得读取 SQLite、runtime internals，也不得 special-case first-party Packages。
4. 不要用性能名义引入内容本体。不要新增 `platform.agent.*`、`platform.model.*`、`platform.memory.*`、`platform.experience.*`、`platform.sharing.*` 等内容或产品命名空间。
5. 高级优化必须有测量依据。能力或 surface cache、RawValue、registry helper/codegen、per-domain crates 等，都要由基线或 profiling 显示出实际收益空间。

## 常用命令

```bash
# workspace correctness
cargo test --workspace

# full charter/conformance gate with timings
cargo run -p plurora-cli -- conformance

# list/filter/fail-fast conformance during focused work
cargo run -p plurora-cli -- conformance --list
cargo run -p plurora-cli -- conformance --case sharing_lab
cargo run -p plurora-cli -- conformance --tag experience
cargo run -p plurora-cli -- conformance --fail-fast --slowest 5

# performance baseline without real network
cargo run -p plurora-cli -- perf baseline
cargo run -p plurora-cli -- perf baseline --format json
cargo run -p plurora-cli -- perf baseline --iterations 30 --warmup 3 --baseline-out perf/baseline.json
cargo run -p plurora-cli -- perf baseline --iterations 30 --compare perf/baseline.json --threshold-pct 10

# web correctness
tsc -p clients/web/tsconfig.json --noEmit
```

## Baseline 范围

`cargo run -p plurora-cli -- perf baseline` 当前覆盖：

- Rust in-process capability invoke。
- 第一方 Package 普通 capability invoke。
- subprocess echo invoke（Python 可用时）。
- in-memory event store append/list/range。
- scale scenarios：1k / 10k / 100k events。
- Work source check 与递归 Assembly 解析。
- profile YAML load。
- subprocess / outbound scenarios：cold start、handshake、1/10/100 KiB steady invoke、fake outbound execute throughput、fake stream TTFT，以及一个记录为 skipped 的 steady-stream slot。

输出 envelope 现在包含 `schema`、`created_at`、`git`、`env`；每个场景包含 p50/p95/p99、RSS delta 和必要时的 `iterations_capped`。已提交 [`../../perf/baseline.json`](../../perf/baseline.json) 作为 Linux 开发机参考，不是 CI 预算；后续优化应把它作为 regression reference。

前端侧性能诊断应使用实际存在的 Web 检查、浏览器 profiler 或针对性测试，不指向不存在的 helper 文件。YdlTavern 的 benchmark 约定位于独立仓库的 [`docs/guides/PERFORMANCE_BASELINE.md`](https://github.com/Youzini-afk/Plurora-Tavern/blob/main/docs/guides/PERFORMANCE_BASELINE.md)。

当前人测前基线还应关注 install/profile/surface/security bridge 路径：项目安装、profile autoload、静态 surface bundle 暴露、bridge allowlist、stream ownership、诊断脱敏与 secret 输入清理。

详细字段见 [`BASELINE.md`](./BASELINE.md)。

## Conformance 反馈环

Conformance 支持：

- `--list`：列出 case id、tag 和说明。
- `--case <pattern>`：按子串运行部分 case。
- `--tag <tag>`：按 tag 运行部分 case。
- `--fail-fast`：首个失败后停止。
- `--slowest <N>`：输出最慢 case。
- per-case duration：每个 case 都打印耗时。

新增 conformance case 时必须带 tag，避免反馈环重新变成不可筛选的巨型串行脚本。`--list` 输出 `<case_id>  [<tag>, ...]`；`--case` 与 `--tag` 同时使用时是 AND。完整 tag 域见 [`../spec/CONFORMANCE_MATRIX.md`](../spec/CONFORMANCE_MATRIX.md)。

## 结构优化纪律

已完成的低风险结构优化包括：

- protocol dispatch 按 domain helper 拆分，同时保持 `PlatformMethod` 为单一事实源。
- first-party in-process dispatch 从线性链改为 provider-indexed table。它仍使用 package-aware routing，不给 first-party fast path。
- shared inproc safety helper 收敛 raw-secret 和拒绝逻辑。
- Work/Package diagnostics 使用 set/index，避免明显的 O(n²) 扫描。

后续结构拆分应继续遵守：

- 公共协议 shape 不变。
- replacement/no-publisher-priority conformance 必须通过。
- 不为了少写 match 而把难审计的宏或生成物作为唯一 truth。

## Event store / replay 纪律

已完成：

- `EventStore::append_with_sequence` 原子追加 API。
- SQLite 与 in-memory store 在同一会话并发 append 时 sequence 不重复。
- `list_kind_prefix` / `list_session_kind_prefix` 查询下推。
- SQLite `kind` 与 `session+kind+sequence` 索引。
- 权限与出站审计避免常规 `list_all()` 全量过滤。

Storage backend 中立工作新增：

- `EventStore` trait 文档明确 backend-neutral event spine contract 定位。`append_with_sequence` 是 runtime 推荐 append path；`append` + `next_sequence` 是 low-level/test/admin path；排序语义按会话内 `(session_id, sequence)` 定义。kind-prefix query 是事件语义查询，不是 SQL/index product。契约不引入 SQL、table、vector 或 DSN 概念。
- In-memory 与 SQLite conformance parity：`storage_backend` tag conformance 用例覆盖基础契约、kind-prefix 等价性、并发 append 无重复、subscription 广播、rehydrate 事件重放语义一致。

`plurora/storage-lab` 提供 package-scoped storage/data 契约预览：

- `plurora/storage-lab` 作为普通 Package 提供 package-scoped storage/data 契约预览。它证明 storage 是普通 Package 层 capability，而非 kernel database/sql/vector API。
- 合约分层模型：event spine backend / package state store / blob store future / projection index future / retrieval provider future。
- Backend class 候选只含 capability flags，不含 secret-bearing backend config。
- Document CRUD preview 输出 write/read/query/delete/snapshot_performed=false，并返回脱敏内容。

Blob/asset store 契约证明新增：

- `plurora/storage-lab` 新增 blob/asset store 契约证明能力：describe_blob_store_contract、put_blob_preview、get_blob_metadata_preview、export_blob_manifest_preview。
- Blob 契约输出 content-addressed 类型、backend 候选（local_content_addressed_future / filesystem_backend_future / object_store_future）和红线（no blob content in events / no raw secrets / no filesystem path leak / content address required）。
- put_blob_preview 输出 content_address。提供 content_hash 时返回规范化 `sha256:`；否则返回确定性 hash。输出还包括 blob_stored=false、filesystem_performed=false、network_performed=false、event_payload_contains_blob=false。它阻断 raw secret、unsafe id 和过大的 inline sample（>4096 chars）。
- 不实现真实 blob store，不读写文件，不联网，也不把 blob content 放入 event payload。

Projection/index 物化契约证明新增：

- `plurora/storage-lab` 新增 projection/index 物化契约证明能力：describe_projection_store_contract、plan_projection_materialization、query_projection_preview、migrate_projection_plan_preview。
- Projection 契约输出 backend candidates（event_derived_projection / package_owned_index / sqlite_materialized_view_future / postgres_materialized_view_future）和红线（no_table_exposure / no_sql_exposure / no_secret_backend_config / no_query_product_leakage / projection_derives_from_events_assets_only）。
- plan_projection_materialization 输出 materialized=false、write_performed=false、backend_selected=false、plan_only=true。阻断 raw secret，校验 projection_id/package_id safe-id。
- query_projection_preview 输出 query_executed=false、rows_returned=false、preview_shape。不含 SQL、table、collection 或 vector 术语。
- migrate_projection_plan_preview 输出 migration_applied=false、data_rewritten=false、requires_rebuild=true。
- 不实现真实 projection storage，不建 DB table/index，不执行 SQL/query，也不重写数据。

Retrieval/vector/multimodal provider 契约证明新增：

- `plurora/storage-lab` 新增 retrieval/vector/multimodal provider 契约证明能力：describe_retrieval_provider_contract、draft_multimodal_index_plan、draft_vector_search_plan、explain_retrieval_backend_fit。
- Retrieval 契约输出 backend candidates（tdb_future / pgvector_future / local_embedding_index_future / remote_vector_provider_future / opensearch_vector_future / redis_vector_future）和红线（no_embedding_generation / no_vector_storage / no_network / no_secret_backend_config / no_kernel_vector_namespace / no_raw_vectors_in_output / no_distance_metric_leakage）。
- draft_multimodal_index_plan 输出 embedding_generated=false、index_created=false、vectors_stored=false、network_performed=false、plan_only=true。阻断 raw secret，校验 package_id/index_id safe-id，modalities 只允许 text/image/audio/video/structured，asset_refs 上限 64。
- draft_vector_search_plan 输出 search_executed=false、embedding_generated=false、vectors_loaded=false、plan_only=true。无实际搜索结果。
- explain_retrieval_backend_fit 输出 fit matrix，不含 secret-bearing backend config。TDB 只作为 future multimodal provider slot。
- 不实现真实 vector DB、TDB 或 embedding。不输出 raw vector、embedding 或 secret-bearing backend config。不新增 kernel vector/database/sql namespace。

后续 event store 优化优先级：

1. 用 baseline 证明具体规模下的瓶颈。
2. 优先改善 query、index 和 transaction，而不是改变 event payload contract。
3. 保持 event payload opaque；不要把内容语义塞进内核查询层。
4. 不绕过脱敏、schema、钩子或审计。

## Web render 纪律

已完成：

- 16ms 渲染调度器，避免 SSE/action burst 连续触发整页 render。
- 有界 JSON preview，限制 depth、array items、object keys 和 string length。
- Forge events/proposals/assets/projections/surfaces 显示 cap。
- event/proposal/surface/projection payload 默认显示 preview details。
- Web 正确性检查与浏览器 profiler 作为前端性能诊断入口。

后续 Web 优化优先级：

1. 先用 render diagnostics 或浏览器 profiler 证明慢点。
2. 优先拆分 view-model、renderer 和 detector，而不是替换 UI 框架。
3. 大 payload 默认折叠；需要时再展开。
4. SSE burst 必须 batching/debounce。
5. 禁止 Web 读取 runtime internals 或 SQLite。

## 何时考虑高级优化

只有满足以下条件时才考虑 cache、codegen、RawValue 等高级优化：

- 有基线或 profiler 数据证明某路径是瓶颈。
- 优化不会改变 public protocol 或 package equality。
- 脱敏、schema、钩子和审计仍显式可审计。
- 有 conformance 或 unit test 覆盖 invalidation、mismatch 和 hostile path。

当前没有证据要求引入 heavy codegen、RawValue rewrite、arena 或第一方 Package fast path；这些保持延后。
