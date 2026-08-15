# Vision

> [English](./VISION.en.md) · [中文](./VISION.md)

Plurora should become open digital ground that people can depend on for a long time: immediately useful to ordinary users, fast for creators to build with, deep enough for professional extension, reliable for operators, and spacious enough for forms the platform authors never anticipated.

AI-native capability is important, but the platform does not define itself as chat, agents, worlds, games, or any other single product form.

## Intended platform shape

The layering diagram is maintained only in [`ARCHITECTURE.md`](ARCHITECTURE.en.md). What the vision needs to keep is this: distributions and products may choose different protocols, components, and Host capabilities. The non-optional boundary is explicit identity, authority, and public contracts.

## Openness in actual use

A user should be able to:

- use Plurora locally without mandatory cloud registration;
- take away data, history, configuration, compositions, and content;
- replace models, components, clients, storage, and Hosts without rebuilding all work;
- inspect which code, service, or principal has which authority;
- reject official defaults in favor of third-party or self-built implementations;
- read important data after an official service disappears.

A third-party author should be able to integrate from public documentation, SDKs, and protocols alone, without copying repository-internal code or receiving a whitelist identity.

## How plurality emerges

The platform decouples these dimensions as far as practical:

```text
content and data model
× protocol semantics
× component implementation
× interaction and surfaces
× models and agents
× execution and isolation
× storage and retrieval
× local / remote / multiple Hosts
× individual / collaborative / automated use
```

A product may select only a small subset. A local tool need not carry deployment. A headless service need not have Home. A world product need not use Project Console. An ordinary Web application need not adopt agents or models.

This requires Plurora to distinguish mechanisms, protocols, distributions, and products rather than forcing everything into a kernel-or-package binary.

## Technical direction

Plurora's technical ambition focuses on foundations that improve long-term capability:

- **Capability-oriented security:** least authority, attenuation, delegation, leases, revocation, and understandable authority chains;
- **Content-addressed data:** verifiable identity for important objects without dependence on local paths;
- **Portable components:** WASM, isolated processes, remote boundaries, and trusted native implementations can serve the same protocols while disclosing different trust guarantees;
- **Local-first and multi-Host:** local and offline operation first, with safe expansion to remote devices and multiple nodes;
- **Streaming execution:** streams, cancellation, deadlines, backpressure, recovery, and long-running observation as platform capabilities;
- **Causality and effects:** historical replay separated from re-execution, with auditable receipts for nondeterminism and external effects;
- **Protocol evolution:** explicit negotiation, maturity, compatibility profiles, deprecation, migration, and versioned readers;
- **Implementation independence:** data and protocols are not bound to a language, UI framework, model vendor, or cloud platform.

These directions do not all enter the stable layer at once. Maturity and real value decide whether a mechanism remains Experimental, becomes Candidate, or reaches Stable.

## Long-term evolution

Plurora does not interpret compatibility as preserving every failed decision forever. It evolves through:

- a very small stable substrate;
- versioned and competitive protocol commons;
- independently addressed components, content, and protocol descriptors;
- compatibility reading, explicit migration, and reversible adoption;
- lossless preservation of unknown artifacts;
- clear separation of historical fact, current state, and re-execution;
- the ability to rewrite the official distribution while user data and third-party components survive.

Contract V1 is the current operational public contract; it does not automatically become the permanent constitution. Long-term ownership and migration boundaries are described by [`CONSTITUTION_V2.md`](CONSTITUTION_V2.en.md) and [`../spec/CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.en.md).

## A usable default distribution

The official Plurora distribution is not merely a demonstration shell. It should be a product people choose to use for the long term. It needs to:

- work locally after installation;
- make discovery, installation, running, stopping, updating, and removal clear;
- present authority, risk, state, and recovery in human terms;
- give creators templates, debugging, hot reload, composition, asset management, and AI assistance;
- make local/remote connection, backup, migration, and recovery direct;
- hide unnecessary complexity from simple use while retaining depth for experts;
- implement everything through public protocols so third-party distributions can choose another path.

The official distribution may organize itself around Projects, Home, a Workbench, or other concepts, but those are product choices rather than ontology requirements for every upper-layer system.

## Platform and product

The platform expands the space of choices and provides trusted common ground. A product makes opinionated choices and completes one path through that space.

Therefore:

- neutrality cannot excuse the platform from building an excellent default experience;
- convenience cannot let a product demand permanent adoption of its ontology;
- the Host may own real machine-operation semantics without moving product content semantics into the substrate;
- a protocol may constrain participants that adopt it, but it is not the only possible protocol;
- a successful official product improves usability while remaining replaceable.

## Desired result

As Plurora matures, it should support all of these at once:

- an official distribution that non-technical users can install and use reliably;
- third-party components, protocols, shells, and complete products;
- local, remote, headless, and embedded deployments;
- AI-intensive products and products with no AI dependency;
- data and history that survive implementation, machine, and time boundaries;
- uses the platform authors neither predicted nor specially authorized.

No single feature, example, or acceptance exercise defines that destination. It is the shared direction maintained throughout construction.
