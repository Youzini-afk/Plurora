# Architecture

> [English](./ARCHITECTURE.en.md) · [中文](./ARCHITECTURE.md)

Plurora is not a closed three-tier product stack of “kernel, packages, projects.” It is a set of open layers with explicit ownership and one-way dependencies: a very small constitutional substrate, evolvable protocols, replaceable components and content, a Host that manages real resources, replaceable distributions, and freely evolving products.

Contract V1 and the current code still retain historical boundaries such as `session`, `package`, `project`, `surface`, and `proposal`. They are the operational public contract, not automatically the permanent architecture. Long-term ownership is described by [`CONSTITUTION_V2.md`](CONSTITUTION_V2.en.md) and [`../spec/CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.en.md).

## Layering model

```text
┌─────────────────────────────────────────────────────────────────┐
│ Products / Experiences / Services                               │
│ Apps, tools, worlds, games, documents, agents, unknown forms    │
├─────────────────────────────────────────────────────────────────┤
│ Distributions / Shells / Clients                                │
│ First-party and third-party Web, Desktop, PWA, CLI, IDE, headless  │
├─────────────────────────────────────────────────────────────────┤
│ Protocol Commons                                                │
│ Shared meaning, profiles, state machines, migration, behavior   │
├─────────────────────────────────────────────────────────────────┤
│ Components / Content / Adapters                                 │
│ WASM, process, remote, trusted native, resources, adapters      │
├─────────────────────────────────────────────────────────────────┤
│ Constitutional Substrate                                        │
│ Identity, authority, objects, journal, causality, effects       │
└─────────────────────────────────────────────────────────────────┘

Host Control Plane / Runtime Fabric is orthogonal to those layers:
installation, processes, files, secrets, network, ports, targets,
deployment, backup, and diagnostics.
```

“Orthogonal” means that a Host can supply machine capability to different products and protocols without owning their content semantics. A headless service, a local creation tool, and a multiplayer world may share the same Host and substrate while using completely different protocols and shells.

## Responsibilities

### Constitutional Substrate

The substrate owns only mechanisms that cannot safely be reimplemented by an ordinary upper layer:

- principals and authenticated call context;
- authority minting, attenuation, delegation, leases, refresh, and revocation;
- content-addressed objects, verifiable references, and unforgeable handles;
- append-only journals, stable order, causal references, and head primitives;
- invoke, stream, cancel, deadlines, and backpressure;
- compare-and-swap, preconditions, idempotency, and atomic commit;
- effect receipts, audit, and provenance connection points;
- minimal component-instance lifecycle and health;
- protocol, version, and profile negotiation.

The substrate does not own Project, Home, Play, Forge, Assistant, models, agents, memory, worlds, documents, deployment products, or a concrete secret store.

The current `plurora-core` and `plurora-runtime` still mix some Host, protocol, and shell semantics. Migration separates them incrementally with compatibility and data safety rather than creating instability through a one-shot rewrite.

### Protocol Commons

Protocol Commons owns shared meaning and behavior required across implementations, including:

- data shape and field meaning;
- lifecycle, state machines, errors, and cancellation;
- authority, privacy, and effect requirements;
- compatibility profiles, version negotiation, and migration;
- executable behavioral contracts and implementation claims.

Agent, Memory, World, Document, Workspace, Surface, Change, and Inference may become protocols here, and competing protocols may coexist. First-party maintenance does not grant substrate routing priority; only participants that explicitly adopt a protocol or profile are constrained by it.

### Components / Content / Adapters

Components implement protocol capability, content carries user and product data, and adapters connect external systems. Execution forms include:

- sandboxed WASM components;
- isolated processes;
- remote service boundaries;
- trusted native implementations;
- static resources and surface bundles;
- foreign capsules.

They may expose capability through the same invocation protocol, but they cannot claim identical isolation, failure, latency, or supply-chain guarantees.

A Package is a retrieval, distribution, and installation envelope rather than the ontology unit for every kind of meaning. One Package may carry multiple components, protocol descriptors, content, and surfaces. Components, content, and protocols should have independent identity, digests, versions, and migration paths where practical. See [`CAPABILITY_PACKAGE.md`](CAPABILITY_PACKAGE.en.md).

### Host Control Plane / Runtime Fabric

The Host manages resources and operations in the real environment:

- package and component retrieval, installation, update, and removal;
- local processes, WASM, remote services, and target lifecycle;
- files, workspaces, secrets, network, ports, and proxies;
- installation instances such as Projects used by a distribution;
- deployment, runtime health, logs, backup, recovery, and diagnostics;
- device identity, resource selectors, and Host administration policy.

Host operations still use substrate identity, authority, effects, and audit. The ability to start a World or Document product does not give the Host authority to interpret its content.

### Distributions / Shells / Clients

A distribution organizes platform capability into a usable product. The current official distribution includes React Web/PWA, Tauri Desktop, and Rust CLI, and uses Home, Settings, Project frames, Console, and a Surface bridge.

These are official product choices:

- they may be opinionated and polished aggressively;
- they use only public contracts and explicit Host APIs;
- they cannot become a bypass for component authority;
- third-party shells, IDEs, headless clients, or radically different distributions may replace them.

Surface slots, Home cards, Forge panels, and Assistant actions belong to the current Shell Profile and Contract V1 compatibility surface, not the constitutional substrate.

### Products / Experiences / Services

The top layer owns domain ontology, business rules, and final interaction. Chat, worlds, games, design tools, ordinary Web services, IDEs, automation, and unknown future forms evolve here.

A product chooses:

- whether to use Project;
- whether to use event sourcing, branches, or approval;
- whether to use AI;
- whether to provide a UI;
- whether it is local, remote, collaborative, or offline;
- which protocols, components, and Host capabilities it adopts.

A product choice cannot become a requirement for every Plurora product in reverse.

## One-way dependency and the downward-movement gate

The intended dependency direction is:

```text
Product
  ↓
Distribution / Shell Profile
  ↓
Protocols and Components
  ↓
Constitutional Substrate
```

The Host supplies authorized real-world resources across the layers without creating reverse semantic dependency.

Before moving an upper-layer need downward, ask:

1. Is it an opinion of this product, or shared meaning needed by multiple independent products?
2. Does it belong in a competitive protocol, a Host machine operation, or a mechanism that truly cannot move above the substrate?
3. Would moving it downward reduce lock-in, or freeze the current first-party implementation into platform law?
4. Are versioning, migration, unknown-data preservation, and old-client compatibility defined?

The default answer is not “everything becomes a package.” It stays in the highest layer that owns its meaning and lifecycle.

## The place of Contract V1

Contract V1 is the current operational public contract, used for generated SDKs and guarded by conformance. It currently carries responsibilities from several layers:

- `context.*`, `journal.*`, `capability.*`, `authority.*`, `object.*`, and `identity.*` are substrate-owned;
- `host.*` owns Project, target, exec, port, proxy, outbound adapters, package operations, and diagnostics;
- `protocol.*`, `change.*`, and `projection.*` belong to evolvable Protocols;
- `shell.*` contribution discovery and surface interpretation belong to a Shell Profile.

Current clients and third-party integrations use these exact v1 identities. New meaning requires an explicit owner, maturity, schema, and negotiated version boundary; it must not accumulate as hidden aliases or first-party shortcuts. See [`../spec/CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.en.md) for itemized ownership.

## Current official distribution

### Web / PWA

`clients/web` is the official React 19 + Tailwind v4 + Vite platform shell and can be installed as a PWA. It uses the public platform and Host boundaries through HTTP `POST /rpc`, SSE, and `/host/v1/*`; it does not read SQLite or import private runtime state.

### SurfaceHost

Third-party Web surfaces are mounted in sandboxed iframes. A surface has no kernel access by default. The Host forwards only explicit methods and capability allowlists, and binds stream ownership, session, project/grant, and short-lived asset leases to the current mount. See [`../guides/SURFACE_HOSTING.md`](../guides/SURFACE_HOSTING.en.md).

### Desktop

`clients/desktop` is a Tauri 2.x wrapper and manages a loopback-only Host sidecar. It prepares a persistent profile, starts `plurora host serve` on a random loopback port, completes health and one-time bootstrap, reveals the Web shell, and terminates the sidecar on exit.

Desktop, Web/PWA, and remote Host connections reuse the same client core and public boundaries. Official Desktop owns no second protocol or private Studio.

### CLI and headless use

`plurora-cli` exposes Host, installation, Project, package, composition, contract, conformance, and operational entry points. Running locally does not allow the CLI to gain product authority by inspecting the Host data directory outside the public boundary.

## Current Project model

Project is the official Host's installable and runnable instance model, composed of `ProjectDescriptor`, `ProjectRegistry`, data directories, secret policy, and lifecycle. It is a Host and distribution concept, not constitutional substrate and not a root object required by every product.

World, Document, Service, Workspace, and other protocol objects may exist independently. When the current official Home needs to manage them, an adapter or product mapping may associate them with a Project without destroying their identity. See [`../guides/PROJECT_MODEL.md`](../guides/PROJECT_MODEL.en.md) and [`../product/PLATFORM_PRODUCT_MODEL.md`](../product/PLATFORM_PRODUCT_MODEL.en.md).

## Repository map

```text
crates/plurora-core      current core types, schemas, identity, events, contracts
crates/plurora-runtime   runtime, component execution, dispatch, some Host work
crates/plurora-service   HTTP / RPC / SSE and Host service boundary
crates/plurora-cli       CLI, Host, scaffolding, contract, conformance tooling
clients/web          official React Web shell / PWA
clients/desktop      Tauri wrapper + managed Host sidecar
packages/plurora    first-party components and experiments via manifests
sdk/                 generated contract SDKs and domain SDKs
profiles/            distribution / Host component and policy compositions
examples/            examples, fixtures, and third-party integration samples
docs/                charter, architecture, protocols, product, guides, status
```

The code layout still reflects historical Contract V1 aggregation. Permanent ownership cannot be inferred from crate names alone.

## Read next

- [`../CHARTER.md`](../CHARTER.en.md) — platform goals and non-negotiable principles;
- [`VISION.md`](VISION.en.md) — the intended long-term shape;
- [`CONSTITUTION_V2.md`](CONSTITUTION_V2.en.md) — candidate constitutional substrate;
- [`../spec/CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.en.md) — itemized ownership of the current contract;
- [`CONSTITUTIONAL_SUBSTRATE.md`](CONSTITUTIONAL_SUBSTRATE.en.md) — substrate responsibility and the current kernel compatibility boundary;
- [`CAPABILITY_PACKAGE.md`](CAPABILITY_PACKAGE.en.md) — Packages, components, content, and execution trust;
- [`../product/PLATFORM_PRODUCT_MODEL.md`](../product/PLATFORM_PRODUCT_MODEL.en.md) — product responsibility of the official distribution;
- [`../guides/PROJECT_MODEL.md`](../guides/PROJECT_MODEL.en.md) — the current official Project model;
- [`../ALPHA_STATUS.md`](../ALPHA_STATUS.en.md) — actual implementation status;
- [`../roadmap/NEXT_STEPS.md`](../roadmap/NEXT_STEPS.en.md) — current construction direction.
