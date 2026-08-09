# Contract Layering Matrix (Candidate)

> [English](./CONTRACT_LAYERING_MATRIX.en.md) · [中文](./CONTRACT_LAYERING_MATRIX.md)

> Status: candidate ownership classification. The operative wire contract is
> [`PUBLIC_CONTRACT.md`](PUBLIC_CONTRACT.en.md); constitutional principles are in
> [`CONSTITUTION_V2.md`](../architecture/CONSTITUTION_V2.en.md).

## Purpose

The public contract spans constitutional substrate, Host control, shared Protocols, and Shell Profiles. This matrix makes ownership explicit without claiming that every current implementation file already matches the ideal boundary.

It answers:

1. which layer owns each public identity now;
2. which responsibilities remain mixed;
3. where future changes require a new explicit version boundary;
4. which Product choices must remain outside the substrate.

## Layer codes

| Code | Layer | Responsibility |
|---|---|---|
| `S` | Constitutional Substrate | Identity, authority, journal, objects, invocation, streams, receipts, causal lineage |
| `H` | Host Control Plane | Installation, processes, targets, ports, proxies, secrets, deployment, local diagnostics |
| `C` | Protocol Commons | Shared semantic contracts, change workflow, projections, extension contracts |
| `P` | Shell / Product Profile | Surface contributions, layout, interaction mapping, Product defaults |
| `X` | Mixed boundary | Current behavior combines responsibilities from more than one layer |

`plurora/*` is a first-party Package publisher namespace, not a layer and not a privilege class.

## Current factual baseline

- 80 exact public method IDs and 80 method schemas.
- 59 explicit platform-owned event kinds and 59 payload schemas.
- 36 top-level schemas; 175 schemas total.
- Contract Registry `0.1.0` exposes one wire ID per method and no aliases.
- Method IDs declare owner through their first dot segment.
- Platform event ownership is explicit; all 59 kinds require writer `plurora/runtime`.
- Package event and capability IDs remain under the exact Package ID slash namespace.
- Explicit contract and Protocol Commons negotiation occurs before dispatch and fails closed.

## Method owner prefixes

| Prefix | Count | Current owner | Boundary note |
|---|---:|---:|---|
| `context.*` | 6 | `S` | Content-free execution/journal scopes and lineage |
| `journal.*` | 3 | `S` | Append, replay, and subscription boundary |
| `capability.*` | 5 | `S` | Discovery, invocation, streaming, cancellation |
| `authority.*` | 7 | `S` | Handles, grants, revocation, decisions |
| `object.*` | 3 | `S` / `H` | Put/get are substrate-shaped; global listing remains Host-shaped |
| `identity.*` | 1 | `S` | Authenticated principal/context discovery |
| `host.*` | 40 | `H` / `X` | Host-local operations; effects still depend on substrate authority and receipts |
| `protocol.*` | 3 | `C` | Extension-contract discovery and subscriptions |
| `change.*` | 6 | `C` | Approval-gated Change protocol facade |
| `projection.*` | 4 | `C` | Derived-view protocol operations |
| `shell.*` | 2 | `P` | Replaceable Shell Profile contribution registry |

## Host method breakdown

| Host area | Count | Classification |
|---|---:|---|
| package lifecycle and audit | 8 | `X`: Host artifact/process concerns plus runtime component evidence |
| project | 5 | `H`: current distribution installation-instance model |
| target / exec / port / proxy | 17 | `H`, with `S` authority and receipt evidence |
| outbound | 6 | `X`: Host network adapters plus `S` policy, secrets, streams, and receipts |
| surface bundle resolution | 1 | `X`: Host serving plus Shell Profile interpretation |
| info / ping / diagnostics | 3 | `H` |

A mixed classification does not create a private API. It identifies where implementation decomposition may continue while the current public contract remains exact and testable.

## Platform event ownership

| Group | Count | Owner |
|---|---:|---:|
| Context | 3 | `S` |
| Package and Project lifecycle | 13 | `H` / `X` |
| Capability and stream lifecycle | 10 | `S` |
| Authority | 3 | `S` |
| Object | 1 | `S` |
| Projection | 1 | `C` |
| Change proposal | 5 | `C` |
| Outbound / WebSocket | 8 | `H` / `X` |
| Exec / port / proxy / deployment | 14 | `H` |
| Runtime error | 1 | `S` |

The owner is semantic. Persistence still uses one `EventEnvelope` and one EventStore boundary.

## Top-level schema ownership

- **Substrate:** event envelope, protocol context/response, capability descriptor and invocation, permission set, artifact/effect evidence.
- **Host:** Package Manifest envelopes, Installation / Run / Exposure / Realization, local execution descriptors, Target Inventory, and Host-facing records.
- **Protocol Commons:** protocol descriptors, Change primitives, portable Work / Assembly / Port / State Slot contracts, AssemblyLock, World Bundle, and World Head.
- **Shell Profile:** contribution and profile descriptors carried through public schemas.

Some top-level schemas join multiple layers for transport. Their fields must still identify ownership rather than collapse the layers into one ontology.

## Change discipline

1. The current exact IDs are the operative public contract.
2. Moving a responsibility between layers does not justify an in-place identity rewrite after stability.
3. A breaking change requires a new contract/profile/version boundary, migration tooling, readable old data, and conformance vectors.
4. Compatibility must not accumulate as hidden dispatch aliases or first-party shortcuts.
5. Product and Shell convenience must not silently become substrate responsibility.

## Current decomposition priorities

- separate Package artifact/install state from active Component execution evidence;
- keep Host network/process adapters behind substrate authority and EffectReceipt contracts;
- complete Protocol-owned projection and extension semantics without adding content ontology to the substrate;
- keep Shell slots and Product organization replaceable through explicit profiles;
- preserve one public transport and authority path for first-party and third-party participants.
