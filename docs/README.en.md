# Yggdrasil Documentation

> [English](./README.en.md) · [中文](./README.md)

Topic-grouped navigation for the developer docs. Every doc has both English and Simplified Chinese versions; the bilingual blockquote at the top of each file switches between them. When writing docs, follow [`STYLE.md`](STYLE.en.md).

## Newcomer 1 / 2 / 3 path

1. Read [`CHARTER.md`](CHARTER.en.md) → [`architecture/VISION.md`](architecture/VISION.en.md) → [`product/PLAY_CREATION_MODEL.md`](product/PLAY_CREATION_MODEL.en.md) for the platform stance.
2. Read [`architecture/ARCHITECTURE.md`](architecture/ARCHITECTURE.en.md) → [`architecture/PLATFORM_KERNEL.md`](architecture/PLATFORM_KERNEL.en.md) → [`architecture/CAPABILITY_PACKAGE.md`](architecture/CAPABILITY_PACKAGE.en.md) for the three-tier architecture and its boundaries.
3. Walk through [`guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](guides/PACKAGE_AUTHORING_WALKTHROUGH.en.md) to ship a first capability package.

After that, hop into the relevant guide / spec / roadmap on demand.

## Stance and status

- [`CHARTER.md`](CHARTER.en.md) — permanent principles
- [`ALPHA_STATUS.md`](ALPHA_STATUS.en.md) — living snapshot of done / partial / deferred
- [`STYLE.md`](STYLE.en.md) — documentation conventions and red lines
- [`LICENSING.md`](LICENSING.en.md) — `AGPL-3.0-only` first-party scope and third-party license boundaries
- [`../BUILDING.md`](../BUILDING.md) — Rust, web, Tauri desktop, and release build notes
- [`product/`](product/README.en.md) — play-creation product stance

## Architecture and protocol

- [`architecture/`](architecture/README.en.md) — kernel + packages + projects layering, capability contract, extension points, event model, lifecycles
- [`protocol/`](protocol/README.en.md) — public protocol spec
- [`spec/`](spec/README.en.md) — executable v1 contract matrix, conformance roadmap, schemas
- [`architecture/CONSTITUTION_V2.md`](architecture/CONSTITUTION_V2.en.md) → [`spec/CONTRACT_LAYERING_MATRIX.md`](spec/CONTRACT_LAYERING_MATRIX.en.md) — candidate v2 constitution and item-by-item contract ownership; v1 remains current

## Authoring

- [`guides/`](guides/README.en.md) — capability package authoring guides, grouped by domain (foundation / agent / model / inference / experience / memory / storage / external projects / sharing)
- [`guides/CAPABILITY_HANDLES.md`](guides/CAPABILITY_HANDLES.en.md) — v1 capability handles, attenuation, revoke, and effect audit
- [`guides/CONFORMANCE_KIT.md`](guides/CONFORMANCE_KIT.en.md) — third-party package v1 conformance kit
- [`guides/PACKAGE_INSTALLATION.md`](guides/PACKAGE_INSTALLATION.en.md) — `yg install/update`, lockfiles, content-addressed store, bundle freshness, and consent prompts
- [`guides/PROJECT_MODEL.md`](guides/PROJECT_MODEL.en.md) — Home project shelf, `project.yaml`, project lifecycle, console diagnostics/update, and project-level secrets
- [`guides/SECRET_MANAGEMENT.md`](guides/SECRET_MANAGEMENT.en.md) — `secret_ref:env:` / `secret_ref:store:`, local encrypted store, and API key management
- [`guides/REAL_MODEL_END_TO_END.md`](guides/REAL_MODEL_END_TO_END.en.md) — end-to-end path from YdlTavern Send to a real provider response
- [`guides/PATH_B_SELF_CONTAINED.md`](guides/PATH_B_SELF_CONTAINED.en.md) — `entry.contract: "none"` self-contained path
- [`guides/SURFACE_HOSTING.md`](guides/SURFACE_HOSTING.en.md) — `clients/web` iframe SurfaceHost and third-party web surface bundle hosting

## Performance and roadmap

- [`performance/`](performance/README.en.md) — performance baseline, conformance feedback loop, code health
- [`roadmap/`](roadmap/README.en.md) — current and upcoming work, model inference prerequisites
- [`roadmap/CONTRACT_V2_MIGRATION.md`](roadmap/CONTRACT_V2_MIGRATION.en.md) — layered-contract migration order, compatibility gates, and definition of done
- [`tavern/`](tavern/README.en.md) — how Yggdrasil relates to YdlTavern, the SillyTavern-compatible integration project

## Shortest path by intent

| If you want to | Read first |
|---|---|
| Understand the platform stance | [`CHARTER.md`](CHARTER.en.md) → [`architecture/VISION.md`](architecture/VISION.en.md) → [`product/PLAY_CREATION_MODEL.md`](product/PLAY_CREATION_MODEL.en.md) |
| Understand the architecture | [`architecture/ARCHITECTURE.md`](architecture/ARCHITECTURE.en.md) → [`guides/PROJECT_MODEL.md`](guides/PROJECT_MODEL.en.md) → [`architecture/PLATFORM_KERNEL.md`](architecture/PLATFORM_KERNEL.en.md) → [`architecture/CAPABILITY_PACKAGE.md`](architecture/CAPABILITY_PACKAGE.en.md) |
| Review the candidate v2 boundaries | [`architecture/CONSTITUTION_V2.md`](architecture/CONSTITUTION_V2.en.md) → [`spec/CONTRACT_LAYERING_MATRIX.md`](spec/CONTRACT_LAYERING_MATRIX.en.md) → [`roadmap/CONTRACT_V2_MIGRATION.md`](roadmap/CONTRACT_V2_MIGRATION.en.md) |
| Use the public protocol | [`protocol/PROTOCOL_V0.md`](protocol/PROTOCOL_V0.en.md) → [`spec/KERNEL_V1_CONTRACT.md`](spec/KERNEL_V1_CONTRACT.en.md) |
| Write your first package | [`guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](guides/PACKAGE_AUTHORING_WALKTHROUGH.en.md) |
| Install capability packages/projects | [`guides/PACKAGE_INSTALLATION.md`](guides/PACKAGE_INSTALLATION.en.md) → [`guides/PROJECT_MODEL.md`](guides/PROJECT_MODEL.en.md) |
| Manage API keys / secrets | [`guides/SECRET_MANAGEMENT.md`](guides/SECRET_MANAGEMENT.en.md) |
| Run real model calls end to end | [`guides/REAL_MODEL_END_TO_END.md`](guides/REAL_MODEL_END_TO_END.en.md) |
| Host third-party web surfaces | [`guides/SURFACE_HOSTING.md`](guides/SURFACE_HOSTING.en.md) |
| Build web / desktop / release | [`../BUILDING.md`](../BUILDING.md) |
| See current status | [`ALPHA_STATUS.md`](ALPHA_STATUS.en.md) |
| See what's next | [`roadmap/NEXT_STEPS.md`](roadmap/NEXT_STEPS.en.md) |
| Write docs | [`STYLE.md`](STYLE.en.md) |
