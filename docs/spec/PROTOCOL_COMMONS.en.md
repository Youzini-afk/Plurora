# Protocol Commons Registry

> [English](./PROTOCOL_COMMONS.en.md) · [中文](./PROTOCOL_COMMONS.md)

Status: Experimental, descriptor schema version 1.

The Protocol Commons is the registry for shared semantics. A JSON shape alone is not a protocol: every registered protocol also names its lifecycle, error and cancellation model, authority boundary, behavioral vectors, compatibility profiles, migrations, and implementations. Registry entries do not receive routing priority, and a first-party provider is evaluated by the same vector set as any third-party provider.

## Descriptor

[`protocol-descriptor.schema.json`](v1/schemas/protocol-descriptor.schema.json) publishes `ProtocolDescriptor` (`urn:plurora:protocol-descriptor:v1`). Its stable fields are:

- `protocol_id`, `version`, and `maturity`;
- JSON Schema and WIT-world references;
- semantic, lifecycle, and error-model document references;
- explicit authority requirements;
- protocol-owned conformance vector identifiers;
- compatibility profiles;
- migrations and adapters;
- implementation claims and the exact vector set used by each claim.

Document references may omit a digest while the referenced document is repository-local. A portable package or World Bundle must materialize such references as content-addressed artifacts before making a cross-host integrity claim.

`host.info` exposes `protocol_commons_registry_version` and the full descriptor registry. The current registry version is `0.2.0` and intentionally includes only:

| Protocol | Version | Profile | Status |
| --- | --- | --- | --- |
| `plurora.change` | `1.0.0` | `plurora.change/default/v1` | Experimental |
| `plurora.shell.default` | `1.0.0` | `plurora.shell.default/v1` | Experimental |
| `plurora.work` | `1.0.0` | `plurora.work/experimental/v1` | Experimental |
| `plurora.assembly` | `1.0.0` | `plurora.assembly/experimental/v1` | Experimental |
| `plurora.world.bundle` | `1.0.0` | `plurora.world.bundle/experimental/v1` | Experimental |

Projection stays an Experimental canonical namespace, but is not admitted as an initial Protocol Commons descriptor. It needs two materially different experiences before shared semantics can be claimed.

## Negotiation

`ContractSelection.protocols[]` selects a protocol ID, version, and optional compatibility profile. Negotiation occurs before method resolution or handler execution.

- An exact supported version/profile produces a `NegotiatedProtocol` record.
- A declared legacy protocol/version uses the named adapter and reports that adapter in the negotiation result.
- An unsupported major produces `runtime/error/unsupported_protocol` with `reason=protocol_major_mismatch`, supported/requested majors, and the available adapters.
- Unknown protocols and profiles fail explicitly. They never fall back to shape compatibility or a weaker profile.

The initial explicit adapter is `platform.proposal@1.0.0 → plurora.change@1.0.0` through `change.proposal.v1`.

## Conformance ownership

Protocol conformance and implementation/package conformance are different reports:

- `ProtocolConformanceReport` identifies the protocol, version, profile, and protocol-owned vector results.
- `ImplementationConformanceReport` additionally identifies the implementation and provider while retaining the same vector identifiers.
- `PackageConformanceReport` continues to assess the distribution envelope, declarations, handshake, permissions, streaming, and handle lifecycle.

The registry rejects implementation claims that omit a required vector, invent a vector outside the protocol descriptor, name an unknown profile, or claim a different protocol version. The Change descriptor includes a Plurora runtime implementation and a test-only third-party reference claim; both are bound to the same four required vector IDs. `test_only` prevents that fixture from being presented as a portable production implementation.

The reports are executable independently:

```text
plurora conformance protocol --protocol plurora.change --json
plurora conformance protocol --protocol plurora.change --implementation plurora.runtime.change-proposal --json
plurora conformance package --path <package>
```

## Change protocol

The Change protocol references the additive Intent, ChangeSet, PolicyDecision, Commit, and EffectReceipt schemas. Its lifecycle, errors, authority rules, Proposal adapter, and behavioral evidence are defined in [`CHANGE_WORKFLOW.md`](CHANGE_WORKFLOW.en.md).

## Shell Default profile

`plurora.shell.default/v1` owns the vocabulary that maps structured contributions and sandboxed surface bundles into a shell. Existing fixed `SurfaceSlot` values are legacy vocabulary accepted through `shell.surface-slot.v1`; they are not substrate ontology.

The profile requires:

- public discovery through `shell.contribution.*`;
- bounded structured metadata with an explicit owner, distributed through the current Package Manifest;
- an explicit surface bridge allowlist and session scope;
- no implicit kernel, filesystem, network, or host-UI authority;
- shell replacement without changing journal history, object identity, or receipts.

The current lifecycle and bridge error model are documented in [`SURFACE_HOSTING.md`](../guides/SURFACE_HOSTING.en.md).

## Work Experimental profile

`plurora.work/experimental/v1` defines immutable, content-addressed `WorkRevision` objects and separates durable logical `WorkId` from an exact revision digest. A Work references its Assembly, content roots, entrypoints, Rights, Transparency, and optional OperationalIntent. It stores no absolute host path, raw secret, actual port, process ID, or current time. Unknown annotations survive lossless round trips while remaining subject to raw-secret and host-path redlines.

Its three required vectors cover canonical-digest stability, portable-identity redlines, and unknown-annotation preservation. `plurora.work.model` is an ordinary implementation claim by the `plurora-work` crate; it receives no execution authority, routing priority, or first-party privilege.

## Assembly Experimental profile

`plurora.assembly/experimental/v1` defines an acyclic recursive graph of Components and nested Assemblies, plus typed Ports, Bindings, exposed Ports, and State Slots. A Port constrains protocol/interface/version/Profile, an open interaction ID, effect class, binding phase, cardinality, and transport requirements. Unknown interactions remain losslessly preservable but cannot bind or execute without an implementation or explicit Adapter.

Its three required vectors cover recursive containment closure, Port-contract compatibility, and State Slot migration boundaries. `AssemblyLock` pins artifacts, behavior digests, trust classes, providers, transports, Profiles, and content roots. It belongs to the Experimental Protocol Commons, not the Constitutional Substrate.

## Work artifact lifecycle

The pure Work-model flow is `construct → validate → canonical JSON → SHA-256 descriptor → explicit persistence/transfer`. Validation and canonicalization have no external effect and grant no `object.write`; persistence requires authority the caller already holds. A WorkRevision is never modified in place—change creates a new content identity.

## Assembly artifact lifecycle

The pure Assembly-model flow is `construct graph → validate local IDs/references → validate an acyclic recursive closure → validate Ports/State → canonicalize → persist`. A later runtime may flatten execution, but it cannot discard nested identity, node paths, exposure mappings, or provenance. Phase 1 model validation does not activate Components or select installation-, launch-, or runtime-phase providers.

## Work and Assembly error model

Invalid IDs, missing or digest-mismatched Artifacts, containment cycles, unresolved or incompatible Ports, unsupported interactions, portable state without a schema, raw secrets, host-local paths, and implementation-budget overruns fail structurally. Errors return a stable reason and redacted explanation without echoing a secret, absolute path, or raw exception. Unknown Artifacts and annotations remain preservable and copyable; unknown semantics block only interpretation, binding, and execution.

## World Bundle Experimental profile

`plurora.world.bundle/experimental/v1` defines portability conditions without adding `World` to the substrate. Its descriptor references event envelopes, artifact descriptors, effect receipts, and the concrete [`WORLD_BUNDLE.md`](WORLD_BUNDLE.en.md) archive/head/journal schemas.

The required vectors cover reference closure, cross-Host import, offline replay, re-execution on a new branch, and shell independence. The current `plurora/playable-creation-board` integration fixture covers those vectors, so `plurora.runtime.world-bundle` registers the first production implementation claim. That claim describes current implementation coverage; it does not make World Bundle the platform's only content profile.

## World Bundle lifecycle

The required lifecycle is `select head → compute closure → verify → export → import into an empty scope → audit/replay → optionally re-execute on a new branch`. Import never treats host paths, process IDs, URLs, or package-local runtime handles as portable identity.

## World Bundle error model

Bundle processing fails explicitly for a missing object, digest or size mismatch, incomplete transitive reference closure, incompatible protocol major, unsupported required profile, altered original envelope, unresolved policy reference, or an attempted historical replay that would execute an external effect. Unknown artifact types are preserved and copied rather than discarded.
