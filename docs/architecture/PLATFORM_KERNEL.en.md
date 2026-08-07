# Platform Substrate and Current Kernel Boundary

> [English](./PLATFORM_KERNEL.en.md) · [中文](./PLATFORM_KERNEL.md)

This document separates two things:

1. the deliberately small **Constitutional Substrate** Plurora should retain for the long term;
2. the implementation boundary currently named kernel in Contract V1.

They do not yet coincide completely. To provide an operational platform, the current `plurora-core` and `plurora-runtime` also carry some Host, protocol, and shell responsibility. The long-term goal is not to invalidate that code, but to return responsibility to the correct layer while preserving compatibility, migration, and data readability.

## Mechanisms owned by the constitutional substrate

### Identity and authenticated context

- principals, caller identity, and authenticated invocation context;
- generic context such as traces, parent invocation, tenant, or Host boundary;
- stable identity that does not depend on display names, paths, or caller-supplied claims.

### Authority

- capability and authority minting, attenuation, delegation, leases, refresh, and revocation;
- resource selectors, conditions, quotas, and expiration;
- policy decisions and authority provenance;
- reauthorization at effect boundaries for long-running work.

A Manifest or protocol declaration is a request ceiling, not an actual grant.

### Objects and verifiable references

- content-addressed object storage;
- open artifact descriptors;
- digest, size, references, and integrity verification;
- lossless preservation and transfer of unknown artifact types.

Host paths, temporary URLs, process IDs, and database row numbers cannot become durable portable identity.

### Journal, causality, and head primitives

- stable, pageable append-only order inside a scope;
- explicit causation, correlation, and parent references;
- the minimal primitives needed for branches and heads;
- historical facts that cannot be silently rewritten.

Protocols own how a domain interprets events, merges branches, or defines a World or Document head.

### Invocation, streams, and cancellation

- capability and component invocation;
- streaming frames, progress, and backpressure;
- cancellation, deadlines, timeout, and terminal state;
- idempotency keys, retry semantics, and invocation receipts.

### Transaction and commit primitives

- compare-and-swap;
- preconditions;
- atomic state update;
- idempotency and explicit partial-failure semantics.

The substrate does not freeze those primitives into one product's proposal or publishing workflow.

### Effect receipts and audit

- auditable records for external effects and nondeterministic behavior;
- references to input, output, component, authority, policy, and approval;
- explicit distinction among success, rejection, cancellation, timeout, and partial completion;
- separation of historical replay from re-execution.

A receipt records necessary references and decisions without copying raw secrets or unrelated user content.

### Minimal component lifecycle

- component-instance activation, health, deactivation, and failure boundary;
- export and import bindings needed for invocation;
- visible trust class and enforced-boundary claims.

Package download, installation directories, and user-facing management belong to the Host and distribution rather than the substrate.

### Protocol and version negotiation

- explicit selection of protocol IDs, versions, and profiles;
- requirements that do not allow silent downgrade;
- Contract Registry, aliases, and legacy-adapter entry points;
- transport-independent behavioral semantics.

## What the constitutional substrate does not own

The substrate does not own:

- Project, Home, Library, Play, Forge, Assistant, or Editor;
- package registries, marketplaces, installation shelves, or update products;
- workspaces, Docker, targets, exec, ports, proxies, deployment, or concrete backup products;
- Chat, Message, Turn, Prompt, Model, Agent, or Memory;
- World, Entity, Scene, Quest, Document, Game, or Simulation;
- a concrete secret store, database, vector store, or model vendor;
- one fixed proposal, approval, change, or publishing workflow;
- any official component ID or UI state.

Those concepts belong respectively to the Host, Protocol Commons, a distribution, or a product. They may be important and stable without belonging to the constitutional substrate.

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

### No official privilege

Official and third-party components use the same registration, bindings, invocation, and audit mechanisms. Package names cannot determine authority or routing priority.

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
3. whether it increases user freedom and replaceability rather than locking in the current official implementation;
4. its error, cancellation, resource-limit, audit, and migration semantics;
5. how it remains compatible with existing v1 data and clients.

Convenience for several features is not enough. A mechanism stays in the highest layer that can own its meaning and lifecycle.

## Stability commitment

Contract V1 remains operational and evolves through compatibility layers; candidate v2 becomes a stable constitution only through explicit adoption. The substrate may grow, but more slowly than products and protocols, and every addition should reduce future lock-in rather than expand it.
