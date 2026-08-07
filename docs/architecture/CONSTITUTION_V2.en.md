# Plurora Constitution v2 (Candidate)

> [English](./CONSTITUTION_V2.en.md) · [中文](./CONSTITUTION_V2.md)

> Status: candidate architecture. The current [`CHARTER.md`](../CHARTER.en.md) and
> [`PUBLIC_CONTRACT.md`](../spec/PUBLIC_CONTRACT.en.md) remain the repository's
> operative contract. This document supersedes those boundaries only after explicit
> adoption; no implementation should claim v2 conformance before then.

## Core promise

Plurora hosts portable, composable, auditable, and long-lived applications, tools, services, worlds, games, agents, and other digital forms. Models, agents, components, engines, clients, and Hosts may be replaced; important content, history, and ownership do not belong to any single implementation or service.

The product may use “world” as its medium identity, but the constitutional substrate does not freeze `World` into system ontology. The substrate knows identity, authority, objects, references, causality, effects, and proof. Higher-level protocols define worlds.

The boundary in one sentence:

> The substrate owns physical laws, Protocol Commons owns shared language, and distributions and products own viewpoints; data and content belong to users and creators.

## Why a new boundary is needed

The current design is strong at extensibility: packages can declare capabilities, events, permissions, surfaces, and multiple entry forms. Long-term durability also requires four additional properties:

- **Evolvability:** wrong abstractions can be deprecated, migrated, and replaced.
- **Portability:** content and history can leave when a host or component disappears.
- **Composability:** independent authors can interoperate through shared protocols, not merely matching JSON shapes.
- **Governance:** protocols have maturity, negotiation, support windows, migration tools, and behavioral conformance.

“The kernel does not know content concepts” does not mean the kernel has no ontology. `session`, `package`, `project`, `proposal`, `surface`, `target`, and `proxy` are ontological choices too. Every noun admitted to a long-lived stable contract must satisfy the strict downward-movement gate that it cannot safely live above the substrate.

## Immutable principles

### 1. Authority is explicit

A manifest declaration is an upper bound request, not runtime authority. Actual authority is expressed through unforgeable, attenuable, leased, and revocable capabilities.

Every cross-boundary action must answer: who, over which resource, under which conditions, with what authority, derived from where, and until when.

### 2. First-party implementations have no privilege

First-party components, protocol implementations, and shell profiles use the same registration, authorization, invocation, audit, and conformance mechanisms as third parties. First-party identity may express maintenance responsibility; it cannot imply authority or routing priority.

### 3. The public contract is above internal calls

In-host calls, HTTP, stdio, WASM imports, remote calls, and future transports must expose equivalent authorization and behavioral semantics. Internal implementations cannot use private bypasses to obtain ecosystem capabilities.

### 4. Important objects have content identity

Portable objects obtain stable identity from content digests rather than host paths or process-local IDs. A reference carries at least type, digest, and size; consumers must be able to verify that content was not substituted.

### 5. Nondeterminism and external effects leave receipts

Model calls, tool calls, network requests, process execution, and other irreversible or nondeterministic actions produce auditable effect receipts. Receipts record references and decisions without copying raw secrets or unnecessary user content.

### 6. Historical replay and re-execution are separate

- **Historical replay** uses recorded outputs and receipts without triggering external effects again.
- **Re-execution** uses current components, models, and policy and must create a new causal branch.

The platform must never silently mix these modes.

### 7. Shared meaning belongs to the protocol commons

Schemas describe shape. Protocols also define meaning, lifecycle, errors, security requirements, and behavior. Shared languages for Agent, World, Memory, Surface, Evaluation, and similar domains belong to evolvable protocols—not to the constitutional substrate and not to private package conventions.

### 8. Package is a distribution envelope, not an ontology unit

A package may carry components, protocol descriptions, content, surfaces, or adapters, but those artifacts have independent identity, versions, digests, dependencies, and migrations. Replacing an execution component must not force all content to migrate; exporting content must not require carrying host UI and executable code.

### 9. Invocation may be uniform; trust must be explicitly unequal

WASM, processes, remote services, and native in-process implementations may implement the same protocol, but they cannot claim identical isolation, failure, latency, or supply-chain guarantees. Every implementation exposes its trust class and the boundaries actually enforced.

### 10. Wrong abstractions can leave the system

Stability is not permanent additive accumulation. Every stable protocol has deprecation, support-window, migration, and versioned-reader mechanisms. A compatibility reader may continue reading old data without granting the old abstraction new behavioral features forever.

## Layering model

```text
┌──────────────────────────────────────────────┐
│ Experiences / Worlds / Products              │
│ Opinionated, forkable, user-owned             │
├──────────────────────────────────────────────┤
│ Shell Profiles                               │
│ Web / Desktop / VR / IDE / headless mapping   │
├──────────────────────────────────────────────┤
│ Protocol Commons                             │
│ Shared semantics, profiles, migration, tests  │
├──────────────────────────────────────────────┤
│ Components & Adapters                        │
│ WASM / process / remote / trusted native      │
├──────────────────────────────────────────────┤
│ Constitutional Substrate                     │
│ Identity, authority, objects, effects, proof  │
└──────────────────────────────────────────────┘

Registry / Governance / Provenance span every layer.
The Host Control Plane is orthogonal and manages local installation, processes, ports, proxies, secrets, and deployment.
```

## Constitutional Substrate

The substrate owns only mechanisms that cannot be safely reimplemented in user space.

### The substrate owns

- principals and authenticated call context;
- capability mint, attenuation, delegation, leases, refresh, and revocation;
- content-addressed objects and verifiable references;
- append-only journals, stable ordering, explicit causal references, and heads;
- effect-receipt and provenance attachment points;
- compare-and-swap, preconditions, atomic commit, and idempotency keys;
- invoke, stream, cancel, deadline, and backpressure;
- minimal component-instance lifecycle and health;
- protocol/version/profile negotiation;
- audit, query, and conformance entry points.

### The substrate does not own

- Agent, Prompt, Message, Turn, or Memory;
- World, Entity, Scene, Quest, rules, economy, or simulation time;
- Home, Play, Forge, Assistant, or Editor;
- project shelves, workspaces, Docker, targets, exec, ports, or proxies;
- providers, model catalogs, or billing policy;
- package registries, marketplaces, or a specific secret-store product.

A platform distribution may use these concepts, but they live in protocols, shell profiles, the host control plane, or experiences.

## Artifacts and content addressing

The substrate does not enumerate every possible artifact kind. It provides a verifiable generic descriptor; protocol profiles define common kinds.

Non-normative minimum shape:

```text
ArtifactDescriptor
├── artifact_type_uri
├── media_type
├── digest
├── size_bytes
├── references[]
└── annotations
```

Rules:

- `digest` is identity, not decorative metadata; content is re-verified after retrieval.
- Unknown `artifact_type_uri` values can still be copied, stored, and exported. Unknown semantics do not make data invalid.
- Host absolute paths, process IDs, and temporary URLs cannot serve as portable identity.
- Packages, components, protocol descriptors, content, compositions, receipts, and world bundles may share the same reference mechanism without becoming a closed substrate enum.

This shape borrows the `mediaType + digest + size` idea from OCI Content Descriptors. Plurora does not require OCI manifests and does not import container semantics into the substrate.

## Journals, causality, and heads

Ordered journals and causal graphs solve different problems, so both remain:

- journal sequence provides cheap, deterministic, pageable operational order within one scope;
- causal references express dependencies across scopes, branches, and effects;
- a head is a set of content references to state, history, composition, policy, and provenance;
- merge belongs to the protocol that owns domain semantics; the substrate does not pretend all states can be merged generically.

`WorldHead` is a head profile defined by a World protocol, not a substrate type. Other protocols may define document heads, workspace heads, or simulation heads without changing the substrate.

Large objects, media, snapshots, and model outputs live in the content-addressed object store. Journals and receipts retain references and only the audit summary that is required.

## Effect Receipt

A receipt is evidence of an effect that happened, not a plan or a log message.

Non-normative minimum shape:

```text
EffectReceipt
├── receipt_type_uri
├── principal
├── component_ref
├── protocol_profiles[]
├── input_refs[]
├── output_refs[]
├── external_effect_refs[]
├── policy_decision_ref
├── approval_ref
├── cost / latency / status
├── trace_id
├── parent_receipts[]
└── replay_mode
```

Receipts distinguish planned values from actual values and distinguish denied, cancelled, timed out, partially completed, and successful outcomes. Raw secrets, full credentials, user content by default, and unnecessary complete prompts never enter receipts.

Receipt envelope, statement, and predicate layers may evolve independently. This resembles in-toto's separation of subject, predicate type, statement, and authenticated envelope, but Plurora receipts describe runtime effects rather than only software supply chains.

## Protocol Commons

A protocol is a shared semantic and behavioral contract, not the API documentation of one implementation package.

Every protocol contains at least:

- a stable namespaced protocol ID and major version;
- schemas, a WIT world, or an equivalent type definition;
- field meaning, units, coordinate systems, clocks, and consistency assumptions;
- lifecycle and state machines;
- error and cancellation semantics;
- required authority, effects, and privacy boundaries;
- test vectors and behavioral conformance;
- compatibility profiles;
- migration, adapter, and deprecation instructions;
- a list of independent implementations that pass conformance.

Protocols may compete and fork. A first-party-maintained protocol has no substrate routing priority. External protocols such as MCP, A2A, or engine protocols may join the commons through adapters; Plurora does not need to reinvent proprietary equivalents.

## Components and execution trust

Recommended trust classes:

| Trust class | Guarantee |
|---|---|
| `sandboxed_component` | Portable, explicit imports, resource-bounded; WASM Component is the preferred candidate |
| `isolated_process` | Process failure isolation; OS network/filesystem enforcement must be proven separately by the host |
| `remote_boundary` | Remote identity, network failure, tenancy, and service policy are explicit |
| `trusted_native` | Host-level trust and performance escape hatch; not for untrusted dynamic code |
| `static_resource` | No code execution; content or a surface bundle only |
| `foreign_capsule` | May be hosted, but makes no protocol-conformance, composability, or portability claim |

Invocation protocols may be shared, but conformance reports separately declare type compatibility, authority enforcement, isolation, resource limits, replayability, and supply-chain evidence.

`contract: none` maps to `foreign_capsule`. It can exist in a product, but it cannot be described as having the same ecosystem guarantees as a conforming component.

## Change workflow

The substrate does not own the product concept of an “assistant proposal.” The generic change chain is:

```text
Intent
→ Plan / ChangeSet
→ PolicyDecision
→ Commit
→ EffectReceipt
```

- Intent states a goal and grants no authority.
- ChangeSet describes expected reads, writes, preconditions, and effects.
- PolicyDecision allows, denies, budgets, requests approval, or requires a branch.
- Commit atomically checks preconditions and produces a new head.
- EffectReceipt records the effects that actually completed.

The current Proposal can become one profile of this protocol. Its UI may continue to say “proposal,” but it is no longer substrate ontology.

## Shell Profile

The substrate protects resources, call bridges, and authority. It does not hard-code Home, Play, Forge, or Assistant.

A shell profile defines how a host interprets interaction resources such as views, actions, editors, presence, streams, layout hints, and input intents. Existing slots such as `experience_entry`, `home_card`, and `forge_panel` migrate into the vocabulary of the `plurora.shell.default/v1` profile.

A shell may be Web, desktop, VR, IDE, or headless. Replacing a shell must not change world history, object identity, protocol state, or effect receipts.

## Experience and World

An Experience is a declarative composition. A World is a persistent higher-level entity after execution. Both belong to the protocol commons and user data, not the substrate.

A World profile may reference:

```text
WorldHead
├── state_root
├── history_root
├── composition_lock
├── protocol_profiles
├── policy_root
└── provenance_root
```

A World Bundle is readable and auditable without executing its original components. A component upgrade produces a new composition lock; re-executing a nondeterministic step creates a new branch; deleting a component cannot make history unreadable.

## Protocol maturity

```text
Experimental
→ Candidate
→ Stable
→ Deprecated
→ Versioned Reader
```

- **Experimental:** may break quickly and has no long-term compatibility promise.
- **Candidate:** semantics, errors, test vectors, and a migration draft exist; at least two distinct consumers use it.
- **Stable:** passes the anti-rigidity rule, behavioral conformance, and independent-implementation requirements.
- **Deprecated:** remains inside a support window with an explicit replacement and migration path.
- **Versioned Reader:** only reads or transforms a prior contract/data boundary and receives no new behavioral features.

Version negotiation is explicit. A client must not silently lose a required capability when falling back.

## Anti-rigidity rule

A new concept enters the Stable substrate only when all conditions hold:

1. at least three dissimilar protocols or experiences need it;
2. it cannot be implemented reliably in the Protocol Commons, Host Control Plane, or a Shell Profile;
3. it is independent of the current UI, model, genre, game type, and deployment mechanism;
4. at least two independent implementations pass behavioral conformance;
5. version negotiation, deprecation, and migration paths are explicit.

Concepts that do not satisfy these conditions remain in Experimental/Candidate protocols or host layers. High usage, first-party maintenance, or implementation convenience are not sufficient reasons for substrate admission.

## Relationship to the current Contract V1

This document does not require deleting existing implementation. Current methods may continue serving the existing Web shell, CLI, packages, and conformance, while their long-term ownership is reclassified by
[`CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.en.md).

Until the candidate layering is explicitly adopted as a stable constitution:

- the exact owner-based Contract V1 remains the operational public boundary, not automatically the permanent constitution;
- compatible v1 evolution follows the additive rule, while breaking behavior requires a new negotiated contract/profile/version boundary;
- new mechanisms enter an Experimental namespace with an explicit owner;
- prior data remains readable through explicit versioned readers and migration tooling rather than hidden dispatch aliases;
- a future v2 preserves original v1 envelopes and unknown fields for lossless transfer.

The codebase already contains owner-based identities, an exact Contract Registry, ObjectStore, EffectReceipt, Change primitives, Protocol Commons, and World Bundles. They provide an operational foundation for the candidate layering, but they do not promote this document to Stable; [`CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.en.md) remains the ownership review reference.

## Conditions for stable adoption

Before the candidate substrate becomes Stable, it must satisfy these compatibility, portability, and independent-implementation conditions:

1. the same World Bundle imports into a second independent host;
2. replacing a model provider requires no world-data migration;
3. replacing WASM, process, remote, or native implementations preserves protocol behavior;
4. two independent authors compose components using only protocol and conformance;
5. an agent-generated component can be bounded, evaluated, revoked, and promoted;
6. history remains readable, verifiable, and auditable after a component is deleted;
7. old worlds run through an adapter after a protocol major upgrade;
8. history replays deterministically while the model service is offline;
9. replacing the Web shell with desktop, VR, or headless does not change world data.

These conditions constrain stability claims and compatibility commitments; they do not decide which product functionality Plurora should build merely to satisfy them.

## Non-normative references

- [OCI Content Descriptors](https://github.com/opencontainers/image-spec/blob/main/descriptor.md) — content type, digest, size, and verifiable references.
- [in-toto Attestation Framework](https://github.com/in-toto/attestation/tree/main/spec) — subject, predicate, statement, and authenticated envelope layering.
- [OpenTelemetry Semantic Conventions](https://opentelemetry.io/docs/specs/semconv/) — shared meaning and maturity beyond types.
- [WebAssembly Component Model](https://component-model.bytecodealliance.org/) — composable components and WIT contracts.
- [A2A Protocol](https://a2a-protocol.org/latest/specification/) and [MCP](https://modelcontextprotocol.io/specification/) — external protocols, version negotiation, and adapter boundaries.
