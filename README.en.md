# Plurora

> [English](./README.en.md) · [中文](./README.md)

[![CI](https://github.com/Youzini-afk/Plurora/actions/workflows/ci.yml/badge.svg)](https://github.com/Youzini-afk/Plurora/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/License-AGPL--3.0--only-blue.svg)](./LICENSE)

**An open digital platform for people, AI, and software to create and operate together.**

If Docker is a runtime for containers, think of Plurora as a **runtime for digital works**: applications, tools, worlds, games, agents, creative environments, and forms not yet named can be created, composed, run, inspected, modified, moved, and replaced. The platform provides a public contract and a trusted substrate. The official Web / Desktop client is a replaceable shell, not the whole platform.

The repository is in Foundation Alpha. Host, public contracts, Web/PWA, Desktop, CLI, and runnable authoring examples already form a substantial operational surface. Implementation detail lives in [`docs/ALPHA_STATUS.md`](docs/ALPHA_STATUS.en.md).

![Plurora](clients/web/public/icons/plurora.svg)

## See the UI in five minutes

You need Rust 1.78+, Node.js 20+, and Git. Compiling the Host from source on Windows also needs Visual Studio Build Tools (C++ desktop workload). WebView2 is only required for Desktop builds.

```bash
# Terminal 1: start the Host
cargo run -p plurora-cli -- host serve \
  --http 127.0.0.1:8787 \
  --profile profiles/forge-alpha.yaml

# Terminal 2: start the Web shell
npm ci --prefix clients/web
npm run dev --prefix clients/web
```

Open [http://127.0.0.1:1420](http://127.0.0.1:1420). Full steps, prerequisites, and troubleshooting are in [`docs/guides/GETTING_STARTED.md`](docs/guides/GETTING_STARTED.en.md). Desktop and release builds are in [`BUILDING.md`](BUILDING.md).

## A few terms

| Term | Meaning |
|---|---|
| **Work** | A portable digital work. Content-addressed; not yet an instance on a machine. |
| **Installation** | A Host's record of adopting one Work. Installed does not mean running. |
| **Run** | One explicitly started execution. Opening a detail page does not start or stop it. |
| **Powerbox** | The explicit chooser used when connecting capabilities across Installations, like a permission prompt. |
| **Realization** | Compile a Work into a plan, then apply it to a Target such as Docker or an Agent after approval. The plan itself has no side effects. |
| **Constitutional substrate** | A small mechanism layer: identity, authority, objects, journal, invoke. It does not own chat, games, or any official UI. |
| **Host** | The control plane for install, processes, files, secrets, network, backup, and diagnostics. |
| **Shell** | A client such as Web, Desktop, or CLI. Third parties may replace any of them. |

The causal chain is `Work → Installation → Run`, plus optional Powerbox connections and Realization. None of these steps happens implicitly.

## What we are building

- **Open:** public source, protocols, data, and extension points; user export, migration, and deletion; no private first-party APIs.
- **Plural:** no single application form, workflow, shell, model, or content ontology.
- **Advanced:** technology that materially improves freedom, security, performance, and portability.
- **Long-lived:** small stable layers, evolvable protocols, readable old data, and retirement for failed abstractions.
- **Usable:** an official distribution that works after install, keeps simple paths simple, and recovers from failure.

See [`docs/CHARTER.md`](docs/CHARTER.en.md) for the full principles.

## Platform and official product

Plurora is more than a kernel and is not identical to the official Web/Desktop product. The current official distribution provides Library, Settings, Installation frames, local and remote Hosts, Runs, Powerbox, Realization, authority, and data management. Those are replaceable default product choices.

- Third parties may replace clients, shells, components, protocols, models, and Hosts.
- Work, Installation, and Run do not belong to the constitutional substrate.
- Home / Play / Forge / Assist belong to optional product profiles.
- First-party components and clients use only public boundaries available to third parties.
- Foreign Work can launch opaquely on the local machine after Rights / Transparency disclosure; it still uses the public Host contract.

See [`docs/product/PLATFORM_PRODUCT_MODEL.md`](docs/product/PLATFORM_PRODUCT_MODEL.en.md) for product responsibility. Play-creation is an optional opinionated profile in [`docs/product/PLAY_CREATION_MODEL.md`](docs/product/PLAY_CREATION_MODEL.en.md).

## Repository map

```text
crates/                 Rust Host, runtime, service, CLI
clients/web/            official React 19 + Tailwind v4 + Vite Web shell / PWA
clients/desktop/        Tauri 2.x wrapper + managed Host sidecar
packages/plurora/       first-party components loaded through ordinary manifests
profiles/               Host component and policy assemblies
examples/               examples, fixtures, and third-party samples
sdk/                    TypeScript and generated Rust contract SDKs
docs/                   charter, architecture, protocol, product, guides, status
```

The directory layout describes the current implementation; crate names alone do not determine permanent architectural ownership.

## Common commands

Until the binary is on PATH, invoke the CLI through Cargo:

```bash
cargo run -p plurora-cli -- work check examples/works/modular-simulation --json
cargo run -p plurora-cli -- installation list
cargo run -p plurora-cli -- realization plan --help
cargo run -p plurora-cli -- play-create-demo
cargo run -p plurora-cli -- conformance
```

To install `plurora` onto PATH:

```bash
cargo install --path crates/plurora-cli
```

Most `installation` / `realization` commands need a running Host. Tests:

```bash
cargo test --workspace
npm run check --prefix clients/web
```

## What to read next

The full index is [`docs/README.md`](docs/README.en.md). Major documents are available in English and Simplified Chinese.

| Goal | Read first |
|---|---|
| Run it in five minutes | [`docs/guides/GETTING_STARTED.md`](docs/guides/GETTING_STARTED.en.md) |
| Write a first Package | [`docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.en.md) |
| Understand platform goals | [`docs/CHARTER.md`](docs/CHARTER.en.md) → [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.en.md) |
| Understand the official product | [`docs/product/PLATFORM_PRODUCT_MODEL.md`](docs/product/PLATFORM_PRODUCT_MODEL.en.md) |
| Use public contracts | [`docs/protocol/PUBLIC_PROTOCOL.md`](docs/protocol/PUBLIC_PROTOCOL.en.md) → [`docs/spec/PUBLIC_CONTRACT.md`](docs/spec/PUBLIC_CONTRACT.en.md) |
| Build Web / Desktop / Release | [`BUILDING.md`](BUILDING.md) |
| Contribute | [`CONTRIBUTING.md`](CONTRIBUTING.en.md) |
| Review current implementation facts | [`docs/ALPHA_STATUS.md`](docs/ALPHA_STATUS.en.md) |

Chat, world simulation, Tavern, game-engine bridges, IDEs, ordinary Web services, agent runtimes, document tools, and node editors may all become products or components on the platform. None is the platform's only center. YdlTavern is an independent integration project; see [`docs/tavern/TAVERN_COMPAT.md`](docs/tavern/TAVERN_COMPAT.en.md).

## License

Plurora is licensed under the GNU Affero General Public License v3.0 only (`AGPL-3.0-only`). In plain language:

- You may use it locally, read the source, and modify a private copy.
- If you offer a modified Host or other first-party code to others over a network, you must make the corresponding source available.
- Third-party Packages should declare their own licenses; running on Plurora does not by itself make them AGPL.
- Contributions to first-party code in this repository are accepted under the same license.

See [`LICENSE`](LICENSE) for the full text and [`docs/LICENSING.md`](docs/LICENSING.en.md) for the boundary. This is not legal advice.
