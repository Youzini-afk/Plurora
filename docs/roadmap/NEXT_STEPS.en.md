# Construction Direction

> [English](./NEXT_STEPS.en.md) · [中文](./NEXT_STEPS.md)

Plurora does not move forward by expanding around one product, example, or workflow, and it does not invent features to complete a proof exercise. Construction direction comes from the five long-term goals in the charter: openness, plurality, advanced capability, longevity, and usability.

These directions progress in parallel. Phase numbers would create a false linear order. Every concrete task should belong to a clear user lifecycle, architecture layer, and long-term responsibility.

Work, Assembly, Installation, Run, Exposure/Binding, Target-compiled Realization, Foreign Work, Rights/Transparency, opaque-state backup, and the modular creation loop now form one operational line. Current priorities shift to complete official-product lifecycles, a continuous creator path, real WASM/remote-component execution boundaries, and long-term data governance. Finished one-time implementation stages remain only in Git history.

## Most important current work

### Make the official distribution complete and usable

Web, Desktop, PWA, and CLI already expose substantial capability, but everyday use can still feel like a platform control plane rather than a mature product. Complete lifecycles before adding more isolated panels:

- first launch, local managed Host, remote Host connection, and recovery;
- discovery, import, installation, authorization, running, stopping, update, and removal;
- state, progress, cancellation, failure explanation, and next actions;
- storage use, backup, export, migration, archive, and deletion;
- human-readable authority requests, duration, target resources, and revocation;
- mobile, keyboard, accessibility, internationalization, slow networks, and low-power devices;
- progressive disclosure between simple use and advanced control.

The official distribution continues to use public contracts and does not create private Desktop capability or first-party Package shortcuts. See [`../product/PLATFORM_PRODUCT_MODEL.md`](../product/PLATFORM_PRODUCT_MODEL.en.md).

### Make creator experience one continuous path

A creator should not need to understand the whole kernel before beginning, and should not be trapped in an unextensible low-code surface when deeper work is needed. The path should connect:

```text
template / import
→ local run and hot reload
→ observe events, calls, objects, and state
→ modify content, Assemblies, components, or protocols
→ human / AI assistance
→ debug and test
→ package, share, realize
→ update and migrate
```

Near-term priorities include:

- clear, low-boilerplate Package, Component, and Surface templates;
- consistent TypeScript, Rust, and future WASM SDK experience;
- local development mode, hot reload, source maps, logs, and error location;
- visible authority, protocol bindings, effects, and artifact provenance;
- understandable Assembly composition and dependency-conflict diagnostics;
- AI tools that work through ordinary capabilities, explicit scope, and reviewable changes rather than a generic root shell;
- a stable path from local work to a shareable artifact.

Play-creation is only an optional Profile. Document tools, services, IDEs, and headless systems may use different creation flows.

### Converge long-term ownership of Contract V1

Contract V1 remains the supported exact public boundary. Current work should:

- make owners across substrate, Host, Protocol Commons, and Shell Profile visible;
- give new capability an explicit namespace, version, and maturity;
- maintain the exact Contract Registry, generated identities, and explicit contract/profile/Protocol negotiation;
- preserve readable prior data and unknown fields through explicit versioned readers and migration tooling when a breaking boundary is introduced;
- classify Surface slots, Work, Targets, and Realization as Profile or Host rather than permanent substrate ontology;
- create a Protocol only when real shared semantics exist, rather than freezing one Package's private JSON prematurely.

See [`../spec/CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.en.md) for itemized ownership and [`../architecture/CONSTITUTION_V2.md`](../architecture/CONSTITUTION_V2.en.md) for the candidate constitution.

### Complete component execution and the trust model

Rust in-process and subprocess execution are operational. WASM and general remote components still provide substantial opportunity. Advanced execution work is selected for real benefit:

- WASM Component and WIT for portability, explicit imports, and resource limits;
- clearer OS-level filesystem, network, and resource enforcement claims for subprocesses;
- identity, tenancy, deadlines, reconnect, idempotency, and effect receipts for remote components;
- trusted native retained as a high-performance escape hatch rather than a home for untrusted dynamic code;
- separation of static resources from executable components;
- UI and conformance that honestly present the guarantees of each trust class.

WASM, remote execution, and new transports do not become priorities merely because they are newer. They progress when they add portability, security, performance, or ecosystem language choice.

### Treat user data, content, and artifacts as long-term assets

ObjectStore, ArtifactDescriptor, World Bundle, Realization plan/resource artifacts, and effect receipts already exist, but long-term data governance must converge:

- clear classification of user data, reconstructable cache, executable artifacts, and temporary diagnostics;
- content digests, references, provenance, reachability, retention, and garbage collection;
- encryption, backup, export, import, migration, and deletion;
- lossless transfer of unknown artifact types and unknown fields;
- Component updates that never silently overwrite user content;
- historical replay that uses recorded results while re-execution creates a new causal branch;
- multi-Host replication and conflict policy defined by adopted protocols rather than accidental filesystem paths.

Portability is part of platform identity, not release polish.

### Build competitive Protocol Commons

The platform needs interoperability stronger than “everyone sends JSON” without freezing official opinions into the only standard. Protocol work includes:

- protocol descriptors, profiles, versions, and maturity;
- field meaning, lifecycle, errors, cancellation, effects, and privacy;
- adapters, migration, and deprecation windows;
- implementation claims and behavioral checks;
- explicit selection when multiple implementations or protocols coexist;
- adapters to external ecosystems such as MCP, A2A, OCI, and WASI rather than unconditional reinvention.

Candidate areas include Surface, Change, Workspace, Inference, Agent, Memory, World, Document, Sharing, and Evaluation. Each may have competing approaches and does not become Stable merely by being official.

### Strengthen local-first, remote, and multi-Host use

Local use remains first-class while users can extend capability to remote devices and services:

- a managed local Host works without a cloud account;
- remote Hosts use explicit HTTPS identity, pairing, grants, and resource selectors;
- one client strictly isolates credentials, caches, and Installation or Target preferences between Hosts;
- device revocation, ancestor revocation, offline state, reconnect, and expiration are understandable;
- large artifact transfer is resumable, verifiable, and bounded;
- multi-Host content and state migration do not depend on local absolute paths;
- remote capability does not degrade into arbitrary shell or unlimited filesystem access.

### Keep improving reliability, performance, and maintainability

Quality systems serve product and platform construction:

- tests cover authority boundaries, migration, recovery, cancellation, concurrency, and data integrity;
- conformance constrains public contract behavior rather than deciding product direction;
- performance baselines cover startup, interaction, streams, object transfer, mobile networks, and resource use;
- failure injection covers process exit, Host restart, network loss, revocation, partial effects, and damaged artifacts;
- documentation, schemas, SDKs, code, and CI remain aligned on implementation facts;
- obsolete compatibility layers and temporary plans are removed instead of accumulating forever.

The current implementation snapshot is in [`../ALPHA_STATUS.md`](../ALPHA_STATUS.en.md).

## Things that are explicitly not the center of the platform

These may continue to be built, but none defines Plurora's only direction:

- one Work-centric workflow;
- play-creation, Tavern, worlds, or chat;
- agents or model inference;
- Docker and the Realization executor;
- the official Web or Desktop shell;
- one protocol profile;
- one demonstration, fixture, or external source.

They are products, protocols, Host capabilities, or use cases on the platform. Importance does not grant ownership of every other direction.

## Areas not expanded proactively

Without clear user value and layer ownership, do not proactively add:

- Realization backends added for feature count;
- kernel-owned chat, agent, memory, world, or product UI semantics;
- first-party Package private APIs, name privilege, or hidden routing;
- arbitrary remote shells, unlimited Host filesystems, or long-lived root credentials;
- Stable schemas without migration paths;
- fashionable technology without capability benefit;
- marketplace, billing, and ecosystem economics while core lifecycles remain incomplete.

These are not permanent prohibitions. They may return when their value, boundary, and maintenance model are clear.

## Selecting concrete work

Before entering the active queue, a task should answer:

1. Which user or creator gains which real capability?
2. Does it make a lifecycle more complete, reliable, or understandable?
3. Does it belong to substrate, protocol, component, Host, distribution, or a product?
4. Does it preserve data ownership, public boundaries, and replacement space?
5. Does its technology produce measurable security, performance, portability, or maintenance benefit?
6. What are its error, cancellation, recovery, migration, and deletion paths?
7. Would it freeze a current official choice into a requirement for the whole platform?

Tests, fixtures, and conformance follow design to keep those goals from regressing; they do not replace the goals.

## Documentation and status

- Platform identity and principles: [`../CHARTER.md`](../CHARTER.en.md)
- Long-term shape: [`../architecture/VISION.md`](../architecture/VISION.en.md)
- Layered architecture: [`../architecture/ARCHITECTURE.md`](../architecture/ARCHITECTURE.en.md)
- Official product responsibility: [`../product/PLATFORM_PRODUCT_MODEL.md`](../product/PLATFORM_PRODUCT_MODEL.en.md)
- Current implementation snapshot: [`../ALPHA_STATUS.md`](../ALPHA_STATUS.en.md)

This roadmap describes construction direction. It does not promise that every item proceeds simultaneously and does not present candidates as implemented facts.
