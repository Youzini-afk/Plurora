# Authoring guides

> [English](./README.en.md) · [中文](./README.md)

Capability-package authoring guides grouped by domain. Each one targets a class of packages or a slice of the creation loop. All of them build on the public protocol, manifests, and surface contract.

## Getting started

- [`PACKAGE_AUTHORING_WALKTHROUGH.md`](PACKAGE_AUTHORING_WALKTHROUGH.en.md) — third-party package authoring walkthrough (init-package, check, run-fixture, reload, composition)
- [`PACKAGE_INSTALLATION.md`](PACKAGE_INSTALLATION.en.md) — package/project install and update, lockfiles, `~/.plurora` layout, signatures/integrity, bundle freshness, and consent prompts
- [`INSTALLATION_MODEL.md`](INSTALLATION_MODEL.en.md) — Work / Workspace / Installation boundaries, journal/projection, state update/removal, and Installation secret policy
- [`RUN_LIBRARY.md`](RUN_LIBRARY.en.md) — Run journal, `host.run.*` lifecycle, structured gaps, and official Library affordances
- [`POWERBOX_BINDING.md`](POWERBOX_BINDING.en.md) — Exposures, Powerbox candidate disclosure, cross-Installation Bindings, and runtime pins
- [`REALIZATION.md`](REALIZATION.en.md) — pure planning, TargetInventory, `host.realization.*`, effect checkpoints, Docker/Agent backends, rollback, and reconcile
- [`FOREIGN_WORK_RIGHTS.md`](FOREIGN_WORK_RIGHTS.en.md) — ForeignCapsules, local launch bindings, Rights/Transparency, opaque-state backup, and entitlement boundaries
- [`DEPLOYMENT_RUNTIME.md`](DEPLOYMENT_RUNTIME.en.md) — low-level target / exec / port / proxy adapters and reverse proxy used by the Realization executor
- [`HOST_OPERATIONS.md`](HOST_OPERATIONS.en.md) — Host liveness/readiness, offline SQLite backup and atomic restore, and release verification
- [`SECRET_MANAGEMENT.md`](SECRET_MANAGEMENT.en.md) — `secret_ref:env:` / `secret_ref:store:`, local encrypted secret store, and API key management
- [`REAL_MODEL_END_TO_END.md`](REAL_MODEL_END_TO_END.en.md) — complete path from YdlTavern Send to a real model provider response
- [`CAPABILITY_HANDLES.md`](CAPABILITY_HANDLES.en.md) — kernel v1 capability handles, attenuation, revoke, bindings, and effect audit
- [`CONFORMANCE_KIT.md`](CONFORMANCE_KIT.en.md) — local v1 contract compliance validation for third-party packages
- [`PATH_B_SELF_CONTAINED.md`](PATH_B_SELF_CONTAINED.en.md) — self-contained Path B packages (`entry.contract: "none"`)
- [`SURFACE_HOSTING.md`](SURFACE_HOSTING.en.md) — iframe SurfaceHost, third-party web surface bundle contract, and host bridge
- [`ZEABUR_QUICK_VALIDATION.md`](ZEABUR_QUICK_VALIDATION.en.md) — single-container web quick-validation deployment for Zeabur

## Creative capability families

- [`CREATIVE_CAPABILITY_KIT.md`](CREATIVE_CAPABILITY_KIT.en.md) — Plurora-native generic creative capability packages (persona / knowledge / context / text-transform)
- [`MODEL_CONNECTIVITY_KIT.md`](MODEL_CONNECTIVITY_KIT.en.md) — model provider profile and route planning kit
- [`MODEL_PROVIDER_INTEGRATION.md`](MODEL_PROVIDER_INTEGRATION.en.md) — multi-provider model integration (OpenAI, Anthropic, Gemini, OpenAI-compatible, OpenRouter, DeepSeek, xAI, Fireworks)
- [`INFERENCE_CAPABILITY_AUTHORING.md`](INFERENCE_CAPABILITY_AUTHORING.en.md) — transport-neutral inference capability authoring

## Agents and experiences

- [`AGENT_PACKAGE_AUTHORING.md`](AGENT_PACKAGE_AUTHORING.en.md) — agent-like capability package authoring
- [`AGENTIC_FORGE_PACKAGE_AUTHORING.md`](AGENTIC_FORGE_PACKAGE_AUTHORING.en.md) — Agentic Forge runtime packages (plan graph, scratch branch, tool bridge)
- [`EXPERIENCE_RUNTIME_AUTHORING.md`](EXPERIENCE_RUNTIME_AUTHORING.en.md) — experience runtime packages (checkpoint, recovery, agent run binding)
- [`MEMORY_PACKAGE_AUTHORING.md`](MEMORY_PACKAGE_AUTHORING.en.md) — memory / knowledge packages

## Platform extensions

- [`SHARING_DISTRIBUTION.md`](SHARING_DISTRIBUTION.en.md) — sharing and distribution: composition bundles, package-set lockfiles, AI disclosure
- [`STORAGE_BACKEND_NEUTRALITY.md`](STORAGE_BACKEND_NEUTRALITY.en.md) — backend-neutral storage contracts and the first-party lab
- [`POSTGRES_TDB_INTEGRATION.md`](POSTGRES_TDB_INTEGRATION.en.md) — PostgreSQL (event backend) + TDB (retrieval provider) integration
- [`WORKSPACE_OPERATING_PLANE.md`](WORKSPACE_OPERATING_PLANE.en.md) — Workspace ownership, static intake, plan/effect separation, and controlled source changes
