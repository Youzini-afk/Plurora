# Object / Artifact Foundation (Experimental)

> [English](./OBJECT_STORE.en.md) · [中文](./OBJECT_STORE.md)

This document defines the currently implemented content-addressed object foundation. It is an Experimental Constitutional Substrate contract; Work, Installation, and their upload scopes are product protocols layered above it, not Constitutional Substrate concepts.

## Identity and descriptors

Object identity is determined only by the digest of its bytes. The required initial algorithm uses the strict form `sha256:<64 lowercase hexadecimal characters>`. The algorithm prefix is part of persistent identity; readers must preserve it, and unknown algorithms must be rejected explicitly rather than silently interpreted as SHA-256.

Portable metadata uses:

```text
ArtifactDescriptor
├── artifact_type_uri
├── media_type
├── digest
├── size_bytes
├── references[]
└── annotations{}
```

`artifact_type_uri` is an open URI. A host that does not understand the type must still be able to copy, export, and verify the bytes while preserving the descriptor. Portable identity must not include host absolute paths, PIDs, temporary URLs, or other host-local transient values.

## ObjectStore contract

`ObjectStore` exposes five asynchronous operations:

- `put(bytes)` computes SHA-256, stores idempotently, and returns digest/size;
- `get(digest)` returns full bytes only after verifying their digest;
- `has(digest)` checks whether an object exists;
- `verify(digest)` recomputes the digest as a stream and returns verified size;
- `stream(digest)` opens a read stream after an integrity preflight and verifies the bytes actually emitted again at EOF; callers must discard consumed output if terminal verification fails.

The current implementations are in-memory and filesystem-backed stores. Filesystem layout is an implementation detail; callers depend only on the digest. Concurrent writes of identical bytes must converge on one object, and temporary writes must complete and sync before atomic publication.

## Separating bytes from journals

Object bytes live only in ObjectStore. Journals, events, and future receipts store descriptors or digest references and must not copy large bodies. An ordinary Asset `object.put` event carries `AssetRecord.descriptor`; event metadata carries only `artifact_digest`, `size_bytes`, and `content_included: false`. An exact CAS upload creates no Asset event.

This boundary does not change secret policy: asset content remains arbitrary user data and is not raw-secret scanned, while asset metadata continues to use the existing raw-secret rejection rule.

## The two `object.put` identities

The public result is uniformly `ObjectPutResponse { asset, descriptor }`, but every request must belong to exactly one of these modes:

- An ordinary Asset omits `artifact`. The Host commits UTF-8 content as a generic blob, creates an `AssetRecord` and `EVENT_ASSET_PUT`, and returns `asset: Some(...)`. `object.get` preserves its original wire contract: request `{ "asset_id": string }`, response `{ "record": AssetRecord, "content": string }`. `object.list` likewise covers only these records.
- An exact CAS upload must carry `ExactArtifactUpload`, whose descriptor, encoding, and bytes must agree exactly, plus a tagged `ObjectPutScope`. The Host only performs an idempotent `ObjectStore.put` and returns `asset: None` with the exact descriptor. It allocates no `asset_id`, appends no Asset event, never appears in `object.list`, and naturally converges across retries and Host restarts through CAS.

`ObjectPutScope` has only `installation_create { work_id }` and `installation_update { installation_id, work_id }`. An ordinary Asset carrying scope, an exact upload missing scope, or unknown fields must fail closed. Scope only declares which subsequent Installation mutation will consume the object; it carries no local path, raw bytes, or secret and grants no authority.

The HTTP Service and Runtime authorize the same typed params independently. An ordinary Asset requires `access_manage` plus an all-installation selector. A create exact upload requires `installation.manage` and exact `host/work/<work_id>`; an update exact upload additionally requires exact `host/installation/<installation_id>`. The Installation mutation separately checks current authority, its persisted request, and receipts; a plan or successful upload never authorizes the mutation by itself.

Installation state audit on `object.get` is a separate, unambiguous branch: request `{ "installation_state_artifact": ArtifactDescriptor }`, response `{ "descriptor", "content", "content_encoding" }`. It reads only explicitly public state decision receipts / authority evidence whose complete descriptor matches one issued by the authoritative Installation journal. Snapshots, generic exact objects, forged descriptors, and tampered descriptors cannot be read through either branch. The retired tagged `kind: "asset"` form is not an alias.

FNV-1a remains available only through `legacy_content_address()` and the explicit `scheme: "fnv1a64"` compatibility path. It cannot become canonical identity for new objects.

## Legacy event migration

When rehydration reads an old `object/put` event containing `metadata.content`, it:

1. commits the old content idempotently to ObjectStore;
2. computes a SHA-256 descriptor and corrects the canonical hash/size;
3. preserves the old asset id, old FNV hash, original event id, sequence, and session id in annotations;
4. neither mutates the old event nor appends a migration event.

Migration is therefore interruptible and repeatable, with CAS providing natural deduplication. For new events without inline content, a missing object, digest mismatch, size mismatch, or media-type mismatch is an explicit failure; rehydration must never substitute an empty string.

## Failure and deployment boundaries

An ordinary Asset is committed to CAS before its referencing event is appended. A failed event append may therefore leave an unreachable object, but it cannot return a successful reference to missing bytes. An exact upload promises only that CAS stored and verified its descriptor; if the subsequent Installation mutation fails, the object may remain temporarily unreachable, and the failure path must not delete a digest that may be shared. Future reachability-based GC uses the journal as its root set. The filesystem implementation uses a temporary file, file sync, and atomic rename; on Unix it also syncs the parent directory after publication.

The default host stores objects under `<data-dir>/objects`. Moving a SQLite journal requires moving that directory with it; hosts sharing a PostgreSQL event store must likewise deploy/configure a shared object backend. Remote object backends and reachability GC remain later runtime work and do not change the current digest/descriptor contract.

## Executable acceptance

- `asset.put_get_list` uses 1 MiB+ content to verify SHA-256 descriptors, v1 reads, and content-free events;
- `asset.legacy_fnv_migration` verifies idempotent legacy migration and provenance retention;
- `object_store.portability_integrity` verifies cross-host digest equality, unknown-type copying, streaming, and tamper rejection;
- scoped exact `object.put` verifies exact Work/Installation authority, no Asset/event/list leakage, retry and restart idempotency, and that an upload failure cannot submit an Installation mutation;
- `substrate.sqlite_rehydrate` verifies restart recovery using the SQLite journal plus an independent filesystem object directory.
