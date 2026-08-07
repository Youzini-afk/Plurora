# Object / Artifact Foundation (Experimental)

> [English](./OBJECT_STORE.en.md) · [中文](./OBJECT_STORE.md)

This document defines the currently implemented content-addressed object foundation. It is an Experimental Constitutional Substrate contract and does not change the `kernel.v1.asset.*` method IDs or their existing request shapes.

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

Object bytes live only in ObjectStore. Journals, events, and future receipts store descriptors or digest references and must not copy large bodies. The `kernel/v1/asset.put` event payload carries the additive `AssetRecord.descriptor`; event metadata carries only `artifact_digest`, `size_bytes`, and `content_included: false`.

This boundary does not change secret policy: asset content remains arbitrary user data and is not raw-secret scanned, while asset metadata continues to use the existing raw-secret rejection rule.

## v1 Asset adapter

`kernel.v1.asset.put/get/list` remain wire-compatible:

- `put` commits UTF-8 content as a generic blob artifact;
- `AssetRecord.hash` is now the canonical SHA-256 digest;
- `AssetRecord.descriptor` is an additive optional field that old clients may ignore;
- `get` reads and verifies through the descriptor, then adapts bytes back to the v1 String content;
- `list` returns records without loading object bodies.

FNV-1a remains available only through `legacy_content_address()` and the explicit `scheme: "fnv1a64"` compatibility path. It cannot become canonical identity for new objects.

## Legacy event migration

When rehydration reads an old `kernel/v1/asset.put` event containing `metadata.content`, it:

1. commits the old content idempotently to ObjectStore;
2. computes a SHA-256 descriptor and corrects the canonical hash/size;
3. preserves the old asset id, old FNV hash, original event id, sequence, and session id in annotations;
4. neither mutates the old event nor appends a migration event.

Migration is therefore interruptible and repeatable, with CAS providing natural deduplication. For new events without inline content, a missing object, digest mismatch, size mismatch, or media-type mismatch is an explicit failure; rehydration must never substitute an empty string.

## Failure and deployment boundaries

An object is committed to CAS before its referencing event is appended. A failed event append may therefore leave an unreachable object, but it cannot return a successful reference to missing bytes. Future reachability-based GC uses the journal as its root set; the failure path must not directly delete a digest that another event may share. The filesystem implementation uses a temporary file, file sync, and atomic rename; on Unix it also syncs the parent directory after publication.

The default host stores objects under `<data-dir>/objects`. Moving a SQLite journal requires moving that directory with it; hosts sharing a PostgreSQL event store must likewise deploy/configure a shared object backend. Remote object backends and reachability GC remain later runtime work and do not change the current digest/descriptor contract.

## Executable acceptance

- `asset.put_get_list` uses 1 MiB+ content to verify SHA-256 descriptors, v1 reads, and content-free events;
- `asset.legacy_fnv_migration` verifies idempotent legacy migration and provenance retention;
- `object_store.portability_integrity` verifies cross-host digest equality, unknown-type copying, streaming, and tamper rejection;
- `substrate.sqlite_rehydrate` verifies restart recovery using the SQLite journal plus an independent filesystem object directory.
