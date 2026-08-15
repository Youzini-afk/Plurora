# Constitutional Substrate and Current Runtime Boundary

> [English](./CONSTITUTIONAL_SUBSTRATE.en.md) · [中文](./CONSTITUTIONAL_SUBSTRATE.md)

This document separates two things:

1. the deliberately small **Constitutional Substrate** Plurora should retain for the long term;
2. the broader implementation boundary currently carried by `plurora-core` and `plurora-runtime`.

They do not yet coincide completely. To provide an operational platform, `plurora-core` and `plurora-runtime` also carry some Host, Protocol, and Shell responsibility. The long-term goal is to return responsibility to the correct layer while preserving explicit version boundaries, migration, and data readability.

## What it owns and does not own

The complete “owns / does not own” list lives in [`ARCHITECTURE.md`](ARCHITECTURE.en.md). This document only covers the gap between that list and the current implementation.

In one sentence: the substrate owns identity, authority, content-addressed objects, journal/causality, invoke/stream/cancel, CAS/idempotency, effect receipts, and protocol negotiation. It does not own Library, chat, agents, worlds, Docker, any official UI, or any first-party Package ID.

## Compatibility responsibilities carried by the current Contract V1 kernel

The current v1 public contract still includes:

- sessions and events;
- package lifecycle and capability routing;
- extension points and hooks;
- assets, projections, and proposals;
- Projects;
- Host info, targets, exec, ports, proxies, and outbound;
- surface contributions;
- permission and audit.

These methods remain supported public interfaces, but their long-term owners differ:

| Current concept | Long-term owner |
|---|---|
| principal, authority, object, journal, invoke, stream, receipt | Constitutional Substrate |
| package retrieval, Project, target, exec, port, proxy, secret, deployment | Host Control Plane |
| projection, change, shared domain state machines | Protocol Commons |
| Surface slots and Home / Forge / Assist mapping | Shell / Product Profile |
| chat, agent, memory, world, and similar meaning | Concrete protocols, components, or products |

See [`../spec/CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.en.md) for itemized classification.

## Boundaries the current implementation must continue to preserve

### One public contract

HTTP, stdio, in-process invocation, future WASM imports, and remote boundaries preserve the same identity, authority, error, and effect semantics. Internal callers receive no capability unavailable to third parties.

### No first-party privilege

First-party and third-party components use the same registration, bindings, invocation, and audit mechanisms. Package names cannot determine authority or routing priority.

### Content meaning stays out of core mechanisms

The kernel and Host do not interpret characters, messages, models, agents, worlds, documents, or game rules. They may preserve opaque data and verifiable references, while protocols and components interpret meaning.

### Handles and call context express authority

Bare permission strings, caller-supplied `session_id`, paths, or target IDs do not establish authority on their own. Actual grants bind authenticated principals, resource selectors, conditions, and lifecycle.

### The event journal is not the only storage for every byte

A journal preserves facts that need durable order, audit, or causality. Large objects, media, snapshots, and model outputs belong in object storage. Current runtime state and Host operations may use dedicated durable control-plane projections. “Events are truth” does not justify copying every byte into a journal.

### Historical replay does not retrigger effects

Historical reading uses recorded output and receipts. Calling a model, network, tool, or process again creates a new invocation and causal branch.

## Execution and trust

A common invocation contract does not imply identical trust guarantees. At minimum, distinguish:

| Trust class | Typical guarantee |
|---|---|
| `sandboxed_component` | explicit imports, resource limits, stronger portability |
| `isolated_process` | process failure isolation; OS file/network enforcement requires inspectable Host enforcement claims |
| `remote_boundary` | explicit remote identity, network failure, tenancy, and service policy |
| `trusted_native` | Host-level trust and performance escape hatch, unsuitable for untrusted dynamic code |
| `static_resource` | no execution; content or surface only |
| `foreign_capsule` | may be hosted without protocol, composition, or portability guarantees |

Rust in-process, subprocess, WASM, and remote implementations may serve the same protocols, but documentation must not reduce them to “only a packaging difference.”

## Transport

The current implementation supports:

- in-process Rust calls;
- HTTP `/rpc`;
- SSE event subscriptions;
- Host stdio JSON-RPC;
- Host-owned HTTP / WebSocket outbound and reverse proxying.

Future transports may be added without changing identity, authority, or terminal semantics of the upper contract.

## Gate for a new substrate mechanism

Before adding substrate responsibility, explain:

1. why an ordinary protocol, component, or Host cannot implement it safely;
2. whether it is independent of a specific product, UI, workflow, or content ontology;
3. whether it increases user freedom and replaceability rather than locking in the current first-party implementation;
4. its error, cancellation, resource-limit, audit, and migration semantics;
5. how it remains compatible with existing v1 data and clients.

Convenience for several features is not enough. A mechanism stays in the highest layer that can own its meaning and lifecycle.

## Stability commitment

Contract V1 remains operational and evolves through compatibility layers; candidate v2 becomes a stable constitution only through explicit adoption. The substrate may grow, but more slowly than products and protocols, and every addition should reduce future lock-in rather than expand it.
