# Packages, Components, and Capability Contracts

> [English](./CAPABILITY_PACKAGE.en.md) · [中文](./CAPABILITY_PACKAGE.md)

Contract V1 currently uses one Package Manifest to describe distribution, execution, capabilities, protocol contributions, surfaces, and permissions. That model remains operational, but the long-term architecture distinguishes separate ownership:

- **Package / Package Envelope:** retrieval, distribution, installation, and supply-chain envelope;
- **Component:** activatable and invokable implementation unit;
- **Protocol:** shared semantic and behavioral contract implemented by more than one component;
- **Content / Artifact:** user or product data, static resources, and immutable artifacts;
- **Adapter:** a component that connects an external system or legacy contract to public protocols.

A Package may carry several of these, but Package itself is not the ontology unit for every kind of platform meaning.

## Equality rule

Official Packages, third-party Packages, and different execution forms use the same:

- descriptor and manifest schemas;
- installation and integrity checks;
- capability and protocol registration;
- authority bindings;
- invocation, streams, cancellation, and effect receipts;
- diagnostics, migration, and conformance entry points.

There is no private API unlocked by Package ID and no implicit official implementation priority. Maintainers or signatures may affect source trust and policy, but cannot grant runtime authority automatically.

## Current V1 Manifest

The V1 Manifest is the current public compatibility format. Its main shape is:

```yaml
schema_version: 1
id: org/name
version: 0.1.0
display_name: Example
description: ...
license: AGPL-3.0-only

entry:
  kind: rust_inproc | subprocess | wasm | remote
  contract: v1 | none
  # kind-specific fields

provides:
  - id: org/name/capability
    version: 0.1.0
    input_schema: {}
    output_schema: {}
    streaming: false
    side_effects: []

consumes:
  - id: other-org/capability
    version: ^0.2

contributes:
  schemas: []
  hooks: []
  extension_points: []
  surfaces: []

permissions:
  network: { hosts: [] }
  filesystem: { paths: [] }
  events: { read: false, append: false }
  capabilities: { invoke: [] }

sandbox_policy:
  cpu_quota_ms_per_invoke: 5000
  memory_mb: 128
  wall_clock_ms: 30000
```

The authoritative fields are in [`../spec/v1/schemas/manifest.schema.json`](../spec/v1/schemas/manifest.schema.json). As Package Envelope, Component Descriptor, Protocol Descriptor, and Content Root separate over time, V1 Manifests remain readable through generation or legacy adapters.

## Package Envelope

A Package Envelope owns:

- source and retrieval coordinates;
- version, signature, license, and maintainer information;
- manifest, tree, and artifact digests;
- references to carried components, protocols, content, and surfaces;
- platform and architecture compatibility requirements;
- metadata required for installation, update, migration, and rollback.

Package installation is a Host Control Plane operation. Successful installation does not mean every Component has been activated or every declared permission has been granted.

## Component

A Component is the implementation unit that provides behavior. At minimum it has:

- independent identity and behavior or artifact digest;
- exports, imports, and adopted protocol profiles;
- trust class and enforced boundaries;
- resource limits;
- activation, health, and deactivation state;
- compatibility and migration claims.

A Package may contain several Components. Updating one Component should not automatically require migration of every piece of content in the same Package.

## Execution forms and trust

A common capability contract does not imply identical isolation guarantees:

| V1 entry / component form | Long-term trust class | Meaning |
|---|---|---|
| `rust_inproc` | `trusted_native` | high performance with Host-process trust; crashes and unenforced effects may affect the Host |
| `subprocess` | `isolated_process` | process failure isolation; OS filesystem/network enforcement depends on Host policy |
| `wasm` | `sandboxed_component` | intended for explicit imports, resource limits, and portability; complete execution support is still under construction |
| `remote` | `remote_boundary` | remote identity, network failure, tenancy, and service policy are explicit; general remote component execution is still under construction |
| static bundle/content | `static_resource` | no code execution; verifiable content or surfaces only |
| `contract: none` | `foreign_capsule` | the Host may supervise lifecycle without promising v1 bindings, composition, or protocol guarantees |

Different forms may implement the same protocol, but conformance and UI must disclose their actual guarantees rather than describing them as only packaging differences.

## Capability contract

A Capability is described by stable ID, version, input/output schemas, streaming, and effect requirements. Callers may select implementations using capability, protocol profile, and version constraints.

Routing considers:

1. whether current authority permits invocation;
2. whether the Component is active and healthy;
3. whether protocol, version, and profile are compatible;
4. whether the distribution or caller explicitly selected a provider;
5. whether multiple implementations remain ambiguous, in which case routing fails and requires a choice.

There is no implicit official priority.

Capability invocation produces structured terminal state. External effects or nondeterminism produce or reference an EffectReceipt. Large inputs and outputs should use ArtifactDescriptor rather than expanding wire envelopes indefinitely.

## Protocol contribution

A Protocol defines shared meaning; a Component implements a Protocol. A Package may carry a protocol descriptor, but maintaining that Package grants no kernel privilege.

A protocol contribution should describe at least:

- protocol ID and version;
- schema or equivalent type contract;
- field semantics and lifecycle;
- error, cancellation, and effect semantics;
- authority and privacy requirements;
- compatibility profiles and migration;
- behavioral checks and implementation claims.

Shared meaning such as extension points, projections, change workflows, agents, memory, worlds, and surfaces should move from Package-private convention into explicit Protocols rather than expanding one monolithic kernel namespace.

## Surface contribution

A V1 Package may contribute Surface descriptors. The Manifest supplies bundle, capability allowlist, activation, and permission requirements; the adopting Shell Profile interprets the slot.

Therefore:

- official and third-party Packages may both provide Surfaces;
- a Surface has no implicit kernel access;
- `experience_entry`, `forge_panel`, and `assistant_action` are current Profile enums rather than constitutional substrate types;
- third-party shells may define or negotiate another Surface Profile;
- static Surface bundles and executable Components may be versioned and addressed independently.

## Authority and declarations

Manifest permissions are the maximum scope requested by a Package, not an actual grant. The Host mints bindings according to user choice, principal, Project or target selectors, policy, and environment.

Execution must ensure:

- ungranted capability, event, network, filesystem, or secret operations are rejected;
- long-running work revalidates grants before important effects;
- delegation, leases, quotas, and revocation take effect;
- raw secrets do not enter Manifests, logs, receipts, or public state;
- declared-versus-used audit does not depend on Package-name privilege.

`entry.contract: "none"` does not turn Manifest declarations into platform authority. It is an explicit self-contained path with reduced interoperability guarantees, not a way to bypass Host security boundaries.

## Lifecycles

Two lifecycles must be kept distinct.

### Package lifecycle (Host)

```text
discovered → resolved → downloaded → verified → installed
           → update available → migrated / rolled back → removed
```

### Component lifecycle (runtime / substrate)

```text
inactive → activating → ready → degraded → stopping → inactive
                         └──────────────→ failed
```

Stopping a Component is not uninstalling a Package. Removing a Package must first address dependencies, running instances, user data, and rollback information.

## Content and user data

Content should not lose independent identity merely because it was distributed beside executable code. Important content should:

- use content digests and open ArtifactDescriptors;
- be copyable and exportable without knowing the original Package implementation;
- reference dependencies, schemas or protocol profiles, and migration explicitly;
- distinguish user-owned data, reconstructable cache, and executable artifacts;
- never be silently overwritten by a Component update.

## Distribution and updates

Package registries, marketplaces, and dependency-resolution services belong to the Host and distribution ecosystem rather than the constitutional substrate. Competing registries may coexist, and offline files and local source remain first-class sources.

Updates should pin:

- retrieval source and immutable commit or digest;
- Package Envelope;
- Component artifacts;
- Protocol profiles;
- Content roots;
- user-approved authority changes and migration plans.

Automatic update cannot bypass new authority, data migration, or trust boundaries.

## Versioning and compatibility

- Package version, Component version, Protocol version, and Manifest `schema_version` are different dimensions;
- breaking Component ABI change must not masquerade as content migration;
- Protocol major change needs compatibility, adapters, or migration;
- unknown fields and Artifacts should be preserved where practical;
- V1 Manifests coexist with future descriptor separation through compatibility generation and adapters.

## Design test

When adding something, identify what it is:

- a bundle of artifacts for retrieval and installation → Package Envelope;
- behavior that is activated and invoked → Component;
- meaning shared by multiple implementations → Protocol;
- portable data owned by users or products → Content / Artifact;
- operation on a real machine → Host;
- interaction viewpoint of a Shell → Product Profile.

Do not assume Package permanently owns a concept merely because the current V1 Manifest can contain another field.
