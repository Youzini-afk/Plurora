# Storage Backend Neutrality

> [English](./STORAGE_BACKEND_NEUTRALITY.en.md) · [中文](./STORAGE_BACKEND_NEUTRALITY.md)

Plurora currently has a SQLite-backed append-only event log, but the platform contract must not be tied to SQLite. SQLite, PostgreSQL, TDB, object stores, and vector/multimodal retrieval systems belong behind backend/provider seams, not in kernel ontology or public protocol database products.

## Layered model

1. Kernel event spine: content-free append, replay, range, kind-prefix, subscription, and rehydrate semantics. In-memory and SQLite backends exist today; PostgreSQL can become a future host/runtime backend without changing protocol.
2. Package-scoped state store: ordinary capability packages may preview package-owned document/KV state contracts, but they do not receive raw DB access, SQL, or backend credentials.
3. Blob / asset store: large objects should enter the platform through content address, hash, size, mime, and provenance. Blob content should not be embedded into event payloads.
4. Projection / index materialization: projections/indexes are package-owned views derived from events/assets. They can plan materialization, query preview, and migration plans without exposing tables, SQL, or query-product semantics.
5. Retrieval / vector / multimodal providers: TDB, pgvector, OpenSearch, Redis Vector, local embedding indexes, and remote retrieval services are provider slots. TDB now has an opt-in Rust adapter proof, but retrieval remains a package/provider-layer ability. It does not replace the event log, audit, proposal lifecycle, or branch lineage.
6. Forge observability: the web shell uses public protocol calls to `official/storage-lab` to display contract summaries. It does not read SQLite, PostgreSQL, TDB, filesystem state, or runtime internals.

## Red lines

- Do not add `platform.sqlite.*`, `platform.postgres.*`, `platform.tdb.*`, `platform.vector.*`, `platform.embedding.*`, `platform.collection.*`, `platform.sql.*`, or `platform.database.*`.
- Do not turn `EventStore` into a generic `DatabaseBackend`.
- Do not expose SQL, DSNs, connection strings, tables, transaction isolation, ANN indexes, vector dimensions, backend topology, or raw credentials to packages.
- SQLite is an early/default/local backend, not the platform contract.
- PostgreSQL is a future server/team backend, not a package API.
- TDB is a future multimodal retrieval provider slot, not the kernel database.
- Retrieval/vector/multimodal search must not replace append-only events, audit, proposal lifecycle, or branch/fork/replay.

## `official/storage-lab`

`official/storage-lab` is an ordinary manifest-loaded package proving that storage/data contracts can be expressed as capabilities instead of kernel database namespaces.

Capability groups:

- storage contract: `describe_storage_contract`, `describe_backend_classes`
- package state: `plan_package_state_store`, `put_document_preview`, `get_document_preview`, `query_document_prefix_preview`, `delete_document_tombstone_preview`, `export_store_snapshot_preview`
- blob / asset: `describe_blob_store_contract`, `put_blob_preview`, `get_blob_metadata_preview`, `export_blob_manifest_preview`
- projection / index: `describe_projection_store_contract`, `plan_projection_materialization`, `query_projection_preview`, `migrate_projection_plan_preview`
- retrieval / multimodal: `describe_retrieval_provider_contract`, `draft_multimodal_index_plan`, `draft_vector_search_plan`, `explain_retrieval_backend_fit`

All of these capabilities are replayable previews or plans:

- no real DB writes
- no filesystem reads/writes
- no network
- no embedding generation
- no vector storage
- no projection materialization
- no blob content persistence
- no raw backend secret output

## Forge Storage Inspector

`clients/web/src/storage/storage-inspector.ts` calls `official/storage-lab` through public protocol and shows the following in Forge:

- event spine and backend class summaries
- package-scoped state plan
- blob/asset content-addressed contract
- projection/index materialization contract
- retrieval/TDB provider slot with opt-in Rust adapter proof
- multimodal index plan preview

The Assistant drawer also has a lightweight storage lane. It displays contract/readiness only and does not execute database operations.

## Current validation

Common validation commands:

```bash
cargo test --workspace
cargo run -p plurora-cli -- conformance --tag storage
cargo run -p plurora-cli -- package check packages/official/storage-lab/manifest.yaml
```

## Next steps

PostgreSQL + TDB integration has completed the first opt-in backend/provider proof. Before adding more retrieval/vector backends, Plurora should add:

- backend selection / host policy
- migration/export/import contracts
- quota / retention / compaction policy
- content-addressed blob persistence
- projection rebuild scheduling
- retrieval provider permission/audit/redaction

These should continue to use package/provider + host policy seams instead of putting database-product semantics into the platform.
