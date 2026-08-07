# Performance and Code Health Guide

> [English](./PERFORMANCE_AND_CODE_HEALTH.en.md) · [中文](./PERFORMANCE_AND_CODE_HEALTH.md)

This is the long-term guide for performance and code health. It replaces the temporary plan and records the measurement, feedback-loop, structure, event-store, and web-rendering discipline future Plurora optimization should follow.

## Principles

1. Measure before optimizing. Use `cargo run -p plurora-cli -- perf baseline`, conformance timing, Web TypeScript diagnostics, and focused tests before changing architecture.
2. Optimization must not change the platform contract. Official and third-party packages must keep sharing the same manifest, capability, permission, hook, schema, redaction, and audit path.
3. UI stays on the public protocol. The web shell must not read SQLite, runtime internals, or special-case official packages.
4. Do not introduce content ontology in the name of performance. Do not add `platform.agent.*`, `platform.model.*`, `platform.memory.*`, `platform.experience.*`, `platform.sharing.*`, or similar product/content namespaces.
5. Advanced optimization needs evidence. Capability or surface caches, RawValue, registry helpers/codegen, per-domain crates, and similar changes require baseline or profiling evidence.

## Common commands

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

## Baseline scope

`cargo run -p plurora-cli -- perf baseline` currently covers:

- Rust in-process capability invocation.
- Ordinary official package capability invocation.
- Subprocess echo invocation when Python is available.
- In-memory event store append/list/range.
- Scale scenarios: 1k / 10k / 100k events.
- Composition check.
- Profile YAML load.
- Subprocess / outbound scenarios: cold start, handshake, 1/10/100 KiB steady invoke, fake outbound execute throughput, fake stream TTFT, and a steady-stream slot documented as skipped.

The output envelope now includes `schema`, `created_at`, `git`, and `env`; each scenario includes p50/p95/p99, RSS delta, and `iterations_capped` when applicable. The committed [`../../perf/baseline.json`](../../perf/baseline.json) is a Linux developer-machine reference, not a CI budget; future optimizations should use it as the regression reference.

Frontend performance diagnostics should use existing web checks, browser profilers, or focused tests; do not point to a helper file that is not present. YdlTavern's benchmark convention is documented in the independent repository's [`docs/guides/PERFORMANCE_BASELINE.en.md`](https://github.com/Youzini-afk/Plurora-Tavern/blob/main/docs/guides/PERFORMANCE_BASELINE.en.md).

The current pre-human-testing baseline should also watch install/profile/surface/security-bridge paths: project install, profile autoload, static surface-bundle serving, bridge allowlists, stream ownership, redacted diagnostics, and secret-input cleanup.

See [`BASELINE.en.md`](./BASELINE.en.md) for fields and limitations.

## Conformance feedback loop

Conformance supports:

- `--list`: list case ids, tags, and descriptions.
- `--case <pattern>`: run cases matching a substring.
- `--tag <tag>`: run cases matching a tag.
- `--fail-fast`: stop after the first failure.
- `--slowest <N>`: print the slowest cases.
- Per-case duration in normal output.

New conformance cases must declare tags so the suite does not become an unfilterable serial script again. See [`CONFORMANCE_FEEDBACK.en.md`](./CONFORMANCE_FEEDBACK.en.md).

## Structure discipline

Completed low-risk structural improvements:

- Protocol dispatch split into domain helpers while preserving `PlatformMethod` as the source of truth.
- Official in-process dispatch moved from a linear chain to a provider-indexed table. It still preserves package-aware routing and avoids official fast paths.
- Shared in-process safety helper for raw-secret and rejection logic.
- Composition/package diagnostics use sets/indexes to avoid obvious O(n²) scans.

Future structural splits should keep:

- Public protocol shapes unchanged.
- Replacement/no-official-priority conformance passing.
- No hard-to-review macros or generated artifacts as the sole truth.

## Event store / replay discipline

Completed:

- `EventStore::append_with_sequence` atomic append API.
- No duplicate sequence numbers under concurrent same-session append for SQLite and in-memory stores.
- `list_kind_prefix` / `list_session_kind_prefix` query pushdown.
- SQLite `kind` and `session+kind+sequence` indexes.
- Permission/outbound audit paths no longer routinely use `list_all()` plus full filtering.

Storage backend neutrality work added:

- `EventStore` trait documentation clarifies backend-neutral event spine contract positioning. `append_with_sequence` is the runtime-recommended append path; `append` + `next_sequence` is the low-level/test/admin path; ordering semantics are per-session `(session_id, sequence)`. Kind-prefix queries are event-semantic queries, not SQL/index product APIs. The contract has no SQL, table, vector, or DSN concepts.
- In-memory and SQLite conformance parity: `storage_backend` tag conformance cases cover the basic contract, kind-prefix equivalence, concurrent append without duplicates, subscription broadcast, and rehydrate event replay semantics.

`official/storage-lab` provides a package-scoped storage/data contract preview:

- `official/storage-lab` is an ordinary package that previews package-scoped storage/data contracts. It proves storage is a package-layer capability, not a kernel database/sql/vector API.
- Layered contract model: event spine backend / package state store / blob store future / projection index future / retrieval provider future.
- Backend class candidates contain capability flags only, no secret-bearing backend config.
- Document CRUD preview outputs write/read/query/delete/snapshot_performed=false with redacted content.

Blob/asset store contract proofs added:

- `official/storage-lab` adds blob/asset store contract proof capabilities: describe_blob_store_contract, put_blob_preview, get_blob_metadata_preview, export_blob_manifest_preview.
- Blob contract outputs content-addressed type, backend candidates (local_content_addressed_future / filesystem_backend_future / object_store_future), red lines (no blob content in events / no raw secrets / no filesystem path leak / content address required).
- put_blob_preview outputs content_address. It returns normalized `sha256:` when content_hash is provided, and a deterministic hash otherwise. It also outputs blob_stored=false, filesystem_performed=false, network_performed=false, event_payload_contains_blob=false. It blocks raw secret, unsafe id, and oversized inline sample (>4096 chars).
- No real blob store implementation, no filesystem reads/writes, no network, no blob content in event payloads.

Projection/index materialization contract proofs added:

- `official/storage-lab` adds projection/index materialization contract proof capabilities: describe_projection_store_contract, plan_projection_materialization, query_projection_preview, migrate_projection_plan_preview.
- Projection contract outputs backend candidates (event_derived_projection / package_owned_index / sqlite_materialized_view_future / postgres_materialized_view_future), red lines (no_table_exposure / no_sql_exposure / no_secret_backend_config / no_query_product_leakage / projection_derives_from_events_assets_only).
- plan_projection_materialization outputs materialized=false, write_performed=false, backend_selected=false, plan_only=true. Blocks raw secret, validates projection_id/package_id safe-id.
- query_projection_preview outputs query_executed=false, rows_returned=false, preview_shape. No SQL/table/collection/vector terms.
- migrate_projection_plan_preview outputs migration_applied=false, data_rewritten=false, requires_rebuild=true.
- No real projection storage, no DB table/index creation, no SQL/query execution, no data rewrite.

Retrieval/vector/multimodal provider contract proofs added:

- `official/storage-lab` adds retrieval/vector/multimodal provider contract proof capabilities: describe_retrieval_provider_contract, draft_multimodal_index_plan, draft_vector_search_plan, explain_retrieval_backend_fit.
- Retrieval contract outputs backend candidates (tdb_future / pgvector_future / local_embedding_index_future / remote_vector_provider_future / opensearch_vector_future / redis_vector_future), red lines (no_embedding_generation / no_vector_storage / no_network / no_secret_backend_config / no_kernel_vector_namespace / no_raw_vectors_in_output / no_distance_metric_leakage).
- draft_multimodal_index_plan outputs embedding_generated=false, index_created=false, vectors_stored=false, network_performed=false, plan_only=true. Blocks raw secret, validates package_id/index_id safe-id, modalities allow text/image/audio/video/structured only, asset_refs capped at 64.
- draft_vector_search_plan outputs search_executed=false, embedding_generated=false, vectors_loaded=false, plan_only=true. No actual search results.
- explain_retrieval_backend_fit outputs fit matrix without secret-bearing backend config. TDB is only a future multimodal provider slot.
- No real vector DB, TDB, or embedding implementation. No raw vector, embedding, or secret-bearing backend config output. No new kernel vector/database/sql namespace.

Future event-store optimization priority:

1. Prove a concrete scale bottleneck with baseline data.
2. Prefer query/index/transaction improvements over event payload contract changes.
3. Keep event payloads opaque; do not put content semantics into kernel query layers.
4. Do not bypass redaction, schema validation, hooks, or audit.

## Web render discipline

Completed:

- 16ms render scheduler to avoid repeated full renders during SSE/action bursts.
- Bounded JSON previews limiting depth, array items, object keys, and string length.
- Display caps for Forge events/proposals/assets/projections/surfaces.
- Event/proposal/surface/projection payloads render as preview details by default.
- Web correctness checks and browser profilers as frontend performance diagnostics entrypoints.

Future web optimization priority:

1. Prove the slow path with render diagnostics or a browser profiler.
2. Prefer partitioning view-models / renderers / detectors before changing UI frameworks.
3. Collapse large payloads by default; expand on demand.
4. Batch/debounce SSE bursts.
5. Forbid runtime-internal or SQLite reads from the web client.

## When to consider advanced optimization

Consider cache/codegen/RawValue-like optimizations only when:

- Baseline or profiler data proves the path is a bottleneck.
- The optimization does not change the public protocol or package equality.
- Redaction/schema/hook/audit behavior remains explicit and reviewable.
- Conformance or unit tests cover invalidation, mismatch, and hostile paths.

There is currently no evidence requiring heavy codegen, RawValue rewrites, arenas, or official-package fast paths; keep them deferred.
