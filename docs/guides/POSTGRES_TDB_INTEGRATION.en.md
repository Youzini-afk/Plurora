# PostgreSQL + TDB Integration

> [English](./POSTGRES_TDB_INTEGRATION.en.md) · [中文](./POSTGRES_TDB_INTEGRATION.md)

This guide records the PostgreSQL and TDB integration boundary. PostgreSQL is an optional host-owned `EventStore` backend. TDB/TriviumDB is an ordinary retrieval and multimodal provider adapter path with a Rust API adapter proof, not a kernel database.

## PostgreSQL event store

`plurora-runtime` provides a feature-gated `PostgresEventStore`:

- feature: `postgres`
- driver: `tokio-postgres` + `deadpool-postgres`
- schema: `events` table, `unique(session_id, sequence)`, session/range/kind/session+kind indexes, payload/metadata as JSONB
- per-session sequence: session-scoped advisory lock + `max(sequence)+1` + unique constraint inside a transaction
- subscribe: host-local broadcast for now, not PostgreSQL LISTEN/NOTIFY
- default: disabled; ordinary builds, CI, and conformance are unaffected

Real PostgreSQL conformance only runs when explicitly requested:

```bash
cargo run -p plurora-cli --features postgres -- conformance --tag postgres
```

with the host-owned test environment variable set:

```text
PLURORA_POSTGRES_TEST_DATABASE_URL
```

The value must never be written to package manifests, events, proposals, logs, or public diagnostics.

## Host profile backend selection

Host profiles can select event-store backends:

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

Example:

```text
profiles/forge-postgres.example.yaml
```

The profile references only an env var name. The real connection details belong only to the host runtime. Host stdout diagnostics show backend kind plus redacted status only.

## TDB / TriviumDB route

The TDB source review lives at:

```text
integrations/tdb/TRIVIUMDB_REVIEW.en.md
```

Conclusion: TriviumDB/TDB fits as a retrieval and multimodal provider adapter, not as:

- kernel event store;
- canonical asset store;
- projection authority;
- raw package database;
- global memory/chat/agent/world store.

TDB is valuable as a local embedded vector, graph, document, and multimodal hybrid retrieval engine. Plurora events, permissions, proposals, branch lineage, and audit still need the event spine as their substrate.

## `plurora/tdb-retrieval-lab`

New ordinary first-party Package:

```text
packages/plurora/tdb-retrieval-lab
```

Capabilities:

```text
describe_tdb_retrieval_contract
draft_tdb_index_plan
draft_tdb_query_plan
explain_tdb_backend_fit
inspect_tdb_adapter_surface
describe_real_tdb_opt_in_seam
```

This package remains a replayable contract/plan layer:

- no real TDB crate linkage (real calls are handled by the `tdb-rust-adapter` opt-in proof)
- no backend open
- no index creation
- no embedding generation
- no vector storage
- no network
- no filesystem access
- no raw backend secret saved or returned

Real TDB wiring is handled by `plurora/tdb-rust-adapter` and `integrations/tdb/rust-adapter-real-crate`; `tdb-retrieval-lab` stays as the default-safe contract/plan layer.

## `plurora/tdb-rust-adapter`

Explicitly loaded ordinary subprocess package:

```text
examples/packages/tdb-rust-adapter/manifest.yaml
```

Adapter source:

```text
integrations/tdb/rust-adapter
integrations/tdb/rust-adapter-real-crate
```

Default adapter:

- can be loaded by the Plurora runtime as an ordinary subprocess package;
- provides `describe_real_tdb_adapter` and `run_real_tdb_smoke`;
- has no `triviumdb` dependency;
- opens no backend;
- makes `run_real_tdb_smoke` return `real_tdb_available=false` and `smoke_executed=false`.

Real published-crate proof:

```bash
cargo test --manifest-path integrations/tdb/rust-adapter-real-crate/Cargo.toml --features real-tdb
```

That proof uses the published `triviumdb = "0.7.0"` crate and actually calls:

```text
Database::<f32>::open_with_config
insert
link
search
search_hybrid
```

It uses a temporary redacted store, exposes no raw path, performs no network, and does not enter the default workspace build. Default profiles do not open a real backend in order to preserve host policy, approval, and resource-limit boundaries. The real Rust proof uses the published `triviumdb = "0.7.0"` crate, not a local absolute path or developer-machine path override.

Recommended real-mode order:

1. Subprocess adapter package: preferred. It isolates native dependency, file lock, panic, repair, and compaction lifecycles.
2. Feature-gated in-process adapter: only when TDB is resolvable in a stable way (published, vendored, submodule, or pinned git rev) and the host explicitly accepts native in-process risk.

Example profile shape:

```text
examples/tdb-provider-profiles/tdb-local.example.json
```

## UI

Forge Storage Inspector calls through public protocol:

```text
plurora/storage-lab
plurora/tdb-retrieval-lab
plurora/tdb-rust-adapter (only when explicitly loaded)
```

It displays:

- event spine / backend classes
- package state / blob / projection contracts
- retrieval provider slot
- TDB adapter contract
- real TDB opt-in seam readiness
- real TDB Rust adapter shell / real-crate proof status

The Web shell does not read SQLite/PostgreSQL/TDB, local filesystems, or runtime internals.

## Red lines

Do not add:

```text
platform.postgres.*
platform.sql.*
platform.database.*
platform.tdb.*
platform.vector.*
platform.embedding.*
```

Packages must not receive raw PostgreSQL pools, SQL, DSNs, TDB paths, backend topology, or raw backend errors.

## Validation

Common validation commands:

```bash
cargo test --workspace
cargo run -p plurora-cli -- conformance --tag storage
cargo run -p plurora-cli -- conformance --tag tdb
cargo run -p plurora-cli -- package check packages/plurora/tdb-retrieval-lab/manifest.yaml
cargo run -p plurora-cli -- package check examples/packages/tdb-rust-adapter/manifest.yaml
cargo check -p plurora-cli --features postgres
```
