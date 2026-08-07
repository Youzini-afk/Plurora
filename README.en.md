# Plurora

> [English](./README.en.md) · [中文](./README.md)

**An open digital platform for people, AI, and software to create and operate together.**

Plurora enables applications, tools, services, worlds, games, agents, creative environments, and forms not yet named to be created, composed, run, inspected, modified, moved, and replaced. The platform provides trusted common ground without prescribing what upper layers must become.

```text
┌──────────────────────────────────────────────────────────┐
│ Products / Experiences / Services                        │
├──────────────────────────────────────────────────────────┤
│ Distributions / Shells / Clients                         │
├──────────────────────────────────────────────────────────┤
│ Protocol Commons / Components / Content                  │
├──────────────────────────────────────────────────────────┤
│ Constitutional Substrate                                 │
└──────────────────────────────────────────────────────────┘

Host Control Plane / Runtime Fabric crosses the layers to manage
installation, execution, files, secrets, network, targets, deployment,
backup, and diagnostics.
```

## What we are building

Plurora pursues five long-term goals:

- **Open:** public source, protocols, data, and extension points; user export, migration, and deletion; no private official APIs.
- **Plural:** no single application form, workflow, shell, model, component, or content ontology.
- **Advanced:** technology that materially improves freedom, security, performance, and portability rather than novelty for its own sake.
- **Long-lived:** small stable layers, evolvable protocols, readable old data, and migration or retirement for failed abstractions.
- **Usable:** an official distribution that works after installation, keeps simple paths simple, discloses complexity progressively, and recovers from failure.

See [`docs/CHARTER.md`](docs/CHARTER.en.md) for the principles and [`docs/architecture/VISION.md`](docs/architecture/VISION.en.md) for the long-term shape.

## Platform and official product

Plurora is more than a kernel and is not identical to the official Web/Desktop product. The current official distribution uses Home, Settings, Project frames, Console, and contributed surfaces to provide a local managed Host, remote Hosts, installation, operation, creation, deployment, authority, and data management.

Those are evolving default product choices rather than permanent ontology for the whole platform:

- third parties may replace clients, shells, components, protocols, models, and Hosts;
- Project is the current official Host's installation-instance model, not a root object required by every product;
- Home / Play / Forge / Assist belong to optional product profiles rather than the constitutional substrate;
- official components and clients use only public boundaries available to third parties.

See [`docs/product/PLATFORM_PRODUCT_MODEL.md`](docs/product/PLATFORM_PRODUCT_MODEL.en.md) for general product responsibility. Play-creation is an optional opinionated profile described in [`docs/product/PLAY_CREATION_MODEL.md`](docs/product/PLAY_CREATION_MODEL.en.md).

## Current status

The repository is in Foundation Alpha. Contract V1, the Rust Host/runtime, HTTP/RPC/SSE, Package and Component lifecycle, Web/PWA, Tauri Desktop, CLI, installation and updates, Project management, authority, objects and artifacts, model integration, controlled development, targets, and deployment already form a substantial operational surface.

The current implementation still reflects historical Contract V1 aggregation: `platform.*` carries a mixture of substrate, Host, protocol, and shell semantics. The architectural direction is incremental ownership and compatibility rather than a destructive rewrite.

See [`docs/ALPHA_STATUS.md`](docs/ALPHA_STATUS.en.md) for implemented, partial, and deferred state, and [`docs/roadmap/NEXT_STEPS.md`](docs/roadmap/NEXT_STEPS.en.md) for construction direction.

## Repository map

```text
crates/
  plurora-core/            current core types, schemas, identity, events, contracts
  plurora-runtime/         runtime, component execution, dispatch, some Host work
  plurora-service/         HTTP / RPC / SSE and Host service boundary
  plurora-cli/             CLI, Host, scaffolding, contract, conformance tooling

clients/web/           official React 19 + Tailwind v4 + Vite Web shell / PWA
clients/desktop/       Tauri 2.x wrapper + managed Host sidecar

packages/official/     first-party components and experiments via manifests
profiles/              distribution / Host component and policy compositions
examples/              examples, fixtures, and third-party integration samples

sdk/typescript/        TypeScript SDK and subprocess component tooling
sdk/rust/              generated Rust contract SDK
docs/                  charter, architecture, protocols, product, guides, status
integrations/          external project and ecosystem integration research
```

The directory layout describes the current implementation; crate names alone do not determine permanent architectural ownership.

## Quick start

Start a Host:

```bash
cargo run -p plurora-cli -- host serve \
  --http 127.0.0.1:8787 \
  --profile profiles/forge-alpha.yaml
```

Check or build the Web shell:

```bash
npm run check --prefix clients/web
npm run build --prefix clients/web
```

Run tests and conformance:

```bash
cargo test --workspace
cargo run -p plurora-cli -- conformance
```

Install and manage Packages and Projects:

```bash
plurora install github.com/user/plurora-package#v1.2.0
plurora list-installed
plurora project list
plurora project start <project-id>
plurora project stop <project-id>
plurora uninstall <package-id-or-project-id>
plurora update [<package-id>|--project-id <project-id>] [--check-only]
plurora lockfile --check
```

Run the blank play-creation example through public contracts:

```bash
cargo run -p plurora-cli -- play-create-demo
```

See [`docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.en.md) for more commands and component-authoring flow.

## Documentation

Major documents are available in English and Simplified Chinese and link to the other language at the top. The complete index is in [`docs/README.md`](docs/README.en.md).

| Goal | Read first |
|---|---|
| Understand platform goals | [`docs/CHARTER.md`](docs/CHARTER.en.md) → [`docs/architecture/VISION.md`](docs/architecture/VISION.en.md) |
| Understand layering | [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.en.md) → [`docs/architecture/PLATFORM_KERNEL.md`](docs/architecture/PLATFORM_KERNEL.en.md) → [`docs/architecture/CAPABILITY_PACKAGE.md`](docs/architecture/CAPABILITY_PACKAGE.en.md) |
| Understand the official product | [`docs/product/PLATFORM_PRODUCT_MODEL.md`](docs/product/PLATFORM_PRODUCT_MODEL.en.md) → [`docs/design/PLATFORM_UI_DESIGN.md`](docs/design/PLATFORM_UI_DESIGN.en.md) |
| Use public contracts | [`docs/protocol/PROTOCOL_V0.md`](docs/protocol/PROTOCOL_V0.en.md) → [`docs/spec/KERNEL_V1_CONTRACT.md`](docs/spec/KERNEL_V1_CONTRACT.en.md) |
| Review long-term contract layering | [`docs/architecture/CONSTITUTION_V2.md`](docs/architecture/CONSTITUTION_V2.en.md) → [`docs/spec/CONTRACT_LAYERING_MATRIX.md`](docs/spec/CONTRACT_LAYERING_MATRIX.en.md) |
| Write a first Package / Component | [`docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.en.md) |
| Install a Package / Project | [`docs/guides/PACKAGE_INSTALLATION.md`](docs/guides/PACKAGE_INSTALLATION.en.md) → [`docs/guides/PROJECT_MODEL.md`](docs/guides/PROJECT_MODEL.en.md) |
| Manage API keys / secrets | [`docs/guides/SECRET_MANAGEMENT.md`](docs/guides/SECRET_MANAGEMENT.en.md) |
| Build agent / model / experience components | [`docs/guides/AGENT_PACKAGE_AUTHORING.md`](docs/guides/AGENT_PACKAGE_AUTHORING.en.md), [`docs/guides/MODEL_PROVIDER_INTEGRATION.md`](docs/guides/MODEL_PROVIDER_INTEGRATION.en.md), [`docs/guides/EXPERIENCE_RUNTIME_AUTHORING.md`](docs/guides/EXPERIENCE_RUNTIME_AUTHORING.en.md) |
| Host third-party Web surfaces | [`docs/guides/SURFACE_HOSTING.md`](docs/guides/SURFACE_HOSTING.en.md) |
| Review current status | [`docs/ALPHA_STATUS.md`](docs/ALPHA_STATUS.en.md) |
| Review construction direction | [`docs/roadmap/NEXT_STEPS.md`](docs/roadmap/NEXT_STEPS.en.md) |
| Write documentation | [`docs/STYLE.md`](docs/STYLE.en.md) |

## Forms that can grow on Plurora

Chat, world simulation, Tavern, game-engine bridges, IDEs, ordinary Web services, agent runtimes, document tools, node editors, and marketplaces may all become products, protocols, components, or distributions on the platform. None is the platform's only center, and none gains hidden authority by being official.

YdlTavern is an independent integration project; see [`docs/tavern/TAVERN_COMPAT.md`](docs/tavern/TAVERN_COMPAT.en.md) for the boundary.

## License

Plurora is licensed under the GNU Affero General Public License v3.0 only (`AGPL-3.0-only`). See [`LICENSE`](LICENSE). The boundary among first-party code, external dependencies, and third-party content is documented in [`docs/LICENSING.md`](docs/LICENSING.en.md).
