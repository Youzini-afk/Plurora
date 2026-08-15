# Plurora documentation

> [English](./README.en.md) · [中文](./README.md)

The docs are organized by try-it path, principles, architecture, product, protocol, and guides. Major documents are available in English and Simplified Chinese. Follow [`STYLE.md`](STYLE.en.md) when writing.

**Just run it:** [`guides/GETTING_STARTED.md`](guides/GETTING_STARTED.en.md) or [`../BUILDING.md`](../BUILDING.md).

## Two reading tracks

### Try-it track (about 15 minutes)

1. [`guides/GETTING_STARTED.md`](guides/GETTING_STARTED.en.md) — start the Host and Web shell, check the example Work.
2. Open Library, or continue with [`guides/MODULAR_SIMULATION.md`](guides/MODULAR_SIMULATION.en.md).
3. To write a package, read [`guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](guides/PACKAGE_AUTHORING_WALKTHROUGH.en.md).

### Understand track (deeper)

1. [`CHARTER.md`](CHARTER.en.md) — why the platform exists.
2. [`architecture/ARCHITECTURE.md`](architecture/ARCHITECTURE.en.md) — layering and ownership; this is the only document that holds the full layering diagram.
3. [`product/PLATFORM_PRODUCT_MODEL.md`](product/PLATFORM_PRODUCT_MODEL.en.md) — official-distribution product responsibility.
4. [`protocol/PUBLIC_PROTOCOL.md`](protocol/PUBLIC_PROTOCOL.en.md) → [`spec/PUBLIC_CONTRACT.md`](spec/PUBLIC_CONTRACT.en.md) — public transport and method semantics.

Implementation reconciliation and exact counts live in [`ALPHA_STATUS.md`](ALPHA_STATUS.en.md). That snapshot is for contributors, not the public front page.

## Principles, product, and status

- [`CHARTER.md`](CHARTER.en.md) — platform identity, five long-term goals, and non-negotiable principles
- [`architecture/VISION.md`](architecture/VISION.en.md) — long-term shape and technical direction (the layering diagram lives in ARCHITECTURE)
- [`product/PLATFORM_PRODUCT_MODEL.md`](product/PLATFORM_PRODUCT_MODEL.en.md) — official-distribution user lifecycle and product boundary
- [`product/PLAY_CREATION_MODEL.md`](product/PLAY_CREATION_MODEL.en.md) — optional play-creation profile, not the only form of the platform
- [`ALPHA_STATUS.md`](ALPHA_STATUS.en.md) — factual snapshot of implemented, partial, and deferred work
- [`roadmap/NEXT_STEPS.md`](roadmap/NEXT_STEPS.en.md) — construction direction, not a dated schedule
- [`STYLE.md`](STYLE.en.md) — documentation truth hierarchy, bilingual sync, and writing red lines
- [`LICENSING.md`](LICENSING.en.md) — `AGPL-3.0-only` first-party scope and third-party boundary
- [`../CONTRIBUTING.md`](../CONTRIBUTING.en.md) — how to make a reviewable change
- [`../BUILDING.md`](../BUILDING.md) — Rust, Web, Desktop, and release builds

## Architecture and public contracts

- [`architecture/`](architecture/README.en.md) — layered architecture, substrate, components, and Host control planes
- [`protocol/`](protocol/README.en.md) — transport and invocation envelope
- [`spec/`](spec/README.en.md) — Contract V1 schemas, registry, and compatibility contracts

The candidate long-term constitution [`architecture/CONSTITUTION_V2.md`](architecture/CONSTITUTION_V2.en.md) has not replaced v1 and is not on the default path. Read [`spec/CONTRACT_LAYERING_MATRIX.md`](spec/CONTRACT_LAYERING_MATRIX.en.md) when you need itemized ownership.

## Authoring, installation, and operation

- [`guides/`](guides/README.en.md) — grouped as start / advanced / models / agents / lab
- [`guides/GETTING_STARTED.md`](guides/GETTING_STARTED.en.md) — see the UI from source
- [`guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](guides/PACKAGE_AUTHORING_WALKTHROUGH.en.md) — first Package / Component
- [`guides/INSTALLATION_MODEL.md`](guides/INSTALLATION_MODEL.en.md) — Work / Workspace / Installation and install operations
- [`guides/RUN_LIBRARY.md`](guides/RUN_LIBRARY.en.md) — start and stop Runs
- [`guides/SECRET_MANAGEMENT.md`](guides/SECRET_MANAGEMENT.en.md) — `secret_ref` and API keys
- [`guides/MODULAR_SIMULATION.md`](guides/MODULAR_SIMULATION.en.md) — continued-creation example Work kit

## Performance, status, and external integrations

- [`performance/`](performance/README.en.md) — performance baselines and code health
- [`roadmap/`](roadmap/README.en.md) — construction direction that still affects choices
- [`tavern/`](tavern/README.en.md) — boundary with the independent YdlTavern project

## Shortest path by intent

| Goal | Read first |
|---|---|
| Run it in five minutes | [`guides/GETTING_STARTED.md`](guides/GETTING_STARTED.en.md) |
| Understand platform goals | [`CHARTER.md`](CHARTER.en.md) → [`architecture/ARCHITECTURE.md`](architecture/ARCHITECTURE.en.md) |
| Understand the official product | [`product/PLATFORM_PRODUCT_MODEL.md`](product/PLATFORM_PRODUCT_MODEL.en.md) → [`design/PLATFORM_UI_DESIGN.md`](design/PLATFORM_UI_DESIGN.en.md) |
| Use public contracts | [`protocol/PUBLIC_PROTOCOL.md`](protocol/PUBLIC_PROTOCOL.en.md) → [`spec/PUBLIC_CONTRACT.md`](spec/PUBLIC_CONTRACT.en.md) |
| Write a first Package | [`guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](guides/PACKAGE_AUTHORING_WALKTHROUGH.en.md) |
| Pack Work / manage Installation | [`guides/INSTALLATION_MODEL.md`](guides/INSTALLATION_MODEL.en.md) |
| Manage API keys | [`guides/SECRET_MANAGEMENT.md`](guides/SECRET_MANAGEMENT.en.md) |
| Build Web / Desktop / Release | [`../BUILDING.md`](../BUILDING.md) |
| Review current implementation facts | [`ALPHA_STATUS.md`](ALPHA_STATUS.en.md) |
| Write documentation | [`STYLE.md`](STYLE.en.md) |
