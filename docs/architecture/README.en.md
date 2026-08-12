# Architecture

> [English](./README.en.md) · [中文](./README.md)

These documents cover Plurora's long-term layering, the current public-contract boundary, and concrete Host control-plane designs. The long-term architecture distinguishes constitutional substrate, Protocol Commons, components/content, Host, distributions, and products rather than reducing every concept to a kernel-or-Package binary.

## Platform shape and substrate

- [`VISION.md`](VISION.en.md) — intended long-term shape, openness, and technical direction
- [`ARCHITECTURE.md`](ARCHITECTURE.en.md) — layers, one-way dependencies, orthogonal Host relationship, and current implementation placement
- [`CONSTITUTIONAL_SUBSTRATE.md`](CONSTITUTIONAL_SUBSTRATE.en.md) — mechanisms owned by the constitutional substrate and responsibilities currently carried by the wider runtime
- [`CONSTITUTION_V2.md`](CONSTITUTION_V2.en.md) — candidate long-term constitution, invariants, and evolution constraints; it does not yet replace Contract V1

## Packages, Components, Protocols, and runtime

- [`CAPABILITY_PACKAGE.md`](CAPABILITY_PACKAGE.en.md) — Package Envelopes, Components, Protocols, Content, and execution trust
- [`EXTENSION_POINTS.md`](EXTENSION_POINTS.en.md) — current extension-point and hook contracts and their long-term protocol ownership
- [`EVENT_MODEL.md`](EVENT_MODEL.en.md) — journals, events, and opaque payloads
- [`RUNTIME_LIFECYCLE.md`](RUNTIME_LIFECYCLE.en.md) — current runtime and Component lifecycles

## Host control planes

- [`HOST_DEVELOPMENT_CONTROL_PLANE.md`](HOST_DEVELOPMENT_CONTROL_PLANE.en.md) — controlled source change, verification, promotion, and recovery
- [`HOST_REMOTE_ACCESS.md`](HOST_REMOTE_ACCESS.en.md) — root/device identity, scope, HTTPS pairing, and application-route exposure
- [`HOST_RESOURCE_AUTHORITY.md`](HOST_RESOURCE_AUTHORITY.en.md) — Host authority, authenticated context, and audit for Work / Workspace / Installation / Run resources
- [`REALIZATION_CONTROLLER.md`](REALIZATION_CONTROLLER.en.md) — desired/observed state, idempotent operations, safe activation, and recovery
- [`TARGET_AGENT_PROTOCOL.md`](TARGET_AGENT_PROTOCOL.en.md) — remote target identity, typed operations, artifact/secret, and tunnel boundaries
- [`OPERATIONS_DATA_RELEASE.md`](OPERATIONS_DATA_RELEASE.en.md) — migration, backup, health, diagnostics, upgrades, and supply chain

These Host designs manage real resources without entering the constitutional substrate or owning upper-layer product content semantics.

## External ecosystem boundaries

- [`PI_INTEGRATION.md`](PI_INTEGRATION.en.md) — Component and Protocol absorption boundary for the pi agent framework

## Ownership of the current contract

Contract V1 remains the current public contract. To classify a method, object, or event as substrate, Host, Protocol, or Shell Profile, read [`../spec/CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.en.md) and [`../spec/CONTRACT_REGISTRY.md`](../spec/CONTRACT_REGISTRY.en.md).
