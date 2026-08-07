# Yggdrasil Documentation

> [English](./README.en.md) · [中文](./README.md)

The documentation is organized by long-term principles, current implementation, architecture layer, product, public contracts, and authoring guides. Major documents are available in English and Simplified Chinese. Follow [`STYLE.md`](STYLE.en.md) when writing documentation.

## Newcomer 1 / 2 / 3 path

1. Read [`CHARTER.md`](CHARTER.en.md) → [`architecture/VISION.md`](architecture/VISION.en.md) to understand why the platform exists and what it pursues for the long term.
2. Read [`architecture/ARCHITECTURE.md`](architecture/ARCHITECTURE.en.md) → [`architecture/PLATFORM_KERNEL.md`](architecture/PLATFORM_KERNEL.en.md) → [`architecture/CAPABILITY_PACKAGE.md`](architecture/CAPABILITY_PACKAGE.en.md) to understand the boundaries among substrate, protocols, components, Host, distributions, and products.
3. Read [`product/PLATFORM_PRODUCT_MODEL.md`](product/PLATFORM_PRODUCT_MODEL.en.md) and walk through [`guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](guides/PACKAGE_AUTHORING_WALKTHROUGH.en.md) to understand official product responsibility and complete a first Package / Component.

Then enter the relevant guide, spec, or roadmap as needed.

## Principles, product, and status

- [`CHARTER.md`](CHARTER.en.md) — platform identity, five long-term goals, and non-negotiable principles
- [`architecture/VISION.md`](architecture/VISION.en.md) — intended long-term shape and technical direction
- [`product/PLATFORM_PRODUCT_MODEL.md`](product/PLATFORM_PRODUCT_MODEL.en.md) — user lifecycles, usability principles, and product boundaries of the official distribution
- [`product/PLAY_CREATION_MODEL.md`](product/PLAY_CREATION_MODEL.en.md) — optional play-creation product profile, not the only form of the platform
- [`ALPHA_STATUS.md`](ALPHA_STATUS.en.md) — factual snapshot of implemented, partial, and deferred work
- [`roadmap/NEXT_STEPS.md`](roadmap/NEXT_STEPS.en.md) — construction direction around openness, plurality, advanced capability, longevity, and usability
- [`STYLE.md`](STYLE.en.md) — documentation truth hierarchy, bilingual synchronization, and writing red lines
- [`LICENSING.md`](LICENSING.en.md) — `AGPL-3.0-only` first-party scope and third-party license boundaries
- [`../BUILDING.md`](../BUILDING.md) — Rust, Web, Tauri Desktop, and release build instructions

## Architecture and public contracts

- [`architecture/`](architecture/README.en.md) — layered architecture, constitutional substrate, components, and Host control planes
- [`protocol/`](protocol/README.en.md) — current public transport and invocation protocol
- [`spec/`](spec/README.en.md) — Contract V1 schemas, Contract Registry, Protocol Commons, and compatibility contracts
- [`architecture/CONSTITUTION_V2.md`](architecture/CONSTITUTION_V2.en.md) → [`spec/CONTRACT_LAYERING_MATRIX.md`](spec/CONTRACT_LAYERING_MATRIX.en.md) — candidate long-term constitution and itemized ownership of the current contract; v1 remains current

## Authoring, installation, and operation

- [`guides/`](guides/README.en.md) — guides grouped by foundation, agent, model, inference, experience, memory, storage, external projects, and distribution
- [`guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](guides/PACKAGE_AUTHORING_WALKTHROUGH.en.md) — first Package / Component
- [`guides/CAPABILITY_HANDLES.md`](guides/CAPABILITY_HANDLES.en.md) — authority handles, attenuation, revocation, and effect audit
- [`guides/CONFORMANCE_KIT.md`](guides/CONFORMANCE_KIT.en.md) — Contract V1 behavioral checks for third-party implementations
- [`guides/PACKAGE_INSTALLATION.md`](guides/PACKAGE_INSTALLATION.en.md) — installation, updates, lockfiles, content-addressed store, and consent prompts
- [`guides/PROJECT_MODEL.md`](guides/PROJECT_MODEL.en.md) — current official Host Project installation-instance model and boundary
- [`guides/SECRET_MANAGEMENT.md`](guides/SECRET_MANAGEMENT.en.md) — `secret_ref`, local encrypted store, and API key management
- [`guides/REAL_MODEL_END_TO_END.md`](guides/REAL_MODEL_END_TO_END.en.md) — real-provider end-to-end invocation
- [`guides/PATH_B_SELF_CONTAINED.md`](guides/PATH_B_SELF_CONTAINED.en.md) — `entry.contract: "none"` self-contained path
- [`guides/SURFACE_HOSTING.md`](guides/SURFACE_HOSTING.en.md) — iframe SurfaceHost and third-party Web Surface hosting

## Performance, status, and external integrations

- [`performance/`](performance/README.en.md) — performance baselines, quality feedback, and code health
- [`roadmap/`](roadmap/README.en.md) — construction direction that still affects current choices
- [`tavern/`](tavern/README.en.md) — relationship between Yggdrasil and the independent YdlTavern integration project

## Shortest path by intent

| Goal | Read first |
|---|---|
| Understand platform goals | [`CHARTER.md`](CHARTER.en.md) → [`architecture/VISION.md`](architecture/VISION.en.md) |
| Understand layering | [`architecture/ARCHITECTURE.md`](architecture/ARCHITECTURE.en.md) → [`architecture/PLATFORM_KERNEL.md`](architecture/PLATFORM_KERNEL.en.md) → [`architecture/CAPABILITY_PACKAGE.md`](architecture/CAPABILITY_PACKAGE.en.md) |
| Understand the official product | [`product/PLATFORM_PRODUCT_MODEL.md`](product/PLATFORM_PRODUCT_MODEL.en.md) → [`design/PLATFORM_UI_DESIGN.md`](design/PLATFORM_UI_DESIGN.en.md) |
| Review long-term contract boundaries | [`architecture/CONSTITUTION_V2.md`](architecture/CONSTITUTION_V2.en.md) → [`spec/CONTRACT_LAYERING_MATRIX.md`](spec/CONTRACT_LAYERING_MATRIX.en.md) → [`spec/CONTRACT_REGISTRY.md`](spec/CONTRACT_REGISTRY.en.md) |
| Use public contracts | [`protocol/PROTOCOL_V0.md`](protocol/PROTOCOL_V0.en.md) → [`spec/KERNEL_V1_CONTRACT.md`](spec/KERNEL_V1_CONTRACT.en.md) |
| Write a first Package / Component | [`guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](guides/PACKAGE_AUTHORING_WALKTHROUGH.en.md) |
| Install a Package / Project | [`guides/PACKAGE_INSTALLATION.md`](guides/PACKAGE_INSTALLATION.en.md) → [`guides/PROJECT_MODEL.md`](guides/PROJECT_MODEL.en.md) |
| Manage API keys / secrets | [`guides/SECRET_MANAGEMENT.md`](guides/SECRET_MANAGEMENT.en.md) |
| Run real model invocation | [`guides/REAL_MODEL_END_TO_END.md`](guides/REAL_MODEL_END_TO_END.en.md) |
| Host third-party Web Surfaces | [`guides/SURFACE_HOSTING.md`](guides/SURFACE_HOSTING.en.md) |
| Build Web / Desktop / Release | [`../BUILDING.md`](../BUILDING.md) |
| Review current status | [`ALPHA_STATUS.md`](ALPHA_STATUS.en.md) |
| Review construction direction | [`roadmap/NEXT_STEPS.md`](roadmap/NEXT_STEPS.en.md) |
| Write documentation | [`STYLE.md`](STYLE.en.md) |
