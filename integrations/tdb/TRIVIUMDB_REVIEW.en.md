# TriviumDB / TDB Integration Review

> [English](./TRIVIUMDB_REVIEW.en.md) · [中文](./TRIVIUMDB_REVIEW.md)

This ledger records what the TriviumDB source review means for Plurora integration, and corrects the previous overly conservative judgment: the TriviumDB README explicitly recommends Rust-side `cargo add triviumdb`, so Plurora should provide a real Rust API adapter proof, not just a plan-only entry.

## Source observations

- `README.md` / `README_EN.md`: installation docs include `cargo add triviumdb`; Rust library integration is the intended route.
- `Cargo.toml`: crate name `triviumdb`, version `0.7.1`, `crate-type = ["cdylib", "rlib"]`, no default features, optional `python` / `nodejs` / `cli` features.
- `src/lib.rs`: crate root exposes `Database`, `Result`, `TriviumError`, `Filter`, and hook/node/vector related types; `Config`, `SearchConfig`, and `StorageMode` must be imported from `triviumdb::database`.
- `src/database/config.rs`: `StorageMode::{Mmap, Rom}`, `Config { dim, sync_mode, storage_mode }`, and `SearchConfig` with `top_k`, `expand_depth`, hybrid text, DPP, PPR, payload filters, and more.
- `src/database/mod.rs`: `Database<T>::open/open_with_config` opens local files, creates directories, lock files, and WAL; supports `insert`, `insert_with_id`, `link`, `begin_tx`, `search`, `search_advanced`, `search_hybrid`, and `search_hybrid_with_context`.
- `src/database/transaction.rs`: transactions use dry-run + WAL-first commit semantics; WAL replay handles insert/link/delete/update operations.
- `triviumdb.d.ts`: Node binding exposes the `TriviumDB` class, vectors, payloads, filters, search config, and search hits.

## Integration judgment

TDB fits Plurora as a **retrieval/multimodal provider adapter**, not as:

- platform event store;
- canonical asset store;
- projection authority;
- raw package database;
- global memory/chat/agent/world store.

Reason: TDB is valuable as a local embedded multimodal/vector/graph/document hybrid retrieval engine; Plurora events, proposals, permissions, and branch lineage still need the event spine as authority.

## Current real Rust adapter proof

This line adds a real Rust integration proof:

```text
integrations/tdb/rust-adapter
integrations/tdb/rust-adapter-real-crate
examples/packages/tdb-rust-adapter/manifest.yaml
```

Default adapter:

- is an ordinary JSON-RPC stdio subprocess package;
- has no `triviumdb` dependency;
- can be loaded and invoked by the Plurora runtime;
- makes `run_real_tdb_smoke` report `real_tdb_available=false` instead of pretending success.

Real published-crate proof:

```bash
cargo test --manifest-path integrations/tdb/rust-adapter-real-crate/Cargo.toml --features real-tdb
```

That proof explicitly uses the published `triviumdb = "0.7.0"` crate and actually calls:

```rust
Database::<f32>::open_with_config(...)
insert(...)
link(...)
search(...)
search_hybrid(...)
```

The real proof uses a temporary redacted store, does not expose raw paths, performs no network, and does not enter the default main workspace build.

## Why default profiles do not open a real backend

This is not “not doing it because TDB is outside the repo”; the real Rust adapter proof is done, and committed configuration uses the published `triviumdb = "0.7.0"` crate instead of a local absolute path or developer-machine path override. Default profiles do not open a real backend in order to preserve host policy, user approval, resource limits, redaction, and lifecycle ownership boundaries.

Therefore the route is dual-track:

1. default adapter shell: compiles, loads, and passes conformance in an ordinary Plurora checkout; it opens no backend;
2. real-crate adapter: explicit opt-in real Rust API proof through the published crate; unpublished source tests should use a developer-owned uncommitted Cargo patch.

If TDB later becomes stably resolvable through crates.io, pinned git rev, submodule, or vendor, the real adapter can move from published-crate proof into a more formal feature-gated package build.

## Recommended real-mode order

1. **Subprocess adapter package**: preferred. It isolates native dependency, file lock, panic, repair, and compaction lifecycles, and can use cross-platform binaries or Node/Python bindings.
2. **Feature-gated in-process adapter**: only after TDB is vendored or published and the host explicitly accepts native in-process risk.
