# Yggdrasil

> [English](./README.en.md) · [中文](./README.md)

**An extensible creation platform for AI-native worlds, games, stories, and play.**

It has three tiers: a small, restrained, opinion-free kernel; an open ecosystem of capability packages; and projects on Home that can be installed, started, and stopped. Every meaningful concept on the platform — characters, prompts, models, agents, worlds, rules, memory — comes from a package, not the kernel; projects are host-runtime concepts.

```text
┌──────────────────────────────────────────────┐
│  Web shell · CLI · third-party clients         │   public protocol only
├──────────────────────────────────────────────┤
│  Public protocol  ·  /rpc + SSE              │
├──────────────────────────────────────────────┤
│  Projects (Home cards: YdlTavern / ...)       │   install/start/stop
├──────────────────────────────────────────────┤
│  Capability packages (official = third-party)  │   manifest-driven
├──────────────────────────────────────────────┤
│  Kernel: sessions · events · permissions · ... │   content-free
└──────────────────────────────────────────────┘
```

## Why this exists

Most AI-native creative tools today split their users in two: players who consume a finished experience, and developers who build it. **Yggdrasil refuses that split.**

A player can inspect the session, ask an assistant to change something, fork it, swap one capability package for another, and ship the result back. A creator works against the same public protocol, with the same packages, in the same surfaces. The substrate is the same in both directions.

For the full product stance, see [`docs/product/PLAY_CREATION_MODEL.md`](docs/product/PLAY_CREATION_MODEL.en.md).

## Center of gravity

- The kernel hosts capability packages and nothing else.
- Every meaningful concept comes from a capability package.
- Official packages have no privileges — same manifest, same fabric, same permission gate.
- Creators are free to compose, replace, or write their own packages.

The platform's job is to make radical AI-native creation possible — not to give any "official path" a head start.

## Status

The platform substrate is in place: install/update flows, surface-bundle freshness safeguards, and the project-console diagnostics/update entry are implemented. Contract V1 is the public platform spec; see [`docs/spec/KERNEL_V1_CONTRACT.md`](docs/spec/KERNEL_V1_CONTRACT.en.md). The next stage isn't more substrate sprawl — real playable experiences pull what comes next.

For the detailed status, capability inventory, and partial / deferred items, see [`docs/ALPHA_STATUS.md`](docs/ALPHA_STATUS.en.md). For what's next, see [`docs/roadmap/NEXT_STEPS.md`](docs/roadmap/NEXT_STEPS.en.md).

## Repository layout

```text
crates/                Rust kernel and runtime
  ygg-core/              Kernel-only contracts and content-free types
  ygg-runtime/           Runtime host: sessions, events, packages, capabilities,
                         hooks, surfaces, proposals, assets, branches, projections
  ygg-service/           Public protocol surface (HTTP /rpc, SSE event subscribe)
  ygg-cli/               Host modes, manifest tools, package authoring, conformance

clients/web/           React 19 + Tailwind v4 + Vite platform web shell
clients/desktop/       Tauri 2.x desktop wrapper embedding the web shell

packages/official/     Foundation capability packages loaded through ordinary manifests
profiles/              Host profiles for autoloading sets of packages
examples/              Example package manifests and fixtures

sdk/typescript/        Subprocess-package authoring helpers and domain SDKs
sdk/rust/              Generated Rust kernel SDK
docs/                  Architecture, protocol, spec, roadmap, product docs
integrations/          Upstream research notes (pi, TavernHeadless, pretext, TDB...)
```

## Quick start

Run a host:

```bash
cargo run -p ygg-cli -- host serve \
  --http 127.0.0.1:8787 \
  --profile profiles/forge-alpha.yaml
```

Check or build the web shell:

```bash
npm run check --prefix clients/web
npm run build --prefix clients/web
```

Run the full conformance suite:

```bash
cargo test --workspace
cargo run -p ygg-cli -- conformance
```

Install and manage capability packages:

```bash
yg install github.com/user/yggdrasil-package#v1.2.0
yg list-installed
yg project list
yg project start <project-id>
yg project stop <project-id>
yg uninstall <package-id-or-project-id>
yg update [<package-id>|--project-id <project-id>] [--check-only]
yg lockfile --check
```

Run the blank play-creation loop end to end via the public protocol:

```bash
cargo run -p ygg-cli -- play-create-demo
```

For more commands (manifest, package, composition, host modes, third-party authoring loop, templates), see [`docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.en.md).

## Documentation

Every developer doc has both English and Simplified Chinese versions; the bilingual blockquote at the top of each file switches between them. [`docs/`](docs/README.en.md) is grouped by topic: architecture, protocol, spec, product, package authoring, performance, roadmap, tavern compatibility.

Shortest path by intent:

| If you want to | Read first |
|---|---|
| Understand the platform stance | [`docs/CHARTER.md`](docs/CHARTER.en.md) → [`docs/architecture/VISION.md`](docs/architecture/VISION.en.md) → [`docs/product/PLAY_CREATION_MODEL.md`](docs/product/PLAY_CREATION_MODEL.en.md) |
| Understand the architecture | [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.en.md) → [`docs/architecture/PLATFORM_KERNEL.md`](docs/architecture/PLATFORM_KERNEL.en.md) → [`docs/architecture/CAPABILITY_PACKAGE.md`](docs/architecture/CAPABILITY_PACKAGE.en.md) |
| Use the public protocol | [`docs/protocol/PROTOCOL_V0.md`](docs/protocol/PROTOCOL_V0.en.md) → [`docs/spec/KERNEL_V1_CONTRACT.md`](docs/spec/KERNEL_V1_CONTRACT.en.md) |
| Write your first package | [`docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.en.md) |
| Install capability packages/projects | [`docs/guides/PACKAGE_INSTALLATION.md`](docs/guides/PACKAGE_INSTALLATION.en.md) → [`docs/guides/PROJECT_MODEL.md`](docs/guides/PROJECT_MODEL.en.md) |
| Manage API keys / secrets | [`docs/guides/SECRET_MANAGEMENT.md`](docs/guides/SECRET_MANAGEMENT.en.md) |
| Run real model calls end to end | [`docs/guides/REAL_MODEL_END_TO_END.md`](docs/guides/REAL_MODEL_END_TO_END.en.md) |
| Write agent / model / experience packages | [`docs/guides/AGENT_PACKAGE_AUTHORING.md`](docs/guides/AGENT_PACKAGE_AUTHORING.en.md), [`docs/guides/MODEL_PROVIDER_INTEGRATION.md`](docs/guides/MODEL_PROVIDER_INTEGRATION.en.md), [`docs/guides/EXPERIENCE_RUNTIME_AUTHORING.md`](docs/guides/EXPERIENCE_RUNTIME_AUTHORING.en.md) |
| Host third-party web surfaces | [`docs/guides/SURFACE_HOSTING.md`](docs/guides/SURFACE_HOSTING.en.md) |
| See current status | [`docs/ALPHA_STATUS.md`](docs/ALPHA_STATUS.en.md) |
| See what's next | [`docs/roadmap/NEXT_STEPS.md`](docs/roadmap/NEXT_STEPS.en.md) |
| Write docs | [`docs/STYLE.md`](docs/STYLE.en.md) |

## Deferred

These are valuable directions, but they don't belong in the kernel — they will arrive as ordinary capability packages:

- YdlTavern — a separate integration project compatible with SillyTavern's resources and extensions, running on Yggdrasil ([`docs/tavern/TAVERN_COMPAT.md`](docs/tavern/TAVERN_COMPAT.en.md)).
- Production-grade long-running autonomous agents, multi-agent collaboration, production memory systems, world simulation, directors.
- External game-engine integrations (UE5, Godot, Unity, web clients).
- A full Studio, ComfyUI-style node editors, a marketplace.
- Final UI visual design.

## License

Yggdrasil is licensed under version 3.0 only of the GNU Affero General Public License (`AGPL-3.0-only`). See [`LICENSE`](LICENSE); first-party and third-party boundaries are documented in [`docs/LICENSING.md`](docs/LICENSING.en.md).
