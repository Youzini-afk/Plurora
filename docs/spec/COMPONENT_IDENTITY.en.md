# Package Envelopes and Component Identity

Status: Experimental, descriptor schema version 1.

Plurora separates how software is retrieved from what behavior it claims to implement. A package is an installation envelope. A component is an independently named implementation inside that envelope. Protocols, content roots, surfaces, and auxiliary artifacts retain their own descriptors and SHA-256 identities.

## Identity layers

| Layer | Owns | Does not imply |
|---|---|---|
| Package envelope | retrieval source, manifest, install transaction, integrity evidence | stable implementation identity |
| Component | executable or static implementation, behavior claim, execution boundary | content ownership or package origin |
| Protocol implementation | protocol/version/profile/vector claim | routing priority or publisher preference |
| Content root | immutable world/project/media data | a particular executable implementation |
| Surface | shell contribution and activation metadata | ownership of the underlying world state |

`PackageManifest.id` therefore remains the package-management key. `EntryDescriptor.component.id` is the component key. They may differ, and the same explicit component declaration may be carried by more than one package envelope.

The current manifest carries one entry component. Descriptor and lock structures use vectors so that a later multi-component envelope can remain additive.

## Descriptors and digests

The following descriptors are canonical JSON hashed as `sha256:<64 lowercase hex>`:

- `ComponentDescriptor` — component artifact, behavior artifact, trust class, boundary claims, capabilities, protocol implementations, content roots, and surfaces;
- `PackageEnvelopeDescriptor` — package manifest, component descriptors, packaged protocols, content roots, surfaces, and auxiliary artifacts;
- `AssemblyLock` — pinned AssemblyRevision, node artifacts, concrete binding providers, protocol profiles, and content roots.

Object ordering is canonicalized, and unordered declaration sets are sorted and deduplicated before hashing. Package identity participates in the envelope digest, but not in an explicit component's behavior digest. Consequently, two packages may differ while preserving the same component ID and behavior claim.

The install tree hash remains the retrieval/integrity proof for bytes copied into the immutable store. It is not reused as the logical package-envelope or component identity.

## Explicit and legacy identities

An explicit component declaration supplies:

- a namespaced component ID and semantic version;
- the package's provided capability set, or an empty list meaning “infer the complete set”;
- protocol implementations with versions, profiles, and conformance-vector IDs;
- content-root artifact descriptors;
- the package's surface IDs, or an empty list meaning “infer the complete set.”

Capability and surface declarations must match what the package actually provides. A shell descriptor may use either the package namespace or the explicit component namespace.

A v1 manifest without `entry.component` receives the synthesized ID:

```text
<package-id>/component/default
```

This `legacy_adapted` identity preserves package-contract composability, but it does not claim cross-package component portability.

## Trust classes and boundary claims

Trust names are descriptive records, not marketing labels. A boundary is reported as enforced only when the current host actually enforces it.

| Trust class | Current guaranteed claim |
|---|---|
| `trusted_native` | code executes inside the trusted host process; no isolation claim |
| `isolated_process` | process-failure isolation only; no OS network or filesystem isolation claim |
| `sandboxed_component` | component boundary is selected; resource/network/filesystem enforcement is not claimed until the host implements it |
| `remote_boundary` | no identity, tenancy, network, or revocation enforcement is claimed until the host verifies it |
| `static_resource` | no code execution |
| `foreign_capsule` | host startup only; no conforming, composable, portable, or isolation claim |

The legacy `TrustLevel` field remains available on package records for API compatibility. `ComponentTrustClass` and `ComponentBoundaryClaims` are the canonical Contract v2 fields.

## Foreign Capsule

`contract:none` always maps to `foreign_capsule`.

- Rust in-process and subprocess capsules may still be hosted under the existing Path B rules.
- Declared capabilities/hooks are not registered, no v1 bindings are minted, and a package principal is denied capability, event, network, and secret-ref authority.
- Protocol conformance claims are rejected during manifest validation and stripped defensively during descriptor construction.
- Package conformance reports a warning stating that composability and portability are not guaranteed.
- No network, filesystem, tenancy, revocation, or sandbox guarantee is inferred from the entry kind.

This is an explicit containment category, not a lower conformance tier.

## Runtime evidence

Package records and lifecycle events expose the package-envelope digest and component descriptors. Capability discovery, invocation results, and successful effect receipts carry:

- provider package ID;
- provider component ID;
- component artifact digest;
- behavior digest;
- component trust class.

Conforming in-process packages receive the component ID and digest in `ComponentEnv`. Subprocess handshakes receive the package-envelope digest and component descriptors; a Foreign Capsule handshake receives empty v1 capability/permission/binding sets. These fields allow audit and replay code to identify the implementation independently from the installer envelope.

The Component artifact payload also records the typed entry and contract mode. Validation re-derives `entry_kind`, trust class, and boundary claims from those facts, and strictly checks protocol/Surface reference types, secret references, and portable paths; self-reported payload fields cannot elevate trust.

## Assembly lock

`AssemblyLock` keeps separate pins rooted at an AssemblyRevision:

```text
nodes              node ID + component/child AssemblyLock artifact + behavior digest + trust class
bindings           provider/consumer Ports + concrete provider component + transport + phase
protocol_profiles  protocol ID + version + selected profile
content_roots      complete ArtifactDescriptor values
```

An Adapter is not a special side channel on BindingLock. It is an ordinary Component node, with provider→adapter-import and adapter-export→consumer represented as two ordinary Bindings, so both transports, multiplicities, behavior, provenance, and content roots enter the same lock DAG. A Component node's complete Port contracts are canonical AssemblyRevision content, so compatibility-semantic changes alter the Assembly, Work, and Lock digests instead of only changing transient resolver input. Replacing a node pin does not mutate content roots. Nested Assemblies form an auditable lock DAG through child `AssemblyLock` artifacts: execution may flatten while lock identity retains encapsulation. Package installation lock entries continue to persist package-envelope, component, profile, and content pins. `check_lockfile` re-derives them from the installed manifest and reports drift independently from manifest, tree, and static-surface hashes.

## Conformance meaning

Package conformance includes the full package envelope and one record per component:

- `declared`: explicit identity and structural composability are independently pinned; portability still requires a separate passing implementation-conformance report;
- `legacy_adapted`: package-contract composability is retained; cross-package portability is not guaranteed;
- `foreign_capsule`: startup may succeed; composability and portability are not guaranteed.

Protocol and implementation conformance remain separate reports. Packaging an implementation never grants protocol priority or waives protocol-owned behavior vectors.

## Schemas

- `component-descriptor.schema.json`
- `package-envelope-descriptor.schema.json`
- `assembly-lock.schema.json`

All are additive Experimental schemas under `docs/spec/v1/schemas/`.
