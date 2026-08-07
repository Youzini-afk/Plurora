# Experimental World Bundle

> [English](./WORLD_BUNDLE.en.md) · [中文](./WORLD_BUNDLE.md)

Status: Experimental profile `plurora.world.bundle/experimental/v1`.

World Bundle is an optional Profile for carrying, reading, and migrating a persistent higher-level World across Hosts without making `World` a substrate type. The concrete archive is readable, integrity-checkable, and auditable without loading the original Package, starting a Component, enabling a model provider, or mounting the Web Shell.

## Archive shape

[`world-bundle.schema.json`](v1/schemas/world-bundle.schema.json) publishes `WorldBundleArchive`:

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

`bundle_descriptor.digest` is the SHA-256 digest of the canonical manifest JSON. Its references enumerate the portable object inventory. Every inline object is rehashed and size-checked before import. Artifact references inside a portable bundle are SHA-256 digests, never host paths, process IDs, temporary URLs, or package-local runtime handles.

The digest identifies bytes. Artifact type, media type, role annotations, and other descriptor metadata are manifest-protected description views; they do not create a second object identity for the same digest. The digest-keyed inventory keeps one canonical view with the union of closure references, while role-local views may use different types, media types, or annotations as long as their digest and size agree and every declared reference is covered by the inventory. Unknown artifact type URIs remain valid and are copied byte-for-byte.

## Head, journal, and lineage

[`world-head.schema.json`](v1/schemas/world-head.schema.json) publishes the current protocol-defined head:

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

[`world-journal-range.schema.json`](v1/schemas/world-journal-range.schema.json) binds one session ID to a contiguous inclusive sequence range and one content-addressed artifact per original v1 `EventEnvelope`. Cross-session global ordering is not invented: the v1 contract guarantees order within a session, while lineage and parent heads express causality across branches.

The original envelope bytes produced at export are retained as objects. Import preserves event IDs, session IDs, sequence numbers, timestamps, writers, kinds, payloads, and metadata. A derived head points to its parent head; re-executing a step never mutates the imported head or receipt.

## Lifecycle

The implemented lifecycle is:

1. Select one or more contiguous journal ranges and a state root.
2. Pin the composition and exact protocol profiles.
3. Materialize event envelopes, receipts, policy/provenance records, and all transitive objects in the SHA-256 ObjectStore.
4. Compute and verify the complete reference closure.
5. Export the canonical manifest plus base64 object payloads.
6. Verify the entire archive before writing anything on the destination host.
7. Import objects and exact envelopes into an empty scope, then rehydrate supported substrate projections.
8. Audit or historically replay envelopes and receipt outputs without invoking any executor.
9. Optionally install another implementation, execute on a new session branch, and export a child head whose lineage names the imported parent.

ObjectStore and EventStore do not share a cross-backend transaction. Import therefore validates the complete archive, checks destination session emptiness, and holds an import lock before writes. The built-in in-memory and SQLite EventStores recheck all selected sessions as empty and append the complete event batch in one atomic operation; an EventStore without that stronger operation, including the current PostgreSQL implementation, fails before object writes. Session reconstruction and supported substrate projections are validated before the journal commit, then the imported projection entries are merged without fallible decoding or replacing unrelated runtime entries. An ObjectStore failure or rejected event batch can leave immutable, unreachable CAS objects, but cannot commit a partial event journal or silently rewrite an object.

## Headless CLI

The archive is a shell-independent data artifact:

```text
plurora world-bundle verify <archive.json> [--json]
plurora world-bundle audit <archive.json> [--json]
plurora world-bundle replay <archive.json> [--json]
plurora world-bundle import <archive.json> --data-dir <fresh-dir> [--json]
```

`replay` is historical-only. It decodes recorded envelopes, receipts, and receipt outputs and reports zero executor invocations. It never calls the original capability provider, network executor, model provider, local process executor, or shell bridge.

## Failure model

Verification or import fails explicitly for:

- a missing object, invalid base64 payload, digest mismatch, or size mismatch;
- a non-SHA-256 or unresolved transitive reference;
- conflicting sizes for the same digest;
- a changed bundle manifest or original event envelope;
- a non-contiguous journal range or an envelope whose session/sequence differs from its range;
- a composition lock, world head, protocol version, or required profile mismatch;
- an unresolved policy or receipt reference;
- import into a non-empty destination session scope.

Unknown artifact semantics are not an error. Their bytes and descriptors remain in the verified inventory.

## Executable conformance

`plurora.runtime.world-bundle` is registered as the first production implementation claim because all five protocol-owned vectors execute successfully:

- `world_bundle.reference_closure`;
- `world_bundle.cross_host_import`;
- `world_bundle.offline_replay`;
- `world_bundle.reexecution_branch`;
- `world_bundle.shell_independence`.

The current integration fixture uses the `plurora/playable-creation-board` Package. The suite creates state, a branch, and controlled capability receipts on Host A; imports into an independent SQLite journal plus filesystem CAS on Host B; replays with no original Package; uses an alternative echo-backed implementation on a new branch; and reads the same archive through the headless CLI. The fixture checks compatibility behavior of this Profile without making that Package or the World ontology the center of the platform.

## Current limits

- The v1 archive encoding is JSON with base64 objects. Compression, chunked transfer, signatures, and authenticated envelopes are future profiles, not implied by this schema.
- The headless CLI rejects archive files larger than 1 GiB and imports only into a nonexistent or completely empty data directory guarded by an exclusive lock. Runtime verification accepts at most 100,000 objects and 4 GiB of decoded object data.
- A World Bundle is not a process-memory snapshot. Packages, subprocess handles, sockets, temporary URLs, and live streams are intentionally absent.
- Import rehydrates the substrate projections supported by the current event model. Historical audit remains available even when a deleted component's live projection code is unavailable.
- Bundle inclusion is an explicit authority/policy decision. Portable history may contain user-authored content; the bundle does not claim that arbitrary asset bodies are non-sensitive.
